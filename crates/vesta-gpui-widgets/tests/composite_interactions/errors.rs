use super::*;
use std::error::Error;

#[test]
fn public_failures_compose_with_host_errors_and_preserve_edit_admission() {
    fn host_error<E: Error + Send + Sync + 'static>() {}
    host_error::<ModalStackError>();
    host_error::<ProgressStateError>();
    host_error::<RadioGroupError>();
    host_error::<RampError>();
    host_error::<SuggestionError>();
    host_error::<TableViewError>();
    host_error::<TileGridError>();
    host_error::<ToastQueueError>();
    host_error::<TreeViewError>();
    host_error::<PointerEditError>();
    host_error::<VirtualListError>();

    fn host_edit(origin: f32) -> Result<HorizontalEditSession<f32>, Box<dyn Error + Send + Sync>> {
        Ok(HorizontalEditSession::new(
            target(7, 11),
            origin,
            0.52,
            3.0,
        )?)
    }
    let failure = host_edit(f32::NAN).expect_err("invalid geometry rejected");
    assert_eq!(
        failure.downcast_ref::<PointerEditError>(),
        Some(&PointerEditError::InvalidGeometry)
    );
    assert!(failure.source().is_none());
    assert!(!failure.to_string().is_empty());
    let mut edit = host_edit(30.0).expect("new valid edit");
    assert_eq!(edit.initial(), &0.52);
    assert!(edit.finish(UiWindowId(8), &0.91, false).is_none());
    assert_eq!(
        edit.finish(UiWindowId(7), &0.91, false),
        Some(PointerEditOutcome::Cancel { restore: 0.52 })
    );
    assert!(edit.finish(UiWindowId(7), &0.91, true).is_none());

    use vesta_gpui::{ReadOnlyTextProjectionError, StyledTextError};
    let layout = MarkdownError::Layout(StyledTextError::InvalidRange);
    let anchor = MarkdownProjectionError::Anchor(MarkdownAnchorError::WrongWindow);
    let geometry = MarkdownProjectionError::Geometry(StyledTextError::InvalidRange);
    let accessibility = MarkdownProjectionError::Accessibility(
        ReadOnlyTextProjectionError::MissingNode(ElementId::new(9)),
    );
    assert_eq!(
        (
            layout
                .source()
                .and_then(|e| e.downcast_ref::<StyledTextError>())
                .copied(),
            anchor
                .source()
                .and_then(|e| e.downcast_ref::<MarkdownAnchorError>())
                .copied(),
            geometry
                .source()
                .and_then(|e| e.downcast_ref::<StyledTextError>())
                .copied(),
            accessibility
                .source()
                .and_then(|e| e.downcast_ref::<ReadOnlyTextProjectionError>())
                .copied(),
        ),
        (
            Some(StyledTextError::InvalidRange),
            Some(MarkdownAnchorError::WrongWindow),
            Some(StyledTextError::InvalidRange),
            Some(ReadOnlyTextProjectionError::MissingNode(ElementId::new(9))),
        ),
        "Markdown wrappers must retain their typed sources"
    );
    assert!(MarkdownError::IdentityExhausted.source().is_none());
}
