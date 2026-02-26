
# Drive Space Analyzer

## Vision

Drive Space Analyzer aims to build a high-performance, formally-specified filesystem analysis engine.  
The project treats filesystem traversal as a deterministic event stream and builds mathematically sound aggregation layers on top of it.

The long-term goal is to provide precise, composable, and verifiable disk space analytics without relying on ad-hoc or opaque tooling.

---

## Engineering

This repository contains a Rust workspace:

- `dsa-core` — traversal engine and event model
- `dsa-cli` — command-line interface

The architecture emphasizes:
- Typed traversal events
- Structured error semantics
- Streaming design (no full-tree materialization)
- Clear separation between engine and interface

This readme is intentionally high-level; current, detailed specifications and mathematical models are maintained elsewhere.

---

## Build, Test and Run the CLI

```bash
# build
cargo build

# test
cargo test

# run the cli
cargo run -p dsa-cli

# create a release build
cargo build --release
```