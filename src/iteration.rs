use egui::epaint::ahash::{HashMap, HashMapExt};
use egui_graphs::Graph;
use petgraph::{stable_graph::NodeIndex, Directed};

use crate::node::Node;

pub type Cursor = (NodeIndex, NodeIndex);

#[derive(Default)]
pub struct StateIteration {
    /// all the roots and their children ranges in NodeIndex space
    roots_ranges: HashMap<NodeIndex, [NodeIndex; 2]>,
    /// all the roots and their hierarchy
    roots_tree: petgraph::Graph<NodeIndex, (), Directed>,
    /// current root index in the root graph
    /// and current element index in the root's range
    cursor: Option<Cursor>,
}

impl StateIteration {
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
            cursor: Some((root, first)),
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
            .find(|i| self.cursor.unwrap().0 == *self.roots_tree.node_weight(*i).unwrap())
            .unwrap();
        self.roots_tree.add_edge(parent_idx, root_idx, ());

        self.cursor = Some((root, self.roots_ranges[&root][0]));
    }

    pub fn set_cursor(&mut self, curr: NodeIndex) {
        self.cursor = Some((self.root(curr).unwrap(), curr));
    }

    /// Gets the next element relative to the cursor.
    ///
    /// If cursor element is the last one in the range, then it returns the first element in the range.
    pub fn next(&mut self) -> NodeIndex {
        let (root, idx) = self.cursor.unwrap();
        let range = self.roots_ranges.get(&root).unwrap();

        let next = match idx == range[1] {
            true => range[0],
            false => NodeIndex::new(idx.index() + 1),
        };

        self.cursor.as_mut().unwrap().1 = next;
        next
    }

    /// Gets the previous element relative to the cursor.
    ///
    /// If current element is the first one in the range, then it returns the last element in the range.
    pub fn prev(&mut self) -> NodeIndex {
        let (root, idx) = self.cursor.unwrap();
        let range = self.roots_ranges.get(&root).unwrap();

        let new_idx = match idx.index() == 0 || idx == range[0] {
            true => range[1],
            false => NodeIndex::new(idx.index() - 1),
        };

        self.cursor.as_mut().unwrap().1 = new_idx;
        new_idx
    }

    // /// Gets next root index from the root tree. If current root is the last one in the root tree,
    // /// it returns the first one.
    // pub fn next_root(&self) -> NodeIndex {
    //     let root = self.current.unwrap().0;
    //     let root_idx = self
    //         .roots_tree
    //         .node_indices()
    //         .find(|i| root == *i)
    //         .unwrap();

    //     let Some(next_root_idx) = self.roots_tree.neighbors(root_idx).next() {

    //     };

    //     *self.roots_tree.node_weight(next_root_idx).unwrap()
    // }

    // /// Gets prev root index from the root graph. If current root is the first one in the root graph,
    // /// it returns the last one.
    // pub fn prev_root(&self) -> NodeIndex {
    //     let root = self.current.unwrap().0;
    //     let root_idx = self
    //         .roots_tree
    //         .node_indices()
    //         .find(|i| root == *i)
    //         .unwrap();

    //     let Some(prev_root_idx) = self.roots_tree.neighbors(root_idx).next() {

    //     };

    //     *self.roots_tree.node_weight(prev_root_idx).unwrap()
    // }

    /// Gets root for the provided index from range
    fn root(&self, idx: NodeIndex) -> Option<NodeIndex> {
        Some(
            *self
                .roots_ranges
                .iter()
                .find(|(_, range)| range[0] <= idx && idx <= range[1])?
                .0,
        )
    }

    /// Gets the last element by NodeIndex value from all the registered roots_ranges.
    fn last(&self) -> Option<NodeIndex> {
        self.roots_ranges.values().map(|r| *r.last().unwrap()).max()
    }
}
