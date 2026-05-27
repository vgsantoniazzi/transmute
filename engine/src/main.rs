use clap::Parser;
use log::{info, trace, warn};
use std::process::exit;
use std::time::Duration;

mod analytics;
mod coverage;
mod file;
mod formatter;
mod runner;

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

    /// formatter
    #[clap(long, default_value = "json")]
    formatter: String,

    /// per-spec timeout in seconds
    #[clap(long, default_value = "600")]
    timeout: u64,

    /// output file path (defaults to result.json for json formatter, index.html for html)
    #[clap(long, default_value = "")]
    output: String,
}

fn main() {
    let args = Args::parse();
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, args.log_level),
    );

    ctrlc::set_handler(|| {
        file::restore_active_mutation();
        exit(130);
    })
    .expect("Error setting Ctrl-C handler");

    info!("Starting transmute.");

    let coverage = match coverage::Coverage::load(&args.coverage) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("transmute: {}", e);
            exit(2);
        }
    };
    let files = file::File::load(&args.files);
    let mut analytics = analytics::AnalyticsResult::start(files.len());
    let mut failed = false;

    info!("Running transmute for files. It can take several minutes..");

    for file in files.iter() {
        'mutate: for mutable in file.mutable_items.iter() {
            let specs = coverage.find(&file.path, mutable.line_number);
            if specs.is_empty() {
                warn!(
                    "No specs cover {}:{}; skipping mutation.",
                    file.path, mutable.line_number
                );
                continue 'mutate;
            }

            let _guard = file::MutationGuard::apply(&file.path, mutable);

            for spec_file in specs.iter() {
                let (exit_code, stdout) =
                    runner::run(&args.command, spec_file, Duration::from_secs(args.timeout));

                trace!("{}", stdout);
                analytics.add(&file.path, mutable, exit_code, stdout);

                if exit_code != 0 {
                    continue 'mutate;
                }
            }

            warn!(
                "Changing '{}' on line '{}' did not break the specs. Consider adding a spec",
                file.path, mutable.line_number
            );

            failed = true;

            if args.fail_fast {
                drop(_guard);
                exit(1);
            }
        }
    }

    formatter::generate(analytics, &args.formatter, &args.output);

    exit(if failed { 1 } else { 0 });
}
