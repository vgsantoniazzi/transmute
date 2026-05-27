use clap::Parser;
use log::{info, trace, warn};
use std::process::exit;
use std::time::Duration;

use transmute::file::ruby as ruby_mod;
use transmute::{analytics, coverage, file, formatter, runner};

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

    /// seed for the mutation RNG (0 = entropy); use for reproducible runs
    #[clap(long, default_value = "0")]
    seed: u64,
}

fn main() {
    let args = Args::parse();
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, args.log_level),
    );

    ctrlc::set_handler(|| {
        file::restore_active_mutations();
        exit(130);
    })
    .expect("Error setting Ctrl-C handler");

    info!("Starting transmute.");

    ruby_mod::init_rng(if args.seed == 0 {
        None
    } else {
        Some(args.seed)
    });

    if !["json", "html"].contains(&args.formatter.as_str()) {
        eprintln!(
            "transmute: unknown --formatter '{}'; valid: json, html",
            args.formatter
        );
        exit(2);
    }

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
    let mut total_runs: usize = 0;
    let mut infra_runs: usize = 0;

    info!("Running transmute for files. It can take several minutes..");

    for file in files.iter() {
        'mutate: for mutable in file.mutable_items.iter() {
            let specs = coverage.find(&file.path, mutable.line_number);
            if specs.is_empty() {
                warn!(
                    "No specs cover {}:{}; recording as surviving.",
                    file.path, mutable.line_number
                );
                analytics.add(&file.path, mutable, 0, String::new());
                failed = true;
                if args.fail_fast {
                    formatter::generate(analytics, &args.formatter, &args.output);
                    exit(1);
                }
                continue 'mutate;
            }

            let _guard = match file::MutationGuard::apply(&file.path, mutable) {
                Ok(g) => g,
                Err(e) => {
                    warn!(
                        "Could not apply mutation to {}: {}; skipping.",
                        file.path, e
                    );
                    continue 'mutate;
                }
            };

            for spec_file in specs.iter() {
                let (exit_code, stdout) =
                    runner::run(&args.command, spec_file, Duration::from_secs(args.timeout));

                trace!("{}", stdout);
                analytics.add(&file.path, mutable, exit_code, stdout);

                total_runs += 1;
                if runner::is_infra_error(exit_code) {
                    infra_runs += 1;
                    warn!(
                        "Runner returned infra exit {} for spec {}; treating mutation as not killed.",
                        exit_code, spec_file
                    );
                    continue;
                }
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
                formatter::generate(analytics, &args.formatter, &args.output);
                exit(1);
            }
        }
    }

    if total_runs > 0 && infra_runs == total_runs {
        warn!(
            "Every test run returned an infra exit code ({} of {}); your --command probably can't execute. Report is inconclusive.",
            infra_runs, total_runs
        );
    }

    formatter::generate(analytics, &args.formatter, &args.output);

    exit(if failed { 1 } else { 0 });
}
