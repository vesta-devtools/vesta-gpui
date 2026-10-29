use std::{error::Error, fmt};
use vesta_gpui::ElementId;
use vulkano::VulkanError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RendererFault {
    DeviceLost,
    SurfaceLost,
    SwapchainOutOfDate,
    ImageUnavailable,
    CompletionTimeout,
    Other,
}

#[derive(Clone, Debug)]
pub struct RendererError {
    pub code: &'static str,
    pub stage: &'static str,
    pub fault: RendererFault,
    pub message: String,
    pub element: Option<ElementId>,
}

impl RendererError {
    pub(crate) fn at(stage: &'static str, error: impl fmt::Display) -> Self {
        Self {
            code: "native_vulkan_failed",
            stage,
            fault: RendererFault::Other,
            message: error.to_string(),
            element: None,
        }
    }

    pub(crate) fn caused_by(stage: &'static str, error: impl Error + 'static) -> Self {
        let mut current: &(dyn Error + 'static) = &error;
        let mut fault = RendererFault::Other;
        let mut message = current.to_string();
        // Vulkano's Validated Display omits the Vulkan cause. Preserve it and
        // classify its type rather than localized or version-specific strings.
        loop {
            if let Some(vulkan) = current.downcast_ref::<VulkanError>() {
                fault = match vulkan {
                    VulkanError::DeviceLost => RendererFault::DeviceLost,
                    VulkanError::SurfaceLost => RendererFault::SurfaceLost,
                    VulkanError::OutOfDate => RendererFault::SwapchainOutOfDate,
                    VulkanError::Timeout | VulkanError::NotReady => RendererFault::ImageUnavailable,
                    _ => RendererFault::Other,
                };
            }
            let Some(cause) = current.source() else { break };
            message.push_str(": ");
            message.push_str(&cause.to_string());
            current = cause;
        }
        let code = match fault {
            RendererFault::DeviceLost => "native_vulkan_device_lost",
            RendererFault::SurfaceLost => "native_vulkan_surface_lost",
            RendererFault::SwapchainOutOfDate => "native_vulkan_swapchain_out_of_date",
            RendererFault::ImageUnavailable => "native_vulkan_image_unavailable",
            RendererFault::CompletionTimeout => "native_vulkan_completion_timeout",
            RendererFault::Other => "native_vulkan_failed",
        };
        Self {
            code,
            stage,
            fault,
            message,
            element: None,
        }
    }
}

impl fmt::Display for RendererError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}; element {:?}): {}",
            self.code, self.stage, self.element, self.message
        )
    }
}

impl Error for RendererError {}

#[cfg(test)]
mod tests {
    use super::*;
    use vulkano::Validated;

    // Prevent loss of machine-readable failure scopes behind Validated's
    // generic Display, and preserve the driver cause for the next recovery slice.
    #[test]
    fn nested_vulkan_errors_preserve_fault_scope_and_underlying_cause() {
        for (error, expected, code) in [
            (
                VulkanError::DeviceLost,
                RendererFault::DeviceLost,
                "native_vulkan_device_lost",
            ),
            (
                VulkanError::SurfaceLost,
                RendererFault::SurfaceLost,
                "native_vulkan_surface_lost",
            ),
            (
                VulkanError::OutOfDate,
                RendererFault::SwapchainOutOfDate,
                "native_vulkan_swapchain_out_of_date",
            ),
            (
                VulkanError::Timeout,
                RendererFault::ImageUnavailable,
                "native_vulkan_image_unavailable",
            ),
            (
                VulkanError::NotReady,
                RendererFault::ImageUnavailable,
                "native_vulkan_image_unavailable",
            ),
        ] {
            let cause = error.to_string();
            let nested = std::io::Error::other(Validated::Error(error));
            let diagnosed = RendererError::caused_by("acquire", nested);
            assert_eq!(diagnosed.fault, expected);
            assert_eq!(diagnosed.code, code);
            assert!(diagnosed.message.contains(&cause));
        }
        // A message containing a Vulkan token is not a typed Vulkan failure.
        let text = RendererError::caused_by("acquire", std::io::Error::other("DeviceLost"));
        assert_eq!(text.fault, RendererFault::Other);
    }
}
