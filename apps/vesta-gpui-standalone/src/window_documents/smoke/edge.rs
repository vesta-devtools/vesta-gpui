//! Native final-step proof with retained scroll space beyond the Document.
use super::*;

pub(crate) enum Phase {
    Position {
        item: ItemId,
        now: Instant,
    },
    Press {
        item: ItemId,
        now: Instant,
        limit: f32,
    },
    Tick {
        item: ItemId,
        now: Instant,
        limit: f32,
        before: f32,
        held: Point,
    },
    Submitted {
        item: ItemId,
        now: Instant,
        limit: f32,
        before: f32,
        held: Point,
    },
}

impl App {
    pub(super) fn advance_document_edge_smoke(&mut self, phase: Phase) -> Result<bool, String> {
        match phase {
            Phase::Position { item, now } => {
                let remaining = self
                    .with_document_input(STANDALONE_WINDOW, |interaction, surface, _| {
                        let node = surface
                            .layout
                            .node(interaction.target().element)
                            .ok_or("edge Document geometry absent")?;
                        let clip = node
                            .rect
                            .intersection(node.inherited_clip)
                            .ok_or("edge Document clip absent")?;
                        Ok::<_, String>(node.rect.bottom() - clip.bottom())
                    })
                    .ok_or("edge owner absent")??;
                let state = self
                    .demo
                    .page_scroll
                    .get_mut(&item)
                    .ok_or("edge scroll absent")?;
                let limit = state.offset_y() + remaining;
                if remaining <= 3.0 || state.max_y() <= limit + 3.0 {
                    return Err(
                        "edge fixture lacks offscreen text or following sibling space".into(),
                    );
                }
                state.scroll_by_point(Point::new(0.0, remaining - 3.0));
                self.document_smoke = Smoke::Edge(Phase::Press { item, now, limit });
            }
            Phase::Press { item, now, limit } => {
                let theme = self.theme;
                let scope = self.scopes.markdown;
                let held = self
                    .with_document_input(STANDALONE_WINDOW, |interaction, surface, input| {
                        let node = surface
                            .layout
                            .node(interaction.target().element)
                            .ok_or("near-edge Document absent")?;
                        let clip = node
                            .rect
                            .intersection(node.inherited_clip)
                            .ok_or("edge clip")?;
                        let remaining = node.rect.bottom() - clip.bottom();
                        if (remaining - 3.0).abs() > 0.01 {
                            return Err(format!("near-edge submitted distance is {remaining}"));
                        }
                        let block = surface
                            .document
                            .block_id_at(surface.range.end - 1)
                            .ok_or("last block")?;
                        let id = surface
                            .document
                            .paragraph_id(scope, block)
                            .map_err(|error| error.to_string())?;
                        let root = interaction.view(surface.document, surface.range.clone(), theme);
                        let paragraph = surface.layout.node(id).ok_or("last paragraph geometry")?;
                        let Some(ElementKind::TextInput { snapshot, .. }) =
                            root.find(id).map(|element| &element.kind)
                        else {
                            return Err("last paragraph snapshot absent".into());
                        };
                        let visible = snapshot
                            .viewport(paragraph.rect, paragraph.inherited_clip, Point::default())
                            .clip;
                        if visible.width <= 0.0 || visible.height <= 0.0 {
                            return Err("last paragraph is not visible".into());
                        }
                        let point = Point::new(
                            visible.x + visible.width * 0.5,
                            visible.y + visible.height * 0.5,
                        );
                        if interaction
                            .press(&surface, point, false, input)
                            .map_err(|error| error.to_string())?
                            != MarkdownPointerResult::Selection
                        {
                            return Err("near-edge press rejected".into());
                        }
                        let held = Point::new(point.x, clip.bottom() + 80.0);
                        interaction
                            .pointer_move(&surface, held, input)
                            .map_err(|error| error.to_string())?;
                        Ok::<_, String>(held)
                    })
                    .ok_or("near-edge owner absent")??;
                let now = now + Duration::from_millis(20);
                if self
                    .tick_window_document_drag(STANDALONE_WINDOW, now)
                    .is_none()
                {
                    return Err("near-edge deadline absent".into());
                }
                let before = self.demo.page_scroll[&item].offset_y();
                self.document_smoke = Smoke::Edge(Phase::Tick {
                    item,
                    now,
                    limit,
                    before,
                    held,
                });
            }
            Phase::Tick {
                item,
                now,
                limit,
                before,
                held,
            } => {
                let now = now + Duration::from_millis(20);
                self.tick_window_document_drag(STANDALONE_WINDOW, now);
                self.document_smoke = Smoke::Edge(Phase::Submitted {
                    item,
                    now,
                    limit,
                    before,
                    held,
                });
            }
            Phase::Submitted {
                item,
                now,
                limit,
                before,
                held,
            } => {
                let next = self.demo.page_scroll[&item].offset_y();
                if (next - limit).abs() > 0.01 || (next - before - 3.0).abs() > 0.01 {
                    return Err(format!(
                        "native final step exceeded its Document boundary: {before} -> {next}, limit {limit}"
                    ));
                }
                if self
                    .tick_window_document_drag(STANDALONE_WINDOW, now + Duration::from_millis(20))
                    .is_some()
                    || self.demo.page_scroll[&item].offset_y() != next
                {
                    return Err("native Document edge retained a scrolling deadline".into());
                }
                self.with_document_input(STANDALONE_WINDOW, |interaction, surface, input| {
                    if interaction
                        .release(&surface, held, PointerButton::Primary, input)
                        .map_err(|error| error.to_string())?
                        != MarkdownPointerResult::Selection
                        || interaction.drag_scroll_deadline().is_some()
                        || input.capture().is_some()
                    {
                        return Err("native edge release retained ownership".into());
                    }
                    Ok::<_, String>(())
                })
                .ok_or("edge release owner absent")??;
                eprintln!(
                    "UI-F02 drag edge submitted: frames={} remaining=3 step={} offset_after={next} document_limit={limit} deadline_none=true released=true supplied_time=true pointer_io=false clipboard_io=false",
                    self.rendered_frames,
                    next - before
                );
                return Ok(true);
            }
        }
        self.request_all_redraw();
        Ok(false)
    }
}
