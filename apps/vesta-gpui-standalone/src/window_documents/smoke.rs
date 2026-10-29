//! Isolated native projection/submit proof, not OS input or Clipboard acceptance.
use super::*;
use vesta_gpui::ElementKind;
mod edge;

pub(crate) fn smoke_enabled() -> bool {
    std::env::args().any(|argument| argument == "--smoke-markdown-selection")
}

pub(crate) fn validate_smoke_args() -> Result<(), String> {
    if smoke_enabled()
        && (!std::env::args().any(|argument| argument == "--smoke")
            || std::env::args().any(|argument| {
                argument == "--mode=3d"
                    || argument.starts_with("--smoke-")
                        && argument != "--smoke-markdown-selection"
                        && !argument.starts_with("--smoke-frames=")
            }))
    {
        return Err("standalone_markdown_smoke_invalid: use isolated --smoke --smoke-markdown-selection in 2D".into());
    }
    Ok(())
}

#[derive(Default)]
pub(crate) enum Smoke {
    #[default]
    Disabled,
    Open,
    Navigate,
    Select,
    PageDown,
    PageUp,
    Selected,
    Reflow {
        width: f32,
        copied: String,
    },
    DragResize {
        item: ItemId,
    },
    Drag {
        item: ItemId,
        now: Instant,
        ticks: u8,
        initial: vesta_gpui::ReadOnlyDocumentSelection,
        held: Point,
        offset: f32,
    },
    Edge(edge::Phase),
}

impl Smoke {
    pub(crate) fn from_args() -> Self {
        if smoke_enabled() {
            Self::Open
        } else {
            Self::Disabled
        }
    }
}

impl App {
    pub(crate) fn advance_document_smoke(&mut self) -> Result<bool, String> {
        if self.rendered_frames > 32 {
            return Err("Markdown smoke exceeded its frame budget".into());
        }
        let phase = std::mem::take(&mut self.document_smoke);
        match phase {
            Smoke::Disabled => return Err("Markdown smoke owner is unavailable".into()),
            Smoke::Open => {
                self.apply_demo_action(DemoAction::ShowDocuments);
                self.document_smoke = Smoke::Navigate;
            }
            Smoke::Navigate => {
                self.set_window_focus(STANDALONE_WINDOW, Some(self.scopes.markdown.element(0)));
                let target = self.scopes.markdown.element(0);
                let rect = self
                    .with_document_input(STANDALONE_WINDOW, |interaction, surface, input| {
                        interaction
                            .key(
                                &surface,
                                KeyChord::new(
                                    KeyStroke::Home,
                                    KeyModifiers {
                                        control: true,
                                        ..Default::default()
                                    },
                                ),
                                64 * 1024,
                                input,
                            )
                            .map_err(|error| error.to_string())?;
                        let moved = interaction
                            .key(
                                &surface,
                                KeyChord::new(
                                    KeyStroke::ArrowDown,
                                    KeyModifiers {
                                        shift: true,
                                        ..Default::default()
                                    },
                                ),
                                64 * 1024,
                                input,
                            )
                            .map_err(|error| error.to_string())?;
                        if moved != Some(MarkdownKeyIntent::RevealSelection) {
                            return Err(format!("Markdown visual navigation rejected: {moved:?}"));
                        }
                        interaction
                            .selection_focus_rect(&surface)
                            .map_err(|error| error.to_string())?
                            .ok_or("Markdown navigation focus geometry unavailable".into())
                    })
                    .ok_or("Markdown navigation projection unavailable")??;
                self.reveal_window_rect(STANDALONE_WINDOW, target, rect);
                self.document_smoke = Smoke::Select;
            }
            Smoke::Select => {
                self.verify_document_navigation_smoke()?;
                self.verify_document_link_smoke()?;
                self.set_window_focus(STANDALONE_WINDOW, Some(self.scopes.markdown.element(0)));
                self.move_document_page_smoke(KeyStroke::PageDown, true)?;
                self.document_smoke = Smoke::PageDown;
            }
            Smoke::PageDown => {
                self.verify_document_page_smoke(false)?;
                self.move_document_page_smoke(KeyStroke::PageUp, false)?;
                self.document_smoke = Smoke::PageUp;
            }
            Smoke::PageUp => {
                self.verify_document_page_smoke(true)?;
                let target = self.scopes.markdown.element(0);
                if !self.ui_actions.focus_order().contains(&target) {
                    return Err("Markdown focus target was not projected".into());
                }
                self.set_window_focus(STANDALONE_WINDOW, Some(target));
                let command = KeyModifiers {
                    control: true,
                    ..Default::default()
                };
                let result =
                    self.with_document_input(STANDALONE_WINDOW, |interaction, surface, input| {
                        interaction.key(
                            &surface,
                            KeyChord::character('a', command),
                            64 * 1024,
                            input,
                        )
                    });
                if !matches!(result, Some(Ok(Some(MarkdownKeyIntent::Handled)))) {
                    return Err(format!("Markdown SelectAll rejected: {result:?}"));
                }
                self.document_smoke = Smoke::Selected;
            }
            Smoke::Selected => {
                let (width, copied, rows) = self.verify_document_smoke()?;
                let window = self
                    .window
                    .as_ref()
                    .ok_or("Markdown native window unavailable")?;
                let size = window.inner_size().to_logical::<f64>(window.scale_factor());
                let next_width = if size.width > 1000.0 { 912.0 } else { 1160.0 };
                let _ = window.request_inner_size(LogicalSize::new(next_width, size.height));
                eprintln!(
                    "UI-F02 selected submitted: frames={} selected_rows={rows} copy_bytes={} width={width}",
                    self.rendered_frames,
                    copied.len()
                );
                self.document_smoke = Smoke::Reflow { width, copied };
            }
            Smoke::Reflow { width, copied } => {
                let (next, current, rows) = self.verify_document_smoke()?;
                if next == width {
                    self.document_smoke = Smoke::Reflow { width, copied };
                    return Ok(false);
                }
                if current != copied {
                    return Err("Markdown reflow changed rendered Copy content".into());
                }
                eprintln!(
                    "UI-F02 selection reflow submitted: frames={} selected_rows={rows} copy_bytes={} old_width={width} new_width={next} clipboard_io=false",
                    self.rendered_frames,
                    current.len()
                );
                let item =
                    ensure_documents_tab(&mut self.demo.workspace).ok_or("document tab missing")?;
                self.demo
                    .page_scroll
                    .entry(item)
                    .or_default()
                    .scroll_by(-1_000_000.0);
                let window = self
                    .window
                    .as_ref()
                    .ok_or("Markdown native window unavailable")?;
                let _ = window.request_inner_size(LogicalSize::new(next as f64, 240.0));
                self.document_smoke = Smoke::DragResize { item };
            }
            Smoke::DragResize { item } => {
                if self.window.as_ref().is_none_or(|window| {
                    window
                        .inner_size()
                        .to_logical::<f64>(window.scale_factor())
                        .height
                        > 260.0
                }) {
                    self.document_smoke = Smoke::DragResize { item };
                    return Ok(false);
                }
                let scope = self.scopes.markdown;
                let theme = self.theme;
                let (initial, held) = self
                    .with_document_input(STANDALONE_WINDOW, |interaction, surface, input| {
                        let root = interaction.view(surface.document, surface.range.clone(), theme);
                        let first = surface
                            .document
                            .block_id_at(0)
                            .ok_or("first block absent")?;
                        let id = surface
                            .document
                            .paragraph_id(scope, first)
                            .map_err(|error| error.to_string())?;
                        let node = surface.layout.node(id).ok_or("heading geometry absent")?;
                        let Some(ElementKind::TextInput { snapshot, .. }) =
                            root.find(id).map(|element| &element.kind)
                        else {
                            return Err("heading snapshot absent".to_owned());
                        };
                        let clip = snapshot
                            .viewport(node.rect, node.inherited_clip, Point::default())
                            .clip;
                        if clip.width <= 0.0 || clip.height <= 0.0 {
                            return Err("drag start is offscreen".into());
                        }
                        let point =
                            Point::new(clip.x + clip.width * 0.5, clip.y + clip.height * 0.5);
                        let node = surface
                            .layout
                            .node(interaction.target().element)
                            .ok_or("Document absent")?;
                        let visible = node
                            .rect
                            .intersection(node.inherited_clip)
                            .ok_or("Document clip absent")?;
                        if node.rect.height <= visible.height {
                            return Err("drag fixture has no offscreen text".into());
                        }
                        if interaction
                            .press(&surface, point, false, input)
                            .map_err(|error| error.to_string())?
                            != MarkdownPointerResult::Selection
                        {
                            return Err("native drag press rejected".into());
                        }
                        let held = Point::new(point.x, visible.y + visible.height + 80.0);
                        interaction
                            .pointer_move(&surface, held, input)
                            .map_err(|error| error.to_string())?;
                        let initial = interaction
                            .accessibility(&surface)
                            .map_err(|error| error.to_string())?
                            .selection
                            .ok_or("initial drag range absent")?;
                        Ok::<_, String>((initial, held))
                    })
                    .ok_or("drag owner unavailable")??;
                let now = Instant::now();
                if self
                    .tick_window_document_drag(STANDALONE_WINDOW, now)
                    .is_none()
                {
                    return Err("native edge timer did not arm".into());
                }
                let offset = self.demo.page_scroll[&item].offset_y();
                self.document_smoke = Smoke::Drag {
                    item,
                    now,
                    ticks: 0,
                    initial,
                    held,
                    offset,
                };
            }
            Smoke::Drag {
                item,
                now,
                ticks,
                initial,
                held,
                offset,
            } => {
                if ticks < 12 {
                    let now = now + Duration::from_millis(20);
                    self.tick_window_document_drag(STANDALONE_WINDOW, now);
                    self.document_smoke = Smoke::Drag {
                        item,
                        now,
                        ticks: ticks + 1,
                        initial,
                        held,
                        offset,
                    };
                } else {
                    let copied = self
                        .with_document_input(STANDALONE_WINDOW, |interaction, surface, input| {
                            let current = interaction
                                .accessibility(&surface)
                                .map_err(|error| error.to_string())?
                                .selection
                                .ok_or("submitted drag range absent")?;
                            if current.anchor != initial.anchor
                                || current.focus == initial.focus
                                || input.focus() != Some(interaction.target())
                            {
                                return Err(
                                    "stationary drag did not retain its anchor and advance focus"
                                        .to_owned(),
                                );
                            }
                            let copied = surface
                                .document
                                .copy_selection(
                                    surface.window,
                                    interaction.selection().ok_or("drag selection absent")?,
                                    64 * 1024,
                                )
                                .map_err(|error| error.to_string())?;
                            if interaction
                                .release(&surface, held, PointerButton::Primary, input)
                                .map_err(|error| error.to_string())?
                                != MarkdownPointerResult::Selection
                                || interaction.drag_scroll_deadline().is_some()
                                || input.capture().is_some()
                            {
                                return Err("native drag release retained its timer/capture".into());
                            }
                            Ok::<_, String>(copied)
                        })
                        .ok_or("submitted drag owner absent")??;
                    let next = self.demo.page_scroll[&item].offset_y();
                    if next <= offset {
                        return Err("native drag timer did not consume retained scroll".into());
                    }
                    eprintln!(
                        "UI-F02 drag scroll submitted: frames={} ticks={ticks} offset_before={offset} offset_after={next} copy_bytes={} anchor_retained=true focus_advanced=true released=true supplied_time=true pointer_io=false clipboard_io=false",
                        self.rendered_frames,
                        copied.len()
                    );
                    self.document_smoke = Smoke::Edge(edge::Phase::Position { item, now });
                }
            }
            Smoke::Edge(phase) => return self.advance_document_edge_smoke(phase),
        }
        Ok(false)
    }

    fn move_document_page_smoke(&mut self, key: KeyStroke, shift: bool) -> Result<(), String> {
        let target = self.scopes.markdown.element(0);
        let rect = self
            .with_document_input(STANDALONE_WINDOW, |interaction, surface, input| {
                if interaction
                    .key(
                        &surface,
                        KeyChord::new(
                            key,
                            KeyModifiers {
                                shift,
                                ..Default::default()
                            },
                        ),
                        64 * 1024,
                        input,
                    )
                    .map_err(|error| error.to_string())?
                    != Some(MarkdownKeyIntent::RevealSelection)
                {
                    return Err("Markdown viewport page navigation was not accepted".to_owned());
                }
                interaction
                    .selection_focus_rect(&surface)
                    .map_err(|error| error.to_string())?
                    .ok_or("Markdown page focus geometry unavailable".to_owned())
            })
            .ok_or("Markdown page navigation projection unavailable")??;
        self.reveal_window_rect(STANDALONE_WINDOW, target, rect);
        Ok(())
    }

    fn verify_document_page_smoke(&mut self, returned: bool) -> Result<(), String> {
        self.with_document_input(STANDALONE_WINDOW, |interaction, surface, input| {
            let expected = if returned {
                let first = surface
                    .document
                    .block_id_at(0)
                    .ok_or("first block missing")?;
                let first = surface
                    .document
                    .anchor(first, 0)
                    .map_err(|error| error.to_string())?;
                surface
                    .document
                    .selection(surface.window, first, first)
                    .map_err(|error| error.to_string())?
            } else {
                surface
                    .document
                    .select_all(surface.window)
                    .map_err(|error| error.to_string())?
                    .ok_or("Markdown page range missing")?
            };
            if interaction.selection() != Some(expected)
                || input.focus() != Some(interaction.target())
            {
                return Err("Markdown page selection or root focus mismatch".to_owned());
            }
            Ok(())
        })
        .ok_or("Markdown page verification projection unavailable")??;
        let bytes = if returned {
            0
        } else {
            self.verify_document_smoke()?.1.len()
        };
        eprintln!(
            "UI-F02 viewport page submitted: frames={} direction={} copy_bytes={bytes} root_focus=true clipboard_io=false",
            self.rendered_frames,
            if returned { "up" } else { "down" }
        );
        Ok(())
    }

    fn verify_document_accessibility_smoke(&mut self) -> Result<(), String> {
        let theme = self.theme;
        let frames = self.rendered_frames;
        self.with_document_input(STANDALONE_WINDOW, |interaction, surface, input| {
            let root = interaction.view(surface.document, surface.range.clone(), theme);
            let projection = interaction.accessibility(&surface).map_err(|error| error.to_string())?;
            let expected = projection.selection.ok_or("Markdown accessible selection is absent")?;
            let mut semantics = AccessibilityTree::default();
            let mut update = semantics.build(&root, surface.layout, input.focus().map(|target| target.element), "Markdown smoke");
            semantics.enrich_read_only_document(&mut update, projection).map_err(|error| error.to_string())?;
            let root_id = semantics.node_for_element(interaction.target().element).ok_or("Markdown accessible root is absent")?;
            let node = &update.nodes.iter().find(|(id, _)| *id == root_id).ok_or("Markdown native Document is absent")?.1;
            let selection = node.text_selection().ok_or("Markdown native range is absent")?;
            let runs = update.nodes.iter().filter(|(_, node)| node.role() == accesskit::Role::TextRun).count();
            if node.role() != accesskit::Role::Document || update.focus != root_id || runs == 0 || semantics.resolve_read_only_selection(interaction.target().element, selection) != Some(expected) {
                return Err("Markdown native range/owner round trip failed".to_owned());
            }
            eprintln!("UI-F02 document accessibility submitted: frames={frames} text_runs={runs} range_round_trip=true root_focus=true reader_io=false");
            Ok(())
        }).ok_or("Markdown accessibility owner is absent")?
    }

    fn verify_document_smoke(&mut self) -> Result<(f32, String, usize), String> {
        self.verify_document_accessibility_smoke()?;
        let theme = self.theme;
        let scope = self.scopes.markdown;
        self.with_document_input(STANDALONE_WINDOW, |interaction, surface, input| {
            let command = KeyModifiers {
                control: true,
                ..Default::default()
            };
            let intent = interaction
                .key(
                    &surface,
                    KeyChord::character('c', command),
                    64 * 1024,
                    input,
                )
                .map_err(|error| error.to_string())?;
            let Some(MarkdownKeyIntent::Copy(copied)) = intent else {
                return Err("Markdown Copy intent unavailable".into());
            };
            if !copied.contains("中文、English 与 粗体、斜体、inline code")
                || !copied.contains("let seed = 42;")
                || copied.contains("**")
                || copied.contains("https://example.invalid")
            {
                return Err("Markdown Copy failed to preserve rendered mixed text".into());
            }
            let root = interaction.view(surface.document, surface.range.clone(), theme);
            let mut rows = 0;
            for index in surface.range.clone() {
                let Some(block) = surface.document.block_id_at(index) else {
                    break;
                };
                let paragraph = surface
                    .document
                    .paragraph_id(scope, block)
                    .map_err(|error| error.to_string())?;
                let node = surface
                    .layout
                    .node(paragraph)
                    .ok_or("Markdown paragraph geometry absent")?;
                let ElementKind::TextInput { snapshot, .. } = &root
                    .find(paragraph)
                    .ok_or("Markdown selected paragraph absent")?
                    .kind
                else {
                    return Err("Markdown selected text is unshaped".into());
                };
                let view = snapshot.viewport(node.rect, node.inherited_clip, Point::default());
                if !snapshot
                    .decorations(view, false, false, 1.0)
                    .selection
                    .is_empty()
                {
                    rows += 1;
                }
            }
            if rows == 0 {
                return Err("Markdown selected View has no visible selection geometry".into());
            }
            Ok((surface.layout.viewport().width, copied, rows))
        })
        .ok_or("Markdown input projection missing")?
    }

    fn verify_document_navigation_smoke(&mut self) -> Result<(), String> {
        let theme = self.theme;
        let scope = self.scopes.markdown;
        let copied = self
            .with_document_input(STANDALONE_WINDOW, |interaction, surface, input| {
                let selection = interaction
                    .selection()
                    .ok_or("Markdown navigation selection unavailable")?;
                let first_block = surface
                    .document
                    .block_id_at(0)
                    .ok_or("first block unavailable")?;
                let second_block = surface
                    .document
                    .block_id_at(1)
                    .ok_or("second block unavailable")?;
                let first_anchor = surface
                    .document
                    .anchor(first_block, 0)
                    .map_err(|error| error.to_string())?;
                let second_anchor = surface
                    .document
                    .anchor(second_block, 0)
                    .map_err(|error| error.to_string())?;
                let expected = surface
                    .document
                    .selection(surface.window, first_anchor, second_anchor)
                    .map_err(|error| error.to_string())?;
                if selection != expected || input.focus() != Some(interaction.target()) {
                    return Err("Markdown Shift+Down did not extend to the next block".into());
                }
                let root = interaction.view(surface.document, surface.range.clone(), theme);
                let first = surface
                    .document
                    .paragraph_id(scope, first_block)
                    .map_err(|error| error.to_string())?;
                let Some(ElementKind::TextInput { snapshot, .. }) =
                    root.find(first).map(|element| &element.kind)
                else {
                    return Err("Markdown navigation selected snapshot unavailable".into());
                };
                let node = surface
                    .layout
                    .node(first)
                    .ok_or("Markdown navigation layout unavailable")?;
                let viewport = snapshot.viewport(node.rect, node.inherited_clip, Point::default());
                if snapshot
                    .decorations(viewport, false, false, 1.0)
                    .selection
                    .is_empty()
                {
                    return Err("Markdown navigation selection was not visibly projected".into());
                }
                match interaction
                    .key(
                        &surface,
                        KeyChord::character(
                            'c',
                            KeyModifiers {
                                control: true,
                                ..Default::default()
                            },
                        ),
                        64 * 1024,
                        input,
                    )
                    .map_err(|error| error.to_string())?
                {
                    Some(MarkdownKeyIntent::Copy(text)) if text == "Vesta 文档预览\n\n" => {
                        Ok(text)
                    }
                    result => Err(format!("Markdown navigation Copy mismatch: {result:?}")),
                }
            })
            .ok_or("Markdown navigation projection missing")??;
        eprintln!(
            "UI-F02 visual navigation submitted: frames={} copy_bytes={} root_focus=true clipboard_io=false",
            self.rendered_frames,
            copied.len()
        );
        Ok(())
    }

    fn verify_document_link_smoke(&mut self) -> Result<(), String> {
        let scope = self.scopes.markdown;
        let target = self
            .with_document_input(STANDALONE_WINDOW, |interaction, surface, input| {
                let link = surface
                    .document
                    .links_in(surface.window, surface.range.clone())
                    .next()
                    .ok_or("Markdown link intent missing")?;
                let target = surface
                    .document
                    .link_target(scope, surface.window, link)
                    .map_err(|error| error.to_string())?;
                let before = input.focus();
                input.set_focus(Some(target));
                let activation = interaction.key(
                    &surface,
                    KeyChord::new(KeyStroke::Enter, KeyModifiers::default()),
                    64 * 1024,
                    input,
                );
                input.set_focus(before);
                if activation != Ok(Some(MarkdownKeyIntent::Activate(link))) {
                    return Err(format!("Markdown projected link rejected: {activation:?}"));
                }
                let resolved = interaction
                    .resolve_activation(&surface, link)
                    .map_err(|error| error.to_string())?
                    .ok_or("Markdown link is not visible")?;
                if !matches!(
                    vesta_gpui_winit::resolve_document_link(resolved.destination, None),
                    Ok(vesta_gpui_winit::DocumentLink::Web(_))
                ) {
                    return Err("Markdown HTTP navigation classification failed".into());
                }
                Ok(target)
            })
            .ok_or("Markdown link projection missing")??;
        if !self.ui_actions.focus_order().contains(&target.element)
            || self.semantics.node_for_element(target.element).is_none()
        {
            return Err("Markdown keyboard/platform link target missing".into());
        }
        eprintln!(
            "UI-F02 link projected: frames={} keyboard_activation=true semantic_target=true navigation_io=false",
            self.rendered_frames
        );
        Ok(())
    }
}
