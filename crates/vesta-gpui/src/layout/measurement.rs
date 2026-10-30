//! Content sizing before arrangement, using the same Taffy and Parley inputs.

use super::*;
use crate::TextInputContext;

/// Optional available logical dimensions. `None` leaves that axis unconstrained.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ContentConstraint {
    pub width: Option<f32>,
    pub height: Option<f32>,
}

/// Intrinsic root sizes, distinct from an arranged rectangle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DesiredSize {
    pub preferred: Size,
    /// Minimum content width and its associated height. Wrapping can make this
    /// taller than `preferred`; these are not two independent lower bounds.
    pub min_content: Size,
}

impl LayoutEngine {
    /// Measures a subtree before arranging it. Fixed, minimum and maximum style
    /// dimensions remain authoritative; indefinite percentages follow Taffy.
    pub fn measure_with_text_context(
        self,
        root: &Element,
        constraint: ContentConstraint,
        context: &mut TextInputContext,
    ) -> Result<DesiredSize, LayoutError> {
        let _trace = crate::trace::detail_span("gpui.layout", "layout.measure");
        if [constraint.width, constraint.height]
            .into_iter()
            .flatten()
            .any(|v| !v.is_finite() || v <= 0.0)
        {
            return Err(LayoutError::InvalidConstraint);
        }
        validate_identity(root)?;
        let mut taffy = LayoutStorage::new();
        let mut nodes = HashMap::new();
        let root_node = build_taffy_tree(
            root,
            &mut taffy,
            &mut nodes,
            false,
            Axis::Column,
            false,
            &mut |request| shaped_metrics(context, request),
        )?;
        let height = constraint
            .height
            .map_or(AvailableSpace::MaxContent, AvailableSpace::Definite);
        let width = constraint
            .width
            .map_or(AvailableSpace::MaxContent, AvailableSpace::Definite);
        run_taffy_layout(
            &mut taffy,
            root_node,
            TaffySize { width, height },
            &mut |text, width, minimum, height| {
                measure_content(context, text, width, minimum, height)
            },
        )?;
        let size = taffy.layout(root_node).map_err(engine_error)?.size;
        let preferred = Size::new(size.width, size.height);
        run_taffy_layout(
            &mut taffy,
            root_node,
            TaffySize {
                width: AvailableSpace::MinContent,
                height,
            },
            &mut |text, width, minimum, height| {
                measure_content(context, text, width, minimum, height)
            },
        )?;
        let size = taffy.layout(root_node).map_err(engine_error)?.size;
        Ok(DesiredSize {
            preferred,
            min_content: Size::new(size.width, size.height),
        })
    }
}

fn measure_content(
    context: &mut TextInputContext,
    text: &TextMeasure,
    width: Option<f32>,
    minimum: bool,
    height: Option<f32>,
) -> TextLayoutMetrics {
    if !text.wrap && height.is_none() {
        return text.intrinsic;
    }
    let width = if minimum {
        context
            .shape_static_text(&text.text, text.font_size, text.role)
            .ok()
            .map(|snapshot| snapshot.content_widths().minimum.max(f32::EPSILON))
    } else {
        width
    };
    shaped_metrics(context, text.request(width, height))
}

#[cfg(test)]
fn shaped_size(
    context: &mut TextInputContext,
    text: &str,
    size: f32,
    role: FontRole,
    width: Option<f32>,
) -> Size {
    context
        .shape_static_text_wrapped(text, size, role, width)
        .map(|snapshot| Size::new(snapshot.layout_width(), snapshot.layout_height()))
        .unwrap_or_else(|_| text_intrinsic_size(text, size, role))
}

fn engine_error(error: impl fmt::Display) -> LayoutError {
    LayoutError::LayoutEngine(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FontRole, Style};
    use std::sync::Arc;

    #[test]
    fn content_measurement_preserves_constraints_wrapping_and_arrangement_contract() {
        let mut context = TextInputContext::default();
        let font = include_bytes!("../../tests/fixtures/MaterialIcons-Regular.ttf");
        assert!(context.register_font(Arc::new(font.to_vec())));
        context.set_font_stack(FontRole::Ui, "Material Icons");
        let mut root = Element::container(ElementId::new(1), ElementKey::new("root"))
            .styled(Style::default().padding(Insets::all(7.0)))
            .child(
                Element::text(
                    ElementId::new(2),
                    ElementKey::new("text"),
                    "Content wraps at real break opportunities\n中文",
                )
                .styled(Style::default().text_wrap(true)),
            );
        let natural = LayoutEngine
            .measure_with_text_context(&root, ContentConstraint::default(), &mut context)
            .expect("natural");
        assert!(natural.min_content.width < natural.preferred.width);
        assert!(natural.min_content.height >= natural.preferred.height);
        let width = (natural.preferred.width + natural.min_content.width) * 0.5;
        root.style = root.style.max_width(Dimension::Px(width));
        let constrained = LayoutEngine
            .measure_with_text_context(
                &root,
                ContentConstraint {
                    width: Some(width),
                    height: None,
                },
                &mut context,
            )
            .expect("constrained");
        assert!(constrained.preferred.width <= width.ceil());
        assert!(constrained.preferred.height >= natural.preferred.height);
        let arranged = LayoutEngine
            .compute_with_text_layout(
                &root,
                Viewport::new(width, constrained.preferred.height, 1.0),
                |text, size, role, width| shaped_size(&mut context, text, size, role, width),
            )
            .expect("arranged");
        let rect = arranged.node(root.id).expect("root").rect;
        assert_eq!(Size::new(rect.width, rect.height), constrained.preferred);
        root.style = root
            .style
            .max_width(Dimension::Auto)
            .size(Dimension::Px(120.0), Dimension::Px(90.0));
        let fixed = LayoutEngine
            .measure_with_text_context(&root, ContentConstraint::default(), &mut context)
            .expect("fixed");
        assert_eq!(fixed.preferred, Size::new(120.0, 90.0));
        assert_eq!(fixed.min_content, fixed.preferred);
        assert!(matches!(
            LayoutEngine.measure_with_text_context(
                &root,
                ContentConstraint {
                    width: Some(f32::NAN),
                    height: None
                },
                &mut context
            ),
            Err(LayoutError::InvalidConstraint)
        ));
        root.style = Style::default()
            .size(Dimension::Px(120.0), Dimension::Auto)
            .max_width(Dimension::Px(90.0))
            .aspect_ratio(2.0);
        let ratio = LayoutEngine
            .measure_with_text_context(&root, ContentConstraint::default(), &mut context)
            .expect("ratio");
        assert_eq!(ratio.preferred, Size::new(90.0, 45.0));
        let arranged = LayoutEngine
            .compute(&root, Viewport::new(100.0, 100.0, 1.0))
            .expect("ratio arrange");
        let rect = arranged.node(root.id).expect("root").rect;
        assert_eq!(Size::new(rect.width, rect.height), ratio.preferred);
        root.style.aspect_ratio = Some(0.0);
        assert!(
            matches!(LayoutEngine.measure_with_text_context(&root, ContentConstraint::default(), &mut context),
            Err(LayoutError::InvalidAspectRatio(id)) if id == root.id)
        );
        root.style.aspect_ratio = None;
        root.children.push(root.children[0].clone());
        assert!(matches!(
            LayoutEngine.measure_with_text_context(
                &root,
                ContentConstraint::default(),
                &mut context
            ),
            Err(LayoutError::DuplicateElementId(_))
        ));
    }
}
