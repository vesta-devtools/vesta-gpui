//! Protect count/identity equivalence of dense plots against the raw event model.
use super::*;
use crate::{import, tests::fixture};
use serde_json::json;
use std::cmp::Reverse;

#[test]
fn indexed_buckets_preserve_exact_visible_counts_points_carry_and_stable_representatives() {
    let mut input = fixture();
    let mut seed = 781u64;
    let events=(1..=257).map(|id|{
        seed=seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let start=(seed>>16)%200;let duration=if id%5==0{0}else{(seed>>24)%90};
        json!({"pid":17,"tid":1,"ts":start,"dur":duration,"ph":"X","cat":"synthetic","name":format!("scope{}",id%3),"args":{"event_id":id,"parent_id":null,"frame_id":null,"frame":false}})
    }).collect::<Vec<_>>();
    input["traceEvents"] = json!(events);
    input["gpui_trace"]["events"] = json!(257);
    input["gpui_trace"]["unframed_events"] = json!(257);
    let session = import::parse(input.to_string().as_bytes(), &AtomicBool::new(false))
        .expect("synthetic plot session");
    let row = &session.lanes[0].rows[0];
    let expected = row
        .events
        .iter()
        .max_by_key(|i| {
            let e = &session.events[**i];
            (e.end - e.start, Reverse(e.key))
        })
        .copied();
    assert_eq!(row.plot.longest(&row.events), expected);
    for start in (0..240000).step_by(7000) {
        for duration in [1, 1000, 37000, 100000] {
            for columns in [1, 2, 7, 128, 8192] {
                let range = TimeRange::new(start, start + duration).expect("query range");
                let raw = row
                    .events
                    .iter()
                    .filter(|i| session.events[**i].visible_in(range))
                    .copied()
                    .collect::<Vec<_>>();
                let buckets = row
                    .plot
                    .buckets(&row.events, &session.events, range, columns);
                assert_eq!(
                    buckets.iter().map(|b| b.count).sum::<usize>(),
                    raw.len(),
                    "start {start} duration {duration} columns {columns}"
                );
                assert!(buckets.len() <= columns + 1);
                let mut picked = std::collections::BTreeSet::new();
                for bucket in buckets {
                    assert!(bucket.count > 0 && raw.contains(&bucket.index));
                    assert!(picked.insert(bucket.index));
                }
            }
        }
    }
    // An admitted point is visible at its start; a span ending there is not.
    let point = row
        .events
        .iter()
        .find(|i| {
            let e = &session.events[**i];
            e.start == e.end
        })
        .expect("point");
    let time = session.events[*point].start;
    let buckets = row.plot.buckets(
        &row.events,
        &session.events,
        TimeRange::new(time, time + 1).expect("point range"),
        1,
    );
    assert_eq!(
        buckets.iter().map(|b| b.count).sum::<usize>(),
        row.visible(&session, TimeRange::new(time, time + 1).expect("range"))
            .count()
    );
}

#[test]
fn filtered_tracks_reuse_only_the_same_source_and_text_and_preserve_original_keys() {
    let cancel = AtomicBool::new(false);
    let input = fixture();
    let session = import::parse(input.to_string().as_bytes(), &cancel).expect("source");
    let tracks = FilteredTracks::prepare(&session, "SCOPE2", None, &cancel)
        .expect("indexed")
        .expect("filtered tracks");
    let reused = FilteredTracks::prepare(&session, "scope2", Some(tracks.clone()), &cancel)
        .expect("cache")
        .expect("tracks");
    assert!(Arc::ptr_eq(&tracks, &reused));
    let keys = tracks
        .lanes
        .iter()
        .flatten()
        .flat_map(|r| r.events.iter().map(|i| session.events[*i].key))
        .collect::<Vec<_>>();
    assert_eq!(
        keys,
        vec![crate::session::EventKey {
            process: 17,
            event: 2
        }]
    );
    let mut changed = input;
    changed["traceEvents"][1]["name"] = json!("different label");
    let changed = import::parse(changed.to_string().as_bytes(), &cancel).expect("other source");
    let replaced = FilteredTracks::prepare(&changed, "scope2", Some(tracks.clone()), &cancel)
        .expect("other source index")
        .expect("tracks");
    assert!(!Arc::ptr_eq(&tracks, &replaced));
    assert_eq!(replaced.source_sha256, changed.source.sha256);
    assert_eq!(
        replaced
            .lanes
            .iter()
            .flatten()
            .map(|r| r.events.len())
            .sum::<usize>(),
        0
    );
    assert!(
        FilteredTracks::prepare(&session, "scope2", Some(tracks), &AtomicBool::new(true)).is_err()
    );
    assert!(
        FilteredTracks::prepare(&session, "", Some(reused), &cancel)
            .expect("unfiltered")
            .is_none()
    );
}
