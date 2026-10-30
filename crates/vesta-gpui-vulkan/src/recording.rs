//! Shared command recording for presented and offscreen targets.
use super::*;
use vulkano::command_buffer::PrimaryAutoCommandBuffer;

#[cfg(feature = "viewport")]
pub(super) type SceneFrame<'a> = Option<(
    vesta_viewport::Camera,
    &'a vesta_viewport::ViewportScene,
    vesta_viewport::render::ViewportRect,
)>;

impl WindowRenderer {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn record_commands(
        &mut self,
        framebuffer: Arc<Framebuffer>,
        extent: [u32; 2],
        prepared: Vec<painter::Prepared>,
        output: &mut RenderOutput,
        options: RenderOptions,
        trace: &mut frame_profile::Trace,
        presented: bool,
        #[cfg(feature = "viewport")] scene: &[crate::SceneView<'_>],
    ) -> Result<
        (
            AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
            Option<gpu_profile::PendingGpuTimestamp>,
        ),
        RendererError,
    > {
        checked_pipeline::recording::record(
            self,
            framebuffer,
            extent,
            prepared,
            output,
            options,
            trace,
            presented,
            #[cfg(feature = "viewport")]
            scene,
        )
    }
}
