//! Validation must decorate the supplied value without replacing Host semantics or ownership.
use super::*;

pub(super) fn preserves_host_context_content_and_admission() {
    let message = "范围由宿主判断 / keep the complete diagnostic";
    let cases = [
        (
            WidgetTheme::dark(),
            PropertyRowPresentation::Standard,
            true,
            true,
            None,
            Some("Host context"),
        ),
        (
            WidgetTheme::light(),
            PropertyRowPresentation::Standard,
            true,
            true,
            Some(ValidationSeverity::Warning),
            Some("Host context"),
        ),
        (
            WidgetTheme::dark(),
            PropertyRowPresentation::Standard,
            false,
            true,
            Some(ValidationSeverity::Error),
            Some("Host context"),
        ),
        (
            WidgetTheme::light(),
            PropertyRowPresentation::Compact,
            true,
            true,
            Some(ValidationSeverity::Info),
            Some(""),
        ),
        (
            WidgetTheme::dark(),
            PropertyRowPresentation::Compact,
            true,
            false,
            Some(ValidationSeverity::Error),
            None,
        ),
    ];
    for (theme, presentation, enabled, visible, severity, context) in cases {
        let mut value = button(
            identity(820),
            identity(821),
            "Host-owned value",
            2_u8,
            ControlState::Idle,
            theme,
        );
        value
            .element
            .semantic
            .as_mut()
            .expect("value semantic")
            .description = context.map(str::to_owned);
        // A typed value's intentional appearance belongs to the caller, even in a disabled row.
        value.element.children[0].style.visual.font_size = 17.5;
        value.element.children[0].style.visual.text_color = vesta_gpui::Color::rgb8(129, 71, 164);
        let content = value.element.children[0].clone();
        let result = property_row_with_presentation(
            PropertyRowIds {
                root: identity(800),
                label: identity(801),
                validation: identity(802),
            },
            PropertyRowState {
                label: "Host property".into(),
                availability: WidgetAvailability { visible, enabled },
                validation: severity.map(|severity| PropertyValidation {
                    severity,
                    message: message.into(),
                }),
            },
            value,
            presentation,
            theme,
        );
        if !visible {
            assert!(
                result.is_none(),
                "hidden validation must not recreate a value or route"
            );
            continue;
        }
        let row = result.expect("visible property");
        let projected_value = row
            .element
            .find(ElementId::new(820))
            .expect("same value owner");
        let expected_context = match (context.filter(|s| !s.is_empty()), severity) {
            (Some(context), Some(_)) => Some(format!("{context}; {message}")),
            (_, Some(_)) => Some(message.to_owned()),
            (context, None) => context.map(str::to_owned),
        };
        assert_eq!(
            projected_value
                .semantic
                .as_ref()
                .expect("value semantic retained")
                .description,
            expected_context
        );
        let projected_content = row
            .element
            .find(ElementId::new(821))
            .expect("caller content retained");
        assert_eq!(projected_content.kind, content.kind);
        assert_eq!(projected_content.style, content.style);
        match (row.element.find(ElementId::new(802)), severity) {
            (Some(hint), Some(_)) => {
                assert_eq!(
                    hint.semantic.as_ref().expect("diagnostic semantic").label,
                    message
                );
                assert_eq!(hint.tooltip.as_deref(), Some(message));
            }
            (None, None) => {}
            _ => panic!("only an actual Host validation may create its diagnostic hint"),
        }
        let route =
            ActionRoute::build(&row.element, row.actions).expect("stable value and row identities");
        assert_eq!(route.action_at(ElementId::new(820)), enabled.then_some(&2));
        assert_eq!(
            route.action_for_hit(ElementId::new(821)),
            enabled.then_some(&2)
        );
        assert_eq!(route.focus_order().contains(&ElementId::new(820)), enabled);
        assert!(
            !route.focus_order().contains(&ElementId::new(802)),
            "a diagnostic must not become another value command"
        );
    }
}
