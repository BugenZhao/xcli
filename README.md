# xcli

CLI for building and running Xcode projects from the terminal, aiming to simplify agentic development on Apple platforms. Supports `.xcworkspace`, SPM `Package.swift`, and Tuist `Project.swift`.

## Features

- Auto-detect `.xcworkspace`, `Package.swift`, and Tuist `Project.swift` projects
- Tuist integration — automatically runs `tuist generate` before building
- Interactive selection of workspace, scheme, configuration, and destination
- Cached selections for repeat builds — configure once, run many times
- Named profiles (`--profile`) — maintain multiple configurations side by side
- Build, clean, and launch in one command
- Launch on simulators, physical devices, and macOS
- Pipe build output through [xcbeautify](https://github.com/cpisciotta/xcbeautify) when available
- Designed for headless / CI / agent-driven workflows

## Install

```sh
cargo install xcli
```

Or from the Git repository:

```sh
cargo install --git https://github.com/BugenZhao/xcli
```

## Usage

```sh
# Show available commands
xcli help

# Build and run (interactively selects workspace, scheme, destination on first use)
xcli launch

# Build without launching
xcli build

# Clean build products
xcli clean

# Other commands...

# Interactively re-select workspace, scheme, configuration, and destination
xcli configure

# List workspaces / schemes / configurations / destinations
xcli workspaces
xcli schemes
xcli configs
xcli destinations

# Clear cached selections
xcli reset
```

All resolve options (workspace, scheme, configuration, destination) are cached in `.xcli/state.toml` so you only need to select them once. Use `xcli configure` to re-select, or `xcli reset` to clear.

### Profiles

Use `--profile <name>` to maintain multiple configurations side by side. Each profile stores its selections in a separate file (`.xcli/state.<name>.toml`).

```sh
# Set up a simulator profile
xcli configure --profile sim --destination "simulator:..."

# Set up a device profile
xcli configure --profile device --destination "device:..."

# Build with a specific profile
xcli launch --profile sim
xcli launch --profile device

# Clear a specific profile
xcli reset --profile sim
```

Without `--profile`, the default `.xcli/state.toml` is used as before.

## Acknowledgments

Inspired by [SweetPad](https://github.com/sweetpad-dev/sweetpad), a VSCode extension for Xcode development.

## License

[MIT](LICENSE)
