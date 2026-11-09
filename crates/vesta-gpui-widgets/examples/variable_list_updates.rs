//! Equal-input CPU comparison of point updates plus scroll-anchor restoration.
//! No window, fonts, renderer, process CPU measurement or FPS assertion.

use std::{
    hint::black_box,
    time::{Duration, Instant},
};
use vesta_gpui_widgets::VariableListMetrics;

const UPDATES: usize = 256;
const QUERY_COUNT: usize = 20_000;
const BLOCKS: usize = 4;

#[derive(Clone, Copy, Debug)]
enum Mode {
    Rebuild,
    Incremental,
}

fn updates(count: usize, mode: Mode) -> (Duration, u64) {
    let mut heights = vec![20.0; count];
    let mut metrics = VariableListMetrics::new(&heights).expect("bounded rows");
    let mut offset = count as f32 * 10.0 + 7.0;
    let mut signature = 0_u64;
    let started = Instant::now();
    for step in 0..UPDATES {
        let anchor = metrics
            .range(140.0, offset, 2)
            .expect("old viewport")
            .anchor()
            .expect("anchor");
        let index = step.wrapping_mul(7_919) % (count / 2);
        heights[index] = if step % 2 == 0 { 48.0 } else { 24.0 };
        match mode {
            Mode::Rebuild => {
                metrics = VariableListMetrics::new(black_box(&heights)).expect("rebuild")
            }
            Mode::Incremental => {
                metrics
                    .update_height(black_box(index), heights[index])
                    .expect("update");
            }
        }
        offset = metrics
            .offset_for_anchor(anchor, anchor.index(), 140.0)
            .expect("same stable row");
        let range = metrics.range(140.0, offset, 2).expect("updated viewport");
        signature = signature
            .wrapping_add(u64::from(black_box(offset).to_bits()))
            .wrapping_add(range.visible_rows().start as u64)
            .wrapping_add(u64::from(metrics.content_height().to_bits()));
    }
    let elapsed = started.elapsed();
    let rebuilt = VariableListMetrics::new(&heights).expect("independent full reconstruction");
    assert_eq!(metrics, rebuilt);
    (elapsed, signature)
}

fn appends(count: usize, mode: Mode) -> (Duration, u64) {
    let mut heights = vec![20.0; count];
    let mut metrics = VariableListMetrics::new(&heights).expect("bounded source");
    let mut offset = count as f32 * 10.0 + 7.0;
    let mut signature = 0_u64;
    let started = Instant::now();
    for step in 0..UPDATES {
        let anchor = metrics
            .range(140.0, offset, 2)
            .expect("old viewport")
            .anchor()
            .expect("row");
        let height = 20.0 + (step % 7) as f32;
        let index = heights.len();
        heights.push(height);
        match mode {
            Mode::Rebuild => {
                metrics = VariableListMetrics::new(black_box(&heights)).expect("full index")
            }
            Mode::Incremental => {
                metrics
                    .splice(index..index, black_box(&[height]))
                    .expect("append");
            }
        }
        offset = metrics
            .offset_for_anchor(anchor, anchor.index(), 140.0)
            .expect("same viewed record");
        signature = signature
            .wrapping_add(u64::from(offset.to_bits()))
            .wrapping_add(u64::from(metrics.content_height().to_bits()))
            .wrapping_add(
                metrics
                    .range(140.0, offset, 2)
                    .expect("new view")
                    .visible_rows()
                    .start as u64,
            );
    }
    let elapsed = started.elapsed();
    assert_eq!(
        metrics,
        VariableListMetrics::new(&heights).expect("independent reference")
    );
    (elapsed, signature)
}

fn queries(count: usize) -> (Duration, Duration) {
    let heights = (0..count)
        .map(|i| 20.0 + (i % 4) as f32)
        .collect::<Vec<_>>();
    let metrics = VariableListMetrics::new(&heights).expect("measured rows");
    // Original full-prefix representation, for the cost of steady range lookup.
    let mut edges = vec![0.0];
    let mut total = 0.0_f64;
    for height in heights {
        total += f64::from(height);
        edges.push(total as f32);
    }
    let mut signatures = [0_usize; 2];
    let prefix_start = Instant::now();
    for step in 0..QUERY_COUNT {
        let offset = (step * 7_919 % (count - 20)) as f32 * 20.0;
        let first = edges
            .partition_point(|edge| *edge <= black_box(offset))
            .saturating_sub(1);
        let end = edges.partition_point(|edge| *edge < offset + 140.0);
        signatures[0] =
            signatures[0].wrapping_add(black_box(first.saturating_sub(2) + (end + 2).min(count)));
    }
    let prefix = prefix_start.elapsed();
    let index_start = Instant::now();
    for step in 0..QUERY_COUNT {
        let offset = (step * 7_919 % (count - 20)) as f32 * 20.0;
        let range = metrics
            .range(140.0, black_box(offset), 2)
            .expect("bounded query")
            .visible_rows();
        signatures[1] = signatures[1].wrapping_add(black_box(range.start + range.end));
    }
    let indexed = index_start.elapsed();
    assert_eq!(signatures[0], signatures[1]);
    (prefix, indexed)
}

fn main() {
    println!("rows,block,mode,update_and_anchor_us_per_step,signature");
    for count in [1_000, 100_000] {
        // Warm both paths, then alternate order to expose machine-load variation.
        assert_eq!(
            updates(count, Mode::Rebuild).1,
            updates(count, Mode::Incremental).1
        );
        for block in 0..BLOCKS {
            let order = if block % 2 == 0 {
                [Mode::Rebuild, Mode::Incremental]
            } else {
                [Mode::Incremental, Mode::Rebuild]
            };
            let mut expected = None;
            for mode in order {
                let (elapsed, signature) = updates(count, mode);
                assert_eq!(*expected.get_or_insert(signature), signature);
                println!(
                    "{count},{block},{mode:?},{:.3},{signature}",
                    elapsed.as_secs_f64() * 1e6 / UPDATES as f64
                );
            }
        }
        let (prefix, indexed) = queries(count);
        println!(
            "query_ns_per_lookup rows={count} original_prefix={:.1} block_index={:.1}",
            prefix.as_secs_f64() * 1e9 / QUERY_COUNT as f64,
            indexed.as_secs_f64() * 1e9 / QUERY_COUNT as f64
        );
        assert_eq!(
            appends(count, Mode::Rebuild).1,
            appends(count, Mode::Incremental).1
        );
        for block in 0..BLOCKS {
            let order = if block % 2 == 0 {
                [Mode::Rebuild, Mode::Incremental]
            } else {
                [Mode::Incremental, Mode::Rebuild]
            };
            let mut expected = None;
            for mode in order {
                let (elapsed, signature) = appends(count, mode);
                assert_eq!(*expected.get_or_insert(signature), signature);
                println!(
                    "append rows={count} block={block} mode={mode:?} us_per_step={:.3} signature={signature}",
                    elapsed.as_secs_f64() * 1e6 / UPDATES as f64
                );
            }
        }
    }
}
