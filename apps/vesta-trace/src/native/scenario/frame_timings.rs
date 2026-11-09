//! Directed current-view acceptance over real frame/submission evidence.
use super::*;
use crate::query::protocol::{Data, FrameTimingSort};

impl App {
    fn frame_table(&self) -> Result<&Data> {
        self.report
            .as_ref()
            .and_then(|r| r.inspection.table.as_ref())
            .filter(|d| matches!(d, Data::FrameTimings { .. }))
            .ok_or_else(|| "trace_frame_table_missing".into())
    }
    fn check_frame_work(&self) -> Result<()> {
        let Data::FrameTimings { items, .. } = self.frame_table()? else {
            unreachable!()
        };
        let session = self.session.as_ref().ok_or("trace_session_missing")?;
        for row in items {
            let records: Vec<_> = session
                .gpu
                .as_ref()
                .map(|capture| {
                    capture
                        .submissions
                        .iter()
                        .filter(|s| {
                            u64::from(capture.process_id) == row.frame.key.process
                                && s.frame_id == Some(row.frame.key.event)
                        })
                        .collect()
                })
                .unwrap_or_default();
            let samples: Vec<_> = records
                .iter()
                .filter_map(|s| match s.outcome {
                    crate::gpu::GpuOutcome::Sample { elapsed_ns, .. } => {
                        Some(u128::from(elapsed_ns))
                    }
                    _ => None,
                })
                .collect();
            let total = (!samples.is_empty()).then(|| samples.iter().sum::<u128>());
            if row.gpu.records != records.len()
                || row.gpu.samples != samples.len()
                || row.gpu.sampled_work_ns.map(u128::from) != total
            {
                return Err("trace_frame_gpu_source_arithmetic_mismatch".into());
            }
        }
        Ok(())
    }
    pub(super) fn frame_timings_presented(
        &mut self,
        event_loop: &ActiveEventLoop,
        step: usize,
    ) -> Result<()> {
        if step == 22 {
            let (frame, key) = self.frame_probe.ok_or("trace_frame_probe_missing")?;
            let key = key.ok_or("trace_frame_gpu_probe_missing")?;
            let gpu = self
                .report
                .as_ref()
                .and_then(|r| r.inspection.gpu.as_ref())
                .ok_or("trace_frame_gpu_detail_missing")?;
            if self.panel != Panel::Gpu || gpu.key != key || gpu.frame != Some(frame) {
                return Err("trace_frame_gpu_drilldown_mismatch".into());
            }
            self.command(Command::FrameOrder(FrameTimingSort::StartAsc))?;
            self.command(Command::Fit)?;
            self.redraw();
            return Ok(());
        }
        self.check_frame_work()?;
        match step {
            19 => {
                let Data::FrameTimings { items, sort, .. } = self.frame_table()? else {
                    unreachable!()
                };
                if self.panel != Panel::Frames
                    || *sort != FrameTimingSort::CpuDurationDesc
                    || !items
                        .windows(2)
                        .all(|r| r[0].frame.duration_ns >= r[1].frame.duration_ns)
                {
                    return Err("trace_frame_cpu_order".into());
                }
                self.click_element(vesta_gpui::ElementId::new(146))?;
            }
            20 => {
                let Data::FrameTimings { items, sort, .. } = self.frame_table()? else {
                    unreachable!()
                };
                if *sort != FrameTimingSort::GpuWorkDesc
                    || !items
                        .windows(2)
                        .all(|r| r[0].gpu.sampled_work_ns >= r[1].gpu.sampled_work_ns)
                {
                    return Err("trace_frame_gpu_order".into());
                }
                let row = items.first().ok_or("trace_frame_row_missing")?;
                let gpu = row.gpu.longest_submission;
                let key = row.frame.key;
                let index = self
                    .session
                    .as_ref()
                    .ok_or("trace_session_missing")?
                    .indices[&key];
                self.frame_probe = Some((key, gpu));
                self.click_element(vesta_gpui::ElementId::new(30_000_000 + index as u64))?;
            }
            21 => {
                let (key, _) = self.frame_probe.ok_or("trace_frame_probe_missing")?;
                if self.timeline.as_ref().and_then(|t| t.selected) != Some(key) {
                    return Err("trace_frame_row_did_not_navigate".into());
                }
                if self.frame_probe.is_some_and(|(_, gpu)| gpu.is_some()) {
                    self.click_element(vesta_gpui::ElementId::new(152))?;
                } else {
                    self.smoke_step = 23;
                    self.command(Command::FrameOrder(FrameTimingSort::StartAsc))?;
                    self.command(Command::Fit)?;
                }
            }
            23 => {
                let Data::FrameTimings {
                    items, sort, page, ..
                } = self.frame_table()?
                else {
                    unreachable!()
                };
                if *sort != FrameTimingSort::StartAsc
                    || !items
                        .windows(2)
                        .all(|r| r[0].frame.start_ns <= r[1].frame.start_ns)
                {
                    return Err("trace_frame_start_order".into());
                }
                if let Some(cursor) = page.next_cursor.clone() {
                    self.command(Command::PageNext(cursor))?;
                } else {
                    self.redraw();
                }
            }
            24 => {
                let Data::FrameTimings { page, .. } = self.frame_table()? else {
                    unreachable!()
                };
                if self
                    .session
                    .as_ref()
                    .is_none_or(|s| Some(&s.source.sha256) != self.smoke_source.as_ref())
                    || self.page.as_ref().is_some_and(|p| p.offset != 100)
                    || page.returned == 0
                {
                    return Err("trace_frame_paging_or_identity".into());
                }
                println!(
                    "trace_frame_timings_complete command_origin=true source_arithmetic=true cpu_gpu_time_sort=true frame_navigation=true raw_gpu_drilldown={} paging={} minimum_layout=true",
                    self.frame_probe.is_some_and(|(_, gpu)| gpu.is_some()),
                    self.page.is_some()
                );
                println!(
                    "trace_inspection_complete command_origin=true steps=25 source_unchanged=true gpu_frame_join={} gpu_detail_scroll={} frame_timings=true",
                    self.session
                        .as_ref()
                        .and_then(|s| s.gpu.as_ref())
                        .is_some_and(|g| !g.submissions.is_empty()),
                    self.session
                        .as_ref()
                        .and_then(|s| s.gpu.as_ref())
                        .is_some_and(|g| !g.submissions.is_empty())
                );
                event_loop.exit();
            }
            _ => return Err("trace_frame_step_overrun".into()),
        }
        self.redraw();
        Ok(())
    }
}
