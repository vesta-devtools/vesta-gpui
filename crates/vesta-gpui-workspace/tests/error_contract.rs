use std::error::Error;
use vesta_gpui::{ElementId, InputTarget, Rect, UiWindowId};
use vesta_gpui_widgets::PointerEditOutcome;
use vesta_gpui_workspace::{SplitResizeError, SplitResizeSession};
use vesta_workspace::{ContainerRef, SplitAxis, SplitId};

#[test]
fn rejected_split_setup_retains_typed_failure_and_valid_session_ownership() {
    fn host_resize(ratio: f32) -> Result<SplitResizeSession, Box<dyn Error + Send + Sync>> {
        Ok(SplitResizeSession::new(
            InputTarget {
                window: UiWindowId(7),
                element: ElementId::new(11),
            },
            ContainerRef::Main,
            SplitId(3),
            SplitAxis::Row,
            Rect {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 600.0,
            },
            ratio,
        )?)
    }
    let failure = host_resize(f32::NAN).expect_err("invalid ratio rejected");
    assert_eq!(
        failure.downcast_ref::<SplitResizeError>(),
        Some(&SplitResizeError::InvalidRatio)
    );
    assert!(failure.source().is_none());
    assert!(!failure.to_string().is_empty());
    let mut resize = host_resize(0.52).expect("valid setup after rejection");
    assert_eq!(resize.container(), ContainerRef::Main);
    assert_eq!(resize.split(), SplitId(3));
    assert!(resize.finish(UiWindowId(8), 0.91, false).is_none());
    assert_eq!(
        resize.finish(UiWindowId(7), 0.91, false),
        Some(PointerEditOutcome::Cancel { restore: 0.52 })
    );
    assert!(resize.finish(UiWindowId(7), 0.91, true).is_none());
}
