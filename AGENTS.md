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
* **Orchestration:** Lazy multi-threaded Tokio runtime running an N-round panel discussion — an opening statement made in isolation, reaction rounds, and a closing statement — with all experts drafting each round in parallel. There is no synthesis/Chairman step; it was removed 2026-07-14 and is deferred to Milestone 9.
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

* **`main` is PR-only. Do not push to it directly** (owner, 2026-08-10). Branch, push the
  branch, open a PR, and let it merge. This applies to agent sessions exactly as it applies to
  the owner — an agent that "just needs one small fix on `main`" is the case the rule exists
  for. Two reasons it is a rule and not a preference:
  * **The Mac and `ai-lab-0` (and their agent sessions) commit concurrently.** Direct pushes to
    a shared `main` are how two sessions silently interleave half-finished work; a branch is a
    place for work to be incomplete without being everyone's problem.
  * **Nothing reviews a direct push.** `.github/workflows/claude-review.yml` triggers on
    `pull_request`, so work that skips the PR skips the review entirely — the automation
    cannot see a commit that was never proposed.
* **Branch naming**: `<type>/<short-slug>` matching the commit type — `fix/`, `feat/`,
  `docs/`, `chore/`.
* **Work on the branch, commit and push liberally, open the PR only when the work is done**
  (owner, 2026-08-10). Pushing to your own branch is free and is the entire point of having
  one: commit early, commit often, push whenever, and let the branch hold work that is not
  yet finished. What is deliberate is the *timing of the PR*, not the timing of the commits.
  * **When completion is defined, completion opens the PR.** If a `/goal` has been set,
    achieving that goal IS the completion point — open the PR then, without being asked again.
  * **Otherwise the owner calls it.** With no goal set, work, push, and wait: the owner
    acknowledges the completion point and the PR follows from that.
  * **This is also what makes it cheap.** `.github/workflows/claude-review.yml` fires when a
    PR is opened AND on every push to an open one, so a PR opened at the *start* of the work
    bills a full model-rate review of half-finished code on every intermediate push. Opening
    at completion buys exactly one review, of work that is actually ready to be read.
* **Pull before push, every time.** Run `git pull --rebase` as the first step of any
  commit-and-push sequence on your branch, and rebase on `main` before opening the PR. If the
  tree holds the owner's uncommitted local edits, fetch and check ahead/behind instead of
  forcing a rebase.
* **The exception is the owner's, not yours.** If the owner explicitly directs a direct push to
  `main`, that is their call and does not need re-litigating — state the rule once, then do as
  asked. An agent never grants itself the exception.
* ⚠ **A rule in this file is not an enforcement mechanism.** The authority is the branch
  protection on `main`; this section only explains it. If a direct push to `main` ever
  *succeeds*, the protection is missing or was bypassed — report that rather than treating it
  as permission.* **Review feedback is closed with the `claude-fix` label, not by hand-waving.** The review
  workflow only comments; `.github/workflows/claude-fix.yml` is what acts on those comments.
  Add the `claude-fix` label to the PR and the fix agent reads the inline comments, commits
  the fixes, replies, and removes the label. It is label-gated deliberately: firing it
  automatically on every submitted review oscillates (fix pushes → `synchronize` → new review
  → fix pushes), and the vendor ships no loop guard. One label, one pass; re-label to run it
  again. A review comment is an argument, not an order — the fix agent is expected to push
  back in a reply where a finding is wrong, rather than making a change it believes is wrong.


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
