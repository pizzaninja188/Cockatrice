//! Registry regressions share one corpus initialization and executable.

#[path = "../common/mod.rs"]
mod common;

#[path = "../issue_101_zone_abilities.rs"]
mod issue_101_zone_abilities;
#[path = "../issue_102_activation_limits.rs"]
mod issue_102_activation_limits;
#[path = "../issue_105_harmonize.rs"]
mod issue_105_harmonize;
#[path = "../issue_106_mobilize.rs"]
mod issue_106_mobilize;
#[path = "../issue_110_library_search.rs"]
mod issue_110_library_search;
#[path = "../issue_111_turn_history.rs"]
mod issue_111_turn_history;
#[path = "../issue_115_attachment_observers.rs"]
mod issue_115_attachment_observers;
#[path = "../issue_117_fight.rs"]
mod issue_117_fight;
#[path = "../issue_120_source_relative_abilities.rs"]
mod issue_120_source_relative_abilities;
#[path = "../issue_123_exile_play.rs"]
mod issue_123_exile_play;
#[path = "../issue_128_phase_triggers.rs"]
mod issue_128_phase_triggers;
#[path = "../issue_130_calibration_creatures.rs"]
mod issue_130_calibration_creatures;
#[path = "../issue_141_target_schema.rs"]
mod issue_141_target_schema;
#[path = "../issue_149_airbend.rs"]
mod issue_149_airbend;
#[path = "../issue_155_attachment_actions.rs"]
mod issue_155_attachment_actions;
#[path = "../issue_156_event_observer_cards.rs"]
mod issue_156_event_observer_cards;
#[path = "../issue_161_turn_boundaries.rs"]
mod issue_161_turn_boundaries;
#[path = "../issue_163_calibration_cards.rs"]
mod issue_163_calibration_cards;
#[path = "../issue_178_aggregate_object_payments.rs"]
mod issue_178_aggregate_object_payments;
#[path = "../issue_181_spell_mana_spent.rs"]
mod issue_181_spell_mana_spent;
#[path = "../issue_190_restricted_mana_any_ability.rs"]
mod issue_190_restricted_mana_any_ability;
#[path = "../issue_191_repartee.rs"]
mod issue_191_repartee;
#[path = "../issue_192_calibration_cards.rs"]
mod issue_192_calibration_cards;
#[path = "../issue_193_selected_counter_removal.rs"]
mod issue_193_selected_counter_removal;
#[path = "../issue_194_dynamic_scope_combat_restrictions.rs"]
mod issue_194_dynamic_scope_combat_restrictions;
#[path = "../issue_198_zone_card_filters.rs"]
mod issue_198_zone_card_filters;
#[path = "../issue_201_multi_object_library.rs"]
mod issue_201_multi_object_library;
#[path = "../issue_205_stack_spell_filters.rs"]
mod issue_205_stack_spell_filters;
#[path = "../issue_206_explore.rs"]
mod issue_206_explore;
#[path = "../issue_210_demolition_field.rs"]
mod issue_210_demolition_field;
#[path = "../issue_214_graveyard_land_play.rs"]
mod issue_214_graveyard_land_play;
#[path = "../issue_216_attachment_scoped_blocking_restrictions.rs"]
mod issue_216_attachment_scoped_blocking_restrictions;
#[path = "../issue_217_target_power_scale.rs"]
mod issue_217_target_power_scale;
#[path = "../issue_218_battlefield_exile_cost.rs"]
mod issue_218_battlefield_exile_cost;
#[path = "../issue_225_mass_tap.rs"]
mod issue_225_mass_tap;
#[path = "../issue_228_library_selection.rs"]
mod issue_228_library_selection;
#[path = "../issue_230_winternight.rs"]
mod issue_230_winternight;
#[path = "../issue_231_spell_filters.rs"]
mod issue_231_spell_filters;
#[path = "../issue_237_token_copy_source.rs"]
mod issue_237_token_copy_source;
#[path = "../issue_277_affinity_artifacts.rs"]
mod issue_277_affinity_artifacts;
#[path = "../issue_278_raid_etb_draw.rs"]
mod issue_278_raid_etb_draw;
#[path = "../issue_279_landfall_damage.rs"]
mod issue_279_landfall_damage;
#[path = "../issue_280_increment.rs"]
mod issue_280_increment;
#[path = "../issue_281_etb_recursion.rs"]
mod issue_281_etb_recursion;
#[path = "../issue_294_direct_ron.rs"]
mod issue_294_direct_ron;
#[path = "../issue_296_every_draw.rs"]
mod issue_296_every_draw;
#[path = "../issue_322_bargain.rs"]
mod issue_322_bargain;
#[path = "../issue_338_additional_costs.rs"]
mod issue_338_additional_costs;
#[path = "../issue_338_direct_ron.rs"]
mod issue_338_direct_ron;
#[path = "../issue_338_direct_ron_batch2.rs"]
mod issue_338_direct_ron_batch2;
#[path = "../issue_338_direct_ron_batch3.rs"]
mod issue_338_direct_ron_batch3;
#[path = "../issue_341_static_spell_cost_filter.rs"]
mod issue_341_static_spell_cost_filter;
#[path = "../issue_342_graveyard_static_scaling.rs"]
mod issue_342_graveyard_static_scaling;
#[path = "../issue_359_direct_ron.rs"]
mod issue_359_direct_ron;
#[path = "../issue_359_direct_ron_batch2.rs"]
mod issue_359_direct_ron_batch2;
#[path = "../issue_359_direct_ron_batch3.rs"]
mod issue_359_direct_ron_batch3;
#[path = "../issue_370_graveyard_count_cost_reduction.rs"]
mod issue_370_graveyard_count_cost_reduction;
#[path = "../issue_371_graveyard_count_tokens.rs"]
mod issue_371_graveyard_count_tokens;
#[path = "../issue_372_static_count_scaled_pt.rs"]
mod issue_372_static_count_scaled_pt;
#[path = "../issue_373_graveyard_count_resolution.rs"]
mod issue_373_graveyard_count_resolution;
#[path = "../issue_374_graveyard_condition_triggers.rs"]
mod issue_374_graveyard_condition_triggers;
#[path = "../issue_375_graveyard_return.rs"]
mod issue_375_graveyard_return;
#[path = "../issue_377_graveyard_static_conditions.rs"]
mod issue_377_graveyard_static_conditions;
#[path = "../issue_37_control_change.rs"]
mod issue_37_control_change;
#[path = "../issue_412_modal_mode_registry.rs"]
mod issue_412_modal_mode_registry;
#[path = "../issue_413_cycling_reminder_batch.rs"]
mod issue_413_cycling_reminder_batch;
#[path = "../issue_414_enters_with_minus_counters.rs"]
mod issue_414_enters_with_minus_counters;
#[path = "../issue_415_clue_equipment.rs"]
mod issue_415_clue_equipment;
#[path = "../issue_416_roads_conditional_entry.rs"]
mod issue_416_roads_conditional_entry;
#[path = "../issue_423_station_threshold_batch.rs"]
mod issue_423_station_threshold_batch;
#[path = "../issue_424_triggered_modal_etb.rs"]
mod issue_424_triggered_modal_etb;
#[path = "../issue_425_choose_one_or_both.rs"]
mod issue_425_choose_one_or_both;
#[path = "../issue_426_one_mode_modal.rs"]
mod issue_426_one_mode_modal;
#[path = "../issue_427_firebending_batch.rs"]
mod issue_427_firebending_batch;
#[path = "../issue_428_choose_two.rs"]
mod issue_428_choose_two;
#[path = "../issue_429_aura_restriction.rs"]
mod issue_429_aura_restriction;
#[path = "../issue_430_direct_ron.rs"]
mod issue_430_direct_ron;
#[path = "../issue_444_commands.rs"]
mod issue_444_commands;
#[path = "../issue_448_semantic_mapping.rs"]
mod issue_448_semantic_mapping;
#[path = "../issue_452_direct_ron.rs"]
mod issue_452_direct_ron;
#[path = "../issue_452_fixed_damage_family.rs"]
mod issue_452_fixed_damage_family;
#[path = "../issue_453_newly_eligible_standard.rs"]
mod issue_453_newly_eligible_standard;
#[path = "../issue_454_activated_surveil.rs"]
mod issue_454_activated_surveil;
#[path = "../issue_455_etb_opponent_pump.rs"]
mod issue_455_etb_opponent_pump;
#[path = "../issue_456_conditional_dual_lands.rs"]
mod issue_456_conditional_dual_lands;
#[path = "../issue_457_power_up_exhaust.rs"]
mod issue_457_power_up_exhaust;
#[path = "../issue_458_pump_and_seven_lands.rs"]
mod issue_458_pump_and_seven_lands;
#[path = "../issue_459_devotee_endure_mobilize.rs"]
mod issue_459_devotee_endure_mobilize;
#[path = "../issue_460_reviewed_instances.rs"]
mod issue_460_reviewed_instances;
#[path = "../issue_465_token_blockers.rs"]
mod issue_465_token_blockers;
#[path = "../issue_476_current_flying_anthem.rs"]
mod issue_476_current_flying_anthem;
#[path = "../issue_476_flying_anthems.rs"]
mod issue_476_flying_anthems;
#[path = "../issue_478_targeted_spell_controller.rs"]
mod issue_478_targeted_spell_controller;
#[path = "../issue_479_land_comparison.rs"]
mod issue_479_land_comparison;
#[path = "../issue_480_tainted_treats.rs"]
mod issue_480_tainted_treats;
#[path = "../issue_481_conditional_library_choices.rs"]
mod issue_481_conditional_library_choices;
#[path = "../issue_483_feed_the_swarm.rs"]
mod issue_483_feed_the_swarm;
#[path = "../issue_486_fancy_footwork.rs"]
mod issue_486_fancy_footwork;
#[path = "../issue_487_red_guardian.rs"]
mod issue_487_red_guardian;
#[path = "../issue_490_beza.rs"]
mod issue_490_beza;
#[path = "../issue_494_damage_mana.rs"]
mod issue_494_damage_mana;
#[path = "../issue_49_triggered_creatures.rs"]
mod issue_49_triggered_creatures;
#[path = "../issue_503_zenith_chronicler.rs"]
mod issue_503_zenith_chronicler;
#[path = "../issue_508_scoped_activated_grants.rs"]
mod issue_508_scoped_activated_grants;
#[path = "../issue_509_retained_exile_cohort.rs"]
mod issue_509_retained_exile_cohort;
#[path = "../issue_510_war_room.rs"]
mod issue_510_war_room;
#[path = "../issue_511_maze_of_ith.rs"]
mod issue_511_maze_of_ith;
#[path = "../issue_512_standstill.rs"]
mod issue_512_standstill;
#[path = "../issue_52_composite_activated_costs.rs"]
mod issue_52_composite_activated_costs;
#[path = "../issue_53_additional_spell_costs.rs"]
mod issue_53_additional_spell_costs;
#[path = "../issue_54_activation_restrictions.rs"]
mod issue_54_activation_restrictions;
#[path = "../issue_65_granted_activated_abilities.rs"]
mod issue_65_granted_activated_abilities;
#[path = "../issue_67_board_state_trigger_conditions.rs"]
mod issue_67_board_state_trigger_conditions;
#[path = "../issue_69_opponent_targets.rs"]
mod issue_69_opponent_targets;
#[path = "../issue_70_source_excluding_targets.rs"]
mod issue_70_source_excluding_targets;
#[path = "../issue_71_power_keyword_filters.rs"]
mod issue_71_power_keyword_filters;
#[path = "../issue_75_opponent_mass_scopes.rs"]
mod issue_75_opponent_mass_scopes;
#[path = "../issue_79_name_counter_scopes.rs"]
mod issue_79_name_counter_scopes;
#[path = "../issue_80_protection.rs"]
mod issue_80_protection;
#[path = "../issue_84_dynamic_amounts.rs"]
mod issue_84_dynamic_amounts;
#[path = "../issue_85_creature_damage.rs"]
mod issue_85_creature_damage;
#[path = "../issue_87_untargeted_life_loss.rs"]
mod issue_87_untargeted_life_loss;
#[path = "../issue_89_battlefield_to_library.rs"]
mod issue_89_battlefield_to_library;
#[path = "../issue_94_player_auras.rs"]
mod issue_94_player_auras;
#[path = "../issue_removal_batch.rs"]
mod issue_removal_batch;
