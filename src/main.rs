mod app;
mod demo;
mod event;
mod metrics;
mod model;
mod platform;
mod ui;

fn main() -> anyhow::Result<()> {
    app::run()
}
