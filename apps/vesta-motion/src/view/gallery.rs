//! Decorative thumbnails inside ordinary public GPUI buttons; not rendered outputs.
use super::*;
use std::f32::consts::{PI, TAU};

pub fn cards(model: &Model, height: f32, columns: u32) -> Widget<Action> {
    let rows = 6 / columns;
    let width = (LEFT - 20.0 - 6.0 * (columns - 1) as f32) / columns as f32;
    let mut grid = container(
        model,
        200,
        LEFT - 20.0,
        height * rows as f32 + 4.0 * (rows - 1) as f32,
        Axis::Column,
        None,
    );
    grid.element.style.gap = 4.0;
    for row in 0..rows {
        let mut pair = container(model, 210 + row, LEFT - 20.0, height, Axis::Row, None);
        pair.element.style.gap = 6.0;
        for study in Study::ALL
            .into_iter()
            .skip((row * columns) as usize)
            .take(columns as usize)
        {
            pair = pair.child(card(model, study, width, height));
        }
        grid = grid.child(pair);
    }
    grid
}
fn card(m: &Model, study: Study, width: f32, height: f32) -> Widget<Action> {
    let base = study.slot();
    let label = if width < 85.0 {
        match study {
            Study::Orbit => "Orbit",
            Study::Wave => "Waves",
            Study::Sphere => "Sphere",
            Study::Helix => "Helix",
            Study::Bloom => "Bloom",
            Study::Spiral => "Spiral",
        }
    } else {
        study.title()
    };
    let mut card = button(
        m,
        base,
        label,
        Action::Study(study),
        width,
        m.study == study,
    );
    card.element.style = card
        .element
        .style
        .size(D::Px(width), D::Px(height))
        .padding(Insets::all(2.0));
    card.element.tooltip = Some(format!(
        "{} · {} · key {}",
        study.title(),
        study.category(),
        study.index() + 1
    ));
    if let Some(label) = card.element.children.first_mut() {
        label.style = label
            .style
            .clone()
            .absolute(5.0, height - 20.0)
            .size(D::Px(width - 10.0), D::Px(18.0))
            .text(9.5, m.theme.palette.text)
            .text_align(TextHAlign::Start, TextVAlign::Center);
    }
    let side = (width - 4.0).min(height - 22.0);
    let w = side;
    let h = side;
    let (drawing_width, drawing_height): (f32, f32) = match study {
        Study::Orbit => (40.0, 40.0),
        Study::Wave => (72.0, 36.0),
        Study::Sphere => (60.0, 60.0),
        Study::Helix => (40.0, 40.0),
        Study::Bloom => (44.0, 44.0),
        Study::Spiral => (56.0, 52.0),
    };
    // Fit the drawing with one scale so narrow or short cards cannot distort it.
    let scale = ((w - 8.0) / drawing_width).min((h - 8.0) / drawing_height);
    let color = match study.defaults().palette {
        0 => GOLD,
        1 => MINT,
        _ => Color::rgb8(190, 174, 223),
    };
    let mut preview = container(m, base + 2, w, h, Axis::Column, Some(DARK));
    preview.element.style = preview
        .element
        .style
        .absolute((width - side) * 0.5, 2.0)
        .radius(2.0)
        .pointer_events(false);
    for band in 0..12 {
        let points = (0..=40)
            .map(|step| {
                let t = step as f32 / 40.0;
                let a = t * TAU;
                let b = band as f32 / 12.0;
                let (x, y) = match study {
                    Study::Orbit => {
                        let r = 12.0 + 4.0 * (a * 2.0 + b * TAU).sin();
                        (r * a.cos(), r * a.sin())
                    }
                    Study::Wave => (
                        (t - 0.5) * 64.0,
                        (t * TAU * 1.2 + b * 3.0).sin() * 5.0 + (b - 0.5) * 19.0,
                    ),
                    Study::Sphere => (a.cos() * 26.0 * (b * PI).cos(), a.sin() * 26.0),
                    Study::Helix => {
                        if band < 2 {
                            (
                                (t * TAU * 2.0 + band as f32 * PI).sin() * 16.0,
                                (t - 0.5) * 32.0,
                            )
                        } else {
                            let y = (b - 0.5) * 28.0;
                            let x = (b * TAU * 2.0).sin() * 16.0;
                            ((t * 2.0 - 1.0) * x, y)
                        }
                    }
                    Study::Bloom => {
                        let r = 2.0 + (t * PI).sin() * 16.0;
                        let angle = b * TAU + (t - 0.5) * 0.65;
                        (r * angle.cos(), r * angle.sin())
                    }
                    Study::Spiral => {
                        let angle = b * TAU * 2.0;
                        let corners = [
                            (-2.0, -2.0),
                            (2.0, -2.0),
                            (2.0, 2.0),
                            (-2.0, 2.0),
                            (-2.0, -2.0),
                        ];
                        let segment = (t * 4.0).floor().min(3.0) as usize;
                        let f = t * 4.0 - segment as f32;
                        let p = corners[segment];
                        let q = corners[segment + 1];
                        (
                            angle.cos() * 22.0 + p.0 + (q.0 - p.0) * f,
                            (b - 0.5) * 40.0 + p.1 + (q.1 - p.1) * f,
                        )
                    }
                };
                Point::new(w * 0.5 + x * scale, h * 0.5 + y * scale)
            })
            .collect();
        preview = preview.child(line(
            m,
            base + 4 + band,
            points,
            0.7,
            Color::srgba(
                color.red,
                color.green,
                color.blue,
                0.55 + band as f32 * 0.035,
            ),
        ));
    }
    card.child(preview)
}
