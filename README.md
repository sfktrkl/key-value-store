# Key-Value Store

### Rust Installation

Install [Rust](https://rustup.rs/):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Verify:

```bash
rustc --version
```

### Dependencies

Rust package manager will automatically download the dependencies.

- [`tokio`](https://crates.io/crates/tokio): An event-driven, non-blocking I/O platform.
- [`serde`](https://crates.io/crates/serde): Serialization and deserialization framework.
- [`serde_json`](https://crates.io/crates/serde_json): JSON serialization file format.

### Build and Run the Program

```bash
cargo run
```

### Running Multiple Nodes

To run multiple nodes locally for replication, start each instance on a different port:

```bash
cargo run -- --port 5000
cargo run -- --port 5001
cargo run -- --port 5002
```
