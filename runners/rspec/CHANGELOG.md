# Changelog

Notable changes to `transmute-rspec`. This project adheres to [Semantic Versioning](https://semver.org).

## 0.1.0

- Initial release. Persistent RSpec runner daemon: boots Rails once and serves spec-run requests over a Unix socket or loopback TCP, replacing per-mutation `bundle exec rspec` invocations.
- In-process (default) and `--fork` execution modes.
- Stdlib-only `transmute-rspec-run` client.
- Loopback-only bind by default, `0600` Unix socket, project-root path confinement, request read-timeout (`--read-timeout`), and request-size cap.
