//! Read-only visual navigation over one immutable, already shaped layout.
use super::*;

#[derive(Clone)]
pub struct TextLayoutCursor {
    visual: Arc<TextInputVisual>,
    cursor: parley::editing::Cursor,
}

impl fmt::Debug for TextLayoutCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TextLayoutCursor")
            .field("byte", &self.byte_index())
            .field("line", &self.line_index())
            .finish()
    }
}

impl TextLayoutCursor {
    pub fn byte_index(&self) -> usize {
        self.cursor.index()
    }

    /// Selection decorations share their glyph layout; reshaping does not.
    pub fn belongs_to(&self, snapshot: &TextInputSnapshot) -> bool {
        Arc::ptr_eq(&self.visual, &snapshot.0)
    }

    pub fn bounds(&self) -> Rect {
        rect(self.cursor.geometry(&self.visual.layout, 1.0))
    }

    pub fn line_index(&self) -> Option<usize> {
        let bounds = self.bounds();
        let y = bounds.y + bounds.height * 0.5;
        self.visual.layout.lines().position(|line| {
            let metrics = line.metrics();
            y >= metrics.min_coord && y < metrics.max_coord
        })
    }

    pub fn line_count(&self) -> usize {
        self.visual.layout.len()
    }

    /// Retain visual affinity when a document snaps to its grapheme boundary.
    pub fn at_byte_index(&self, byte: usize) -> Result<Self, StyledTextError> {
        let cursor = parley::editing::Cursor::from_byte_index(
            &self.visual.layout,
            byte,
            self.cursor.affinity(),
        );
        if cursor.index() != byte || self.line_count() == 0 {
            return Err(StyledTextError::InvalidRange);
        }
        Ok(Self {
            visual: self.visual.clone(),
            cursor,
        })
    }

    /// Exact Parley point navigation on an explicit visual line.
    pub fn on_line(&self, line: usize, x: f32) -> Option<Self> {
        if !x.is_finite() {
            return None;
        }
        let target = self.visual.layout.get(line)?;
        let metrics = target.metrics();
        let y = metrics.max_coord - metrics.ascent * 0.5;
        Some(Self {
            visual: self.visual.clone(),
            cursor: parley::editing::Cursor::from_point(&self.visual.layout, x, y),
        })
    }
}

impl TextInputSnapshot {
    pub fn layout_cursor(&self, byte: usize) -> Result<TextLayoutCursor, StyledTextError> {
        TextLayoutCursor {
            visual: self.0.clone(),
            cursor: parley::editing::Cursor::from_byte_index(
                &self.0.layout,
                byte,
                parley::Affinity::Downstream,
            ),
        }
        .at_byte_index(byte)
    }

    pub fn layout_cursor_at(&self, point: Point) -> Option<TextLayoutCursor> {
        if !point.x.is_finite() || !point.y.is_finite() || self.0.layout.is_empty() {
            return None;
        }
        Some(TextLayoutCursor {
            visual: self.0.clone(),
            cursor: parley::editing::Cursor::from_point(&self.0.layout, point.x, point.y),
        })
    }
}
