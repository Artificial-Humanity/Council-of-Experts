# Technical & Alignment Review — Council-of-Experts

_Reviewed 2026-08-01. Scope: full repository at commit `f9263bf` (clean tree) — planning docs, Rust core, FFI layer, SwiftUI app, build scripts, and process artifacts. `cargo test --workspace` passes (4/4)._

This is a one-off deep review covering both **alignment** (is the project sound and heading toward its stated goal?) and **code** (correctness, security, quality). It is not the routine commit-range review defined in AGENTS.md §3 and deliberately does not follow that format.

> **Status (updated 2026-08-01, same day):** the correctness, security, and robustness findings in §3, the smaller items in §4/§5, and the documentation drift in §5 have since been fixed in a follow-up pass — see the 2026-08-01 entry in [CHANGELOG.md](CHANGELOG.md). What remains open is the design and measurement work: the agent-coding consensus mechanic (§2.1), the evaluation harness (§2.2), history-growth capping (§2.3), the `lib.rs` module split, and per-provider SSE fixture tests. Individual items below are marked **[FIXED]** where they have been addressed. The analysis is left intact rather than deleted, since the reasoning is what makes the remaining decisions legible.

---

## 1. Verdict in brief

The **panel-discussion chat product is sound and largely delivered**: the Rust-core + UniFFI + SwiftUI architecture is appropriate, the provider abstraction is clean, real-world testing has already driven good decisions (removing the Chairman, fixing temperature/streaming bugs), and the docs record decisions with reasons — that discipline is a genuine strength.

Three things need attention before further feature work:

1. **Correctness debt in multi-turn conversations** — the Gemini client sends an invalid role for history messages, and the way history is modeled (every expert's output becomes an undifferentiated `assistant` message for every other expert) is both a probable API error source and a reasoning-quality problem.
2. **The agentic coding flow (Milestone 8) contradicts the project's own design philosophy** and has a real security hole (unconstrained file writes from model output). It should be treated as a prototype to redesign, not a foundation to extend.
3. **Process drift** — the changelog and several docs no longer reflect reality, which undermines the AGENTS.md workflow this repo defines for itself.

---

## 2. Alignment review

### 2.1 What is the goal, and is the project on track?

The stated goal (README, AGENTS.md) is two-phase: (a) a multi-provider council that reduces confident hallucination through structured peer review, evolving into (b) a vendor-independent, multi-source agentic coding platform.

**Phase (a) is on track.** Milestones 1–7 are done, the multi-round discussion format is a genuine improvement over the old single critique pass, and removing the Chairman until providers are solid was the right call, correctly recorded as deferred rather than abandoned (Milestone 9).

**Phase (b) is not yet on a sound footing.** The architecture doc's stated philosophy is:

> *"Tandem Isolation: Designing orchestrators to allow agents from different vendors to work in isolated code threads concurrently without blocking each other."*
> *"Diverse Consensus Mechanics: … different models critique, vote, and build upon each other's code modifications."*

The current `run_agent_coding_flow` implements the opposite of both: every expert writes complete files to the **same** workspace, applied sequentially in expert order, so the last writer silently wins for any shared path ([lib.rs:1302-1313](../crates/core/src/lib.rs)). There is no isolation, no voting, no selection — the "consensus" is whatever file soup survives the overwrites, and the build verifies that soup rather than any expert's coherent proposal. The critique-repair loop then re-applies *all* revised drafts with the same overwrite behavior. Before investing further in Milestone 8, decide the consensus mechanic explicitly. Two designs that match the stated philosophy:

- **Champion selection**: each expert proposes a full patch set; the patches are applied to isolated copies (or git worktrees), each is built/tested, and the passing candidate (or the one the panel votes for) is applied to the real workspace.
- **Partitioned tandem work**: an explicit up-front split of files/modules per expert, so writes cannot collide — closer to the "Tandem Isolation" idea.

Either is a meaningful design step; the current all-writes-win flow is not a base to iterate on.

### 2.2 The council's core premise is untested

The product's differentiating bet — *heterogeneous multi-model debate produces better answers than one strong model* — is empirically testable and currently unmeasured. The multi-agent-debate literature (e.g. Du et al. 2023) finds real gains but also documents **convergence/sycophancy**: models drift toward agreement over rounds, and returns diminish or invert past ~2–3 rounds. Two design choices in the current flow make that failure mode *more* likely:

- **Experts never see their own previous statements.** Each round's prompt contains only the *other* panelists' previous round ([lib.rs:1112-1116](../crates/core/src/lib.rs)), yet instructs the expert to "refine your position" — a position that is no longer in its context. This actively accelerates position drift and makes closing statements less anchored. Including the expert's own prior statement is a small change with likely outsized quality impact.
- **Only the immediately preceding round is visible**, so by round 4+ the original disagreements have been laundered out of context entirely.

Recommendation: before adding features, add a cheap evaluation harness — even a script that runs the same 20 questions through (1) a single model, (2) the council at 2 rounds, (3) the council at 4 rounds, and diffs factual accuracy by hand. This directly informs Milestone 9 (whether a synthesis step earns its keep), the default round count, and whether the premise holds at all. It converts the project's central claim from vibes to data.

### 2.3 Cost and context scaling deserves a line in the roadmap

Each round, every expert receives: full session history + all other experts' previous round. Session history itself accumulates *every* expert message from *every* round of *every* prior turn (`messages` → `ffiHistory`, [CouncilViewModel.swift:625-632](../platforms/apple/Sources/App/CouncilViewModel.swift)). With 4 experts × 3 rounds, each user turn adds ~13 messages, and each of those is replayed to all experts every round of every subsequent turn. Input-token cost grows roughly quadratically in turns × experts × rounds. Consider capping history (last N turns, or closing statements only — the closing statement is by design the summary of each expert's position).

### 2.4 Roadmap direction (LiteRT-LM)

Native local execution for a cost-free option is a good direction and consistent with the multiplexed-persona idea (one loaded model, N system prompts). Two flags before committing:

- LiteRT-LM is a C++ runtime with no mature Rust bindings — the FFI work lands on you. **llama.cpp** (MIT) has mature Rust bindings, broader model coverage (including Gemma), and the same multiplexing capability; worth an explicit comparison note before building.
- One local instance serves experts **sequentially**, so an N-expert round becomes N× generation latency, versus today's parallel cloud calls. Fine, but it changes the UX assumptions (the parallel drafting grid becomes a queue).

---

## 3. Correctness findings (ranked)

### P0 — Gemini rejects multi-turn history: invalid role `"assistant"` **[FIXED]**

`role_to_string` maps assistant-side roles to `"assistant"` ([lib.rs:132-138](../crates/core/src/lib.rs)), and the Gemini client sends that verbatim in `contents[].role` ([lib.rs:618-625](../crates/core/src/lib.rs), [699-708](../crates/core/src/lib.rs)). The Gemini API only accepts `"user"` and `"model"` — any second-turn conversation with a Gemini expert should 400 with INVALID_ARGUMENT. This is the same *class* of live bug (works single-turn, breaks in real use) that STATE.md records fixing on 2026-07-14; multi-turn Gemini apparently wasn't in that test pass. Fix: map to `"model"` in the Gemini client.

### P0 — Agent coding mode: unconstrained file writes from model output **[FIXED]**

`parse_file_edits` paths are joined onto the workspace with no containment check ([lib.rs:1305](../crates/core/src/lib.rs)). `Path::join` with `../../..` escapes the workspace, and with an **absolute** path replaces the base entirely — so a hallucinating or prompt-injected model can write `~/.zshenv` or anything else the user can. The realistic attack chain: untrusted file in the workspace → its contents are prepended into the prompt (Milestone 7) → injected instructions emit a `<write_file>` outside the workspace → arbitrary code execution on the next shell/build. STATE.md's "refine local sandbox execution safety" next-step is this item; it should be done **before** any further Milestone 8 work, not after. Minimum fix: canonicalize `workspace.join(path)` and reject results outside the canonicalized workspace root; also reject absolute paths and symlink-escapes.

### P1 — Anthropic multi-turn history: non-alternating roles **[FIXED]**

Every expert bubble becomes a consecutive `assistant` message in history. The Anthropic Messages API historically rejects non-alternating role sequences ("roles must alternate"), which would break every second-turn Anthropic call in a session with prior expert messages. Verify against the current API; if enforced, merge consecutive same-role history messages per provider (this is what most middleware does).

Related, and worth fixing at the same time: because *all* experts' outputs map to `assistant`, each model is told it personally authored every other expert's statements across all prior turns. That confuses attribution ("as I said earlier…" about a rival's claim) independent of any API error. A cheaper and cleaner history model: fold prior-turn expert statements into a single labeled context block (or a `user`-role transcript message), keeping only genuine user/assistant alternation at the API level.

### P1 — SSE parsers can mangle multi-byte UTF-8 at chunk boundaries **[FIXED]**

All three streaming parsers do `String::from_utf8_lossy(&chunk)` per network chunk and accumulate the *string* ([lib.rs:331](../crates/core/src/lib.rs), [560](../crates/core/src/lib.rs), [778](../crates/core/src/lib.rs)). A multi-byte character split across two HTTP chunks becomes U+FFFD replacement characters in the visible stream. Users will see it with CJK text and emoji. Fix: buffer raw `Vec<u8>` and only convert complete lines, or use an SSE crate (`eventsource-stream`).

### P1 — No request timeouts, no cancellation **[FIXED]**

The reqwest clients are built with defaults (no total timeout), and `run_expert_round` waits for **all** experts before the round completes. One hung provider (LAN box asleep, dropped connection) stalls the entire council forever; the UI stays "Orchestrating…" with Send disabled and no way to stop ([ContentView.swift:468](../platforms/apple/Sources/App/ContentView.swift) disables Send while executing; there is no Stop control). Add per-request timeouts (connect + an idle-read timeout for streams) and a user-facing cancel that aborts the flow.

### P1 — API-key hygiene **[FIXED]**

- **Gemini key travels in the URL query string** ([lib.rs:609-612](../crates/core/src/lib.rs), [691-694](../crates/core/src/lib.rs), [891](../crates/core/src/lib.rs)). Worse, reqwest error strings include the request URL, and those strings flow into `on_expert_error` and the UI — so a connection failure can display the API key. Use the `x-goog-api-key` header instead.
- **Keys live in plaintext `UserDefaults`** ([App.swift:24-27](../platforms/apple/Sources/App/App.swift)) — readable by any unsandboxed process as the user. The Keychain is the right store; the README currently presents UserDefaults storage as a feature.

### P2 — Two exported FFI functions will panic off-runtime **[FIXED]**

`list_available_models` correctly wraps its work in `RUNTIME.spawn` with a comment explaining reqwest needs a Tokio reactor ([ffi/src/lib.rs:243-252](../crates/ffi/src/lib.rs)). But `generate_expert_response` and `generate_expert_stream` call reqwest directly on whatever context UniFFI polls from ([ffi/src/lib.rs:254-291](../crates/ffi/src/lib.rs)) — the exact hazard that comment describes. The app never calls them today, so this is latent — but they're exported API. Wrap them the same way or remove them.

### P2 — Round failures degrade silently **[FIXED]**

If an expert errors in a round it drops out of `previous_round` for the rest of the discussion (arguably fine), but if **all** experts fail, the flow proceeds: an empty opening round produces round-2 prompts containing an empty "what the other panelists said" section, burning tokens on a broken discussion, and the run still "succeeds". Consider aborting the flow (or at least the discussion rounds) when a round yields zero results, and surfacing a top-level `executionError`.

### P3 — smaller items **[all FIXED except the iOS/Package.swift note]**

- **Mock provider's critique detection is stale**: it keys on `"Other panel experts have generated"` ([lib.rs:828](../crates/core/src/lib.rs)), which only the *coding* critique prompt still contains — discussion rounds 2+ now say "what the other panelists said", so mock runs return "initial draft" text for every round. Tests still pass, but the sandbox no longer exercises the distinction it was built for.
- **`temperature` is effectively dead config**: the Swift side hardcodes `0.7` ([CouncilViewModel.swift:528](../platforms/apple/Sources/App/CouncilViewModel.swift)), core ignores it for OpenAI/Anthropic by design, and Gemini's fallback default is also 0.7. Either remove the field or wire it through as a real setting.
- **Workspace scanner has no directory ignore list** ([CouncilViewModel.swift:241-266](../platforms/apple/Sources/App/CouncilViewModel.swift)): pointing it at a Rust or Node project indexes `target/`, `node_modules/`, `.build/` — thousands of junk entries that swamp the file picker. Add a standard ignore set; also consider a whitelist of text extensions instead of a binary blacklist, and a size cap on attached file content.
- **Unclamped UserDefaults loads**: `activeExpertCount` is restored without clamping ([CouncilViewModel.swift:148-151](../platforms/apple/Sources/App/CouncilViewModel.swift)); a stale/corrupt value > 8 crashes `ForEach(0..<count)` indexing into the 8-element config array. Clamp on load like `councilRounds` does (and give `councilRounds` an upper clamp too).
- **Package.swift declares iOS 17** but `build_frameworks.sh` only produces a macos-arm64 slice — the iOS declaration is aspirational and will confuse a future porting attempt.
- **Version drift**: the UI hardcodes "v0.8.0" ([ContentView.swift:239](../platforms/apple/Sources/App/ContentView.swift)); Cargo manifests say 0.1.0; the app bundle plist says 1.0; there are no git tags. Pick one source of truth.

---

## 4. Code quality

- **`crates/core/src/lib.rs` is a 1,570-line monolith** holding three provider clients, two orchestrators, model discovery, file-edit parsing, subprocess execution, and tests. Split into modules (`providers/{openai,anthropic,gemini,mock}.rs`, `council.rs`, `coding.rs`) before it grows further — this also makes the next finding tractable.
- **Heavy duplication**: each client's `generate` and `generate_stream` duplicate the entire request-building block (~60 lines each), and the three clients repeat the same shape. A shared `build_request(expert, prompt, attachments, history) -> RequestParts` per provider would remove ~300 lines and make fixes (like the Gemini role bug) single-site. Similarly, `run_agent_coding_flow`'s draft and critique loops are near-identical spawn blocks that don't reuse `run_expert_round`.
- **Naming drift**: `critique_rounds` now means "retry once on build failure" (the UI label already says so) but keeps its old name and `u32` type for what is effectively a bool gate. `Role::ExpertDraft/ExpertCritique` carry `expert_id`s that nothing reads — the Swift side maps everything to `.assistant` — dead complexity across the FFI surface.
- **Test coverage is thin where it hurts**. The bugs real-world testing caught (Gemini stream parsing, temperature rejection) are exactly the kind that golden-fixture tests prevent: feed each parser canned SSE bytes (including a multi-byte char split across chunks, `[DONE]`, malformed lines) and assert emitted chunks. `parse_file_edits` needs edge-case tests: unclosed tags, single-quoted attributes, `../` and absolute paths (tied to the P0 fix). These are cheap and directly protect the least-observable code in the project.

---

## 5. Process & documentation alignment

The repo defines its own working agreements in AGENTS.md; several are currently not being met, and the docs disagree with each other in places:

- **CHANGELOG is ~7 code commits behind** — nothing after 2026-07-13 is recorded, which means the most significant redesign in the project's history (multi-round discussions `912ba39`, Chairman removal + live-bug fixes `804afa7`, model-list dropdowns `7ee6457`, thinking notes `7bce907`, resizable grid `0481f59`, round-robin chat `672245a`, build consolidation `5c81e86`) has no changelog entry. Also **no entry carries the commit SHA** that AGENTS.md §2 mandates.
- **The AGENTS.md §3 code-review process has never run**: no `notes/code-review-*.md` exists and STATE.md has no "Latest code review" pointer line for §3 to repoint. Either run the first review (range: all commits, per the "no prior review" rule) or amend §3 to match reality.
- **AGENTS.md itself is stale**: the Core Stack Matrix still says orchestration includes "Chairman synthesis" (removed 2026-07-14).
- **STATE.md ↔ roadmap contradiction**: STATE.md records Milestones 5, 6, 7 and the M8 first pass as completed; architecture-and-roadmap.md still shows all their checkboxes unchecked and labeled [SUGGESTED].
- **Broken/stale links**: STATE.md line 5 links AGENTS.md as `../../AGENTS.md` (points outside the repo; should be `../AGENTS.md`); CHANGELOG.md's header still says it lives at `Notes/Council-of-Experts/CHANGELOG.md`, a pre-split path.

None of these are code bugs, but this repo's whole documentation system is built on the premise that a fresh session can trust STATE.md/CHANGELOG.md — right now it can't, and the drift started the same week the rules were written.

---

## 6. Recommended order of work

1. **Fix the multi-turn correctness cluster** (Gemini `model` role; Anthropic alternation handling; rethink history modeling so experts aren't credited with each other's words). This is the highest bug-per-effort ratio and unblocks trustworthy real-world use.
2. **Close the agent-mode write hole** (path containment) — small fix, removes the worst-case outcome. Gate any further Milestone 8 work behind an explicit consensus-mechanic design decision (§2.1).
3. **Robustness pass on streaming**: byte-buffered SSE parsing, request timeouts, a Stop button.
4. **Key hygiene**: Gemini key to header; keys to Keychain.
5. **Include each expert's own prior statement in round prompts**, then build the small evaluation harness (§2.2) — it will tell you whether rounds > 2 help and whether the Chairman should return (Milestone 9).
6. **Docs/process catch-up**: backfill CHANGELOG with SHAs, sync AGENTS.md/roadmap/STATE.md, fix links, run the first §3 code review.
7. **Refactor `lib.rs` into modules + add SSE/parser fixture tests** — best done after 1–3 so the tests lock in the fixed behavior.

---

_First evaluated commit: `8c69b3b` (earliest in history reviewed) · Last evaluated commit: `f9263bf` (HEAD)._
