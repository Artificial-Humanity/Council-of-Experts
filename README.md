# 🧑‍⚖️ Council of Experts

Welcome to **Council of Experts**! 🏛️✨

Instead of asking one model and hoping for the best, Council of Experts convenes a configurable panel of LLM experts and runs them through a multi-round discussion: an opening statement in isolation, one or more reaction rounds where each expert reads the others' previous round and rebuts/agrees/refines, and a closing statement. Think fewer confident hallucinations, more peer review.

It's a native macOS app (SwiftUI) backed by a Rust core, with support for Anthropic (Claude), Google Gemini, OpenAI-compatible models (ChatGPT, Grok, Ollama, LM Studio), and a Mock provider for sandboxed testing.

---

## 🏛️ The Cast & Crew

### 1. Crates (Rust Core)
*   [**`core`**](crates/core): The orchestration engine. Async provider clients, SSE token-streaming, the multi-round panel discussion flow (`run_council_flow`), and a first-pass agentic coding flow (`run_agent_coding_flow`) that applies workspace-contained file edits, runs a build command, and triggers critique-repair loops on failure. The coding flow is an experiment, not a finished feature — every expert writes into one shared workspace, so overwrites between experts are reported but not yet resolved.
*   [**`ffi`**](crates/ffi): UniFFI bindings exposing the core's async streaming callbacks to Swift.

### 2. Platforms
*   [**`apple`**](platforms/apple): A Swift Package (`CouncilOfExpertsKit` + the `CouncilOfExpertsApp` executable) wrapping the FFI target and driving the SwiftUI dashboard.

---

## 🎛️ Capabilities

*   **Multi-round panel discussion**: Round 1 is an opening statement in isolation; the last round is a closing statement; anything in between is a reaction round where each expert reads the others' previous round and rebuts, agrees, or refines. 2–10 rounds, default 3.
*   **Configurable council**: 1–8 experts, editable names, per-expert provider/model/base-URL, and custom system prompts — including LAN-hosted local models (e.g. `http://localhost:11434/v1` for Ollama/LM Studio).
*   **Workspace context**: Select a local folder; indexed files can be attached to prompts.
*   **Multimodal input**: Image attachments across Anthropic, Gemini, and OpenAI providers.
*   **Session persistence**: Conversations are saved locally and reloaded with full history on relaunch.
*   **Native Settings panel**: API credentials are entered through a proper macOS Preferences window and stored in the login Keychain.
*   **Stop control**: Cancel an in-flight discussion between rounds instead of waiting out the remaining ones.

---

## 🚀 Getting Started

### Build the Rust workspace
```bash
cargo build --release
```

### Regenerate the FFI framework and Swift bindings
Requires macOS (produces the `council_of_experts_ffiFFI.xcframework` consumed by the Swift package):
```bash
./build_frameworks.sh
```

### Build the app bundle
Rebuilds the framework, compiles the SwiftUI app in release mode, and assembles a double-clickable `CouncilOfExperts.app`:
```bash
./build_app.sh
```

---

## 📄 License

Apache License 2.0. See [LICENSE](LICENSE) for details.
