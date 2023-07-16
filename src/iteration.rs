use egui::epaint::ahash::{HashMap, HashMapExt};
use egui_graphs::Graph;
use petgraph::{stable_graph::NodeIndex, Directed};

use crate::node::Node;

#[derive(Default)]
pub struct StateIteration {
    roots_ranges: HashMap<NodeIndex, [NodeIndex; 2]>,
    roots_tree: petgraph::Graph<NodeIndex, (), Directed>,
    /// keeps track of current root index in the root graph
    /// and current element index in the root's range
    current: Option<(NodeIndex, NodeIndex)>,
}

impl StateIteration {
    pub fn new(root: NodeIndex, g: &Graph<Node, (), Directed>) -> Self {
        let mut roots_ranges = HashMap::new();

        roots_ranges.insert(
            root,
            [
                g.node_indices().next().unwrap(),
                g.node_indices().last().unwrap(),
            ],
        );

        let mut roots = petgraph::Graph::new();
        roots.add_node(root);
        Self {
            roots_ranges,
            roots_tree: roots,
            current: Some((root, g.node_indices().next().unwrap())),
        }
    }

    pub fn add(&mut self, root: NodeIndex, g: &Graph<Node, (), Directed>) {
        self.roots_ranges
            .insert(root, [self.last(), g.node_indices().last().unwrap()]);

        let root_idx = self.roots_tree.add_node(root);
        let parent_idx = self
            .roots_tree
            .node_indices()
            .find(|i| self.current.unwrap().0 == *i)
            .unwrap();

        self.roots_tree.add_edge(parent_idx, root_idx, ());
        self.current = Some((root, self.roots_ranges[&root][0]));
    }

    pub fn set_current(&mut self, curr: NodeIndex) {
        self.current = Some((self.root(curr), curr));
    }

    /// gets root for the provided index from range
    pub fn root(&self, idx: NodeIndex) -> NodeIndex {
        *self
            .roots_ranges
            .iter()
            .find(|(_, range)| range[0] <= idx && idx <= range[1])
            .unwrap()
            .0
    }

    /// Gets next element index from current root range. If current element is the last one in the
    /// range, then it returns the first element in the range.
    pub fn next(&mut self) -> NodeIndex {
        let (root, idx) = self.current.unwrap();
        let range = self.roots_ranges.get(&root).unwrap();

        let mut new_idx = NodeIndex::new(idx.index() + 1);
        if new_idx > range[1] {
            new_idx = range[0]
        }

        self.current.as_mut().unwrap().1 = new_idx;
        new_idx
    }

    /// Gets prev element index from current root range. If current element is the first one in the
    /// range, then it returns the last element in the range.
    pub fn prev(&mut self) -> NodeIndex {
        let (root, idx) = self.current.unwrap();
        let range = self.roots_ranges.get(&root).unwrap();

        if idx.index() == 0 || idx == range[0] {
            self.current.as_mut().unwrap().1 = range[1];
            return range[1];
        }

        let new_idx = NodeIndex::new(idx.index() - 1);
        self.current.as_mut().unwrap().1 = new_idx;
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
    // pub fn prev_root(&self) -> NodeIndex {}

    fn last(&self) -> NodeIndex {
        self.roots_ranges
            .values()
            .map(|r| *r.last().unwrap())
            .max()
            .unwrap()
    }
}
