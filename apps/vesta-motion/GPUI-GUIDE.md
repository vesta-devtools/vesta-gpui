# Vesta Motion：GPUI 应用示例导读

从[示例入口](../../README.md#vesta-motion)或[运行说明](README.md)进入。本文解释现有实现怎样组合公共 GPUI 能力，应用代码的唯一入口是本目录的 Cargo package `vesta-motion`。

## 从可见界面找到实现

| 界面或行为 | 使用的 GPUI 能力 | Motion 源码 |
| --- | --- | --- |
| 顶栏、工具栏、左右面板、底部区域 | `Element`、`Style`、布局尺寸、间距与主题；`RetainedLayoutEngine` 求导出布局 | [view.rs](src/view.rs)、[app.rs](src/app.rs) |
| 按钮、参数滑块、图层列表 | 共享 `button_with_presentation`、`slider`、`list_item`；调用方提供稳定身份和 typed Action | [view.rs](src/view.rs) |
| 六张案例卡片 | 共享按钮承载自定义内容；缩略图通过 GPUI polyline 绘制，正方形预览区内等比居中 | [view/gallery.rs](src/view/gallery.rs) |
| 时间刻度、轨道、播放头与曲线 | GPUI 文字、线条、背景与命中；时间轴发布 Slider 语义 | [view/timeline.rs](src/view/timeline.rs) |
| 点击、拖动、Tab、方向键与快捷键 | `HitTestTree`、`ActionRoute`、`PointerClick`、`InputOwnership`、`FocusScope` 与 winit 事件 | [app.rs](src/app.rs) |
| 控件无障碍语义 | `AccessibilityTree` 投影与 winit `AccessibilityAdapter`，按钮及滑块提供名称和状态 | [app.rs](src/app.rs)、[view.rs](src/view.rs) |
| GPUI 与三维画面同窗呈现 | `PaintList`、文本光栅化、公共 Vulkan `WindowRenderer` 与 Viewport 场景 | [app.rs](src/app.rs) |

时间轴、曲线和案例缩略图是这个应用用 GPUI 绘制原语组合的内容，当前没有把它们声明成共享的 Timeline / Curve Editor 控件。无障碍语义已接入，完整读屏资格仍由平台验收台账判定。

## 跟踪一次参数拖动

1. `view.rs` 的参数行调用共享 `slider`，为 root、track、fill、thumb 和值标签提供稳定身份，绑定 `Action::Parameter(param)`。
2. `App::draw` 用当前 `Model` 构建界面，通过 `RetainedLayoutEngine` 得到实际布局，再建立 Action 路由、命中树和绘制列表。
3. `app.rs` 把 winit 指针坐标转换成逻辑坐标，从当前命中与路由取得 typed Action。主键拖动由 `InputOwnership` 捕获，应用把滑块位置转换为参数值。
4. `model.rs` 的 `Model::set` 约束有效数值，修改当前案例的内存参数。下一帧的文字、滑块、曲线与三维预览读取同一份状态。
5. `preview::scene` 生成宿主显示网格，公共渲染器把 GPUI 的绘制列表与 Viewport 场景合成到同一窗口。GPUI 不承担几何生成语义。

取消拖动时应用恢复手势开始前的快照；这属于预览手势回滚，尚未提供文档 Undo/Redo。切换案例使用同一条 typed Action 路径，每个案例保留自己的参数；Reset 只影响当前案例。

## 建议阅读顺序

| 文件 | 职责 |
| --- | --- |
| [main.rs](src/main.rs) | 程序入口与安全约束 |
| [study.rs](src/study.rs) | 案例类型、稳定 key / UI slot、显示名称和默认参数 |
| [model.rs](src/model.rs) | 调用方拥有的状态、typed Action、参数与有界播放时间 |
| [view.rs](src/view.rs) | 工作区声明、共享控件组合及参数显示 |
| [view/gallery.rs](src/view/gallery.rs) | 自适应案例卡片与矢量示意缩略图 |
| [view/timeline.rs](src/view/timeline.rs) | 时间轴及只读动画驱动图形 |
| [app.rs](src/app.rs) | 原生事件、输入所有权、焦点、布局、GPU 生命周期与有限 smoke |
| [preview.rs](src/preview.rs)、[preview/gallery.rs](src/preview/gallery.rs) | 六种宿主显示预览与显式时间动画 |
| [preview/geometry.rs](src/preview/geometry.rs) | 仅用于显示的有界几何辅助函数 |
| [dependency_boundaries.rs](tests/dependency_boundaries.rs) | 公共 UI 包与宿主预览依赖边界的既有检查 |

## 可以从这个示例复用什么

可以参考稳定控件身份、typed Action、调用方状态、共享 Widgets 组合，以及把实际布局依次投影成命中、焦点、绘制和语义的方式。图形内容使用同一套 GPUI 基础绘制能力，宿主负责时间、刷新和 Viewport 内容。

接入自己的应用时保留原有领域模型的权威性。Motion 的 `Model` 仅管理内存预览；需要正式 Vesta Graph 作者功能时，应通过 Core Document / Command / Evaluation 契约接入。三维预览不会写作者文档，不进入 GraphResult 或缓存，不提供第二套 Graph Runtime。

Motion 适合演示专业界面、实时参数交互、自定义绘制和 GPU 视口集成。文本编辑 / 中文 IME、复杂集合、菜单、可拖动停靠、持久化和文档撤销工作流需要查看或接入对应的共享能力；本应用当前未演示这些行为。三项既有测试与有限 native smoke 的具体范围、历史证据见 [README 验证部分](README.md#verification)。
