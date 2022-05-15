use clap::Parser;
use log::{info, warn, trace};
use std::process::exit;

mod coverage;
mod file;
mod runner;
mod analytics;
mod formatter;

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

    /// fail fast
    #[clap(long)]
    fail_fast: bool,

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

    let coverage = coverage::Coverage::load(&args.coverage);
    let files = file::File::load(&args.files);
    let mut analytics = analytics::AnalyticsResult::start(files.len().try_into().unwrap());
    let mut failed = false;

    info!("Running transmute for files. It can take several minutes..");

    for file in files.iter() {
        'mutate: for mutable in file.mutable_items.iter() {
            mutable.transmute(&file.path);

            for spec_file in coverage.find(&file.path, mutable.line_number).iter() {
                let (exit_code, stdout) = runner::run(&args.command, spec_file);

                trace!("{}", stdout);
                analytics.add(&file.path, mutable, exit_code, stdout);

                if exit_code != 0 {
                    mutable.undo(&file.path);
                    failed = true;
                    continue 'mutate;
                }
            }

            warn!(
                "Changing '{}' on line '{}' did not break the specs. Consider adding a spec",
                file.path, mutable.line_number
            );

            mutable.undo(&file.path);

            if args.fail_fast {
                exit(1);
            }
        }
    }

    formatter::generate(&analytics);

    exit(if failed { 1 } else { 0 });
}
