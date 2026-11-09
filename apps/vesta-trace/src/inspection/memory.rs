//! Native memory views request the same typed, source-bound evidence as Agents.
use super::*;
use crate::query::protocol;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MemoryMode {
    #[default]
    All,
    LiveAtEnd,
    NewSurvivors,
    Released,
    Common,
    Transient,
}
impl MemoryMode {
    pub const ALL: [(Self, &'static str); 6] = [
        (Self::All, "All lifetimes"),
        (Self::LiveAtEnd, "Live at B"),
        (Self::NewSurvivors, "New at B"),
        (Self::Released, "Released"),
        (Self::Common, "Common"),
        (Self::Transient, "Transient"),
    ];
}
#[derive(Clone, Debug, Default)]
pub struct MemorySelection {
    pub allocation: Option<MemoryAllocationKey>,
    pub mode: MemoryMode,
}
#[derive(Debug)]
pub struct MemoryInspection {
    pub mode: MemoryMode,
    pub coverage: Option<MemoryCoverage>,
    pub statistics: Option<MemoryStatistics>,
    pub points: Vec<MemoryPoint>,
    pub comparison: Option<MemoryComparison>,
    pub allocation: Option<MemoryAllocationRecord>,
    pub stack: Option<MemoryStackRecord>,
    pub message: String,
}
pub(super) fn inspect(
    session: &Session,
    range: TimeRange,
    text: &str,
    selection: &Selection,
    cancel: &AtomicBool,
) -> Result<(MemoryInspection, Option<Data>), TraceError> {
    let ask_memory = |query, filter, page| {
        ask(
            session,
            Request {
                schema_version: PROTOCOL_VERSION,
                request_id: "native.memory".into(),
                query,
                selection: filter,
                page,
                budget: Budget::default(),
            },
            cancel,
        )
    };
    let (coverage, statistics) =
        match ask_memory(Query::MemorySummary, Default::default(), Default::default())? {
            Data::MemorySummary {
                coverage,
                statistics,
            } => (coverage, statistics),
            _ => unreachable!(),
        };
    let mut view = MemoryInspection {
        mode: selection.memory.mode,
        coverage,
        statistics,
        points: Vec::new(),
        comparison: None,
        allocation: None,
        stack: None,
        message: String::new(),
    };
    let Some(through) = view.coverage.as_ref().map(|c| c.through_ns) else {
        view.message="No heap recording in this capture. Enable Memory before starting a capture on a host with UiTraceAllocator.".into();
        return Ok((view, None));
    };
    if let Data::MemorySeries { items, .. } = ask_memory(
        Query::MemorySeries { columns: 128 },
        Default::default(),
        Page {
            limit: 1000,
            cursor: None,
        },
    )? {
        view.points = items;
    }
    if let Some(key) = selection.memory.allocation {
        if let Data::MemoryAllocation { item } = ask_memory(
            Query::MemoryAllocation { key },
            Default::default(),
            Default::default(),
        )? {
            view.allocation = Some(item);
        }
        if let Some(key) = view.allocation.as_ref().and_then(|a| a.stack)
            && let Data::MemoryStack { item } = ask_memory(
                Query::MemoryStack { key },
                Default::default(),
                Default::default(),
            )?
        {
            view.stack = Some(item);
        }
    }
    let Some(clipped) = TimeRange::new(range.start, range.end.min(through + 1)) else {
        view.message="Selected interval is outside known memory coverage. Choose an earlier interval; later live-byte totals are unknown.".into();
        return Ok((view, None));
    };
    let (start_ns, end_ns) = (clipped.start, clipped.end - 1);
    if start_ns < end_ns
        && let Data::MemoryCompare { comparison } = ask_memory(
            Query::MemoryCompare { start_ns, end_ns },
            protocol::Selection {
                text: text.into(),
                ..Default::default()
            },
            Default::default(),
        )?
    {
        view.comparison = Some(comparison);
    }
    view.message = format!(
        "A {} ns · B {} ns · allocation stacks / addresses searched · observed Rust heap only{}",
        start_ns,
        end_ns,
        if clipped != range {
            " · range clipped to heap coverage"
        } else {
            ""
        }
    );
    let lifetime = match selection.memory.mode {
        MemoryMode::All => MemoryPredicate::All,
        MemoryMode::LiveAtEnd => MemoryPredicate::LiveAt { time_ns: end_ns },
        MemoryMode::NewSurvivors => MemoryPredicate::NewSurvivors { start_ns, end_ns },
        MemoryMode::Released => MemoryPredicate::Released { start_ns, end_ns },
        MemoryMode::Common => MemoryPredicate::Common { start_ns, end_ns },
        MemoryMode::Transient => MemoryPredicate::Transient { start_ns, end_ns },
    };
    if start_ns == end_ns
        && !matches!(
            selection.memory.mode,
            MemoryMode::All | MemoryMode::LiveAtEnd
        )
    {
        view.message
            .push_str(" · select a wider A–B interval to compare");
        return Ok((view, None));
    }
    let table = ask_memory(
        Query::MemoryAllocations {
            sort: MemorySort::SizeDesc,
            lifetime,
            stack: None,
        },
        protocol::Selection {
            range_ns: Some([clipped.start, clipped.end]),
            text: text.into(),
            ..Default::default()
        },
        Page {
            limit: 100,
            cursor: selection.cursor.clone(),
        },
    )?;
    Ok((view, Some(table)))
}
