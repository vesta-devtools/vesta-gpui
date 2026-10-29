//! Host geometry/sensitivity admission and off-thread PNG replies.
use super::*;
use serde_json::{Value, json};
use std::sync::OnceLock;
use vesta_gpui::LayoutEngine;
use vesta_gpui_vulkan::{CaptureRegion, CaptureTicket};

pub(crate) use vesta_gpui_winit::UiCaptureRequest as Query;
pub(super) struct Binding {
    pub presented: Arc<OnceLock<UiSnapshotId>>,
    viewport: Viewport,
    element: Option<ElementId>,
    rect: Rect,
}
#[derive(Debug)]
pub(crate) struct Reply {
    ticket: CaptureTicket,
    source: UiSnapshotId,
    presented: Arc<OnceLock<UiSnapshotId>>,
    viewport: Viewport,
}

pub(super) fn parse(value: &Value) -> Result<Operation, &'static str> {
    Query::parse(value).map(Operation::Capture)
}

impl Reply {
    pub(super) fn serialize(self) -> Result<Value, &'static str> {
        use base64::Engine;
        use image::ImageEncoder;
        let readback = self
            .ticket
            .wait(std::time::Duration::from_secs(10))
            .map_err(|e| e.stage)?;
        let pixels = readback.rgba().map_err(|e| e.stage)?;
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(
                &pixels,
                readback.region.extent[0],
                readback.region.extent[1],
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|_| "capture_encode")?;
        let snapshot = if readback.offscreen {
            self.source
        } else {
            let deadline = Instant::now() + std::time::Duration::from_secs(1);
            while self.presented.get().is_none() && Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            *self
                .presented
                .get()
                .ok_or("capture_presentation_unavailable")?
        };
        Ok(
            json!({"mode":if readback.offscreen {"offscreen"} else {"next_presented"},"instance":snapshot.instance.to_string(),"window":snapshot.window.0.to_string(),
            "revision":snapshot.revision.to_string(),"renderer_frame":readback.frame.to_string(),
            "viewport":{"width":self.viewport.width,"height":self.viewport.height,"scale_factor":self.viewport.scale_factor},
            "region":{"x":readback.region.origin[0],"y":readback.region.origin[1],"width":readback.region.extent[0],"height":readback.region.extent[1]},
            "format":"png","color_space":"srgb","encoding":"base64","data":base64::engine::general_purpose::STANDARD.encode(png),
            "resource_context":if readback.offscreen {"current_host_resources"}else{"presented_frame"},
            "coverage":{"gpui_render_target":true,"native_child_surfaces":false,"os_compositor":false}}),
        )
    }
}

fn region(rect: Rect, viewport: Viewport) -> Result<CaptureRegion, &'static str> {
    if !viewport.width.is_finite()
        || !viewport.height.is_finite()
        || !viewport.scale_factor.is_finite()
        || viewport.width <= 0.0
        || viewport.height <= 0.0
        || !(0.25..=4.0).contains(&viewport.scale_factor)
        || rect.width <= 0.0
        || rect.height <= 0.0
    {
        return Err("invalid_capture_region");
    }
    let rect = rect
        .intersection(viewport.logical_rect())
        .ok_or("offscreen_target")?;
    let scale = viewport.scale_factor;
    let origin = [
        (rect.x * scale).floor().max(0.0) as u32,
        (rect.y * scale).floor().max(0.0) as u32,
    ];
    let end = [
        ((rect.x + rect.width) * scale).ceil() as u32,
        ((rect.y + rect.height) * scale).ceil() as u32,
    ];
    let region = CaptureRegion {
        origin,
        extent: [
            end[0].saturating_sub(origin[0]),
            end[1].saturating_sub(origin[1]),
        ],
    };
    region
        .validate([
            (viewport.width * scale).ceil() as u32,
            (viewport.height * scale).ceil() as u32,
        ])
        .map_err(|_| "invalid_capture_region")?;
    Ok(region)
}

impl App {
    pub(crate) fn capture_geometry_changed(
        &self,
        window: UiWindowId,
        gallery: ElementScope,
        root: &Element,
        layout: &LayoutTree,
        route: &ActionRoute<DemoAction>,
    ) -> bool {
        self.ui_query.captures.get(&window).is_some_and(|binding| {
            binding.viewport != layout.viewport()
                || self
                    .capture_query_snapshot(window, gallery, root, layout, route, None, None)
                    .is_none_or(|snapshot| reject_sensitive(&snapshot, binding.rect).is_err())
                || binding.element.is_some_and(|id| {
                    layout
                        .node(id)
                        .and_then(|n| n.rect.intersection(n.inherited_clip))
                        .and_then(|r| r.intersection(layout.viewport().logical_rect()))
                        != Some(binding.rect)
                })
        })
    }

    pub(super) fn answer_ui_capture(&mut self, query: &Query) -> Result<Reply, &'static str> {
        if !self.ui_query.control_enabled {
            return Err("control_disabled");
        }
        if query.instance != self.ui_query.instance {
            return Err("foreign_instance");
        }
        if self.ui_query.dirty.contains(&query.window) {
            return Err("presentation_pending");
        }
        let latest = self
            .ui_query
            .latest
            .get(&query.window)
            .cloned()
            .ok_or("not_ready")?;
        let source = if latest.id.revision == query.revision {
            latest.clone()
        } else {
            self.ui_query
                .pinned
                .iter()
                .find(|snapshot| {
                    snapshot.id.window == query.window && snapshot.id.revision == query.revision
                })
                .cloned()
                .ok_or("stale_snapshot")?
        };
        let viewport = query.viewport.unwrap_or(source.viewport);
        let offscreen_layout;
        let root = self
            .ui_query
            .pinned_roots
            .get(&(query.window, query.revision))
            .cloned()
            .or_else(|| {
                (source.id == latest.id)
                    .then(|| self.ui_query.roots.get(&query.window).cloned())
                    .flatten()
            })
            .ok_or("capture_source_unavailable")?;
        let observed = if query.viewport.is_some() {
            // Relayout allocates a new target; presented capture only reads its requested crop.
            region(viewport.logical_rect(), viewport)?;
            // Relayout the same immutable realized tree; this does not realize unloaded model data.
            offscreen_layout = LayoutEngine
                .compute_with_text_context(&root, viewport, &mut self.text_context)
                .map_err(|_| "capture_layout")?;
            let scopes = self.window_scopes(query.window).ok_or("window_not_found")?;
            self.capture_query_snapshot(
                query.window,
                scopes.gallery,
                &root,
                &offscreen_layout,
                self.window_actions(query.window)
                    .ok_or("window_not_found")?,
                None,
                self.focus_root_for_window(query.window),
            )
            .ok_or("capture_layout")?
        } else {
            return self.capture_presented(query, source, viewport);
        };
        let rect = requested_rect(query, &observed, viewport)?;
        reject_sensitive(&observed, rect)?;
        let paint = PaintList::build_with_focus(
            &root,
            &offscreen_layout,
            source.focus,
            self.theme.palette.focus,
        );
        let options = vesta_gpui_vulkan::RenderOptions {
            clear: theme_clear(
                self.theme,
                query.window == STANDALONE_WINDOW && self.mode == AcceptanceMode::Viewport3d,
            ),
            selection: self.theme.palette.selection,
            caret: self.theme.palette.focus,
            caret_visible: false,
        };
        let state = if query.window == STANDALONE_WINDOW {
            self.gpu.as_mut().ok_or("capture_renderer_unavailable")?
        } else {
            &mut self
                .native_windows
                .values_mut()
                .find(|h| h.ui_window == query.window)
                .ok_or("window_not_found")?
                .surface
        };
        let renderer = state
            .capture_renderer()
            .map_err(|_| "capture_renderer_unavailable")?;
        let pixels = region(rect, viewport)?;
        let ticket = renderer
            .capture_offscreen_with_scene(
                viewport,
                &paint,
                &mut self.text_context,
                &mut self.text_rasterizer,
                Some(&DemoImages::for_frame(self.rendered_frames)),
                options,
                pixels,
                (query.window == STANDALONE_WINDOW && self.mode == AcceptanceMode::Viewport3d)
                    .then_some((
                        self.camera,
                        &self.scene,
                        vesta_viewport::render::ViewportRect::full(
                            (viewport.width * viewport.scale_factor).ceil() as u32,
                            (viewport.height * viewport.scale_factor).ceil() as u32,
                        ),
                    )),
            )
            .map_err(|e| e.stage)?;
        self.request_all_redraw();
        Ok(Reply {
            ticket,
            source: source.id,
            presented: Arc::new(OnceLock::new()),
            viewport,
        })
    }

    fn capture_presented(
        &mut self,
        query: &Query,
        source: Arc<UiSnapshot>,
        viewport: Viewport,
    ) -> Result<Reply, &'static str> {
        let rect = requested_rect(query, &source, viewport)?;
        let pixels = region(rect, viewport)?;
        reject_sensitive(&source, rect)?;
        let state = if query.window == STANDALONE_WINDOW {
            self.gpu.as_mut().ok_or("capture_renderer_unavailable")?
        } else {
            &mut self
                .native_windows
                .values_mut()
                .find(|h| h.ui_window == query.window)
                .ok_or("window_not_found")?
                .surface
        };
        let ticket = state
            .capture_renderer()
            .map_err(|_| "capture_renderer_unavailable")?
            .capture_next_frame(pixels)
            .map_err(|e| e.stage)?;
        let presented = Arc::new(OnceLock::new());
        self.ui_query.captures.insert(
            query.window,
            Binding {
                presented: presented.clone(),
                viewport,
                element: query.element,
                rect,
            },
        );
        self.request_all_redraw();
        Ok(Reply {
            ticket,
            source: source.id,
            presented,
            viewport,
        })
    }
}

fn requested_rect(
    query: &Query,
    snapshot: &UiSnapshot,
    viewport: Viewport,
) -> Result<Rect, &'static str> {
    match query.element {
        Some(element) => snapshot
            .nodes()
            .iter()
            .find(|n| n.id == element)
            .ok_or("target_not_captured")?
            .visible_bounds
            .ok_or("offscreen_target"),
        None => Ok(query.rect.unwrap_or(viewport.logical_rect())),
    }
}
fn reject_sensitive(snapshot: &UiSnapshot, rect: Rect) -> Result<(), &'static str> {
    if snapshot.node_limit_reached || snapshot.depth_limit_reached {
        return Err("capture_incomplete_source");
    }
    if snapshot.nodes().iter().any(|n| {
        (n.value.redacted || n.text.redacted || n.name.redacted)
            && n.visible_bounds
                .is_some_and(|bounds| bounds.intersection(rect).is_some())
    }) {
        return Err("sensitive_region");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presented_crop_budget_is_independent_of_a_large_existing_window() {
        let mut app = App::new();
        app.ui_query = Store::new(true);
        app.ui_query.control_enabled = true;
        crate::tests::activate_gallery_page(&mut app, "gallery.inputs");
        let root = crate::tests::install_action_route(&mut app);
        let viewport = Viewport::new(1720.0, 672.0, 2.0);
        let layout = LayoutEngine.compute(&root, viewport).expect("layout");
        let snapshot = app
            .capture_query_snapshot(
                STANDALONE_WINDOW,
                app.scopes.gallery,
                &root,
                &layout,
                &app.ui_actions,
                None,
                None,
            )
            .expect("snapshot");
        let sensitive = snapshot
            .nodes()
            .iter()
            .find(|node| node.value.redacted && node.visible_bounds.is_some())
            .expect("realized password field")
            .id;
        let query = Query {
            instance: snapshot.id.instance,
            window: snapshot.id.window,
            revision: snapshot.id.revision,
            element: Some(app.scopes.gallery.element(TEXT_FIELD)),
            rect: None,
            viewport: None,
        };
        app.ui_query
            .publish_root(snapshot, root, &app.ui_actions, &layout, "test".into());
        assert_eq!(
            app.answer_ui_capture(&query).err(),
            Some("capture_renderer_unavailable"),
            "a small presented crop passes geometry admission before this headless Host lacks a GPU"
        );
        assert_eq!(
            app.answer_ui_capture(&Query {
                element: Some(sensitive),
                ..query.clone()
            })
            .err(),
            Some("sensitive_region"),
            "allowing a small crop must not bypass password redaction"
        );
        assert_eq!(
            app.answer_ui_capture(&Query {
                element: None,
                ..query.clone()
            })
            .err(),
            Some("invalid_capture_region"),
            "a requested whole image still exceeds the readback budget"
        );
        assert_eq!(
            app.answer_ui_capture(&Query {
                viewport: Some(viewport),
                ..query
            })
            .err(),
            Some("invalid_capture_region"),
            "offscreen allocation still budgets the whole requested target"
        );
    }
}
