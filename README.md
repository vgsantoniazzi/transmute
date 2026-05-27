# Transmute

Mutation testing, written in Rust.

Your test suite is green. So what? Green means your tests didn't fail — not that they would catch a real bug. Transmute proves they would. It changes your code in small, plausible ways and runs your tests. If the tests still pass, the mutation **survived**, and the line it touched has no test that meaningfully covers it.

A surviving mutation is a missing test.

## What it does

Take an expression like `age >= 18`. Transmute rewrites it — `age > 18`, `age == 18`, `age != 18`, `age <= 18` — and runs your suite after each change. If the tests still pass after `age == 18`, you have no test for the boundary. Transmute reports the file, the line, the change, and the run that should have failed.

## Why it's fast

Most mutation testers run the whole suite for every mutation. Transmute doesn't. It reads a coverage map produced during your normal test run — a JSON file that maps every line of source to the tests that exercise it — and runs only those tests.

A change on `src/billing.ext:42` runs only the tests that touched line 42. Nothing else.

The map is required. If a mutated line has no entry in it, Transmute reports the mutation as surviving — and rightly so, because no test covers the line.

The engine is language-agnostic. A language plugs in with a mutation set and a coverage producer; files with an unrecognized extension are skipped with a warning.

## Install

Build the binary from source:

```sh
git clone git@github.com:vgsantoniazzi/transmute.git
cd transmute
make release
```

The binary is written to `engine/target/x86_64-unknown-linux-musl/release/transmute`.

The coverage map is produced by a small library that hooks into your test runner. See `coverage/` for available adapters and run your suite once with `COVERAGE=true` to write `transmute.json`.

## Usage

```sh
transmute \
  --files "src/**/*" \
  --coverage transmute.json \
  --command "<your test runner> {file}" \
  --formatter html
```

| Flag | Purpose |
|------|---------|
| `--files` | Glob of files to mutate. Append `:N` to target a single line, e.g. `app/models/user.rb:42`. |
| `--command` | Shell command that runs a single test file. `{file}` is replaced with each affected test path. |
| `--coverage` | Path to the coverage map. Defaults to `transmute.json`. |
| `--formatter` | `json` or `html`. Defaults to `json`. |
| `--fail-fast` | Exit on the first surviving mutation. |
| `--log-level` | `trace`, `debug`, `info`, `warn`, `error`. Defaults to `info`. |

Transmute exits non-zero if any mutation survives. Wire it into CI and the build fails the moment your suite stops catching real changes.

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

The Rust engine lives in `engine/`. Test-runner adapters live under `coverage/`. Engine tests are integration tests under `engine/tests/`.

Pull requests welcome. Squash before merging.

## License

GPL v3. See [LICENSE](LICENSE).
