use egui::{Response, ScrollArea, TextEdit, Ui};
use egui_graphs::Graph;
use petgraph::{stable_graph::NodeIndex, Directed};

use crate::node::Node;

use super::style::header_accent;

const HEADING: &str = "Wiki Links";
const MSG_SCRAPPING: &str = "scrapping links ...";

pub struct State<'a> {
    pub loading: bool,
    pub spacing: f32,
    pub g: &'a Graph<Node, (), Directed>,
    pub selected_node: Option<NodeIndex>,
}

/// Draws toolbox view and returns response from `get links` button if it was displayed.
pub fn draw_view_toolbox(ui: &mut Ui, state: &State) -> Option<Response> {
    let mut resp = None;
    ScrollArea::vertical().show(ui, |ui| {
        ui.vertical_centered(|ui| {
            ui.add_space(state.spacing);
            ui.label(header_accent(HEADING));

            ui.add_space(state.spacing);
            ui.separator();

            ui.label(format!("urls: {}", state.g.node_count()));
            ui.label(format!("connections: {}", state.g.edge_count()));

            match state.loading {
                true => {
                    ui.add_space(state.spacing);
                    ui.label(MSG_SCRAPPING);
                    ui.centered_and_justified(|ui| ui.spinner());
                }
                false => {
                    ui.add_space(state.spacing);
                    resp = draw_selected_node(ui, state);
                }
            }
        })
    });

    resp
}

pub fn draw_selected_node(ui: &mut Ui, state: &State) -> Option<Response> {
    state.selected_node?;

    let node = state
        .g
        .node_weight(state.selected_node.unwrap())
        .unwrap()
        .data()
        .unwrap();

    ui.label(format!("{:?}", node.url().url_type()));

    ui.add(
        TextEdit::singleline(&mut node.url().val())
            .cursor_at_end(true)
            .frame(false),
    );

    if ui.button("copy").clicked() {
        todo!()
    };

    if ui.button("open").clicked() {
        open::that(node.url().val()).unwrap();
    };

    match node.url().url_type() {
        crate::url::Type::Article => Some(ui.button("get links")),
        _ => None,
    }
}
