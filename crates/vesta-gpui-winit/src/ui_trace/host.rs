//! Host-owned, on-demand entry into the separate native analyzer.
use super::control::{ControlServer, ENVIRONMENT};
use std::{
    io,
    path::{Path, PathBuf},
    process::{Child, Command},
};

/// Create this owner in response to a host's Trace action and retain it until
/// the host event loop returns. No capture starts implicitly. The analyzer and
/// authorized agents attach through the same local session descriptor.
pub struct UiTraceHost {
    // Drop the listener before its private directory.
    _control: Option<ControlServer>,
    _directory: Option<tempfile::TempDir>,
    descriptor: PathBuf,
    analyzer: Option<Child>,
}

impl UiTraceHost {
    pub fn new() -> io::Result<Self> {
        if let Some(value) = std::env::var_os(ENVIRONMENT) {
            let config: serde_json::Value =
                serde_json::from_str(&value.to_string_lossy()).map_err(io::Error::other)?;
            let descriptor = PathBuf::from(config["descriptor"].as_str().ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "Missing trace descriptor")
            })?);
            if !descriptor.is_absolute() {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "Host trace control is unavailable",
                ));
            }
            if descriptor.is_file() {
                return Ok(Self {
                    _control: None,
                    _directory: None,
                    descriptor,
                    analyzer: None,
                });
            }
        }
        let directory = tempfile::Builder::new()
            .prefix("gpui-trace-host-")
            .tempdir()?;
        let descriptor = directory.path().join("session.json");
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|e| io::Error::other(e.to_string()))?;
        let token = random.iter().map(|b| format!("{b:02x}")).collect();
        let control = ControlServer::start(descriptor.clone(), token)?;
        Ok(Self {
            _control: Some(control),
            _directory: Some(directory),
            descriptor,
            analyzer: None,
        })
    }

    /// Private local capability for an authorized Agent. Do not publish its contents.
    pub fn descriptor(&self) -> &Path {
        &self.descriptor
    }

    /// Launch an explicit analyzer executable without a shell. Returns false if
    /// this owner's analyzer is still running. Closing it leaves the host alive.
    pub fn open_analyzer(&mut self, executable: impl AsRef<Path>) -> io::Result<bool> {
        if let Some(child) = &mut self.analyzer
            && child.try_wait()?.is_none()
        {
            return Ok(false);
        }
        if !self.descriptor.is_file() {
            // A tool-launched helper can retire while the host remains alive.
            // Reopening Trace creates a new host-owned endpoint on demand.
            *self = Self::new()?;
        }
        let executable = executable.as_ref().canonicalize()?;
        self.analyzer = Some(
            Command::new(executable)
                .arg(format!("--connect={}", self.descriptor.display()))
                // The analyzer must not start a second SDK listener from our environment.
                .env_remove(ENVIRONMENT)
                .spawn()?,
        );
        Ok(true)
    }

    /// The shipped native analyzer lives beside the host; no PATH lookup.
    pub fn open_default_analyzer(&mut self) -> io::Result<bool> {
        let name = if cfg!(windows) {
            "vesta-trace.exe"
        } else {
            "vesta-trace"
        };
        self.open_analyzer(std::env::current_exe()?.with_file_name(name))
    }
}
