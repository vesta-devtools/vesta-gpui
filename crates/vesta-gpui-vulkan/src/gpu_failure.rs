//! A logical device generation never becomes healthy again. ADR-0328.
use crate::{RendererError, RendererFault};
use std::sync::OnceLock;

#[derive(Default)]
pub(crate) struct GpuFailure(OnceLock<RendererError>);

impl GpuFailure {
    pub fn record(&self, error: &RendererError) {
        if error.fault == RendererFault::DeviceLost {
            let _ = self.0.set(error.clone());
        }
    }
    pub fn error(&self) -> Option<&RendererError> {
        self.0.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use vulkano::VulkanError;

    #[test]
    fn surface_failure_stays_local_and_first_device_failure_reaches_all_windows() {
        let context = Arc::new(GpuFailure::default());
        let second_window = context.clone();
        context.record(&RendererError::caused_by(
            "present",
            VulkanError::SurfaceLost,
        ));
        assert!(second_window.error().is_none());
        context.record(&RendererError::caused_by("submit", VulkanError::DeviceLost));
        second_window.record(&RendererError::caused_by(
            "acquire",
            VulkanError::DeviceLost,
        ));
        assert_eq!(context.error().expect("device failure").stage, "submit");
        assert_eq!(
            second_window.error().expect("shared failure").fault,
            RendererFault::DeviceLost
        );
    }
}
