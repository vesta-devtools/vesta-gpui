use std::collections::HashSet;
use std::fmt;

use crate::Point;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MenuId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MenuItemId(pub u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuItem<A> {
    pub id: MenuItemId,
    pub label: String,
    pub shortcut: Option<String>,
    /// Optional host-resolved image resource for the existing leading slot.
    pub icon: Option<String>,
    /// 可选的复选状态。呈现层应使用自身的图标资源绘制，不得把复选框字符拼进 label。
    pub checked: Option<bool>,
    pub enabled: bool,
    pub action: A,
    /// 分隔线项：无文字、无动作，渲染为一条横线且不参与命中。
    pub separator: bool,
    /// hover 提示：缺省回落到 label（最近文件项用它显示完整路径）。
    pub tooltip: Option<String>,
    /// UE 风格 section 头：显示在该 section 分割线前的小字分组名
    /// （构造时同时置 `separator = true`，渲染为「小字名 + 分割线」）。
    pub section_header: Option<String>,
    /// Optional child menu. The item still owns a typed action so hosts may record opening it;
    /// activation normally enters the submenu instead of executing the leaf action.
    pub submenu: Option<MenuId>,
}

impl<A> MenuItem<A> {
    pub fn new(id: MenuItemId, label: impl Into<String>, action: A) -> Self {
        Self {
            id,
            label: label.into(),
            shortcut: None,
            icon: None,
            checked: None,
            enabled: true,
            action,
            separator: false,
            tooltip: None,
            section_header: None,
            submenu: None,
        }
    }

    #[must_use]
    pub fn shortcut(mut self, value: impl Into<String>) -> Self {
        self.shortcut = Some(value.into());
        self
    }

    #[must_use]
    pub fn icon(mut self, resource: impl Into<String>) -> Self {
        self.icon = Some(resource.into());
        self
    }

    #[must_use]
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = Some(checked);
        self
    }

    #[must_use]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// hover 提示文本（如最近文件项的完整路径）；未设置时渲染层回落到 label。
    #[must_use]
    pub fn tooltip(mut self, value: impl Into<String>) -> Self {
        self.tooltip = Some(value.into());
        self
    }

    #[must_use]
    pub const fn submenu(mut self, menu: MenuId) -> Self {
        self.submenu = Some(menu);
        self
    }
}

impl<A: Default> MenuItem<A> {
    /// 分隔线项：无文字、无动作，渲染为一条横线且不参与命中。
    pub fn separator(id: MenuItemId) -> Self {
        Self {
            id,
            label: String::new(),
            shortcut: None,
            icon: None,
            checked: None,
            enabled: false,
            action: A::default(),
            separator: true,
            tooltip: None,
            section_header: None,
            submenu: None,
        }
    }

    /// UE 风格 section 头：小字分组名显示在该 section 的分割线前。
    pub fn section_header(id: MenuItemId, name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            id,
            label: name.clone(),
            shortcut: None,
            icon: None,
            checked: None,
            enabled: false,
            action: A::default(),
            separator: true,
            tooltip: None,
            section_header: Some(name),
            submenu: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Menu<A> {
    pub id: MenuId,
    pub label: String,
    pub items: Vec<MenuItem<A>>,
    /// 弹出位置 override：Some 时菜单显示在该点（如面板齿轮菜单），
    /// None 时按主菜单栏布局。
    pub position: Option<Point>,
}

impl<A> Menu<A> {
    pub fn new(id: MenuId, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            items: Vec::new(),
            position: None,
        }
    }

    #[must_use]
    pub fn item(mut self, item: MenuItem<A>) -> Self {
        self.items.push(item);
        self
    }

    #[must_use]
    pub fn with_position(mut self, position: Point) -> Self {
        self.position = Some(position);
        self
    }
}

#[derive(Clone, Debug)]
pub struct MenuRegistry<A> {
    menus: Vec<Menu<A>>,
}

impl<A> Default for MenuRegistry<A> {
    fn default() -> Self {
        Self { menus: Vec::new() }
    }
}

impl<A> MenuRegistry<A> {
    pub fn register(&mut self, menu: Menu<A>) -> Result<(), MenuRegistryError> {
        if self.menus.iter().any(|registered| registered.id == menu.id) {
            return Err(MenuRegistryError::DuplicateMenu(menu.id));
        }

        let mut ids = self
            .menus
            .iter()
            .flat_map(|registered| registered.items.iter().map(|item| item.id))
            .collect::<HashSet<_>>();
        for item in &menu.items {
            if !ids.insert(item.id) {
                return Err(MenuRegistryError::DuplicateItem(item.id));
            }
        }

        self.menus.push(menu);
        Ok(())
    }

    #[must_use]
    pub fn menus(&self) -> &[Menu<A>] {
        &self.menus
    }

    #[must_use]
    pub fn menu(&self, id: MenuId) -> Option<&Menu<A>> {
        self.menus.iter().find(|menu| menu.id == id)
    }

    #[must_use]
    pub fn item(&self, id: MenuItemId) -> Option<&MenuItem<A>> {
        self.menus
            .iter()
            .flat_map(|menu| menu.items.iter())
            .find(|item| item.id == id)
    }

    /// 整体替换一个菜单的条目（动态内容，如最近文件列表）。
    ///
    /// 新条目 id 不得与其它菜单的条目冲突；被替换菜单自身的旧条目不计入冲突。
    pub fn replace_items(
        &mut self,
        menu_id: MenuId,
        items: Vec<MenuItem<A>>,
    ) -> Result<(), MenuRegistryError> {
        if !self.menus.iter().any(|menu| menu.id == menu_id) {
            return Err(MenuRegistryError::MissingMenu(menu_id));
        }
        let mut ids = self
            .menus
            .iter()
            .filter(|other| other.id != menu_id)
            .flat_map(|other| other.items.iter().map(|item| item.id))
            .collect::<HashSet<_>>();
        for item in &items {
            if !ids.insert(item.id) {
                return Err(MenuRegistryError::DuplicateItem(item.id));
            }
        }
        if let Some(menu) = self.menus.iter_mut().find(|menu| menu.id == menu_id) {
            menu.items = items;
        }
        Ok(())
    }

    /// 更新一个菜单的弹出位置（面板齿轮菜单等按点击位置定位）。
    pub fn set_position(&mut self, menu_id: MenuId, position: Point) {
        if let Some(menu) = self.menus.iter_mut().find(|menu| menu.id == menu_id) {
            menu.position = Some(position);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuRegistryError {
    DuplicateMenu(MenuId),
    DuplicateItem(MenuItemId),
    MissingMenu(MenuId),
}

impl fmt::Display for MenuRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateMenu(id) => write!(formatter, "menu id {} is already registered", id.0),
            Self::DuplicateItem(id) => {
                write!(formatter, "menu item id {} is already registered", id.0)
            }
            Self::MissingMenu(id) => write!(formatter, "menu id {} is not registered", id.0),
        }
    }
}

impl std::error::Error for MenuRegistryError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_duplicate_menu_ids() {
        let mut registry = MenuRegistry::<()>::default();
        registry
            .register(Menu::new(MenuId(1), "First"))
            .expect("first menu should register");

        let error = registry
            .register(Menu::new(MenuId(1), "Second"))
            .expect_err("duplicate menu id should fail");
        assert_eq!(error, MenuRegistryError::DuplicateMenu(MenuId(1)));
    }

    #[test]
    fn replace_items_swaps_dynamic_entries_without_duplicate_ids() {
        let mut registry = MenuRegistry::<u8>::default();
        registry
            .register(
                Menu::new(MenuId(1), "File")
                    .item(MenuItem::new(MenuItemId(10), "Static", 1))
                    .item(MenuItem::separator(MenuItemId(11)))
                    .item(MenuItem::new(MenuItemId(12), "Dynamic Slot", 2)),
            )
            .expect("initial menu registers");
        // 替换动态区：同 id 12 复用不算冲突。
        registry
            .replace_items(
                MenuId(1),
                vec![
                    MenuItem::new(MenuItemId(10), "Static", 1),
                    MenuItem::separator(MenuItemId(11)),
                    MenuItem::new(MenuItemId(12), "recent-a.vsj", 2),
                    MenuItem::new(MenuItemId(13), "recent-b.vsj", 3),
                ],
            )
            .expect("replace with reused ids");
        let menu = registry.menu(MenuId(1)).expect("menu exists");
        assert_eq!(menu.items.len(), 4);
        assert_eq!(menu.items[2].label, "recent-a.vsj");
        // 与其它菜单的 id 冲突被拒绝，且替换不生效（保持旧内容）。
        registry
            .register(Menu::new(MenuId(2), "Window").item(MenuItem::new(MenuItemId(90), "W", 9)))
            .expect("second menu registers");
        let error = registry
            .replace_items(MenuId(1), vec![MenuItem::new(MenuItemId(90), "clash", 0)])
            .expect_err("cross-menu duplicate id must fail");
        assert_eq!(error, MenuRegistryError::DuplicateItem(MenuItemId(90)));
        assert_eq!(
            registry.menu(MenuId(1)).expect("menu exists").items.len(),
            4,
            "failed replace keeps the previous items"
        );
        // 替换不存在的菜单报 MissingMenu。
        let error = registry
            .replace_items(MenuId(99), Vec::new())
            .expect_err("missing menu must fail");
        assert_eq!(error, MenuRegistryError::MissingMenu(MenuId(99)));
    }
}
