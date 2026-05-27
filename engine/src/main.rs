use clap::Parser;
use log::{info, trace, warn};
use std::process::exit;
use std::time::Duration;

use transmute::file::ruby as ruby_mod;
use transmute::parallel;
use transmute::worktree;
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

    /// Coverage database path
    #[clap(long, default_value = "transmute.sqlite")]
    coverage: String,

    /// Cap how many specs run per mutation. Omit for unlimited (default; matches pre-0.2 semantics). For each (file, line), specs are ranked by how many lines of that file they cover (more = closer), and the top N run. Survivors produced under a cap are tagged low_confidence_failures.
    #[clap(long)]
    max_specs_per_mutation: Option<usize>,

    /// Number of parallel workers. 1 = serial (default). N > 1 partitions files across N git worktrees and runs them concurrently. Requires a clean working tree and coverage produced by transmute-ruby 0.3+.
    #[clap(long, default_value = "1")]
    jobs: usize,

    /// Shell command to run inside each worktree before its mutations start (e.g. "bundle install"). Only used with --jobs > 1.
    #[clap(long, default_value = "")]
    setup_command: String,

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
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, &args.log_level),
    );

    ctrlc::set_handler(|| {
        runner::kill_active_child();
        parallel::kill_active_workers();
        file::restore_active_mutations();
        worktree::cleanup_active_worktrees();
        exit(130);
    })
    .expect("Error setting Ctrl-C handler");

    if !["json", "html"].contains(&args.formatter.as_str()) {
        eprintln!(
            "transmute: unknown --formatter '{}'; valid: json, html",
            args.formatter
        );
        exit(2);
    }

    if args.jobs > 1 {
        info!(
            "Starting transmute (jobs={}, max-specs-per-mutation={:?}).",
            args.jobs, args.max_specs_per_mutation
        );
        run_parallel(&args);
    } else {
        info!(
            "Starting transmute (max-specs-per-mutation={:?}).",
            args.max_specs_per_mutation
        );
        run_serial(&args);
    }
}

fn run_parallel(args: &Args) -> ! {
    if args.fail_fast {
        warn!(
            "--fail-fast is ignored when --jobs > 1; workers can't cheaply signal each other yet"
        );
    }
    let setup = if args.setup_command.is_empty() {
        None
    } else {
        Some(args.setup_command.as_str())
    };

    match parallel::run(
        &args.files,
        &args.coverage,
        &args.command,
        &args.log_level,
        args.timeout,
        args.seed,
        args.max_specs_per_mutation,
        args.jobs,
        setup,
    ) {
        Ok(result) => {
            let failures = result.analytics.failures();
            formatter::generate(result.analytics, &args.formatter, &args.output);
            if result.any_worker_failed_to_produce_output {
                exit(2);
            }
            exit(if failures > 0 { 1 } else { 0 });
        }
        Err(e) => {
            eprintln!("transmute: {}", e);
            exit(2);
        }
    }
}

fn run_serial(args: &Args) -> ! {
    ruby_mod::init_rng(args.seed);

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
            let match_ =
                coverage.find(&file.path, mutable.line_number, args.max_specs_per_mutation);
            let specs = match_.specs;
            let specs_total = match_.total;
            if specs.is_empty() {
                warn!(
                    "No specs cover {}:{}; recording as surviving.",
                    file.path, mutable.line_number
                );
                analytics.add(&file.path, mutable, 0, String::new(), specs_total);
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
                analytics.add(&file.path, mutable, exit_code, stdout, specs_total);

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
