//! Native counter point/envelope tracks. Lines are guides, not sampled values.
use super::*;
use vesta_gpui::Point;
pub(super) struct Counts {
    pub drawn: usize,
    pub collapsed: usize,
    pub visible: usize,
    pub height: f32,
}
pub(super) fn build(
    b: &mut Builder,
    session: &Session,
    timeline: &Timeline,
    tracks: Option<&crate::plot::FilteredTracks>,
    filter: &str,
    track: Rect,
    columns: usize,
) -> Counts {
    let mut result = Counts {
        drawn: 0,
        collapsed: 0,
        visible: 0,
        height: 0.0,
    };
    let mut y = track.y - timeline.scroll;
    for (lane_index, lane) in session.lanes.iter().enumerate() {
        let selected = tracks.and_then(|t| t.counters.get(lane_index));
        let count = if filter.is_empty() {
            lane.counters.len()
        } else {
            selected.map_or(0, Vec::len)
        };
        if count == 0 {
            continue;
        }
        if y >= track.y && y + 24.0 <= track.bottom() {
            b.block(
                Rect::new(12.0, y, track.right() - 12.0, 24.0),
                Color::rgb8(30, 39, 51),
            );
            b.label(
                Rect::new(20.0, y + 3.0, 160.0, 20.0),
                format!("P{} / thread {}", lane.key.process, lane.key.thread),
                12.0,
                TEXT,
            );
            b.label(
                Rect::new(track.x + 4.0, y + 3.0, track.width - 8.0, 20.0),
                "Raw points / min-max bins · connecting lines are visual guides",
                11.0,
                MUTED,
            );
        }
        y += 27.0;
        let all = (0..lane.counters.len()).take(if filter.is_empty() { usize::MAX } else { 0 });
        let filtered = selected
            .filter(|_| !filter.is_empty())
            .into_iter()
            .flatten()
            .copied();
        for index in all.chain(filtered) {
            if y + 80.0 > track.y && y < track.bottom() {
                let series = &lane.counters[index];
                let anchor = &session.events[series.events[0]];
                if y >= track.y {
                    b.label(
                        Rect::new(20.0, y + 3.0, 160.0, 40.0),
                        &anchor.name,
                        11.0,
                        TEXT,
                    );
                }
                let graph = Rect::new(track.x, y + 24.0, track.width, 50.0);
                if let Some(clip) = graph.intersection(track) {
                    b.block(clip, Color::rgb8(21, 28, 38));
                    if let Some(envelope) = series.envelope(&session.events, timeline.view, columns)
                    {
                        result.visible += envelope.count;
                        if y >= track.y {
                            b.label(
                                Rect::new(track.x + 4.0, y + 3.0, track.width - 8.0, 20.0),
                                format!(
                                    "{} / {} · min {:.4e} · max {:.4e} · {} observations",
                                    anchor.category,
                                    anchor.name,
                                    envelope.min,
                                    envelope.max,
                                    envelope.count
                                ),
                                11.0,
                                MUTED,
                            );
                        }
                        let point = |i: usize| {
                            let e = &session.events[i];
                            Point::new(
                                graph.x
                                    + (timeline.fraction(e.start) * f64::from(graph.width)) as f32,
                                graph.bottom()
                                    - crate::counters::fraction(
                                        crate::counters::value(e),
                                        envelope.min,
                                        envelope.max,
                                    ) * graph.height,
                            )
                        };
                        if envelope.points.len() > 1 {
                            let points = envelope
                                .points
                                .iter()
                                .map(|i| {
                                    let p = point(*i);
                                    Point::new(p.x - clip.x, p.y - clip.y)
                                })
                                .collect();
                            let value = b.next;
                            b.next += 1;
                            b.add(Widget::from_element(
                                Element::container(
                                    id(value),
                                    ElementKey::new(format!("trace.counter.guide.{value}")),
                                )
                                .styled(
                                    geometry(clip)
                                        .polyline(points, 1.0)
                                        .background(ACCENT)
                                        .pointer_events(false),
                                ),
                            ));
                        }
                        let mut drawn = 0;
                        for i in &envelope.points {
                            if result.drawn >= 6000 {
                                break;
                            }
                            let e = &session.events[*i];
                            let p = point(*i);
                            if let Some(rect) =
                                Rect::new(p.x - 2.0, p.y - 2.0, 4.0, 4.0).intersection(clip)
                            {
                                b.target(
                                    7_000_000 + *i as u64,
                                    rect,
                                    &format!(
                                        "{} = {} at {} ms; event {}",
                                        e.name,
                                        crate::counters::value(e),
                                        ms(e.start),
                                        e.key.event
                                    ),
                                    ACCENT,
                                    timeline.selected == Some(e.key),
                                    Command::Scope(e.key),
                                );
                                result.drawn += 1;
                                drawn += 1;
                            }
                        }
                        result.collapsed += envelope.count - drawn;
                    } else if y >= track.y {
                        b.label(
                            Rect::new(track.x + 8.0, y + 32.0, track.width - 16.0, 23.0),
                            "No recorded samples in this visible range",
                            12.0,
                            MUTED,
                        );
                    }
                }
            }
            y += 82.0;
        }
        y += 10.0;
    }
    result.height = y + timeline.scroll - track.y;
    if result.height == 0.0 {
        b.label(
            Rect::new(track.x + 12.0, track.y + 12.0, track.width - 24.0, 30.0),
            "No matching recorded counter series",
            13.0,
            MUTED,
        );
    }
    result
}
