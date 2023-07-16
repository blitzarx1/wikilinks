use eframe::{run_native, CreationContext, Frame, NativeOptions};
use egui::Context;

const APP_NAME: &str = "Wiki Links";

mod app;
mod iteration;
mod node;
mod state;
mod url;
mod url_retriever;
mod views;

pub struct App {
    app: app::App,
}

impl App {
    fn new(_: &CreationContext<'_>) -> Self {
        Self {
            app: app::App::default(),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, _: &mut Frame) {
        self.app.update(ctx);
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let native_options = NativeOptions::default();
    run_native(
        APP_NAME,
        native_options,
        Box::new(|cc| Box::new(App::new(cc))),
    )
    .unwrap();
}
