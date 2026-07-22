# Changelog — Council-of-Experts

This document tracks technical changes, refactoring milestones, and build-system adjustments for Project Council-of-Experts.

> **Maintenance:** This changelog is append-only within a release cycle — keep it current every session. It lives at `Notes/Council-of-Experts/CHANGELOG.md`.

---

## [2026-07-13]

### Added
- **Project Scaffold Initialized**: Created initial directory structure, state tracking (`STATE.md`), and this `CHANGELOG.md` to track developments.
- **Architecture & Roadmap Documented**: Drafted [architecture-and-roadmap.md](architecture-and-roadmap.md) outlining the Rust Core + UniFFI + SwiftUI FFI layer, configuration-based providers, and milestone roadmap. Added future high-ambition goals for a multi-model agentic coding platform working in tandem and established the agentic design philosophy (inspired by Antigravity, Claude Code, and Codex).
- **Milestone 1 Completed**: Set up Cargo workspace, `crates/core`, `crates/ffi`, `platforms/apple` SwiftPM package, and `build_frameworks.sh` script.
- **Milestone 2 Completed**:
  * Integrated async clients in `crates/core` for Anthropic (Claude), Google Gemini, and OpenAI-compatible models (ChatGPT, Grok, Ollama, LM Studio).
  * Implemented Server-Sent Events (SSE) stream parsing for token-by-token callbacks across the FFI.
  * Added unit test coverage for factory construction and serialization mappings.
  * Packaged `council_of_experts_ffiFFI.xcframework` and verified Apple SwiftPM target compilation.
  * Added Apache 2.0 LICENSE file to the workspace.
- **Milestone 3 Completed**:
  * Set up a lazy global multi-threaded Tokio runtime inside the core crate.
  * Programmed the `run_council_flow` orchestrator to run multiple expert drafting calls concurrently in parallel task loops.
  * Defined `CouncilCallback` to propagate individual expert started/chunk/completed events and Chairman started/chunk/completed events to the host app.
  * Implemented synthesis context compile logic and Chairman routing.
  * Added `ProviderType::Mock` and `MockProvider` to enable sandboxed testing.
  * Wrote concurrent unit test validation and successfully generated and linked the updated FFI package in SwiftPM.
- **Milestone 4 Completed**:
  * Added executable target `CouncilOfExpertsApp` to the Swift Package.
  * Implemented `CouncilViewModel` using `@Published` properties, conforming to `FfiCouncilCallback` to stream intermediate state updates to the SwiftUI main thread.
  * Developed the premium dashboard UI in `ContentView.swift` featuring sidebars for API credential setups, dynamic grid blocks for parallel expert streams, and a central synthesis display card for the Chairman.
  * Created `build_app.sh` script compiling release targets and copying binaries + embedded frameworks into standard, double-clickable `CouncilOfExperts.app` macOS App bundles.
- **Critique Loops Refinement Completed**:
  * Added `critique_rounds` configuration to the Rust Core, FFI layer, and Swift models.
  * Programmed parallel critique execution where each expert receives the user query + drafts of other panelists, producing a critique and revised draft proposal concurrently.
  * Expanded FFI callbacks and SwiftUI state managers to support streaming live critique/revision text chunks in real-time.
  * Integrated segmented picker tab views on expert cards in the SwiftUI dashboard, automatically navigating to the critique tab during execution to stream critique chunks visually.
  * Configured mock provider streaming paths to enable testing of drafting vs critique phases.
- **LAN-hosted Models & Dynamic Configs Completed**:
  * Refactored `CouncilViewModel` to hold dynamic configuration editors for panel members, converting UI models dynamically to FfiExpert targets.
  * Added `ExpertConfigSection` view using native macOS `DisclosureGroup` collapsible panels inside the sidebar.
  * Added picker options for local model execution along with model name input and base URL targeting fields.
  * Verified local OpenAI-compatible routing maps down to the Rust core client.
- **xAI Grok Provider Integration Completed**:
  * Programmed `OpenAiCompatibleClient` to use xAI's default base URL (`https://api.x.ai/v1`) for Grok.
  * Added `xAI Grok` option to the provider settings dropdown and added a separate `Grok (xAI) Key` field to the credentials inspector card.
- **Native macOS Settings Panel Integration Completed**:
  * Added native `Settings` scene to `App.swift` opening a dedicated Preferences panel.
  * Bound fields in `SettingsView` to `@AppStorage` keys (`openAiKey`, `anthropicKey`, `geminiKey`, `grokKey`), saving values transparently to `UserDefaults` (persisting across restarts).
  * Removed credential entry fields from the sidebar and added an interactive gear/cog button in the sidebar header to programmatically trigger the Settings preferences window.
  * Configured `CouncilViewModel` to pull credential keys directly from `UserDefaults` at runtime.
- **Tailored Behavior/System Prompts Integration Completed**:
  * Added `systemPrompt` field to `ExpertConfigInput` mapping directly to FFI models.
  * Added scrollable behavior prompt `TextEditor` boxes in the sidebar config sections.
  * Added Milestones 5 and 6 (Multi-turn persistent logging and Multimodal inputs) to the roadmap documentation.
- **Global Rebranding to Council of Experts**:
  * Renamed project directories, crates, packages, executable targets, and document references from Panel of Experts to Council of Experts.
- **Milestone 5 Completed (Chat History & Session Persistence)**:
  * Implemented automatic conversation state saving and loading to local JSON file inside Application Support.
  * Refactored Rust Core orchestrator to receive and feed previous conversational messages to experts on subsequent turns.
- **Milestone 7 Completed (Local Directory Integration)**:
  * Integrated workspace folder selection using `NSOpenPanel`, scanning regular files asynchronously.
  * Added checkable workspace files sidebar listing and formatted their payloads to prepend to user prompts.
  * Rendered concise attachment header badges in user bubbles to avoid UI text flooding.
- **Configurable Council Size & Names Completed**:
  * Added sidebar stepper to dynamically control the active expert count from 1 to 8.
  * Seeded configurations with 8 default developer personas with custom name editing capabilities.
  * Resolved Settings Cog wheel selector crashes using SwiftUI's `SettingsLink`.
- **Milestone 6 Completed (Multimodal Inputs & Media Integration)**:
  * Added media picker in SwiftUI dashboard supporting system image files.
  * Staged user image files inside the application container directory for session logging and thumbnail persistence.
  * Refactored OpenAI, Anthropic, and Gemini clients in Rust Core to construct native base64 multimodal request structures.
  * Rendered image thumbnail previews in prompt inputs and scrollable image attachments inside chat bubbles.


