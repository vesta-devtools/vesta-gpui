use crate::WidgetTheme;
use crate::document_text::{label, wrapped};
use vesta_gpui::{
    Axis, Color, Dimension, Element, ElementScope, FontRole, Style, TextHAlign, TextVAlign,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffViewMode {
    Source,
    Unified,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffLineKind {
    Source,
    Context,
    Added,
    Removed,
    Hunk,
    Metadata,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub old: Option<u64>,
    pub new: Option<u64>,
    pub text: String,
    pub kind: DiffLineKind,
}

impl DiffLine {
    pub fn source(number: Option<u64>, text: &str, added: bool) -> Self {
        Self {
            old: None,
            new: number,
            text: text.to_owned(),
            kind: if added {
                DiffLineKind::Added
            } else {
                DiffLineKind::Source
            },
        }
    }
}

struct Range {
    next: u64,
    remaining: u64,
}

impl Range {
    fn parse(value: &str, prefix: char) -> Option<Self> {
        let value = value.strip_prefix(prefix)?;
        let (start, count) = value.split_once(',').unwrap_or((value, "1"));
        let next = start.parse::<u64>().ok()?;
        let remaining = count.parse::<u64>().ok()?;
        if remaining > 0 && (next == 0 || next.checked_add(remaining - 1).is_none()) {
            return None;
        }
        Some(Self { next, remaining })
    }

    fn take(&mut self) -> Option<u64> {
        if self.remaining == 0 {
            return None;
        }
        let number = self.next;
        self.remaining -= 1;
        if self.remaining > 0 {
            self.next += 1;
        }
        Some(number)
    }
}

fn hunk(value: &str) -> Option<(Range, Range)> {
    let mut parts = value.strip_prefix("@@ ")?.split_whitespace();
    let old = Range::parse(parts.next()?, '-')?;
    let new = Range::parse(parts.next()?, '+')?;
    (parts.next()? == "@@").then_some((old, new))
}

pub fn parse_unified_diff(value: &str) -> Vec<DiffLine> {
    let mut ranges = None;
    value
        .lines()
        .map(|text| {
            let mut line = DiffLine {
                old: None,
                new: None,
                text: text.to_owned(),
                kind: DiffLineKind::Metadata,
            };
            if text.starts_with("@@") {
                ranges = hunk(text);
                if ranges.is_some() {
                    line.kind = DiffLineKind::Hunk;
                }
            } else if !text.starts_with("\\ ")
                && let Some((old, new)) = &mut ranges
            {
                match text.as_bytes().first() {
                    Some(b'+') if new.remaining > 0 => {
                        line.new = new.take();
                        line.kind = DiffLineKind::Added;
                    }
                    Some(b'-') if old.remaining > 0 => {
                        line.old = old.take();
                        line.kind = DiffLineKind::Removed;
                    }
                    Some(b' ') if old.remaining > 0 && new.remaining > 0 => {
                        line.old = old.take();
                        line.new = new.take();
                        line.kind = DiffLineKind::Context;
                    }
                    _ => ranges = None,
                }
                if line.kind != DiffLineKind::Metadata {
                    line.text = text[1..].to_owned();
                }
            }
            line
        })
        .collect()
}

fn tint(color: Color, alpha: f32) -> Color {
    Color { alpha, ..color }
}

pub fn diff_view(
    scope: ElementScope,
    lines: &[DiffLine],
    range: std::ops::Range<usize>,
    mode: DiffViewMode,
    theme: WidgetTheme,
) -> Element {
    let diff = mode == DiffViewMode::Unified;
    let size = theme.typography.font_size;
    let mut content = Element::container(scope.element(0), scope.identity(0, "diff.lines").key)
        .styled(
            Style::default()
                .axis(Axis::Column)
                .size(Dimension::Percent(1.0), Dimension::Auto),
        );
    let digits = lines
        .iter()
        .flat_map(|line| [line.old, line.new])
        .flatten()
        .max()
        .unwrap_or(1)
        .to_string()
        .len()
        .max(3);
    let gutter_width = digits as f32 * size * (8.0 / 13.0) + 10.0;
    for (index, line) in lines
        .iter()
        .enumerate()
        .skip(range.start)
        .take(range.end.saturating_sub(range.start))
    {
        let mut next = 10000 + index as u32 * 6;
        let id = next;
        next += 1;
        let mut row = Element::container(
            scope.element(id),
            scope.identity(id, format!("preview.line.{index}")).key,
        )
        .styled(
            Style::default()
                .axis(Axis::Row)
                .gap(6.0)
                .size(Dimension::Percent(1.0), Dimension::Auto)
                .min_height(Dimension::Px(size * 1.5))
                .flex(0.0, 0.0),
        );
        let color = match line.kind {
            DiffLineKind::Added => Some(theme.palette.success),
            DiffLineKind::Removed => Some(theme.palette.danger),
            DiffLineKind::Hunk => Some(theme.palette.accent),
            _ => None,
        };
        row.style.visual.background = color.map(|color| tint(color, 0.14));
        let metadata = matches!(line.kind, DiffLineKind::Hunk | DiffLineKind::Metadata);
        if !metadata {
            for number in [line.old, line.new].into_iter().skip(usize::from(!diff)) {
                let mut gutter = label(
                    scope,
                    &mut next,
                    &number.map(|number| number.to_string()).unwrap_or_default(),
                    size,
                    theme.palette.text_muted,
                );
                gutter.style.width = Dimension::Px(gutter_width);
                gutter.style.flex_shrink = 0.0;
                gutter.style.text_h_align = TextHAlign::End;
                gutter.style.text_v_align = TextVAlign::Start;
                gutter.style.visual.font_role = FontRole::Mono;
                gutter.style.visual.background = color.map(|color| tint(color, 0.12));
                row = row.child(gutter);
            }
            if diff {
                let marker = match line.kind {
                    DiffLineKind::Added => "+",
                    DiffLineKind::Removed => "−",
                    _ => " ",
                };
                let mut marker = label(
                    scope,
                    &mut next,
                    marker,
                    size,
                    color.unwrap_or(theme.palette.text_muted),
                );
                marker.style.width = Dimension::Px(18.0);
                marker.style.flex_shrink = 0.0;
                marker.style.text_v_align = TextVAlign::Start;
                marker.style.visual.font_role = FontRole::Mono;
                row = row.child(marker);
            }
        }
        let mut body = wrapped(
            scope,
            &mut next,
            &line.text,
            size,
            if metadata {
                theme.palette.text_muted
            } else {
                theme.palette.text
            },
        );
        body.style.width = Dimension::Auto;
        body.style.min_width = Dimension::Px(0.0);
        body.style.min_height = Dimension::Px(size * 1.5);
        body.style.flex_grow = 1.0;
        body.style.flex_basis = Some(Dimension::Px(0.0));
        body.style.text_v_align = TextVAlign::Start;
        body.style.visual.font_role = FontRole::Mono;
        row = row.child(body);
        content = content.child(row);
    }
    content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unified_patch_retains_source_ranges_and_non_source_records() {
        let lines = parse_unified_diff(
            "diff --git a/中文.txt b/中文.txt\n--- a/中文.txt\n+++ b/中文.txt\n@@ -2,2 +5,3 @@ 标题\n keep\n-old\n+++literal\n+中文\n\\ No newline at end of file\n@@ -0,0 +1 @@\n+new file\n@@ -9 +0,0 @@\n-delete\nBinary files a/pic.png and b/pic.png differ\n@@ invalid\n+unparsed tail\n",
        );
        assert_eq!(lines[2].kind, DiffLineKind::Metadata);
        assert_eq!(lines[3].kind, DiffLineKind::Hunk);
        assert_eq!((lines[4].old, lines[4].new), (Some(2), Some(5)));
        assert_eq!((lines[5].old, lines[5].new), (Some(3), None));
        assert_eq!(
            lines[6],
            DiffLine {
                old: None,
                new: Some(6),
                text: "++literal".to_owned(),
                kind: DiffLineKind::Added
            }
        );
        assert_eq!(lines[7].new, Some(7));
        assert_eq!(lines[8].kind, DiffLineKind::Metadata);
        assert_eq!(lines[10].new, Some(1));
        assert_eq!(lines[12].old, Some(9));
        assert!(
            lines[13..]
                .iter()
                .all(|line| line.kind == DiffLineKind::Metadata
                    && line.old.is_none()
                    && line.new.is_none())
        );
        let exhausted = parse_unified_diff("@@ -1 +1 @@\n same\n+outside hunk\n");
        assert_eq!(exhausted[2].kind, DiffLineKind::Metadata);
        let truncated = parse_unified_diff("@@ -1,2 +1,2 @@\n one\n+partial");
        assert_eq!(truncated[2].new, Some(2));
        assert_eq!(truncated[2].text, "partial");
        let scope = vesta_gpui::ElementIdAllocator
            .allocate_scope()
            .expect("diff scope");
        let other = vesta_gpui::ElementIdAllocator
            .allocate_scope()
            .expect("other diff scope");
        let parent = vesta_gpui::ElementIdAllocator
            .allocate_scope()
            .expect("parent scope");
        let view = diff_view(
            scope,
            &lines,
            5..8,
            DiffViewMode::Unified,
            WidgetTheme::light(),
        );
        assert_eq!(view.children.len(), 3);
        let first_id = scope.element(10000 + 5 * 6);
        let first = view.find(first_id).expect("removed row");
        assert_eq!(
            first.key,
            diff_view(
                scope,
                &lines,
                4..6,
                DiffViewMode::Unified,
                WidgetTheme::dark()
            )
            .find(first_id)
            .expect("stable row")
            .key
        );
        assert_eq!(first.children.len(), 4);
        let source = [DiffLine::source(Some(12), "中文 source", false)];
        let source_view = diff_view(
            other,
            &source,
            0..1,
            DiffViewMode::Source,
            WidgetTheme::dark(),
        );
        assert_eq!(source_view.children[0].children.len(), 2);
        let combined = Element::container(parent.element(0), parent.identity(0, "diffs").key)
            .styled(Style::default().axis(Axis::Column))
            .child(view)
            .child(source_view);
        vesta_gpui::LayoutEngine
            .compute(&combined, vesta_gpui::Viewport::new(400.0, 500.0, 1.0))
            .expect("source and diff views have independent identities");
    }
}
