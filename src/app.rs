use std::collections::HashMap;

use crossbeam::channel::{unbounded, Receiver, Sender};
use egui::{CentralPanel, SidePanel};
use egui::{Context, InputState, Stroke, Style, Ui};
use egui_graphs::{add_edge, add_node_custom, Change, ChangeNode, Graph, Node};
use log::error;
use log::info;
use petgraph::{
    stable_graph::{NodeIndex, StableGraph},
    Directed,
};
use rand::seq::IteratorRandom;
use rand::Rng;
use reqwest::Error;
use tokio::task::JoinHandle;

use crate::views::graph::{self, draw_view_graph};
use crate::views::input::draw_view_input;
use crate::views::style::{COLOR_ACCENT, COLOR_LEFT_LOW, COLOR_SUB_ACCENT, CURSOR_WIDTH};
use crate::views::toolbox::{self, draw_view_toolbox};
use crate::{
    node,
    state::{next, Fork, State},
    url::{self, Url},
    url_retriever::UrlRetriever,
};

pub struct App {
    root_article_url: String,
    state: State,

    style: Style,

    active_tasks: HashMap<NodeIndex, (Receiver<Result<Url, Error>>, JoinHandle<()>)>,

    g: Graph<node::Node, (), Directed>,

    selected_node: Option<NodeIndex>,

    changes_sender: Sender<Change>,
    changes_receiver: Receiver<Change>,

    node_by_url: HashMap<Url, NodeIndex>,
}

impl App {
    pub fn new() -> Self {
        let mut style = Style::default();
        style.visuals.text_cursor_width = CURSOR_WIDTH;
        style.visuals.selection.stroke = Stroke::new(1., COLOR_ACCENT);
        style.visuals.selection.bg_fill = COLOR_SUB_ACCENT;

        let (changes_sender, changes_receiver) = unbounded();

        App {
            style,
            changes_sender,
            changes_receiver,

            root_article_url: Default::default(),
            state: Default::default(),
            g: Default::default(),
            active_tasks: Default::default(),
            selected_node: Default::default(),
            node_by_url: Default::default(),
        }
    }

    pub fn update(&mut self, ctx: &Context) {
        ctx.set_style(self.style.clone());

        self.handle_state();
        self.draw(ctx);
        self.handle_keys(ctx);
    }

    fn handle_state(&mut self) {
        match self.state {
            State::GraphAndLoading => self.handle_state_graph_and_loading(),
            State::Graph => self.handle_state_graph(),
            State::GraphAndLoadingError | State::Input | State::InputError => (),
        }
    }

    fn draw(&mut self, ctx: &Context) {
        match self.state {
            State::Input => self.draw_input(ctx),
            State::InputError => self.draw_input_error(ctx),
            State::GraphAndLoading => self.draw_graph_and_loading(ctx),
            State::Graph => self.draw_graph(ctx),
            State::GraphAndLoadingError => todo!(),
        }
    }

    fn handle_state_graph(&mut self) {
        if let Ok(change) = self.changes_receiver.try_recv() {
            match change {
                Change::Node(change_node) => match change_node {
                    ChangeNode::Selected { id, old, new } => match new {
                        true => {
                            info!("node {:?} selected", id);
                            self.selected_node = Some(id);
                        }
                        false => {
                            info!("node {:?} deselected", id);
                            self.selected_node = None;
                        }
                    },
                    _ => (),
                },
                _ => (),
            }
        }
    }

    /// Checks for results from the url retriever for every active task. If any task is finished,
    /// moves to the next state.
    fn handle_state_graph_and_loading(&mut self) {
        match self.process_active_tasks() {
            Ok(_) => {
                if self.active_tasks.is_empty() {
                    info!("all tasks finished");
                    self.state = next(&self.state, Fork::Success);
                }
            }
            Err(err) => {
                error!("error while checking active tasks: {}", err);
                self.state = next(&self.state, Fork::Failure);
            }
        }
    }

    /// Processes results from the url retriever for every active task.
    ///
    /// Updates the graph with the retrieved urls.
    ///
    /// If any task is finished, removes it from the active tasks.
    ///
    /// If we got any url, function returns true, otherwise false. If an error was got function returns error.
    fn process_active_tasks(&mut self) -> Result<(), Error> {
        let mut finished_tasks = Vec::new();
        self.active_tasks
            .iter()
            .for_each(
                |(parent_idx, (receiver, join_handle))| match receiver.try_recv() {
                    Ok(result) => match result {
                        Ok(url) => {
                            info!("got new url from the retriver: {}", url.val());

                            let parent_loc = self.g.node_weight(*parent_idx).unwrap().location();

                            match self.node_by_url.get(&url) {
                                Some(idx) => {
                                    add_edge(&mut self.g, *parent_idx, *idx, &());
                                }
                                None => {
                                    let idx = add_node_custom(
                                        &mut self.g,
                                        &node::Node::new(url.clone()),
                                        |_, n| {
                                            let mut rng = rand::thread_rng();

                                            let color = match n.url().url_type() {
                                                url::Type::Article => Some(COLOR_SUB_ACCENT),
                                                url::Type::File => Some(COLOR_LEFT_LOW),
                                                url::Type::Other => None,
                                            };

                                            let mut res = Node::new(
                                                egui::Vec2 {
                                                    x: parent_loc.x + rng.gen_range(-100.0..100.),
                                                    y: parent_loc.y + rng.gen_range(-100.0..100.),
                                                },
                                                n.clone(),
                                            )
                                            .with_label(n.url().val().to_string());

                                            if let Some(c) = color {
                                                res = res.with_color(c);
                                            }

                                            res
                                        },
                                    );
                                    self.node_by_url.insert(url, idx);
                                    add_edge(&mut self.g, *parent_idx, idx, &());
                                }
                            };
                        }
                        Err(err) => {
                            error!("got error from the retriver: {}", err);
                        }
                    },

                    Err(_) => {
                        if join_handle.is_finished() {
                            finished_tasks.push(*parent_idx);
                        }
                    }
                },
            );

        finished_tasks.iter().for_each(|finished| {
            info!(
                "task finished; received all children urls for: {}",
                self.g
                    .node_weight(*finished)
                    .unwrap()
                    .data()
                    .unwrap()
                    .url()
                    .val()
            );
            self.active_tasks.remove(finished);
        });

        Ok(())
    }

    fn handle_keys(&mut self, ctx: &Context) {
        ctx.input(|i| match self.state {
            State::Input => self.handle_keys_input(i),
            State::InputError
            | State::GraphAndLoading
            | State::GraphAndLoadingError
            | State::Graph => (),
        });
    }

    fn draw_input_error(&mut self, ctx: &Context) {
        let input_resp = CentralPanel::default().show(ctx, |ui| {
            draw_view_input(
                &mut self.root_article_url,
                ui,
                false,
                ui.available_height() / 5.,
                ui.available_height() / 20.,
            )
        });

        if input_resp.inner.changed() {
            self.state = next(&self.state, Fork::Success);
        }
    }

    fn draw_input(&mut self, ctx: &Context) {
        CentralPanel::default().show(ctx, |ui| {
            draw_view_input(
                &mut self.root_article_url,
                ui,
                true,
                ui.available_height() / 5.,
                ui.available_height() / 20.,
            );
        });
    }

    fn draw_graph_and_loading(&mut self, ctx: &Context) {
        SidePanel::right("toolbox").resizable(true).show(ctx, |ui| {
            ui.centered_and_justified(|ui| {
                draw_view_toolbox(ui, &self.generate_toolbox_state(ui, true))
            });
        });
        CentralPanel::default().show(ctx, |ui| {
            draw_view_graph(ui, self.generate_graph_state(true));
        });
    }

    fn create_new_task(&mut self, idx: NodeIndex, url: Url) {
        let (sender, receiver) = unbounded();
        let retriever = UrlRetriever::new(sender);

        info!("started retriever for {}", url.val());

        self.active_tasks
            .insert(idx, (receiver, retriever.run(url)));
    }

    fn draw_graph(&mut self, ctx: &Context) {
        SidePanel::right("toolbox").resizable(true).show(ctx, |ui| {
            if let Some(resp) = draw_view_toolbox(ui, &self.generate_toolbox_state(ui, false)) {
                if !resp.clicked() {
                    return;
                }

                let idx = self.selected_node.unwrap();
                let n = self.g.node_weight(idx).unwrap().data().unwrap();

                self.create_new_task(idx, n.url().clone());
                self.state = State::GraphAndLoading;
            }
        });
        CentralPanel::default().show(ctx, |ui| {
            draw_view_graph(ui, self.generate_graph_state(false));
        });
    }

    fn handle_keys_input(&mut self, i: &InputState) {
        if i.key_pressed(egui::Key::Enter) {
            match url::Url::new(&self.root_article_url) {
                Ok(u) => {
                    if !u.is_wiki() {
                        self.state = next(&self.state, Fork::Failure);
                        return;
                    }

                    self.g = StableGraph::new();

                    let idx = add_node_custom(&mut self.g, &node::Node::new(u.clone()), |_, n| {
                        let mut rng = rand::thread_rng();
                        Node::new(
                            egui::Vec2 {
                                x: rng.gen_range(-100.0..100.),
                                y: rng.gen_range(-100.0..100.),
                            },
                            n.clone(),
                        )
                        .with_label(n.url().val().to_string())
                        .with_color(COLOR_ACCENT)
                    });

                    self.node_by_url.insert(u.clone(), idx);

                    self.create_new_task(idx, u);

                    self.state = next(&self.state, Fork::Success);
                }
                Err(_) => {
                    self.state = next(&self.state, Fork::Failure);
                }
            };
        };
    }

    fn generate_graph_state(&mut self, loading: bool) -> graph::State {
        graph::State {
            loading,
            g: &mut self.g,
            sender: self.changes_sender.clone(),
            receiver: self.changes_receiver.clone(),
        }
    }

    fn generate_toolbox_state(&mut self, ui: &Ui, loading: bool) -> toolbox::State {
        toolbox::State {
            loading,
            spacing: ui.available_height() / 30.,
            selected_node: self.selected_node,
            g: &self.g,
        }
    }
}
