# Codebase Audit Findings

> **Status (2026-08-02):** every pending finding below was re-verified against the current tree on
> 2026-08-02. Findings that had since been fixed were deleted (see *Resolved since the audit*);
> line references and rationale were refreshed for the ones that survived.

## Applied Fixes

### 2026-08-04

- **Cleanup discard encoded "index 0" and "no selector" identically** (`ruled_v1.proto`,
  `engine/priority.rs`, `player_actions.cpp`): `DiscardToHandSize` had both a proto3 scalar
  `hand_card_index` and a repeated `hand_card_indices`. Because an unset proto3 scalar reads as
  zero—and setting it to zero writes no field—a one-card cleanup selecting the first hand slot was
  indistinguishable on the wire from an empty command. The scalar is removed and its number/name
  reserved; every cleanup now sends the repeated list, including `[0]` for a singleton. The engine
  consumes only that list and requires both the submitted count and distinct count to equal the
  excess hand size. That second check closed a related validation hole found by the new coverage:
  duplicate `[0, 0]` previously collapsed to `[0]` and could be accepted when one discard was due.
  The audit's suggested direct `oneof` was not used because protobuf forbids repeated fields inside
  a `oneof`, and there is no longer a second selector shape to discriminate. This is an intentional
  clean break: old scalar-form ruled clients and deterministic command logs are unsupported, and
  legacy index-zero bytes cannot be migrated because they contain no selector information. The
  general Cockatrice protocol version remains unchanged; ruled deployments continue to require
  same-tree client/server/sidecar builds.
  Manual verification then exposed a relay bug hidden by the protobuf-level E2E: battlefield and
  hand ObjectIds share `RuledPlayerBinding`'s map, and every `private_zones_unchanged` view cleared
  the whole map to rebuild the battlefield half but had no hand rows with which to restore the hand
  half. At cleanup, `PermanentMoved` could not resolve the selected engine ObjectId to a physical
  hand card, so the move was skipped and the following hand/library reconcile rejected an 8-vs-7
  count mismatch. The UI stayed at eight cards and its next click addressed a drifted hand slot.
  Battlefield rebuild now preserves mappings belonging to cards still physically in hand; a relay
  regression covers full hand sync → omitted private-zone view → cleanup move and asserts the
  exact selected `Server_Card` reaches the graveyard.
  Coverage also proves `[0]` survives a Prost round trip and the IPC decode path, multi-card cleanup
  succeeds, and empty/duplicate/wrong-count/out-of-range lists are rejected without changing zones,
  cleanup state, or `command_index`. Full `cargo test` (408 scenarios plus unit/integration/doc
  tests), Clippy with warnings denied, `cargo fmt --check`, the full Windows C++ build, and all 18
  C++ tests exit 0, including `ruled_client_test`, `game_prompt_widget_test`, `ruled_batch_test`, and
  the real Servatrice + sidecar `ruled_e2e_smoke_test`.
- **The engine's 2-player assumptions were fixed-arity data plus ten ambiguous call sites**
  (`tricerules-core/src/state.rs`, `engine/{mod,opening,combat,priority,legal_actions}.rs`):
  `OpeningSequence.mulligans_taken`/`resolved` became seat-sized `Vec`s, and the two places
  `opening.rs` hand-rolled seat order (`1 - mulliganed_idx` and `let order = [start, 1 - start]`,
  plus `resolved[0] && resolved[1]`) collapsed into one pure `next_unresolved_from(&[bool], start)`
  — the same "an ordering with two implementations is an ordering that drifts" fix `apnap_rank` and
  `battlefield_sources_apnap` already got. At two seats the behaviour is identical; at N it is
  round-robin from the starting player, which is *closer* to CR 103.4 than the alternation it
  replaced.
  The real work was the audit. `defending_player_id_1v1` answered three different questions behind
  one `Option`, so every caller silently inherited the arity-2 assumption. It is gone, replaced by
  `defending_player_ids()` (every nonactive, non-lost seat in APNAP order), `is_defending_player()`
  and `sole_defending_player_id()`. **Six of the ten call sites were never arity-2 to begin with**
  and are now seat-generic: the four "is *this* player defending" guards (`mod.rs` ×2,
  `priority.rs`, `legal_actions.rs`), `required_attacker_ids`' "is there anyone to attack"
  (`defending_player_ids().is_empty()`), and the declare-blockers priority handoff (first defender
  in APNAP order). **Four genuinely need to name *the* defender** — `combat.rs`
  `defending_player_has_eligible_blockers`, `required_blocker_ids`, `set_blockers`, and combat
  damage — because `DeclareAttackers` is a bare creature-id list with no per-attacker defender to
  choose between. Those four now call `sole_defending_player_id`, whose doc comment names them as
  *the* list to revisit; `SUPPORTED_PLAYER_COUNT` in `engine/mod.rs` points at it and vice versa.
  Ten scattered greps became one chokepoint and one named constant.
  Also removed a panic: combat damage was `defending_player_id_1v1().unwrap()` and now returns
  `EngineError::Illegal("defender missing")` like `set_blockers` already did — a rejected command
  rather than a dead sidecar task.
  **The gate deliberately stayed.** `GameEngine::new` still rejects any count but 2. Lifting it
  needs attack-target selection per attacker in `ruled_v1.proto` plus client UI (CR 506.2's "a
  player or planeswalker the attacking player chooses"), which is a separate project — shipping a
  seat-generic core without it would just move the failure later.
  Coverage: four unit tests in `state.rs` exercising **3 and 4 seats** (wrap-around, all-resolved,
  APNAP rotation, `has_lost` exclusion) — the only way to prove genericity while the gate stands,
  and something the arithmetic they replaced could not be tested for at all; a test that
  `GameEngine::new` rejects 0/1/3/4 players (the gate itself was untested); and a combat test that
  the defender lookup yields `None` once the opponent has lost. `resolve_combat_damage` with no
  defender is not reachable through public commands (`sweep_life` names a winner first) and is
  `pub(super)`, so the test covers the guard's precondition and says so. The existing opening
  scenarios pass unchanged, which is the 2-seat regression proof. Full `cargo test` (405 scenario +
  all unit tests), `clippy --all-targets -D warnings` and `cargo fmt --check` exit 0. Rust-only, no
  `.proto` edit, so no C++ rebuild was required.
- **Custom-effect registration was a hand-written `match`, and one finding's premise was wrong**
  (`tricerules-core/build.rs` *(new)*, `src/custom/{mod,brainstorm,gifts_ungiven}.rs`,
  `tests/scenario/custom_resolution.rs`): two pending findings, closed together because they are the
  same file and the second turned out to argue against itself.
  **`put_on_top_of_library` emitting no Hand → Library event is correct, not a gap.**
  `PermanentMoved` is `FIELD_VISIBILITY_PUBLIC` on every field *including `card_id`*
  (`ruled_v1.proto:950-972`), so announcing a Brainstorm put-back would tell the opponent exactly
  which two cards were hidden on top of the library. Hand and library are hidden zones (CR 400.2)
  and reach each player only through the redacted per-player zone view. The silence stayed; what was
  missing was any statement or test that it is deliberate. The two ad-hoc `match` arms in
  `move_to_zone` became `public_move_event_destination`, exhaustive over `Zone` with no `_` arm, so
  a new zone variant must make the decision rather than inherit silence — plus a unit test asserting
  the rule directly and a scenario test that no `PermanentMoved` in a Brainstorm batch names either
  put-back oid or card id (verified to fail once the "fix" is applied). One behaviour delta:
  `Zone::Battlefield` now maps to `DESTINATION_BATTLEFIELD`, matching the engine's own reanimation
  emission, so "public zone ⇒ event" holds without exception; no custom effect reaches it today.
  Also corrected `move_to_zone`'s doc comment, which claimed the zone view omits the graveyard — it
  carries `graveyard_object_ids`, oids without identity, which is the actual reason the event is
  still needed there.
  **The key-uniqueness finding was really a registration-scale problem.** `lookup` was a `match` on
  the key string: the last place in the card pipeline needing a central source edit per card, and
  unenumerable, which is precisely why the reverse check was unwritable. With hundreds of custom
  cards ahead, that had to go first. A new `tricerules-core/build.rs` mirrors
  `tricerules-cards/build.rs` — recursive scan of `src/custom/**/*.rs`, sorted for determinism,
  skipping `mod.rs` and `support/` — emitting `#[path]` module declarations and an `EFFECT_IMPLS`
  table that `mod.rs` `include!`s. **Absolute `#[path]` with flat module names is required**: a bare
  `mod foo;` inside an `include!`d file resolves relative to `OUT_DIR`, not the include site. **The
  file stem *is* the card id**, so the RON's `custom_effect` stays the only declaration of the
  binding and adding a card is one new file exporting `EFFECT` — no shared file edited, no key
  written in Rust. `lookup` is now a `OnceLock<HashMap>` built once, with `keys()` beside it.
  **The finding's proposed 1:1 mapping was implemented as written, after considering and rejecting
  an alias mechanism** for functional reprints: no tier-3 candidate has one (Brainstorm, Gifts
  Ungiven, Fact or Fiction, Intuition, Scroll Rack, Sylvan Library are all one-of-a-kind designs),
  reprints cluster in *simple* cards, and — decisively — the tier-3 gate admits a card only when no
  `(effect_kind, parameters)` description exists, so two cards sharing an algorithm *are* that
  description and belong in a widened primitive. An alias mechanism would have undercut the review
  rule that keeps `custom/` from becoming a scripting dump. Uniqueness is enforced at two levels
  instead: two files claiming one id fails the **build**, two RON cards claiming one key fails the
  suite naming both.
  Coverage: two unit tests (`keys()`↔`lookup` round trip, the public-zone rule) and three scenario
  tests (forward resolve + 1:1 uniqueness, reverse orphan check, the no-leak assertion). All were
  verified to fail when deliberately broken — duplicate stem, typo'd filename, duplicate RON claim,
  injected library event — and a subdirectory move confirmed the recursive scan. Full `cargo test`
  (405 scenario + all unit tests), `clippy --all-targets -D warnings` and `cargo fmt --check` exit
  0. Rust-only, no `.proto` edit, no new dependency, no C++ rebuild.
  **Known scale ceiling, deliberately not addressed:** at ~1000 effects every engine edit recompiles
  all of them, since they live in `tricerules-core`. Splitting them into a `tricerules-custom` crate
  is blocked by a cycle (`ResolutionCtx` wraps core's `GameState`; core's resolution calls `lookup`)
  and needs a startup-injected registry — a real decision against the "pure function of the command
  log" determinism story, not a mechanical move. This design does not block it: the build script,
  the `EFFECT` convention and stem-as-key all move wholesale. Relatedly, tier-3 impls take no RON
  parameters, so two cards sharing an algorithm but differing in a *number* would need two
  near-identical files; that argues for a future parameter hook or a widened primitive, not for key
  aliasing.

### 2026-08-03

- **Every ruled batch re-serialized both players' hands and libraries** (`ruled_v1.proto`,
  `tricerules-core/src/engine/{events,mod,priority,opening}.rs`,
  `ruled_player_binding.{h,cpp}`, `ruled_game_driver.cpp`): `ev_zone_view_sync` shipped an absolute
  snapshot of hand, library, battlefield and graveyard on every accepted command — ~120 freshly
  cloned card-id strings per batch for two 60-card decks, at 5–15 commands per turn, nearly all of
  them identical to the batch before. `RuledPerPlayerView` gained
  `private_zones_unchanged` (SERVER_ONLY, like the two fields it describes), and the new
  `ev_zone_view_sync_tracked` — the emission path for every batch after startup — omits `hand_cards`
  and `library_card_ids` for any player whose zones match the last view broadcast to them. Same
  absent-means-unchanged contract as `HandSlotMap` above. The saving is threefold: the engine's
  clone, the per-participant serialization, and — the big one — Servatrice's
  `applyRuledEngineZoneView` reconcile, an O(n²) match of every engine card id against a rebuilt
  ~60-card `Server_Card` pool (each comparison a `ruledCardIdForName` lookup) that concluded
  "identical" every time.
  Deliberate scope: **only** the two concealed zones. `battlefield_objects`,
  `graveyard_object_ids` and `first_strike_step_pending` still ride every view — the client
  dispatcher, the driver's oid-map rebuild and the e2e fake client all treat a view as a full
  battlefield replacement, and nothing about that contract changed. Hand and library are omitted
  **jointly** because the server reconciles them against one pool (deck zone + hand zone) and cannot
  apply half a snapshot. The decision is **per player**, so a draw re-sends only the player who drew.
  Change detection compares hand/library *ObjectId* sequences, not card ids: `card_id` is fixed for
  an object's lifetime, so the oid sequence determines the strings exactly while doing none of the
  string work the omission exists to avoid. The cache lives on `GameEngine`, not `GameState` — it
  never affects a rules decision, and being a pure function of the applied command sequence a replay
  from a fresh engine reproduces the same omission pattern. Two fail-closed guards: the startup path
  treats an omission on the *first* view (which seeds the physical zones) exactly like the existing
  library-count mismatch — warn, shuffle, drop the relay — and the binding warns if an omission
  arrives before any full reconcile ever landed, since the reconcile used to self-heal drift on the
  next batch and now only runs when the engine reports a real change.
  Coverage: a new `tricerules-core/tests/scenario/zone_view.rs` whose centerpiece is a mirror
  receiver implementing the contract (apply present, keep absent) asserted against the engine's own
  zones after *every* command of a multi-turn walk, plus per-player cases for land drops, draws, the
  two-view turn-roll batch and dev conjures; on the C++ side, a driver test that an omitted view
  leaves the physical hand and deck untouched while the oid map and tap state still rebuild
  (verified to fail with the guard neutralized), and the redaction test extended to assert the new
  flag is stripped from client copies. Full Rust suite, Clippy with warnings denied, `cargo fmt
  --check`, a full C++ build and all 18 C++ tests exit 0 — including `ruled_e2e_smoke_test`, which
  drives a scripted ruled game through real servatrice and sidecar processes and so exercises the
  omit/reconcile handshake against live `Server_Card` zones.

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

- **`ev_zone_view_sync` still re-sends every player's full battlefield on every `apply_command`
  return** (`tricerules-core/src/engine/events.rs`): the concealed half of this finding is fixed
  (see *Applied Fixes*, 2026-08-03) — hand and library are now omitted while unchanged. The public
  half remains: each view recomputes `battlefield_objects` from scratch per permanent
  (`characteristics()` layer computation, a per-ability `ability_activatable` check, a 16-keyword
  scan) for every player, every batch. Unlike the concealed zones this one has real client
  consumers that treat a view as a full replacement (`RuledEventDispatcher::applyZoneView`, the
  driver's oid-map rebuild, the e2e fake client), so an omission here is a genuine protocol change
  rather than a server-side-only one, and it needs its own design.
