use vesta_gpui::{
    Color, Dimension, Element, ElementId, ElementKey, LayoutEngine, PaintCommand, PaintList, Point,
    Polyline, Style, Viewport, render,
};

#[test]
fn named_render_payload_projects_through_the_same_layout_and_paint() {
    let line = Polyline {
        points: vec![
            Point::new(4.0, 4.0),
            Point::new(20.0, 4.0),
            Point::new(20.0, 20.0),
        ],
        width: 3.0,
    };
    let root = Element::container(ElementId::new(1), ElementKey::new("payload")).styled(Style {
        polyline: Some(line.clone()),
        ..Style::default()
            .absolute(5.0, 7.0)
            .size(Dimension::Px(40.0), Dimension::Px(40.0))
            .background(Color::srgba(0.2, 0.3, 0.4, 0.5))
    });
    let viewport = Viewport::new(100.0, 100.0, 1.25);
    let layout = LayoutEngine.compute(&root, viewport).expect("layout");
    let paint = PaintList::build(&root, &layout);
    let (points, width, color) = paint
        .commands()
        .iter()
        .find_map(|command| match command {
            PaintCommand::FillPolyline {
                element,
                points,
                width,
                color,
            } if *element == root.id => Some((points, *width, *color)),
            _ => None,
        })
        .expect("same typed paint");
    let bounds = layout.node(root.id).expect("bounds").rect;
    assert_eq!(
        points,
        &line
            .points
            .iter()
            .map(|p| Point::new(p.x + bounds.x, p.y + bounds.y))
            .collect::<Vec<_>>()
    );
    assert_eq!(width, line.width);
    let vertices = render::polyline_vertices(points, width, color, viewport);
    assert!(!vertices.is_empty());
    assert!(
        vertices
            .iter()
            .all(|v| v.position.iter().all(|x| x.is_finite()))
    );
    assert_eq!(
        root.style.polyline.as_ref(),
        Some(&line),
        "CPU preparation cannot mutate the Host payload"
    );
}
