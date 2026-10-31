use std::error::Error;
use vesta_gpui::{
    ActionRoute, ActionRouteError, Element, ElementId, ElementKey, FontRole, KeymapError,
    StyledTextError, TEXT_INPUT_MAX_BYTES, TextInputContext, TextInputError, TextInputState,
    TextInputViewError, TextRasterError, UiQueryError,
};

fn host_replace(
    context: &mut TextInputContext,
    editor: &mut TextInputState,
    value: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    editor.reset(context, value)?;
    Ok(())
}

#[test]
fn rejected_operations_preserve_typed_causes_and_original_state() {
    fn host_error<E: Error + Send + Sync + 'static>() {}
    host_error::<StyledTextError>();
    host_error::<TextInputViewError>();
    host_error::<ActionRouteError>();
    host_error::<KeymapError>();
    host_error::<UiQueryError>();
    host_error::<TextRasterError>();
    let mut context = TextInputContext::default();
    let mut editor = TextInputState::default();
    host_replace(&mut context, &mut editor, "retained draft").expect("initial draft");
    let failure = host_replace(
        &mut context,
        &mut editor,
        &"x".repeat(TEXT_INPUT_MAX_BYTES + 1),
    )
    .expect_err("bounded edit rejected");
    assert_eq!(
        failure.downcast_ref::<TextInputError>(),
        Some(&TextInputError::TooLong)
    );
    assert!(!failure.to_string().is_empty());
    assert_eq!(editor.text(), "retained draft");
    let ellipsis = context
        .shape_static_text_ellipsized(
            &"x".repeat(TEXT_INPUT_MAX_BYTES + 1),
            14.0,
            FontRole::Ui,
            100.0,
        )
        .expect_err("source limit");
    assert_eq!(
        ellipsis
            .source()
            .and_then(|cause| cause.downcast_ref::<TextInputError>()),
        Some(&TextInputError::TooLong)
    );

    fn host_route(root: &Element) -> Result<ActionRoute<u8>, Box<dyn Error + Send + Sync>> {
        Ok(ActionRoute::build(root, [(root.id, 1), (root.id, 2)])?)
    }
    let root = Element::container(ElementId::new(101), ElementKey::new("host.root"));
    let rejected = host_route(&root).expect_err("ambiguous action rejected");
    assert_eq!(
        rejected.downcast_ref::<ActionRouteError>(),
        Some(&ActionRouteError::DuplicateAction(root.id))
    );
    assert!(rejected.source().is_none());
    assert!(!rejected.to_string().is_empty());
    let route = ActionRoute::build(&root, [(root.id, 1_u8)]).expect("valid action after rejection");
    assert_eq!(route.action_at(root.id), Some(&1));
}
