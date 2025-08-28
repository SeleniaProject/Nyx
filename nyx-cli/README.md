# Nyx CLI

Command line interface for interacting with Nyx daemon and network operations.

## Features

- **Daemon Management**: Start, stop, and configure Nyx daemon instances
- **Network Operations**: Create circuits, send messages, manage streams
- **Configuration**: Load and manage configuration files
- **Monitoring**: View network status, performance metrics, and logs
- **I18n Support**: Multi-language interface support

## Installation

```bash
cargo build --release
./target/release/nyx-cli --help
```

## Usage

### Basic Commands

```bash
# Show daemon information
nyx-cli info

# Reload configuration
nyx-cli reload

# Create a new circuit
nyx-cli circuit create

# Send a message
nyx-cli send --target <address> --message "Hello, Nyx!"

# Monitor network status
nyx-cli status

# View logs
nyx-cli logs --follow
```

### Configuration

The CLI reads configuration from:
- Command line arguments (highest priority)
- Environment variables
- Configuration files (`~/.nyx/config.toml`, `/etc/nyx/config.toml`)

### Authentication

The CLI supports multiple authentication methods:
- API tokens (`--token` or `NYX_TOKEN` environment variable)
- Unix socket authentication (default on Unix systems)
- Named pipe authentication (default on Windows)

## Architecture

The CLI communicates with the Nyx daemon through:
- **gRPC** (when available, currently disabled to avoid C dependencies)
- **Pure Rust IPC** (default implementation)
- **JSON-RPC over Unix sockets/Named pipes**

## API Bindings

Previously, this crate included generated gRPC API bindings. These have been removed
to eliminate dependencies on `ring` and other C libraries. The CLI now uses a pure
Rust communication protocol with the daemon.

If gRPC support is needed in the future, the bindings can be regenerated using:
```bash
# This is currently disabled
# protoc --rust_out=src/ --grpc_out=src/ --plugin=protoc-gen-grpc=`which grpc_rust_plugin` api.proto
```

## Development

### Building

```bash
cargo build
```

### Testing

```bash
cargo test
```

### Running with specific endpoints

```bash
# Connect to specific daemon endpoint
nyx-cli --endpoint "127.0.0.1:9090" info

# Use custom timeout
nyx-cli --timeout-ms 5000 status
```

## Platform Support

- **Linux**: Unix domain sockets, systemd integration
- **macOS**: Unix domain sockets, launchd integration  
- **Windows**: Named pipes, Windows service integration
- **FreeBSD/OpenBSD**: Unix domain sockets

## Security

The CLI implements several security measures:
- Token-based authentication
- Encrypted communication channels
- Secure configuration file handling
- Input validation and sanitization

## License

Licensed under either of:
- Apache License, Version 2.0
- MIT License

at your option.
