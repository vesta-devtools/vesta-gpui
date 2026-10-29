//! Readable shared drag captions without turning the overlay into an input owner.
use super::*;
use vesta_gpui::{
    ActionRoute, ElementIdAllocator, HitTestTree, LayoutEngine, PaintCommand, PaintList, Viewport,
};
use vesta_gpui_widgets::WidgetTheme;

pub(super) fn qualify() {
    let scope = ElementIdAllocator.allocate_scope().expect("overlay scope");
    let visual = DockDragVisual {
        target: None,
        cursor: Point::new(200.0, 100.0),
        title: "Dragged tab".into(),
    };
    let mut geometry = None;
    let mut original_paint = None;
    let mut previous_paint = None;
    let mut unreadable = Vec::new();
    for (index, theme) in [
        WidgetTheme::dark(),
        WidgetTheme::light(),
        WidgetTheme::dark(),
    ]
    .into_iter()
    .enumerate()
    {
        let overlay = dock_drag_overlay(scope, &visual, theme, Size::new(800.0, 600.0));
        let layout = LayoutEngine
            .compute(&overlay, Viewport::new(800.0, 600.0, 1.25))
            .expect("overlay layout");
        let current_geometry = layout
            .nodes()
            .iter()
            .map(|node| (node.id, node.rect))
            .collect::<Vec<_>>();
        if let Some(original) = &geometry {
            assert_eq!(
                &current_geometry, original,
                "theme must preserve the drag's current geometry and IDs"
            );
        } else {
            geometry = Some(current_geometry);
        }
        let paint = PaintList::build(&overlay, &layout);
        let fill = paint
            .commands()
            .iter()
            .find_map(|command| match command {
                PaintCommand::FillRoundedRect { element, color, .. }
                    if *element == scope.element(102) =>
                {
                    Some(*color)
                }
                _ => None,
            })
            .expect("actual ghost backing");
        let (caption, bounds) = paint
            .commands()
            .iter()
            .find_map(|command| match command {
                PaintCommand::Text {
                    element,
                    text,
                    color,
                    bounds,
                    ..
                } if *element == scope.element(102) => {
                    assert_eq!(text, "  Dragged tab");
                    Some((*color, *bounds))
                }
                _ => None,
            })
            .expect("actual ghost caption");
        // Bound the alpha-composited luminance with dark/light opaque contents,
        // rather than assuming this floating caption always covers the canvas.
        for backdrop in [
            theme.palette.canvas,
            Color::rgb8(0, 0, 0),
            Color::rgb8(255, 255, 255),
        ] {
            let fill_luma = luminance(fill) * f64::from(fill.alpha)
                + luminance(backdrop) * (1.0 - f64::from(fill.alpha));
            let caption_luma = luminance(caption);
            let ratio = (fill_luma.max(caption_luma) + 0.05) / (fill_luma.min(caption_luma) + 0.05);
            if ratio < 4.5 {
                unreadable.push((theme.mode, backdrop, ratio));
            }
        }
        assert!(
            HitTestTree::build(&overlay, &layout)
                .hit(Point::new(
                    bounds.x + bounds.width / 2.0,
                    bounds.y + bounds.height / 2.0
                ))
                .is_none(),
            "the ghost cannot steal a drop from the underlying pane"
        );
        let route = ActionRoute::build(&overlay, Vec::<(vesta_gpui::ElementId, u8)>::new())
            .expect("decorative overlay route");
        assert!(route.focus_order().is_empty());
        if index == 0 {
            original_paint = Some(paint.clone());
        } else if index == 1 {
            assert_ne!(Some(&paint), previous_paint.as_ref());
        } else {
            assert_eq!(
                Some(&paint),
                original_paint.as_ref(),
                "dark theme must restore completely"
            );
        }
        previous_paint = Some(paint);
    }
    assert!(
        unreadable.is_empty(),
        "unreadable Dock ghost captions: {unreadable:?}"
    );
}

fn luminance(color: Color) -> f64 {
    let linear = |channel: f32| {
        let channel = f64::from(channel);
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.red) + 0.7152 * linear(color.green) + 0.0722 * linear(color.blue)
}
