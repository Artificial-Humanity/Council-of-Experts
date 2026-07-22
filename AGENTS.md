# AGENTS — Council-of-Experts

This is the entry point for any agent or developer working on Council-of-Experts (the
multi-provider LLM council app). This is an independent GitHub repo. Internal engineering
notes — changelog, current state, roadmap — live in [notes/](notes/). Before starting work,
read [notes/STATE.md](notes/STATE.md) for the current state of the project and the most
immediate must-do items.

---

## Core Stack Matrix

* **Language Ecosystem:** Rust core (Cargo workspace: `crates/core`, `crates/ffi`) bridged over **UniFFI** to native front-ends.
* **Front-End:** Native macOS SwiftUI app (`platforms/apple` SwiftPM package, `CouncilOfExpertsApp`), packaged into a double-clickable `.app` bundle via `build_app.sh` / `build_frameworks.sh`.
* **Providers:** Config-based multi-vendor clients — Anthropic, Gemini, and OpenAI-compatible (ChatGPT, Grok/xAI, and LAN-hosted Ollama/LM Studio via custom `base_url`) — with SSE token streaming; Mock provider for sandboxed testing.
* **Orchestration:** Lazy multi-threaded Tokio runtime running parallel expert drafting, critique loops, and Chairman synthesis.
* **Roadmap Direction:** Native LiteRT-LM execution (Gemma 4 variants multiplexed into prompt-driven expert personas) for a cost-free offline option.

---

## Integration Dependencies

* This repo is a standalone Rust + SwiftUI repository: a multi-provider LLM "council" that
  drafts, critiques, and synthesizes answers in parallel, evolving toward a multi-source
  agentic coding platform. State and roadmap live in [notes/STATE.md](notes/STATE.md) and
  [notes/architecture-and-roadmap.md](notes/architecture-and-roadmap.md).

---

## File Naming Conventions

Names must be predictable so links resolve on case-sensitive systems (Linux/CI) as well as
case-insensitive macOS/Windows.

* **Canonical root marker files → `UPPERCASE`** (`SCREAMING_SNAKE_CASE` if multi-word): `README.md`, `LICENSE`, `CONTRIBUTING.md`, `CHANGELOG.md`, `ROADMAP.md`, `AGENTS.md`. Keep this set small and curated.
* **Top-level anchor docs → `UPPERCASE`, single word preferred:** `ARCHITECTURE.md`, `STATE.md`.
* **All other docs & notes → `lowercase-kebab-case.md`:** e.g. `open-decisions.md`, `code-review-findings.md`. This is the rule for everything in `notes/`.
* **Source code → the language's own convention:** Rust `snake_case.rs`, Swift `PascalCase.swift`, Kotlin `PascalCase.kt`.
* **Never** let case be the only difference between two paths, and always reference files with their exact case.

---

## System Operational Mandates

### 1. Commit Hygiene

* **Pull before push, every time.** The Mac and `ai-lab-0` (and their agent sessions) commit to
  the same `main` branch concurrently: run `git pull --rebase` as the first step of any
  commit-and-push sequence. If the tree holds the owner's uncommitted local edits, fetch and
  check ahead/behind instead of forcing a rebase.

### 2. Changelog Maintenance Requirement

* The project changelog lives at [notes/CHANGELOG.md](notes/CHANGELOG.md). Append a detailed chronological entry describing all technical modifications, refactoring milestones, and build-system changes **after committing** the corresponding work.
* **Scope: code work only.** Changelog entries are required for source, build-config, and dependency-manifest changes (`crates/`, `platforms/`, `build_app.sh`/`build_frameworks.sh`, `Cargo.toml`/`Cargo.lock`, build scripts). They are **not** required for docs-only commits (`*.md`, comments-only changes).
* Every entry must be accompanied by the short 7-character commit SHA associated with the work.
* **The changelog is append-only across a release cycle.** Do not prune, rewrite, or remove historical entries. Entries are pruned/rolled over **only** when we tag and release a new version of the overall project — at which point the released entries are collected under that version's heading and the working section is reset for the next cycle.
* New entries go at the top under the current date, following the existing `Added` / `Changed` / `Fixed` / `Removed` structure.

### 3. Code Review Execution Standards

* **Scope: code work only.** Code reviews cover the same code changes that warrant changelog entries (see §2) — source, build config, and dependency manifests. Docs-only commits are out of scope and need no review.
* When performing a code review, cross-reference the changelog and corresponding commits.
* Create a review document matching the format `notes/code-review-[year][month][day]-[hhmmss].md`. Begin the document with the first evaluated short commit SHA, and end with the last evaluated commit SHA.
* Determine the range of commits to review by starting with the commit immediately following the end SHA of the *previous* code review. If no prior review exists, use all commits from the previous and current day.
* Once the new code review document has been written, delete the previous one to keep only the latest review active.
* Repoint the **Latest code review** pointer in [notes/STATE.md](notes/STATE.md) to the new document (only the link target changes; the surrounding line is phrased generically) so a session can find the current review without globbing the folder.
