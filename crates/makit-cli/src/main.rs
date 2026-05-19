use clap::{Parser, Subcommand};
use anyhow::Result;

mod commands;

/// makit — A multi-tool CLI and TUI for AEC workflows
#[derive(Parser)]
#[command(name = "makit", version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// List all registered tools, sources, and actions
    List {
        /// Filter by tool name
        #[arg(short, long)]
        tool: Option<String>,
    },
    /// Execute a task
    Exec {
        /// Tool name
        tool: String,
        /// Category name
        category: String,
        /// Task name
        task: String,
        /// Options as key=value pairs
        #[arg(short, long, value_parser = parse_option)]
        option: Vec<(String, String)>,
    },
    /// Analyze an IFC or geometry file
    Analyze {
        /// Input file path
        file: String,
        /// Analysis type
        #[arg(short = 't', long, default_value = "summary")]
        analysis_type: String,
    },
    /// Launch the interactive TUI
    Tui,
    /// Show status of connected tools
    Status,
    /// Initialize a new makit project
    Init,
}

fn parse_option(s: &str) -> Result<(String, String), String> {
    let parts: Vec<&str> = s.splitn(2, '=').collect();
    if parts.len() == 2 {
        Ok((parts[0].to_string(), parts[1].to_string()))
    } else {
        Err(format!("invalid option format: {s} (expected key=value)"))
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Register all tools
    makit_tools::register_all_tools();

    match cli.command {
        Some(Commands::List { tool }) => commands::list::run(tool),
        Some(Commands::Exec { tool, category, task, option }) => {
            commands::exec::run(&tool, &category, &task, &option)
        }
        Some(Commands::Analyze { file, analysis_type }) => {
            commands::analyze::run(&file, &analysis_type)
        }
        Some(Commands::Tui) => commands::tui::run(),
        Some(Commands::Status) => {
            println!("makit status — checking connected tools...");
            Ok(())
        }
        Some(Commands::Init) => {
            println!("Initializing makit project...");
            Ok(())
        }
        None => {
            // Default: launch TUI
            commands::tui::run()
        }
    }
}
