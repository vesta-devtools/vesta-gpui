//! Owned synthetic scene phases for real offscreen depth-clipping qualification.
use super::*;
use vesta_viewport::display::{
    DisplayOptions, DisplaySources, NumericAtlas, NumericLabel, NumericLabelStyle, NumericLabels,
};
use vesta_viewport::{Camera, Projection, Vec3, ViewportScene};

const PHASES: usize = 8;
const CAPTURE_VIEWPORT: Viewport = Viewport::new(1280.0, 800.0, 1.0);
const CAPTURE_RECT: Rect = Rect::new(624.0, 384.0, 32.0, 32.0);

pub(super) struct Probe {
    next: usize,
    atlas: Arc<NumericAtlas>,
    finishing: Option<(Instant, u64)>,
}

impl Probe {
    pub(super) fn new() -> Self {
        Self {
            next: 0,
            atlas: Arc::new(NumericAtlas {
                width: 100,
                height: 10,
                rgba: vec![255; 100 * 10 * 4],
            }),
            finishing: None,
        }
    }

    fn prepare(
        &self,
        request: &vesta_gpui_winit::UiCaptureRequest,
        camera: &mut Camera,
        scene: &mut ViewportScene,
    ) -> Result<(), &'static str> {
        if self.next >= PHASES
            || request.window != STANDALONE_WINDOW
            || request.viewport != Some(CAPTURE_VIEWPORT)
            || request.rect != Some(CAPTURE_RECT)
            || request.element.is_some()
        {
            return Err("viewport_probe_request");
        }
        *camera = Camera {
            target: Vec3::new(0.0, 0.0, 1.0),
            distance: 1.0,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            near: 1.0,
            far: 8.0,
            projection: if self.next < 4 {
                Projection::Perspective
            } else {
                Projection::Orthographic { vertical_size: 4.0 }
            },
            ..Camera::default()
        };
        let depth = match self.next % 4 {
            2 => 0.5,
            3 => 9.0,
            _ => 4.0,
        };
        *scene = ViewportScene {
            display: DisplayOptions {
                grid: false,
                point_numbers: true,
                ..DisplayOptions::default()
            },
            sources: Some(DisplaySources {
                numeric_labels: Some(NumericLabels {
                    atlas: (self.next % 4 != 0).then(|| self.atlas.clone()),
                    points: vec![NumericLabel {
                        position: [0.0, 0.0, depth],
                        number: 7,
                    }],
                    primitives: Vec::new(),
                    style: NumericLabelStyle {
                        cell_size: [12.0, 12.0],
                        glyph_scale: 1.0,
                        advance_fraction: 0.6,
                        offset: [-6.0, 0.0],
                        max_digits: 1,
                        atlas_cells: 10,
                        color: [1.0; 4],
                        scale_factor: 1.0,
                        depth_tolerance: 0.25,
                        occlusion_fraction: 1.0,
                    },
                }),
                ..DisplaySources::default()
            }),
            ..ViewportScene::default()
        };
        Ok(())
    }

    fn admitted(&mut self, frames: u64) {
        let _trace = vesta_gpui::trace::span("fixture", "viewport.capture_admitted");
        vesta_gpui::trace::counter("fixture", "viewport.phase", self.next as f64);
        vesta_gpui::trace::counter(
            "fixture",
            "viewport.projection",
            if self.next >= 4 { 1.0 } else { 0.0 },
        );
        self.next += 1;
        if self.next == PHASES {
            self.finishing = Some((Instant::now(), frames));
        }
    }

    pub(super) fn advance(&mut self, app: &mut App) -> bool {
        if let Some((since, frames)) = self.finishing
            && since.elapsed() >= Duration::from_millis(500)
            && app.rendered_frames.saturating_sub(frames) >= 2
        {
            let _trace = vesta_gpui::trace::span("fixture", "viewport.complete");
            vesta_gpui::trace::counter("fixture", "viewport.captures", self.next as f64);
            vesta_gpui::trace::counter(
                "fixture",
                "viewport.later_presentations",
                app.rendered_frames.saturating_sub(frames) as f64,
            );
            eprintln!(
                "Viewport scene workload complete: captures=8 later_main_presentations={} physical_input=false",
                app.rendered_frames.saturating_sub(frames)
            );
            return true;
        }
        app.redraw();
        false
    }
}

impl App {
    pub(crate) fn prepare_viewport_capture(
        &mut self,
        request: &vesta_gpui_winit::UiCaptureRequest,
    ) -> Result<(), &'static str> {
        if let Some(probe) = &mut self.frame_reflow_probe
            && let Workload::Viewport(probe) = &mut probe.workload
        {
            probe.prepare(request, &mut self.camera, &mut self.scene)?;
        }
        Ok(())
    }

    pub(crate) fn viewport_capture_admitted(&mut self) {
        if let Some(probe) = &mut self.frame_reflow_probe
            && let Workload::Viewport(probe) = &mut probe.workload
        {
            probe.admitted(self.rendered_frames);
        }
    }
}
