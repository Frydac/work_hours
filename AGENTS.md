# Repository Guidelines

## Project Structure & Module Organization
`src/` contains the application code. `src/main.rs` boots the native or WASM app, `src/lib.rs` exposes the crate entry point, `src/app.rs` holds the main `eframe` application state, and `src/ui/` contains smaller UI-focused modules such as `day.rs` and `time_point.rs`. Utility or integration code lives alongside them, for example `src/supabase.rs`. Ad hoc binaries belong in `src/bin/`, such as `src/bin/test_supabase.rs`.

Static web assets live in `assets/`, while `index.html` and `Trunk.toml` define the Trunk-based web build. Notes and informal documentation go under `doc/`.

## Build, Test, and Development Commands
Use these commands from the repository root:

- `cargo run --release`: run the native desktop app.
- `trunk serve`: build and serve the WASM app locally at `http://127.0.0.1:8080`.
- `cargo check --workspace --all-targets`: fast compile check for normal targets.
- `cargo test --workspace --all-targets --all-features`: run unit, integration, and binary tests.
- `./check.sh`: run the full local CI sequence, including `fmt`, `clippy`, tests, doc tests, and `trunk build`.

## Coding Style & Naming Conventions
Follow standard Rust formatting: 4-space indentation and `cargo fmt` output as the source of truth. Keep module and function names in `snake_case`, types and traits in `CamelCase`, and constants in `SCREAMING_SNAKE_CASE`. Prefer small UI modules under `src/ui/` instead of growing `app.rs` further.

Run `cargo fmt --all` before opening a PR. `clippy` is enforced with `-D warnings`, so treat warnings as failures.

## Testing Guidelines
There is no large dedicated test tree yet, so add focused `#[test]` coverage near the code it validates or add targeted binaries under `src/bin/` when exercising external integrations. Keep test names descriptive, for example `parses_week_total_correctly`. Always run `./check.sh` before submitting changes, especially for WASM-facing edits.

## Commit & Pull Request Guidelines
Recent commits use short, lowercase summaries such as `update readme` and `upgrade egui/eframe from 0.30 to 0.31`. Keep commits small, specific, and written in the imperative mood. For pull requests, include a brief description of behavior changes, list commands you ran to verify the work, and attach screenshots when UI layout or rendering changes.
