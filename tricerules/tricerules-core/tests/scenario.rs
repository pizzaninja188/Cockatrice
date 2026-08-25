//! Scripted command sequences (M2), split into themed submodules.
//!
//! `tests/scenario.rs` is the crate root of the `scenario` integration-test binary, so a bare
//! `mod foo;` would resolve to `tests/foo.rs` (which Cargo would then compile as its own test
//! binary). The `#[path]` attributes keep every submodule under `tests/scenario/` while
//! preserving a single `scenario` test binary.

#[path = "scenario/activation_restrictions.rs"]
mod activation_restrictions;
#[path = "scenario/additional_spell_costs.rs"]
mod additional_spell_costs;
#[path = "scenario/attacking_scopes.rs"]
mod attacking_scopes;
#[path = "scenario/auras.rs"]
mod auras;
#[path = "scenario/block_event_triggers.rs"]
mod block_event_triggers;
#[path = "scenario/blocking_restrictions.rs"]
mod blocking_restrictions;
#[path = "scenario/board_state_trigger_conditions.rs"]
mod board_state_trigger_conditions;
#[path = "scenario/casting_and_lands.rs"]
mod casting_and_lands;
#[path = "scenario/combat.rs"]
mod combat;
#[path = "scenario/combat_keywords.rs"]
mod combat_keywords;
#[path = "scenario/composite_activated_costs.rs"]
mod composite_activated_costs;
#[path = "scenario/conditional_characteristics.rs"]
mod conditional_characteristics;
#[path = "scenario/conditional_spell_costs.rs"]
mod conditional_spell_costs;
#[path = "scenario/control.rs"]
mod control;
#[path = "scenario/copy_effects.rs"]
mod copy_effects;
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
#[path = "scenario/end_step_triggers.rs"]
mod end_step_triggers;
#[path = "scenario/enters_tapped.rs"]
mod enters_tapped;
#[path = "scenario/enters_with_counters.rs"]
mod enters_with_counters;
#[path = "scenario/equipment.rs"]
mod equipment;
#[path = "scenario/helpers.rs"]
mod helpers;
#[path = "scenario/issue_100_omen.rs"]
mod issue_100_omen;
#[path = "scenario/issue_101_zone_abilities.rs"]
mod issue_101_zone_abilities;
#[path = "scenario/issue_102_activation_limits.rs"]
mod issue_102_activation_limits;
#[path = "scenario/issue_103_ward.rs"]
mod issue_103_ward;
#[path = "scenario/issue_104_optional_cast_costs.rs"]
mod issue_104_optional_cast_costs;
#[path = "scenario/issue_105_harmonize.rs"]
mod issue_105_harmonize;
#[path = "scenario/issue_107_graveyard_actions.rs"]
mod issue_107_graveyard_actions;
#[path = "scenario/issue_108_graveyard_aggregates.rs"]
mod issue_108_graveyard_aggregates;
#[path = "scenario/issue_109_temporary_exile.rs"]
mod issue_109_temporary_exile;
#[path = "scenario/issue_110_library_search.rs"]
mod issue_110_library_search;
#[path = "scenario/issue_111_turn_history.rs"]
mod issue_111_turn_history;
#[path = "scenario/issue_112_spell_cost_reductions.rs"]
mod issue_112_spell_cost_reductions;
#[path = "scenario/issue_114_disjunctive_filters.rs"]
mod issue_114_disjunctive_filters;
#[path = "scenario/issue_115_attachment_observers.rs"]
mod issue_115_attachment_observers;
#[path = "scenario/issue_116_condition_branches.rs"]
mod issue_116_condition_branches;
#[path = "scenario/issue_117_fight.rs"]
mod issue_117_fight;
#[path = "scenario/issue_119_self_combat_restrictions.rs"]
mod issue_119_self_combat_restrictions;
#[path = "scenario/issue_121_player_set_zone_effects.rs"]
mod issue_121_player_set_zone_effects;
#[path = "scenario/issue_122_result_cohorts.rs"]
mod issue_122_result_cohorts;
#[path = "scenario/issue_123_exile_play.rs"]
mod issue_123_exile_play;
#[path = "scenario/issue_125_death_replacement.rs"]
mod issue_125_death_replacement;
#[path = "scenario/issue_126_opponent_hand_exile.rs"]
mod issue_126_opponent_hand_exile;
#[path = "scenario/issue_127_trigger_event_filters.rs"]
mod issue_127_trigger_event_filters;
#[path = "scenario/issue_128_phase_triggers.rs"]
mod issue_128_phase_triggers;
#[path = "scenario/issue_130_calibration_creatures.rs"]
mod issue_130_calibration_creatures;
#[path = "scenario/issue_139_pending_trigger_publication.rs"]
mod issue_139_pending_trigger_publication;
#[path = "scenario/issue_142_endure.rs"]
mod issue_142_endure;
#[path = "scenario/issue_57_targeting_costs.rs"]
mod issue_57_targeting_costs;
#[path = "scenario/issue_59_resolution_choices.rs"]
mod issue_59_resolution_choices;
#[path = "scenario/issue_63_attachment_event_triggers.rs"]
mod issue_63_attachment_event_triggers;
#[path = "scenario/issue_64_granted_delayed_triggers.rs"]
mod issue_64_granted_delayed_triggers;
#[path = "scenario/issue_65_granted_activated_abilities.rs"]
mod issue_65_granted_activated_abilities;
#[path = "scenario/issue_71_filters.rs"]
mod issue_71_filters;
#[path = "scenario/issue_73_grouped_targets.rs"]
mod issue_73_grouped_targets;
#[path = "scenario/issue_80_protection.rs"]
mod issue_80_protection;
#[path = "scenario/issue_82_attached_untap.rs"]
mod issue_82_attached_untap;
#[path = "scenario/issue_83_destroy_attached.rs"]
mod issue_83_destroy_attached;
#[path = "scenario/issue_85_creature_damage.rs"]
mod issue_85_creature_damage;
#[path = "scenario/issue_86_related_player_recipients.rs"]
mod issue_86_related_player_recipients;
#[path = "scenario/issue_89_battlefield_to_library.rs"]
mod issue_89_battlefield_to_library;
#[path = "scenario/issue_90_evolving_wilds.rs"]
mod issue_90_evolving_wilds;
#[path = "scenario/issue_92_library_choice.rs"]
mod issue_92_library_choice;
#[path = "scenario/issue_96_surveil.rs"]
mod issue_96_surveil;
#[path = "scenario/issue_97_entry_replacements.rs"]
mod issue_97_entry_replacements;
#[path = "scenario/issue_98_face_down.rs"]
mod issue_98_face_down;
#[path = "scenario/issue_99_rooms.rs"]
mod issue_99_rooms;
#[path = "scenario/legend_rule.rs"]
mod legend_rule;
#[path = "scenario/mana.rs"]
mod mana;
#[path = "scenario/mill_results.rs"]
mod mill_results;
#[path = "scenario/modal_spells.rs"]
mod modal_spells;
#[path = "scenario/multi_attacker_triggers.rs"]
mod multi_attacker_triggers;
#[path = "scenario/multi_face.rs"]
mod multi_face;
#[path = "scenario/name_counter_scopes.rs"]
mod name_counter_scopes;
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
#[path = "scenario/skip_next_untap.rs"]
mod skip_next_untap;
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
#[path = "scenario/type_adding_effects.rs"]
mod type_adding_effects;
#[path = "scenario/untap.rs"]
mod untap;
#[path = "scenario/x_multi_target.rs"]
mod x_multi_target;
#[path = "scenario/zone_view.rs"]
mod zone_view;
