//! Command-oriented history for the editable Canvas surface.
//!
//! This intentionally keeps the domain document as the source of truth: the
//! domain's clone implementations also retain fields unknown to this crate.

use tachylyte_structured::{CanvasDocument, Edge, Error, History, Node, Result};

/// A Canvas document together with its undo/redo history.
#[derive(Debug, Default)]
pub(crate) struct CanvasHistory {
    document: CanvasDocument,
    history: History,
    undo_depth: usize,
    redo_depth: usize,
}

impl CanvasHistory {
    pub(crate) fn new(document: CanvasDocument) -> Self {
        Self {
            document,
            ..Self::default()
        }
    }

    pub(crate) fn document(&self) -> &CanvasDocument {
        &self.document
    }
    pub(crate) fn can_undo(&self) -> bool {
        self.undo_depth != 0
    }
    pub(crate) fn can_redo(&self) -> bool {
        self.redo_depth != 0
    }

    /// Apply one atomic operation. Failed operations do not affect history.
    pub(crate) fn execute<F>(&mut self, operation: F) -> Result<()>
    where
        F: FnOnce(&mut CanvasDocument) -> Result<()>,
    {
        self.history.execute(&mut self.document, operation)?;
        self.undo_depth += 1;
        self.redo_depth = 0;
        Ok(())
    }

    pub(crate) fn undo(&mut self) -> bool {
        let changed = self.history.undo(&mut self.document);
        if changed {
            self.undo_depth -= 1;
            self.redo_depth += 1;
        }
        changed
    }

    pub(crate) fn redo(&mut self) -> bool {
        let changed = self.history.redo(&mut self.document);
        if changed {
            self.redo_depth -= 1;
            self.undo_depth += 1;
        }
        changed
    }

    pub(crate) fn create_node(&mut self, node: Node) -> Result<()> {
        self.execute(move |doc| {
            node.validate()?;
            if doc.node(&node.id).is_some() {
                return Err(Error::Invalid("duplicate node id".into()));
            }
            doc.nodes.push(node);
            Ok(())
        })
    }

    /// Delete a node and every edge incident to it.
    pub(crate) fn delete_node(&mut self, id: &str) -> Result<()> {
        let id = id.to_owned();
        self.execute(move |doc| {
            let before = doc.nodes.len();
            doc.nodes.retain(|node| node.id != id);
            if before == doc.nodes.len() {
                return Err(Error::Invalid("unknown node".into()));
            }
            doc.edges
                .retain(|edge| edge.from_node != id && edge.to_node != id);
            Ok(())
        })
    }

    /// Duplicate a node using a stable `-copy`, `-copy-2`, ... identifier.
    /// The complete domain node is cloned, including unknown fields.
    pub(crate) fn duplicate_node(&mut self, id: &str) -> Result<Node> {
        let source = self
            .document
            .node(id)
            .cloned()
            .ok_or_else(|| Error::Invalid("unknown node".into()))?;
        let mut copy = source.clone();
        let mut suffix = 1usize;
        loop {
            let candidate = if suffix == 1 {
                format!("{}-copy", id)
            } else {
                format!("{}-copy-{suffix}", id)
            };
            if self.document.node(&candidate).is_none() {
                copy.id = candidate;
                break;
            }
            suffix += 1;
        }
        self.create_node(copy.clone())?;
        Ok(copy)
    }

    #[allow(dead_code)]
    pub(crate) fn connect_edge(&mut self, edge: Edge) -> Result<()> {
        self.execute(move |doc| doc.connect(edge))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn node(id: &str) -> Node {
        Node {
            id: id.into(),
            kind: "text".into(),
            x: 0.,
            y: 0.,
            width: 100.,
            height: 50.,
            text: None,
            file: None,
            url: None,
            color: None,
            extra: [("future".into(), json!(true))].into(),
        }
    }

    #[test]
    fn crud_is_atomic_and_preserves_unknown_fields() {
        let mut h = CanvasHistory::default();
        h.create_node(node("a")).unwrap();
        let copy = h.duplicate_node("a").unwrap();
        assert_eq!(copy.extra["future"], json!(true));
        h.connect_edge(Edge {
            id: "e".into(),
            from_node: "a".into(),
            to_node: copy.id.clone(),
            ..Default::default()
        })
        .unwrap();
        h.delete_node("a").unwrap();
        assert!(h.document().edges.is_empty());
        assert!(h.undo());
        assert!(h.document().node("a").is_some());
        assert!(h.redo());
        assert!(!h.can_redo());
    }

    #[test]
    fn failed_command_does_not_create_history() {
        let mut h = CanvasHistory::default();
        assert!(h.delete_node("missing").is_err());
        assert!(!h.can_undo());
    }
}
