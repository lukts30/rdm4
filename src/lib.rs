//! Application commands shared by the CLI entry points.

pub mod cli;
pub mod commands;

/// Initialize CLI logging, respecting `RUST_LOG`.
pub fn init_logging(verbosity: u8) {
    use env_logger::Env;

    match verbosity {
        0 => env_logger::Builder::from_env(Env::default().default_filter_or("info")).init(),
        1 => env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init(),
        2 => env_logger::Builder::from_env(Env::default().default_filter_or("trace")).init(),
        _ => log::warn!("Don't be crazy"),
    }
}
