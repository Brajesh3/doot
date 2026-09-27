# Doot (दूत)

[![CI](https://github.com/Brajesh3/doot/actions/workflows/ci.yml/badge.svg)](https://github.com/Brajesh3/doot/actions/workflows/ci.yml)
[![Rust Edition](https://img.shields.io/badge/rust-2024_edition-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

A sovereign, zero-server peer-to-peer (P2P) messaging and file/folder transfer application built on [Iroh](https://iroh.computer) QUIC, featuring real-time connection telemetry, desktop GUI via [egui](https://github.com/emilk/egui), and mobile client via [Jetpack Compose](https://developer.android.com/compose).

---

## Overview

Doot (*Sanskrit/Hindi for "messenger" or "emissary"*) operates entirely without central servers. Every instance acts as an independent cryptographic node identified by an Ed25519 keypair. Peer communication occurs directly over encrypted QUIC UDP connections with automatic NAT hole punching and fallback to DERP relays when peers are behind symmetric firewalls.

### Key Capabilities

- **Protocol Multiplexing**: Uses `iroh::protocol::Router` to multiplex official ecosystem protocols over a single QUIC endpoint and UDP socket:
  - `iroh-blobs`: Content-addressed, chunked file and streaming transfer.
  - `iroh-gossip`: Epidemic broadcast topic pub/sub.
  - `iroh-ping`: Low-level sub-millisecond round-trip latency measurement.
  - `DOOT_CHAT_ALPN` (`/doot/chat/1`): Bi-directional wire messaging protocol.
- **Connection Telemetry**: Live network path detection displaying direct P2P connections (`Direct { addr, rtt_ms }` via UDP) versus relayed connections (`Relay { url, rtt_ms }` via DERP), with real-time RTT metrics and active path migration tracking.
- **File and Directory Transfer**: Streaming file transfer and directory packaging preserving folder structures, verified against path traversal vulnerabilities.
- **Decoupled Architecture**: Headless core engine (`message-core`) shared between a hardware-accelerated desktop client (`message-desktop`) and an Android client (`android-test` via `message-android` JNI).
- **Offline Peering**: One-click shareable ticket format (`doot:...`) encoding node public key, relay URLs, and direct socket addresses (with backward compatibility for `iroh-msg:...`).

---

## Workspace Layout

```
├── Cargo.toml                       # Root workspace manifest (Rust edition 2024, resolver 3)
├── crates/
│   ├── message-core/                # Platform-agnostic P2P networking and state engine
│   │   ├── src/
│   │   │   ├── node.rs              # RouterBuilder, connection pool, path watcher
│   │   │   ├── events.rs            # ConnectionType telemetry and event channels
│   │   │   ├── protocol.rs          # Wire message serialization and ticket codec
│   │   │   ├── store.rs             # Local JSON-backed contact and message persistence
│   │   │   ├── folder.rs            # Directory archive pack/unpack utilities
│   │   │   └── identity.rs          # Ed25519 cryptographic key management
│   │   └── tests/
│   │       └── integration_tests.rs # Full multi-node integration test suite
│   ├── message-desktop/             # Hardware-accelerated egui / eframe desktop GUI
│   ├── message-android/             # Android JNI bindings for message-core (cdylib)
│   └── message-bot/                 # Headless CLI echo and testing bot
├── android-test/                    # Native Android Jetpack Compose application
│   └── app/src/main/
│       ├── java/.../messenger/      # JNI bridge and reactive StateFlow ViewModel
│       ├── java/.../ui/chat/        # Compose Chat UI with connection telemetry
│       └── jniLibs/arm64-v8a/       # Native arm64 libmessage_jni.so
└── .github/workflows/               # GitHub Actions CI/CD workflows
    ├── ci.yml                       # Multi-platform quality gate & artifact uploads
    ├── release.yml                  # Tagged multi-platform release publisher
    └── security.yml                 # Cargo audit security scanner
```

---

## Building and Running

### Prerequisites

- **Rust**: 1.85+ (Edition 2024 support)
  ```bash
  rustup update stable
  ```
- **Linux GUI dependencies** (Linux desktop only):
  ```bash
  sudo apt-get install -y libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev pkg-config
  ```
- **Android SDK & NDK** (Android builds only): NDK `r27c` or higher, JDK 21.

---

### Desktop Application

To compile and launch the desktop client:

```bash
# Debug build
cargo run -p message-desktop

# Optimized release build
cargo run --release -p message-desktop
```

Pre-compiled release binaries for Windows and Linux are automatically built and uploaded on every commit via GitHub Actions.

---

### Headless Bot

For automated testing and headless peering:

```bash
cargo run -p message-bot
```

The bot initializes an ephemeral Iroh node and prints a ticket (`doot:...`). Connecting to this ticket from any client enables automated echo replies, typing simulation, and file mirror testing over QUIC.

---

### Android Client

1. **Compile the native JNI library**:
   ```bash
   cargo build --target aarch64-linux-android -p message-android --release
   mkdir -p android-test/app/src/main/jniLibs/arm64-v8a
   cp target/aarch64-linux-android/release/libmessage_jni.so android-test/app/src/main/jniLibs/arm64-v8a/
   ```

2. **Build and install the APK**:
   ```bash
   cd android-test
   ./gradlew assembleDebug
   adb install -r app/build/outputs/apk/debug/app-debug.apk
   ```

---

## Testing

Run unit tests and multi-node integration test scenarios (spawns isolated local QUIC nodes, connects via ticket, and verifies text and binary transfer):

```bash
# Run all workspace tests
cargo test --workspace

# Run with full trace logging
RUST_LOG=info cargo test --workspace -- --nocapture
```

Verify formatting and strict linter compliance:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

---

## AI Disclaimer

> [!NOTE]
> This project, including its codebase, architecture, tests, and documentation, was developed with the assistance of AI pair-programming tools under human direction, review, and verification.

---

## License

This project is licensed under the MIT License.
