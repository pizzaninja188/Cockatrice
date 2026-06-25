# Codebase Audit Findings

## Applied Fixes
_None applied yet — all findings are pending review._

## Pending Findings

### Bugs

- **`EngineError::GameOver` is dead code; game-over events are never emitted via the intended path** (`tricerules-core/src/engine/mod.rs:56,420`, `priority.rs:45,60`): Both `sweep_life` (called after every command dispatch) and `concede_batch` set `state.winner` and return `Ok(batch)` — they never return `Err(EngineError::GameOver(...))`. The `GameOver` arm in `player_command_ipc` (line 420) is dead code; `game_over_batch_winner` is never called. Clients receive no dedicated game-winner event. Fix: in `dispatch_command`, after `sweep_life()`, check `state.winner.is_some()` and return `Err(GameOver(w))`, or push a winner log event into the batch directly.

- **`DamageAll` effect never sets `deathtouch_damage` on hit permanents** (`tricerules-core/src/engine/resolution.rs:791-804`): Increments `o.damage` but never sets `o.deathtouch_damage = true`, unlike `DamageTarget` and combat damage. Currently no card exercises this (spells don't have Deathtouch), but any future activated ability on a Deathtouch creature using `DamageAll` would miss the SBA death check (CR 704.5h).

- **`UpkeepBegin` trigger scan only covers the active player's battlefield** (`tricerules-core/src/engine/triggers.rs:147-162`): `collect_triggers` for `GameEvent::UpkeepBegin` only iterates `self.state.players[ap_idx].battlefield`. During Player 2's upkeep, Player 1's permanents with `AtBeginningOfControllerUpkeep` are never checked. Fix: iterate ALL players' battlefields (APNAP order) and let the filter match only when `controller == active_player_id()`.

- **`ExileTargetGainLifeEqualToPower` uses card `owner` instead of `controller` for life gain** (`tricerules-core/src/engine/resolution.rs:656-657`): Swords to Plowshares says "its controller gains life equal to its power" — the creature's current controller. The code reads `o.owner`. Currently harmless (owner == controller), but will be wrong once control-changing effects are added. Requires storing `controller` separately from `owner` on `GameObject`.

### Reusability / Scalability

- **`legal_labels` omits the damage-assignment label for trample-with-single-blocker** (`tricerules-core/src/engine/legal_actions.rs:135-148`): The label is generated only when `blks.len() > 1`, but `damage_assignment_needed` is also set when `blks.len() == 1 && has_trample`. The trample case shows no prompt hint even though assignment is required. Fix: mirror the condition from `set_blockers`: `blks.len() > 1 || (blks.len() == 1 && has_trample)`.

- **`compute_spell_targets` iterates `state.objects.values()` (HashMap — non-deterministic order)** (`tricerules-core/src/engine/targeting.rs:600-613`): `valid_permanent_ids` and `valid_stack_ids` in `SpellTargets` may vary in order across executions. Fix: collect from `state.players[*].battlefield` in player order (APNAP), matching the deterministic pattern used elsewhere.

- **`spell_has_no_legal_targets_at_resolution` fizzles if ANY targeted effect has no target, instead of ALL** (`tricerules-core/src/engine/targeting.rs:243-252`): Uses `effects.iter().any(...)`. CR 608.2b says fizzle only when ALL targets are illegal. Wrong for multi-effect spells that mix targeted and untargeted sub-effects. Currently harmless (all implemented spells have at most one targeted effect), but will misfire once compound spells are added.

- **No `TriggerCondition` variant for "beginning of each player's upkeep" or "each opponent's upkeep"** (`tricerules-cards/src/primitives.rs:653-654`): `AtBeginningOfControllerUpkeep` is the only upkeep trigger. Cards like Howling Mine ("at the beginning of each player's draw step") and similar cannot be expressed. Needs `AtBeginningOfEachUpkeep` and `AtBeginningOfEachOpponentUpkeep` variants plus engine scan.

- **Hard-coded 2-player assumption is pervasive** (`tricerules-core/src/state.rs:419-427`, `engine/mod.rs:110`, `opening.rs` `[u32; 2]` arrays): `defending_player_id_1v1()`, 2-player-only `OpeningSequence.mulligans_taken: [u32; 2]`, and the `GameEngine::new` 2-player assertion are all point of failure for any future multiplayer expansion. Should be documented explicitly at each site.

- **`ResolutionCtx::put_on_top_of_library` emits no intermediate event for Hand → Library moves** (`tricerules-core/src/custom/mod.rs:188-208`): The zone move is correct; Servatrice learns the final state via `zone_view`. But if replay animation or intermediate reveal is ever added, this gap becomes a bug. The pattern is asymmetric — `move_to_zone` emits `PermanentMoved` for graveyard/exile but not hand/library.

- **`discard_to_hand_size` proto design conflates absent vs. zero for `hand_card_index`** (`ruled_v1.proto:80-85`, `priority.rs:473-477`): Proto3 defaults `hand_card_index` to `0`, making "not set" indistinguishable from "card 0". The Rust code correctly checks `hand_card_indices.is_empty()` first, but the proto interface is misleading. Suggest `oneof discard_selector { uint32 single_index; repeated uint32 batch_indices; }` to make the distinction explicit and safe.

- **`custom_key` uniqueness is never validated at startup** (`tricerules-core/src/custom/mod.rs:311-317`): If two different RON card definitions accidentally share the same `custom_effect` key, Brainstorm/Gifts logic could be applied to the wrong card with no error. Suggest: add a registry startup assertion that each `custom_effect` key appears on exactly one card id.
