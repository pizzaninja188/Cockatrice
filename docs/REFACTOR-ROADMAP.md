# Refactoring roadmap — ruled-mode fork at 35k-card scale

Audience: anyone (human or agent) doing **structural** work on this fork. For day-to-day card
implementation, AGENTS.md remains the authority; this doc governs refactors and records the
standing design rules that keep the codebase workable on the way to the full MTG card base
(~35k cards; ~855 implemented when this was written, 2026-07).

Grounded in a three-way audit (Rust engine, C++ server, Qt client) of the codebase as of
July 2026. Line numbers were verified then and will drift; the *shapes* they describe are the
point.

**Why this doc exists.** The audit's core finding: fork code is heavily interleaved inside
upstream files (`server_game.cpp` is ~56% ruled code; `game_event_handler.{h,cpp}` carry the
whole client ruled fan-out), and several structures grow linearly per mechanic added. Both
problems compound with every card implemented, and the fork wants to keep upstream files
mergeable so upstream features can be ported. The refactors below fix readability and merge
safety with the same move: extraction into fork-owned files.

**How to read it.** Two tiers, deliberately different:

- **Core sequence** — ordered, scheduled work where delay compounds: every added card or
  mechanic makes it harder. Execute in order; each step is roughly one focused session/PR
  series.
- **Trigger-gated backlog** — explicitly *unscheduled*. Each entry names its trigger; none is
  ever "next up" by default. Do not pick a backlog item over the next core step.

Card *data* additions (RON batches) can continue throughout — they touch none of the files the
core sequence moves, except occasionally `primitives.rs`.

---

## Standing design rules

These apply to all work from now on, refactor or not.

1. **Extraction, never in-place restructuring, for upstream files.** The fork delta in an
   upstream file must converge toward: a member pointer, one friend declaration, and 1–3-line
   call-site hooks. Everything else lives in new fork-owned files. Never rename, reorder, or
   rewrite upstream code in place — every such diff is a future merge conflict.
2. **Greppable ownership.** Every fork-owned C++ file is prefixed `ruled_` (existing:
   `ruled_utils`, `rules_relay` predates the convention). New client fork files live in
   `cockatrice/src/game/ruled/`. One `grep -rl ruled_` shows exactly what is fork territory.
3. **Pure code motion first, behavior change second — never both in one PR.** Every landing
   compiles and tests end-to-end (C++ and Rust), per AGENTS.md.
4. **Player-set-generic, always.** Multiplayer/Commander is eventually in scope. No primitive,
   proto field, engine helper, or UI flow may assume exactly one opponent: "each opponent" /
   "each player" are player *sets*; turn order is a rotation, not a toggle. Any 2-seat-only
   simplification carries a comment naming the assumption. (Actual multi-seat support is
   deferred indefinitely — this rule is about not poisoning the well.)
5. **Prioritize by bleeding rate.** The clock that matters: every new mechanic currently adds
   lines to `server_game.cpp`, `game_event_handler.{h,cpp}`, `RuledPerPlayerView`,
   `resolution.rs`'s match, and `game_prompt_widget`'s state. Work that stops that bleeding
   outranks work that doesn't.

---

## Core sequence

### Step 1 — Hygiene (½ day)

> **Done 2026-07-23.** The committed build log was removed, scenario imports were normalized,
> stale root notes moved under `docs/`, and upstream-owned trees were left untouched (commit
> `a53b3ca5`).

- Delete `tricerules/build_log3.txt` (committed build log).
- Fix the two scenario test files importing `super::helpers` instead of `crate::helpers`
  (`equipment.rs`, `regenerate.rs`).
- Move the stale root working notes into `docs/` with a one-line status header each:
  `issues.md`, `AUTOMATION_STATUS.md`, `engine-and-scenario-module-split-plan.md`,
  the copy-effects and multiface working plans (their remaining work now lives in `issues.md`).
  Delete any whose work fully landed. Root keeps `README.md` + `AGENTS.md` + the `CLAUDE.md`
  pointer.
- Leave `doc/carddatabase_v3|v4` and `webclient/` alone — upstream content; deleting them
  creates permanent merge conflicts for zero benefit.

### Step 2 — Build/test loop speedups (½–1 day, pays off every later step)

> **Done 2026-07-23.** `windows-ninja-all` preset + `scripts/build-ninja.ps1` landed and made
> canonical in AGENTS.md (measured vs MSBuild `/m:16`: no-op 0.4 s vs 7.9 s, one-file rebuild
> 7.3 s vs 12.8 s). MSBuild presets kept for CI parity, now with `jobs: 16`. Targeted
> verification codified in AGENTS.md. Debug config measured ~11% faster per compile than
> Release on a heavy TU — not worth a second tree; Release stays the iteration config.

Agent turnaround on simple changes is dominated half by navigation (fixed by the extractions)
and half by the build/test loop, which is independently fixable:

- **Add a Ninja-based Windows preset** (`windows-ninja-all`: Ninja generator + MSVC toolchain
  via a vcvars environment) alongside the existing MSBuild presets in `CMakePresets.json`.
  Ninja's incremental builds skip MSBuild's per-project solution scanning. Benchmark a no-op
  build and a one-file rebuild against `windows-msvc-all` before switching AGENTS.md's
  canonical commands; keep the MSBuild preset for CI parity.
- **Set `jobs` in the build presets** (or document `-j`) —
  `cmake --build --preset windows-msvc-all-release` currently passes no parallelism to MSBuild.
- **Codify targeted verification in AGENTS.md**: for single-component changes,
  `ctest -R <test-name>` and `cargo test --test scenario <filter>` (plus clippy/fmt) satisfy
  the "relevant tests" rule; the full suite runs before commit, not on every iteration. Note
  which components each change class requires building (Rust-only → no C++ build;
  client-only → skip servatrice).
- Measure whether a Debug iteration config is faster per-compile before assuming; tests run
  Release today.

### Step 3 — Automated ruled E2E smoke test (1–2 days; safety net for Steps 4–5)

> **Done 2026-07-24.** `tests/ruled_e2e_smoke/ruled_e2e_smoke_test.cpp` (ctest name
> `ruled_e2e_smoke_test`, runs in ~1 s): spawns the real `tricerules-server` + servatrice
> (temp config, ports 17391/47997), drives two scripted protobuf-level QTcpSocket clients
> through one fixed seeded game — deck-validation block on Black Lotus + NotifyUser popup,
> ChooseStartingPlayer, one London mulligan + bottoming, land plays, mana taps, Lightning
> Bolt targeted cast (LifeChanged −3), Hill Giant combat (DeclareAttackers/DeclareBlockers/
> combat damage), Brainstorm tier-3 ResolutionChoiceRequired/SubmitResolutionChoice, and
> cleanup DiscardToHandSize. The clients are reactive (driven by LegalActions labels, zone
> views, and object maps), so the script stays legal for any shuffle; determinism comes from
> the new `COCKATRICE_RULED_SEED` env override in `startRuledSidecarSession` (test-only;
> the seed is verified via the sidecar's session-start log line and deliberately never
> broadcast to clients). SKIPs when either server binary is missing (`RULED_E2E_REQUIRE=1`
> forces failure); part of the default ctest run, so it executes before/after every
> extraction PR per the standard gate.

The ruled path has no automated end-to-end coverage: engine tests are excellent, but nothing
exercises servatrice relay + sidecar + client wiring together — exactly the ~3,700 lines the
extraction steps move.

Build a minimal headless E2E smoke: launch `tricerules-server` + servatrice (start from the
`scripts/` launch helpers), drive two scripted clients (or protobuf-level scripted
connections — simplest thing that works) through one fixed seeded game exercising deck
validation, opening/mulligan, a land play, a targeted cast, one combat, one tier-3 resolution
choice, and cleanup discard; assert on the resulting game-log/event stream. The deterministic
seed makes the expected stream stable. Run locally before/after every extraction PR; CI
integration optional/later.

Scope note: this layer validates engine + relay + protocol wiring, **not** Qt client UI logic.
Client coverage arrives with Step 5's headless client-core suite. The full test stack then is:
Rust scenario tests (rules) → `tests/ruled_batch_tests/` (server translation) → client-core
batch tests (client translation) → `tests/game_prompt/` (presentation) → this E2E smoke
(everything wired together).

### Step 4 — Server extraction: `RuledGameDriver` (~1,500 lines moved, 3 PRs)

> **Done 2026-07-24** (3 commits, one per planned PR). `ruled_game_driver.{h,cpp}` and
> `ruled_player_binding.{h,cpp}` landed as pure motion; `server_game.cpp` shrank 2440 → 943
> lines and `server_player.h` carries zero ruled code (no `ruled_v1.pb.h` include). Residual
> upstream hooks: the `ruledGame` flag + owning `unique_ptr` + `friend class RuledGameDriver`
> + `ruled()` accessor + `processRuledPayload`/`getRuledPriorityPlayer` delegators on
> `Server_Game`; one `friend struct RuledPlayerBinding` on `Server_AbstractPlayer` (protected
> `sendCreateTokenEvents`); `getRuledGame()` guards in `server_player.cpp`. `RuledBatchTest`
> retargeted at the driver. The follow-up split landed with pass names matching the *actual*
> pass structure — `applyTokenCreations` / `applyPermanentMoves` / `applyPhaseStackAndZoneViews`
> / `applyAttachmentRestores` / `applyLifeManaAndCombatEvents`, and broadcast staged as strip →
> `appendServerObjectMaps` → `redactBatchForParticipant` — rather than the provisional names
> below (catalog indexing lives in `applyRuledStartupBatch`, not the batch path). Verified per
> commit: full build + full ctest (incl. `ruled_batch_test`, `ruled_e2e_smoke_test`); manual
> game still recommended before the next release-ish milestone.

`server_game.cpp` (upstream, 2440 lines) carries ~1,372 contiguous lines of ruled code plus a
helper block, bolted onto the upstream `Server_Game` class; `Server_Player` carries 8 more
ruled identity maps. Extract into new fork files in
`libcockatrice_network/.../server/remote/game/`:

- **`ruled_game_driver.{h,cpp}`** — class `RuledGameDriver`, one per ruled game, owned
  `std::unique_ptr` on `Server_Game`. Absorbs the 12 ruled members (`server_game.h:91–133`),
  the anonymous-namespace helpers (`server_game.cpp:73–169`, incl.
  `stripRuledServerOnlyEventsForBroadcast`), and all 15 ruled methods (`processRuledPayload`,
  `applyRuledBatch`, `broadcastRuledResponse`, `applyRuledStartupBatch`,
  `ruledCardIdForName/NameForId`, engine-loss handling, …; lines 1027–2399).
- **`ruled_player_binding.{h,cpp}`** — struct holding the 8 per-player maps currently on
  `Server_Player` (`engineOidToServerCardId`, summoning-sick/haste/trample/creature maps,
  graveyard map) plus the moved per-player methods (`applyRuledEngineZoneView`,
  `createRuledToken`, `findCardByEngineOid`, …) rewritten to take `Server_Player *` as a
  parameter; stored as `QHash<int, RuledPlayerBinding>` in the driver. This removes the
  `ruled_v1.pb.h` include from `server_player.h` entirely.

Upstream hook (~12 lines total in `server_game.h`):

```cpp
bool ruledGame;
std::unique_ptr<RuledGameDriver> ruledDriver;  // non-null iff ruledGame
friend class RuledGameDriver;                  // covers participants / currentReplay
RuledGameDriver *ruled() const { return ruledDriver.get(); }
```

The ~12 scattered `if (ruledGame)` sites become one-line delegations. Most of what the ruled
code needs on `Server_Game` is already public (`getParticipants()`, `prepareGameEvent()`,
`sendGameEventContainer()`, `gameMutex`); the friend line covers the rest. Retarget
`RuledBatchTest` at the driver so the old `friend class RuledBatchTest` can leave the upstream
header.

Follow-up PR (structure-only): split the 553-line `applyRuledBatch` into one named private
method per pass — `applyCatalogAndTokens`, `applyZoneViews`, `applyStackEvents`,
`applyCombatEvents`, `applyLifeManaAndPhase`, `buildObjectMaps` — same six-pass order; do
**not** merge passes (order dependencies are load-bearing). Split `broadcastRuledResponse`
into `stripServerOnlyEvents` + `redactForParticipant`.

Risk: medium — pure motion inside `Server_Game`'s call frames; mutex and participant lifecycle
unchanged. Verify: `RuledBatchTest` + the Step 3 smoke + one manual game.

### Step 5 — Client extraction: `cockatrice/src/game/ruled/` (~2,200 lines moved, 4–5 PRs)

> **Done 2026-07-25.** `cockatrice/src/game/ruled/` now holds `ruled_client_state.{h,cpp}`,
> `ruled_event_dispatcher.{h,cpp}`, `ruled_actions.{h,cpp}`, and a fourth file the plan below did
> not anticipate: **`ruled_client_host.h`**, a pure-virtual seam the state and dispatcher use to
> reach the Qt UI (local seat id, turn/phase writes, synthetic stack cards, P/T fallback, command
> transport, the modal-choice fallback, arrow resync). `GameEventHandler` implements it. That seam
> is what makes the new suite possible at all — the state and dispatcher compile with **zero**
> dependency on `AbstractGame` / `Player` / `CardItem`, so `tests/ruled_client_tests/` links just
> those two `.cpp` files plus `libcockatrice_protocol` and drives them with a `FakeHost`.
>
> `game_event_handler.h` went 851 → 204 lines and `game_event_handler.cpp` 3,146 → 956; the
> `RULED_PAYLOAD` case is now one line. The temporary-forwarder sub-step was skipped deliberately:
> writing a ~400-line forwarding header only to delete it in the next PR is pure churn when the
> whole move can be compiled and tested in one pass, so consumers were repointed to
> `geh->ruled()->…` directly and the redundant `Ruled*` name prefixes were dropped inside
> `RuledClientState` (`getRuledCombatPhase` → `getCombatPhase`, `ruledCombatStateChanged` →
> `combatStateChanged`, …). `RuledActions::isRuledGame()` took over **every** verbatim
> `getGameMetaInfo()->proto().ruled_game()` chain in `cockatrice/src` — `grep -rn 'ruled_game()'`
> now hits only the helper itself and the create-game dialog's checkbox (a raw proto field read,
> not a mode test).
>
> **`tests/ruled_client_tests/ruled_client_test.cpp`** (ctest `ruled_client_test`, 43 tests,
> ~0.04 s): identity maps both ways, per-kind legal-action label parsing (land + MDFC faces,
> cast + target flag, cleanup discard, the three opening modes), targeting tables by hand
> slot/face and by ability, requirement sets surviving preview echoes, stack LIFO + countered-spell
> cleanup + synthetic ability/copy cards, phase→toolbar mapping and first-strike transitions,
> attacker/blocker staging with preview commands and the rejected-declaration rollback, combat
> damage seeding (lethal-first and trample), zone-view pipe-delimited ability parsing, all five
> resolution choice kinds plus the modal fallback, opening-bottom index adjustment, and session
> reset. Verified: full build + full ctest (16/16, incl. `ruled_batch_test` and
> `ruled_e2e_smoke_test`); a manual game is still worth doing before the next release-ish
> milestone, since the E2E smoke drives protobuf-level clients, not the Qt UI.
>
> Not moved, and deliberately so: the ruled **pending-cast state machine** in
> `player_actions.cpp` (~1,300 lines — `PendingRuledSpellCast`, flex-pip mana payment, ability
> activation, damage allocation). It is local-player UI state reached *through* the click
> interpreters, not part of the engine mirror, and moving it would have doubled this step. See
> the new backlog entry.

`game_event_handler.h` (851 lines) is ~80% ruled members/methods/signals on an upstream class;
the `RULED_PAYLOAD` case (`game_event_handler.cpp:1416`, ~700 lines) handles all 18 event
kinds inline; `player_actions.cpp`, `tab_game.cpp`, `card_item.cpp` interleave more. Extract
three fork-owned units:

1. **`ruled_client_state.{h,cpp}`** — QObject parented to `GameEventHandler`; absorbs ALL
   ruled members, inline query methods, and `ruled*` signals from the handler (oid/card-id
   maps, `SpellTargetData` tables, combat staging, stack tracking,
   opening/cleanup/resolution-pick state).
2. **`ruled_event_dispatcher.{h,cpp}`** — `processBatch(payload)`; one private method per
   event kind replacing the 18 inline `has_*()` blocks; mutates `RuledClientState`, emits its
   signals.
3. **`ruled_actions.{h,cpp}`** — the command-sending and click-interpretation side currently
   spread through `player_actions.cpp` / `card_item.cpp` / handler free functions. Functions
   return `bool consumed` so upstream call sites become guards:
   `if (RuledActions::tryHandleHandCardClick(game, card)) return;`. Add one
   `bool isRuledGame(AbstractGame *)` helper replacing the 59 verbatim
   `game->getGameMetaInfo()->proto().ruled_game()` chains.

Migration in two sub-steps: (a) move state + dispatcher with temporary one-line forwarders on
the handler, proving a behavioral no-op; (b) mechanically repoint the ~14 consumer files to
`geh->ruled()->…` and **delete the forwarders** (a 400-line forwarding header defeats the
purpose). Qt `connect` lines in `tab_game.cpp` re-point to `geh->ruled()` — those one-liners
are the acceptable residual fork delta.

**Deliverable alongside the move: a headless client-core test suite**
(`tests/ruled_client_tests/`), mirroring `tests/ruled_batch_tests/ruled_batch_test.cpp` onto
the client: feed synthetic `ruled::v1` event batches into `RuledEventDispatcher`, assert on
`RuledClientState` — zone mirroring, legal-action parsing for every action kind, stack
tracking, pending-choice state, identity maps. This is what makes all-encompassing client
testing possible at all: today the logic is buried behind the UI; extraction exposes it as
plain QObjects testable offscreen with no rendering. Write the tests as each piece is
extracted, using current behavior as the oracle. The widget layer stays covered by the
`tests/game_prompt/` offscreen pattern. Qt module additions go in
`cmake/FindQtRuntime.cmake` per AGENTS.md.

Risk: medium-high (signal re-plumbing, lifetimes — parent `RuledClientState` to the handler;
preserve `clearRuledSessionState` semantics verbatim). Mitigated by the new suite + Step 3
smoke; manual games covering combat, targeting, tier-3 picks, opening as the final check.

### Step 6 — Small proto enums (4a/4b; ~1½ days, interleave anywhere after Step 1)

> **Done 2026-07-26** (2 commits, one per sub-step).
>
> **6a `ChoiceKind`.** Same tag, same varint — a pure type change. The hand-maintained Rust
> mirror is gone: `custom::ChoiceKind` is now a `pub use` of the generated proto enum (its
> `as_proto()` and the "kept in sync" comment deleted), `PendingResolution.choice_kind` is typed
> instead of `i32`, and the literals in `resolution.rs`/`continuous.rs` are named values. Server
> redaction reads `isPrivateChoiceKind()` in `ruled_utils` — one place naming the three
> concealed-zone kinds, unknown values treated as private — covered by `ruled_utils_test`. The
> client's local `kChoiceKind*` constant block is deleted. Two variants got the roadmap's shorter
> names (`RevealedCards` → `Revealed`, `PrivateRevealedHand` → `OpponentHand`).
>
> **6b `PhaseId`.** `PhaseChanged.phase` (string) is `reserved 1` and replaced by
> `PhaseId phase_id = 3`; `ev_phase_labeled(&str)` became `ev_phase(rv1::PhaseId)` across 24 call
> sites. The three hand-maintained label parsers are now `switch`es on the enum:
> `ruledPhaseLabelToCockatricePhase` → `ruledPhaseToCockatricePhase`, and the client's
> `mapRuledPhaseSlug*` pair → `mapRuledPhase*`; `RuledClientState::lastEnginePhaseSlug` (QString)
> → `lastEnginePhaseId` (`ruled::v1::PhaseId`), which is why `ruled_client_state.h` now includes
> `ruled_v1.pb.h` — the no-`AbstractGame`/`Player`/`CardItem` rule is unaffected. Two deliberate
> extras in the enum: `PHASE_ID_ASSIGN_COMBAT_DAMAGE` (a fork pause inside the combat damage step,
> not a CR step) and `PHASE_ID_CLEANUP` (never emitted today — clients keep highlighting the end
> step through cleanup — but both C++ parsers already had a branch for it). Mapping tables are
> unchanged, including the asymmetry where the server maps assign-combat-damage to no toolbar slot
> while the client maps it to the declare-blockers slot.
>
> Verified per commit: full ninja build + full ctest (16/16, incl. `ruled_batch_test`,
> `ruled_client_test`, `ruled_e2e_smoke_test`) and `cargo test` + `clippy -D warnings` + `fmt`.

Proto is fork-owned with no deployed users: breaking changes are fine; keep the `reserved`
discipline for removed tags. Each step is one end-to-end commit (C++ + Rust).

- **`choice_kind` → enum.** `ResolutionChoiceRequired.choice_kind` is an untyped `int32`
  documented in comments and duplicated as magic ints in server redaction
  (`== 0 || == 2 || == 4`) and client dispatch (`== 3`, `== 5`, …). Add
  `enum ChoiceKind { CHOICE_KIND_HAND_CARDS = 0; REVEALED = 1; LIBRARY_SEARCH = 2;
  TARGET_OBJECTS = 3; OPPONENT_HAND = 4; LEGEND_KEEP = 5; }` and change the field type —
  wire-identical varint, same tag. Replace all magic ints with named values; add an
  `isPrivateChoiceKind()` helper next to the server redaction.
- **Phase labels → enum.** Phase names are free strings (`"main1"`, `"begin_combat"`) emitted
  by Rust (`engine/priority.rs`), re-parsed by a 13-branch chain in `ruled_utils.cpp`, and
  independently re-parsed in the client — the same label set hand-maintained in three places.
  Add `enum PhaseId` + a `phase_id` field on `PhaseChanged`; Rust emits it; both C++ consumers
  switch to it; then delete the string field (keep `LogMessage` for human-readable text).

### Step 7 — Client generic-action model (3 PRs; after Step 5, inside fork files)

> **Done 2026-07-26** (3 commits, one per sub-step).
>
> **7.1 hand actions.** `RuledHandActionKind` + `RuledHandActionSet` +
> `QHash<RuledHandActionKind, RuledHandActionSet> handActions` replaced the three per-action member
> families. The three label parsers became one table-driven pass over `LegalActions.labels()` —
> `HandActionLabelSpec { kind, regex, ThirdCapture }` rows, where the third capture is a face index
> (CR 712 MDFC lands) or the `, target` flag — and the four `RuledActions::resolve*HandIndex`
> functions became `resolveHandActionIndex(state, kind, card)`. `RuledLandFaceOption` →
> `RuledFaceOption` (no longer land-specific). `ruled_actions.h` forward-declares the kind enum with
> a fixed underlying type so it stays free of the generated proto header. Two behaviours are
> deliberately kind-specific and stayed: OpeningBottom resolves a clicked card against *all* legal
> slots rather than by card name, and CastSpell keeps the multi-face `A // B` name fallback. This
> consolidated the label parsing but kept the stringly-typed wire format it parses — replacing that
> with a structured message is **8c**, deferred so the proto churn rides Step 8's single pass.
>
> **7.2 pending choices.** `PendingCopyTargetChoice`, `PendingLegendKeepChoice`, the five
> `pendingTrigger*` members and `ResolutionHandPick` collapsed into one
> `std::optional<RuledPendingChoice>` with a `Kind` (TriggerTarget / CopyTarget / LegendKeep /
> ResolutionPick). The holder is exclusive — the engine parks one choice and blocks — so
> `setPendingChoice()` tears down what it replaces (closing the revealed-cards popup) and
> `clearPendingChoiceOfKind()` is how a follow-up engine event retires the one choice it answers;
> one `sendResolutionChoice()` is the only `SubmitResolutionChoice` sender. **Not** folded in: the
> trigger *stack bookkeeping* (`lastTriggerSourceOid` / `lastTriggerAbilityIndex` /
> `lastTriggerControllerPlayerId`). `TriggerNeedsTarget` is recorded on every client because the
> synthetic stack card and its source arrow are built from it on seats that never choose the target;
> only the controller parks a choice. Behaviour change: a new choice now displaces a stale one of a
> different kind instead of the two coexisting.
>
> **7.3 prompt state.** Thirteen per-mechanic members in `GamePromptWidget` became two:
> `RuledPromptState { PromptMode mode; int required, selected; QString text; QVector<int>
> openingPickSeatIds; }` plus a `TargetingSources` flag set. `PromptMode` is
> Normal / Targeting / ClickChoice / CleanupDiscard / ResolutionPick / OpeningChooseFirst /
> OpeningMulligan / OpeningBottom, and `effectiveMode()` holds **the** priority chain that both
> `updateCombatButtonsVisibility()` and `refreshPromptLabel()` now switch on. Targeting is derived,
> never pushed: three PlayerActions signals (spell targeting / cast pending / ability target) raise
> and drop its sources independently, so they stayed as thin flag setters rather than becoming modes.
> The mode decision moved out of the widget's internal bool chain *and* out of `tab_game`'s
> `enginePromptFeed` lambda into one `TabGame::refreshRuledPromptState()`, called from every ruled
> signal that can change it. Two small deviations from the plan above: `openingPickSeatIds` rides in
> the state struct (mode payload, not an orthogonal input), and a resolution pick with `required == 0`
> now suppresses the composed phase label like every other pick (the old `> 0` test let it through).
>
> Verified per commit: full ninja build + full ctest (16/16, incl. `ruled_batch_test`,
> `ruled_client_test`, `ruled_e2e_smoke_test`). New coverage: one `ruled_client_test` case for the
> choice holder's teardown-on-displace, extra assertions in the hand-action parsing cases (a cast
> label never lands in the land set; the name-keyed lookup works for opening-bottom too), and 7 new
> `game_prompt_widget_test` cases (mode priority, the targeting OR-set, pick/opening button gating).
> A manual game is still worth doing before the next release-ish milestone.

Changes the *slope* of client growth: per-mechanic cost drops from ~6 touch points (member
family + parser + setters + bool + label branch) to ~2 (enum value + switch case).

1. **One legal-hand-action model.** Replace the three near-duplicate per-action families
   (`legalRuledLandPlay*` / `legalRuledSpellCast*` / `legalRuledCleanupDiscard*`, their three
   `isRuledXLegalForHandIndex` / `getRuledXHandIndicesForCardName` /
   `resolveRuledXHandIndexForClickedCard` helper triples, and the three `parseRuledXActions`
   parsers) with:

   ```cpp
   enum class RuledHandActionKind { PlayLand, CastSpell, CleanupDiscard, OpeningBottom };
   struct RuledHandActionSet {
       QSet<int> handIndices;
       QMultiHash<QString, int> indicesByCardName;
       QHash<int, QVector<RuledFaceOption>> faceOptionsByIndex; // lands/casts
       QSet<int> needsTargetIndices;                            // casts
   };
   QHash<RuledHandActionKind, RuledHandActionSet> handActions;
   ```

   One parser, one `isLegal(kind, idx)`, one `resolveHandIndexForClickedCard(kind, card)`. A
   new hand-action mechanic (cycling, foretell, …) becomes an enum value + a parse case. Stop
   there — do **not** abstract the per-mechanic selection UX (toggle-sets vs single-click
   differ legitimately).
2. **One pending-choice holder.** Merge `PendingCopyTargetChoice`, `PendingLegendKeepChoice`,
   `pendingTrigger*`, and (partially) `ResolutionHandPick` into one
   `std::optional<RuledPendingChoice> { kind; candidates; prompt; min; max; ordered; … }`.
   Renderers stay kind-specific; state, clearing, and `SubmitResolutionChoice` submission
   unify.
3. **Prompt widget state enum.** In fork-owned `game_prompt_widget`, replace the ~10
   per-mechanic bools + setters with one
   `struct RuledPromptState { enum class Mode {…} mode; int required, selected; QString text; }`
   and one `setPromptState()`; `refreshPromptLabel()` becomes a switch on Mode. Keep
   combat/priority/sticky-error as orthogonal inputs — they genuinely coexist with the modes.

### Step 8 — Proto restructure + hidden-info classification (2–4 days; after Steps 4–5, before the keyword count grows)

Three changes, one field-by-field pass over `ruled_v1.proto`:

**8a. `RuledPerPlayerView` parallel arrays → structured messages.** The view has grown to ~40
fields, mostly index-aligned parallel arrays over the battlefield (`battlefield_tapped`,
`battlefield_object_id`, `battlefield_power/toughness/damage`,
`battlefield_haste/trample/first_strike/double_strike`, four pipe-delimited
`battlefield_activated_ability_*` strings, …). Every new permanent property means another
parallel `repeated` kept index-aligned across the IPC boundary and two consumers — this
message already produced one tag-collision/corruption bug (see its `reserved 3, 4` comment).
Replace with:

```proto
message AbilityInfo { string text = 1; string mana_cost = 2; string mana_produced = 3; string cost_label = 4; }
message BattlefieldObject {
  uint32 object_id = 1; string card_id = 2; bool tapped = 3; bool summoning_sick = 4;
  bool is_creature = 5; uint32 power = 6; uint32 toughness = 7; uint32 damage = 8;
  repeated string keywords = 9;                  // mirror engine Keyword serde names
  repeated AbilityInfo activated_abilities = 10; // kills the pipe-delimited encoding
  string counters_annotation = 11; uint32 attached_to_oid = 12; uint32 face_up_index = 13;
}
message HandCard { string card_id = 1; uint32 object_id = 2; }
```

Two deliberate choices: **keywords as a `repeated string`** (a new keyword = zero proto
change; fold `BattlefieldObjectMap.Entry`'s per-keyword bools the same way), and
**`AbilityInfo`** replacing the pipe-delimited sub-encoding (same bug class as the memorialized
corruption). Rewrite consumers in one commit: the Rust zone-view builder
(`engine/events.rs`), `ruled_player_binding.cpp`, `ruled_event_dispatcher.cpp`, and the
redaction path.

**8b. Hidden-info classification (the anticheat structural fix).** Today's redaction is a
denylist — `stripServerOnlyEvents` clears *known*-private fields — so every future proto field
leaks hidden information by default unless someone remembers to classify it. At hundreds of
mechanics, someone will forget. Replace vigilance with a guarantee: classify every
`ruled_v1.proto` field as **public / per-player / server-only** (field-comment convention or a
custom option), and add a protobuf-reflection test that enumerates all fields of
broadcast-reachable messages, **fails when any field is unclassified**, and asserts the
redaction pass actually clears each non-public field. New fields then break the build until
classified. Land with 8a since it touches every zone-view field anyway; add a redaction unit
test to the ruled batch tests in the same pass.

**8c. `LegalActions.labels` → structured hand actions.** Same move as 8a, on the other
stringly-typed encoding. `labels` is a `repeated string` the engine `format!`s
(`engine/legal_actions.rs`: `"Play land {name} (hand idx {i}, face {face_index})"`,
`"Cast {name} (hand idx {i}, target)"`, `"Discard {name} (cleanup, hand idx {i})"`,
`"Put {name} on bottom (opening, hand idx {i})"`) and the client regexes back apart
(`handActionLabelSpecs()` in `ruled_event_dispatcher.cpp`, Step 7.1).

The defect is that one field serves two masters: `labels` is *both* the prompt-feed display text
and the only data channel saying which hand slot can do what. So a label can never be reworded or
localized without breaking gameplay, and the data can never be restructured without breaking the
log. It is also the odd one out on its own message — `valid_targets_by_hand_slot`,
`required_attacker_ids` and `undoable_mana_abilities` all arrive structured.

Add alongside `labels`, which stays and becomes display-only:

```proto
enum HandActionKind { HAND_ACTION_PLAY_LAND = 0; CAST_SPELL = 1; CLEANUP_DISCARD = 2;
                      OPENING_BOTTOM = 3; }
message LegalHandAction {
  HandActionKind kind = 1; uint32 hand_index = 2; string card_name = 3;
  uint32 face_index = 4;   // CR 712: one entry per (slot, face), mirroring today's labels
  bool needs_target = 5;   // CastSpell only
}
repeated LegalHandAction hand_actions = N;
```

One entry per *(slot, face)* rather than per slot: that is what the labels already emit for an MDFC
land, and the client's `faceOptionsByIndex` already groups them. `HandActionKind` replaces the
client-side `RuledHandActionKind` the same way 6a's `ChoiceKind` replaced its hand-maintained Rust
mirror. The parse table and `parseHandActions()` are deleted outright; `applyLegalActions` copies
fields.

This also closes a coverage hole worth naming: `ruled_client_test` hardcodes the label strings it
feeds in, so it only tests the parser against itself — the *contract* is checked nowhere but
`ruled_e2e_smoke_test`, and only for the four forms that script happens to drive. A new hand
mechanic gets no cross-language check at all, and the failure mode is silent (no regex match → empty
set → the card is simply unclickable, no error anywhere).

> **Completed 2026-07-26.** `RuledPerPlayerView` now carries structured hand, battlefield,
> ability, and graveyard data; both zone-view consumers use it directly, and battlefield keywords
> are generic strings. Every broadcast-reachable field has a `FieldVisibility` protobuf option,
> enforced by a reflection test, while Servatrice clears per-player and server-only data
> generically and restores only explicitly routed recipient data. `LegalHandAction` and
> `HandActionKind` are the gameplay contract for all four hand actions; labels remain display-only
> and the client label parser is gone. The Rust suite, clippy/fmt gates, full Windows Ninja build,
> full C++ test suite, client contract tests, and ruled end-to-end smoke test all pass.

### Step 9 — Rust: resolution split + primitives split (~3 days; before mass primitive growth)

> **Completed 2026-07-26.** `engine/resolution.rs` is now the
> `engine/resolution/` module: `mod.rs` retains stack setup, fizzle/custom handoff, shared
> helpers, and the single exhaustive `SpellEffectKind` dispatch; resolution logic lives in
> `damage`, `life`, `zones`, `pump_counters`, `mass`, `tokens`, `stack_ops`, and `misc`.
> `EffectCx` carries the shared resolution inputs, and an internal outcome preserves the
> existing early-exit behavior of effects that park a choice. `primitives.rs` is likewise a
> re-exporting `primitives/` module split into `effects`, `targeting`, `costs`, `abilities`,
> and `keywords`, leaving every existing `primitives::X` path and RON serde shape unchanged.
> Verified with the full Rust workspace test suite (including 271 scenarios and registry
> conformance), `cargo clippy -- -D warnings`, and `cargo fmt --check`.

**9a. `resolution.rs` → `engine/resolution/` directory.** The file (1,814 lines) holds one
36-arm `match effect { SpellEffectKind::… }` with some arms dozens of lines inline. Convert:
`mod.rs` keeps `resolve_top`, the fizzle check, the custom-resolution handoff, and the
**single exhaustive match** — but every arm becomes a one-liner delegating to a domain
submodule (`damage.rs`, `life.rs`, `zones.rs`, `pump_counters.rs`, `mass.rs`, `tokens.rs`,
`stack_ops.rs`, `misc.rs`). Introduce `struct EffectCx<'a>` bundling the ~8 values every arm
closes over (`&mut GameState`, registry, event buffer, targets, chosen_x/face, controller,
spell label) so extracted fns don't take 8 parameters. Exhaustiveness stays compiler-checked
because the match stays in `mod.rs`: a new `SpellEffectKind` variant is still a compile error
until an arm is added. Document the contract in the mod header: *new primitive ⇒ new arm + fn
in best-fit module; arms contain no logic.* Pure motion; the scenario suite is the net.

**9b. `primitives.rs` → `primitives/` module dir.** The file (1,148 lines) is the grab-bag
vocabulary: `SpellEffectKind`, `ContinuousEffectKind`, `StaticAbilityDef`, `TriggerCondition`,
`Keyword`, `TargetFilter`, `CounterKind`, `Amount`, `AbilityCost`, …. Split into `effects.rs`,
`targeting.rs`, `costs.rs`, `abilities.rs`, `keywords.rs` with `pub use submodule::*;`
re-exports so every existing `use crate::primitives::X` and all RON serde names stay
byte-identical. Near-zero risk; coordinate timing with any in-flight primitive PR. At hundreds
of primitives, `effects.rs` can split again along the 9a domains — the module dir makes that a
rename, not a redesign.

### Step 10 — Characteristics pipeline + `continuous.rs` split (with or right after 9a; strictly before any clone / control-change / type-change card)

> **Layer-2 slot filled 2026-07-29 (entry-time control only), by issue #20 / Reanimate.**
> `GameObject::controller` is now the CR 110.2 base value that `characteristics()` reads, and the
> per-player `battlefield` list became the control index (invariant asserted in `apply_sbas`). The
> sweep converted every `owner`-means-controller read — combat attack/block legality and lifelink
> attribution, trigger controllers and APNAP ordering, `emit_static_abilities_on_enter`'s anthem
> scoping — and fixed three owner-only `retain`s that would have stranded ghost permanents.
> `PermanentMoved.controller_player_id` and `BattlefieldObject.owner_player_id` carry the pair to
> the relay, which moves the physical card to the controller's table and annotates it
> `Owner: <name>`.
>
> **Still deferred:** layer-2 *continuous* control-change effects (Mind Control, Threaten,
> Confiscate). `apply_layer_2_control` remains an empty stub and documents the two traps its first
> implementation faces: it cannot use `ordered_effects` (circular via `pre_layer_6.controller` —
> CR 613.8), and once the derived controller can diverge from the field, the battlefield lists stop
> being a valid control index and need rebuilding. **Trigger: the first control-change card.**

> **Completed 2026-07-26.** `GameEngine::characteristics(oid)` now returns one owned
> `Characteristics` snapshot for controller, types/supertypes, colors, keywords, and P/T through
> explicit CR 613 layer slots. Layers 1–5 and the not-yet-needed layer-7 sublayers are named
> identity stages; layer 6 keyword grants and layer 7c/7d modifiers/counters moved into the
> pipeline in timestamp order. The ordered-effect boundary is the documented insertion point for
> deferred CR 613.8 dependency ordering, and the pure state/registry calculation is ready for
> memoization without changing callers. Battlefield legality, combat, zone views, triggered
> permanent-type checks, and last-known creature state for dies triggers now consume the pipeline.
>
> `continuous.rs` retains effect creation/expiry only; the characteristics evaluator lives in
> `characteristics.rs` and the CR 704 fixed-point loop plus its tests live in `state_based.rs`.
> The paired deterministic big-board scenario drives 200 creatures through a full turn with 20
> global layer effects and enforces the roadmap's <2s release bound. Verified with the targeted
> scenario/unit tests, release stress scenario, full Rust workspace test suite,
> `cargo clippy -- -D warnings`, and `cargo fmt --check`.

The engine's ordering skeleton is genuinely good: continuous effects carry CR 613.7
timestamps, layers 6/7c/7d apply in explicit order, a CR 704.4 fixed-point SBA loop runs
per-rule passes (704.5f/g/h/j/m/p), and prevention shields (CR 614.1a) and regeneration are
modeled. The gaps are the ones that turn into rewrites if retrofitted late:

- No single characteristics pipeline — P/T and keyword queries are separate helpers
  (`effective_power/toughness`, `has_keyword`), each re-walking effects.
- Layers 1–5 (copy, control-change, text-change, type-change, color-change) absent.
- No CR 613.8 dependency ordering, no CR 613.3 CDA handling, no CR 616 replacement-effect
  ordering.

**Act now:** introduce one entry point `characteristics(oid) -> Characteristics` computing all
derived characteristics through an explicit ordered layer pipeline — layers 1–5 as identity
functions today, layers 6/7 moved in from the existing helpers — and make it the **only** way
engine code reads derived P/T/types/colors/keywords. Build it memoization-friendly (this is
the classic MTG-engine hot path). Document the layer order and intentional simplifications in
the module header. While doing this, split `continuous.rs` (736 lines, three concerns): the
pipeline becomes `characteristics.rs`, the SBA loop `state_based.rs`, following the same
domain-file pattern as the rest of `engine/`.

**Defer with triggers** (see backlog): CR 613.8 dependency ordering; CR 616 replacement
ordering. The pipeline is built with slots for both.

### Step 11 — `CardDefinition` → faces-only (2 PRs, ~2–3 days; before mass per-card attributes)

> **Done 2026-07-26** (2 commits, one per planned PR).
>
> **11.1 (mechanical).** Every engine read of a flat `CardDefinition` field now goes through
> `face(i)` / `primary_face()` / a new `any_face(pred)`. `FaceRef` first gained the per-face data
> those call sites needed (`is_aura`, `must_attack_if_able`, `must_block_if_able`,
> `colors_override`, plus `colors()`); `CardFace` gained the matching `colors_override` slot.
> Three multi-face behaviours changed as a side effect, all in the direction of correctness —
> flat fields are *empty* for multi-face cards, so those reads previously answered "no": library
> and graveyard type filters now consult every face (CR 709.4; a split card is findable by
> Mystical Tutor, covered by a new scenario test), `GameEvent::SpellCast` carries the cast
> `face_index` so cast triggers filter on the face on the stack, and the conformance sweep plays
> a multi-face *land* face as a land instead of trying to cast it.
>
> **11.2 (storage flip).** `RawCardDefinition` is the serde-only authoring schema — today's RON
> byte-for-byte, flat fields and all — and `RawCardDefinition::into_definition()` in
> `registry.rs` is the single place that knows about it. Runtime `CardDefinition` is
> `{ id, name, layout, faces, partial }` with a **non-empty** `faces` vec; `FaceRef<'a>` is now
> literally `pub type FaceRef<'a> = &'a CardFace;` and the borrowed-mirror struct is gone, so a
> new per-card characteristic is declared once on `CardFace` instead of three times. The
> conversion also enforces the layout/faces invariant both ways (a non-`Normal` layout must author
> `faces`; a `Normal` card must not), with tests. `CardDefinition` no longer derives serde at all.
>
> Deviation from the plan below: `colors_override` moved onto `CardFace` rather than staying a
> whole-card field, because color is a face characteristic (CR 202.2a / 111.4) and `FaceRef` has
> to answer `colors()` without reaching back to the card. Tokens project to a one-face definition.
>
> Verified per commit: `cargo test` (273 scenarios + registry + conformance resolving every
> card), `clippy --all-features -D warnings`, `fmt --check`, and the checklist `--check` gate
> (855 full + 6 partial, unchanged). Rust-only — no C++ build needed.

`card_def.rs` carries three parallel representations of a card's characteristics: flat
top-level fields, a `faces: Vec<CardFace>` repeating the same fields, and a borrowed
`FaceRef<'a>` mirror — every new per-card attribute is added three times. Unify without
touching the 870+ RON files:

- Introduce a serde-only `RawCardDefinition` matching today's RON schema exactly (single-face
  authoring stays flat — do **not** force `faces:` wrappers onto 35k generated files).
  Registry load converts: `Normal` layout ⇒ `faces = vec![face_from_flat_fields]`; multi-face
  as authored.
- Runtime `CardDefinition` becomes `{ id, name, layout, faces, partial, whole-card flags }`;
  delete the flat runtime fields; `FaceRef<'a>` becomes `&CardFace` (keep a
  `pub type FaceRef<'a> = &'a CardFace;` alias during migration).
- Migrate residual direct `def.types` / `def.mana_cost` reads to `def.face(i)` first
  (mechanical PR), then flip storage.

Best-tested code in the repo (registry validation + conformance resolves every card + full
scenario suite), so medium risk despite the width.

### Step 12 — Docs & agent navigability (after the dust settles; pull the identity glossary forward if agents struggle sooner)

> **Done 2026-07-26.** [docs/ARCHITECTURE.md](ARCHITECTURE.md) landed with all ten planned sections
> (system + ownership table, life of a command traced through a Lightning Bolt cast, identity
> glossary, the driver's batch pipeline, trust model + fail-closed redaction, effect ordering,
> performance posture, fork-ownership table, extension recipes, test stack). Two additions the plan
> did not list: a **ruled command allowlist** note (`Server_AbstractParticipant::processGameCommand`
> is where a ruled game refuses freeform manipulation — worth naming in the trust model), and a
> `RuledGameDriver` section, since the batch pipeline is the thing readers most often need and it
> did not fit in "life of a command".
>
> [`cockatrice/src/game/ruled/README.md`](../cockatrice/src/game/ruled/README.md) covers the four
> units, a signal→consumer table, five invariants, and the test target. Writing that table surfaced
> one dead signal: `RuledClientState::triggerNeedsTarget` is emitted but has no consumer (the text
> reaches the panel via `enginePromptFeed`); it is documented as such rather than silently removed.
>
> `ruled_game_driver.h` gained the pipeline header comment — naming **five** `applyRuledBatch`
> passes plus the two broadcast stages, matching the Step 4 follow-up's actual structure, not the
> provisional "six passes" in the plan below.
>
> AGENTS.md: extraction-never-restructure and the `ruled_` prefix were promoted from a parenthetical
> inside the roadmap pointer to mandatory-workflow rule 6; an ARCHITECTURE.md pointer heads the
> Architecture section; the "Ruled prompt UI" section was rewritten for the post–Step-7.3 reality
> (`PromptMode` / `RuledPromptState` / `TabGame::refreshRuledPromptState()`, correct signal names).
> Three stale references fixed while there: `stripRuledServerOnlyEventsForBroadcast` (gone since 8b
> — now the reflection redactor), `Server_Game::ruledCardIdForName` (moved to `RuledGameDriver`),
> and `primitives.rs` (a module dir since 9b). `AGENTS.md` is now canonical and `CLAUDE.md` is a
> pointer to it.
>
> **Not done, deliberately:** the opportunistic `oid`/`cardId`/`serverCardId`/`handSlot` renames. The
> convention is documented in the glossary and applies to new and touched lines; a sweeping rename
> across settled fork files is diff noise with no reader benefit. Verified: full ninja build + full
> ctest (16/16) — the only code change is comments.

- **`docs/ARCHITECTURE.md`** — the one file read before any cross-component work:
  - System diagram: client ⇄ Servatrice ⇄ tricerules; who owns what state; the engine is the
    single writer.
  - **Life of a command**: click → `RuledActions` → `Command_RuledPayload` →
    `RuledGameDriver::processPayload` → `RulesRelay` IPC → engine dispatch → `RuledEventBatch`
    → driver applies to physical zones → two-stage redaction → client
    `RuledEventDispatcher` → `RuledClientState` signals → UI. One concrete traced example
    (Lightning Bolt).
  - **Identity glossary** — the most confusing undocumented thing in the codebase: engine
    `ObjectId` (oid) vs tricerules `card_id` (slug) vs Oracle name vs `Server_Card.id` vs
    engine hand slot vs face index; which maps convert which; who owns each (`CardCatalog`,
    `BattlefieldObjectMap`, `HandSlotMap`, `GraveyardObjectMap`). Adopt canonical variable
    names — `oid`, `cardId`, `serverCardId`, `handSlot` — and rename stragglers
    opportunistically.
  - **Hidden-info / trust model**: client untrusted; Servatrice trusted; sidecar fully trusts
    Servatrice (loopback-only port); what each boundary checks; where hidden info is stripped;
    private choice kinds. Freeform mode is explicitly out of scope (trust-based by upstream
    design — don't try to harden it).
  - **Effect-ordering guarantees**: the layer pipeline order, when SBAs run, timestamp
    semantics — so implementers and agents know where ordering is decided instead of
    re-deriving it per card.
  - **Runtime performance posture**: card-base size does not affect per-game runtime (games
    touch ~120 cards; registry lookups are O(1)). The hot paths that grow with *board
    complexity*: legal-action enumeration per priority window, triggered-ability scans per
    event, SBA fixed-point passes, characteristics recomputation, zone-view serialization.
    Measure (backlog stress test), don't pre-optimize.
  - **Fork-ownership table**: fork-owned (restructure freely) / upstream-with-hooks
    (extraction only) / pristine upstream. Doubles as the merge manual and the agent guardrail.
  - **Extension recipes**: "add a data-tier card / a primitive / a keyword / a UI prompt / a
    tier-3 card" — each a checklist of exact files.
- **AGENTS.md**: rewrite the stale "Ruled prompt UI" section (post-extraction reality); add
  the extract-don't-restructure rule, the `ruled_` prefix convention, and a pointer to
  ARCHITECTURE.md.
- **`cockatrice/src/game/ruled/README.md`** (class responsibilities + signal map) and a header
  comment in `ruled_game_driver.h` naming the six batch passes. Skip per-directory READMEs
  elsewhere — Rust module docs already carry that weight.

---

## Trigger-gated backlog

Unscheduled by design. Each entry fires on its trigger, not before.

- **Data-dir moves** — *trigger: anytime, trivial.* Move the 138 hand-authored flat RON files
  present as of 2026-08-05 (`data/*.ron`) into `data/authored/<letter>/` mirroring `generated/`
  (`build.rs` walks recursively — no code change; first confirm `gen-cards` skip-existing matches
  by id/name, not path). Normalize the few dash-named files (`bad-moon.ron`, …). Two-letter sharding of
  `generated/<letter>/` only when a single dir exceeds ~2,000 files (pure `git mv`).
- **Binary card-data embed** — *trigger: registry load > ~1s or visible CI wall-time
  regression (expected ~5–10k cards).* The `include_str!`-every-RON const array is fine until
  then. Fix: build.rs parses all RON at build time and embeds one postcard/bincode blob;
  startup becomes a single deserialize; RON syntax errors move to compile time; the
  `card_data_hash` determinism stamp hashes the blob. Keep embedding — the single-binary
  sidecar and replay hash depend on it; do **not** move to runtime file loading.
- **Big-board stress test** — **done 2026-07-26**, with Step 10. Lives in
  `tricerules-core/tests/scenario/performance.rs`: a seeded deterministic scenario driving 200
  creatures through a full turn with 20 global layer effects, asserting the < 2 s release bound.
  No optimization work until it (or real play) surfaces a number; the deterministic replay makes
  profiling exactly reproducible.
- **CR 613.8 dependency ordering** — *trigger: first card whose layers interact by dependency
  (Humility-class).* The Step 10 pipeline is built with a slot for it.
- **CR 616 replacement-effect ordering** — *trigger: first time two replacement effects can
  apply to one event.* Same: slot exists, machinery deferred.
- **Dev-loop tooling** — *trigger: when manual-testing pain outweighs ~3–4 days.* Three
  pieces, impact order: (1) **one-command game launch** — **done 2026-07-27**, see below.
  (2) **Dev console + `DevCommand`** — **done 2026-07-27**, see below.
  (3) **Scenario save/load via replay** (near-free from
  determinism): "save" dumps `(seed, command log)`; "load" replays into a fresh session and
  hands control to live clients — reusable tricky-board fixtures, shareable with the E2E
  harness. Pulls forward well right after Step 3 (shared plumbing).

  > **Piece (1) done 2026-07-27.** `scripts/launch-ruled-game.ps1` starts the sidecar, servatrice
  > and two clients, and both seats reach "ready" on their own — measured ~2 s from the command to
  > `startRuledSidecarSession`. `-Stop` tears the set down again (also the fix for the stale-`.exe`
  > link failure). Decks are `-DeckA` / `-DeckB`, defaulting to two implemented-card decks in
  > `scripts/decks/`; `-Seed` pins `COCKATRICE_RULED_SEED` so a board state can be replayed;
  > `-Freeform` and `-NoServers` cover the other two cases.
  >
  > **Deviation from "no engine changes / extend `scripts/` only":** there was no automation
  > surface to script against — the pre-game sequence is pure GUI. The autopilot is therefore a
  > new fork-owned client unit, `cockatrice/src/game/ruled/ruled_autopilot.{h,cpp}`, off unless
  > `--autopilot host|join` is passed. Upstream cost is one option pair plus one call in
  > `main.cpp`, and one `friend class RuledAutopilot` each on `TabSupervisor` and `TabGame`.
  > No engine, proto, or server change. It sends only the commands the buttons send and stops at
  > game start, so it drives the real UI rather than substituting for it.
  >
  > One thing the design pass surfaced, worth remembering for piece (2): a room's initial game list
  > rides in the `Response_JoinRoom` response, and only later changes arrive as `Event_ListGames` —
  > so an event-only trigger silently never fires for a game created before you logged in.
  > `Command_GetGamesOfUser` is the race-free lookup.
  >
  > **Piece (2) done 2026-07-27** (5 commits). `./scripts/launch-ruled-game.ps1 -Dev` gets a
  > command entry in each client's Messages dock. Two commands, not the six sketched above:
  > `put [seat] <zone> <card name> [ready]` and `mana [seat] <symbols>`. Those two kill both
  > measured pains — no deck editing, no lands, no turns passed — and `ready` closes the third
  > (attacking the turn a creature arrives). Draw N / set life / add counters / skip-to-phase /
  > act-as-player are designed for but deliberately unbuilt; skip-to-phase is the one to add when
  > upkeep/draw triggers become the next pain.
  >
  > **`put` conjures; `move` relocates.** A card in no decklist is minted from outside the game
  > (CR 400.11-shaped), which is what removes deck editing entirely. `put` was briefly a single
  > verb that guessed — move an existing copy if there was one, else conjure — on the theory that
  > tutoring should not silently duplicate. That was wrong: it made "give me a second Serra Angel"
  > impossible to express, and re-issuing `put bf` on a permanent yanked it off the board and
  > re-applied summoning sickness. Split into two explicit verbs. Conjuring is hand/battlefield
  > only — graveyard, exile and library keep separate physical binding maps, and `move` is how you
  > reach them (conjure to hand, then move).
  > This cost more than the tutor-only design, almost all of it in Servatrice, and the blocker was
  > not the obvious one: `applyRuledEngineZoneView` resolves physical cards by translating their
  > names through the **session card catalog**, built once from the decklists. An unknown name
  > matches nothing, and the reconcile then abandons the whole sync with only a `qWarning` — a
  > silent client desync. Hence `indexCardCatalogEvents` as batch pass 0 (extracted first, as pure
  > motion) and the engine re-emitting the catalog on conjure. `createRuledDevCard` is a sibling of
  > `createRuledToken`, not a reuse: a real card gets no `"Token"` annotation and no
  > `destroyOnZoneChange`, which a token needs for CR 111.7 but which would make a conjured
  > Lightning Bolt vanish on leaving the battlefield.
  >
  > **`put bf` fires `EntersBattlefield`, and that is load-bearing.** `fire_triggers`' arm for it
  > is the only path to `emit_static_abilities_on_enter`, so skipping it would leave a conjured
  > anthem granting nothing — no error, no log line. CR 601 cast triggers correctly stay silent
  > (nothing was cast), giving the same event profile as a real put-onto-the-battlefield effect.
  > Deliberately **not** covered: `put gy` does not fire dies triggers (CR 700.4 is specifically
  > battlefield → graveyard, and most uses come from the library or outside the game).
  >
  > The gate is two independent halves ANDed engine-side — `SessionStart.dev_commands_enabled`
  > from `COCKATRICE_RULED_DEV`, plus the sidecar's own `TRICERULES_DEV_COMMANDS` — so a production
  > sidecar cannot be talked into dev mode from upstream. The rejection sits in `apply_command`
  > *before* the `command_index` bump, so a refused command perturbs no determinism input and,
  > since Servatrice appends to `ruled_command_log` only on an ok response, never enters a replay.
  > Accepted ones are ordinary logged commands, so a dev-built board still replays exactly. Note
  > `RuledCommand`'s `reserved 9`: `AddManaToPool` was deleted because a tampered client could mint
  > mana, and `add_mana` re-opens exactly that surface — which is why the gate is not optional.
  > Both new event types are `SERVER_ONLY`, so a hand conjure cannot leak the card name; the
  > client learns of it through the redacted full-state resync instead of a broadcast event.
  >
  > Client: `ruled_dev_command_parser` (pure text → typed proto, linked into the headless test
  > target) and `ruled_dev_console` (the widget, which emits and never sends). The parse deviates
  > from the plan in one place worth knowing: a leading number is ambiguous for `mana` — `mana 12`
  > is twelve generic, `mana 2 UU` is the second seat — so it is read as a seat only when it is a
  > valid ordinal *with* symbols after it. Seats are 1-based ordinals, never raw player ids, since
  > with ids 0 and 1 the two readings collide. Verified: 20 scenario tests, 12 parser tests, a new
  > offscreen `ruled_dev_console_test`, 2 `ruled_batch_test` cases each for catalog indexing and
  > conjure minting, and the E2E smoke now conjures a Serra Angel (in neither decklist) and adds
  > mana — the only cross-language check that a C++-built `DevCommand` decodes in Rust. Full ninja
  > build + full ctest (17/17) and `cargo test` + `clippy -D warnings` + `fmt` per commit.
- **Security audit checklist** — *trigger: before any public deployment.* Verify and
  document: command sender identity is bound to the authenticated session server-side (never
  client-supplied player ids); the sidecar port (`TRICERULES_HOST`, default 127.0.0.1) is a
  full-trust boundary that must never be network-exposed (consider refusing non-loopback binds
  without an explicit override flag); replays contain both players' full hidden info by design
  (document before any replay-sharing feature ships).
- **Session crash-recovery** — *trigger: if/when wanted; a feature, not a refactor.* Replay
  `(seed, command log)` into a fresh sidecar to resurrect a session after a crash. The
  architecture supports it nearly for free; noting it here so nobody designs against it.
- **Test-harness split** — *trigger: `tests/scenario/helpers.rs` passes ~1,500 lines.* Split
  by concern (setup builders vs assertion helpers), not by card theme.
- **`PendingRuledSpellCast` extraction** — *trigger: when a new cast-time mechanic (kicker,
  additional costs, alternative costs) would add another parallel pending-* family to
  `player_actions.cpp`.* Step 5 left the ruled pending-cast state machine in place
  (`PendingRuledSpellCast` + `PendingActivatedAbility`, flex-pip mana payment, X prompting,
  multi-target collection, damage allocation — ~1,300 lines in the upstream
  `player_actions.{h,cpp}`). It is genuinely local-player UI state, not part of the engine
  mirror, and the click interpreters in `RuledActions` already gate every entry into it, so it
  is not on the bleeding path the way `game_event_handler` was. When the trigger fires, move it
  to `cockatrice/src/game/ruled/ruled_pending_cast.{h,cpp}` with `PlayerActions` keeping a
  member pointer — and fold the three pending-* families into Step 7's one pending-choice holder
  in the same pass rather than porting them as-is.

---

## Do NOT do

Over-engineering traps and rule violations, recorded so they stay decided:

- **No in-place restructuring or renaming of upstream code.** Don't move
  `game_event_handler.cpp` itself; don't delete `webclient/` or `doc/carddatabase_v3|v4`.
- **No per-set RON splits** and no set metadata in the rules DB — identity is name/slug-keyed;
  sets are an Oracle/display concern.
- **No scripting DSL** for card effects — the three-tier model + the "name two cards" widening
  rule is the scaling strategy; a DSL recreates tier-3 sprawl with worse tooling (see
  appendix: Forge).
- **Don't unify the freeform and ruled turn systems** — freeform machinery is load-bearing
  *inside* ruled games; ruled mode stays layered on top.
- **No proto versioning/back-compat machinery** — no external users; single-repo atomic
  changes are the feature. Keep the `reserved` discipline for removed tags.
- **Don't make `RulesRelay` async** — synchronous request/response under the game mutex is a
  simplicity feature at this scale.
- **Don't generalize the prompt widget into a UI framework** — one state enum is the right
  altitude; per-mode rendering stays hand-written.
- **Don't split the scenario test crate into multiple binaries** (link-time explosion), and
  don't shard test helpers per card theme.
- **No full-GUI click automation** (synthetically driving the QGraphicsScene board in a real
  window) — brittle, timing-flaky, slow; tried before and correctly abandoned. Client coverage
  lives at the view-model layer (headless batch-in/state-out tests) plus offscreen widget
  tests; real-GUI verification stays manual.
- **Don't pre-build backlog items** (binary embed, two-letter sharding, dependency/replacement
  ordering) — each has a cheap, named trigger.

---

## Appendix — prior-art lessons (Forge / XMage)

Architectural generalities from the two big open-source MTG engines, recorded so their
mistakes stay avoided and their wins get stolen. (General knowledge, not audited citations.)

**They bracket the design space.** XMage implements one Java class per card (~25k classes):
type-safe but crushing boilerplate and endemic near-duplication — the failure mode the tier-3
review gate exists to prevent. Forge implements nearly everything in a stringly homegrown
script DSL: proof that data-driven cards scale to ~30k, and proof of why the "no scripting
DSL" rule exists — the DSL grew into an untyped, undocumented language with runtime-only
errors. This fork's RON + typed `SpellEffectKind` is Forge's idea with a type checker; the
three-tier model is the deliberate middle path. Keep its gates.

**Never-do list from their pain:**
- No deep-copy-the-world for legality checks or AI (XMage's core slowness). This fork's
  substitute is determinism: `(seed, command log)` replay. A future AI/what-if feature must
  not import copy-everything.
- No blocking mid-resolution player callbacks (XMage's threading pain). The park/resume
  `ResolutionInterrupt` + logged-command design stays the *only* interaction pattern.
- Layers retrofitted late hurt both projects — independent validation of Step 10's timing.
- XMage's continuous-effects recompute is a known hot path — hence "memoization-friendly" in
  Step 10.

**Worth stealing:**
1. **Watchers** (XMage): a dedicated event-memory subsystem — "creatures that died this turn",
   "spells you've cast this turn" — that trigger conditions and effects query. Build it as a
   subsystem when the first Morbid/revolt-class card lands; never as one-off `GameState`
   fields per mechanic.
2. **Scenario-test DSL**: XMage's declarative test player + massive regression suite is what
   keeps 25k cards from rotting. `tests/scenario/helpers.rs` is this fork's equivalent — treat
   it as a first-class product; keep investing in one-line-per-action builders.
3. **Template-driven generation** (Forge's real scaling secret): a huge fraction of Magic text
   is templated ("Destroy target X", "Draw N cards", "Counter target spell"). Extending
   `gen-cards` beyond vanillas to recognize common Oracle templates and emit tier-1 RON — with
   the registry/conformance tests as the net — is the coverage-jump path to thousands of cards
   without hand-authoring.
