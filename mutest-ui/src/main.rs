use std::path::PathBuf;

use clap::{Parser};
use mutest_ui::{server, config};

#[derive(Parser, Debug)]
#[command(
    name = "cargo mutest-ui",
    bin_name = "cargo mutest-ui",
    about = "Runs the mutest web server for results visualization",
    styles = mutest_driver_cli::common::clap_styles(),
)]
struct CargoCli {
    /// Pre-caches all static code files before running the server.
    #[arg(long, default_value_t = false)]
    pre_cache_all: bool,

    /// Sets the diffing type used by the server when displaying mutations.
    #[arg(long = "diff-type", default_value = "advanced", value_name = "DIFF_TYPE")]
    sys_diff_type: config::SysDiffType,

    /// Sets the port that the server is run on.
    #[arg(short, long, default_value = "8080")]
    port: String,

    /// Directory holding mutest run results; the server assumes the default layout and
    /// auto-discovers source code by walking up the directory tree
    #[arg(long, default_value = "mutest/report")]
    results_dir: PathBuf,

    /// Specify the source code directory when the results directory has been moved from its
    /// default location
    #[arg(long)]
    source_dir: Option<PathBuf>,

    /// The path to the /assets directory in the server source code.
    #[arg(long, required = true)]
    resource_dir: PathBuf,
}

fn main() {
    let args = CargoCli::parse();
    let config = config::ServerConfig {
        pre_cache_all: args.pre_cache_all,
        sys_diff_type: args.sys_diff_type,
        port: args.port,
        results_dir: args.results_dir,
        resource_dir: args.resource_dir,
        source_dir: args.source_dir,
    };
    server(config);
}