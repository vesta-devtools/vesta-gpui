//! Immutable document text projected from the paint snapshot (ADR-0361).
use super::*;
use crate::TextInputSnapshot;
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

pub struct ReadOnlyText<'a> {
    pub element: ElementId,
    pub source: &'a str,
    pub snapshot: &'a TextInputSnapshot,
    pub origin: Point,
    /// None keeps the document's existing default; table cells supply tab/LF.
    pub separator_after: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadOnlyTextPosition {
    pub element: ElementId,
    pub byte: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadOnlyDocumentSelection {
    pub root: ElementId,
    pub revision: u64,
    pub anchor: ReadOnlyTextPosition,
    pub focus: ReadOnlyTextPosition,
}

pub struct ReadOnlyDocument<'a> {
    pub root: ElementId,
    pub revision: u64,
    pub paragraphs: Vec<ReadOnlyText<'a>>,
    pub selection: Option<ReadOnlyDocumentSelection>,
    pub paragraph_separator: &'a str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadOnlyTextProjectionError {
    MissingNode(ElementId),
    InvalidSource(ElementId),
    InvalidBoundary(ElementId),
    WrongDocument,
}

impl std::fmt::Display for ReadOnlyTextProjectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "read-only text projection: {self:?}")
    }
}
impl std::error::Error for ReadOnlyTextProjectionError {}

#[derive(Clone)]
pub(super) struct DocumentProjection {
    revision: u64,
    paragraphs: HashMap<ElementId, ParagraphProjection>,
    boundaries: HashMap<NodeId, (ReadOnlyTextPosition, ReadOnlyTextPosition)>,
}

#[derive(Clone)]
struct ParagraphProjection {
    source: Arc<str>,
    snapshot: TextInputSnapshot,
    layout: parley::layout::LayoutAccessibility,
    runs: HashMap<NodeId, (usize, Vec<u8>)>,
    separator: Option<NodeId>,
}

fn boundary(text: &str, byte: usize) -> bool {
    byte == text.len() || text.grapheme_indices(true).any(|(start, _)| start == byte)
}

impl AccessibilityTree {
    /// Enrich an already built native Document without a second editor or shape.
    /// Sources must belong to their supplied snapshots; run ranges are checked before Parley reads them.
    pub fn enrich_read_only_document(
        &mut self,
        update: &mut TreeUpdate,
        document: ReadOnlyDocument<'_>,
    ) -> Result<(), ReadOnlyTextProjectionError> {
        use ReadOnlyTextProjectionError as Error;
        if document.paragraph_separator.len() > usize::from(u8::MAX) {
            return Err(Error::InvalidSource(document.root));
        }
        let root_id = self
            .node_for_element(document.root)
            .ok_or(Error::MissingNode(document.root))?;
        let root_index = update
            .nodes
            .iter()
            .position(|(id, node)| *id == root_id && node.role() == Role::Document)
            .ok_or(Error::MissingNode(document.root))?;
        let mut descendants = update.nodes[root_index].1.children().to_vec();
        let mut owned = HashSet::new();
        while let Some(id) = descendants.pop() {
            if owned.insert(id)
                && let Some((_, node)) = update.nodes.iter().find(|(candidate, _)| *candidate == id)
            {
                descendants.extend(node.children());
            }
        }
        let mut elements = HashSet::new();
        for text in &document.paragraphs {
            if text
                .separator_after
                .unwrap_or(document.paragraph_separator)
                .len()
                > usize::from(u8::MAX)
            {
                return Err(Error::InvalidSource(text.element));
            }
            let id = self
                .node_for_element(text.element)
                .filter(|id| owned.contains(id))
                .ok_or(Error::MissingNode(text.element))?;
            if !elements.insert(text.element)
                || !update
                    .nodes
                    .iter()
                    .any(|(candidate, node)| *candidate == id && node.role() == Role::Label)
            {
                return Err(Error::MissingNode(text.element));
            }
            if parley::editing::Cursor::from_byte_index(
                &text.snapshot.0.layout,
                usize::MAX,
                parley::Affinity::Upstream,
            )
            .index()
                != text.source.len()
                || !text.origin.x.is_finite()
                || !text.origin.y.is_finite()
                || text
                    .snapshot
                    .0
                    .layout
                    .lines()
                    .flat_map(|line| line.runs())
                    .any(|run| {
                        text.source.get(run.text_range()).is_none()
                            || run.clusters().any(|cluster| {
                                text.source.get(cluster.text_range()).is_none()
                                    || cluster.text_range().len() > usize::from(u8::MAX)
                            })
                    })
            {
                return Err(Error::InvalidSource(text.element));
            }
        }
        if let Some(selection) = document.selection {
            if selection.root != document.root || selection.revision != document.revision {
                return Err(Error::WrongDocument);
            }
            for position in [selection.anchor, selection.focus] {
                let text = document
                    .paragraphs
                    .iter()
                    .find(|text| text.element == position.element)
                    .ok_or(Error::MissingNode(position.element))?;
                if !boundary(text.source, position.byte)
                    || text.snapshot.layout_cursor(position.byte).is_err()
                {
                    return Err(Error::InvalidBoundary(position.element));
                }
            }
        }
        let retired: HashSet<_> = self
            .read_only_documents
            .get(&document.root)
            .into_iter()
            .flat_map(|old| old.paragraphs.values())
            .flat_map(|paragraph| paragraph.runs.keys().copied().chain(paragraph.separator))
            .collect();
        let mut previous = self
            .read_only_documents
            .get(&document.root)
            .filter(|old| old.revision == document.revision)
            .cloned();
        let mut projection = DocumentProjection {
            revision: document.revision,
            paragraphs: HashMap::new(),
            boundaries: HashMap::new(),
        };
        let mut staged = TreeUpdate {
            nodes: Vec::new(),
            tree: None,
            focus: update.focus,
        };
        let mut paragraphs = document.paragraphs.into_iter().peekable();
        while let Some(text) = paragraphs.next() {
            let id = self.ids[&text.element];
            let (_, mut node) = update
                .nodes
                .iter()
                .find(|(candidate, _)| *candidate == id)
                .expect("validated paragraph")
                .clone();
            let mut paragraph = previous
                .as_mut()
                .and_then(|old| old.paragraphs.remove(&text.element))
                .filter(|old| {
                    Arc::ptr_eq(&old.snapshot.0, &text.snapshot.0)
                        && old.source.as_ref() == text.source
                })
                .unwrap_or_else(|| ParagraphProjection {
                    source: Arc::from(text.source),
                    snapshot: text.snapshot.clone(),
                    layout: Default::default(),
                    runs: HashMap::new(),
                    separator: None,
                });
            node.set_children(
                node.children()
                    .iter()
                    .copied()
                    .filter(|id| !retired.contains(id))
                    .collect::<Vec<_>>(),
            );
            paragraph.runs.clear();
            let run_start = staged.nodes.len();
            paragraph.layout.build_nodes(
                text.source,
                &text.snapshot.0.layout,
                &mut staged,
                &mut node,
                || self.allocate(),
                f64::from(text.origin.x),
                f64::from(text.origin.y),
            );
            for (run_id, run) in &staged.nodes[run_start..] {
                let position = accesskit::TextPosition {
                    node: *run_id,
                    character_index: 0,
                };
                if let Some(cursor) = parley::editing::Cursor::from_access_position(
                    &position,
                    &text.snapshot.0.layout,
                    &paragraph.layout,
                ) {
                    paragraph
                        .runs
                        .insert(*run_id, (cursor.index(), run.character_lengths().to_vec()));
                }
            }
            let separator_text = text.separator_after.unwrap_or(document.paragraph_separator);
            if let Some(next) = paragraphs.peek().filter(|_| !separator_text.is_empty()) {
                let separator = *paragraph.separator.get_or_insert_with(|| self.allocate());
                let bounds = text
                    .snapshot
                    .layout_cursor(text.source.len())
                    .map_err(|_| Error::InvalidBoundary(text.element))?
                    .bounds();
                let mut gap = Node::new(Role::TextRun);
                gap.set_value(separator_text);
                gap.set_character_lengths(vec![separator_text.len() as u8]);
                gap.set_character_positions(vec![0.0]);
                gap.set_character_widths(vec![0.0]);
                gap.set_word_lengths(vec![1]);
                gap.set_bounds(accesskit::Rect {
                    x0: f64::from(bounds.x + text.origin.x),
                    x1: f64::from(bounds.x + text.origin.x),
                    y0: f64::from(bounds.y + text.origin.y),
                    y1: f64::from(bounds.bottom() + text.origin.y),
                });
                projection.boundaries.insert(
                    separator,
                    (
                        ReadOnlyTextPosition {
                            element: text.element,
                            byte: text.source.len(),
                        },
                        ReadOnlyTextPosition {
                            element: next.element,
                            byte: 0,
                        },
                    ),
                );
                node.push_child(separator);
                staged.nodes.push((separator, gap));
            } else {
                paragraph.separator = None;
            }
            node.set_read_only();
            staged.nodes.push((id, node));
            projection.paragraphs.insert(text.element, paragraph);
        }
        let mut root = update.nodes[root_index].1.clone();
        if let Some(selection) = document.selection {
            let convert = |position: ReadOnlyTextPosition| {
                let paragraph = &projection.paragraphs[&position.element];
                parley::editing::Cursor::from_byte_index(
                    &paragraph.snapshot.0.layout,
                    position.byte,
                    parley::Affinity::Downstream,
                )
                .to_access_position(&paragraph.snapshot.0.layout, &paragraph.layout)
                .ok_or(Error::InvalidBoundary(position.element))
            };
            root.set_text_selection(accesskit::TextSelection {
                anchor: convert(selection.anchor)?,
                focus: convert(selection.focus)?,
            });
        } else {
            root.clear_text_selection();
        }
        root.set_read_only();
        let enabled = !root.is_disabled();
        if enabled {
            root.add_action(Action::SetTextSelection);
        } else {
            root.remove_action(Action::SetTextSelection);
        }
        self.remember_actions(document.root, &root);
        staged.nodes.push((root_id, root));
        update.nodes.retain(|(id, _)| !retired.contains(id));
        for (_, node) in &mut update.nodes {
            if node.children().iter().any(|id| retired.contains(id)) {
                node.set_children(
                    node.children()
                        .iter()
                        .copied()
                        .filter(|id| !retired.contains(id))
                        .collect::<Vec<_>>(),
                );
            }
        }
        for (id, node) in staged.nodes {
            if let Some((_, current)) = update
                .nodes
                .iter_mut()
                .find(|(candidate, _)| *candidate == id)
            {
                *current = node;
            } else {
                update.nodes.push((id, node));
            }
        }
        self.read_only_documents.insert(document.root, projection);
        if enabled {
            self.active_read_only_documents.insert(document.root);
        } else {
            self.active_read_only_documents.remove(&document.root);
        }
        Ok(())
    }

    /// Only current run IDs owned by this tree/document can resolve a platform range.
    pub fn resolve_read_only_selection(
        &self,
        root: ElementId,
        selection: &accesskit::TextSelection,
    ) -> Option<ReadOnlyDocumentSelection> {
        if !self.active_read_only_documents.contains(&root) || !self.live.contains(&root) {
            return None;
        }
        let projection = self.read_only_documents.get(&root)?;
        let resolve = |position: accesskit::TextPosition| {
            if let Some((previous, next)) = projection.boundaries.get(&position.node) {
                return match position.character_index {
                    0 => Some(*previous),
                    1 => Some(*next),
                    _ => None,
                };
            }
            projection
                .paragraphs
                .iter()
                .find_map(|(element, paragraph)| {
                    let (start, lengths) = paragraph.runs.get(&position.node)?;
                    let byte = start.checked_add(
                        lengths
                            .get(..position.character_index)?
                            .iter()
                            .map(|length| usize::from(*length))
                            .sum::<usize>(),
                    )?;
                    boundary(&paragraph.source, byte).then_some(ReadOnlyTextPosition {
                        element: *element,
                        byte,
                    })
                })
        };
        Some(ReadOnlyDocumentSelection {
            root,
            revision: projection.revision,
            anchor: resolve(selection.anchor)?,
            focus: resolve(selection.focus)?,
        })
    }
}

#[cfg(test)]
mod tests;
