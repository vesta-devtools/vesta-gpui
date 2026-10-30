use std::error::Error;
use vesta_gpui::{
    FontRole, StyledTextError, TEXT_INPUT_MAX_BYTES, TextInputContext, TextInputError,
    TextInputState, TextInputViewError,
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
fn rejected_edit_composes_with_host_errors_without_erasing_its_typed_cause_or_buffer() {
    fn host_error<E: Error + Send + Sync + 'static>() {}
    host_error::<StyledTextError>();
    host_error::<TextInputViewError>();
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
}
