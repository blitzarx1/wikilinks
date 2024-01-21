use std::collections::HashMap;

use crossbeam::channel::{unbounded, Receiver, Sender};
use egui::{CentralPanel, Pos2, SidePanel, Vec2};
use egui::{Context, InputState, Stroke, Style, Ui};
use egui_graphs::events::{Event, PayloadNodeSelect};
use egui_graphs::{add_node_custom, Graph, Node};
use fdg_sim::glam::Vec3;
use fdg_sim::{ForceGraph, Simulation, SimulationParameters};
use log::error;
use log::info;
use petgraph::{
    stable_graph::{NodeIndex, StableGraph},
    Directed,
};
use rand::Rng;
use reqwest::Error;
use tokio::task::JoinHandle;

use crate::cursor::Cursor;
use crate::views::graph::{self, draw_view_graph};
use crate::views::input::draw_view_input;
use crate::views::style::{
    COLOR_ACCENT, COLOR_LEFT_LOW, COLOR_RIGHT_LOW, COLOR_SUB_ACCENT, CURSOR_WIDTH,
};
use crate::views::toolbox::{self, draw_view_toolbox};
use crate::{
    node,
    state::{next, Fork, State},
    url::{self, Url},
    url_retriever::UrlRetriever,
};

const SIMULATION_DT: f32 = 0.035;
const EDGE_WEIGHT: f32 = 0.1;
const COOL_OFF: f32 = 0.5;
const SCALE: f32 = 50.;

type ActiveTasks = HashMap<NodeIndex, (Receiver<Result<Url, Error>>, JoinHandle<()>)>;

pub struct App {
    root_article_url: String,
    state: State,

    style: Style,

    active_tasks: ActiveTasks,

    g: Graph<node::Node, (), Directed>,
    sim: Simulation<(), f32>,

    selected_node: Option<NodeIndex>,

    cursor: Option<Cursor>,

    changes_sender: Sender<Event>,
    changes_receiver: Receiver<Event>,

    node_by_url: HashMap<Url, NodeIndex>,
}

impl Default for App {
    fn default() -> Self {
        let mut style = Style::default();
        style.visuals.selection.stroke = Stroke::new(1., COLOR_ACCENT);
        style.visuals.selection.bg_fill = COLOR_SUB_ACCENT;

        let (changes_sender, changes_receiver) = unbounded();
        let sim = construct_simulation();

        let g = Graph::new(StableGraph::new());

        App {
            style,
            changes_sender,
            changes_receiver,
            sim,
            g,

            root_article_url: Default::default(),
            state: Default::default(),
            active_tasks: Default::default(),
            selected_node: Default::default(),
            node_by_url: Default::default(),
            cursor: Default::default(),
        }
    }
}

impl App {
    pub fn update(&mut self, ctx: &Context) {
        ctx.set_style(self.style.clone());

        self.handle_state();
        self.draw(ctx);
        self.handle_keys(ctx);

        sync_graph_with_simulation(&mut self.g, &mut self.sim);
        update_simulation(&mut self.sim);
    }

    fn handle_state(&mut self) {
        match self.state {
            State::GraphAndLoading => self.handle_state_graph_and_loading(),
            State::GraphLoaded => self.handle_state_graph_loaded(),
            State::Graph => self.handle_state_graph(),
            State::GraphAndLoadingError | State::Input | State::InputError => (),
        }
    }

    fn draw(&mut self, ctx: &Context) {
        match self.state {
            State::Input => self.draw_input(ctx),
            State::InputError => self.draw_input_error(ctx),
            State::GraphAndLoading => self.draw_graph_and_loading(ctx),
            State::Graph | State::GraphLoaded => self.draw_graph(ctx),
            State::GraphAndLoadingError => todo!(),
        }
    }

    fn handle_state_graph_loaded(&mut self) {
        if self.cursor.is_none() {
            let first_root = NodeIndex::new(0);
            self.cursor = Some(Cursor::new(first_root, &self.g));
            self.select_node(first_root)
        } else {
            self.cursor
                .as_mut()
                .unwrap()
                .update(self.selected_node.unwrap(), &self.g);
        }

        self.state = next(&self.state, Fork::Success)
    }

    fn handle_state_graph(&mut self) {
        if let Ok(Event::NodeSelect(PayloadNodeSelect { id })) = self.changes_receiver.try_recv() {
            self.select_node(NodeIndex::new(id));
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

                            let parent_loc = self.g.g.node_weight(*parent_idx).unwrap().location();

                            match self.node_by_url.get(&url) {
                                Some(idx) => {
                                    add_edge(&mut self.g, &mut self.sim, *parent_idx, *idx);
                                }
                                None => {
                                    let idx = add_node(
                                        &mut self.g,
                                        &mut self.sim,
                                        parent_loc,
                                        &node::Node::new(url.clone()),
                                    );
                                    self.node_by_url.insert(url, idx);
                                    add_edge(&mut self.g, &mut self.sim, *parent_idx, idx);
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
                    .g
                    .node_weight(*finished)
                    .unwrap()
                    .payload()
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
            | State::GraphLoaded => (),
            State::Graph => self.handle_keys_graph(i),
        });
    }

    fn select_node(&mut self, idx: NodeIndex) {
        if let Some(selected) = self.selected_node {
            let n = self.g.g.node_weight_mut(selected).unwrap();
            n.set_selected(false);
        }

        let n = self.g.g.node_weight_mut(idx).unwrap();
        n.set_selected(true);
        self.selected_node = Some(idx);
    }

    fn select_next(&mut self) -> NodeIndex {
        let cursor = self.cursor.as_mut().unwrap();
        let next = cursor.next_child();

        self.select_node(next);

        next
    }

    fn select_prev(&mut self) -> NodeIndex {
        let cursor = self.cursor.as_mut().unwrap();
        let prev = cursor.prev_child();

        self.select_node(prev);

        prev
    }

    fn select_next_article(&mut self) {
        let cursor = self.cursor.as_mut().unwrap();
        let mut next = || cursor.next_child();
        let mut n = next();
        loop {
            if self.g.g.node_weight(n).unwrap().payload().url().url_type() == url::Type::Article {
                break;
            }

            n = next();
        }

        self.select_node(n);
    }

    fn select_prev_article(&mut self) {
        let cursor = self.cursor.as_mut().unwrap();
        let mut prev = || cursor.prev_child();

        let mut p = prev();
        loop {
            if self.g.g.node_weight(p).unwrap().payload().url().url_type() == url::Type::Article {
                break;
            }

            p = prev();
        }

        self.select_node(p);
    }

    fn handle_keys_graph(&mut self, i: &InputState) {
        if i.key_pressed(egui::Key::L) {
            self.select_next();
        }
        if i.key_pressed(egui::Key::H) {
            self.select_prev();
        }
        if i.key_pressed(egui::Key::ArrowLeft) {
            self.select_prev_article();
        }
        if i.key_pressed(egui::Key::ArrowRight) {
            self.select_next_article();
        }
        if i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::J) {
            self.select_next_root();
        }
        if i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::K) {
            self.select_prev_root();
        }
        if i.key_pressed(egui::Key::Enter) {
            if let Some(idx) = self.selected_node {
                let n = self.g.g.node_weight(idx).unwrap().payload();

                self.create_new_task(idx, n.url().clone());
                self.state = State::GraphAndLoading;
            }
        }
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
                let n = self.g.g.node_weight(idx).unwrap().payload();

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

                    self.g.g = StableGraph::new();
                    let mut rng = rand::thread_rng();
                    let loc = egui::Vec2 {
                        x: rng.gen_range(-100.0..100.),
                        y: rng.gen_range(-100.0..100.),
                    };

                    let idx: NodeIndex =
                        add_node_custom(&mut self.g, &node::Node::new(u.clone()), |idx, n| {
                            let mut res = Node::new(n.clone()).with_label(n.label());
                            res.bind(idx, loc.to_pos2());
                            res
                        });

                    self.node_by_url.insert(u.clone(), idx);

                    add_node_to_sim(&mut self.sim, idx, loc);

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
        let mut selected_node_root = None;
        if self.selected_node.is_some() {
            selected_node_root = Some(self.cursor.as_ref().unwrap().position().0);
        }

        toolbox::State {
            loading,
            selected_node_root,
            spacing: ui.available_height() / 30.,
            selected_node: self.selected_node,
            g: &self.g,
        }
    }

    fn select_next_root(&mut self) {
        let cursor = self.cursor.as_mut().unwrap();
        let curr_root = cursor.position().0;
        if let Some(next) = cursor.next_root() {
            self.select_node(next);
        } else {
            self.select_node(curr_root);
        }
    }

    fn select_prev_root(&mut self) {
        let cursor = self.cursor.as_mut().unwrap();
        let curr_root = cursor.position().0;
        if let Some(prev) = cursor.prev_root() {
            self.select_node(prev);
        } else {
            self.select_node(curr_root);
        }
    }
}

fn add_node(
    g: &mut Graph<node::Node, (), Directed>,
    sim: &mut Simulation<(), f32>,
    loc_center: Pos2,
    n: &node::Node,
) -> NodeIndex {
    let mut rng = rand::thread_rng();
    let loc = egui::Vec2 {
        x: loc_center.x + rng.gen_range(-100.0..100.),
        y: loc_center.y + rng.gen_range(-100.0..100.),
    };

    let color = match n.url().url_type() {
        url::Type::Article => Some(COLOR_SUB_ACCENT),
        url::Type::ExternalArticle => Some(COLOR_RIGHT_LOW),
        url::Type::File => Some(COLOR_LEFT_LOW),
        url::Type::Other => None,
    };

    let idx = add_node_custom(g, n, |idx, n| {
        let mut res = Node::new(n.clone()).with_label(n.label());
        res.bind(idx, loc.to_pos2());
        res
    });

    add_node_to_sim(sim, idx, loc)
}

fn add_node_to_sim(sim: &mut Simulation<(), f32>, idx: NodeIndex, loc: Vec2) -> NodeIndex {
    let mut sim_node = fdg_sim::Node::new(idx.index().to_string().as_str(), ());
    sim_node.location = Vec3::new(loc.x, loc.y, 0.);
    sim.get_graph_mut().add_node(sim_node)
}

fn add_edge(
    g: &mut Graph<node::Node, (), Directed>,
    sim: &mut Simulation<(), f32>,
    start: NodeIndex,
    end: NodeIndex,
) {
    egui_graphs::add_edge(g, start, end, &());
    sim.get_graph_mut().add_edge(start, end, EDGE_WEIGHT);
}

fn construct_simulation() -> Simulation<(), f32> {
    // create force graph
    let force_graph = ForceGraph::default();

    // initialize simulation
    let mut params = SimulationParameters::default();
    let force = fdg_sim::force::fruchterman_reingold_weighted(SCALE, COOL_OFF);
    params.set_force(force);

    Simulation::from_graph(force_graph, params)
}

fn update_simulation(sim: &mut Simulation<(), f32>) {
    // the following manipulations is a hack to avoid having looped edges in the simulation
    // because they cause the simulation to blow up;
    // this is the issue of the fdg_sim engine we use for the simulation
    // https://github.com/grantshandy/fdg/issues/10
    // * remove loop edges
    // * update simulation
    // * restore loop edges

    // remove looped edges
    let looped_nodes = {
        let graph = sim.get_graph_mut();
        let mut looped_nodes = vec![];
        let mut looped_edges = vec![];
        graph.edge_indices().for_each(|idx| {
            let edge = graph.edge_endpoints(idx).unwrap();
            let looped = edge.0 == edge.1;
            if looped {
                looped_nodes.push((edge.0, ()));
                looped_edges.push(idx);
            }
        });

        for idx in looped_edges {
            graph.remove_edge(idx);
        }

        sim.update(SIMULATION_DT);

        looped_nodes
    };

    // restore looped edges
    let graph = sim.get_graph_mut();
    for (idx, _) in looped_nodes.iter() {
        graph.add_edge(*idx, *idx, EDGE_WEIGHT);
    }
}

/// Syncs the graph with the simulation.
///
/// Changes location of nodes in `g` according to the locations in `sim`. If node from `g` is dragged its location is prioritized
/// over the location of the corresponding node from `sim` and this location is set to the node from the `sim`.
fn sync_graph_with_simulation(
    g: &mut Graph<node::Node, (), Directed>,
    sim: &mut Simulation<(), f32>,
) {
    let g_indices = g.g.node_indices().collect::<Vec<_>>();
    g_indices.iter().for_each(|g_n_idx| {
        let g_n = g.g.node_weight_mut(*g_n_idx).unwrap();
        let sim_n = sim.get_graph_mut().node_weight_mut(*g_n_idx).unwrap();

        if g_n.dragged() {
            let loc = g_n.location();
            sim_n.location = Vec3::new(loc.x, loc.y, 0.);
            return;
        }

        let loc = sim_n.location;
        g_n.set_location(Pos2::new(loc.x, loc.y));
    });
}
