//! Host-owned copy for the standalone acceptance UI. Widget and render crates
//! only receive resolved text; author data stays verbatim.

use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};

use vesta_gpui::{Element, ElementKind, TextSource};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UiLocale {
    #[default]
    EnUs,
    ZhCn,
}

impl UiLocale {
    pub const fn tag(self) -> &'static str {
        match self {
            Self::EnUs => "en-US",
            Self::ZhCn => "zh-CN",
        }
    }

    pub const fn toggled(self) -> Self {
        match self {
            Self::EnUs => Self::ZhCn,
            Self::ZhCn => Self::EnUs,
        }
    }

    pub const fn window_title(self) -> &'static str {
        match self {
            Self::EnUs => "Vesta GPUI Standalone — 2D Widget Gallery / 3D Viewport",
            Self::ZhCn => "Vesta GPUI 独立界面 — 2D 控件展示 / 3D 视口",
        }
    }
}

struct CopyEntry {
    key: &'static str,
    en: &'static str,
    zh: &'static str,
}

macro_rules! copy {
    ($key:literal, $en:literal, $zh:literal) => {
        CopyEntry {
            key: $key,
            en: $en,
            zh: $zh,
        }
    };
}

// Keys are semantic copy identities. The English lookup only adapts older
// widgets that still accept bare strings; new call sites pass a keyed value.
const COPY: &[CopyEntry] = &[
    copy!("chrome.gallery", "2D Gallery", "2D 展示"),
    copy!("chrome.viewport", "3D Viewport", "3D 视口"),
    copy!("chrome.file", "File", "文件"),
    copy!("chrome.view", "View", "视图"),
    copy!("chrome.window", "Window", "窗口"),
    copy!("chrome.windowCompact", "Win", "窗口"),
    copy!("chrome.light", "Light", "浅色"),
    copy!("chrome.dark", "Dark", "深色"),
    copy!("chrome.appMenu", "App menu", "应用菜单"),
    copy!("chrome.fileMenu", "File menu", "文件菜单"),
    copy!("chrome.viewMenu", "View menu", "视图菜单"),
    copy!("chrome.windowMenu", "Window menu", "窗口菜单"),
    copy!("chrome.minimize", "Minimize window", "最小化窗口"),
    copy!(
        "chrome.maximize",
        "Maximize or restore window",
        "最大化或还原窗口"
    ),
    copy!("chrome.close", "Close window", "关闭窗口"),
    copy!(
        "chrome.toggleTheme",
        "Toggle light and dark theme",
        "切换浅色与深色主题"
    ),
    copy!("chrome.switchTheme", "Switch theme", "切换主题"),
    copy!("chrome.resetWorkspace", "Reset workspace", "重置工作区"),
    copy!("chrome.moreStatus", "More status items", "更多状态信息"),
    copy!(
        "chrome.moreToolbar",
        "More toolbar actions",
        "更多工具栏操作"
    ),
    copy!(
        "chrome.switchThemeShortcut",
        "Switch theme (T)",
        "切换主题（T）"
    ),
    copy!("chrome.newTab", "New Tab", "新建标签页"),
    copy!("chrome.newGalleryTab", "New Gallery tab", "新建展示标签页"),
    copy!("chrome.openGallery", "Open 2D Gallery", "打开 2D 展示"),
    copy!("chrome.openViewport", "Open 3D Viewport", "打开 3D 视口"),
    copy!("chrome.toggleThemeCommand", "Toggle Theme", "切换主题"),
    copy!(
        "chrome.resetWorkspaceCommand",
        "Reset Workspace",
        "重置工作区"
    ),
    copy!(
        "chrome.cancelActivity",
        "Cancel Gallery Activity",
        "取消展示活动"
    ),
    copy!(
        "chrome.restartActivity",
        "Restart Gallery Activity",
        "重新开始展示活动"
    ),
    copy!("pane.controls", "Controls", "控件"),
    copy!("pane.inputs", "Inputs", "输入"),
    copy!("pane.collections", "Collections", "集合"),
    copy!("pane.layout", "Layout", "布局"),
    copy!("pane.actions", "Actions", "操作"),
    copy!("pane.info", "Info", "信息"),
    copy!("pane.general", "General", "常规"),
    copy!("pane.render", "Render", "渲染"),
    copy!("pane.input", "Input", "输入"),
    copy!("pane.sceneHierarchy", "Scene hierarchy", "场景层级"),
    copy!("pane.unavailable", "Workspace unavailable", "工作区不可用"),
    copy!("pane.headerActions", "Pane actions", "面板操作"),
    copy!("pane.tabMenu", "Tab", "标签页"),
    copy!("pane.zoom", "Zoom this pane", "放大此面板"),
    copy!("pane.restore", "Restore split panes", "还原分割面板"),
    copy!(
        "pane.moveRight",
        "Move active tab into a pane on the right",
        "将当前标签页移到右侧面板"
    ),
    copy!("pane.move", "Move active tab", "移动当前标签页"),
    copy!("pane.split", "Split active tab", "分割当前标签页"),
    copy!("pane.float", "Float active tab", "浮动当前标签页"),
    copy!(
        "pane.tearOffActive",
        "Tear off active tab",
        "撕下当前标签页"
    ),
    copy!("pane.tearOff", "Tear off tab", "撕下标签页"),
    copy!("pane.tearOffShort", "Tear off", "撕下"),
    copy!("pane.back", "Back in pane history", "面板历史后退"),
    copy!("pane.forward", "Forward in pane history", "面板历史前进"),
    copy!("pane.closeControls", "Close Controls", "关闭控件页"),
    copy!("pane.closeInputs", "Close Inputs", "关闭输入页"),
    copy!("pane.closeCollections", "Close Collections", "关闭集合页"),
    copy!("pane.closeLayout", "Close Layout", "关闭布局页"),
    copy!("pane.resize", "Resize dock panes", "调整停靠面板大小"),
    copy!("pane.dockBack", "Dock back", "停靠回主窗口"),
    copy!("pane.toggleTheme", "Toggle theme", "切换主题"),
    copy!("menu.darkTheme", "Dark theme", "深色主题"),
    copy!(
        "gallery.buttons",
        "Buttons & command controls",
        "按钮与命令控件"
    ),
    copy!(
        "gallery.toggles",
        "Toggle / Check / Radio",
        "开关 / 复选 / 单选"
    ),
    copy!(
        "gallery.conditions",
        "Compound condition: toggle disables; Option B hides",
        "复合条件：开关禁用；选项 B 隐藏"
    ),
    copy!("gallery.feedback", "Range & feedback", "范围与反馈"),
    copy!(
        "gallery.inputs",
        "Text / Number / Search input",
        "文本 / 数字 / 搜索输入"
    ),
    copy!(
        "gallery.multiline",
        "Multiline text / wrap / IME",
        "多行文本 / 折行 / 输入法"
    ),
    copy!("gallery.selection", "Selection controls", "选择控件"),
    copy!("gallery.property", "Composed property row", "组合属性行"),
    copy!(
        "gallery.navigation",
        "Navigation & collections",
        "导航与集合"
    ),
    copy!(
        "gallery.nestedScroll",
        "Nested scroll (wheel at edge passes to page)",
        "嵌套滚动（到边缘后滚动页面）"
    ),
    copy!(
        "gallery.horizontalScroll",
        "Horizontal scroll (trackpad X / Shift + wheel)",
        "水平滚动（触控板 X / Shift + 滚轮）"
    ),
    copy!(
        "gallery.grid",
        "Grid tracks and cell spans",
        "网格轨道与跨列单元格"
    ),
    copy!(
        "gallery.collapse",
        "Collapsible / accordion",
        "折叠区 / 手风琴"
    ),
    copy!(
        "gallery.virtualList",
        "Virtual list · 10,000 objects",
        "虚拟列表 · 10,000 个对象"
    ),
    copy!(
        "gallery.virtualTree",
        "Virtual tree · 1,000 objects",
        "虚拟树 · 1,000 个对象"
    ),
    copy!("gallery.overlays", "Layout & overlays", "布局与叠加层"),
    copy!("control.primary", "Primary", "主要"),
    copy!("control.hovered", "Hovered", "悬停"),
    copy!("control.pressed", "Pressed", "按下"),
    copy!("control.disabled", "Disabled", "禁用"),
    copy!("control.checkbox", "Checkbox", "复选框"),
    copy!("control.columnSpan", "Column span 2", "跨两列"),
    copy!("control.switchThemeTitle", "Switch Theme", "切换主题"),
    copy!("control.apply", "Apply", "应用"),
    copy!("control.save", "Save", "保存"),
    copy!("control.expand", "Expand", "展开"),
    copy!("control.hide", "Hide", "隐藏"),
    copy!("control.clearSearch", "Clear search", "清除搜索"),
    copy!("control.commandPalette", "Command palette", "命令面板"),
    copy!("control.colorModel", "Color model", "色彩模式"),
    copy!("control.pickColor", "Pick color", "选取颜色"),
    copy!("control.hexRgba", "Hex RGBA", "十六进制 RGBA"),
    copy!("control.rgbaColor", "RGBA color", "RGBA 颜色"),
    copy!("control.mixed", "Mixed", "混合值"),
    copy!("control.list", "List", "列表"),
    copy!("control.noSuggestions", "No suggestions", "没有建议"),
    copy!("control.information", "Information", "信息"),
    copy!("control.error", "Error", "错误"),
    copy!("control.success", "Success", "成功"),
    copy!("control.warning", "Warning", "警告"),
    copy!("control.runningStatus", "Running", "运行中"),
    copy!("control.stopped", "Stopped", "已停止"),
    copy!("control.unavailable", "Unavailable", "不可用"),
    copy!("control.dismissNotice", "Dismiss notification", "关闭通知"),
    copy!(
        "control.copyDisabled",
        "Copy and cut are disabled",
        "复制和剪切已禁用"
    ),
    copy!(
        "control.hexHint",
        "Enter #RRGGBB or #RRGGBBAA",
        "输入 #RRGGBB 或 #RRGGBBAA"
    ),
    copy!(
        "control.numberHint",
        "Type a number or drag horizontally to adjust",
        "输入数字或水平拖动以调整"
    ),
    copy!(
        "control.mixedHint",
        "Type a number to replace the mixed value",
        "输入数字以替换混合值"
    ),
    copy!(
        "control.shortcutHint",
        "Press a shortcut · Esc cancels",
        "按下快捷键 · Esc 取消"
    ),
    copy!(
        "control.shortcutActivate",
        "Activate, then press a shortcut; Escape cancels",
        "激活后按下快捷键；Escape 取消"
    ),
    copy!(
        "control.shortcutConflict",
        "Shortcut in use; press another key",
        "快捷键已占用；请按其他键"
    ),
    copy!("control.collapse", "Collapse", "折叠"),
    copy!("control.decrease", "Decrease", "减小"),
    copy!("control.increase", "Increase", "增大"),
    copy!("control.editableLabel", "Editable label", "可编辑标签"),
    copy!("control.number", "Number", "数字"),
    copy!("control.numberField", "Number field", "数字输入框"),
    copy!("control.password", "Password", "密码"),
    copy!("control.pathAncestors", "Path ancestors", "路径上级"),
    copy!("control.positionX", "Position X", "位置 X"),
    copy!("control.positionY", "Position Y", "位置 Y"),
    copy!("control.positionZ", "Position Z", "位置 Z"),
    copy!("control.progress", "Progress", "进度"),
    copy!("control.rampEditor", "Ramp editor", "渐变编辑器"),
    copy!("control.resizeKind", "Resize Kind column", "调整类型列宽"),
    copy!(
        "control.resizeName",
        "Resize Name ↑ column",
        "调整名称 ↑ 列宽"
    ),
    copy!("control.search", "Search objects", "搜索对象"),
    copy!("control.suggestions", "Suggestions", "建议"),
    copy!(
        "control.searchSuggestions",
        "Search suggestions",
        "搜索建议"
    ),
    copy!("control.table", "Table", "表格"),
    copy!("control.text", "Text", "文本"),
    copy!("control.textArea", "Text area", "多行文本框"),
    copy!("control.tiles", "Tiles", "平铺项"),
    copy!("control.value", "Value", "数值"),
    copy!("control.feature", "Feature", "功能"),
    copy!("control.editColor", "Edit color", "编辑颜色"),
    copy!(
        "help.rampSelection",
        "Ctrl: toggle point; Shift: select range; drag selected points together",
        "Ctrl：切换选择；Shift：选择范围；拖动选中的控制点可整体移动"
    ),
    copy!(
        "control.editPointColor",
        "Edit point color",
        "编辑控制点颜色"
    ),
    copy!("control.interpConstant", "Interp: Constant", "插值：常量"),
    copy!("control.interpLinear", "Interp: Linear", "插值：线性"),
    copy!("control.interpSmooth", "Interp: Smooth", "插值：平滑"),
    copy!("control.searchObjects", "Search objects…", "搜索对象…"),
    copy!("control.setShortcut", "Set shortcut", "设置快捷键"),
    copy!("control.running", "Loading: Running", "加载中：运行中"),
    copy!(
        "control.galleryLocale",
        "中文 · switch to English",
        "中文 · 切换到 English"
    ),
    copy!("control.optionA", "Option A", "选项 A"),
    copy!("control.optionB", "Option B", "选项 B"),
    copy!("control.parentToggle", "Parent: toggle", "父级：开关"),
    copy!("control.openChild", "Open child", "打开子项"),
    copy!("control.show", "Show", "显示"),
    copy!("control.clear", "Clear", "清除"),
    copy!("control.commands", "Commands", "命令"),
    copy!(
        "control.commandHint",
        "Commands (Ctrl+Shift+P)",
        "命令（Ctrl+Shift+P）"
    ),
    copy!("control.sampleShortcut", "Sample shortcut", "示例快捷键"),
    copy!("control.projection", "Projection", "投影"),
    copy!("control.perspective", "Perspective", "透视"),
    copy!("control.orthographic", "Orthographic", "正交"),
    copy!("control.camera", "Camera", "相机"),
    copy!("control.popover", "Popover", "弹出层"),
    copy!("control.showNotice", "Show notice", "显示通知"),
    copy!("control.restart", "Restart", "重新开始"),
    copy!("control.increment", "Increment", "增加"),
    copy!("control.incrementValue", "Increment Value", "增加数值"),
    copy!("control.decrement", "Decrease value", "减小数值"),
    copy!("control.increaseValue", "Increase value", "增大数值"),
    copy!(
        "control.decreaseSample",
        "Decrease sample value",
        "减小示例数值"
    ),
    copy!(
        "control.hoverTooltip",
        "Hover or focus for tooltip",
        "悬停或聚焦查看提示"
    ),
    copy!(
        "control.tooltipBody",
        "Tooltip is framework metadata, not product state",
        "提示属于框架元数据，不是产品状态"
    ),
    copy!("control.openDialog", "Open dialog", "打开对话框"),
    copy!("control.openNested", "Open nested", "打开嵌套对话框"),
    copy!("control.sampleSidebar", "Sample sidebar", "示例侧栏"),
    copy!(
        "control.drawerPage",
        "Host-owned drawer page",
        "宿主持有的抽屉页"
    ),
    copy!("control.mainContent", "Main content", "主要内容"),
    copy!(
        "control.panelPair",
        "Panel A     │     Panel B",
        "面板 A     │     面板 B"
    ),
    copy!(
        "control.rootPath",
        "Root  ›  Assets  ›  Mesh",
        "根目录  ›  资产  ›  网格"
    ),
    copy!("control.assets", "Assets", "资产"),
    copy!("control.objects", "Objects", "对象"),
    copy!("control.noObjects", "No matching objects", "没有匹配的对象"),
    copy!("control.tableName", "Name", "名称"),
    copy!("control.tableKind", "Kind", "类型"),
    copy!("control.tableSize", "Size", "大小"),
    copy!("control.project", "Project", "项目"),
    copy!("control.appearance", "Appearance", "外观"),
    copy!("menu.group.application", "Application", "应用"),
    copy!("menu.group.views", "Views", "视图"),
    copy!("menu.group.tabs", "Tabs", "标签页"),
    copy!("menu.group.workspace", "Workspace", "工作区"),
    copy!("menu.group.output", "Output", "输出"),
    copy!("control.export", "Export", "导出"),
    copy!("control.exit", "Exit", "退出"),
    copy!("control.add", "Add", "添加"),
    copy!("control.remove", "Remove", "移除"),
    copy!("control.undo", "Undo", "撤销"),
    copy!("control.redo", "Redo", "重做"),
    copy!("control.ready", "Ready", "就绪"),
    copy!("control.infoReady", "Information: Ready", "信息：就绪"),
    copy!("control.loading", "Loading", "加载中"),
    copy!("control.saved", "Saved", "已保存"),
    copy!("control.successSaved", "Success: Saved", "成功：已保存"),
    copy!("control.failed", "Failed", "失败"),
    copy!("control.errorFailed", "Error: Failed", "错误：失败"),
    copy!("control.review", "Review", "查看"),
    copy!("control.warningReview", "Warning: Review", "警告：请查看"),
    copy!("control.pinned", "Pinned", "已固定"),
    copy!("control.togglePinned", "Toggle pinned", "切换固定状态"),
    copy!("control.scale", "Scale", "缩放"),
    copy!("control.position", "Position", "位置"),
    copy!("control.rotate", "Rotate", "旋转"),
    copy!("control.roughness", "Roughness", "粗糙度"),
    copy!("control.iterations", "Iterations", "迭代次数"),
    copy!("control.values", "Values", "数值"),
    copy!("control.select", "Select", "选择"),
    copy!("control.rampAbove", "Above", "上方"),
    copy!("control.rampBelow", "Below", "下方"),
    copy!(
        "control.rangeAbove",
        "Above suggested range",
        "高于建议范围"
    ),
    copy!("control.north", "North", "北"),
    copy!("control.south", "South", "南"),
    copy!("control.east", "East", "东"),
    copy!("control.west", "West", "西"),
    copy!("control.left", "Left", "左"),
    copy!("control.right", "Right", "右"),
    copy!("dialog.gallery", "Gallery confirmation", "展示操作确认"),
    copy!("dialog.nested", "Nested confirmation", "嵌套操作确认"),
    copy!(
        "dialog.galleryBody",
        "Confirm adds one to the sample value. The nested dialog closes first.",
        "确认后示例数值加一。嵌套对话框会先关闭。"
    ),
    copy!(
        "dialog.nestedBody",
        "This dialog owns focus until it is confirmed or cancelled.",
        "确认或取消之前，焦点保持在此对话框内。"
    ),
    copy!("dialog.confirm", "Confirm", "确认"),
    copy!("dialog.cancel", "Cancel", "取消"),
    copy!("dialog.dismiss", "Dismiss", "关闭"),
    copy!("palette.search", "Search commands", "搜索命令"),
    copy!("palette.empty", "No matching commands", "没有匹配的命令"),
    copy!("viewport.overlay", "Viewport Overlay", "视口叠加层"),
    copy!(
        "viewport.orbitHint",
        "Left drag: Orbit   Middle drag: Pan",
        "左键拖动：环绕   中键拖动：平移"
    ),
    copy!(
        "viewport.dollyHint",
        "Wheel: Dolly   F: Frame   O: Projection",
        "滚轮：推拉   F：适配   O：投影"
    ),
    copy!(
        "viewport.pipeline",
        "Grid → Selection/Gizmo → UI Overlay",
        "网格 → 选择/操纵器 → 界面叠加层"
    ),
    copy!("viewport.move", "Move", "移动"),
    copy!("viewport.reset_camera", "Reset camera", "重置相机"),
    copy!("viewport.projection", "Proj", "投影"),
    copy!("viewport.topFront", "TOP\nFRONT", "顶视\n前视"),
    copy!("gallery.richIntro", "Explore ", "探索 "),
    copy!(
        "gallery.navigationLink",
        "Gallery navigation link",
        "展示导航链接"
    ),
    copy!(
        "gallery.wrapSample",
        "Parley wraps mixed text within this narrow label: Vesta 地形 مرحبا 😀",
        "Parley 在窄标签内折行混合文本：Vesta 地形 مرحبا 😀"
    ),
    copy!(
        "control.pngLater",
        "PNG image (coming later)",
        "PNG 图像（后续支持）"
    ),
    copy!(
        "control.sceneLater",
        "Scene snapshot (coming later)",
        "场景快照（后续支持）"
    ),
];

fn english_index() -> &'static HashMap<&'static str, &'static str> {
    static INDEX: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut keys = HashSet::new();
        let mut english = HashMap::new();
        for entry in COPY {
            assert!(keys.insert(entry.key), "duplicate copy key: {}", entry.key);
            assert!(
                english.insert(entry.en, entry.zh).is_none(),
                "duplicate English copy: {}",
                entry.en
            );
        }
        english
    })
}

fn translated(value: &str) -> Option<String> {
    if let Some(translation) = english_index().get(value) {
        return Some((*translation).to_owned());
    }
    for (prefix, translated_prefix) in [
        ("Current value: ", "当前值："),
        ("Increase ", "增大 "),
        ("Child: step ", "子项：步进 "),
        ("Sample notice ", "示例通知 "),
        ("Panel unavailable: ", "面板不可用："),
        ("Floating workspace item: ", "浮动工作区项目："),
        ("Floating workspace item ", "浮动工作区项目 "),
        ("Name ", "名称 "),
        ("Edit label: ", "编辑标签："),
        ("Unit: ", "单位："),
    ] {
        if let Some(suffix) = value.strip_prefix(prefix) {
            return Some(format!("{translated_prefix}{suffix}"));
        }
    }
    if let Some(column) = value
        .strip_prefix("Resize ")
        .and_then(|value| value.strip_suffix(" column"))
    {
        let column = translated(column).unwrap_or_else(|| column.to_owned());
        return Some(format!("调整{column}列宽"));
    }
    if let Some((caption, shortcut)) = value.split_once(" (")
        && let Some(shortcut) = shortcut.strip_suffix(')')
        && let Some(caption) = translated(caption)
    {
        return Some(format!("{caption}（{shortcut}）"));
    }
    if let Some(rest) = value.strip_prefix("Point ")
        && let Some((index, position)) = rest.split_once(" at ")
    {
        return Some(format!("控制点 {index}，位置 {position}"));
    }
    if let Some(id) = value
        .strip_prefix("Point ")
        .and_then(|rest| rest.strip_suffix(" color"))
    {
        return Some(format!("控制点 {id} 颜色"));
    }
    None
}

pub fn display_text(value: &str, locale: UiLocale) -> String {
    if locale == UiLocale::EnUs {
        value.to_owned()
    } else {
        translated(value).unwrap_or_else(|| value.to_owned())
    }
}

/// Applies the transitional copy adapter to a complete ephemeral UI tree.
/// TextInput snapshots and author content are deliberately untouched.
pub fn localize_tree(element: &mut Element, locale: UiLocale) {
    if locale == UiLocale::EnUs {
        return;
    }
    if element.text_source == TextSource::Unspecified
        && let ElementKind::Text(value) = &mut element.kind
        && let Some(translation) = translated(value)
    {
        *value = translation;
    }
    if element.text_source == TextSource::Unspecified
        && let Some(semantic) = element.semantic.as_mut()
    {
        if let Some(translation) = translated(&semantic.label) {
            semantic.label = translation;
        }
        if let Some(description) = semantic.description.as_mut()
            && let Some(translation) = translated(description)
        {
            *description = translation;
        }
    }
    if let Some(tooltip) = element.tooltip.as_mut()
        && let Some(translation) = translated(tooltip)
    {
        *tooltip = translation;
    }
    for child in &mut element.children {
        localize_tree(child, locale);
    }
}
