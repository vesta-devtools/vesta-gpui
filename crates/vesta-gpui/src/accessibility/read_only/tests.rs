use super::*;
use crate::{
    Axis, Dimension, ElementKey, FontRole, LayoutEngine, Style, TextInputContext, Viewport,
};

#[test]
fn document_runs_reject_foreign_stale_and_invalid_ranges_without_reshaping() {
    let mut context = TextInputContext::default();
    assert!(context.register_font(Arc::new(
        include_bytes!("../../../tests/fixtures/MaterialIcons-Regular.ttf").to_vec()
    )));
    context.set_font_stack(FontRole::Ui, "Material Icons");
    let source = "Ae\u{301}中文😀 first line wraps";
    let second = "末尾\n";
    let shape = |context: &mut TextInputContext, width| {
        context
            .shape_styled_text(source, 13.0, FontRole::Ui, &[], width)
            .expect("first shape")
    };
    let first = shape(&mut context, 70.0);
    let last = context
        .shape_styled_text(second, 13.0, FontRole::Ui, &[], 70.0)
        .expect("second shape");
    let root = ElementId::new(80);
    let a = ElementId::new(81);
    let b = ElementId::new(82);
    let tree = |snapshot: &TextInputSnapshot| {
        Element::container(root, ElementKey::new("document"))
            .semantic(Semantic::new(SemanticRole::Document, "Document").read_only(true))
            .styled(
                Style::default()
                    .axis(Axis::Column)
                    .size(Dimension::Px(100.0), Dimension::Px(200.0))
                    .focusable(true),
            )
            .child(
                Element::text_input(a, ElementKey::new("a"), snapshot.clone(), false)
                    .semantic(Semantic::new(SemanticRole::Label, source))
                    .styled(Style::default().size(Dimension::Px(100.0), Dimension::Px(100.0))),
            )
            .child(
                Element::text_input(b, ElementKey::new("b"), last.clone(), false)
                    .semantic(Semantic::new(SemanticRole::Label, second))
                    .styled(Style::default().size(Dimension::Px(100.0), Dimension::Px(100.0))),
            )
    };
    let initial = tree(&first);
    let layout = LayoutEngine
        .compute(&initial, Viewport::new(100.0, 50.0, 1.25))
        .expect("clipped document layout");
    let mut semantics = AccessibilityTree::default();
    let mut update = semantics.build(&initial, &layout, Some(root), "Test");
    assert!(
        semantics.node_for_element(b).is_some(),
        "the bounded document keeps its clipped paragraphs semantic"
    );
    let selection = ReadOnlyDocumentSelection {
        root,
        revision: 4,
        anchor: ReadOnlyTextPosition {
            element: a,
            byte: 0,
        },
        focus: ReadOnlyTextPosition {
            element: b,
            byte: second.len(),
        },
    };
    let projection = |snapshot, revision, selection| ReadOnlyDocument {
        root,
        revision,
        paragraphs: vec![
            ReadOnlyText {
                element: a,
                source,
                snapshot,
                origin: Point::new(8.0, 0.0),
                separator_after: None,
            },
            ReadOnlyText {
                element: b,
                source: second,
                snapshot: &last,
                origin: Point::new(8.0, 100.0),
                separator_after: None,
            },
        ],
        selection,
        paragraph_separator: "\n\n",
    };
    semantics
        .enrich_read_only_document(&mut update, projection(&first, 4, Some(selection)))
        .expect("runs and selection");
    let node_id = semantics.node_for_element(root).expect("document root");
    assert_eq!(update.focus, node_id);
    let node = &update
        .nodes
        .iter()
        .find(|(id, _)| *id == node_id)
        .expect("native document")
        .1;
    assert_eq!(node.role(), Role::Document);
    assert!(node.supports_action(Action::SetTextSelection));
    assert!(!node.supports_action(Action::SetValue));
    let route = crate::ActionRoute::<()>::build(&initial, []).expect("document route");
    let scope = crate::FocusScope::new(crate::UiWindowId(1), &route, None);
    assert!(
        scope
            .accessible_target(&semantics, node_id, Action::SetTextSelection)
            .is_some(),
        "document enrichment must publish its newly available selection action"
    );
    assert!(
        scope
            .accessible_target(&semantics, node_id, Action::SetValue)
            .is_none()
    );
    let platform = *node.text_selection().expect("cross-block range");
    let consumer = accesskit_consumer::Tree::new(update.clone(), true);
    let native_document = consumer
        .state()
        .node_by_id(node_id)
        .expect("consumer Document");
    assert!(native_document.supports_text_ranges());
    assert_eq!(
        native_document
            .text_selection()
            .expect("consumer selected range")
            .text(),
        format!("{source}\n\n{second}"),
        "the native text contract must preserve the owner's paragraph separator"
    );
    assert_eq!(
        semantics.resolve_read_only_selection(root, &platform),
        Some(selection)
    );
    for separator in ["\t", "\n", ""] {
        let mut overrides = AccessibilityTree::default();
        let mut native = overrides.build(&initial, &layout, Some(root), "Cell boundaries");
        let mut candidate = projection(&first, 4, Some(selection));
        candidate.paragraphs[0].separator_after = Some(separator);
        overrides
            .enrich_read_only_document(&mut native, candidate)
            .expect("cell separator override");
        let owner = overrides.node_for_element(root).expect("document");
        let consumer = accesskit_consumer::Tree::new(native.clone(), true);
        assert_eq!(
            consumer
                .state()
                .node_by_id(owner)
                .expect("document")
                .document_range()
                .text(),
            format!("{source}{separator}{second}")
        );
        let before = native.nodes.len();
        let excessive = "x".repeat(256);
        let mut invalid = projection(&first, 4, Some(selection));
        invalid.paragraphs[0].separator_after = Some(&excessive);
        assert_eq!(
            overrides.enrich_read_only_document(&mut native, invalid),
            Err(ReadOnlyTextProjectionError::InvalidSource(a))
        );
        assert_eq!(native.nodes.len(), before);
    }
    let mut invalid = platform;
    invalid.focus.character_index = usize::MAX;
    assert!(
        semantics
            .resolve_read_only_selection(root, &invalid)
            .is_none()
    );
    let mut other = AccessibilityTree::default();
    let mut other_update = other.build(&initial, &layout, Some(root), "Other window");
    other
        .enrich_read_only_document(&mut other_update, projection(&first, 4, Some(selection)))
        .expect("independent active tree");
    assert!(other.resolve_read_only_selection(root, &platform).is_none());
    let old_runs = semantics.read_only_documents[&root].paragraphs[&a]
        .runs
        .keys()
        .copied()
        .collect::<HashSet<_>>();
    let mut next = semantics.build(&initial, &layout, Some(root), "Test");
    assert!(
        semantics
            .resolve_read_only_selection(root, &platform)
            .is_none(),
        "a new pass must be enriched before accepting platform ranges"
    );
    semantics
        .enrich_read_only_document(&mut next, projection(&first, 4, Some(selection)))
        .expect("unchanged layout");
    assert_eq!(
        old_runs,
        semantics.read_only_documents[&root].paragraphs[&a]
            .runs
            .keys()
            .copied()
            .collect()
    );
    semantics
        .enrich_read_only_document(&mut next, projection(&first, 4, Some(selection)))
        .expect("repeat enrichment");
    let a_node = semantics.node_for_element(a).expect("paragraph");
    let children = next
        .nodes
        .iter()
        .find(|(id, _)| *id == a_node)
        .expect("paragraph node")
        .1
        .children();
    assert_eq!(
        children.len(),
        children.iter().collect::<HashSet<_>>().len(),
        "repeat enrichment cannot attach duplicate text runs"
    );
    let mut invalid_source = projection(&first, 4, Some(selection));
    invalid_source.paragraphs[0].source = "x";
    let before_nodes = next.nodes.len();
    assert_eq!(
        semantics.enrich_read_only_document(&mut next, invalid_source),
        Err(ReadOnlyTextProjectionError::InvalidSource(a))
    );
    assert_eq!(next.nodes.len(), before_nodes);
    assert_eq!(
        semantics.resolve_read_only_selection(root, &platform),
        Some(selection),
        "a failed candidate preserves the last published projection"
    );
    let mut backwards = selection;
    std::mem::swap(&mut backwards.anchor, &mut backwards.focus);
    let mut backwards_update = semantics.build(&initial, &layout, Some(root), "Test");
    semantics
        .enrich_read_only_document(
            &mut backwards_update,
            projection(&first, 4, Some(backwards)),
        )
        .expect("reverse selection");
    let backwards_platform = backwards_update
        .nodes
        .iter()
        .find(|(id, _)| *id == node_id)
        .expect("root")
        .1
        .text_selection()
        .expect("reverse range");
    assert_eq!(
        semantics.resolve_read_only_selection(root, backwards_platform),
        Some(backwards)
    );
    let mut bad = selection;
    bad.anchor.byte = 2;
    let mut rejected = semantics.build(&initial, &layout, Some(root), "Test");
    let before = rejected.nodes.len();
    assert_eq!(
        semantics.enrich_read_only_document(&mut rejected, projection(&first, 4, Some(bad))),
        Err(ReadOnlyTextProjectionError::InvalidBoundary(a))
    );
    assert_eq!(
        rejected.nodes.len(),
        before,
        "validation cannot publish half a document"
    );
    let mut disabled = initial.clone();
    disabled.style.action_boundary = true;
    let mut disabled_update = semantics.build(&disabled, &layout, None, "Disabled document");
    semantics
        .enrich_read_only_document(&mut disabled_update, projection(&first, 4, Some(selection)))
        .expect("disabled text remains readable");
    let disabled_node = &disabled_update
        .nodes
        .iter()
        .find(|(id, _)| *id == node_id)
        .expect("disabled document")
        .1;
    assert!(disabled_node.is_disabled());
    assert!(!disabled_node.supports_action(Action::SetTextSelection));
    assert!(disabled_node.text_selection().is_some());
    assert!(
        semantics
            .resolve_read_only_selection(root, &platform)
            .is_none()
    );
    let narrow = shape(&mut context, 35.0);
    let rebuilt = tree(&narrow);
    let rebuilt_layout = LayoutEngine
        .compute(&rebuilt, Viewport::new(100.0, 50.0, 1.25))
        .expect("reflow layout");
    let mut rebuilt_update = semantics.build(&rebuilt, &rebuilt_layout, Some(root), "Test");
    semantics
        .enrich_read_only_document(&mut rebuilt_update, projection(&narrow, 4, Some(selection)))
        .expect("reflow projection");
    assert!(
        semantics
            .resolve_read_only_selection(root, &platform)
            .is_none(),
        "retired shape run IDs cannot select the new layout"
    );
    semantics
        .enrich_read_only_document(&mut rebuilt_update, projection(&first, 4, Some(selection)))
        .expect("replace a shape in the same update");
    let replaced_consumer = accesskit_consumer::Tree::new(rebuilt_update.clone(), true);
    assert_eq!(
        replaced_consumer
            .state()
            .node_by_id(node_id)
            .expect("replaced document")
            .document_range()
            .text(),
        format!("{source}\n\n{second}"),
        "repeated enrichment must detach every retired run and separator"
    );
    let fresh = *rebuilt_update
        .nodes
        .iter()
        .find(|(id, _)| *id == node_id)
        .expect("new root")
        .1
        .text_selection()
        .expect("new selection");
    let mut replaced = semantics.build(&rebuilt, &rebuilt_layout, Some(root), "Test");
    semantics
        .enrich_read_only_document(&mut replaced, projection(&narrow, 5, None))
        .expect("new revision");
    assert!(
        semantics
            .resolve_read_only_selection(root, &fresh)
            .is_none(),
        "a replaced source revision retires all old run positions"
    );
    let empty = Element::container(ElementId::new(90), ElementKey::new("other page"));
    let empty_layout = LayoutEngine
        .compute(&empty, Viewport::new(100.0, 50.0, 1.25))
        .expect("another page");
    semantics.build(&empty, &empty_layout, None, "Test");
    assert!(semantics.read_only_documents.is_empty());
    assert!(
        semantics
            .resolve_read_only_selection(root, &fresh)
            .is_none()
    );
}
