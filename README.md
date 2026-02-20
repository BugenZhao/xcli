# xcli

CLI for building and running Xcode projects from the terminal, aiming to simplify agentic development on Apple platforms. Supports both `.xcworkspace` and SPM `Package.swift`.

## Features

- Auto-detect `.xcworkspace` and `Package.swift` projects
- Interactive selection of workspace, scheme, configuration, and destination
- Cached selections for repeat builds — configure once, run many times
- Build, clean, and launch in one command
- Launch on simulators, physical devices, and macOS
- Pipe build output through [xcbeautify](https://github.com/cpisciotta/xcbeautify) when available
- Designed for headless / CI / agent-driven workflows

## Install

```sh
cargo install --git https://github.com/BugenZhao/xcli
```

Or build from source:

```sh
git clone https://github.com/BugenZhao/xcli.git
cd xcli
cargo install --path .
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
