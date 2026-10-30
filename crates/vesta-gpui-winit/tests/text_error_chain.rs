use std::{error::Error, io};
use vesta_gpui::TextInputError;
use vesta_gpui_winit::TextInputBridgeError;

#[test]
fn native_edit_and_clipboard_failures_retain_their_authoritative_source() {
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
}
