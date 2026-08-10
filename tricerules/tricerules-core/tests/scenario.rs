//! Scripted command sequences (M2), split into themed submodules.
//!
//! `tests/scenario.rs` is the crate root of the `scenario` integration-test binary, so a bare
//! `mod foo;` would resolve to `tests/foo.rs` (which Cargo would then compile as its own test
//! binary). The `#[path]` attributes keep every submodule under `tests/scenario/` while
//! preserving a single `scenario` test binary.

#[path = "scenario/auras.rs"]
mod auras;
#[path = "scenario/casting_and_lands.rs"]
mod casting_and_lands;
#[path = "scenario/combat.rs"]
mod combat;
#[path = "scenario/combat_keywords.rs"]
mod combat_keywords;
#[path = "scenario/control.rs"]
mod control;
#[path = "scenario/counters_and_pump.rs"]
mod counters_and_pump;
#[path = "scenario/custom_resolution.rs"]
mod custom_resolution;
#[path = "scenario/damage_prevention.rs"]
mod damage_prevention;
#[path = "scenario/dev_commands.rs"]
mod dev_commands;
#[path = "scenario/dynamic_amounts.rs"]
mod dynamic_amounts;
#[path = "scenario/enters_tapped.rs"]
mod enters_tapped;
#[path = "scenario/enters_with_counters.rs"]
mod enters_with_counters;
#[path = "scenario/equipment.rs"]
mod equipment;
#[path = "scenario/helpers.rs"]
mod helpers;
#[path = "scenario/legend_rule.rs"]
mod legend_rule;
#[path = "scenario/mana.rs"]
mod mana;
#[path = "scenario/modal_spells.rs"]
mod modal_spells;
#[path = "scenario/multi_face.rs"]
mod multi_face;
#[path = "scenario/opening.rs"]
mod opening;
#[path = "scenario/opponent_life_loss.rs"]
mod opponent_life_loss;
#[path = "scenario/opponent_targets.rs"]
mod opponent_targets;
#[path = "scenario/performance.rs"]
mod performance;
#[path = "scenario/priority_and_turns.rs"]
mod priority_and_turns;
#[path = "scenario/regenerate.rs"]
mod regenerate;
#[path = "scenario/scry.rs"]
mod scry;
#[path = "scenario/source_excluding_targets.rs"]
mod source_excluding_targets;
#[path = "scenario/spell_effects.rs"]
mod spell_effects;
#[path = "scenario/stack_and_counterspells.rs"]
mod stack_and_counterspells;
#[path = "scenario/targeting.rs"]
mod targeting;
#[path = "scenario/tokens.rs"]
mod tokens;
#[path = "scenario/triggers.rs"]
mod triggers;
#[path = "scenario/turn_history.rs"]
mod turn_history;
#[path = "scenario/tutor_search.rs"]
mod tutor_search;
#[path = "scenario/untap.rs"]
mod untap;
#[path = "scenario/x_multi_target.rs"]
mod x_multi_target;
#[path = "scenario/zone_view.rs"]
mod zone_view;
