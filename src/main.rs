mod app;
mod event;
mod metrics;
mod model;
mod platform;
mod ui;

fn main() -> anyhow::Result<()> { app::run() }

