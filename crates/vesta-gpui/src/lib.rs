//! Backend-neutral declarative UI contracts for native tool hosts.
//!
//! The crate owns stable element identity and deterministic derived layout, paint, and hit-test
//! data. It deliberately has no dependency on Bevy, wgpu, winit, Graph semantics, or host assets.

#![forbid(unsafe_code)]

/// Version of the compiled GPUI package; this alone does not identify an immutable SDK release.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

mod accessibility;
mod control;
pub use control::{UiCommand, UiOwnerRegistry, UiTargetRef};
mod action_route;
mod element;
mod focus;
mod geometry;
mod image;
mod input_ownership;
mod inspection;
mod interaction_lifecycle;
mod keyboard;
mod layout;
mod menu;
mod paint;
pub mod render;
mod style;
mod text_input;
mod text_raster;
mod text_session;
mod text_views;

pub use accessibility::{
    AccessibilityTree, ReadOnlyDocument, ReadOnlyDocumentSelection, ReadOnlyText,
    ReadOnlyTextPosition, ReadOnlyTextProjectionError, Semantic, SemanticNumeric, SemanticRole,
    SemanticToggle,
};
pub use action_route::{ActionRoute, ActionRouteError, EventPhase, PointerClick};
pub use element::{
    Element, ElementId, ElementIdAllocator, ElementIdentity, ElementKey, ElementKind, ElementScope,
    ElementScopeExhausted, TextSource,
};
pub use focus::{FocusScope, InputReconciliation};
pub use geometry::{Point, Rect, Size, Viewport};
pub use image::{ImageAlign, ImageFit, ImagePlacement, ImagePresentation};
pub use input_ownership::{
    FocusInput, InputOwnership, InputTarget, PointerButton, PointerCapture, UiWindowId,
};
pub use inspection::{
    UiActionAvailability, UiFieldContent, UiNode, UiQuery, UiQueryError, UiQueryLimits,
    UiQueryPage, UiSnapshot, UiSnapshotChanges, UiSnapshotFrame, UiSnapshotId, UiSnapshotText,
};
pub use interaction_lifecycle::{InteractionCleanupStage, finish_window_interaction};
pub use keyboard::{KeyChord, KeyModifiers, KeyStroke, Keymap, KeymapError};
pub use layout::{
    ContentConstraint, DesiredSize, LayoutEngine, LayoutError, LayoutNode, LayoutTree,
    RetainedLayoutEngine, TextLayoutMetrics, TextLayoutRequest, text_intrinsic_size,
};
pub use menu::{Menu, MenuId, MenuItem, MenuItemId, MenuRegistry, MenuRegistryError};
pub use paint::{
    HitTestTree, PaintCommand, PaintInteraction, PaintList, SCROLLBAR_GUTTER, SCROLLBAR_INSET,
    SCROLLBAR_SIZE, scrollbar_geometry, scrollbar_geometry_in_direction,
};
pub use style::{
    Align, Axis, ClipMode, Color, Dimension, FlexWrap, FlowDirection, FocusIndicator, FontRole,
    GridCell, GridTrack, HitShape, Insets, Justify, PositionMode, ScrollbarGutter, ScrollbarStyle,
    ScrollbarVisibility, Style, TextHAlign, TextVAlign, VisualStyle,
};
pub use text_input::{
    EllipsisError, EllipsizedText, StyledTextAlignment, StyledTextError, StyledTextOptions,
    StyledTextSpan, TEXT_INPUT_MAX_BYTES, TextContentWidths, TextDecorationRects, TextEditKind,
    TextInputAction, TextInputContext, TextInputError, TextInputMark, TextInputSelection,
    TextInputSnapshot, TextInputState, TextInputViewport, TextLayoutCursor,
    TextMeasurementRevision, text_input_wrap_width,
};
pub use text_raster::{
    GlyphKey, RasterGlyph, TextInputBitmap, TextInputColors, TextRasterError, TextRasterizer,
};
pub use text_session::{TextSessionTransition, WindowTextSession};
pub use text_views::{TextInputViewError, TextInputViews};
