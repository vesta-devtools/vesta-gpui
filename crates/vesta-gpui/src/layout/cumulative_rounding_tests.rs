//! Final local positions must agree with cumulatively rounded grid edges.
use super::*;
use crate::{HitTestTree, Style, TextInputContext};

pub(super) fn qualify() {
    let pair = Element::container(ElementId::new(90), ElementKey::new("half.root"))
        .styled(Style::default().size(Dimension::Px(10.0), Dimension::Px(10.0)))
        .child(
            Element::container(ElementId::new(91), ElementKey::new("half.a")).styled(
                Style::default()
                    .absolute(-0.5, 0.0)
                    .size(Dimension::Px(1.0), Dimension::Px(1.0)),
            ),
        )
        .child(
            Element::container(ElementId::new(92), ElementKey::new("half.b")).styled(
                Style::default()
                    .absolute(0.5, 0.0)
                    .size(Dimension::Px(1.0), Dimension::Px(1.0)),
            ),
        );
    let pair_layout = LayoutEngine
        .compute(&pair, Viewport::new(10.0, 10.0, 1.0))
        .expect("half-edge layout");
    assert_eq!(
        HitTestTree::build(&pair, &pair_layout).hit(Point::new(0.5, 0.5)),
        Some(ElementId::new(91)),
        "adjacent negative-half boxes must not leave a hit gap"
    );
    let id = ElementId::new;
    let mut failures = Vec::new();
    for flow in [FlowDirection::LeftToRight, FlowDirection::RightToLeft] {
        let mut grid = Element::container(id(3), ElementKey::new("grid")).styled(
            Style::default()
                .size(Dimension::Px(340.4), Dimension::Px(38.0))
                .grid(vec![GridTrack::Fr(1.0); 3], vec![GridTrack::Px(38.0)])
                .gap(4.0)
                .clipped(),
        );
        grid.style.left = Some(0.3);
        for column in 0..3 {
            grid = grid.child(
                Element::container(id(4 + column), ElementKey::new(format!("cell.{column}")))
                    .styled(
                        Style::default()
                            .size(Dimension::Percent(1.0), Dimension::Percent(1.0))
                            .grid_cell(
                                GridCell::new(Some(column as u16 + 1), 1),
                                GridCell::new(Some(1), 1),
                            ),
                    ),
            );
        }
        let mut parent = Element::container(id(2), ElementKey::new("parent"))
            .styled(Style::default().size(Dimension::Px(380.0), Dimension::Px(80.0)))
            .child(grid);
        parent.style.left = Some(0.3);
        let mut root = Element::container(id(1), ElementKey::new("root"))
            .styled(Style::default().size(Dimension::Px(400.0), Dimension::Px(100.0)))
            .child(parent);
        root.style.flow_direction = Some(flow);
        let viewport = Viewport::new(400.0, 100.0, 1.0);
        let mut retained = RetainedLayoutEngine::default();
        let mut context = TextInputContext::default();
        for frame in 0..2 {
            let cold = LayoutEngine
                .compute(&root, viewport)
                .expect("fractional grid");
            let warm = retained
                .compute_with_text_context(&root, viewport, &mut context)
                .expect("retained fractional grid");
            for cell in 3..7 {
                assert_eq!(
                    cold.node(id(cell)).expect("cold node").rect,
                    warm.node(id(cell)).expect("warm node").rect
                );
            }
            let parent = cold.node(id(3)).expect("grid rect").rect;
            let cell = cold.node(id(6)).expect("last cell");
            let point = Point::new(
                if flow == FlowDirection::LeftToRight {
                    cell.rect.right() - 0.25
                } else {
                    cell.rect.x + 0.25
                },
                cell.rect.y + cell.rect.height * 0.5,
            );
            let hit = HitTestTree::build(&root, &cold).hit(point);
            println!(
                "GPS03-CUMULATIVE flow={flow:?} frame={frame} parent_x={} parent_w={} cell_x={} cell_w={} hit={hit:?}",
                parent.x, parent.width, cell.rect.x, cell.rect.width
            );
            if cell.rect.x < parent.x - 0.001
                || cell.rect.right() > parent.right() + 0.001
                || hit != Some(id(6))
            {
                failures.push(format!("{flow:?}/{frame}"));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "cumulative grid edge mismatch: {failures:?}"
    );
}
