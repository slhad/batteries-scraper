# Repository Guidelines

## Project Structure & Module Organization

This repository is a small Rust service with a flat layout:

- `src/main.rs` contains the HTTP server, battery parsing, XML generation, systemd user-service installer, and unit tests.
- `Cargo.toml` defines package metadata and dependencies.
- `README.md` documents runtime usage and Home Assistant integration.
- `.gitignore` excludes local build artifacts and runtime logs from commits.
- `target/` contains build artifacts and should not be edited manually.
- `server.log` is runtime output from local testing and should be treated as disposable.

Keep new logic close to existing helpers unless the file becomes hard to navigate, then split into modules under `src/`.

## Build, Test, and Development Commands

- `cargo build --release`
  Builds the production binary.
- `cargo test`
  Runs the unit test suite.
- `cargo fmt --check`
  Verifies Rust formatting.
- `cargo fmt`
  Applies standard formatting.
- `cargo run`
  Starts the server with default bind/device settings.
- `cargo run -- --install-systemd-user`
  Installs and enables the user service.

Example local override:

```bash
BIND_ADDR=0.0.0.0:12321 cargo run
```

## Coding Style & Naming Conventions

Use standard Rust formatting via `cargo fmt`. Prefer small pure helper functions for parsing and rendering so they are easy to test. Use:

- `snake_case` for functions and variables
- `SCREAMING_SNAKE_CASE` for constants
- short, explicit error messages intended for logs

Do not hand-edit generated files in `target/`, and do not commit `target/` or `server.log`.

## Testing Guidelines

Unit tests live in `src/main.rs` under `#[cfg(test)]`. Add tests for every change to:

- `upower` output parsing
- XML payload shape
- service file generation
- response content types

Name tests to describe behavior, for example `parse_battery_percentage_rejects_missing_percentage`.

## Commit & Pull Request Guidelines

Use short imperative commit messages such as:

- `Add systemd user-service installer`
- `Match LGSTrayBattery XML response format`
- `Add unit tests for XML payload generation`

PRs should include:

- a short summary of behavior changes
- verification steps (`cargo test`, manual curl checks, HA validation)
- any firewall, systemd, or Home Assistant changes required for deployment
- note whether generated or local-only files were intentionally excluded from the commit

## Security & Configuration Tips

This service exposes battery data over HTTP. If Home Assistant runs on another host, allow only the needed network path, for example `12321/tcp`, and prefer LAN-only binding where possible.
