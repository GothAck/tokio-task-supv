use std::io;

use anyhow::Result;
use tracing_subscriber::{filter::LevelFilter, fmt, prelude::*, EnvFilter};

pub fn init() -> Result<()> {
    let layer_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env()?;

    let layer_stderr = fmt::Layer::new()
        .with_thread_ids(true)
        .with_writer(io::stderr);

    tracing_subscriber::registry()
        .with(layer_filter)
        .with(layer_stderr)
        .init();

    Ok(())
}
