//! Startup-only sample font configuration using the normal host editor constructors.
use super::*;

fn fields(app: &App) -> [&TextInputState; 11] {
    [
        &app.text_state,
        &app.search_state,
        &app.area_state,
        &app.number_state,
        &app.vector_states[0],
        &app.vector_states[1],
        &app.vector_states[2],
        &app.color_hex_state,
        &app.password_state,
        &app.table_note_state,
        &app.readonly_label_state,
    ]
}

fn seeded(
    source: &TextInputState,
    mut target: TextInputState,
    context: &mut TextInputContext,
) -> Result<TextInputState, String> {
    target
        .reset(context, source.text())
        .map_err(|error| format!("native_configured_theme_text: {error:?}"))?;
    Ok(target)
}

pub(super) fn prepare(app: &mut App) -> Result<(), String> {
    if app.window.is_some()
        || !app.native_windows.is_empty()
        || app.rendered_frames != 0
        || app.text_session.is_some()
        || app.input_ownership.focus().is_some()
        || app.input_ownership.capture().is_some()
        || fields(app).iter().any(|field| field.is_composing())
    {
        return Err(
            "native_configured_theme_order: requires idle startup before native frames".into(),
        );
    }
    let mut theme = app.theme.with_density(1.25);
    theme.typography.font_size *= 1.25;
    theme.typography.small_font_size *= 1.25;
    theme.typography.heading_font_size *= 1.25;
    let size = theme.typography.font_size;
    let editor = || TextInputState::new(size, "sans-serif");
    // All replacements are prepared before publishing any new field or theme.
    // These are the existing sample's policies, not a live-editor font migration.
    let text = seeded(&app.text_state, editor(), &mut app.text_context)?;
    let search = seeded(&app.search_state, editor(), &mut app.text_context)?;
    let area = seeded(
        &app.area_state,
        TextInputState::new_multiline(size, "sans-serif", 344.0),
        &mut app.text_context,
    )?;
    let number = seeded(&app.number_state, editor(), &mut app.text_context)?;
    let [x, y, z] = app
        .vector_states
        .each_ref()
        .map(|source| seeded(source, editor(), &mut app.text_context));
    let vector = [x?, y?, z?];
    let color = seeded(&app.color_hex_state, editor(), &mut app.text_context)?;
    let password = seeded(
        &app.password_state,
        TextInputState::new_password(size, "sans-serif", 256),
        &mut app.text_context,
    )?;
    let note = seeded(&app.table_note_state, editor(), &mut app.text_context)?;
    let label = seeded(
        &app.readonly_label_state,
        TextInputState::new_read_only(size, "sans-serif", None),
        &mut app.text_context,
    )?;
    app.text_state = text;
    app.search_state = search;
    app.area_state = area;
    app.number_state = number;
    app.vector_states = vector;
    app.color_hex_state = color;
    app.password_state = password;
    app.table_note_state = note;
    app.readonly_label_state = label;
    app.theme = theme;
    eprintln!(
        "native_configured_theme prepared density=1.25 font_size={} small_font_size={} heading_font_size={} control_height={} compact_control_height={} spacing={} shaped_editors=11 before_first_native_frame=true",
        theme.typography.font_size,
        theme.typography.small_font_size,
        theme.typography.heading_font_size,
        theme.metrics.control_height,
        theme.metrics.compact_control_height,
        theme.metrics.spacing
    );
    Ok(())
}

#[cfg(test)]
pub(super) fn qualify() {
    let signature = |app: &App| {
        fields(app).map(|field| {
            (
                field.text().to_owned(),
                field.inspection_selection(),
                field.is_password(),
                field.is_read_only(),
                field.is_multiline(),
            )
        })
    };
    let mut app = App::new();
    let original = signature(&app);
    let workspace = app.demo.workspace.clone();
    let values = (app.demo.number, app.demo.vector, app.demo.slider);
    let height = app.text_state.snapshot().layout_height();
    prepare(&mut app).expect("configured sample preparation");
    assert_eq!(
        signature(&app),
        original,
        "sample buffers and editor policies"
    );
    assert_eq!(app.demo.workspace, workspace, "theme is not an author edit");
    assert_eq!((app.demo.number, app.demo.vector, app.demo.slider), values);
    let configured = app.text_state.snapshot().layout_height();
    assert!(
        height > 0.0 && configured > height * 1.2 && configured < height * 1.3,
        "editor snapshot must actually reshape with the configured font: {height} -> {configured}"
    );
    let theme = app.theme;
    app.rendered_frames = 1;
    assert!(prepare(&mut app).is_err(), "never replace a live editor");
    assert_eq!(signature(&app), original);
    assert_eq!(
        app.theme, theme,
        "late refusal preserves current configuration"
    );
    assert_eq!(app.demo.workspace, workspace);
}
