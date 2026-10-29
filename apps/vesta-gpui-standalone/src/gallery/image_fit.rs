//! Real public image-presentation consumer, shared by Main and native floats.
use super::*;
use vesta_gpui::{ImageAlign, ImageFit, ImagePresentation, Semantic, SemanticRole};

pub(super) fn samples(scope: ElementScope, theme: WidgetTheme) -> Element {
    let mut root = Element::container(
        scope.element(27_100_000),
        ElementKey::new("gallery.image-fit"),
    )
    .styled(Style::default().axis(Axis::Column).gap(8.0));
    root = root.child(
        Element::text(
            scope.element(27_100_001),
            ElementKey::new("gallery.image-fit.heading"),
            "Image fitting · Stretch / Contain / Cover",
        )
        .styled(
            Style::default()
                .text(theme.typography.font_size, theme.palette.text)
                .text_wrap(true),
        ),
    );
    let mut row_index = 0;
    for (tall, aspect) in [(false, "wide"), (true, "tall")] {
        for (align, anchor) in [
            (ImageAlign::Min, "min"),
            (ImageAlign::Center, "center"),
            (ImageAlign::Max, "max"),
        ] {
            let mut row = Element::container(
                scope.element(27_100_010 + row_index),
                ElementKey::new(format!("gallery.image-fit.row.{aspect}.{anchor}")),
            )
            .styled(
                Style::default()
                    .axis(Axis::Row)
                    .flex_wrap(FlexWrap::Wrap)
                    .gap(8.0),
            );
            for (column, fit, name) in [
                (0, ImageFit::Stretch, "stretch"),
                (1, ImageFit::Contain, "contain"),
                (2, ImageFit::Cover, "cover"),
            ] {
                let base = 27_100_100 + row_index * 30 + column * 5;
                let key = format!("gallery.image-fit.{aspect}.{anchor}.{name}");
                let label = format!("{aspect} · {name} · {anchor}");
                row = row.child(
                    Element::container(scope.element(base), ElementKey::new(format!("{key}.cell")))
                        .styled(Style::default().axis(Axis::Column).gap(4.0).flex(0.0, 0.0))
                        .child(
                            Element::text(
                                scope.element(base + 1),
                                ElementKey::new(format!("{key}.caption")),
                                &label,
                            )
                            .styled(
                                Style::default()
                                    .size(Dimension::Px(120.0), Dimension::Auto)
                                    .text(12.0, theme.palette.text_muted)
                                    .text_wrap(true),
                            ),
                        )
                        .child(
                            Element::fitted_image(
                                scope.element(base + 2),
                                ElementKey::new(key),
                                if tall {
                                    "demo://image-tall"
                                } else {
                                    "demo://image-wide"
                                },
                                ImagePresentation::new(fit).aligned(align, align),
                            )
                            .styled(
                                Style::default()
                                    .size(Dimension::Px(120.0), Dimension::Px(80.0))
                                    .flex(0.0, 0.0)
                                    .background(theme.palette.surface),
                            )
                            .semantic(Semantic::new(SemanticRole::Label, label)),
                        ),
                );
            }
            root = root.child(row);
            row_index += 1;
        }
    }
    let mut extra = Element::container(
        scope.element(27_100_400),
        ElementKey::new("gallery.image-fit.extra"),
    )
    .styled(
        Style::default()
            .axis(Axis::Row)
            .gap(16.0)
            .flex_wrap(FlexWrap::Wrap),
    );
    for (legacy, name, frame_width, frame_height) in [
        (true, "wide · legacy · center", 120.0, 80.0),
        (false, "wide · cover · clipped", 84.0, 52.0),
    ] {
        let id = if legacy { 27_100_410 } else { 27_100_420 };
        let mut image = if legacy {
            Element::image(
                scope.element(id + 2),
                ElementKey::new(format!("gallery.image-fit.{id}.image")),
                "demo://image-wide",
            )
        } else {
            Element::fitted_image(
                scope.element(id + 2),
                ElementKey::new(format!("gallery.image-fit.{id}.image")),
                "demo://image-wide",
                ImagePresentation::new(ImageFit::Cover).aligned(ImageAlign::Max, ImageAlign::Min),
            )
        }
        .styled(
            Style::default()
                .size(Dimension::Px(120.0), Dimension::Px(80.0))
                .background(theme.palette.surface),
        )
        .semantic(Semantic::new(SemanticRole::Label, name));
        if !legacy {
            image.style = image.style.absolute(-16.0, -12.0);
        }
        extra = extra.child(
            Element::container(
                scope.element(id),
                ElementKey::new(format!("gallery.image-fit.{id}.cell")),
            )
            .styled(Style::default().axis(Axis::Column).gap(4.0).flex(0.0, 0.0))
            .child(
                Element::text(
                    scope.element(id + 1),
                    ElementKey::new(format!("gallery.image-fit.{id}.caption")),
                    if legacy {
                        "Legacy / Stretch"
                    } else {
                        "Ancestor clip (84×52)"
                    },
                )
                .styled(Style::default().text(12.0, theme.palette.text_muted)),
            )
            .child(
                Element::container(
                    scope.element(id + 3),
                    ElementKey::new(format!("gallery.image-fit.{id}.frame")),
                )
                .styled(
                    Style::default()
                        .size(Dimension::Px(frame_width), Dimension::Px(frame_height))
                        .flex(0.0, 0.0)
                        .background(theme.palette.surface),
                )
                .child(image),
            ),
        );
    }
    root.child(extra)
}
