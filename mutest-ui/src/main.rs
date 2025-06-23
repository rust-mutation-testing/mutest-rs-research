use std::path::PathBuf;

use clap::{Parser, Subcommand};
use mutest_ui::{server, report, common};

#[derive(Parser)]
#[command(name = "cargo mutest-ui")]
#[command(bin_name = "cargo mutest-ui")]
#[command(styles = mutest_driver_cli::common::clap_styles())]
struct CargoCli {
    #[clap(subcommand)]
    command: MutestUiCommands,
}

#[derive(Subcommand)]
enum MutestUiCommands {
    #[command(
        about = "Runs the Mutest UI server for real time report generation.", 
        long_about = "Runs the Mutest UI server on the localhost address. The Mutest UI server provides an enhanced feature set which allows viewing of the mutest call-graph for each mutation.",
    )]
    #[command(arg_required_else_help = true)]
    Server {
        #[arg(
            short = 'd',
            long = "mutest-data-dir",
            value_name = "MUTEST_OUTPUT_PATH",
            default_value = mutest_driver_cli::common::DEFAULT_JSON_DIR,
        )]
        json_path: PathBuf,
    },
    #[command(
        about = "Export the Mutest UI report of the mutest output.",
        long_about = "Exports the Mutest UI report as static html files. Due to the number and size of each generated file, the report has a reduced feature set compared to the server.",
    )]
    #[command(arg_required_else_help = true)]
    Report {
        #[arg(
            short = 'd', 
            long = "mutest-data-dir",
            value_name = "MUTEST_OUTPUT_PATH",
            default_value = mutest_driver_cli::common::DEFAULT_JSON_DIR,
        )]
        json_path: PathBuf,
        #[arg(
            short = 'e',
            long = "export-dir",
            value_name = "REPORT_EXPORT_PATH",
            default_value = common::DEFAULT_REPORT_DIR,
        )]
        export_path: PathBuf,
    }
}

fn main() {
    let args = CargoCli::parse();

    match args.command {
        MutestUiCommands::Server { json_path } => {
            server(&json_path);
        },
        MutestUiCommands::Report { json_path, export_path } => {
            report(&json_path, &export_path);
        }
    }
}