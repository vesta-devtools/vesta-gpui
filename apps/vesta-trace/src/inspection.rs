//! Native inspection requests use the same bounded evidence engine as Agents.
use crate::{
    analysis,
    query::{self, protocol::*},
    session::*,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Panel {
    #[default]
    Timers,
    Callers,
    Callees,
    Events,
    Tree,
    Counters,
    Diagnostics,
    Gpu,
}
impl Panel {
    pub const ALL: [(Self, &'static str); 8] = [
        (Self::Timers, "Timers"),
        (Self::Callers, "Callers"),
        (Self::Callees, "Callees"),
        (Self::Events, "Occurrences"),
        (Self::Tree, "Call tree"),
        (Self::Counters, "Counters"),
        (Self::Diagnostics, "Quality"),
        (Self::Gpu, "GPU"),
    ];
}
#[derive(Clone, Debug, Default)]
pub struct Selection {
    pub gpu: Option<GpuKey>,
    pub panel: Panel,
    pub timer: Option<TimerKey>,
    pub event: Option<EventKey>,
    pub cursor: Option<Cursor>,
}
#[derive(Debug)]
pub struct EventDetail {
    pub item: EventRecord,
    pub children: usize,
    pub self_ns: Option<u64>,
}
#[derive(Debug)]
pub struct Inspection {
    pub gpu: Option<GpuRecord>,
    pub timer: Option<TimerKey>,
    pub table: Option<Data>,
    pub detail: Option<EventDetail>,
}
#[derive(Debug)]
pub struct Report {
    pub analysis: Arc<analysis::Analysis>,
    pub inspection: Inspection,
    pub tracks: Option<Arc<crate::plot::FilteredTracks>>,
}

fn ask(session: &Session, request: Request, cancel: &AtomicBool) -> Result<Data, TraceError> {
    let response = query::execute(session, &request, cancel);
    match response.result {
        Outcome::Ok { data, .. } => Ok(*data),
        Outcome::Error { code, message } => Err(TraceError::new(
            "trace_native_query",
            format!("{code}: {message}"),
        )),
    }
}
pub fn analyze(
    session: &Session,
    range: TimeRange,
    text: &str,
    selection: &Selection,
    cancel: &AtomicBool,
    cached: Option<Arc<crate::plot::FilteredTracks>>,
) -> Result<Report, TraceError> {
    let analysis = Arc::new(analysis::analyze(session, range, text, cancel)?);
    let tracks = crate::plot::FilteredTracks::prepare(session, text, cached, cancel)?;
    let timer = selection.timer.clone().or_else(|| {
        analysis.timers.first().map(|t| TimerKey {
            category: t.category.clone(),
            name: t.name.clone(),
        })
    });
    let query = match selection.panel {
        Panel::Timers | Panel::Diagnostics => None,
        Panel::Callers | Panel::Callees => timer.clone().map(|timer| Query::Relations {
            timer,
            direction: if selection.panel == Panel::Callers {
                Direction::Callers
            } else {
                Direction::Callees
            },
        }),
        Panel::Events => timer.clone().map(|timer| Query::Events {
            timer: Some(timer),
            kind: None,
        }),
        Panel::Tree => selection.event.map(|root| Query::Tree { root }),
        Panel::Counters => Some(Query::Counters {
            category: None,
            name: None,
        }),
        Panel::Gpu => Some(Query::GpuSubmissions {
            sort: GpuSort::DurationDesc,
        }),
    };
    let table = query
        .map(|query| {
            ask(
                session,
                Request {
                    schema_version: PROTOCOL_VERSION,
                    request_id: "native.inspection".into(),
                    query,
                    selection: crate::query::protocol::Selection {
                        range_ns: Some([range.start, range.end]),
                        text: text.into(),
                        ..Default::default()
                    },
                    page: Page {
                        limit: 100,
                        cursor: selection.cursor.clone(),
                    },
                    budget: Budget::default(),
                },
                cancel,
            )
        })
        .transpose()?;
    let detail = selection
        .event
        .map(|key| {
            ask(
                session,
                Request {
                    schema_version: PROTOCOL_VERSION,
                    request_id: "native.event".into(),
                    query: Query::Event { key },
                    selection: Default::default(),
                    page: Default::default(),
                    budget: Default::default(),
                },
                cancel,
            )
        })
        .transpose()?
        .map(|data| match data {
            Data::Event {
                item,
                direct_children,
                self_ns,
            } => Ok(EventDetail {
                item,
                children: direct_children,
                self_ns,
            }),
            _ => Err(TraceError::new(
                "trace_native_query_kind",
                "event query returned the wrong projection",
            )),
        })
        .transpose()?;
    let gpu = selection
        .gpu
        .map(|key| {
            ask(
                session,
                Request {
                    schema_version: PROTOCOL_VERSION,
                    request_id: "native.gpu".into(),
                    query: Query::GpuSubmission { key },
                    selection: Default::default(),
                    page: Default::default(),
                    budget: Default::default(),
                },
                cancel,
            )
        })
        .transpose()?
        .map(|data| match data {
            Data::GpuSubmission { item } => Ok(item),
            _ => Err(TraceError::new(
                "trace_native_query_kind",
                "GPU query returned the wrong projection",
            )),
        })
        .transpose()?;
    if cancel.load(Ordering::Relaxed) {
        return Err(TraceError::new(
            "trace_cancelled",
            "native inspection cancelled",
        ));
    }
    Ok(Report {
        tracks,
        analysis,
        inspection: Inspection {
            gpu,
            timer,
            table,
            detail,
        },
    })
}

impl Inspection {
    pub fn page(&self) -> Option<&PageInfo> {
        match &self.table {
            Some(
                Data::Relations { page, .. }
                | Data::Events { page, .. }
                | Data::Tree { page, .. }
                | Data::Counters { page, .. }
                | Data::GpuSubmissions { page, .. },
            ) => Some(page),
            _ => None,
        }
    }
}
