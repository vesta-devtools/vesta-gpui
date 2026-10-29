//! Text display and mixed-script Gallery samples.
use super::*;

pub(super) fn text_samples(
    input: &UiBuildState<'_>,
    ids: &mut Ids,
    actions: &mut Vec<(ElementId, DemoAction)>,
    area: Element,
) -> Element {
    let theme = input.theme;
    let demo = input.demo;
    let static_sample = text_label(
        ids.identity("text.literal.sample"),
        TextContent::Literal("Vesta 地形 مرحبا 😀"),
        theme,
    );
    let locale_button = widget_element(
        button(
            ids.identity("text.locale.toggle"),
            ids.identity("text.locale.toggle.label"),
            if demo.locale == UiLocale::EnUs {
                "English · switch to 中文"
            } else {
                "中文 · switch to English"
            },
            DemoAction::ToggleLocale,
            ControlState::Idle,
            theme,
        ),
        actions,
    );
    let mut localized_sample = text_label(
        ids.identity("text.localized.sample"),
        TextContent::Localized {
            key: "gallery.localizedSample",
            value: if demo.locale == UiLocale::EnUs {
                "Create a world from a brief, then refine its terrain and details."
            } else {
                "从创作意图生成世界，再细化地形与细节。"
            },
        },
        theme,
    );
    localized_sample.style.width = Dimension::Px(280.0);
    localized_sample.style.max_width = Dimension::Percent(1.0);
    let mut wrapped_sample = label(
        ids,
        "Parley wraps mixed text within this narrow label: Vesta 地形 مرحبا 😀",
        theme.palette.text_muted,
        theme,
    );
    wrapped_sample.style.width = Dimension::Px(280.0);
    wrapped_sample.style.max_width = Dimension::Percent(1.0);
    wrapped_sample.style.height = Dimension::Auto;
    wrapped_sample.style.text_wrap = true;
    let rich = widget_element(
        rich_text(
            ids.identity("rich-text.root"),
            "Gallery navigation link",
            vec![
                RichTextRun {
                    identity: ids.identity("rich-text.intro"),
                    text: "Explore ".into(),
                    tone: RichTextTone::Normal,
                    action: None,
                    enabled: true,
                },
                RichTextRun {
                    identity: ids.identity("rich-text.link"),
                    text: "3D Viewport".into(),
                    tone: RichTextTone::Emphasis,
                    action: Some(DemoAction::SelectMode(AcceptanceMode::Viewport3d)),
                    enabled: demo.viewport3d_available,
                },
            ],
            theme,
        ),
        actions,
    );
    let mut multiline = section(
        ids,
        "Multiline text / wrap / IME",
        vec![
            area,
            static_sample,
            locale_button,
            localized_sample,
            wrapped_sample,
            rich,
        ],
        theme,
    );
    multiline.style.height = Dimension::Px(330.0);
    multiline
}
