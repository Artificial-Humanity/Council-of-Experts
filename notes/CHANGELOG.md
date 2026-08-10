# Changelog — Council-of-Experts

This document tracks technical changes, refactoring milestones, and build-system adjustments for Project Council-of-Experts.

> **Maintenance:** This changelog is append-only within a release cycle — keep it current every session. It lives at `notes/CHANGELOG.md`.

---

## [2026-08-10]

_All code changes in this entry: `13027e6`._

### Added
- **`.github/workflows/claude-fix.yml` — the review loop is closed** (`4ffba8a`). The reviewer only
  ever commented; nothing acted on those comments. Adding the `claude-fix` label to a PR now runs
  a fix pass that reads the inline review comments (they are **not** in `gh pr view` — it fetches
  `/pulls/N/comments`), commits fixes, replies with what it changed and what it deliberately did
  not, and removes the label.
  - **Label-gated on purpose.** Firing on `pull_request_review` submitted oscillates: fix pushes →
    `synchronize` → reviewer posts → fix pushes → …, each lap billed at full model rates. The
    vendor ships no loop guard and does not document which token it pushes with, so GitHub's own
    recursion protection can't be relied on either. One label, one pass; re-label to repeat.
  - Load-bearing details: `contents: write` (this job pushes, the reviewer only comments);
    checkout `ref: head.ref` + `repository: head.repo.full_name` (the default checkout is a
    detached merge commit, which cannot be pushed from); `fetch-depth: 0`; and
    `cancel-in-progress: false`, since cancelling mid-pass can leave edits applied but uncommitted.

### Fixed
- **`.github/workflows/claude-review.yml` — the automated PR reviewer could not post** (`13027e6`).
  It ran to completion and delivered nothing. Three independent gaps, each a silent no-op on
  its own, plus a wrong secret name:
  - **No `permissions:` block.** This repo's default workflow token is read-only
    (`default_workflow_permissions: "read"`), so the job analyzed the diff, billed the tokens,
    and had no right to comment. Added `contents: read` / `pull-requests: write` /
    `id-token: write`.
  - **No `--allowedTools`.** Permissions grant the TOKEN the right; `--allowedTools` grants the
    AGENT the tool. Without the inline-comment MCP tool and the `gh` allowlist there was no
    mechanism to post at all.
  - **The prompt never said to post to GitHub** — added `REPO` / `PR NUMBER` context and
    explicit `gh pr comment` / `create_inline_comment` instructions.
  - **The secret is the org-level `CLAUDE_OAUTH_TOKEN`**, not `CLAUDE_CODE_OAUTH_TOKEN`. The
    action's *input* name stays `claude_code_oauth_token`, so the two deliberately disagree;
    the file carries a comment on that line because it reads as a typo.
- Dropped `--effort xhigh` — not a documented `claude_args` flag, an unrecognized flag fails the
  run outright, and Claude Code already defaults to xhigh effort on capable models.
- Replaced the severity filter. "Ignore superficial style or formatting nitpicks" is followed
  *literally*: the model finds the bugs, then declines to report anything below the bar, so
  precision looks excellent while real findings vanish. Now a concrete bar (anything that could
  cause incorrect behavior, a test failure, a security weakness, or a misleading result) with
  explicit severity, and an instruction not to filter past it.
- Softened "security exploits" to "security vulnerabilities" (`claude-fable-5` runs classifiers
  over cybersecurity content and can return `stop_reason: "refusal"`), added a `concurrency`
  group so rapid pushes cancel superseded billed runs, and pinned checkout `@v6`.

## [2026-08-01]

_All code changes in this entry: `46b179a`._

### Fixed
- **Multi-turn history is now valid for every provider** (`normalize_history` in `crates/core`): expert turns are labeled with their author, consecutive same-role turns are merged, and the transcript is trimmed to start at the first user turn.
  * Gemini received the literal role `"assistant"` for every history entry, which that API rejects — it only accepts `user` and `model`. Any second-turn conversation with a Gemini expert failed.
  * Anthropic receives one message per expert per round, producing runs of consecutive `assistant` turns that the Messages API rejects.
  * Every expert's output was replayed to every other expert as its *own* prior turn, so each model was told it had personally written its rivals' statements.
- **Agent coding mode could write outside the workspace** (`resolve_workspace_path`): `<write_file>` paths from model output were joined onto the workspace with no containment check, so `../` traversal or an absolute path could write anywhere the user can — reachable via prompt injection from a workspace file being reviewed. Traversal, absolute paths, and symlink escapes are now refused and reported.
- **SSE streams no longer corrupt non-ASCII output**: all three parsers decoded each network chunk independently, turning any multi-byte UTF-8 character split across a chunk boundary into replacement characters. Line buffering is now byte-based.
- **Gemini API key moved from the query string to the `x-goog-api-key` header**: reqwest embeds the request URL in its error strings, which are surfaced verbatim in the UI, so a failed request could display the user's key on screen.
- **`generate_expert_response` / `generate_expert_stream` now run on the shared Tokio runtime**, matching `list_available_models`; they previously called reqwest on whatever thread UniFFI polled them from, with no reactor guaranteed.
- **A council whose opening round produced nothing now errors** instead of spending the remaining rounds asking each model to react to an empty transcript.
- **Mock provider recognises reaction rounds again**: its marker string still matched only the old pre-2026-07-14 critique prompt, so sandbox runs returned opening-statement text for every round.
- **`activeExpertCount` and `councilRounds` are clamped on load and on write**: an out-of-range value restored from a stale defaults plist indexed past the end of the expert config array.

### Added
- **Stop button** (`cancel_active_run` over FFI): cancels an in-flight run between rounds and between stream chunks, so a runaway discussion stops costing tokens without waiting out the remaining rounds.
- **Request timeouts**: 20s connect, 300s total for non-streaming calls, and a 120s idle-gap timeout for streams (a total timeout can't distinguish a healthy long answer from a stalled connection).
- **Each expert now sees its own previous statement** in reaction and closing rounds. Rounds are independent single-turn requests, so an expert asked to "refine your position" previously had no record of what that position was.
- **Overwrite collisions in agent coding mode are reported** through a new `on_workspace_warning` callback. The underlying last-writer-wins behaviour is unchanged and remains a known limitation — see Milestone 8.
- **Test coverage** for the above: history normalization, UTF-8 chunk splitting, workspace path traversal and symlink escapes, unclosed `<write_file>` tags, a full multi-turn council run, and the empty-round failure path (13 tests, up from 4).

### Changed
- **API keys moved from `UserDefaults` to the login Keychain** (`Credentials.swift`), with one-time migration of existing keys and removal of the plaintext copies. A `UserDefaults` plist is readable by any process running as the user.
- **Workspace scanner skips build and dependency directories** (`.git`, `target`, `node_modules`, `.build`, `DerivedData`, and similar) and inlines at most 256 KB per attached file; selected files are sorted for prompt-cache stability.
- **Provider request building was extracted into per-client `headers`/`body` helpers**, removing the duplication between each client's `generate` and `generate_stream` — the history fix needed to land in one place per provider rather than six.
- **Version badge reads `CFBundleShortVersionString`** instead of a hardcoded string that had drifted from every other version in the repo.
- Default model fallbacks refreshed to `claude-sonnet-5` and `gemini-2.5-pro`.

---

## [2026-07-22]

### Added
- **AGENTS.md added** (`e2ec34f`): entry-point document defining the stack matrix, file naming conventions, commit hygiene, changelog maintenance rules, and code review execution standards.
- **Notes restructured** (`bfa8612`): moved engineering notes into the repo's own `notes/` directory.

---

## [2026-07-14]

### Added
- **Multi-round panel discussions with a response-length cap** (`912ba39`): replaced the single opening+critique pass with a configurable N-round discussion — round 1 is an opening statement made in isolation, the final round is a closing statement, and everything between is a reaction round where each expert reads the others' previous round. 2–10 rounds, default 3. Added a `max_response_words` setting applied as a prompt instruction each round.
- **Provider model discovery** (`7ee6457`): `list_models` queries Anthropic, Gemini, and OpenAI-compatible endpoints for their available models and offers them as a dropdown beside the model name field.
- **Optional thinking/reasoning notes** (`7bce907`): added `enable_thinking` to the provider config, wired Anthropic extended thinking and Gemini thought summaries through a separate `on_thinking_chunk` callback path, and displayed them in a collapsed-by-default pane on each expert card.
- **Round-robin expert messages in the main chat, plus clear-chat reset** (`672245a`).
- **Resizable drafting grid and toggleable sidebar** (`0481f59`).

### Fixed
- **Live-use bugs found in real testing** (`804afa7`):
  * Anthropic and OpenAI reasoning-tier models rejected any non-default `temperature` with a 400 — temperature is no longer sent to either.
  * Gemini's streaming parser silently dropped all output; switched the endpoint to `alt=sse` so it emits one complete JSON object per `data:` line instead of a single slowly-growing JSON array.

### Removed
- **Chairman / "Gaston" synthesis step removed entirely** (`804afa7`): real-world testing found it wasn't earning its keep while the individual provider integrations still had live bugs that made synthesis on top of unreliable inputs unreliable itself. The council is now a flat list of up to 8 expert cards with no synthesizer. Deferred rather than abandoned — see Milestone 9 in [architecture-and-roadmap.md](architecture-and-roadmap.md).

### Changed
- **Build outputs consolidated under `build/`** (`5c81e86`): frameworks and the app bundle now share one output root inside the sub-project.

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


