# xcraft

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
cargo install xcraft
```

Or from the Git repository:

```sh
cargo install --git https://github.com/BugenZhao/xcraft
```

## Usage

```sh
# Show available commands
xcraft help

# Build and run (interactively selects workspace, scheme, destination on first use)
xcraft launch

# Build without launching
xcraft build

# Clean build products
xcraft clean

# Other commands...

# Interactively re-select workspace, scheme, configuration, and destination
xcraft configure

# List workspaces / schemes / configurations / destinations
xcraft workspaces
xcraft schemes
xcraft configs
xcraft destinations

# Clear cached selections
xcraft reset
```

All resolve options (workspace, scheme, configuration, destination) are cached in `.xcraft/state.toml` so you only need to select them once. Use `xcraft configure` to re-select, or `xcraft reset` to clear.

### Profiles

Use `--profile <name>` to maintain multiple configurations side by side. Each profile stores its selections in a separate file (`.xcraft/state.<name>.toml`).

```sh
# Set up a simulator profile
xcraft configure --profile sim --destination "simulator:..."

# Set up a device profile
xcraft configure --profile device --destination "device:..."

# Build with a specific profile
xcraft launch --profile sim
xcraft launch --profile device

# Clear a specific profile
xcraft reset --profile sim
```

Without `--profile`, the default `.xcraft/state.toml` is used as before.

## Acknowledgments

Inspired by [SweetPad](https://github.com/sweetpad-dev/sweetpad), a VSCode extension for Xcode development.

## License

[MIT](LICENSE)
