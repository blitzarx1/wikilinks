use crossbeam::channel::{Receiver, Sender};
use egui::{Color32, Ui};
use egui_graphs::{events::Event, Graph, SettingsInteraction, SettingsNavigation, SettingsStyle};
use petgraph::Directed;

use crate::node::Node;

const EDGE_WEIGHT: f32 = 0.05;
const EDGE_COLOR: Color32 = Color32::from_rgba_premultiplied(128, 128, 128, 64);

pub struct State<'a> {
    pub loading: bool,
    pub g: &'a mut Graph<Node, (), Directed>,
    pub sender: Sender<Event>,
    pub receiver: Receiver<Event>,
}

pub fn draw_view_graph(ui: &mut Ui, state: State) {
    let mut w = egui_graphs::GraphView::new(state.g);
    let styles = &SettingsStyle::default();
    if state.loading {
        w = w.with_styles(styles);
    } else {
        w = w.with_interactions(
            &SettingsInteraction::default()
                .with_node_selection_enabled(true)
                .with_dragging_enabled(true),
        );
        w = w.with_navigations(
            &SettingsNavigation::default()
                .with_fit_to_screen_enabled(false)
                .with_zoom_and_pan_enabled(true),
        );
        w = w.with_styles(styles);
        w = w.with_events(&state.sender)
    }

    ui.add(&mut w);
}
