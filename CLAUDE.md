# Acacia

Optimized Rust Minecraft Bedrock client library. Design and decisions: see docs/DESIGN.md.

- Crate dependencies point downward only (see DESIGN.md); `acacia-raknet`, `acacia-proto` and `acacia-session` stay network-free: no sockets, no async, no tokio.
- `acacia-proto` sources are generated — edit `tools/codegen`, never the output.
- Background `cargo build`/`cargo test` of the whole workspace; use `-p <crate>` for single-crate checks.
