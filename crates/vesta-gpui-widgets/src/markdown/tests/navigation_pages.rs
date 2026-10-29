use super::*;
use vesta_gpui::{
    FontRole, InputOwnership, KeyChord, KeyModifiers, KeyStroke, LayoutEngine, TextInputContext,
    Viewport,
};

pub(super) fn qualify() {
    let window = UiWindowId(33);
    let scope = ElementIdAllocator
        .allocate_scope()
        .expect("page navigation scope");
    let mut document =
        MarkdownView::parse("```\nabcdefghij\nab\nabcdefghij\n```\n\nNext page context")
            .expect("page document");
    let mut context = TextInputContext::default();
    assert!(context.register_font(std::sync::Arc::new(
        include_bytes!("../../../../vesta-gpui/tests/fixtures/MaterialIcons-Regular.ttf").to_vec()
    )));
    for role in [FontRole::Ui, FontRole::Mono] {
        context.set_font_stack(role, "Material Icons");
    }
    let project = |document: &mut MarkdownView,
                   range: std::ops::Range<usize>,
                   context: &mut TextInputContext| {
        for _ in 0..5 {
            let layout = LayoutEngine
                .compute(
                    &document.view(scope, range.clone(), WidgetTheme::dark()),
                    Viewport::new(500.0, 300.0, 1.0),
                )
                .expect("page layout");
            if !document
                .sync_layout(scope, &layout, context, WidgetTheme::dark())
                .expect("page shape")
            {
                return layout;
            }
        }
        panic!("page did not settle")
    };
    let first = project(&mut document, 0..1, &mut context);
    let mut interaction = MarkdownInteraction::new(window, scope);
    let mut input = InputOwnership::default();
    input.set_focus(Some(interaction.target()));
    let surface = MarkdownSurface {
        window,
        document: &document,
        layout: &first,
        range: 0..1,
    };
    let plain = KeyModifiers::default();
    for _ in 0..8 {
        interaction
            .key(
                &surface,
                KeyChord::new(KeyStroke::ArrowRight, plain),
                4096,
                &mut input,
            )
            .expect("preferred column eight");
    }
    interaction
        .key(
            &surface,
            KeyChord::new(KeyStroke::ArrowDown, plain),
            4096,
            &mut input,
        )
        .expect("short line");
    let accepted = interaction.selection().expect("short-line selection");
    assert_eq!(accepted.focus.byte(), 13);
    let next = project(&mut document, 0..2, &mut context);
    let surface = MarkdownSurface {
        window,
        document: &document,
        layout: &next,
        range: 0..2,
    };
    interaction.sync(&surface, &mut input);
    assert_eq!(interaction.selection(), Some(accepted));
    interaction
        .key(
            &surface,
            KeyChord::new(KeyStroke::ArrowDown, plain),
            4096,
            &mut input,
        )
        .expect("long line after page reproject");
    assert_eq!(
        interaction
            .selection()
            .expect("original column")
            .focus
            .byte(),
        22,
        "a completed same-source navigation keeps its preferred column across page projection"
    );
    assert!(input.capture().is_none());
}
