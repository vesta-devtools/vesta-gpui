//! Shared parameter/property-set shell around authoritative host projections.
use crate::{CollapsibleIds, Widget, WidgetAvailability, WidgetIdentity, WidgetTheme, collapsible};
use vesta_gpui::{Axis, Dimension, Element, Insets, Point, Semantic, SemanticRole, Style};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PropertySetIds {
    pub root: WidgetIdentity,
    pub title: WidgetIdentity,
    pub description: WidgetIdentity,
    pub scroll: WidgetIdentity,
    pub empty: WidgetIdentity,
}

/// The host supplies stable parameter/group identities and any existing value
/// widgets, including joined rows, variable-height text, ramps and repeated sets.
/// This shell neither interprets a parameter schema nor mutates author values.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertySetSection<A> {
    pub ids: CollapsibleIds,
    pub title: String,
    pub expanded: bool,
    pub toggle_action: A,
    pub availability: WidgetAvailability,
    pub content: Widget<A>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PropertySetSpec<A> {
    pub title: String,
    pub description: Option<String>,
    pub empty_text: String,
    pub scroll_offset: Point,
    pub sections: Vec<PropertySetSection<A>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PropertySetPresentation {
    #[default]
    Standard,
    Compact,
}

/// Rebuild from host-owned selection, expansion, scroll and typed controls.
/// Hidden/disabled sections remove descendant actions through the same shared
/// availability path as a property row; collapse removes the value subtree.
#[must_use]
pub fn property_set<A>(
    ids: PropertySetIds,
    spec: PropertySetSpec<A>,
    theme: WidgetTheme,
) -> Widget<A> {
    property_set_with_presentation(ids, spec, PropertySetPresentation::Standard, theme)
}

#[must_use]
pub fn property_set_with_presentation<A>(
    ids: PropertySetIds,
    spec: PropertySetSpec<A>,
    presentation: PropertySetPresentation,
    theme: WidgetTheme,
) -> Widget<A> {
    let compact = presentation == PropertySetPresentation::Compact;
    let label = |id: WidgetIdentity, text: String| {
        Element::text(id.element, id.key, &text)
            .semantic(Semantic::new(SemanticRole::Label, text))
            .styled(
                Style::default()
                    .size(Dimension::Percent(1.0), Dimension::Auto)
                    .text(theme.typography.font_size, theme.palette.text_muted)
                    .pointer_events(false),
            )
    };
    let mut root = Widget::from_element(
        Element::container(ids.root.element, ids.root.key)
            .semantic(Semantic::new(SemanticRole::Group, &spec.title))
            .styled(
                Style::default()
                    .axis(Axis::Column)
                    .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
                    .background(theme.palette.panel)
                    .clipped(),
            ),
    );
    let mut title = label(ids.title, spec.title);
    title.style = title
        .style
        .padding(Insets::all(12.0))
        .text(theme.typography.font_size, theme.palette.text);
    if compact {
        title.style = title
            .style
            .size(Dimension::Percent(1.0), Dimension::Px(30.0))
            .flex(0.0, 0.0)
            .padding(Insets::symmetric(6.0, 4.0))
            .background(theme.palette.surface)
            .text(10.0, theme.palette.text);
    }
    root = root.child(Widget::from_element(title));
    if let Some(text) = spec.description.filter(|s| !s.is_empty()) {
        let mut description = label(ids.description, text);
        description.style = description.style.padding(Insets::symmetric(12.0, 4.0));
        root = root.child(Widget::from_element(description));
    }
    let mut body = Widget::from_element(
        Element::scroll_box(ids.scroll.element, ids.scroll.key, spec.scroll_offset).styled(
            Style::default()
                .axis(Axis::Column)
                .gap(if compact { 1.0 } else { 4.0 })
                .padding(Insets::all(if compact { 4.0 } else { 6.0 }))
                .size(Dimension::Percent(1.0), Dimension::Px(0.0))
                .flex(1.0, 1.0),
        ),
    );
    let mut present = false;
    for section in spec.sections {
        if let Some(mut group) = collapsible(
            section.ids,
            section.title,
            section.expanded,
            section.content,
            section.toggle_action,
            theme,
        )
        .conditioned(section.availability)
        {
            if compact {
                group.element.style.gap = 1.0;
                group.element.style.visual.border_width = 0.0;
                let header = &mut group.element.children[0];
                header.style = header
                    .style
                    .clone()
                    .size(Dimension::Percent(1.0), Dimension::Px(22.0))
                    .padding(Insets::symmetric(4.0, 0.0))
                    .gap(4.0)
                    .background(theme.palette.field);
                if let Some(label) = header.children.last_mut() {
                    label.style = label.style.clone().text(10.0, theme.palette.text);
                }
            }
            present = true;
            body = body.child(group);
        }
    }
    if !present {
        body = body.child(Widget::from_element(label(ids.empty, spec.empty_text)));
    }
    root.child(body)
}
