use clap::Parser;
use log::info;
use std::process::exit;

/// transmute: Automatically change your code and make the tests fail. If don't, we will raise it for you.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Regex with files to run
    #[clap(long)]
    files: String,

    /// Command to run individual tests
    #[clap(long)]
    command: String,

    /// Coverage file name
    #[clap(long, default_value = "transmute.json")]
    coverage: String,

    /// log_level
    #[clap(long, default_value = "info")]
    log_level: String,
}

fn main() {
    let args = Args::parse();
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, args.log_level),
    );

    info!("Starting transmute.");
    info!("Loading coverage {}..", args.coverage);
    info!("Loading files {}..", args.files);

    exit(0);
}
