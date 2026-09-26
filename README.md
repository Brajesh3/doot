# ⚡ Iroh P2P Messenger

A modern, native-performance, end-to-end peer-to-peer (P2P) cross-platform messenger written in Rust, powered by the [Iroh](https://iroh.computer) networking engine (v1.2.0), [egui / eframe](https://github.com/emilk/egui) for Desktop, and [Jetpack Compose](https://developer.android.com/compose) for Android.

---

## 🌟 Key Features

- **True Direct P2P over QUIC**: Direct encrypted peer connections dialed by public keys via Iroh endpoints. No central chat servers, no intermediaries snooping on traffic.
- **Relay & NAT Hole-Punching**: Seamless connectivity even behind strict firewalls and carrier-grade NATs using Iroh's default DERP relay network (`presets::N0`) and STUN.
- **Shareable Base64 Tickets**: One-click "Copy Ticket" generates an address (`iroh-msg:...`) bundling the node's public key, DERP relay URL, and direct IP addresses.
- **Cross-Platform Architecture**:
  - **Desktop (Windows/macOS/Linux)**: Pure Rust hardware-accelerated immediate-mode UI (`egui`/`eframe`), instant startup, high-DPI support, dark theme, and smooth 60+ FPS rendering.
  - **Android**: Modern Jetpack Compose UI with native ARM64 JNI shared library (`libmessage_jni.so`), reactive `StateFlow` ViewModel, and full Iroh QUIC node.
  - **CLI Echo Bot**: Built-in test bot (`message-bot`) for rapid automated P2P verification.
- **Local Persistence**: User identities (Ed25519 secret/public keys), contact lists, and message history are automatically persisted locally in standard OS data directories.

---

## 🏗️ Project Architecture

```
message/
├── Cargo.toml                      # Root Cargo workspace
├── .cargo/config.toml              # NDK cross-compilation config
├── crates/
│   ├── message-core/               # Headless, platform-agnostic P2P engine
│   │   ├── src/
│   │   │   ├── lib.rs              # Public exports
│   │   │   ├── identity.rs         # Ed25519 keypair generation & storage
│   │   │   ├── protocol.rs         # Wire messages & shareable ticket codec
│   │   │   ├── store.rs            # Local JSON message & contact persistence
│   │   │   ├── events.rs           # Commands & event channel contracts
│   │   │   └── node.rs             # Tokio background engine & QUIC connection pool
│   │   └── tests/
│   │       └── integration_tests.rs# Automated 2-node P2P test
│   ├── message-desktop/            # Hardware-accelerated egui/eframe desktop client
│   │   └── src/main.rs             # Full GUI messenger
│   ├── message-android/            # JNI dynamic library for Android
│   │   └── src/lib.rs              # Rust JNI bindings to message-core
│   └── message-bot/                # Interactive CLI peer bot for testing
│       └── src/main.rs             # Auto-reply echo bot
├── android-test/                   # Android Jetpack Compose client
│   ├── app/src/main/
│   │   ├── java/com/example/testapp/
│   │   │   ├── MainActivity.kt
│   │   │   ├── messenger/          # JNI bridge & ViewModel
│   │   │   └── ui/chat/ChatScreen.kt# Jetpack Compose Chat UI
│   │   └── jniLibs/arm64-v8a/      # libmessage_jni.so destination
└── README.md
```

---

## 🚀 Getting Started

### 1. Launching the Desktop Messenger (Windows)

```powershell
# Run optimized release binary
start target/release/message-desktop.exe

# Or compile and run from source
cargo run --release -p message-desktop
```

### 2. Testing with the Built-In CLI Bot

Open a terminal and run:
```powershell
cargo run -p message-bot
```
1. The bot will print a shareable ticket: `iroh-msg:...`
2. On your Desktop or Android phone, tap/click **"+ New Chat"**, paste the ticket, and tap **Connect**.
3. Send any message—the bot will simulate typing and echo back over direct P2P QUIC!

### 3. Running the Android Client

```powershell
cd android-test
./gradlew.bat assembleDebug

# Deploy to connected device via Android CLI:
android run --device=<device-id> --apks=app/build/outputs/apk/debug/app-debug.apk
```

---

## 🧪 Running Automated Tests

Run the integration test suite that spawns two live Iroh P2P nodes, connects them over QUIC, and verifies bidirectional message delivery and acknowledgements:

```bash
cargo test -p message-core
```
