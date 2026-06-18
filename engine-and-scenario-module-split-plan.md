# Split `engine.rs` and `scenario.rs` into module directories

## Context

`tricerules-core/src/engine.rs` (6,531 lines) and `tricerules-core/tests/scenario.rs`
(10,900 lines) are the two largest files in the crate and grow with every new card/mechanic.
Both are monoliths that hurt navigation and make AI-agent edits slower and riskier (large
read windows, easy to lose track of where a method lives). This change is a **pure
mechanical refactor** — split each into a directory of themed files with **zero behavior
change**. Success = identical public API, all existing tests pass, `clippy -D warnings`
and `fmt --check` clean.

Key Rust facts this plan relies on:
- A type's `impl` block can be **split across many files** in the same module tree. We turn
  the single `impl GameEngine` into one `impl GameEngine { ... }` per submodule file.
- **Child modules can access ancestor-private items.** `GameEngine` (with private `registry`
  field) and the `EngineError`/`GameEvent` types stay declared in `engine/mod.rs`; every
  `engine/*.rs` submodule can use them and call private methods/fields without new `pub`.
- Free helper functions shared between submodules become `pub(super)` (visible throughout the
  `engine` module tree; siblings reach them via `super::<submod>::fn`). Helpers used by only
  one submodule move into that submodule and stay private.
- Integration tests: only top-level `tests/*.rs` become separate binaries. A subdirectory
  `tests/scenario/` is **not** auto-compiled, so we keep a single `scenario` binary whose
  root (`tests/scenario.rs`) just declares `mod` children — confirmed choice.

---

## Part 1 — `src/engine.rs` → `src/engine/`

Create `src/engine/mod.rs` plus the submodules below. No change to `lib.rs` (`pub mod engine;`
still resolves to the directory). Re-exports in `lib.rs` (`EngineError`, `GameEngine`) keep
working because both types remain defined in `engine` (in `mod.rs`).

### `engine/mod.rs` (the hub)
- Module doc comment + the full `use` block (imports stay here; submodules add `use super::*;`).
- `const MAX_HAND_SIZE`.
- `enum EngineError`, `enum GameEvent`, `struct GameEngine { pub state, registry }`.
- The core entrypoint `impl GameEngine` methods that are the natural front door:
  `new`, `new_with_default_decks`, `apply_command`, `dispatch_command`, `player_command_ipc`,
  `clear_all_mana_pools`.
- `mod` declarations for all submodules.
- Small cross-cutting free fns that don't belong to one theme can live here as `pub(super)`
  (or move with their closest consumer — see below).

### Submodules (each = one `impl GameEngine { ... }` block + its theme's free fns)

| File | `impl GameEngine` methods | Free fns / types moved here |
|------|---------------------------|------------------------------|
| `engine/opening.rs` | `apply_opening_command`, `opening_set_next_actor_after_mulligan`, `opening_pick_next_or_finish` | `mulligan_redraw`, `shuffle_player_library` (stays `pub(crate)`) |
| `engine/priority.rs` | `pass_priority`, `pass_priority_on_stack`, `adv_on_empty_stack`, `primitive_yield_structured`, `start_cleanup_or_roll_turn`, `finish_cleanup_roll_new_turn`, `next_cleanup_discard_needed`, `discard_to_hand_size`, `sweep_life`, `concede_batch` | `sorcery_speed_available`, `instant_timing_step_allowed`, `ev_phase_labeled`, `ev_priority_changed` |
| `engine/combat.rs` | `can_block`, `active_player_has_eligible_attackers`, `defending_player_has_eligible_blockers`, `set_attackers`, `set_blockers`, `assign_combat_damage`, `resolve_combat_damage_step`, `resolve_combat_damage` | `DamagePass`, `object_participates_in_pass`, `combat_needs_first_strike_step`, `is_attacking_or_blocking`, `priority_locked_for_combat_declaration` |
| `engine/casting.rs` | `cast_spell`, `activate_ability`, `pay_ability_cost`, `check_tappable`, `tap_for_cost`, `resolve_mana_ability`, `play_land` | `pay_mana`, `solve_flex`, `color_index`, `FlexPip`, `POOL_C`, `mana_amount_symbols`, `castable_at_instant_speed`; **inline `mod mana_payment_tests`** moves here |
| `engine/resolution.rs` | `resolve_top_of_stack`, `create_tokens` | `move_object_to_zone`, `destroy_permanent`, `sacrifice_permanent`, `draw_card`, `resolve_anthem_scope`, `counter_label`, `permanent_moved_event` (stays `pub(crate)`) |
| `engine/triggers.rs` | `fire_triggers`, `collect_triggers`, `matching_triggered_abilities`, `push_trigger`, `choose_trigger_target` | — |
| `engine/custom_resolution.rs` | `begin_custom_resolution`, `submit_resolution_choice`, `park_or_finish` | — |
| `engine/continuous.rs` | `effect_affects`, `emit_static_abilities_on_enter`, `effective_power`, `effective_toughness`, `cleanup_until_end_of_turn_creature_pt`, `cleanup_marked_damage`, `apply_sbas`, `apply_sbas_once`, `apply_legend_sbas` | **inline `mod sba_tests`** moves here |
| `engine/targeting.rs` | — | `compute_spell_targets`, `damage_spell_target_legal`, `destroy_spell_target_legal`, `player_target_legal`, `any_battlefield_permanent_target_legal`, `object_targetable_by`, `object_matches_mass_filter`, `battlefield_objects_matching`, `target_filter_legal`, `stack_spell_target_legal`, `spell_has_no_legal_targets_at_resolution`, `effect_target_legal_at_resolution`, `spell_effect_kind_needs_target`, `validate_effect_targets`, `validate_spell_targets`, `spell_target_legality_error` |
| `engine/events.rs` | `initial_response_batch`, `game_over_batch_winner`, `ev_card_catalog`, `ev_mana_pool_updated`, `ev_zone_view_sync` | `finish_with_events`, `ev_log`, `color_string`, `object_display_name`, `describe_target_for_log`, `format_spell_targets_log`, `mana_amount_symbols` (if not in casting), `default_deck_list` |
| `engine/legal_actions.rs` | — | `fill_legal`, `legal_labels`, `opening_legal_labels` |

Result: `mod.rs` ~500 lines; submodules ~300–700 each.

### Mechanics / conventions
- Each submodule starts with `use super::*;` (pulls in the shared imports + types) and adds
  any extra `use` it specifically needs.
- Change every currently-private free helper that is called from a **different** submodule
  to `pub(super)`. Helpers used only within their own submodule stay private. Almost all of
  these helpers are currently file-private and used only within `engine`, so `pub(super)` is
  the correct, minimal visibility — do **not** widen them to `pub(crate)`/`pub`. **Two
  exceptions:** `shuffle_player_library` and `permanent_moved_event` are already `pub(crate)`
  and are called from **outside** the `engine` module; they keep `pub(crate)` (do **not**
  narrow them to `pub(super)`, or the wider crate won't build).
- The two inline test modules keep `#[cfg(test)] mod ...; use super::*;` and move to the
  submodule that owns the code they test (mana payment → `casting.rs`, SBA → `continuous.rs`).
- A helper referenced by two themes (e.g. `mana_amount_symbols`, `resolve_anthem_scope`)
  lives in one submodule and is `pub(super)`; pick the primary owner per the table.

---

## Part 2 — `tests/scenario.rs` → `tests/scenario/` (single binary)

`tests/scenario.rs` becomes a thin root that only declares modules; all helpers and tests
move into `tests/scenario/`. One `scenario` test binary is preserved (chosen approach).

### `tests/scenario.rs` (new root)
```rust
mod helpers;
mod priority_and_turns;
mod mana;
mod casting_and_lands;
mod combat;
mod combat_keywords;
mod targeting;
mod counters_and_pump;
mod stack_and_counterspells;
mod triggers;
mod tokens;
mod spell_effects;
mod opening;
mod multi_face;
mod custom_resolution;
```

### `tests/scenario/helpers.rs`
All ~300 lines of shared helpers from the current top of the file (`pass`, `primitive_yield`,
`concede`, `discard_cleanup*`, `resolve_cleanup_discards_if_any`, `play_land`, `cast_spell*`,
`give_mana`/`ManaGift`, `activate_ability`, `deploy_to_battlefield`, `target_player`,
`declare_attackers`, `declare_blockers`, `hand_index_for_card`, `count_card_id_in_graveyard`,
`take_card_from_library_to_hand`, `battlefield_object_for_card`, `end_active_turn`,
`priority_changes_in`, `pass_both_players`, `resolve_entire_stack_two_player`,
`advance_to_main1_from_game_start`). Mark each `pub(crate)` (or `pub`). Each test submodule
starts with `use crate::helpers::*;`.

### Test submodules — group the 196 tests by dominant theme
Representative mapping (full split done by test-name theme; ~10–18 tests/file):
- `priority_and_turns.rs` — priority/phase/turn-roll/cleanup tests (`*_priority*`, `*empty_stack*`, `new_turn_stops_*`, `main2_double_pass_*`, `untap_and_draw_*`, `mana_pools_empty_on_step_change`, `second_sorcery_rejected_*`, `cleanup_*`).
- `mana.rs` — mana abilities + payment (`mana_ability_*`, `*_mana_ability_*`, `dual_land_*`, `cast_1u_creature_*`, `cast_grizzly_bears_*`, `non_active_player_with_priority_pays_*`, `hybrid_*`, `mono_hybrid_*`, `phyrexian_*`, `cannot_add_mana_while_*`).
- `casting_and_lands.rs` — basics + X spells (`play_land_*`, `cast_lightning_bolt_resolves_*`, `new_with_custom_deck_length`, `can_cast_new_vanilla_creature_*`, `blaze_*`, `x_value_on_non_x_spell_rejected`, `casting_spell_keeps_priority_*`, `caster_can_cast_second_spell_*`, `nonactive_player_cannot_play_land_*`).
- `combat.rs` — declare/resolve combat & damage assignment (`declare_attackers_*`, `declare_blockers_*`, `assign_combat_damage_*`, `begin_combat_*`, `*combat_damage*`, `duplicate_attacker_*`, `same_blocker_*`, `full_combat_*`, `*_blockers_damage_order_*`, `summoning_sick_creature_can_block`).
- `combat_keywords.rs` — evasion/combat keywords (`flying_*`, `intimidate_*`, `vigilance_*`, `lifelink_*`, `haste_*`, `deathtouch_*`, `menace_*`, `trample_*`, `first_strike_*`, `double_strike_*`, `vanilla_combat_skips_first_strike_step`, `indestructible_*`, `defender_*`, `lone_defender_*`, `flash_*`).
- `targeting.rs` — legality/fizzle/hexproof/shroud (`*_rejects_*_target`, `*_fizzles_*`, `hexproof_*`, `shroud_*`, `royal_assassin_*`, `doom_blade_targets_*`, `divine_verdict_targets_*`, `essence_scatter_and_negate_*`).
- `counters_and_pump.rs` — counters + pump/duration (`*_counter*`, `marked_damage_clears_*`, `giant_growth_*`, `two_giant_growths_*`, `fiery_hellhound_*`, `glorious_charge_*`, plus anthems: `glorious_anthem_*`, `anthem_*`, `crusade_*`, `captain_of_the_watch_*`).
- `stack_and_counterspells.rs` — LIFO stack + counters/copies (`three_bolts_*`, `five_lightning_bolts_*`, `non_active_holds_priority_*`, `counterspell_*`, `twincast_*`, `countering_a_spell_copy_*`).
- `triggers.rs` — triggered abilities (`*_trigger*`, `argothian_enchantress_*`, `soul_warden_*`, `simultaneous_combat_damage_triggers_*`).
- `tokens.rs` — token creation/identity/ceasing (`raise_the_alarm_*`, `call_the_cavalry_*`, `bestial_menace_*`, `token_dies_*`, `bounced_token_*`, `anthem_buffs_token_*`).
- `spell_effects.rs` — life/drain/draw/mill/bounce/destroy/exile/board-wipe (`cast_divination_*`, `healing_salve_*`, `angels_mercy_*`, `bump_in_the_night_*`, `blood_tithe_*`, `eyeblights_ending_*`, `swords_to_plowshares_*`, `unsummon_*`, `boomerang_*`, `tome_scour_*`, `mind_sculpt_*`, `go_for_the_throat_*`, `wrath_of_god_*`, `pyroclasm_*`, `draw_spell_decking_out_*`).
- `opening.rs` — mulligan/opening (`opening_*`, `concede_is_legal_during_opening_sequence`).
- `multi_face.rs` — split/MDFC (`fire_ice_*`).
- `custom_resolution.rs` — tier-3 custom effects (`brainstorm_*`, `gifts_ungiven_*`, `every_custom_effect_key_has_an_impl`, `recast_bounced_creature_is_summoning_sick`).

`conformance.rs` and `smoke.rs` are untouched.

---

## Execution order (build green at every step)

1. **Engine first.** Create `engine/mod.rs` by moving the hub items; then extract one
   submodule at a time, running `cargo build -p tricerules-core` after each extraction to
   catch visibility errors early. Delete `engine.rs` once `engine/mod.rs` exists.
2. **Tests second.** Create `tests/scenario/helpers.rs`, rewrite `tests/scenario.rs` to the
   `mod` list, then move test groups one file at a time, running `cargo test` between moves.
3. Use `git mv`/manual moves so history is traceable; this is mechanical — **no logic edits**.
   If clippy/fmt flags anything (e.g. an import now unused in a submodule), fix it in place.

## Verification (from `tricerules/`)

```bash
cd tricerules
cargo test                  # all scenario (196) + conformance + smoke + inline unit tests pass
cargo clippy -- -D warnings # no new warnings
cargo fmt --check           # formatting clean
```
Behavior is unchanged, so the existing test suite passing **is** the correctness proof. Spot-
check that `cargo test` still reports a single `scenario` binary and that the inline
`mana_payment_tests` / `sba_tests` still run (now under their new submodules).

## MTG applicability
No MTG rules surface area — this is a pure code-organization refactor. No engine behavior,
proto, relay, or UI changes; CR/Oracle do not govern file layout.
