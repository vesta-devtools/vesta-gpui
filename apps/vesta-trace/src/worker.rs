//! One application-owned, bounded I/O/analysis worker; never enters the UI model.
use crate::{
    import,
    inspection::{self, Report, Selection},
    session::*,
};
use image::ImageEncoder;
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::AtomicBool,
        mpsc::{self, SyncSender, TrySendError},
    },
    thread::{self, JoinHandle},
};
use winit::event_loop::EventLoopProxy;

pub enum Task {
    Capture {
        ticket: vesta_gpui_vulkan::CaptureTicket,
        path: PathBuf,
    },
    Load(PathBuf),
    Analyze {
        session: Arc<Session>,
        range: TimeRange,
        filter: String,
        selection: Selection,
        tracks: Option<Arc<crate::plot::FilteredTracks>>,
    },
}
pub struct Request {
    pub generation: u64,
    pub cancel: Arc<AtomicBool>,
    pub task: Task,
}
pub enum Product {
    CaptureSaved(PathBuf),
    Loaded {
        path: PathBuf,
        session: Arc<Session>,
        report: Report,
    },
    Analyzed(Report),
}
pub struct Message {
    pub generation: u64,
    pub result: Result<Product, TraceError>,
}
pub struct Worker {
    sender: Option<SyncSender<Request>>,
    thread: Option<JoinHandle<()>>,
}
impl Worker {
    pub fn new<T: From<Message> + Send + 'static>(
        proxy: EventLoopProxy<T>,
    ) -> Result<Self, TraceError> {
        let (sender, receiver) = mpsc::sync_channel::<Request>(1);
        let thread = thread::Builder::new()
            .name("VestaTrace.Analysis".into())
            .spawn(move || {
                while let Ok(request) = receiver.recv() {
                    let result = (|| match request.task {
                        Task::Capture { ticket, path } => {
                            let pixels = ticket
                                .wait(std::time::Duration::from_secs(8))
                                .map_err(|e| TraceError::caused("trace_capture", e))?;
                            let rgba = pixels
                                .rgba()
                                .map_err(|e| TraceError::caused("trace_capture_map", e))?;
                            if request.cancel.load(std::sync::atomic::Ordering::Relaxed) {
                                return Err(TraceError::new(
                                    "trace_cancelled",
                                    "capture cancelled before file output",
                                ));
                            }
                            let output = std::fs::OpenOptions::new()
                                .write(true)
                                .create_new(true)
                                .open(&path)
                                .map_err(|e| TraceError::caused("trace_capture_create", e))?;
                            image::codecs::png::PngEncoder::new(output)
                                .write_image(
                                    &rgba,
                                    pixels.region.extent[0],
                                    pixels.region.extent[1],
                                    image::ColorType::Rgba8.into(),
                                )
                                .map_err(|e| TraceError::caused("trace_capture_encode", e))?;
                            Ok(Product::CaptureSaved(path))
                        }
                        Task::Load(path) => {
                            let session = Arc::new(import::read(&path, &request.cancel)?);
                            let report = inspection::analyze(
                                &session,
                                session.bounds,
                                "",
                                &Selection::default(),
                                &request.cancel,
                                None,
                            )?;
                            Ok(Product::Loaded {
                                path,
                                session,
                                report,
                            })
                        }
                        Task::Analyze {
                            session,
                            range,
                            filter,
                            selection,
                            tracks,
                        } => inspection::analyze(
                            &session,
                            range,
                            &filter,
                            &selection,
                            &request.cancel,
                            tracks,
                        )
                        .map(Product::Analyzed),
                    })();
                    if proxy
                        .send_event(
                            Message {
                                generation: request.generation,
                                result,
                            }
                            .into(),
                        )
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .map_err(|e| TraceError::caused("trace_worker_start", e))?;
        Ok(Self {
            sender: Some(sender),
            thread: Some(thread),
        })
    }
    pub fn submit(&self, request: Request) -> Result<Option<Request>, TraceError> {
        let sender = self
            .sender
            .as_ref()
            .ok_or_else(|| TraceError::new("trace_worker_closed", "worker is closed"))?;
        match sender.try_send(request) {
            Ok(()) => Ok(None),
            Err(TrySendError::Full(request)) => Ok(Some(request)),
            Err(TrySendError::Disconnected(_)) => Err(TraceError::new(
                "trace_worker_closed",
                "worker disconnected",
            )),
        }
    }
    /// Call only after the native event loop returns. Active work is already cancelled.
    pub fn finish(&mut self) -> Result<(), TraceError> {
        self.sender = None;
        if let Some(thread) = self.thread.take() {
            thread.join().map_err(|_| {
                TraceError::new("trace_worker_panicked", "analysis worker panicked")
            })?;
        }
        Ok(())
    }
}
