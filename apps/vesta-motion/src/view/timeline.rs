use super::*;
use crate::model::DURATION;

pub fn build(m: &Model, width: f32) -> Widget<Action> {
    let mut root = container(m, 950, width, BOTTOM - 24.0, Axis::Column, Some(PANEL));
    let mut header = container(m, 951, width, 32.0, Axis::Row, Some(DARK));
    header.element.style = header
        .element
        .style
        .align(Align::Center)
        .padding(Insets::symmetric(14.0, 0.0))
        .gap(25.0)
        .border(1.0, BORDER);
    header = header
        .child(text(m, 952, "Timeline", 12.0, MINT))
        .child(text(m, 953, "Animation samples · read-only", 11.0, MUTED));
    let mut transport = container(m, 960, width, 38.0, Axis::Row, Some(DARK));
    transport.element.style = transport
        .element
        .style
        .align(Align::Center)
        .padding(Insets::symmetric(14.0, 0.0))
        .gap(8.0);
    transport = transport
        .child(text(m, 961, m.timecode(), 14.0, MINT))
        .child(text(m, 962, " 30 fps ", 10.0, MUTED))
        .child(button(m, 66, "|◀", Action::Start, 30.0, false))
        .child(button(
            m,
            PLAY,
            if m.playing { "Ⅱ" } else { "▶" },
            Action::Play,
            38.0,
            true,
        ))
        .child(button(m, 72, "▶|", Action::End, 30.0, false))
        .child(button(m, 74, "∞", Action::Loop, 32.0, m.looping));
    let mut spacer = text(m, 963, "Drag timeline to scrub", 10.0, MUTED);
    spacer.element.style = spacer.element.style.flex(1.0, 1.0);
    transport =
        transport
            .child(spacer)
            .child(text(m, 964, "Motion drivers · read-only", 10.0, MUTED));
    let curve_width = (width * 0.25).min(360.0);
    let track_width = width - LEFT - curve_width - 2.0;
    let height = BOTTOM - 24.0 - 70.0;
    let mut body = container(m, 970, width, height, Axis::Row, None);
    let mut names = container(m, 971, LEFT, height, Axis::Column, Some(PANEL));
    names.element.style = names.element.style.padding(Insets::symmetric(16.0, 0.0));
    for (i, (name, color)) in [
        ("", MUTED),
        (m.study.geometry(), m.theme.palette.text),
        ("      ·  Phase", MUTED),
        ("      ·  Cycle", MUTED),
        (m.layer_name(Layer::Trails), m.theme.palette.text),
        ("T    Title", m.theme.palette.text),
    ]
    .into_iter()
    .enumerate()
    {
        let mut row = text(m, 980 + i as u32, name, 11.0, color);
        row.element.style.height = D::Px(24.0);
        names = names.child(row);
    }
    let mut tracks = container(
        m,
        SEEK,
        track_width,
        height,
        Axis::Column,
        Some(Color::rgb8(25, 29, 32)),
    )
    .on_click(Action::Seek);
    tracks.element.style = tracks.element.style.focusable(true);
    tracks.element.semantic = Some(
        Semantic::new(SemanticRole::Slider, "Timeline position")
            .value(m.timecode())
            .numeric(m.time as f64, 0.0, 12.0, Some(1.0 / 30.0)),
    );
    for second in 0..=12 {
        let x = 10.0 + (track_width - 20.0) * second as f32 / DURATION;
        tracks = tracks.child(line(
            m,
            1100 + second * 2,
            vec![Point::new(x, 18.0), Point::new(x, height)],
            0.7,
            BORDER,
        ));
        tracks = tracks.child(at(
            text(m, 1101 + second * 2, format!("{second}s"), 9.0, MUTED),
            x + 2.0,
            -4.0,
        ));
    }
    for (i, (start, end, color)) in [
        (0.0, 12.0, Color::rgb8(56, 82, 75)),
        (0.0, 12.0, Color::rgb8(48, 56, 61)),
        (0.0, 12.0, Color::rgb8(48, 56, 61)),
        (0.0, 12.0, Color::rgb8(51, 58, 77)),
        (0.0, 12.0, Color::rgb8(83, 66, 43)),
    ]
    .into_iter()
    .enumerate()
    {
        let y = 24.0 + i as f32 * 24.0;
        let x = 10.0 + (track_width - 20.0) * start / DURATION;
        let w = (track_width - 20.0) * (end - start) / DURATION;
        let mut clip = container(m, 1200 + i as u32, w, 17.0, Axis::Row, Some(color));
        clip.element.style = clip
            .element
            .style
            .border(0.7, Color::rgb8(92, 105, 107))
            .radius(2.0)
            .pointer_events(false);
        tracks = tracks.child(at(clip, x, y + 3.0));
    }
    for (i, seconds) in [
        vec![0.0, 3.0, 6.0, 9.0, 12.0],
        vec![0.0, 3.0, 6.0, 9.0, 12.0],
    ]
    .into_iter()
    .enumerate()
    {
        for (j, second) in seconds.into_iter().enumerate() {
            let x = 10.0 + (track_width - 20.0) * second / DURATION;
            let mut key = container(
                m,
                1250 + i as u32 * 10 + j as u32,
                6.0,
                6.0,
                Axis::Row,
                Some(if i == 0 { MINT } else { GOLD }),
            );
            key.element.style = key
                .element
                .style
                .rotation(std::f32::consts::FRAC_PI_4)
                .pointer_events(false);
            tracks = tracks.child(at(key, x - 3.0, 56.0 + i as f32 * 24.0));
        }
    }
    let x = 10.0 + (track_width - 20.0) * m.time / DURATION;
    tracks = tracks.child(line(
        m,
        1290,
        vec![Point::new(x, 17.0), Point::new(x, height)],
        1.1,
        MINT,
    ));
    tracks = tracks.child(at(
        container(m, 1291, 8.0, 6.0, Axis::Row, Some(MINT)),
        x - 4.0,
        16.0,
    ));
    body = body
        .child(names)
        .child(tracks)
        .child(container(m, 1292, 1.0, height, Axis::Column, Some(BORDER)))
        .child(curve(m, curve_width + 1.0, height));
    root = root.child(header).child(transport).child(body);
    root
}

fn curve(m: &Model, width: f32, height: f32) -> Widget<Action> {
    let mut plot = container(m, 2000, width, height, Axis::Column, Some(DARK));
    let w = width - 32.0;
    let h = height - 33.0;
    for i in 0..=6 {
        let x = 16.0 + i as f32 * w / 6.0;
        plot = plot.child(line(
            m,
            2010 + i,
            vec![Point::new(x, 8.0), Point::new(x, height - 20.0)],
            0.6,
            BORDER,
        ));
    }
    for i in 0..=3 {
        let y = 8.0 + i as f32 * h / 3.0;
        plot = plot.child(line(
            m,
            2020 + i,
            vec![Point::new(16.0, y), Point::new(width - 16.0, y)],
            0.6,
            BORDER,
        ));
    }
    for (i, color) in [MINT, GOLD].into_iter().enumerate() {
        let points = (0..=80)
            .map(|step| {
                let t = step as f32 / 80.0;
                let value = if i == 0 {
                    t
                } else {
                    0.5 + crate::preview::twist(t * DURATION) * 2.0
                };
                Point::new(16.0 + t * w, 8.0 + (1.0 - value) * h)
            })
            .collect();
        plot = plot.child(line(m, 2030 + i as u32, points, 1.5, color));
    }
    let x = 16.0 + m.time / DURATION * w;
    plot.child(line(
        m,
        2040,
        vec![Point::new(x, 6.0), Point::new(x, height - 20.0)],
        1.0,
        MINT,
    ))
    .child(at(text(m, 2041, "Turns", 9.0, MINT), 16.0, height - 20.0))
    .child(at(text(m, 2042, "Cycle", 9.0, GOLD), 86.0, height - 20.0))
    .child(at(
        text(m, 2043, "12 s", 9.0, MUTED),
        width - 40.0,
        height - 20.0,
    ))
}
