//! Bounded borrowed views; one shared depth target with disjoint visible regions.
use crate::{RendererError, RendererFault};
use vesta_viewport::{Camera, ViewportScene, render::ViewportRect};

pub const MAX_SCENE_VIEWS: usize = 32;

/// An independent camera/scene projection in physical target pixels.
/// viewport must fit in the target. clip restricts pixels without changing aspect.
#[derive(Clone, Copy, Debug)]
pub struct SceneView<'a> {
    pub camera: Camera,
    pub scene: &'a ViewportScene,
    pub viewport: ViewportRect,
    pub clip: ViewportRect,
}
impl<'a> SceneView<'a> {
    pub fn new(camera: Camera, scene: &'a ViewportScene, viewport: ViewportRect) -> Self {
        Self {
            camera,
            scene,
            viewport,
            clip: viewport,
        }
    }
}
const REJECTED: &str = "native_vulkan_scene_batch_rejected";
fn rejected(message: impl Into<String>) -> RendererError {
    RendererError {
        code: REJECTED,
        stage: "scene_batch",
        fault: RendererFault::Other,
        message: message.into(),
        element: None,
    }
}
pub(crate) fn input_rejected(error: &RendererError) -> bool {
    error.code == REJECTED || error.stage == "frame_scene_mode"
}

/// Validate the same bounded CPU plan consumed by presented and offscreen frames.
/// Visible clips must be pairwise disjoint. Edge touching is admitted.
pub fn validate_scene_views(
    views: &[SceneView<'_>],
    extent: [u32; 2],
) -> Result<(), RendererError> {
    SceneInput::Batch(views).prepare(extent).map(|_| ())
}

pub(crate) type SceneFrame<'a> = Option<(Camera, &'a ViewportScene, ViewportRect)>;

#[derive(Clone, Copy)]
pub(crate) enum SceneInput<'a> {
    Single(SceneFrame<'a>),
    Batch(&'a [SceneView<'a>]),
}
fn intersection(a: ViewportRect, b: ViewportRect) -> Option<ViewportRect> {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let right = a.x.saturating_add(a.width).min(b.x.saturating_add(b.width));
    let bottom =
        a.y.saturating_add(a.height)
            .min(b.y.saturating_add(b.height));
    (right > x && bottom > y).then(|| ViewportRect {
        x,
        y,
        width: right - x,
        height: bottom - y,
    })
}
impl<'a> SceneInput<'a> {
    pub(crate) fn is_present(self) -> bool {
        match self {
            Self::Single(scene) => scene.is_some(),
            Self::Batch(views) => !views.is_empty(),
        }
    }
    pub(crate) fn prepare(self, extent: [u32; 2]) -> Result<Vec<SceneView<'a>>, RendererError> {
        let legacy;
        let source = match self {
            Self::Single(scene) => {
                legacy = scene.map(|(camera, data, rect)| {
                    // Preserve the legacy single-scene clamp before deriving camera aspect.
                    let rect = ViewportRect {
                        x: rect.x.min(extent[0]),
                        y: rect.y.min(extent[1]),
                        width: rect.width.min(extent[0].saturating_sub(rect.x)),
                        height: rect.height.min(extent[1].saturating_sub(rect.y)),
                    };
                    SceneView::new(camera, data, rect)
                });
                legacy.as_slice()
            }
            Self::Batch(views) => views,
        };
        if source.len() > MAX_SCENE_VIEWS {
            return Err(rejected("scene view limit exceeded"));
        }
        let mut prepared: Vec<SceneView<'a>> = Vec::with_capacity(source.len());
        for (index, source) in source.iter().enumerate() {
            let rect = source.viewport;
            let right = rect.x.checked_add(rect.width).filter(|v| *v <= extent[0]);
            let bottom = rect.y.checked_add(rect.height).filter(|v| *v <= extent[1]);
            if right.is_none()
                || bottom.is_none()
                || source.clip.x.checked_add(source.clip.width).is_none()
                || source.clip.y.checked_add(source.clip.height).is_none()
            {
                return Err(rejected(format!(
                    "view {index}: viewport outside target or rectangle overflow"
                )));
            }
            let Some(clip) = intersection(rect, source.clip) else {
                continue;
            };
            let aspect = rect.width as f32 / rect.height as f32;
            if !vesta_viewport::render::camera_uniform(source.camera, aspect)
                .iter()
                .flatten()
                .all(|v| v.is_finite())
                || !vesta_viewport::render::view_projection(source.camera, aspect)
                    .iter()
                    .flatten()
                    .all(|v| v.is_finite())
            {
                return Err(rejected(format!(
                    "view {index}: non-finite camera projection"
                )));
            }
            if prepared
                .iter()
                .any(|previous| intersection(previous.clip, clip).is_some())
            {
                return Err(rejected(format!(
                    "view {index}: visible scene clips overlap"
                )));
            }
            prepared.push(SceneView { clip, ..*source });
        }
        Ok(prepared)
    }
}
