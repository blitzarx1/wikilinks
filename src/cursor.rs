use std::collections::{HashMap, HashSet};

use egui_graphs::Graph;
use petgraph::{
    stable_graph::NodeIndex,
    Directed,
    Direction::{Incoming, Outgoing},
};

use crate::node::Node;

pub type Position = (NodeIndex, NodeIndex);

#[derive(Default)]
pub struct Cursor {
    /// All the roots and their children. Root itself is included in children. Children are sorted.
    elements_by_root: HashMap<NodeIndex, Vec<NodeIndex>>,

    /// All the elements and their roots. Root can be element itself.
    roots_by_element: HashMap<NodeIndex, HashSet<NodeIndex>>,

    /// Contains roots relations;
    roots_tree: petgraph::Graph<NodeIndex, (), Directed>,

    /// current root index in the root graph
    /// and current element index in the root's range
    position: Position,
}

impl Cursor {
    pub fn new(root: NodeIndex, g: &Graph<Node, (), Directed>) -> Self {
        let mut elements_by_root = HashMap::new();
        let elements = get_children_unique_inclusive_sorted(root, g);

        let mut roots = petgraph::Graph::new();
        roots.add_node(root);

        let roots_by_element = elements
            .iter()
            .map(|idx| {
                let mut val = HashSet::new();
                val.insert(root);
                (*idx, val)
            })
            .collect::<HashMap<NodeIndex, HashSet<NodeIndex>>>();

        elements_by_root.insert(root, elements);

        Self {
            elements_by_root,
            roots_by_element,
            roots_tree: roots,
            position: (root, root),
        }
    }

    pub fn position(&self) -> Position {
        self.position
    }

    /// Updates cursor with new roots and elements.
    ///
    /// Provided graph should already contain the root and all his children.
    pub fn update(&mut self, root: NodeIndex, g: &Graph<Node, (), Directed>) {
        let elements = get_children_unique_inclusive_sorted(root, g);
        elements.iter().for_each(|idx| {
            if let Some(val) = self.roots_by_element.get_mut(idx) {
                val.insert(root);
            } else {
                let mut val = HashSet::new();
                val.insert(root);
                self.roots_by_element.insert(*idx, val);
            }
        });
        self.elements_by_root.insert(root, elements);

        self.add_root_to_tree(root);

        self.position = (root, root);
    }

    /// Gets the next element relative to the cursor position.
    ///
    /// If cursor element is the last one in the range, then it returns the first element in the range.
    ///
    /// This call also moves cursor position.
    pub fn next_child(&mut self) -> NodeIndex {
        let (root, idx) = self.position;

        let next = match self.elements_by_root[&root]
            .iter()
            .skip_while(|el| **el != idx)
            .skip(1)
            .next()
        {
            Some(item) => *item,
            None => *self.elements_by_root[&root].first().unwrap(),
        };

        self.position = (root, next);
        next
    }

    /// Gets the previous element relative to the cursor position.
    ///
    /// If current element is the first one in the range, then it returns the last element in the range.
    ///
    /// This call also moves cursor position.
    pub fn prev_child(&mut self) -> NodeIndex {
        let (root, idx) = self.position;
        let prev = match self.elements_by_root[&root]
            .iter()
            .rev()
            .skip_while(|el| **el != idx)
            .skip(1)
            .next()
        {
            Some(item) => *item,
            None => *self.elements_by_root[&root].last().unwrap(),
        };

        self.position = (root, prev);
        prev
    }

    /// Gets next root index from the root tree.
    /// This call also moves cursor position.
    pub fn next_root(&mut self) -> Option<NodeIndex> {
        let curr = self.position.0;
        let curr_rt_idx = self
            .roots_tree
            .node_indices()
            .find(|i| curr == *self.roots_tree.node_weight(*i).unwrap())
            .unwrap();

        let next_idx = self
            .roots_tree
            .neighbors_directed(curr_rt_idx, Outgoing)
            .next()?;
        let next = *self.roots_tree.node_weight(next_idx).unwrap();

        self.position = (next, *self.elements_by_root[&next].first().unwrap());
        Some(next)
    }

    /// Gets prev root index from the root graph.
    /// This call also moves cursor position.
    pub fn prev_root(&mut self) -> Option<NodeIndex> {
        let curr = self.position.0;
        let curr_rt_idx = self
            .roots_tree
            .node_indices()
            .find(|i| curr == *self.roots_tree.node_weight(*i).unwrap())
            .unwrap();

        let next = self
            .roots_tree
            .neighbors_directed(curr_rt_idx, Incoming)
            .next()?;

        self.position = (next, *self.elements_by_root[&next].first().unwrap());
        Some(next)
    }

    /// Gets all the roots for the provided element.
    pub fn roots(&self, idx: NodeIndex) -> Option<Vec<NodeIndex>> {
        self.roots_by_element
            .get(&idx)
            .map(|r| r.iter().cloned().collect())
    }

    /// Returns root for a node and in case multiple roots found it picks
    /// current root or returns the first one
    pub fn sticky_root(&self, idx: NodeIndex) -> Option<NodeIndex> {
        let roots = self.roots(idx)?;

        if roots.len() == 1 {
            return Some(roots[0]);
        }

        let (root, _) = self.position;

        if roots.contains(&root) {
            return Some(root);
        }

        Some(roots[0])
    }

    /// Adds root node to the root node tree.
    fn add_root_to_tree(&mut self, root: NodeIndex) {
        let root_idx = self.roots_tree.add_node(root);
        let parent_idx = self
            .roots_tree
            .node_indices()
            .find(|i| self.position.0 == *self.roots_tree.node_weight(*i).unwrap())
            .unwrap();
        self.roots_tree.add_edge(parent_idx, root_idx, ());
    }
}

fn get_children_unique_inclusive_sorted(
    root: NodeIndex,
    g: &Graph<Node, (), Directed>,
) -> Vec<NodeIndex> {
    let mut children = g.neighbors_directed(root, Outgoing).collect::<Vec<_>>();
    children.push(root);
    children.sort();
    children.dedup();
    children
}
