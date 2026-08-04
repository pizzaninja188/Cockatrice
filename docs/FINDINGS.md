# Codebase Audit Findings

> **Status (2026-08-02):** every pending finding below was re-verified against the current tree on
> 2026-08-02. Findings that had since been fixed were deleted (see *Resolved since the audit*);
> line references and rationale were refreshed for the ones that survived.

## Applied Fixes

### 2026-08-03

- **`HandSlotMap` rode along on every ruled batch** (`ruled_game_driver.{h,cpp}`,
  `cockatrice/src/game/ruled/ruled_event_dispatcher.cpp`): `appendServerObjectMaps` now caches the
  last-broadcast map and injects the event only when the mapping actually changed — plus whenever
  the participant set changes (a joiner or reconnector starts with an empty client map) and on
  `resetForNewGame`. The client contract flipped to match: `resetPerBatchLegalActions` no longer
  clears `ownedCardToEngineHandSlot` per batch, and `applyHandSlotMap` clears before filling, so an
  **absent map means unchanged** and a **present map is a full replacement** — the same shape
  `applyBattlefieldObjectMap` / `applyGraveyardObjectMap` already had. The map therefore disappears
  from the large majority of batches (priority passes, mana taps, phase rolls), where it was
  previously re-serialized once for the batch and again per participant during redaction. The
  audit's suggested `if (hm->entries_size() > 0)` guard was **rejected as inert**: the map is built
  across *all* players' hands, so it is empty only when every hand is empty, and it says nothing
  about the batches the finding actually costs — the ones with no hand *change*. Deliberate
  over-emission kept: the comparison is over the pre-redaction all-players map, so an opponent's
  hand change re-sends a recipient their unchanged rows (correct, and keeps the cache single-copy).
  Coverage: a driver-level test asserting emit-on-change/participant-change/new-game, and the client
  test that asserted the old per-batch-clear contract rewritten as `HandSlotMapPersistsUntilReplaced`.
  Full build and all 18 C++ tests exit 0, including `ruled_e2e_smoke_test` (real servatrice +
  sidecar, full scripted ruled game).

- **Sidecar connections had no idle guard and left Nagle on** (`tricerules-server/src/main.rs`):
  `read_proto` is now wrapped in an idle timeout, so a peer that dies *without closing its socket* —
  a remote/partitioned Servatrice, or a hung one; a local crash already closes its sockets and hits
  EOF — no longer parks the task on `read_exact` forever holding a live `GameEngine`. The timeout is
  **split** (`IdleTimeouts`), because dropping a connection that carries a game kills that game for
  good: **60 s before `SessionStart`** (a connection with no session holds nothing, and Servatrice
  sends `SessionStart`/`ValidateDeck` within one round trip of connecting — no connection sits open
  while players pick decks) and **4 h once a session exists**, where no plausible game reaches it.
  The audit's suggested flat 30 s was rejected: the relay keeps one connection per ruled game, idle
  between commands for as long as players take to act. `TRICERULES_IDLE_TIMEOUT_SECS` overrides the
  session leash and caps the pre-session one; `0` disables both. An idle drop returns `Ok` and logs
  its own line rather than surfacing as `connection error:`. Also sets `TCP_NODELAY` on accept and
  collapses `write_proto`'s two `write_all` calls into one `encode_frame` buffer, plus a
  Ctrl+C/SIGTERM shutdown arm on the accept loop with an `Arc<AtomicUsize>` live-session count (the
  `signal` tokio feature was already enabled and unused; `time` was added). Coverage: env-policy and
  framing unit tests plus four real-loopback-socket tests (silent peer dropped, disabled timeout
  keeps it, a started session held to the long leash, request/response round trip through the new
  framing). Full `cargo test`, Clippy with warnings denied, `cargo fmt --check` and all 18 C++ tests
  exit 0; the drop was also confirmed against the real binary at a 3 s timeout.

- **A dropped engine connection froze the game silently instead of announcing itself**
  (`rules_relay.{h,cpp}`): found by manual testing of the idle timeout above, but pre-existing and
  reachable any time the sidecar restarts or the socket dies mid-game. `RulesRelay::connectIfNeeded`
  transparently reconnected after the socket dropped; the sidecar keys the engine session to the
  *connection*, so the fresh one answered every command `ok=false "no session"` — a successful
  transport, which `RuledGameDriver` reports as a plain `RespContextError` that the client renders
  as nothing. Players saw buttons doing nothing and no error, and the existing
  `handleRuledEngineConnectionLost()` notice ("this ruled game can no longer continue… please
  concede or leave") never fired. `RulesRelay` now tracks `sessionActive` from a successful
  `sessionStart`, and once set, a dropped socket fails instead of reconnecting — a reconnect cannot
  rebuild engine state, so pretending it can was the bug. Covered by a new `ruled_e2e_smoke_test`
  case that runs a real sidecar at a 1 s idle timeout, waits for the hangup, and asserts the popup;
  verified to fail (no popup, 20 s timeout) with the guard neutralized.

### 2026-08-02

- **Noncombat damage now records deathtouch from its source** (`tricerules-core/src/engine/`):
  Centralized permanent damage for `DamageTarget`, `DamageTargets`, and `DamageAll`, added
  generation-scoped last-known keyword snapshots for abilities whose sources leave and return,
  and expired deathtouch history after each state-based-action check. Regression coverage includes
  activated, divided, mass, fully prevented, leave-and-return, and indestructible-survivor cases.
  Focused scenarios and unit tests, full `cargo test`, Clippy with warnings denied, and
  `cargo fmt --check` all exit 0.

- **Game-over commands now emit a complete terminal batch** (`tricerules-core/src/engine/`): Fixed
  the dead `EngineError::GameOver` path by appending the winner log to the successful command batch,
  preserving resolution/state events and clearing all legal actions once a winner is set.
  Concession, empty-library loss, and lethal life loss scenarios now verify the terminal response;
  focused scenarios, full `cargo test`, Clippy with warnings denied, and `cargo fmt --check` all exit 0.

- **Trample attacker with one blocker omitted its damage-assignment label** (`tricerules-core/src/engine/combat.rs`, `engine/legal_actions.rs`): Fixed by centralizing the explicit-assignment predicate used for multiply-blocked attackers and single-blocked attackers with trample, then using it for combat state, assignment completion, and `LegalActions.labels`. The Colossal Dreadmaw versus Grizzly Bears scenario now verifies the active player's named assignment prompt and the defender's waiting prompt. Focused scenario test, full `cargo test`, Clippy with warnings denied, and `cargo fmt --check` all exit 0.

- **Spell-target candidates were published in nondeterministic `HashMap` order** (`tricerules-core/src/engine/targeting.rs`): Fixed by enumerating battlefield and graveyard candidates from authoritative player zone vectors in APNAP order and stack candidates bottom-to-top from `GameState::stack`. This also fixes copied spells being omitted because they intentionally have no backing `GameObject`. Regression scenarios cover both active-player rotations, within-zone order, copied spells, and ability exclusion. Focused targeting scenarios, full `cargo test`, Clippy with warnings denied, and `cargo fmt --check` all exit 0.

### 2026-06-25

- **`ruledEngineConnectionLost` never reset between back-to-back ruled games** (`libcockatrice_network/.../server_game.cpp:519-524`): Fixed by adding `ruledEngineConnectionLost = false` to the per-game-start reset block in `doStartGameIfReady`. Without this reset, if game 1 lost the sidecar connection, `handleRuledEngineConnectionLost()` would return early for game 2 (guard on line 2138), sending no notification and never dropping `rulesRelay`. Every subsequent ruled command in game 2 would then time out against the dead relay instead of failing fast. Build: 14/14 C++ tests pass. Committed `03f4225a` and pushed.

## Resolved since the audit (2026-08-02 re-verification)

- **`ExileTargetGainLifeEqualToPower` used `owner` instead of `controller`** — fixed. `zones::exile_target_gain_life_equal_to_power` (`tricerules-core/src/engine/resolution/zones.rs:78-105`) now reads `o.controller` for the life gain and uses `o.owner` only to route the exile move.
- **Resolution fizzled if ANY targeted effect had no legal target** — fixed. `resolve_stack_top` (`tricerules-core/src/engine/resolution/mod.rs:213-224`) now fizzles only when `targeted_effects.iter().all(...)` have no legal target, per CR 608.2b.
- **No `TriggerCondition` for "beginning of each player's draw step"** — fixed. `AtBeginningOfDrawStep { player: CastTriggerPlayer }` exists, the `GameEvent::DrawStepBegin` arm scans all battlefields in APNAP order, and Howling Mine / Kami of the Crescent Moon are implemented.

- **`UpkeepBegin` scan covered only the active player's battlefield** *and* **no `TriggerCondition` for "each player's / each opponent's upkeep"** — both fixed together, as this file recommended. `AtBeginningOfControllerUpkeep` became `AtBeginningOfUpkeep { player: CastTriggerPlayer }` (default `Controller`), `GameEvent::UpkeepBegin` gained a `player` payload so `trigger_player_for` routes "that player", and the APNAP preamble four arms had copy-pasted became `GameEngine::battlefield_sources_apnap` — one correct implementation, which is the real fix; the `UpkeepBegin` arm rolled its own and that is how it drifted. Shipped with Sulfuric Vortex (`AnyPlayer`; partial — the CR 614 lifegain-prevention clause) and Phyrexian Arena (`Controller`), plus nine scenario tests in `tricerules-core/tests/scenario/triggers.rs`.

  Writing those tests surfaced a **deeper bug the audit missed**: `finish_cleanup_roll_new_turn` (`engine/priority.rs`) walks Untap → Upkeep *inline* when a turn rolls and never fired `UpkeepBegin` at all. The `adv_on_empty_stack` `Untap` arm that does fire it is unreachable on a normal roll, because CR 502.1 gives nobody priority in the untap step. Upkeep triggers therefore never fired in normal play — which is exactly why the scan bug was invisible, and why "no shipped card uses the variant" was doing more work than it looked. The event now fires at that transition, before `ev_priority_changed` per CR 503.1a.

## Pending Findings

### Reusability / Scalability

- **Hard-coded 2-player assumption is pervasive** (`tricerules-core/src/state.rs:325,515-523`, `engine/mod.rs:210-212`): `defending_player_id_1v1()` returns `None` for anything but exactly 2 players and has ~10 call sites across `combat.rs`/`priority.rs`/`legal_actions.rs`; `OpeningSequence.mulligans_taken` is `[u32; 2]`; `GameEngine::new` rejects any player count != 2. Partially mitigated since the audit — `defending_player_id_1v1` carries a doc comment and `new` returns a clear `Illegal("M2: exactly 2 players")` rather than panicking — but the `[u32; 2]` array and the unaudited call sites remain the work item for any multiplayer expansion.

- **`ev_zone_view_sync` serializes the entire game state on every single `apply_command` return** (`tricerules-core/src/engine/mod.rs:620`): every priority pass, mana tap, and phase transition serializes all players' libraries (as comma-joined card-id strings), hands, and battlefields. For two 60-card decks, a typical turn produces 5–15 commands, each emitting ~200 strings into the zone-view. Correct-by-construction (authoritative snapshot) but O(deck size × commands per turn). Consider a delta-only zone view (emit only changed zones, or `ZoneDelta` events) for high-volume paths.

- **`ResolutionCtx::put_on_top_of_library` emits no intermediate event for Hand → Library moves** (`tricerules-core/src/custom/mod.rs:167-191`): the zone move is correct and Servatrice learns the final state via `zone_view`, but the pattern is asymmetric — `move_to_zone` emits `PermanentMoved` for graveyard/exile and nothing for hand/library. If replay animation or an intermediate reveal is ever added, this gap becomes a bug.

- **`discard_to_hand_size` proto conflates absent vs. zero for `hand_card_index`** (`ruled_v1.proto:193-198`, `priority.rs:472-477`): proto3 defaults `hand_card_index` to `0`, making "not set" indistinguishable from "card 0". The Rust code is correct today because it checks `hand_card_indices.is_empty()` first, but the interface invites a future caller to get it wrong. Suggest `oneof discard_selector { uint32 single_index; repeated uint32 batch_indices; }`.

- **`custom_effect` key uniqueness is never validated** (`tricerules-core/src/custom/mod.rs:288-294`, test at `tricerules-core/tests/scenario/custom_resolution.rs:331`): the existing test asserts every registry `custom_effect` key *resolves* to an impl, but not that each key is claimed by exactly one card id. Two RON cards accidentally sharing `custom_effect: "brainstorm"` would both resolve as Brainstorm with no error. Fix: extend `every_custom_effect_key_has_an_impl` to also assert a 1:1 key↔card-id mapping.
