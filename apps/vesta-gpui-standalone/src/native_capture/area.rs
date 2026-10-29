//! Startup-only observer preparation; keep Main Area live during Float input.
use super::*;

pub(super) fn prepare(app: &mut App, item: ItemId) -> Result<ItemId, String> {
    if app
        .demo
        .workspace
        .item(item)
        .is_none_or(|item| item.panel_type.as_str() != "gallery.layout")
    {
        return Err("native_area_source: expected the Layout fixture item".into());
    }
    let main_input = {
        let mut inputs = app
            .demo
            .workspace
            .root()
            .panes()
            .into_iter()
            .filter_map(|id| app.demo.workspace.pane(id))
            .flat_map(|pane| pane.items.iter())
            .filter(|item| item.panel_type.as_str() == "gallery.inputs")
            .map(|item| item.id);
        let input = inputs.next().ok_or("native_area_main: Inputs missing")?;
        if inputs.next().is_some() {
            return Err("native_area_main: Inputs is ambiguous".into());
        }
        input
    };
    let (window, pane) = app
        .demo
        .workspace
        .floating_windows()
        .iter()
        .filter(|window| window.root.panes().len() == 1)
        .flat_map(|window| {
            window
                .root
                .panes()
                .into_iter()
                .map(move |pane| (window.id, pane))
        })
        .find(|(_, pane)| {
            app.demo.workspace.pane(*pane).is_some_and(|pane| {
                pane.items.len() == 1 && pane.items.iter().all(|entry| entry.id == item)
            })
        })
        .ok_or("native_area_source: expected a single-pane floating Layout")?;
    let main_root = app.demo.workspace.root().clone();
    let main_panes = main_root
        .panes()
        .into_iter()
        .filter_map(|id| app.demo.workspace.pane(id).cloned().map(|pane| (id, pane)))
        .collect::<Vec<_>>();
    let mut prepared = app.demo.workspace.clone();
    let input = prepared
        .add_item(pane, "Inputs", PanelTypeId::new("gallery.inputs"))
        .map_err(|error| format!("native_area_add: {error}"))?;
    prepared
        .activate(input)
        .map_err(|error| format!("native_area_activate: {error}"))?;
    if prepared.root() != &main_root
        || main_panes
            .iter()
            .any(|(id, pane)| prepared.pane(*id) != Some(pane))
    {
        return Err("native_area_preparation: Main changed".into());
    }
    app.demo.workspace = prepared;
    app.demo.sync_workspace_ui();
    eprintln!(
        "native_area_fixture prepared window={} main_input={} float_input={} typed_workspace_commands=true direct_tree=false main_unchanged=true",
        window.0, main_input.0, input.0
    );
    Ok(input)
}

#[cfg(test)]
pub(super) fn qualify() {
    let mut app = App::new();
    let item = app
        .demo
        .workspace
        .panes()
        .into_iter()
        .flat_map(|pane| pane.items.iter())
        .find(|item| item.panel_type.as_str() == "gallery.layout")
        .expect("initial Layout")
        .id;
    let original = app.demo.workspace.clone();
    assert!(prepare(&mut app, item).is_err());
    assert_eq!(app.demo.workspace, original);
    assert!(app.float_workspace_item(item, WindowPlacement::new(80.0, 40.0, 360.0, 640.0)));
    let before = app.demo.workspace.clone();
    let input = prepare(&mut app, item).expect("floating Inputs preparation");
    assert_eq!(app.demo.workspace.root(), before.root());
    for pane in before.root().panes() {
        assert_eq!(app.demo.workspace.pane(pane), before.pane(pane));
    }
    let pane = app
        .demo
        .workspace
        .floating_windows()
        .iter()
        .flat_map(|window| window.root.panes())
        .find_map(|id| {
            app.demo
                .workspace
                .pane(id)
                .filter(|pane| pane.items.iter().any(|entry| entry.id == input))
        })
        .expect("same floating source");
    assert_eq!(pane.active, Some(input));
    assert_eq!(
        pane.items.iter().map(|entry| entry.id).collect::<Vec<_>>(),
        [item, input]
    );
    assert_eq!(
        app.demo
            .workspace
            .item(input)
            .expect("Inputs")
            .panel_type
            .as_str(),
        "gallery.inputs"
    );
    let prepared = app.demo.workspace.clone();
    assert!(prepare(&mut app, item).is_err());
    assert_eq!(app.demo.workspace, prepared);
}
