//! Narrow owned-device calibration FFI. No query runs without a traced sample.
use std::sync::Arc;
use vesta_gpui::trace::gpu::{GpuCalibration, GpuCalibrationFailure, GpuCalibrationSupport};
use vulkano::device::Device;

pub(crate) struct CalibratedClock {
    device: Arc<Device>,
    support: GpuCalibrationSupport,
}
impl CalibratedClock {
    pub(crate) fn new(device: &Arc<Device>) -> Self {
        Self {
            device: device.clone(),
            support: support(device),
        }
    }
    pub(crate) fn support(&self) -> GpuCalibrationSupport {
        self.support
    }
    pub(crate) fn sample(&self) -> Result<GpuCalibration, GpuCalibrationFailure> {
        if !self.support.available() {
            return Err(GpuCalibrationFailure::InvalidMetadata);
        }
        sample(&self.device)
    }
}
fn support(device: &Arc<Device>) -> GpuCalibrationSupport {
    #[cfg(not(windows))]
    {
        let _ = device;
        GpuCalibrationSupport::UnsupportedPlatform
    }
    #[cfg(windows)]
    {
        use ash::vk;
        use vulkano::VulkanObject;
        let fns = device.physical_device().instance().fns();
        let (function, support) = if device.enabled_extensions().khr_calibrated_timestamps {
            (
                fns.khr_calibrated_timestamps
                    .get_physical_device_calibrateable_time_domains_khr,
                GpuCalibrationSupport::VulkanKhrQpc,
            )
        } else if device.enabled_extensions().ext_calibrated_timestamps {
            (
                fns.ext_calibrated_timestamps
                    .get_physical_device_calibrateable_time_domains_ext,
                GpuCalibrationSupport::VulkanExtQpc,
            )
        } else {
            return GpuCalibrationSupport::UnsupportedExtension;
        };
        let mut domains = [vk::TimeDomainKHR::DEVICE; 16];
        let mut count = domains.len() as u32;
        // SAFETY: the matching extension is enabled; the owned physical device
        // retains its instance. The output/count point to live bounded storage.
        let result = unsafe {
            function(
                device.physical_device().handle(),
                &mut count,
                domains.as_mut_ptr(),
            )
        };
        if result != vk::Result::SUCCESS || count as usize > domains.len() {
            return GpuCalibrationSupport::CapabilityQueryFailed;
        }
        let domains = &domains[..count as usize];
        if !domains.contains(&vk::TimeDomainKHR::DEVICE)
            || !domains.contains(&vk::TimeDomainKHR::QUERY_PERFORMANCE_COUNTER)
        {
            return GpuCalibrationSupport::UnsupportedHostDomain;
        }
        support
    }
}
fn sample(device: &Arc<Device>) -> Result<GpuCalibration, GpuCalibrationFailure> {
    #[cfg(not(windows))]
    {
        let _ = device;
        Err(GpuCalibrationFailure::QueryFailed)
    }
    #[cfg(windows)]
    {
        use ash::vk;
        use vulkano::VulkanObject;
        use windows_sys::Win32::System::Performance::{
            QueryPerformanceCounter, QueryPerformanceFrequency,
        };
        let function = if device.enabled_extensions().khr_calibrated_timestamps {
            device
                .fns()
                .khr_calibrated_timestamps
                .get_calibrated_timestamps_khr
        } else if device.enabled_extensions().ext_calibrated_timestamps {
            device
                .fns()
                .ext_calibrated_timestamps
                .get_calibrated_timestamps_ext
        } else {
            return Err(GpuCalibrationFailure::QueryFailed);
        };
        let infos = [
            vk::CalibratedTimestampInfoKHR::default().time_domain(vk::TimeDomainKHR::DEVICE),
            vk::CalibratedTimestampInfoKHR::default()
                .time_domain(vk::TimeDomainKHR::QUERY_PERFORMANCE_COUNTER),
        ];
        let mut ticks = [0u64; 2];
        let mut deviation = 0u64;
        let (mut frequency, mut before, mut after) = (0i64, 0i64, 0i64);
        // SAFETY: each Windows API receives a writable live i64. QPC has no
        // handle lifetime or external synchronization requirement.
        if unsafe { QueryPerformanceFrequency(&mut frequency) } == 0 || frequency <= 0 {
            return Err(GpuCalibrationFailure::HostClockFailed);
        }
        // SAFETY: before is writable live storage for the same handle-free QPC.
        if unsafe { QueryPerformanceCounter(&mut before) } == 0 || before < 0 {
            return Err(GpuCalibrationFailure::HostClockFailed);
        }
        // SAFETY: support() verified both domains for this owned device; its
        // enabled extension selects this function table. Structs are initialized,
        // have null pNext and unique domains; all outputs have matching capacity.
        let result = unsafe {
            function(
                device.handle(),
                infos.len() as u32,
                infos.as_ptr(),
                ticks.as_mut_ptr(),
                &mut deviation,
            )
        };
        // SAFETY: after is live writable storage for the same monotonic QPC.
        if unsafe { QueryPerformanceCounter(&mut after) } == 0 || after < before {
            return Err(GpuCalibrationFailure::HostClockFailed);
        }
        if result != vk::Result::SUCCESS {
            return Err(GpuCalibrationFailure::QueryFailed);
        }
        Ok(GpuCalibration {
            device_ticks: ticks[0],
            host_ticks: ticks[1],
            host_before_ticks: before as u64,
            host_after_ticks: after as u64,
            host_frequency_hz: frequency as u64,
            cpu_before_ns: 0,
            cpu_after_ns: 0,
            max_deviation_ns: deviation,
        })
    }
}
