use super::*;
use vesta_gpui_widgets::{
    TableBandIds, TableRetainedFocus, TableWindowRow, WideTableIds, table_view_columns,
};

fn bands(scope: ElementScope, base: u32) -> TableBandIds {
    TableBandIds {
        leading: identity(scope, base),
        scrolling: identity(scope, base + 1),
        trailing: identity(scope, base + 2),
    }
}

pub(super) fn cell_element(scope: ElementScope, row: u32, column: u16) -> ElementId {
    scope.element(TABLE_ROW_BASE + row * 4096 + 11 + u32::from(column) * 4)
}

pub(super) fn project(
    source: &Source,
    view: &View,
    scope: ElementScope,
    theme: WidgetTheme,
) -> Widget<DemoAction> {
    let geometry = source.geometry(view);
    let window = geometry
        .window(
            view.table_width,
            view.table_scroll.offset().x,
            1,
            view.flow(),
        )
        .expect("Host responsive frozen policy");
    let range = VirtualListRange::compute(
        source.rows.len(),
        ROW_HEIGHT,
        VIEW_HEIGHT,
        view.table_scroll.offset_y(),
        1,
    )
    .expect("bounded table rows");
    let retained = match view.focus {
        Some(Action::Cell(row, column)) => Some(TableRetainedFocus {
            row: source.rows.iter().position(|&id| id == row),
            column: source.columns.iter().position(|&id| id == column),
        }),
        Some(Action::Sort(column)) => Some(TableRetainedFocus {
            row: None,
            column: source.columns.iter().position(|&id| id == column),
        }),
        _ => None,
    };
    let rectangle = view.rectangle(source);
    let mut widget = table_view_columns(
        WideTableIds {
            table: TableWindowIds {
                root: identity(scope, 80_000_100),
                header: identity(scope, 80_000_101),
                viewport: identity(scope, TABLE_VIEWPORT),
                content: identity(scope, 80_000_103),
            },
            header_bands: bands(scope, 80_000_104),
        },
        &window,
        range,
        retained,
        |index| {
            let column = source.columns[index];
            let base = 80_001_000 + u32::from(column) * 4;
            let title = match column {
                0 => "Name".into(),
                511 => "Status".into(),
                _ => format!("Field {column}"),
            };
            let text = identity(scope, base + 1);
            TableColumnContent {
                header: identity(scope, base),
                width: source.widths[usize::from(column)],
                content: WidgetContent::new(
                    &title,
                    Widget::from_element(
                        Element::text(text.element, text.key, &title).styled(
                            Style::default()
                                .text(13.0, theme.palette.text)
                                .pointer_events(false),
                        ),
                    ),
                ),
                sort_action: Some(DemoAction::Sample(Action::Sort(column))),
                resize: None,
            }
        },
        |index, columns| {
            let row = source.rows[index];
            let base = TABLE_ROW_BASE + row * 4096;
            TableWindowRow {
                bands: bands(scope, base + 1),
                label: format!("Asset {row}"),
                row: TableRowContent {
                    identity: identity(scope, base),
                    selected: false,
                    select_action: DemoAction::Sample(Action::Cell(row, source.columns[0])),
                    cells: columns
                        .iter()
                        .map(|&column_index| {
                            let column = source.columns[column_index];
                            let base = base + 10 + u32::from(column) * 4;
                            let child = identity(scope, base + 1);
                            let text = Source::value(row, column);
                            TableCellContent {
                                identity: identity(scope, base),
                                selected: rectangle.as_ref().is_some_and(|(rows, columns)| {
                                    rows.contains(&index) && columns.contains(&column_index)
                                }),
                                select_action: None,
                                content: WidgetContent::new(
                                    &text,
                                    Widget::from_element(
                                        Element::text(child.element, child.key, &text)
                                            .semantic(Semantic::new(
                                                SemanticRole::Button,
                                                format!("Asset {row}, field {column}: {text}"),
                                            ))
                                            .styled(
                                                Style::default()
                                                    .size(
                                                        Dimension::Percent(1.0),
                                                        Dimension::Percent(1.0),
                                                    )
                                                    .focusable(true)
                                                    .text(13.0, theme.palette.text),
                                            ),
                                    )
                                    .on_click(DemoAction::Sample(Action::Cell(row, column))),
                                ),
                            }
                        })
                        .collect(),
                },
            }
        },
        theme,
    )
    .expect("bounded source column factories and explicit identities");
    // The next layout pass feeds the actual pane width back to this window's View.
    widget.element.style.width = Dimension::Percent(1.0);
    widget.element.style.max_width = Dimension::Percent(1.0);
    widget.element.semantic = Some(Semantic::new(SemanticRole::Table, "Assets"));
    widget
}

impl View {
    pub(super) fn flow(&self) -> FlowDirection {
        if self.rtl {
            FlowDirection::RightToLeft
        } else {
            FlowDirection::LeftToRight
        }
    }

    pub(super) fn prepare_reveal(&mut self, source: &Source, focus: Option<Action>) {
        match focus {
            Some(Action::Message(id)) => self.messages.reveal(source, id),
            Some(Action::Cell(row, column)) => {
                let Some(ri) = source.rows.iter().position(|&id| id == row) else {
                    return;
                };
                let Some(ci) = source.columns.iter().position(|&id| id == column) else {
                    return;
                };
                let geometry = source.geometry(self);
                let window = geometry
                    .window(
                        self.table_width,
                        self.table_scroll.offset().x,
                        1,
                        self.flow(),
                    )
                    .expect("bounded reveal geometry");
                let desired_x = window.offset_to_reveal(ci).expect("live source column");
                let y = ri as f32 * ROW_HEIGHT;
                let offset = self.table_scroll.offset_y();
                let desired_y = if y < offset {
                    y
                } else if y + ROW_HEIGHT > offset + VIEW_HEIGHT {
                    y + ROW_HEIGHT - VIEW_HEIGHT
                } else {
                    offset
                };
                self.table_scroll.scroll_by_point(Point::new(
                    self.flow()
                        .horizontal(desired_x - self.table_scroll.offset().x),
                    desired_y - offset,
                ));
            }
            _ => {}
        }
    }
}
