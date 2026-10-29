//! Caller-owned mixed values composed through the public property and value widgets.
use super::{Source, debug_error, require};
use vesta_gpui::{
    ActionRoute, Axis, Dimension, Element, ElementKey, ElementKind, FocusScope, HitTestTree,
    InputTarget, LayoutTree, Point, PointerButton, Style, TextEditKind, TextInputAction,
    TextInputContext, TextInputState, TextInputViews, TextSessionTransition, UiWindowId, Viewport,
    WindowTextSession,
};
use vesta_gpui_widgets::{
    ControlState, DraftResolution, NumberFieldIds, PointerEditOutcome, PropertyRowIds,
    PropertyRowState, ValueEditSession, VectorComponentEdit, VectorComponentValue, VectorFieldEdit,
    VectorFieldIds, VectorFieldSpec, Widget, WidgetAvailability, WidgetIdentity, WidgetTheme,
    editable_number_field, editable_vector_field, property_row_widget, resolve_value_draft,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Component {
    X,
    Y,
    Z,
}

impl Component {
    const ALL: [Self; 3] = [Self::X, Self::Y, Self::Z];

    fn index(self) -> usize {
        match self {
            Self::X => 0,
            Self::Y => 1,
            Self::Z => 2,
        }
    }

    fn base(self) -> u32 {
        match self {
            Self::X => 2_000,
            Self::Y => 2_020,
            Self::Z => 2_040,
        }
    }

    fn target(self, source: &Source) -> InputTarget {
        source.target(self.base() + 1)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NumericAction {
    Scalar(i8),
    Component(Component, i8),
}

struct Projection<'a> {
    values: &'a [Option<f32>; 3],
    drafts: &'a [TextInputState; 3],
    scalar: &'a TextInputState,
    vector_first: bool,
    availability: WidgetAvailability,
}

struct NumericFrame {
    root: Element,
    route: ActionRoute<NumericAction>,
    layout: LayoutTree,
    views: TextInputViews<Component>,
}

fn frame(
    source: &mut Source,
    context: &mut TextInputContext,
    projection: Projection<'_>,
) -> Result<NumericFrame, String> {
    let identity = |slot| WidgetIdentity::new(source.id(slot), format!("numeric.{slot}"));
    let number_ids = |base| NumberFieldIds {
        root: identity(base),
        value_label: identity(base + 1),
        decrement: identity(base + 2),
        decrement_label: identity(base + 3),
        increment: identity(base + 4),
        increment_label: identity(base + 5),
    };
    let theme = WidgetTheme::default();
    let vector = editable_vector_field(
        VectorFieldIds {
            root: identity(1_900),
            axis_labels: [identity(1_901), identity(1_902), identity(1_903)],
            components: Component::ALL.map(|component| number_ids(component.base())),
        },
        VectorFieldSpec {
            label: "Position",
            values: projection.values.map(|value| {
                value.map_or(VectorComponentValue::Mixed, VectorComponentValue::Value)
            }),
            unit: "m",
            precision: 3,
            state: ControlState::Idle,
        },
        VectorFieldEdit {
            components: Component::ALL.map(|component| VectorComponentEdit {
                snapshot: projection.drafts[component.index()].snapshot(),
                focused: source.input.focus() == Some(component.target(source)),
            }),
            min: -999.0,
            max: 999.0,
            step: 1.0,
        },
        Component::ALL.map(|component| {
            [
                NumericAction::Component(component, -1),
                NumericAction::Component(component, 1),
            ]
        }),
        theme,
    );
    let scalar = editable_number_field(
        number_ids(1_000),
        projection.scalar.snapshot(),
        false,
        8.0,
        -999.0,
        999.0,
        1.0,
        NumericAction::Scalar(-1),
        NumericAction::Scalar(1),
        ControlState::Idle,
        theme,
    );
    let row = |base, value, availability| {
        property_row_widget(
            PropertyRowIds {
                root: identity(base),
                label: identity(base + 1),
                validation: identity(base + 2),
            },
            PropertyRowState {
                label: "Value".into(), // Duplicate labels must not address the editors.
                availability,
                validation: None,
            },
            value,
            theme,
        )
        .map(|mut row| {
            row.element.style.max_width = Dimension::Px(700.0);
            row
        })
    };
    let mut rows = [
        row(3_000, scalar, WidgetAvailability::default()),
        row(3_010, vector, projection.availability),
    ];
    let mut root = Widget::from_element(
        Element::container(source.id(900), ElementKey::new("numeric.form")).styled(
            Style::default()
                .size(Dimension::Px(700.0), Dimension::Px(160.0))
                .axis(Axis::Column),
        ),
    );
    for index in if projection.vector_first {
        [1, 0]
    } else {
        [0, 1]
    } {
        if let Some(row) = rows[index].take() {
            root = root.child(row);
        }
    }
    let layout = source
        .layout
        .compute_with_text_context(&root.element, Viewport::new(720.0, 180.0, 1.0), context)
        .map_err(debug_error)?;
    let geometry = layout
        .nodes()
        .iter()
        .filter_map(|node| match &root.element.find(node.id)?.kind {
            ElementKind::TextInput { snapshot, .. } => Some((
                node.id,
                snapshot.viewport(node.rect, node.inherited_clip, Point::default()),
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    let views = TextInputViews::from_layout(source.window, &geometry, |element| {
        Component::ALL
            .into_iter()
            .find(|component| component.target(source).element == element)
    })
    .map_err(debug_error)?;
    let route = ActionRoute::build(&root.element, root.actions).map_err(debug_error)?;
    FocusScope::new(source.window, &route, None).reconcile(&mut source.input);
    Ok(NumericFrame {
        root: root.element,
        route,
        layout,
        views,
    })
}

pub(super) fn qualify(context: &mut TextInputContext) -> Result<(), String> {
    for component in Component::ALL {
        for hidden in [false, true] {
            let mut first = Source::new(UiWindowId(31))?;
            let mut second = Source::new(UiWindowId(47))?;
            let values = [Some(1.0), None, Some(3.0)];
            let mut drafts = std::array::from_fn(|_| TextInputState::new(13.0, "sans-serif"));
            for (state, value) in drafts.iter_mut().zip(values) {
                state
                    .reset(
                        context,
                        &value.map_or_else(String::new, |value| value.to_string()),
                    )
                    .map_err(debug_error)?;
            }
            let mut scalar = TextInputState::new(13.0, "sans-serif");
            scalar.reset(context, "8").map_err(debug_error)?;
            let owner = component.target(&first);
            first.input.set_focus(Some(owner));
            first.input.capture_pointer(owner, PointerButton::Primary);
            let foreign = component.target(&second);
            second.input.set_focus(Some(foreign));
            let mut edit = ValueEditSession::new(owner, values[component.index()]);
            let before = frame(
                &mut first,
                context,
                Projection {
                    values: &values,
                    drafts: &drafts,
                    scalar: &scalar,
                    vector_first: false,
                    availability: WidgetAvailability::default(),
                },
            )?;
            let state = &mut drafts[component.index()];
            state.act(context, TextInputAction::SelectAll);
            state
                .insert(context, "17", TextEditKind::Typing)
                .map_err(debug_error)?;
            state.preedit(context, "未", None).map_err(debug_error)?;
            let draft = state.text().to_owned();
            let after = frame(
                &mut first,
                context,
                Projection {
                    values: &values,
                    drafts: &drafts,
                    scalar: &scalar,
                    vector_first: true,
                    availability: WidgetAvailability::default(),
                },
            )?;
            require(
                before.layout.node(owner.element).ok_or("old axis")?.rect.y
                    != after
                        .layout
                        .node(owner.element)
                        .ok_or("reordered axis")?
                        .rect
                        .y
                    && first.input.focus() == Some(owner)
                    && first
                        .input
                        .capture()
                        .is_some_and(|capture| capture.target == owner)
                    && after.views.field_at(owner) == Some(component)
                    && after
                        .views
                        .field_at(InputTarget {
                            window: second.window,
                            ..owner
                        })
                        .is_none(),
                "property reorder changed the stable axis, focus, capture or source geometry",
            )?;
            let rect = after.layout.node(owner.element).ok_or("current axis")?.rect;
            let hit = HitTestTree::build(&after.root, &after.layout).hit(Point::new(
                rect.x + rect.width / 2.0,
                rect.y + rect.height / 2.0,
            ));
            require(
                hit == Some(owner.element)
                    && after.route.action_for_hit(owner.element).is_none()
                    && Component::ALL.into_iter().all(|axis| {
                        after.route.action_at(first.id(axis.base() + 4))
                            == Some(&NumericAction::Component(axis, 1))
                    }),
                "reordered editor hit/action targets no longer match the semantic axis",
            )?;
            let _ = frame(
                &mut second,
                context,
                Projection {
                    values: &values,
                    drafts: &drafts,
                    scalar: &scalar,
                    vector_first: false,
                    availability: WidgetAvailability::default(),
                },
            )?;
            let transfer = TextSessionTransition::new(
                Some(WindowTextSession {
                    window: first.window,
                    field: component,
                }),
                Some(WindowTextSession {
                    window: second.window,
                    field: component,
                }),
            );
            require(
                !transfer.fields_changed()
                    && transfer.ime_owner_to_end() == Some(first.window)
                    && second.input.focus() == Some(foreign)
                    && drafts[component.index()].text() == draft
                    && drafts[component.index()].is_composing(),
                "foreign projection or same-field transfer changed the mixed draft/composition",
            )?;
            let state = &mut drafts[component.index()];
            state.cancel_compose(context);
            require(
                state.act(context, TextInputAction::Undo)
                    && state.text()
                        == values[component.index()].map_or_else(String::new, |v| v.to_string())
                    && state.act(context, TextInputAction::Redo)
                    && state.text() == "17",
                "reorder split or discarded the caller text history",
            )?;
            let unavailable = frame(
                &mut first,
                context,
                Projection {
                    values: &values,
                    drafts: &drafts,
                    scalar: &scalar,
                    vector_first: true,
                    availability: WidgetAvailability {
                        visible: !hidden,
                        enabled: false,
                    },
                },
            )?;
            require(
                first.input.focus().is_none()
                    && first.input.capture().is_none()
                    && !FocusScope::new(first.window, &unavailable.route, None).accepts(owner)
                    && (!hidden || unavailable.views.get(component).is_none())
                    && second.input.focus() == Some(foreign),
                "hidden/disabled numeric owner retained input or cleared the other source",
            )?;
            require(
                edit.finish(second.window, &Some(17.0), true).is_none()
                    && edit.finish(first.window, &Some(17.0), false)
                        == Some(PointerEditOutcome::Cancel {
                            restore: values[component.index()],
                        })
                    && edit.finish(first.window, &Some(17.0), true).is_none()
                    && values == [Some(1.0), None, Some(3.0)]
                    && scalar.text() == "8",
                "numeric owner retirement published or retargeted an unaccepted draft",
            )?;
            let mut accepted = values;
            let resolution = resolve_value_draft(
                drafts[component.index()].text(),
                &values[component.index()],
                |raw| raw.parse::<f32>().ok().filter(|v| v.is_finite()).map(Some),
            );
            let DraftResolution::Accepted { value, .. } = resolution else {
                return Err("finite caller validation failed".into());
            };
            let mut session = ValueEditSession::new(foreign, values[component.index()]);
            let Some(PointerEditOutcome::Commit { before, after }) =
                session.finish(second.window, &value, true)
            else {
                return Err("matching owner did not commit the validated axis".into());
            };
            accepted[component.index()] = after;
            require(
                before == values[component.index()]
                    && Component::ALL.into_iter().all(|axis| {
                        accepted[axis.index()]
                            == if axis == component {
                                Some(17.0)
                            } else {
                                values[axis.index()]
                            }
                    })
                    && session.finish(second.window, &value, true).is_none(),
                "one accepted terminal changed another axis or emitted a second commit",
            )?;
            println!(
                "numeric-composition axis={component:?} retirement={} reorder/source/draft/preedit/history/hit/terminal passed",
                if hidden { "hidden" } else { "disabled" }
            );
        }
    }
    Ok(())
}
