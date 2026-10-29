//! Read-only platform placement query. No window creation, ownership or mutation.
use winit::{dpi::PhysicalPosition, monitor::MonitorHandle};

/// Physical desktop bounds of the monitor's usable work area. The tuple is
/// (left, top, width, height); negative desktop origins remain intact.
/// Windows excludes taskbars. Other platforms currently use monitor bounds.
#[must_use]
pub fn popup_monitor_bounds(monitor: &MonitorHandle) -> (PhysicalPosition<i32>, [u32; 2]) {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITORINFO};
        use winit::platform::windows::MonitorHandleExtWindows;
        let mut info: MONITORINFO = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        // SAFETY: winit supplies a live monitor handle; info is a writable,
        // correctly sized value buffer. This only reads monitor geometry.
        if unsafe { GetMonitorInfoW(monitor.hmonitor() as _, &mut info) } != 0 {
            let r = info.rcWork;
            if r.right > r.left && r.bottom > r.top {
                return (
                    PhysicalPosition::new(r.left, r.top),
                    [(r.right - r.left) as u32, (r.bottom - r.top) as u32],
                );
            }
        }
    }
    let size = monitor.size();
    (monitor.position(), [size.width, size.height])
}
