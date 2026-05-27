# Transmute

Mutation testing, written in Rust.

Your test suite is green. So what? Green means your tests didn't fail — not that they would catch a real bug. Transmute proves they would. It changes your code in small, plausible ways and runs your tests. If the tests still pass, the mutation **survived**, and the line it touched has no test that meaningfully covers it.

A surviving mutation is a missing test.

## What it does

Take an expression like `age >= 18`. Transmute rewrites it — `age > 18`, `age == 18`, `age != 18`, `age <= 18` — and runs your suite after each change. If the tests still pass after `age == 18`, you have no test for the boundary. Transmute reports the file, the line, the change, and the run that should have failed.

## Why it's fast

Most mutation testers run the whole suite for every mutation. Transmute doesn't. It reads a coverage map produced during your normal test run — a SQLite database that maps every line of source to the tests that exercise it — and runs only those tests.

A change on `src/billing.ext:42` runs only the tests that touched line 42. Nothing else.

The map is required. If a mutated line has no entry in it, Transmute reports the mutation as surviving — and rightly so, because no test covers the line.

On large codebases, the per-mutation spec list can still be long. `--max-specs-per-mutation N` caps how many specs run per mutation. For each `(file, line)`, specs are ranked by how many lines of the mutated file they cover (more = more focused on that file), and the top N run. Survivors produced under a cap are reported separately as `low_confidence_failures` so they can be distinguished from real survivors and from uncovered lines (`uncovered_failures`).

The engine is language-agnostic. A language plugs in with a mutation set and a coverage producer; files with an unrecognized extension are skipped with a warning.

## Install

Build the binary from source:

```sh
git clone git@github.com:vgsantoniazzi/transmute.git
cd transmute
make release
```

The binary is written to `engine/target/x86_64-unknown-linux-musl/release/transmute`.

The coverage map is produced by a small library that hooks into your test runner. See `coverage/` for available adapters and run your suite once with `COVERAGE=true` to write `transmute.sqlite`.

For Rails projects, there's also a persistent runner sidecar in `runners/rspec/` (`transmute-rspec` gem) that boots Rails once and accepts spec-run requests over a socket, replacing per-mutation `bundle exec rspec` invocations. Benchmark on a real Rails 8 app: 16 minutes → 7.5 seconds (~130× speedup). See `runners/rspec/README.md`.

## Usage

```sh
transmute \
  --files "src/**/*" \
  --coverage transmute.sqlite \
  --command "<your test runner> {file}" \
  --formatter html
```

| Flag | Purpose |
|------|---------|
| `--files` | Glob of files to mutate. Append `:N` to target a single line, e.g. `app/models/user.rb:42`. |
| `--command` | Shell command that runs a single test file. `{file}` is replaced with each affected test path. |
| `--coverage` | Path to the coverage database. Defaults to `transmute.sqlite`. |
| `--max-specs-per-mutation` | Cap how many specs run per mutation. `0` = unlimited (default; matches pre-0.2 semantics). |
| `--jobs` | Number of parallel workers. `1` = serial (default). N>1 partitions files across N git worktrees and runs them concurrently. |
| `--setup-command` | Shell command to run inside each worktree before its mutations start (e.g. `"bundle install"`). Only used with `--jobs > 1`. |
| `--formatter` | `json` or `html`. Defaults to `json`. |
| `--fail-fast` | Exit on the first surviving mutation. Ignored under `--jobs > 1`. |
| `--log-level` | `trace`, `debug`, `info`, `warn`, `error`. Defaults to `info`. |

Transmute exits non-zero if any mutation survives. Wire it into CI and the build fails the moment your suite stops catching real changes.

### Capping specs per mutation

`--max-specs-per-mutation N` caps how many specs the engine runs for each mutation. For each `(file, line)` covered by more than N specs, the engine ranks covering specs by how many lines of the mutated file they cover (more = more focused on that file) and runs the top N. Default `0` means unlimited (all covering specs run).

Suggested starting points:

- `--max-specs-per-mutation 10` for mid-size codebases where the full spec set per line is impractical
- `--max-specs-per-mutation 3` for large codebases where you want a fast triage signal
- Default (unlimited) for authoritative runs and CI gates

Per-mutation JSON includes `specs_total` (how many specs cover the line in the database). When `specs_total > specs_run`, the survivor is counted under `low_confidence_failures` in the report header. When `specs_total == 0`, the survivor is counted under `uncovered_failures` instead — the line has no spec at all.

To confirm a low-confidence survivor, re-run the engine targeted at that line with no cap: `--files app/models/user.rb:42 --max-specs-per-mutation 0`.

### Parallel mutation runs

`--jobs N` (with N>1) partitions the input file glob across N git worktrees and runs them concurrently. Each worker mutates and tests in its own isolated source tree, so concurrent mutations never step on each other.

```sh
transmute \
  --files "app/**/*.rb" \
  --coverage transmute.sqlite \
  --command "bin/rspec {file}" \
  --jobs 4 \
  --setup-command "bundle install --quiet"
```

Preconditions and behavior:

- **Clean working tree required.** Worktrees are created from `HEAD`; uncommitted work would not be tested. The engine refuses with a clear error if `git status --porcelain` is non-empty.
- **Coverage must include capture-time cwd.** The engine translates source paths from runtime cwd (the worktree) back to the cwd recorded by the coverage gem. Requires `transmute-ruby 0.3+`. Older coverage DBs fail with a clear migration message.
- **`--setup-command` runs once per worktree** before mutations begin (e.g. `bundle install`, `npm ci`). Required if your test runner needs per-tree dependencies; skip it if your runtime can share dependencies across trees.
- **Disk and RAM scale with `--jobs`.** N workers ≈ N× source tree on disk and N× test-runner memory. Default `--jobs 1`; tune up cautiously.
- **`--fail-fast` is ignored under `--jobs > 1`.** Workers can't cheaply signal each other yet; planned for a later release.
- **Worktrees live in `$TMPDIR/transmute-worker-<pid>-w<n>/`** and are removed on success. A failed setup leaves the worktree on disk for inspection.

### Parallel test runners (coverage capture)

The coverage gem writes one SQLite file per test process. The engine reads one file per run.

- Single-process suites (default `rspec`): the gem writes `transmute.sqlite` atomically (tmpfile + rename), so an interrupted run never leaves a half-written DB.
- Parallel runners (`parallel_tests`, `knapsack_pro`): each worker that sets `TEST_ENV_NUMBER` writes to `transmute.${TEST_ENV_NUMBER}.sqlite`. The engine does not yet read across multiple files; until then, capture coverage with a non-parallelized run, or merge per-worker files manually before invoking the engine. Multi-file ingest is planned for a later release.

## Migrating

### From 0.2 to 0.3

Coverage format is unchanged; existing `transmute.sqlite` files keep working for serial runs. To use `--jobs > 1`, upgrade `transmute-ruby` to `0.3.0+` and regenerate `transmute.sqlite` — the new gem version writes the capture-time cwd into `schema_meta`, which the engine needs to translate paths inside worktrees. Older coverage DBs raise a clear error when used with `--jobs > 1`.

### From 0.1 to 0.2

Transmute 0.2 dropped the JSON coverage format.

1. Update the `transmute-ruby` gem to `0.2.0` or higher — it writes `transmute.sqlite` and depends on the `sqlite3` Ruby gem (`libsqlite3-dev` headers must be available on your build hosts).
2. Re-run your test suite with `COVERAGE=true` to produce a fresh `transmute.sqlite`.
3. Update any pipeline that referenced `transmute.json` to point at `transmute.sqlite`.

The engine refuses to load `.json` coverage files with a migration error pointing you here.

## What it mutates

- **Strings** — replaced with a random string
- **Numbers** — replaced with a random integer
- **Comparison operators** — `>`, `<`, `>=`, `<=` rotated
- **Equality operators** — `==` flipped to `!=` and back

Constructs that look like comparisons but aren't — class declarations, append operators, and the like — are left alone where the adapter knows about them.

## Status

Transmute is early. The engine works and is in active development. Richer mutation operators — boolean flips, branch removal, method call rewriting — are not implemented yet, and new adapters are landing.

If you want to run it against a real codebase, expect to read the source. If you want to help, see below.

## Development

```sh
make build         # debug build
make test          # cargo test
make format        # cargo fmt
make run.dummy     # run against the fixture project in engine/tests/fixtures
```

The Rust engine lives in `engine/`. Coverage adapters live under `coverage/`. Runner sidecars live under `runners/`. Engine tests are integration tests under `engine/tests/`.

Pull requests welcome. Squash before merging.

## License

GPL v3. See [LICENSE](LICENSE).
