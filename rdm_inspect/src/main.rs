use std::fs;
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{ArgAction, Parser};
use rdm_inspect::{output, tui, Analysis};

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Explore RDM block structure without knowing record semantics",
    after_help = "The default is an interactive hex/graph TUI when stdout is a terminal, and --dump otherwise."
)]
struct Options {
    /// RDM file to inspect.
    #[arg(value_name = "FILE")]
    input: PathBuf,

    /// Print the linear block map and inferred offset fields.
    #[arg(long, conflicts_with_all = ["graph", "dot"])]
    dump: bool,

    /// Print the forward-candidate graph as text.
    #[arg(long, conflicts_with_all = ["dump", "dot"])]
    graph: bool,

    /// Print the inferred reference graph in Graphviz DOT format.
    #[arg(long, conflicts_with_all = ["dump", "graph"])]
    dot: bool,

    /// Initial byte offset in the TUI, in decimal or with a 0x prefix.
    #[arg(long, value_parser = parse_offset, default_value = "0")]
    offset: usize,

    /// Disable ANSI colors in --dump output.
    #[arg(long, action = ArgAction::SetTrue)]
    no_color: bool,
}

fn parse_offset(value: &str) -> Result<usize, String> {
    if let Some(hex) = value.strip_prefix("0x") {
        usize::from_str_radix(hex, 16).map_err(|error| error.to_string())
    } else {
        value.parse::<usize>().map_err(|error| error.to_string())
    }
}

fn main() -> ExitCode {
    match run(Options::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("rdm-inspect: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(options: Options) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = fs::read(&options.input).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("cannot read {}: {error}", options.input.display()),
        )
    })?;
    let analysis = Analysis::parse(bytes)?;
    if options.offset >= analysis.bytes.len() {
        return Err(format!(
            "initial offset 0x{:x} lies beyond the {}-byte file",
            options.offset,
            analysis.bytes.len()
        )
        .into());
    }

    if options.dot {
        print!("{}", output::dot(&analysis));
    } else if options.graph {
        print!("{}", output::graph(&analysis));
    } else if options.dump || !io::stdout().is_terminal() {
        let color = !options.no_color && io::stdout().is_terminal();
        print!("{}", output::summary(&analysis, color));
    } else {
        tui::run(
            &analysis,
            &options.input.display().to_string(),
            options.offset,
        )?;
    }
    Ok(())
}
