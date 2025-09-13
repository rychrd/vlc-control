# WARP.md

This file provides guidance to WARP (warp.dev) when working with code in this repository.

## Project Overview

This is a VLC Controller application written in Rust that acts as a network proxy for controlling VLC media player remotely. The application runs concurrent TCP and UDP servers that accept commands and either process them locally (for system commands) or forward them to a VLC instance running with its RC (Remote Control) interface enabled.

## Development Commands

### Building and Running
```bash
# Build the project
cargo build

# Build with optimizations (release mode)
cargo build --release

# Run the application
cargo run

# Check code without building (faster than full build)
cargo check
```

### Testing and Development
```bash
# Run all tests
cargo test

# Run tests with output shown
cargo test -- --nocapture

# Run specific test
cargo test <test_name>

# Format code
cargo fmt

# Check formatting without making changes
cargo fmt --check

# Run Clippy linter
cargo clippy

# Run Clippy with strict warnings
cargo clippy -- -D warnings
```

### Testing the Application
```bash
# Test TCP interface (in another terminal)
echo "pause" | nc localhost 55550

# Test UDP interface (in another terminal)  
echo "play" | nc -u localhost 55551

# Connect to VLC directly for testing
telnet localhost 54322
```

### Logging and Debugging
```bash
# Run with different log levels
RUST_LOG=info cargo run     # Default: INFO and above
RUST_LOG=debug cargo run    # All logs including debug traces
RUST_LOG=warn cargo run     # Only warnings and errors

# Run the logging demo script
./demo_logging.sh
```

## Architecture

### Network Architecture
The application implements a **dual-protocol proxy server** pattern:

- **TCP Server** (port 55550): Handles persistent connections with line-based protocol
- **UDP Server** (port 55551): Handles datagram-based commands  
- **VLC Connection** (port 54322): Forwards commands to VLC's RC interface

### Concurrency Model
Uses **Tokio async runtime** with:
- `tokio::select!` for running multiple servers concurrently
- `tokio::spawn` for handling multiple TCP clients simultaneously
- Async I/O throughout to prevent blocking

### Command Processing Flow
1. Commands received on TCP/UDP servers
2. **System commands** (`pi_restart_vlc`, `pi_shutdown`, `pi_reboot`) executed locally via `std::process::Command`
3. **VLC commands** forwarded to VLC's RC interface at `127.0.0.1:54322`
4. Responses sent back to clients (TCP only)

## Key Components

### Server Components
- `run_tcp_server()`: Accepts TCP connections, spawns handler tasks
- `run_udp_server()`: Processes UDP datagrams in event loop
- `handle_tcp_connection()`: Manages individual TCP client lifecycle

### Command Processing
- `process_command()`: Central dispatcher for all commands
- `forward_to_vlc()`: Handles VLC RC interface communication
- Pattern matching on specific system commands vs. VLC passthrough

### Important Implementation Details
- **Ownership Handling**: TCP connections split into reader/writer halves for concurrent access
- **Error Propagation**: Uses `anyhow::Result` for simplified error handling
- **Async I/O**: All network operations use Tokio's async primitives
- **Structured Logging**: Uses `tracing` for structured, async-aware logging with configurable levels

## VLC Integration Requirements

### VLC Setup
VLC must be running with RC interface enabled:
```bash
# Start VLC with RC interface
vlc --intf rc --rc-host 127.0.0.1:54322
```

### Systemd Service (Raspberry Pi)
The code references `vlc-loader.service` for VLC process management:
- Service should run VLC with RC interface enabled
- Must be user service (uses `--user` flag)

## Network Configuration

### Port Usage
- **55550/TCP**: Main command interface (persistent connections)
- **55551/UDP**: Datagram command interface
- **54322/TCP**: VLC RC interface (outbound connections)

### Security Considerations
- Servers bind to `0.0.0.0` (all interfaces) - consider firewall rules
- System commands (`sudo shutdown`) require appropriate privileges
- No authentication/authorization implemented

## Dependencies

- **tokio**: Async runtime with full feature set
- **anyhow**: Simplified error handling
- **tracing**: Structured, async-aware logging framework
- **tracing-subscriber**: Log formatting and filtering (with env-filter feature)
- Standard library: process execution, networking

## Common Development Patterns

When adding new commands:
1. Add pattern match case in `process_command()`
2. For system commands: use `std::process::Command`
3. For VLC commands: let them fall through to `forward_to_vlc()`

When modifying network protocols:
- TCP: modify `handle_tcp_connection()` for protocol changes
- UDP: modify `run_udp_server()` for datagram handling
- Both use the same `process_command()` dispatcher
