use egui::epaint::ahash::{HashMap, HashMapExt};
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
    /// all the roots and their children ranges in NodeIndex space
    roots_ranges: HashMap<NodeIndex, [NodeIndex; 2]>,
    /// all the roots and their hierarchy
    roots_tree: petgraph::Graph<NodeIndex, (), Directed>,
    /// current root index in the root graph
    /// and current element index in the root's range
    position_by_root: Option<Position>,
}

impl Cursor {
    pub fn new(root: NodeIndex, g: &Graph<Node, (), Directed>) -> Self {
        let mut roots_ranges = HashMap::new();

        let first = g.node_indices().next().unwrap();
        let last = g.node_indices().last().unwrap();
        roots_ranges.insert(root, [first, last]);

        let mut roots = petgraph::Graph::new();
        roots.add_node(root);
        Self {
            roots_ranges,
            roots_tree: roots,
            position_by_root: Some((root, first)),
        }
    }

    /// Adds new root to the root graph and updates the root ranges.
    ///
    /// Given graph should already contain the root and all his children.
    pub fn add(&mut self, root: NodeIndex, g: &Graph<Node, (), Directed>) {
        self.roots_ranges.insert(
            root,
            [self.last().unwrap(), g.node_indices().last().unwrap()],
        );

        let root_idx = self.roots_tree.add_node(root);
        let parent_idx = self
            .roots_tree
            .node_indices()
            .find(|i| self.position_by_root.unwrap().0 == *self.roots_tree.node_weight(*i).unwrap())
            .unwrap();
        self.roots_tree.add_edge(parent_idx, root_idx, ());

        self.position_by_root = Some((root, self.roots_ranges[&root][0]));
    }

    pub fn set_child(&mut self, idx: NodeIndex) -> Option<NodeIndex> {
        let found_root = self.root(idx)?;
        self.position_by_root = Some((found_root, idx));
        Some(idx)
    }

    pub fn set_root(&mut self, idx: NodeIndex) -> Option<NodeIndex> {
        let found_root = self.root(idx)?;
        self.position_by_root = Some((found_root, self.roots_ranges[&idx][0]));
        Some(idx)
    }

    /// Gets the next element relative to the cursor position.
    ///
    /// If cursor element is the last one in the range, then it returns the first element in the range.
    pub fn next_child(&self) -> NodeIndex {
        let (root, idx) = self.position_by_root.unwrap();
        let range = self.roots_ranges[&root];

        match idx == range[1] {
            true => range[0],
            false => NodeIndex::new(idx.index() + 1),
        }
    }

    /// Gets the previous element relative to the cursor position.
    ///
    /// If current element is the first one in the range, then it returns the last element in the range.
    pub fn prev_child(&self) -> NodeIndex {
        let (root, idx) = self.position_by_root.unwrap();
        let range = self.roots_ranges[&root];

        match idx.index() == 0 || idx == range[0] {
            true => range[1],
            false => NodeIndex::new(idx.index() - 1),
        }
    }

    /// Gets next root index from the root tree. If current root is the last one in the root tree,
    /// it returns the first one.
    pub fn next_root(&self) -> NodeIndex {
        let curr = self.position_by_root.unwrap().0;
        let curr_rt_idx = self
            .roots_tree
            .node_indices()
            .find(|i| curr == *self.roots_tree.node_weight(*i).unwrap())
            .unwrap();

        match self
            .roots_tree
            .neighbors_directed(curr_rt_idx, Outgoing)
            .next()
        {
            Some(next_rt_idx) => *self.roots_tree.node_weight(next_rt_idx).unwrap(),
            None => *self.roots_tree.node_weight(NodeIndex::new(0)).unwrap(),
        }
    }

    /// Gets prev root index from the root graph. If current root is the first one in the root graph,
    /// it returns the last one.
    pub fn prev_root(&self) -> NodeIndex {
        let curr = self.position_by_root.unwrap().0;
        let curr_rt_idx = self
            .roots_tree
            .node_indices()
            .find(|i| curr == *self.roots_tree.node_weight(*i).unwrap())
            .unwrap();

        match self
            .roots_tree
            .neighbors_directed(curr_rt_idx, Incoming)
            .next()
        {
            Some(next_rt_idx) => *self.roots_tree.node_weight(next_rt_idx).unwrap(),
            None => self.root(self.last().unwrap()).unwrap(),
        }
    }

    /// Gets root for the provided NodeIndex.
    ///
    /// If the provided NodeIndex is a root itself then it returns the NodeIndex.
    pub fn root(&self, idx: NodeIndex) -> Option<NodeIndex> {
        if self.roots_ranges.keys().any(|root| *root == idx) {
            return Some(idx);
        }

        Some(
            *self
                .roots_ranges
                .iter()
                .find(|(_, r)| r[0] <= idx && idx <= r[1])?
                .0,
        )
    }

    /// Gets the last element by NodeIndex value from all the registered roots_ranges.
    fn last(&self) -> Option<NodeIndex> {
        self.roots_ranges.values().map(|r| *r.last().unwrap()).max()
    }
}
