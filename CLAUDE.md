# sweetpad-cli

CLI equivalent of the SweetPad VSCode extension for building and running Xcode projects (`.xcworkspace` and SPM `Package.swift`) from the terminal.

## Tech Stack

- Rust (edition 2024), binary name `sweetpad`
- Main dependencies
  - clap (CLI parsing)
  - dialoguer (interactive prompts)
  - serde/serde_json (JSON parsing)
  - walkdir (file discovery)
  - anyhow (error handling)
  - tempfile

## Project Structure

- `src/main.rs` — CLI entry point, clap subcommands (`detect`, `schemes`, `configs`, `destinations`, `launch`)
- `src/workspace.rs` — workspace detection and resolution (depth-4 scan for `.xcworkspace` / `Package.swift`)
- `src/scheme.rs` — scheme and configuration listing/resolution (SPM via `swift package dump-package`, Xcode via `xcodebuild -list`)
- `src/destination.rs` — destination listing (simulators via `simctl`, physical devices via `devicectl`, macOS)
- `src/build.rs` — `xcodebuild build` execution + build settings extraction + optional xcbeautify pipe
- `src/launch.rs` — app launch by destination type (macOS direct exec, simulator simctl install/launch, device devicectl install/launch)
- `src/util.rs` — command execution helpers + fault-tolerant JSON parsing (handles non-JSON prefixes in xcodebuild output)
- `docs/build-run-launch-flow.md` — detailed flow documentation based on SweetPad extension source (Chinese)

## Build

```sh
cargo build
cargo run -- launch
```

## Development Workflow

After finishing a task, run `cargo fmt` and `cargo clippy` to ensure formatting and lint compliance.
