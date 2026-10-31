use std::{error::Error, io};
use vesta_gpui::{ElementId, InputTarget, Point, TextInputError, UiWindowId};
use vesta_gpui_winit::{
    TextBridgeRegistrationError, TextInputBridgeError, TextPointerAction, WindowTextInputBridges,
};

#[test]
fn native_errors_preserve_sources_and_registered_window_state() {
    let edit: TextInputBridgeError<io::Error> =
        TextInputBridgeError::Edit(TextInputError::ReadOnly);
    assert_eq!(
        edit.source()
            .and_then(|e| e.downcast_ref::<TextInputError>()),
        Some(&TextInputError::ReadOnly)
    );
    let clipboard =
        TextInputBridgeError::Clipboard(io::Error::from(io::ErrorKind::PermissionDenied));
    assert_eq!(
        clipboard
            .source()
            .and_then(|e| e.downcast_ref::<io::Error>())
            .map(io::Error::kind),
        Some(io::ErrorKind::PermissionDenied)
    );

    fn host_register(
        bridges: &mut WindowTextInputBridges,
        window: UiWindowId,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        bridges.register(window)?;
        Ok(())
    }
    let mut bridges = WindowTextInputBridges::default();
    let window = UiWindowId(73);
    let target = InputTarget {
        window,
        element: ElementId::new(11),
    };
    let point = Point { x: 10.0, y: 10.0 };
    let at = std::time::Instant::now();
    host_register(&mut bridges, window).expect("first registration");
    assert_eq!(
        bridges
            .get_mut(window)
            .expect("registered bridge")
            .text_pointer_press(target, point, at, false),
        Some(TextPointerAction::Caret)
    );
    let failure = host_register(&mut bridges, window).expect_err("identity cannot be replaced");
    assert_eq!(
        failure.downcast_ref::<TextBridgeRegistrationError>(),
        Some(&TextBridgeRegistrationError::DuplicateWindow(window))
    );
    assert!(failure.source().is_none());
    assert!(!failure.to_string().is_empty());
    assert_eq!(
        bridges
            .get_mut(window)
            .expect("original bridge")
            .text_pointer_press(
                target,
                point,
                at + std::time::Duration::from_millis(100),
                false
            ),
        Some(TextPointerAction::Word),
        "failed registration must preserve the original bridge's sequence"
    );

    #[cfg(not(target_arch = "wasm32"))]
    {
        use vesta_gpui_winit::{DocumentLinkError, WebLinkOpenError};
        let destination = WebLinkOpenError::Destination(DocumentLinkError::UnsupportedScheme);
        let launch = WebLinkOpenError::Launch(io::Error::from(io::ErrorKind::PermissionDenied));
        assert_eq!(
            (
                destination
                    .source()
                    .and_then(|e| e.downcast_ref::<DocumentLinkError>())
                    .copied(),
                launch
                    .source()
                    .and_then(|e| e.downcast_ref::<io::Error>())
                    .map(io::Error::kind),
            ),
            (
                Some(DocumentLinkError::UnsupportedScheme),
                Some(io::ErrorKind::PermissionDenied)
            ),
            "native link wrappers must retain their typed sources"
        );
    }
}
