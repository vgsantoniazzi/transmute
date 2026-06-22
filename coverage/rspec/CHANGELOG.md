# Changelog

Notable changes to `transmute-ruby`. This project adheres to [Semantic Versioning](https://semver.org).

## 0.3.1

- Refactor internals from `class Transmute` to `module Transmute` with a `Recorder` class, so the gem can share the `Transmute` namespace with `transmute-rspec` in one process. Public API (`Transmute.start`, `Transmute.add_coverage`, `Transmute.store!`) is unchanged.
- Guard `Coverage.start` against double-start when the library is required more than once.
- Add unit specs for the recorder under `spec/`.
- Clarify the license as GPL-3.0.

## 0.3.0

- Record the capture-time working directory in `schema_meta` so the engine can translate paths inside `--jobs > 1` worktrees. Older coverage databases raise a clear error when used with parallel jobs.

## 0.2.0

- Switch the coverage format from JSON to a normalized `transmute.sqlite` database. Adds a runtime dependency on `sqlite3` (`libsqlite3-dev` headers required on the build host). The engine refuses `.json` coverage files with a migration error.

## 0.1.0

- Initial release. JSON reverse-coverage map.
