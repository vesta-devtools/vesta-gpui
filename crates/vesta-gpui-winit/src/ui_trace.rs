//! Optional native-host capture launch and post-event-loop export.
use std::{error::Error, ffi::OsString, fmt, fs::OpenOptions, io::Write, path::PathBuf};
use vesta_gpui::trace::{
    TraceCapture, TraceConfig, TraceDetail, TraceError, TraceSnapshot, TraceStats,
};

#[derive(Debug)]
pub enum UiTraceError {
    Argument,
    Capture(TraceError),
    Io(std::io::Error),
    Export(serde_json::Error),
}
impl fmt::Display for UiTraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Argument => {
                f.write_str("gpui_trace_argument: use one --trace-ui=<new-output.json|html>")
            }
            Self::Capture(error) => write!(f, "{error}"),
            Self::Io(error) => write!(f, "gpui_trace_io: {error}"),
            Self::Export(error) => write!(f, "gpui_trace_export: {error}"),
        }
    }
}
impl Error for UiTraceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Argument => None,
            Self::Capture(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::Export(error) => Some(error),
        }
    }
}

/// Shared `--trace-ui=<new-output.json|html>` launch convention. Ordinary launch
/// does not capture or touch files. The default capture budget is 30s / 100k
/// events; expiration does not stop the application. Export only after the host
/// event loop returns (or explicitly on a host-owned background I/O task).
pub struct UiTraceRecording {
    active: Option<(TraceCapture, PathBuf)>,
}

/// Frozen capture and its explicit destination. Send this owned value to the
/// host's I/O worker; it has no connection to a successor capture or live UI.
pub struct UiTraceExport {
    snapshot: TraceSnapshot,
    path: PathBuf,
}

fn validate_destination(path: &std::path::Path) -> Result<(), UiTraceError> {
    if !path
        .extension()
        .is_some_and(|ext| ext == "json" || ext == "html")
    {
        return Err(UiTraceError::Argument);
    }
    if path.try_exists().map_err(UiTraceError::Io)? {
        return Err(UiTraceError::Io(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "trace destination already exists",
        )));
    }
    Ok(())
}

impl UiTraceRecording {
    /// On-demand recording; no capture is started until this call succeeds.
    pub fn start(
        path: impl Into<PathBuf>,
        config: TraceConfig,
        detail: TraceDetail,
    ) -> Result<Self, UiTraceError> {
        let path = path.into();
        validate_destination(&path)?;
        Ok(Self {
            active: Some((
                TraceCapture::start_with_detail(config, detail).map_err(UiTraceError::Capture)?,
                path,
            )),
        })
    }

    pub fn stats(&self) -> Option<TraceStats> {
        self.active.as_ref().map(|(capture, _)| capture.stats())
    }

    /// Freeze without JSON serialization or file I/O. The host owns worker lifetime.
    pub fn stop(self) -> Option<UiTraceExport> {
        self.active.map(|(capture, path)| UiTraceExport {
            snapshot: capture.stop(),
            path,
        })
    }

    /// Unrelated host arguments are ignored. Duplicate/empty destinations and
    /// existing paths are rejected before starting a capture. No output is
    /// created until finish; the final create_new also rejects a later race.
    pub fn from_arguments(
        arguments: impl IntoIterator<Item = impl Into<OsString>>,
    ) -> Result<Self, UiTraceError> {
        let mut output = None;
        for argument in arguments {
            let argument: OsString = argument.into();
            let Some(argument) = argument.to_str() else {
                continue;
            };
            if argument == "--trace-ui" {
                return Err(UiTraceError::Argument);
            }
            let Some(value) = argument.strip_prefix("--trace-ui=") else {
                continue;
            };
            let path = PathBuf::from(value);
            if output.is_some()
                || value.is_empty()
                || !path
                    .extension()
                    .is_some_and(|ext| ext == "json" || ext == "html")
            {
                return Err(UiTraceError::Argument);
            }
            validate_destination(&path)?;
            output = Some(path);
        }
        output.map_or(Ok(Self { active: None }), |path| {
            Self::start(path, TraceConfig::default(), TraceDetail::Stages)
        })
    }

    /// Stop and export once. Dropping without finish discards the capture with
    /// no I/O. Calling this on the live UI thread can stall it during export.
    pub fn finish(self) -> Result<(), UiTraceError> {
        self.stop()
            .map_or(Ok(()), |export| export.write().map(|_| ()))
    }

    /// Convenience for main(): preserve an application failure if export also
    /// fails, and report the secondary export failure to stderr.
    pub fn finish_after_run<T>(
        self,
        result: Result<T, Box<dyn Error>>,
    ) -> Result<T, Box<dyn Error>> {
        match (result, self.finish()) {
            (Ok(value), Ok(())) => Ok(value),
            (Ok(_), Err(error)) => Err(Box::new(error)),
            (Err(error), export) => {
                if let Err(secondary) = export {
                    eprintln!("{secondary}");
                }
                Err(error)
            }
        }
    }
}

impl UiTraceExport {
    pub fn stats(&self) -> TraceStats {
        self.snapshot.stats()
    }

    /// Serialize and write once, refusing existing/racing output paths.
    pub fn write(self) -> Result<PathBuf, UiTraceError> {
        let content = if self.path.extension().is_some_and(|ext| ext == "html") {
            self.snapshot.to_html_report()
        } else {
            self.snapshot.to_chrome_json()
        }
        .map_err(UiTraceError::Export)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.path)
            .map_err(UiTraceError::Io)?;
        file.write_all(content.as_bytes())
            .map_err(UiTraceError::Io)?;
        Ok(self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_capture_exports_after_run_and_never_overwrites_a_racing_file() {
        let path = std::env::temp_dir().join(format!(
            "gpui-frame-trace-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let argument = format!("--trace-ui={}", path.display());
        assert!(UiTraceRecording::from_arguments([&argument, &argument]).is_err());
        assert!(!vesta_gpui::trace::is_recording());
        UiTraceRecording::from_arguments(["host", "--unrelated"])
            .expect("disabled")
            .finish()
            .expect("no output");
        let recording = UiTraceRecording::from_arguments([&argument]).expect("capture launch");
        assert!(!path.exists(), "no live-loop file I/O");
        {
            let _frame = vesta_gpui::trace::frame("Host.Main");
        }
        recording
            .finish_after_run(Ok(()))
            .expect("post-loop export");
        let original = std::fs::read(&path).expect("exported capture");
        let json: serde_json::Value = serde_json::from_slice(&original).expect("frame JSON");
        assert_eq!(json["gpui_frames"][0]["name"], "Host.Main");
        assert!(UiTraceRecording::from_arguments([&argument]).is_err());
        std::fs::remove_file(&path).expect("own export cleanup");
        let recording = UiTraceRecording::from_arguments([&argument]).expect("second launch");
        std::fs::write(&path, b"racing writer").expect("race fixture");
        assert!(recording.finish().is_err());
        assert_eq!(
            std::fs::read(&path).expect("preserved writer"),
            b"racing writer"
        );
        std::fs::remove_file(&path).expect("own fixture cleanup");
        assert!(!vesta_gpui::trace::is_recording());
        let path = path.with_extension("html");
        let recording = UiTraceRecording::start(&path, TraceConfig::default(), TraceDetail::Stages)
            .expect("on-demand start");
        {
            let _frame = vesta_gpui::trace::frame("OnDemand.Main");
        }
        assert!(recording.stats().is_some_and(|stats| stats.events == 1));
        let export = recording.stop().expect("frozen handoff");
        assert!(!path.exists());
        let successor =
            TraceCapture::start(TraceConfig::default()).expect("successor while exporting");
        let written = std::thread::spawn(move || export.write())
            .join()
            .expect("worker joined")
            .expect("worker export");
        assert!(
            successor.is_recording(),
            "export must not stop a successor capture"
        );
        drop(successor);
        let html = std::fs::read_to_string(&written).expect("HTML output");
        assert!(html.contains("OnDemand.Main"));
        std::fs::remove_file(written).expect("own deferred export cleanup");
    }
}
