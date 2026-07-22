# Build & Run Guide

## Quick Start

### Compile and Run (Debug)
```bash
cargo run
```

### Compile and Run (Release)
```bash
cargo run --release
```

## Build Only

### Debug Build
```bash
cargo build
```

### Release Build
```bash
cargo build --release
```

## Running with Commands

The MCP server starts in server mode by default. To run specific commands:

```bash
# List all available tools
cargo run -- tools

# Convert a file to markdown
cargo run -- convert <filepath>

# Search vault content
cargo run -- search <query>

# View help
cargo run -- --help
```

## Testing

Run the full test suite:
```bash
cargo test
```

Run tests for a specific module:
```bash
cargo test <module_name>
```

Run a specific test:
```bash
cargo test <test_name> -- --exact
```

## Development Notes

- **Debug mode** (`cargo run`): Faster compilation, slower execution, better debugging
- **Release mode** (`cargo run --release`): Slower compilation, optimized execution
- **Tests**: 302 tests across lib, main, CLI, and JSON-RPC integration tests
