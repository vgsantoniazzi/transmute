# transmute-rspec

Persistent RSpec runner daemon for [transmute](https://github.com/vgsantoniazzi/transmute) mutation testing.

Mutation testing shells out to your test runner once per mutation. On a Rails app, that's ~3–5 seconds of framework boot per `rspec` invocation, and it adds up fast — a benchmark on a real Rails 8 app (10 models, 151 mutations, 423 spec runs) took **16 minutes** of wall clock, almost all of it Rails boot.

This gem boots Rails once and accepts spec-run requests over a socket. Two execution modes (see "Two modes" below):

- **In-process** (default): same Ruby process across all spec runs. Fastest path: **6m41s on the reference bench, ~2.4× faster than cold rspec**. Trade-off: ~13% of kills drop into a low-confidence bucket because class-level Rails state leaks between specs.
- **Fork** (`--fork`): forks per request so each spec runs in clean state. Restores kill-rate parity with cold rspec at the cost of fork+reload overhead (~5s/spec vs ~1s in-process). Use when you need authoritative results in one pass.

## Installation

Add both `transmute-ruby` (coverage capture) and `transmute-rspec` (this gem, runner sidecar):

```ruby
group :test do
  gem 'transmute-ruby'
  gem 'transmute-rspec'
end
```

## Usage

**1. Start the daemon** (in a terminal, or backgrounded):

```sh
bundle exec transmute-rspec serve \
  --listen tcp://0.0.0.0:9876 \
  --require ./config/environment
```

The `--require` flag preloads Rails (or any other framework) so the first spec invocation doesn't pay boot cost.

You can also listen on a Unix socket:

```sh
bundle exec transmute-rspec serve --listen /tmp/transmute-rspec.sock --require ./config/environment
```

**2. Point transmute's `--command` at the client**:

```sh
transmute \
  --files "app/**/*.rb" \
  --coverage transmute.sqlite \
  --command "TRANSMUTE_RSPEC_SOCKET=tcp://localhost:9876 bundle exec transmute-rspec-run {file}" \
  --max-specs-per-mutation 3
```

The `transmute-rspec-run` client connects to the daemon, sends the spec path, forwards the response stdout, and propagates the exit code. To transmute it looks like any other `--command`.

**3. Kill the daemon** when done (Ctrl-C, or `kill <pid>`).

## How it works

```
┌─────────────┐    spec path    ┌──────────────────┐
│   engine    ├────────────────>│ transmute-rspec  │
│  (Rust)     │  (per mutation) │     daemon       │
│             │<────────────────┤  (long-running)  │
└─────────────┘  exit + stdout  └──────────────────┘
                                  │
                                  ├─ boots Rails once
                                  ├─ reloads changed source per request
                                  │   (Rails reloader if present;
                                  │    mtime-based otherwise)
                                  ├─ Kernel.load(spec_path)
                                  └─ RSpec::Core::Runner.run([])
```

Between requests:
- **Rails apps**: `Rails.application.reloader.reload!` clears Zeitwerk's loaded constants, so the next reference re-autoloads from disk. Picks up mutations cleanly.
- **Plain Ruby**: scans `$LOADED_FEATURES` for files whose mtime changed and `Kernel.load`s them. Has a known limitation — `Kernel.load` on a re-opened class adds new methods but doesn't remove old ones, so some mutations may falsely survive in non-Rails Ruby projects. Use the standard `bundle exec rspec` command (no sidecar) for plain Ruby.

## Protocol

Line-delimited JSON over Unix socket or TCP. One request per line:

```json
{"action":"run","spec":"spec/models/user_spec.rb"}
```

Response:

```json
{"exit_code":0,"stdout":"...rspec output..."}
```

Other actions: `ping`, `quit`.

## Two modes: in-process (default) vs fork

```sh
bundle exec transmute-rspec serve --listen tcp://0.0.0.0:9876 --require ./config/environment       # in-process
bundle exec transmute-rspec serve --listen tcp://0.0.0.0:9876 --require ./config/environment --fork  # fork-per-request
```

| | wall per spec (Rails 8) | kill-rate parity vs cold rspec |
|---|---|---|
| In-process (default) | ~1s warm | ~87% (drops ~13% of kills to low-confidence) |
| Fork (`--fork`) | ~5s | ~100% (each spec runs in fresh child) |
| Cold `bundle exec rspec` | ~7s | 100% |

In-process keeps Rails warm across all specs — fast but class-level state (memoized singletons, Faraday pools, instance vars on classes) leaks between specs and masks some mutations. `--fork` calls `Process.fork` per request: child re-establishes its DB connection, reloads changed source, runs the spec, exits. Fresh state per spec, but fork+reload+DB-reconnect in Ruby/Rails costs ~3-4s on top of the actual spec.

Pick by what matters:
- **Fast first pass** to triage missing tests → default (in-process). Promote any survivor with a cold re-run for confirmation.
- **Authoritative results in one pass** → `--fork`. Slower per spec, the report you get is trustworthy as-is.

## Caveats

- **State pollution in default mode**. Documented above. Use `--fork` if it matters.
- **Single writer**. One daemon per project. The daemon serves requests sequentially; concurrent clients will queue.
- **Memory growth**. Long-running Ruby processes leak; restart the daemon every few hundred requests.
- **`--jobs` parallel mode is not yet supported**. Each transmute worker would need its own daemon. Use serial mode (`--jobs 1`, the default) with the sidecar for now.
- **TCP listener has no authentication**. Use `tcp://127.0.0.1:PORT` to bind to localhost only, or use Unix sockets.

## Why a separate gem

Coverage capture (`transmute-ruby`) and runner sidecar (this gem) are independent concerns with different lifecycles and different test-framework bindings. Future minitest support would ship as `transmute-minitest`. Future pytest support would ship as `transmute-pytest`. Users install the two pieces that match their stack.

## License

GPL-3.0. See [LICENSE](../../LICENSE).
