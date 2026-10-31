//! CPU-only ownership observation for per-view resource I/O and profile retirement.
use crate::WebError;
use std::{
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

#[derive(Debug, Default)]
struct State {
    pending: usize,
    error: Option<WebError>,
}
#[derive(Debug, Default)]
struct Shared {
    state: Mutex<State>,
    finished: Condvar,
}

/// Host-created observation group, shared by views created during that host run.
/// It retains no native view/window, resource sender, file or profile directory.
/// Dispose views on their event thread, then observe after the event loop returns.
#[derive(Clone, Debug, Default)]
pub struct WebRetirement(Arc<Shared>);

impl WebRetirement {
    pub fn pending(&self) -> bool {
        self.0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pending
            != 0
    }

    /// Observe completion, never cancel I/O or force an unfinished owner to drop.
    /// A zero budget is a nonblocking observation. Positive budgets belong off
    /// the live UI loop. The first cleanup/interruption error remains recorded.
    pub fn observe(&self, budget: Duration) -> Result<(), WebError> {
        let state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        let (state, _) = self
            .0
            .finished
            .wait_timeout_while(state, budget, |state| state.pending != 0)
            .unwrap_or_else(|e| e.into_inner());
        if state.pending != 0 {
            return Err(WebError::new(
                "web_retirement_observation_timeout",
                "Web resource/profile ownership is still pending",
            ));
        }
        state.error.clone().map_or(Ok(()), Err)
    }
    #[cfg(any(windows, test))]
    pub(crate) fn lease(&self) -> Lease {
        self.0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pending += 1;
        Lease {
            state: self.0.clone(),
            entered: false,
            settled: false,
        }
    }
}

#[cfg(any(windows, test))]
pub(crate) struct Lease {
    state: Arc<Shared>,
    entered: bool,
    settled: bool,
}
#[cfg(any(windows, test))]
impl Lease {
    pub(crate) fn enter(&mut self) {
        self.entered = true;
    }
    pub(crate) fn finish(mut self, result: Result<(), WebError>) {
        if let Err(error) = result {
            self.state
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .error
                .get_or_insert(error);
        }
        self.settled = true;
    }
}
#[cfg(any(windows, test))]
impl Drop for Lease {
    fn drop(&mut self) {
        let mut state = self.state.state.lock().unwrap_or_else(|e| e.into_inner());
        if self.entered && !self.settled {
            state.error.get_or_insert_with(|| {
                WebError::new(
                    "web_resource_worker_interrupted",
                    "Resource worker stopped before retirement completed",
                )
            });
        }
        state.pending -= 1;
        self.state.finished.notify_all();
    }
}

#[cfg(any(windows, test))]
pub(crate) fn retire_profile(profile: Option<tempfile::TempDir>) -> Result<(), WebError> {
    let Some(profile) = profile else {
        return Ok(());
    };
    // This exact path is owned by tempfile, never a caller's installed profile.
    let start = std::time::Instant::now();
    loop {
        match std::fs::remove_dir_all(profile.path()) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(_) if start.elapsed() < Duration::from_secs(10) => {
                std::thread::sleep(Duration::from_millis(25))
            }
            Err(error) => {
                return Err(WebError::new(
                    "web_profile_cleanup_failed",
                    format!("{}: {error}", profile.path().display()),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_start_pending_retirement_and_interruption_keep_one_ownership_count() {
        let group = WebRetirement::default();
        drop(group.lease()); // Worker spawn failed before entry.
        group
            .observe(Duration::ZERO)
            .expect("no unpublished worker remains");
        let observer = group.clone();
        let mut lease = group.lease();
        lease.enter();
        assert_eq!(
            observer.observe(Duration::ZERO).expect_err("pending").code,
            "web_retirement_observation_timeout"
        );
        let profile = tempfile::tempdir().expect("owned profile");
        let path = profile.path().to_owned();
        std::fs::write(path.join("fixture.json"), b"{}").expect("owned data");
        #[cfg(windows)]
        let locked = {
            use std::os::windows::fs::OpenOptionsExt;
            std::fs::OpenOptions::new()
                .read(true)
                .share_mode(0)
                .open(path.join("fixture.json"))
                .expect("native browser-style file lock")
        };
        let worker = std::thread::spawn(move || lease.finish(retire_profile(Some(profile))));
        #[cfg(windows)]
        {
            assert_eq!(
                observer
                    .observe(Duration::from_millis(40))
                    .expect_err("locked profile is not retired")
                    .code,
                "web_retirement_observation_timeout"
            );
            assert!(path.exists());
            drop(locked);
        }
        observer
            .observe(Duration::from_secs(2))
            .expect("released profile retires");
        worker.join().expect("worker terminal");
        observer
            .observe(Duration::ZERO)
            .expect("all I/O owners retired");
        assert!(!path.exists());
        let mut interrupted = group.lease();
        interrupted.enter();
        drop(interrupted);
        assert_eq!(
            observer
                .observe(Duration::ZERO)
                .expect_err("interruption persists")
                .code,
            "web_resource_worker_interrupted"
        );
    }
}
