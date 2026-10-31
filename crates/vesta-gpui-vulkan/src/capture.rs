//! On-demand transfer; completion owns the staging buffer until it can leave the UI thread.
use crate::{RendererError, WindowRenderer};
use std::{
    sync::{
        Arc,
        mpsc::{self, Receiver, SyncSender},
    },
    time::Duration,
};
use vulkano::{
    buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::{
        AutoCommandBufferBuilder, BufferImageCopy, CopyImageToBufferInfo, PrimaryAutoCommandBuffer,
    },
    format::Format,
    image::Image,
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
};
mod offscreen;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_bounds_and_closed_producer_do_not_release_unbounded_or_unowned_requests() {
        CaptureRegion {
            origin: [1, 2],
            extent: [20, 30],
        }
        .validate([100, 100])
        .expect("bounded");
        for region in [
            CaptureRegion {
                origin: [0, 0],
                extent: [0, 1],
            },
            CaptureRegion {
                origin: [0, 0],
                extent: [4097, 1],
            },
            CaptureRegion {
                origin: [0, 0],
                extent: [4096, 4096],
            },
            CaptureRegion {
                origin: [u32::MAX, 0],
                extent: [1, 1],
            },
        ] {
            assert!(region.validate([u32::MAX, u32::MAX]).is_err());
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let mut state = CaptureState {
            queued: Some((
                CaptureRegion {
                    origin: [0, 0],
                    extent: [1, 1],
                },
                sender,
            )),
            recorded: None,
        };
        state.fail(RendererError::at("capture_closed", "owned renderer closed"));
        assert_eq!(
            receiver
                .try_recv()
                .expect("resolved cancellation")
                .expect_err("no fabricated pixels")
                .stage,
            "capture_closed"
        );
        assert!(!state.pending());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureRegion {
    pub origin: [u32; 2],
    pub extent: [u32; 2],
}
impl CaptureRegion {
    pub fn validate(self, target: [u32; 2]) -> Result<(), RendererError> {
        if self.extent.contains(&0)
            || self.extent.iter().any(|n| *n > 4096)
            || u64::from(self.extent[0]) * u64::from(self.extent[1]) > 4_194_304
            || (0..2).any(|axis| {
                self.origin[axis]
                    .checked_add(self.extent[axis])
                    .is_none_or(|end| end > target[axis])
            })
        {
            return Err(RendererError::at(
                "capture_region",
                "invalid, out-of-target or oversized capture",
            ));
        }
        Ok(())
    }
}

/// The producer publishes only after actual GPU completion. Waiting/mapping belongs to a worker.
#[derive(Debug)]
pub struct CaptureTicket(Receiver<Result<CaptureReadback, RendererError>>);
impl CaptureTicket {
    pub fn wait(self, timeout: Duration) -> Result<CaptureReadback, RendererError> {
        self.0.recv_timeout(timeout).map_err(|_| {
            RendererError::at("capture_wait", "capture timed out or renderer closed")
        })?
    }
}

#[derive(Debug)]
pub struct CaptureReadback {
    pub region: CaptureRegion,
    pub frame: u64,
    pub offscreen: bool,
    buffer: Subbuffer<[u8]>,
    format: Format,
}
impl CaptureReadback {
    /// RGBA8/sRGB with the render target's alpha. Map on a worker after completion.
    pub fn rgba(&self) -> Result<Vec<u8>, RendererError> {
        let mut pixels = self
            .buffer
            .read()
            .map_err(|e| RendererError::caused_by("capture_map", e))?
            .to_vec();
        if self.format == Format::B8G8R8A8_SRGB {
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
        }
        Ok(pixels)
    }
}

#[derive(Default)]
pub(super) struct CaptureState {
    queued: Option<(
        CaptureRegion,
        SyncSender<Result<CaptureReadback, RendererError>>,
    )>,
    recorded: Option<(
        CaptureReadback,
        SyncSender<Result<CaptureReadback, RendererError>>,
    )>,
}
impl CaptureState {
    pub fn pending(&self) -> bool {
        self.queued.is_some() || self.recorded.is_some()
    }
    pub fn complete(&mut self) {
        if let Some((readback, sender)) = self.recorded.take() {
            let _ = sender.try_send(Ok(readback));
        }
    }
    pub fn fail(&mut self, error: RendererError) {
        if let Some((_, sender)) = self.queued.take() {
            let _ = sender.try_send(Err(error.clone()));
        }
        if let Some((_, sender)) = self.recorded.take() {
            let _ = sender.try_send(Err(error));
        }
    }
}

impl WindowRenderer {
    pub fn cancel_capture(&mut self) {
        self.capture.fail(RendererError::at(
            "capture_cancelled",
            "capture cancelled by the host",
        ));
    }
    /// Copy a region of the next successfully submitted presentation, never an arbitrary old swapchain image.
    /// A rejected presentation fails this ticket; request a fresh capture after recovery.
    pub fn capture_next_frame(
        &mut self,
        region: CaptureRegion,
    ) -> Result<CaptureTicket, RendererError> {
        region.validate(self.swapchain.image_extent())?;
        if !self
            .swapchain
            .image_usage()
            .contains(vulkano::image::ImageUsage::TRANSFER_SRC)
        {
            return Err(RendererError::at(
                "capture_unsupported",
                "surface images do not support transfer source",
            ));
        }
        self.begin_capture(region)
    }
    pub(super) fn begin_capture(
        &mut self,
        region: CaptureRegion,
    ) -> Result<CaptureTicket, RendererError> {
        if self.capture.pending() {
            return Err(RendererError::at(
                "capture_busy",
                "one capture is already in flight",
            ));
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        self.capture.queued = Some((region, sender));
        Ok(CaptureTicket(receiver))
    }
    pub(super) fn record_capture(
        &mut self,
        builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>,
        image: Arc<Image>,
        frame: u64,
        offscreen: bool,
    ) -> Result<(), RendererError> {
        let Some((region, sender)) = self.capture.queued.take() else {
            return Ok(());
        };
        // Keep the sender installed even when allocation or recording fails.
        self.capture.queued = Some((region, sender));
        region.validate([image.extent()[0], image.extent()[1]])?;
        let memory = Arc::new(StandardMemoryAllocator::new_default(
            self.gpu.device.clone(),
        ));
        let buffer = Buffer::new_slice::<u8>(
            memory,
            BufferCreateInfo {
                usage: BufferUsage::TRANSFER_DST,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_HOST
                    | MemoryTypeFilter::HOST_RANDOM_ACCESS,
                ..Default::default()
            },
            u64::from(region.extent[0]) * u64::from(region.extent[1]) * 4,
        )
        .map_err(|e| RendererError::caused_by("capture_buffer", e))?;
        let mut info = CopyImageToBufferInfo::image_buffer(image.clone(), buffer.clone());
        info.regions = [BufferImageCopy {
            image_subresource: image.subresource_layers(),
            image_offset: [region.origin[0], region.origin[1], 0],
            image_extent: [region.extent[0], region.extent[1], 1],
            ..Default::default()
        }]
        .into();
        builder
            .copy_image_to_buffer(info)
            .map_err(|e| RendererError::caused_by("capture_copy", e))?;
        if let Some((_, sender)) = self.capture.queued.take() {
            self.capture.recorded = Some((
                CaptureReadback {
                    region,
                    frame,
                    offscreen,
                    buffer,
                    format: image.format(),
                },
                sender,
            ));
        }
        Ok(())
    }
}
