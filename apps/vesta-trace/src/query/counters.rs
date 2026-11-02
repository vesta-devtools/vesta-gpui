//! The same raw-point envelope model serves native plots and bounded AI queries.
use super::*;
pub(super) struct Context<'a> {
    pub session: &'a Session,
    pub request: &'a Request,
    pub range: TimeRange,
    pub selection: &'a analysis::Selection,
    pub hash: &'a str,
    pub columns: usize,
    pub timer: Option<&'a TimerKey>,
}
pub(super) fn run(context: Context<'_>, control: &mut Control<'_>) -> Result<Data, TraceError> {
    let Context {
        session,
        request,
        range,
        selection,
        hash,
        columns,
        timer,
    } = context;
    if !(1..=256).contains(&columns) {
        return Err(TraceError::new(
            "trace_counter_columns",
            "counter_series columns must be 1..256",
        ));
    }
    let labels = analysis::Selection {
        text: selection.text.clone(),
        lane: selection.lane,
        frame: None,
    };
    let mut groups = Vec::new();
    for (lane_index, lane) in session.lanes.iter().enumerate() {
        for (series_index, series) in lane.counters.iter().enumerate() {
            control.check(1)?;
            let anchor = &session.events[series.events[0]];
            if !labels.matches(anchor, session.bounds)
                || timer.is_some_and(|t| t.category != anchor.category || t.name != anchor.name)
            {
                continue;
            }
            let start = series
                .events
                .partition_point(|i| session.events[*i].start < range.start);
            let end = series
                .events
                .partition_point(|i| session.events[*i].start < range.end);
            if start == end {
                continue;
            }
            let subset = if let Some(frame) = selection.frame {
                let mut subset = Vec::new();
                for i in &series.events[start..end] {
                    control.check(1)?;
                    if session.events[*i].frame == Some(frame) {
                        subset.push(*i);
                    }
                }
                if subset.is_empty() {
                    continue;
                }
                if subset.len() == end - start {
                    None
                } else {
                    Some(subset)
                }
            } else {
                None
            };
            groups.push((lane_index, series_index, subset));
        }
    }
    let (groups, page) = page(groups, session, request, hash)?;
    let point = |i: usize| {
        let e = &session.events[i];
        CounterPoint {
            event: e.key,
            time_ns: e.start,
            value: crate::counters::value(e),
        }
    };
    let mut items = Vec::with_capacity(groups.len());
    for (lane, series, subset) in groups {
        let original = &session.lanes[lane].counters[series];
        let selected = if let Some(indices) = subset {
            control.check(indices.len() as u64 * 12)?;
            Some(crate::counters::Series::new(
                indices,
                &session.events,
                control.cancellation(),
            )?)
        } else {
            None
        };
        let series = selected.as_ref().unwrap_or(original);
        let depth = usize::BITS - series.events.len().max(1).leading_zeros();
        control.check((columns as u64 * 4 + 2) * u64::from(depth) * 2)?;
        let envelope = series
            .envelope(&session.events, range, columns)
            .ok_or_else(|| {
                TraceError::new(
                    "trace_counter_index",
                    "counter envelope lost a matching series",
                )
            })?;
        control.check(envelope.points.len() as u64)?;
        let anchor = &session.events[series.events[0]];
        items.push(CounterTrack {
            lane: anchor.lane,
            timer: TimerKey {
                category: anchor.category.clone(),
                name: anchor.name.clone(),
            },
            count: envelope.count,
            minimum: point(envelope.minimum),
            maximum: point(envelope.maximum),
            omitted_observations: envelope.count - envelope.points.len(),
            points: envelope.points.into_iter().map(point).collect(),
        });
    }
    control.check(0)?;
    Ok(Data::CounterSeries {
        columns,
        items,
        page,
    })
}
