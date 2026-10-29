//! Visual line affinity and preferred columns belong to this projection only.
use super::*;

pub(super) struct VerticalNavigation {
    focus: MarkdownAnchor,
    cursor: TextLayoutCursor,
    preferred_x: Option<f32>,
}

impl VerticalNavigation {
    pub(super) fn from_pointer(focus: MarkdownAnchor, cursor: TextLayoutCursor) -> Self {
        Self {
            focus,
            cursor,
            preferred_x: None,
        }
    }
}

impl MarkdownInteraction {
    pub(super) fn move_vertical(
        &mut self,
        surface: &MarkdownSurface<'_>,
        chord: KeyChord,
    ) -> Result<bool, MarkdownProjectionError> {
        let document = surface.document;
        let range = document.block_range(surface.range.clone());
        let Some(first) = document
            .blocks
            .get(range.start)
            .filter(|_| !range.is_empty())
        else {
            return Ok(false);
        };
        let initial = document.anchor(first.id, 0)?;
        let current =
            self.selection
                .unwrap_or(document.selection(surface.window, initial, initial)?);
        document.selection_bounds(surface.window, current)?;
        let index = document.block_index(current.focus.block)?;
        let row = document.row_index(current.focus.block)?;
        if !range.contains(&index) {
            return Ok(false);
        }
        let (snapshot, viewport) =
            document.block_viewport(self.scope, surface.layout, current.focus.block)?;
        let retained = self.navigation.as_ref().filter(|navigation| {
            navigation.focus == current.focus && navigation.cursor.belongs_to(snapshot)
        });
        let cursor = match retained {
            Some(navigation) => navigation.cursor.clone(),
            None => snapshot
                .layout_cursor(current.focus.byte)
                .map_err(MarkdownProjectionError::Geometry)?,
        };
        let preferred_x = retained
            .and_then(|navigation| navigation.preferred_x)
            .unwrap_or(cursor.bounds().x + viewport.origin.x);
        let Some(line) = cursor.line_index() else {
            return Ok(false);
        };
        let down = matches!(chord.key, KeyStroke::ArrowDown | KeyStroke::PageDown);
        let adjacent = if down {
            (line + 1 < cursor.line_count()).then_some(line + 1)
        } else {
            line.checked_sub(1)
        };
        let (block, next) = if matches!(chord.key, KeyStroke::PageUp | KeyStroke::PageDown) {
            let Some((block, next)) =
                self.page_cursor(surface, &cursor, viewport.origin, preferred_x, down)?
            else {
                return Ok(false);
            };
            (block, Some(next))
        } else if let Some(line) = adjacent {
            (
                current.focus.block,
                cursor.on_line(line, preferred_x - viewport.origin.x),
            )
        } else if (down && row + 1 < surface.range.end.min(document.len()))
            || (!down && row > surface.range.start)
        {
            let Some((block, cursor)) = self.row_cursor(
                surface,
                if down { row + 1 } else { row - 1 },
                preferred_x,
                down,
            )?
            else {
                return Ok(false);
            };
            (block, Some(cursor))
        } else {
            let byte = if down {
                document.blocks[index].content.text.len()
            } else {
                0
            };
            (
                current.focus.block,
                Some(
                    cursor
                        .at_byte_index(byte)
                        .map_err(MarkdownProjectionError::Geometry)?,
                ),
            )
        };
        let Some(next) = next else {
            return Ok(false);
        };
        let (focus, cursor) = document.snap_cursor(block, next)?;
        let selection = document.selection(
            surface.window,
            if chord.modifiers.shift {
                current.anchor
            } else {
                focus
            },
            focus,
        )?;
        self.selection = Some(selection);
        self.navigation = Some(VerticalNavigation {
            focus,
            cursor,
            preferred_x: Some(preferred_x),
        });
        Ok(true)
    }

    fn row_cursor(
        &self,
        surface: &MarkdownSurface<'_>,
        row: usize,
        x: f32,
        down: bool,
    ) -> Result<Option<(MarkdownBlockId, TextLayoutCursor)>, MarkdownProjectionError> {
        let mut best: Option<(f32, MarkdownBlockId, TextLayoutCursor)> = None;
        for block in surface.document.blocks_in(row..row.saturating_add(1)) {
            let (snapshot, viewport) =
                surface
                    .document
                    .block_viewport(self.scope, surface.layout, block.id)?;
            let bounds = surface
                .layout
                .node(self.scope.element(block.id.0))
                .ok_or(MarkdownProjectionError::MissingLayout(block.id))?
                .rect;
            let distance = (x - x.clamp(bounds.x, bounds.right())).abs();
            let cursor = snapshot
                .layout_cursor(0)
                .map_err(MarkdownProjectionError::Geometry)?;
            let line = if down {
                0
            } else {
                cursor.line_count().saturating_sub(1)
            };
            let next = cursor
                .on_line(line, x - viewport.origin.x)
                .unwrap_or(cursor);
            if best.as_ref().is_none_or(|best| distance < best.0) {
                best = Some((distance, block.id, next));
            }
        }
        Ok(best.map(|(_, block, cursor)| (block, cursor)))
    }

    fn page_cursor(
        &self,
        surface: &MarkdownSurface<'_>,
        current: &TextLayoutCursor,
        origin: Point,
        preferred_x: f32,
        down: bool,
    ) -> Result<Option<(MarkdownBlockId, TextLayoutCursor)>, MarkdownProjectionError> {
        let Some(visible) = surface
            .layout
            .node(self.target.element)
            .and_then(|node| node.rect.intersection(node.inherited_clip))
        else {
            return Ok(None);
        };
        let current_bounds = current.bounds();
        let current_y = origin.y + current_bounds.y + current_bounds.height * 0.5;
        let distance = (visible.height - current_bounds.height).max(current_bounds.height);
        let target = Point::new(
            preferred_x,
            current_y + if down { distance } else { -distance },
        );
        let document = surface.document;
        let mut best: Option<((f32, f32), MarkdownBlockId, TextLayoutCursor)> = None;
        let mut edge = None;
        for block in document.blocks_in(surface.range.clone()) {
            // Validate every candidate before committing, including offscreen blocks.
            let (snapshot, viewport) =
                document.block_viewport(self.scope, surface.layout, block.id)?;
            let cursor = snapshot
                .layout_cursor_at(viewport.to_layout(target))
                .ok_or(MarkdownProjectionError::Geometry(
                    vesta_gpui::StyledTextError::InvalidRange,
                ))?;
            let bounds = cursor.bounds();
            let y = viewport.origin.y + bounds.y + bounds.height * 0.5;
            let in_direction = if down { y > current_y } else { y < current_y };
            let score = if block.content.cell.is_some() {
                let bounds = surface
                    .layout
                    .node(self.scope.element(block.id.0))
                    .ok_or(MarkdownProjectionError::MissingLayout(block.id))?
                    .rect;
                (
                    (target.y - target.y.clamp(bounds.y, bounds.bottom())).abs(),
                    (target.x - target.x.clamp(bounds.x, bounds.right())).abs(),
                )
            } else {
                ((y - target.y).abs(), 0.0)
            };
            if in_direction && best.as_ref().is_none_or(|best| score < best.0) {
                best = Some((score, block.id, cursor));
            }
            if down || edge.is_none() {
                edge = Some((
                    block.id,
                    snapshot
                        .layout_cursor(if down { block.content.text.len() } else { 0 })
                        .map_err(MarkdownProjectionError::Geometry)?,
                ));
            }
        }
        Ok(best.map(|(_, block, cursor)| (block, cursor)).or(edge))
    }

    /// Unclipped current focus geometry for the Host's existing scroll owners.
    /// It never serves as a pointer/link hit region or changes root focus.
    pub fn selection_focus_rect(
        &self,
        surface: &MarkdownSurface<'_>,
    ) -> Result<Option<Rect>, MarkdownProjectionError> {
        if surface.window != self.target.window {
            return Ok(None);
        }
        let Some(selection) = self.selection else {
            return Ok(None);
        };
        let document = surface.document;
        document.selection_bounds(surface.window, selection)?;
        if !surface
            .range
            .contains(&document.row_index(selection.focus.block)?)
        {
            return Ok(None);
        }
        let (snapshot, viewport) =
            document.block_viewport(self.scope, surface.layout, selection.focus.block)?;
        let cursor = match self.navigation.as_ref().filter(|navigation| {
            navigation.focus == selection.focus && navigation.cursor.belongs_to(snapshot)
        }) {
            Some(navigation) => navigation.cursor.clone(),
            None => snapshot
                .layout_cursor(selection.focus.byte)
                .map_err(MarkdownProjectionError::Geometry)?,
        };
        let bounds = cursor.bounds();
        Ok(Some(Rect::new(
            bounds.x + viewport.origin.x,
            bounds.y + viewport.origin.y,
            bounds.width,
            bounds.height,
        )))
    }
}
