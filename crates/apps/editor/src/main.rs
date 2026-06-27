use tracing::info;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init()
        .ok();

    let egui_context = egui::Context::default();
    let _ = egui_context;
    info!("editor dev shell initialized");
    println!("WOT editor dev shell ready. Next step: map and vehicle inspectors on top of egui.");
    Ok(())
}
