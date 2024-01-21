use egui::{Response, ScrollArea, TextEdit, Ui};
use egui_graphs::Graph;
use petgraph::{stable_graph::NodeIndex, Directed};

use crate::{node::Node, utils};

use super::style::header_accent;

const HEADING: &str = "Wiki Links";
const MSG_SCRAPPING: &str = "scrapping links ...";

pub struct State<'a> {
    pub loading: bool,
    pub spacing: f32,
    pub g: &'a Graph<Node, (), Directed>,
    pub selected_node: Option<NodeIndex>,
    pub selected_node_root: Option<NodeIndex>,
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

            ui.label(format!("urls: {}", state.g.g.node_count()));
            ui.label(format!("connections: {}", state.g.g.edge_count()));

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

    let selected_idx = state.selected_node.unwrap();

    let node = state.g.g.node_weight(selected_idx).unwrap().payload();

    ui.label(format!(
        "{}->{}: {:?}",
        state.selected_node_root.unwrap().index(),
        selected_idx.index(),
        node.url().url_type()
    ));
    ScrollArea::horizontal().show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.add(
                TextEdit::singleline(&mut node.label())
                    .cursor_at_end(true)
                    .clip_text(false)
                    .frame(false),
            );
        });
    });

    if ui.button("copy").clicked() {
        todo!()
    };

    if ui.button("open").clicked() {
        utils::url::open_url(node.url().val());
    };

    match node.url().url_type() {
        crate::url::Type::Article => Some(ui.button("get links")),
        _ => None,
    }
}
