//! Native acceptance argument validation; ordinary product arguments stay in the Host.
use super::*;

pub(super) fn validate_args() -> Result<(), String> {
    let options: Vec<_> = std::env::args()
        .filter(|arg| arg.starts_with("--native-capture-fixture"))
        .collect();
    if options.is_empty() {
        return Ok(());
    }
    if options.len() != 1
        || !matches!(
            options[0].strip_prefix(PREFIX),
            Some(
                "main"
                    | "float"
                    | "main-remove"
                    | "float-remove"
                    | "main-reset"
                    | "float-reset"
                    | "main-pane-remove"
                    | "float-restore"
                    | "main-blur-dock"
                    | "float-blur-dock"
                    | "float-minimize-dock"
                    | "main-minimize-dock"
                    | "main-minimize-return-dock"
                    | "main-hide-dock"
                    | "main-hide-return-dock"
                    | "main-blur-splitter"
                    | "float-blur-splitter"
                    | "main-resize-splitter"
                    | "float-hide-dock"
                    | "main-blur-document"
                    | "float-blur-document"
                    | "float-restore-document"
                    | "main-modal-dock-target"
                    | "main-palette-dock-target"
                    | "main-minimize-dock-target"
                    | "main-move-dock-target"
                    | "main-key-replay"
                    | "float-key-replay"
                    | "main-disable"
                    | "float-disable"
                    | "main-remove-slider"
                    | "float-remove-slider"
                    | "main-reset-slider"
                    | "float-reset-slider"
                    | "main-hide-slider"
                    | "float-hide-slider"
                    | "main-disable-slider"
                    | "float-disable-slider"
                    | "main-focus-slider"
                    | "float-focus-slider"
                    | "main-modal-slider"
                    | "float-modal-slider"
                    | "main-hide-number"
                    | "float-hide-number"
                    | "main-hide-vector"
                    | "float-hide-vector"
                    | "main-modal-number"
                    | "float-blur-vector"
                    | "main-hide-scrollbar"
                    | "float-hide-scrollbar"
                    | "main-modal-scrollbar"
                    | "float-modal-scrollbar"
                    | "main-blur-scrollbar"
                    | "float-blur-scrollbar"
                    | "float-resize-scrollbar"
                    | "main-narrow"
                    | "observe"
                    | "observe-configured"
                    | selected_feedback::CASE
                    | dock_visual::CASE
                    | "observe-float"
                    | "observe-narrow"
                    | "observe-nowrap"
                    | "observe-area"
            )
        )
        || std::env::args().any(|arg| {
            arg == "--smoke" || arg.starts_with("--smoke-") || arg.starts_with("--mode=")
        })
    {
        return Err(
                "native_capture_args: choose main|float with optional -remove|-reset|-key-replay|-disable|-remove-slider|-reset-slider|-hide-slider|-disable-slider|-focus-slider|-modal-slider|-hide-number|-hide-vector|-hide-scrollbar|-modal-scrollbar|-blur-scrollbar|-blur-document, main-modal-number, float-blur-vector, main-pane-remove, float-restore, float-restore-document, main-modal-dock-target, main-palette-dock-target, main-minimize-dock-target, main-move-dock-target, main-narrow, observe, observe-float, observe-narrow, observe-nowrap, observe-area, observe-selected-feedback or observe-dock-visual in default 2d mode"
                .into(),
        );
    }
    Ok(())
}

pub(super) fn source(case: &str) -> Option<Source> {
    Some(match case {
        "main"
        | "main-remove"
        | "main-reset"
        | "main-pane-remove"
        | "main-blur-dock"
        | "main-minimize-dock"
        | "main-minimize-return-dock"
        | "main-hide-dock"
        | "main-hide-return-dock"
        | "main-blur-splitter"
        | "main-resize-splitter"
        | "main-key-replay"
        | "main-disable"
        | "main-remove-slider"
        | "main-reset-slider"
        | "main-narrow"
        | "main-hide-slider"
        | "main-disable-slider"
        | "main-focus-slider"
        | "main-modal-slider"
        | "main-hide-number"
        | "main-hide-vector"
        | "main-modal-number"
        | "main-hide-scrollbar"
        | "main-modal-scrollbar"
        | "main-blur-scrollbar"
        | "main-blur-document"
        | "main-modal-dock-target"
        | "main-palette-dock-target"
        | "main-minimize-dock-target"
        | "main-move-dock-target"
        | "observe"
        | "observe-configured"
        | selected_feedback::CASE
        | dock_visual::CASE
        | "observe-narrow"
        | "observe-nowrap"
        | "observe-area" => Source::Main,
        "float"
        | "float-remove"
        | "float-reset"
        | "float-restore"
        | "float-blur-dock"
        | "float-blur-splitter"
        | "float-minimize-dock"
        | "float-hide-dock"
        | "float-key-replay"
        | "float-disable"
        | "float-remove-slider"
        | "float-reset-slider"
        | "float-hide-slider"
        | "float-disable-slider"
        | "float-focus-slider"
        | "float-modal-slider"
        | "float-hide-number"
        | "float-hide-vector"
        | "float-blur-vector"
        | "float-hide-scrollbar"
        | "float-modal-scrollbar"
        | "float-blur-scrollbar"
        | "float-resize-scrollbar" => Source::Float,
        "float-blur-document" | "float-restore-document" => Source::Float,
        "observe-float" => Source::Float,
        _ => return None,
    })
}
