//! Pending work never replaces the last validated source/view until publication.
use super::*;

#[derive(Default)]
pub(super) enum Activity {
    #[default]
    Idle,
    Import(PathBuf),
    Analysis,
    Capture,
}
pub(super) struct Checkpoint {
    source: String,
    timeline: Timeline,
    panel: Panel,
    frame_sort: crate::query::protocol::FrameTimingSort,
    memory: crate::inspection::MemorySelection,
    timer: Option<TimerKey>,
    gpu: Option<GpuKey>,
    gpu_scroll: vesta_gpui_widgets::ScrollAreaState,
    page: Option<Cursor>,
    previous_pages: Vec<Option<Cursor>>,
    table_scroll: usize,
    filter: String,
    focus: Option<InputTarget>,
}
impl App {
    pub(super) fn busy(&self) -> bool {
        matches!(self.activity, Activity::Import(_) | Activity::Analysis)
    }
    pub(super) fn importing(&self) -> bool {
        matches!(self.activity, Activity::Import(_))
    }
    pub(super) fn begin_change(&mut self) {
        if self.checkpoint.is_some() {
            return;
        }
        if let Some((session, timeline)) = self.session.as_ref().zip(self.timeline.as_ref()) {
            self.checkpoint = Some(Checkpoint {
                source: session.source.sha256.clone(),
                timeline: timeline.clone(),
                panel: self.panel,
                frame_sort: self.frame_sort,
                memory: self.memory.clone(),
                timer: self.timer.clone(),
                gpu: self.gpu,
                gpu_scroll: self.gpu_scroll,
                page: self.page.clone(),
                previous_pages: self.previous_pages.clone(),
                table_scroll: self.table_scroll,
                filter: self.filter.clone(),
                focus: self.ownership.focus(),
            });
        }
    }
    pub(super) fn restore_view(&mut self) -> std::result::Result<(), TraceError> {
        let saved = self.checkpoint.take();
        self.suspend_geometry();
        self.search.due = None;
        if let Some(saved) = saved {
            if self
                .session
                .as_ref()
                .is_none_or(|s| s.source.sha256 != saved.source)
            {
                return Err(TraceError::new(
                    "trace_checkpoint_source",
                    "view belongs to another source",
                ));
            }
            self.timeline = Some(saved.timeline);
            self.panel = saved.panel;
            self.frame_sort = saved.frame_sort;
            self.memory = saved.memory;
            self.timer = saved.timer;
            self.gpu = saved.gpu;
            self.gpu_scroll = saved.gpu_scroll;
            self.page = saved.page;
            self.previous_pages = saved.previous_pages;
            self.table_scroll = saved.table_scroll;
            self.filter = saved.filter;
            // Search is a global draft, not source data. Keep its current focus
            // and text; source-owned focus returns with the validated view.
            if !self.search_focused() {
                self.set_focus(saved.focus);
            }
        }
        Ok(())
    }
    pub(super) fn source_name(&self) -> String {
        self.source_path
            .as_ref()
            .and_then(|p| p.file_name())
            .map_or_else(
                || "No capture loaded".into(),
                |p| p.to_string_lossy().into_owned(),
            )
    }
    pub(super) fn cancel_work(&mut self) -> std::result::Result<(), TraceError> {
        if !self.busy() {
            return Ok(());
        }
        self.cancel.store(true, Ordering::Relaxed);
        self.pending = None;
        self.generation = self.generation.checked_add(1).ok_or_else(|| {
            TraceError::new("trace_generation_exhausted", "job identities exhausted")
        })?;
        self.activity = Activity::Idle;
        self.restore_view()?;
        self.status = format!("{} · work cancelled", self.source_name());
        self.redraw();
        Ok(())
    }
    pub(super) fn open_source(&mut self, path: PathBuf) -> std::result::Result<(), TraceError> {
        let desired_panel = self.panel;
        self.cancel_work()?;
        self.suspend_geometry();
        self.begin_change();
        self.panel = desired_panel;
        self.queue(Task::Load(path))
    }
}
