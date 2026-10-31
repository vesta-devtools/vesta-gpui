//! Host geometry/sensitivity admission and off-thread PNG replies.
use super::*;
use serde_json::{Value, json};
use std::sync::OnceLock;
use vesta_gpui::LayoutEngine;
use vesta_gpui_vulkan::{CaptureRegion, CaptureTicket};
use vesta_gpui_winit::UiCaptureGeometry;

pub(crate) use vesta_gpui_winit::UiCaptureRequest as Query;
pub(super) struct Binding {
    pub presented: Arc<OnceLock<UiSnapshotId>>,
    viewport: Viewport,
    request: Query,
    geometry: UiCaptureGeometry,
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
        let readback = self.ticket.wait(std::time::Duration::from_secs(10));
        #[cfg(feature = "gpu-fault-injection")]
        frame_reflow_probe::capture_result(
            self.source.window,
            readback.as_ref().map(|_| ()).map_err(|e| e.fault),
        );
        let readback = readback.map_err(|e| e.stage)?;
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
                    .and_then(|snapshot| {
                        binding.request.geometry(&snapshot, layout.viewport()).ok()
                    })
                    .is_none_or(|geometry| geometry.logical != binding.geometry.logical)
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
            UiCaptureGeometry::new(viewport.logical_rect(), viewport)?;
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
        let geometry = query.geometry(&observed, viewport)?;
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
        let pixels = CaptureRegion {
            origin: geometry.origin,
            extent: geometry.extent,
        };
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
        let geometry = query.geometry(&source, viewport)?;
        let pixels = CaptureRegion {
            origin: geometry.origin,
            extent: geometry.extent,
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
        let ticket = renderer.capture_next_frame(pixels).map_err(|e| e.stage)?;
        #[cfg(feature = "gpu-fault-injection")]
        if self
            .frame_reflow_probe
            .as_mut()
            .is_some_and(|probe| probe.capture_requested(query.window))
        {
            renderer.inject_present_out_of_date();
        }
        let presented = Arc::new(OnceLock::new());
        self.ui_query.captures.insert(
            query.window,
            Binding {
                presented: presented.clone(),
                viewport,
                request: query.clone(),
                geometry,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rounded_readback_rejects_adjacent_sensitive_pixels(app: &mut App) {
        let id = app.scopes.gallery.element(500);
        let secret = app.scopes.gallery.element(501);
        let root = Element::container(id, ElementKey::new("capture.root"))
            .styled(Style::default().size(Dimension::Px(32.0), Dimension::Px(16.0)))
            .child(
                Element::container(secret, ElementKey::new("capture.secret"))
                    .styled(
                        Style::default()
                            .absolute(10.0, 0.0)
                            .size(Dimension::Px(2.0), Dimension::Px(16.0)),
                    )
                    .semantic(
                        Semantic::new(SemanticRole::PasswordInput, "credential")
                            .value("never export"),
                    ),
            );
        let viewport = Viewport::new(32.0, 16.0, 1.25);
        let layout = LayoutEngine.compute(&root, viewport).expect("layout");
        let route = ActionRoute::<DemoAction>::build(&root, []).expect("route");
        let snapshot = app
            .capture_query_snapshot(
                STANDALONE_WINDOW,
                app.scopes.gallery,
                &root,
                &layout,
                &route,
                None,
                None,
            )
            .expect("snapshot");
        let query = Query {
            instance: snapshot.id.instance,
            window: snapshot.id.window,
            revision: snapshot.id.revision,
            element: None,
            rect: Some(Rect::new(0.0, 0.0, 9.8, 16.0)),
            viewport: None,
        };
        let safe = Query {
            rect: Some(Rect::new(0.0, 0.0, 9.5, 16.0)),
            ..query.clone()
        };
        let geometry = safe
            .geometry(&snapshot, viewport)
            .expect("initial safe crop");
        app.ui_query
            .publish_root(snapshot, root.clone(), &route, &layout, "test".into());
        assert_eq!(
            app.answer_ui_capture(&query).err(),
            Some("sensitive_region"),
            "Standalone must reject the sensitive rounded pixel before GPU admission"
        );
        app.ui_query.captures.insert(
            STANDALONE_WINDOW,
            Binding {
                presented: Arc::new(OnceLock::new()),
                viewport,
                request: safe,
                geometry,
            },
        );
        assert!(!app.capture_geometry_changed(
            STANDALONE_WINDOW,
            app.scopes.gallery,
            &root,
            &layout,
            &route
        ));
        let mut changed = root;
        changed.children[0].style = changed.children[0].style.clone().absolute(9.0, 0.0);
        let changed_layout = LayoutEngine
            .compute(&changed, viewport)
            .expect("moved redacted control");
        assert!(app.capture_geometry_changed(
            STANDALONE_WINDOW,
            app.scopes.gallery,
            &changed,
            &changed_layout,
            &route
        ));
        app.ui_query.captures.remove(&STANDALONE_WINDOW);
    }

    #[test]
    fn presented_crop_budget_is_independent_of_a_large_existing_window() {
        let mut app = App::new();
        app.ui_query = Store::new(true);
        app.ui_query.control_enabled = true;
        rounded_readback_rejects_adjacent_sensitive_pixels(&mut app);
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
