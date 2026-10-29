//! Sample source and bounded projection policy owned by this example host.
use super::*;

const PAGE_ROWS: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum DocumentAction {
    NextSample,
    PreviousPage,
    NextPage,
}

#[derive(Clone, Copy, Default, PartialEq)]
enum Sample {
    #[default]
    Brief,
    Long,
    Paged,
}

#[derive(Default)]
pub(super) struct Paging {
    sample: Sample,
    start: usize,
}

impl Paging {
    pub(super) fn is_paged(&self) -> bool {
        self.sample == Sample::Paged
    }

    pub(super) fn load_sample(
        &mut self,
        long: bool,
        document: &mut MarkdownView,
    ) -> Result<bool, MarkdownError> {
        if (self.sample != Sample::Brief) == long {
            return Ok(false);
        }
        let source = if long {
            long_source()
        } else {
            MARKDOWN.to_owned()
        };
        document.replace(&source)?;
        self.sample = if long { Sample::Long } else { Sample::Brief };
        self.start = 0;
        Ok(true)
    }
    pub(super) fn range(&self, len: usize) -> std::ops::Range<usize> {
        if self.sample != Sample::Paged {
            return 0..len;
        }
        let start = self.start.min(last_page(len));
        start.saturating_sub(1)..start.saturating_add(PAGE_ROWS + 1).min(len)
    }

    pub(super) fn follow(&mut self, row: usize, len: usize) -> bool {
        if self.sample != Sample::Paged {
            return false;
        }
        let before = self.range(len);
        self.start = row / PAGE_ROWS * PAGE_ROWS;
        self.range(len) != before
    }

    pub(super) fn apply(&mut self, action: DocumentAction, len: usize) -> bool {
        if action == DocumentAction::NextSample {
            if self.sample != Sample::Long {
                return false;
            }
            self.sample = Sample::Paged;
            self.start = 0;
            return true;
        }
        if self.sample != Sample::Paged {
            return false;
        }
        let before = self.range(len);
        self.start = match action {
            DocumentAction::PreviousPage => self.start.saturating_sub(PAGE_ROWS),
            DocumentAction::NextPage => self.start.saturating_add(PAGE_ROWS).min(last_page(len)),
            DocumentAction::NextSample => unreachable!("handled above"),
        };
        self.range(len) != before
    }

    pub(super) fn controls(
        &self,
        scope: ElementScope,
        len: usize,
        theme: WidgetTheme,
        actions: &mut Vec<(ElementId, DemoAction)>,
    ) -> Element {
        let identity = |slot, part| {
            WidgetIdentity::new(
                scope.element(slot),
                format!("documents.paging.{slot}.{part}"),
            )
        };
        let mut controls = Widget::from_element(
            Element::container(scope.element(3), ElementKey::new("documents.paging")).styled(
                Style::default()
                    .axis(Axis::Row)
                    .flex_wrap(vesta_gpui::FlexWrap::Wrap)
                    .gap(theme.metrics.spacing),
            ),
        );
        let next_label = match self.sample {
            Sample::Brief => "Long document",
            Sample::Long => "Paged document",
            Sample::Paged => "Brief document",
        };
        for (slot, part, label, action, enabled) in [
            (4, "sample", next_label, DocumentAction::NextSample, true),
            (
                6,
                "previous",
                "Previous page",
                DocumentAction::PreviousPage,
                self.sample == Sample::Paged && self.start > 0,
            ),
            (
                8,
                "next",
                "Next page",
                DocumentAction::NextPage,
                self.sample == Sample::Paged && self.start < last_page(len),
            ),
        ] {
            controls = controls.child(button(
                identity(slot, part),
                identity(slot + 1, part),
                label,
                DemoAction::Document(action),
                if enabled {
                    ControlState::Idle
                } else {
                    ControlState::Disabled
                },
                theme,
            ));
        }
        let range = self.range(len);
        let mode = match self.sample {
            Sample::Brief => "Brief",
            Sample::Long => "Long",
            Sample::Paged => "Paged",
        };
        controls = controls.child(Widget::from_element(text_label(
            identity(10, "range"),
            TextContent::Literal(&format!(
                "{mode}: rows {}–{} / {len}",
                if len == 0 { 0 } else { range.start + 1 },
                range.end
            )),
            theme,
        )));
        widget_element(controls, actions)
    }
}

fn last_page(len: usize) -> usize {
    len.saturating_sub(1) / PAGE_ROWS * PAGE_ROWS
}

fn long_source() -> String {
    use std::fmt::Write;
    let mut source = MARKDOWN.to_owned();
    for index in 1..=60 {
        writeln!(source, "\nParagraph {index} / 段落：English、中文、مرحبا 与 👨‍👩‍👧‍👦 共用稳定文档。A long line wraps at the current window width.").expect("write sample String");
    }
    source.push_str("\n| Ending | Value | Note |\n| :--- | ---: | :---: |\n| Last | 256 | **完成** |\n| Empty | | مرحبا 👨‍👩‍👧‍👦 |\n");
    source
}
