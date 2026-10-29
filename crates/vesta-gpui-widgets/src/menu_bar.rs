//! Shared collapsible application-menu headers and stable root navigation.
use crate::{
    ChromeButtonRole, ChromeButtonStyle, Widget, WidgetIdentity, WidgetTheme, chrome_button,
};
use vesta_gpui::{Dimension, ElementId, MenuId, Point, Semantic, SemanticRole};

pub struct MenuBarEntry<A> {
    pub menu: MenuId,
    pub root: WidgetIdentity,
    pub label: WidgetIdentity,
    pub caption: String,
    pub accessible_name: String,
    pub width: f32,
    pub action: A,
}

#[derive(Clone, Copy)]
pub struct MenuBarView {
    pub expanded: bool,
    pub current: Option<MenuId>,
    pub hovered: Option<ElementId>,
    pub height: f32,
}

pub fn menu_bar<A>(
    entries: Vec<MenuBarEntry<A>>,
    view: MenuBarView,
    theme: WidgetTheme,
) -> Vec<Widget<A>> {
    if !view.expanded {
        return Vec::new();
    }
    entries
        .into_iter()
        .map(|entry| {
            let open = view.current == Some(entry.menu);
            let hovered = view.hovered == Some(entry.root.element);
            let mut widget = chrome_button(
                entry.root,
                entry.label,
                &entry.caption,
                &entry.accessible_name,
                entry.action,
                ChromeButtonStyle {
                    role: ChromeButtonRole::Command,
                    width: entry.width,
                    selected: open,
                    hovered,
                    destructive: false,
                },
                theme,
            );
            widget.element.style.height = Dimension::Px(view.height);
            widget.element.semantic =
                Some(Semantic::new(SemanticRole::MenuItem, entry.accessible_name).expanded(open));
            widget
        })
        .collect()
}

#[must_use]
pub fn menu_bar_next(roots: &[MenuId], current: MenuId, backwards: bool) -> Option<MenuId> {
    let index = roots.iter().position(|root| *root == current)?;
    let next = if backwards {
        (index + roots.len() - 1) % roots.len()
    } else {
        (index + 1) % roots.len()
    };
    roots.get(next).copied()
}

#[must_use]
pub fn menu_bar_anchor(roots: &[(MenuId, f32)], current: MenuId, origin: Point) -> Option<Point> {
    let mut x = origin.x;
    for &(root, width) in roots {
        if root == current {
            return Some(Point::new(x, origin.y));
        }
        x += width.max(0.0);
    }
    None
}
