use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::prelude::*;

/// Initialize structured logging with the given verbosity level.
/// Uses JSON format when `json` is true, otherwise human-readable format.
pub fn init_logging(level: LevelFilter, json: bool) {
    let env_filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(level.into())
        .from_env_lossy();

    if json {
        let fmt_layer = fmt::layer().json().with_writer(std::io::stderr);
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();
    } else {
        let fmt_layer = fmt::layer().with_writer(std::io::stderr);
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();
    }
}
