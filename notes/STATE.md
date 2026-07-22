# Project State — Council-of-Experts

_Last updated: 2026-07-14._

The committed, curated snapshot of where the Council-of-Experts project stands and what to do next. Behavioral rules and the stack/layout manifest live in [AGENTS.md](../../AGENTS.md).

---

## Current State

- **Scaffold initialized (2026-07-13).** Created initial directory structures for notes, changelogs, and state tracking.
- **Architecture and Roadmap Drafted (2026-07-13).** Formulated the Rust Core + UniFFI + Swift FFI architecture and outlined configuration-based providers and the initial milestone targets.
- **Milestone 1 Completed (2026-07-13).** Set up Rust workspace, FFI scaffolding, dynamic xcframework packaging script, and verified Swift Package builds.
- **Milestone 2 Completed (2026-07-13).** Integrated provider clients (Anthropic, Gemini, OpenAI compatible, and Grok) with SSE token streaming, added mockup unit tests, and verified bridge generation. Added Apache 2.0 license file.
- **Milestone 3 Completed (2026-07-13).** Implemented the Council Orchestrator, allowing parallel expert drafting, intermediate state callbacks, a Lazy multi-threaded Tokio runtime inside the core library, and the final synthesis step using the Chairman provider. Verified with concurrent unit tests and compiled frameworks.
- **Milestone 4 Completed (2026-07-13).** Developed the native macOS SwiftUI dashboard testbed (`CouncilOfExpertsApp`) binding FFI streaming callbacks directly to `@Published` properties, supporting both "Mock Sandbox" and "Live APIs". Built `build_app.sh` script to package compile targets into double-clickable `CouncilOfExperts.app` macOS bundles.
- **Critique Loops Refinement Completed (2026-07-13).** Implemented parallel consensus critique loops, where experts review and critique each other's initial drafts and revise their proposals in parallel. Updated FFI wrappers, view models, and the SwiftUI dashboard to display live critiques using segmented picker tab views.
- **LAN-hosted Models & Dynamic Configs Completed (2026-07-13).** Refactored the SwiftUI application's state manager and sidebar UI to support fully dynamic, per-expert configurations. Users can select providers (Mock, Anthropic, OpenAI, Gemini, Local OpenAI-Compatible), input custom model names, and define custom local base URLs (e.g. `http://localhost:11434/v1` for Ollama/LM Studio) which route directly to the core Rust network request layer.
- **Milestone 5: Chat History & Session Persistence Completed (2026-07-13).** Programmed a persistent conversational log backed by local JSON serialization. Conversations are loaded on launch and Gaston's responses append seamlessly. Historical context is fed back into the Rust core orchestrator on subsequent turns to sustain continuity.
- **Milestone 7: Local Workspace Directory Integration Completed (2026-07-13).** Implemented workspace folder selection using NSOpenPanel. Regular text and code files are scanned and indexed asynchronously. Checkboxes let users select target files, which have their contents formatted and automatically prepended to prompt payloads. Clean attachment indicators are rendered in chat bubbles to prevent text flooding.
- **Configurable Council Size & Names Completed (2026-07-13).** Added a sidebar stepper control letting users dynamically configure the council size from 1 up to a hard limit of 8 experts. Seeded the state array with 8 diverse developer, auditor, and QA personas. Added editable custom name fields for all experts. The main drafting grid and sidebar controls scale automatically based on the active count.
- **Milestone 6: Multimodal Inputs & Media Integration Completed (2026-07-13).** Extended Rust core clients (Anthropic, Gemini, OpenAI) to construct native base64 multimodal request structures. Added a media picker in SwiftUI and staged user attachments inside the app Support container directory. Previews and thumbnails display in bubbles and prompt cards.
- **Milestone 8: Multi-Source Agentic Coding Platform First Pass Completed (2026-07-13).** Implemented file editing parser (`<write_file>`) and command line subprocess build runner. Designed the `run_agent_coding_flow` orchestrator that applies modifications, runs compiler/test suites, captures logs, triggers parallel critique repair loops on errors, and yields Chairman synthesis reports. Added sidebar switches, build command inputs, and a dark console logs terminal view.
- **Multi-round discussion format + Chairman/"Gaston" removed (2026-07-14).** Replaced the single opening+critique pass with a configurable N-round panel discussion (opening statement → reaction rounds → closing statement, min 2 rounds, default 3). Real-world testing then surfaced live bugs in the individual provider integrations (Anthropic and OpenAI reasoning-tier models rejecting a non-default `temperature`; Gemini's streaming parser silently dropping all output) that made a synthesis step on top of already-unreliable inputs not worth keeping — removed the Chairman/"Gaston" concept entirely. The council is now a flat list of up to 8 standard expert cards with no synthesizer; discussion just ends after the closing round. **Deferred**, not abandoned — see Milestone 9 in [architecture-and-roadmap.md](architecture-and-roadmap.md) for revisiting this once provider integrations are solid.

---

## Next Steps

- [ ] Further refine local sandbox execution safety and custom patch diff blocks.
- [ ] Integrate native LiteRT-LM model execution supporting Gemma 3/4 series. This enables a cost-free local offline option utilizing a single loaded weights instance (e.g. Gemma 4 E2B) multiplexed to different prompt-driven expert personalities.





---

## Pointers

- Change history — [CHANGELOG.md](CHANGELOG.md)
- Architecture & Roadmap — [architecture-and-roadmap.md](architecture-and-roadmap.md)
