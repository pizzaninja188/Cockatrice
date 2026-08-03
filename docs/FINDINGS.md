# Codebase Audit Findings

> **Status (2026-08-02):** every pending finding below was re-verified against the current tree on
> 2026-08-02. Findings that had since been fixed were deleted (see *Resolved since the audit*);
> line references and rationale were refreshed for the ones that survived.

## Applied Fixes

### 2026-08-02

- **Game-over commands now emit a complete terminal batch** (`tricerules-core/src/engine/`): Fixed
  the dead `EngineError::GameOver` path by appending the winner log to the successful command batch,
  preserving resolution/state events and clearing all legal actions once a winner is set.
  Concession, empty-library loss, and lethal life loss scenarios now verify the terminal response;
  focused scenarios, full `cargo test`, Clippy with warnings denied, and `cargo fmt --check` all exit 0.

- **Trample attacker with one blocker omitted its damage-assignment label** (`tricerules-core/src/engine/combat.rs`, `engine/legal_actions.rs`): Fixed by centralizing the explicit-assignment predicate used for multiply-blocked attackers and single-blocked attackers with trample, then using it for combat state, assignment completion, and `LegalActions.labels`. The Colossal Dreadmaw versus Grizzly Bears scenario now verifies the active player's named assignment prompt and the defender's waiting prompt. Focused scenario test, full `cargo test`, Clippy with warnings denied, and `cargo fmt --check` all exit 0.

### 2026-06-25

- **`ruledEngineConnectionLost` never reset between back-to-back ruled games** (`libcockatrice_network/.../server_game.cpp:519-524`): Fixed by adding `ruledEngineConnectionLost = false` to the per-game-start reset block in `doStartGameIfReady`. Without this reset, if game 1 lost the sidecar connection, `handleRuledEngineConnectionLost()` would return early for game 2 (guard on line 2138), sending no notification and never dropping `rulesRelay`. Every subsequent ruled command in game 2 would then time out against the dead relay instead of failing fast. Build: 14/14 C++ tests pass. Committed `03f4225a` and pushed.

## Resolved since the audit (2026-08-02 re-verification)

- **`ExileTargetGainLifeEqualToPower` used `owner` instead of `controller`** — fixed. `zones::exile_target_gain_life_equal_to_power` (`tricerules-core/src/engine/resolution/zones.rs:78-105`) now reads `o.controller` for the life gain and uses `o.owner` only to route the exile move.
- **Resolution fizzled if ANY targeted effect had no legal target** — fixed. `resolve_stack_top` (`tricerules-core/src/engine/resolution/mod.rs:213-224`) now fizzles only when `targeted_effects.iter().all(...)` have no legal target, per CR 608.2b.
- **No `TriggerCondition` for "beginning of each player's draw step"** — fixed. `AtBeginningOfDrawStep { player: CastTriggerPlayer }` exists, the `GameEvent::DrawStepBegin` arm scans all battlefields in APNAP order, and Howling Mine / Kami of the Crescent Moon are implemented.

- **`UpkeepBegin` scan covered only the active player's battlefield** *and* **no `TriggerCondition` for "each player's / each opponent's upkeep"** — both fixed together, as this file recommended. `AtBeginningOfControllerUpkeep` became `AtBeginningOfUpkeep { player: CastTriggerPlayer }` (default `Controller`), `GameEvent::UpkeepBegin` gained a `player` payload so `trigger_player_for` routes "that player", and the APNAP preamble four arms had copy-pasted became `GameEngine::battlefield_sources_apnap` — one correct implementation, which is the real fix; the `UpkeepBegin` arm rolled its own and that is how it drifted. Shipped with Sulfuric Vortex (`AnyPlayer`; partial — the CR 614 lifegain-prevention clause) and Phyrexian Arena (`Controller`), plus nine scenario tests in `tricerules-core/tests/scenario/triggers.rs`.

  Writing those tests surfaced a **deeper bug the audit missed**: `finish_cleanup_roll_new_turn` (`engine/priority.rs`) walks Untap → Upkeep *inline* when a turn rolls and never fired `UpkeepBegin` at all. The `adv_on_empty_stack` `Untap` arm that does fire it is unreachable on a normal roll, because CR 502.1 gives nobody priority in the untap step. Upkeep triggers therefore never fired in normal play — which is exactly why the scan bug was invisible, and why "no shipped card uses the variant" was doing more work than it looked. The event now fires at that transition, before `ev_priority_changed` per CR 503.1a.

## Pending Findings

### Bugs

- **Neither spell-damage path sets `deathtouch_damage`** (`tricerules-core/src/engine/resolution/mass.rs:103-137` `damage_all`, `resolution/damage.rs:3-59` `damage_target`, `damage.rs:62+` `damage_targets`): only combat damage sets the flag (`combat.rs:871,890,955,976`), which the lethal-damage SBA reads (`state_based.rs:115`). No implemented card exercises this — no spell or ability source currently has Deathtouch — but any Deathtouch source dealing non-combat damage (a Deathtouch creature's activated ping ability, e.g.) would miss the CR 702.2b / 704.5h death check. *(Corrected from the June note, which claimed `DamageTarget` set the flag and only `DamageAll` missed it — neither does.)*

### Reusability / Scalability

- **`compute_spell_targets` iterates `state.objects.values()` (HashMap — non-deterministic order)** (`tricerules-core/src/engine/targeting.rs:761-785`; `objects` is a `HashMap` per `state.rs:430`): `valid_permanent_ids`, `valid_stack_ids` and `valid_graveyard_ids` in `SpellTargets` may therefore vary in order across executions. Fix: collect from `state.players[*].battlefield` / `.graveyard` in APNAP player order, matching the deterministic pattern the same file's player loop (lines 787-803) and the trigger scans already use.

- **Hard-coded 2-player assumption is pervasive** (`tricerules-core/src/state.rs:325,515-523`, `engine/mod.rs:210-212`): `defending_player_id_1v1()` returns `None` for anything but exactly 2 players and has ~10 call sites across `combat.rs`/`priority.rs`/`legal_actions.rs`; `OpeningSequence.mulligans_taken` is `[u32; 2]`; `GameEngine::new` rejects any player count != 2. Partially mitigated since the audit — `defending_player_id_1v1` carries a doc comment and `new` returns a clear `Illegal("M2: exactly 2 players")` rather than panicking — but the `[u32; 2]` array and the unaudited call sites remain the work item for any multiplayer expansion.

- **`ev_zone_view_sync` serializes the entire game state on every single `apply_command` return** (`tricerules-core/src/engine/mod.rs:620`): every priority pass, mana tap, and phase transition serializes all players' libraries (as comma-joined card-id strings), hands, and battlefields. For two 60-card decks, a typical turn produces 5–15 commands, each emitting ~200 strings into the zone-view. Correct-by-construction (authoritative snapshot) but O(deck size × commands per turn). Consider a delta-only zone view (emit only changed zones, or `ZoneDelta` events) for high-volume paths.

- **`tricerules-server` has no per-connection timeout or idle guard** (`tricerules-server/src/main.rs:77-83`, `handle_connection:86-90`): each accepted connection spawns a `tokio::task` that loops indefinitely on `read_proto`. If Servatrice crashes mid-game without sending `SessionEnd`, the task leaks and holds an open `GameEngine` session in memory. Fix: wrap `read_proto` in `tokio::time::timeout` (e.g. 30 s) and exit the task on idle-timeout, or add a SIGTERM handler that drains all sessions on shutdown.

- **`tricerules-server` does not set `TCP_NODELAY` on the accepted socket** (`tricerules-server/src/main.rs:78`, `write_proto:223-232`): `write_proto` issues two separate `write_all` calls — a 4-byte length prefix and the payload. With Nagle enabled (the tokio `TcpStream` default) the length segment may be held briefly until the next write. Sub-millisecond on loopback, but it compounds per round-trip on any deployment where the sidecar and Servatrice are on different hosts. Fix: `sock.set_nodelay(true)?` after `listener.accept().await?`, or merge length+payload into one `write_all`.

- **`ResolutionCtx::put_on_top_of_library` emits no intermediate event for Hand → Library moves** (`tricerules-core/src/custom/mod.rs:167-191`): the zone move is correct and Servatrice learns the final state via `zone_view`, but the pattern is asymmetric — `move_to_zone` emits `PermanentMoved` for graveyard/exile and nothing for hand/library. If replay animation or an intermediate reveal is ever added, this gap becomes a bug.

- **`discard_to_hand_size` proto conflates absent vs. zero for `hand_card_index`** (`ruled_v1.proto:193-198`, `priority.rs:472-477`): proto3 defaults `hand_card_index` to `0`, making "not set" indistinguishable from "card 0". The Rust code is correct today because it checks `hand_card_indices.is_empty()` first, but the interface invites a future caller to get it wrong. Suggest `oneof discard_selector { uint32 single_index; repeated uint32 batch_indices; }`.

- **`custom_effect` key uniqueness is never validated** (`tricerules-core/src/custom/mod.rs:288-294`, test at `tricerules-core/tests/scenario/custom_resolution.rs:331`): the existing test asserts every registry `custom_effect` key *resolves* to an impl, but not that each key is claimed by exactly one card id. Two RON cards accidentally sharing `custom_effect: "brainstorm"` would both resolve as Brainstorm with no error. Fix: extend `every_custom_effect_key_has_an_impl` to also assert a 1:1 key↔card-id mapping.

- **`appendServerObjectMaps` unconditionally appends a `HandSlotMap` event to every batch, including batches with no hand change** (`libcockatrice_network/libcockatrice/network/server/remote/game/ruled_game_driver.cpp:1375-1398`): unlike `BattlefieldObjectMap` (guarded by `map->entries_size() > 0` at line 1369) and `GraveyardObjectMap` (line 1418), the `HandSlotMap` event is always injected. For a 60-card game each hand contributes up to 7 entries, serialized on every single ruled command (priority passes, mana taps, …) and again per participant during redaction (`:1520-1531`). Fix: guard with `if (hm->entries_size() > 0)` before the append, matching the two sibling blocks. *(Moved from `server_game.cpp` since the audit — the June path reference is stale.)*
