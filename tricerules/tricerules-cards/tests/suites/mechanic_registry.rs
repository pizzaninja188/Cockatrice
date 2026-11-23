//! Registry regressions share one corpus initialization and executable.

#[path = "../common/mod.rs"]
mod common;

#[path = "../affinity_flying_batch.rs"]
mod affinity_flying_batch;
#[path = "../aura_etb_draw_pump_batch.rs"]
mod aura_etb_draw_pump_batch;
#[path = "../aura_flying_pump_batch.rs"]
mod aura_flying_pump_batch;
#[path = "../authored_operation_reuse.rs"]
mod authored_operation_reuse;
#[path = "../damage_flashback_batch.rs"]
mod damage_flashback_batch;
#[path = "../flash_aura_pump_batch.rs"]
mod flash_aura_pump_batch;
#[path = "../flash_aura_shrink_batch.rs"]
mod flash_aura_shrink_batch;
#[path = "../flying_self_pump_batch.rs"]
mod flying_self_pump_batch;
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
#[path = "../issue_524_color_graveyard.rs"]
mod issue_524_color_graveyard;
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
#[path = "../lifelink_pump_batch.rs"]
mod lifelink_pump_batch;
#[path = "../mass_shrink_batch.rs"]
mod mass_shrink_batch;

#[path = "../damage_any_target_batch.rs"]
mod damage_any_target_batch;

#[path = "../pump_team_batch.rs"]
mod pump_team_batch;

#[path = "../equipment_batch.rs"]
mod equipment_batch;

#[path = "../forestwalk_batch.rs"]
mod forestwalk_batch;

#[path = "../regenerate_batch.rs"]
mod regenerate_batch;

#[path = "../shrink_batch.rs"]
mod shrink_batch;

#[path = "../pump_self_batch.rs"]
mod pump_self_batch;

#[path = "../pump_self_cohort.rs"]
mod pump_self_cohort;

#[path = "../pump_trample_batch.rs"]
mod pump_trample_batch;

#[path = "../pump_aura_batch.rs"]
mod pump_aura_batch;

#[path = "../warped_landscape_batch.rs"]
mod warped_landscape_batch;

#[path = "../arachnoid_adaptation_batch.rs"]
mod arachnoid_adaptation_batch;

#[path = "../hieromancers_cage_batch.rs"]
mod hieromancers_cage_batch;

#[path = "../captains_tutelage_batch.rs"]
mod captains_tutelage_batch;

#[path = "../viashino_racketeer_batch.rs"]
mod viashino_racketeer_batch;

#[path = "../agonizing_syphon_batch.rs"]
mod agonizing_syphon_batch;

#[path = "../glint_batch.rs"]
mod glint_batch;

#[path = "../high_rise_sawjack_batch.rs"]
mod high_rise_sawjack_batch;

#[path = "../puncturing_blow_batch.rs"]
mod puncturing_blow_batch;

#[path = "../soothsayer_adept_batch.rs"]
mod soothsayer_adept_batch;

#[path = "../darkness_batch.rs"]
mod darkness_batch;

#[path = "../izzet_chronarch_batch.rs"]
mod izzet_chronarch_batch;

#[path = "../urborg_volcano_batch.rs"]
mod urborg_volcano_batch;

#[path = "../dueling_rapier_batch.rs"]
mod dueling_rapier_batch;

#[path = "../recollect_batch.rs"]
mod recollect_batch;

#[path = "../aggressive_urge_batch.rs"]
mod aggressive_urge_batch;

#[path = "../quirion_explorer_batch.rs"]
mod quirion_explorer_batch;

#[path = "../mindstatic_batch.rs"]
mod mindstatic_batch;

#[path = "../colorless_two_mana_batch.rs"]
mod colorless_two_mana_batch;

#[path = "../timber_gorge_batch.rs"]
mod timber_gorge_batch;

#[path = "../ronom_unicorn_batch.rs"]
mod ronom_unicorn_batch;

#[path = "../brazen_freebooter_batch.rs"]
mod brazen_freebooter_batch;

#[path = "../gaeas_anthem_batch.rs"]
mod gaeas_anthem_batch;

#[path = "../sylvan_ranger_batch.rs"]
mod sylvan_ranger_batch;

#[path = "../breath_of_life_batch.rs"]
mod breath_of_life_batch;

#[path = "../tap_dual_remnants_batch.rs"]
mod tap_dual_remnants_batch;

#[path = "../any_color_mana_batch.rs"]
mod any_color_mana_batch;

#[path = "../graveyard_two_to_hand_batch.rs"]
mod graveyard_two_to_hand_batch;

#[path = "../enters_tapped_creatures_batch.rs"]
mod enters_tapped_creatures_batch;

#[path = "../prowess_creatures_batch.rs"]
mod prowess_creatures_batch;

#[path = "../flying_etb_scry_batch.rs"]
mod flying_etb_scry_batch;

#[path = "../flying_creatures_batch.rs"]
mod flying_creatures_batch;

#[path = "../colorless_double_mana_batch.rs"]
mod colorless_double_mana_batch;

#[path = "../destroy_artifact_batch.rs"]
mod destroy_artifact_batch;

#[path = "../tap_target_activated_batch.rs"]
mod tap_target_activated_batch;

#[path = "../destroy_land_batch.rs"]
mod destroy_land_batch;

#[path = "../attack_pump_batch.rs"]
mod attack_pump_batch;

#[path = "../etb_counter_target_batch.rs"]
mod etb_counter_target_batch;

#[path = "../exile_artifact_enchantment_batch.rs"]
mod exile_artifact_enchantment_batch;

#[path = "../destroy_tapped_batch.rs"]
mod destroy_tapped_batch;

#[path = "../landfall_pump_batch.rs"]
mod landfall_pump_batch;

#[path = "../attack_each_combat_batch.rs"]
mod attack_each_combat_batch;

#[path = "../counter_creature_spell_batch.rs"]
mod counter_creature_spell_batch;

#[path = "../flying_cant_block_batch.rs"]
mod flying_cant_block_batch;

#[path = "../flying_prowess_batch.rs"]
mod flying_prowess_batch;

#[path = "../etb_team_pump_batch.rs"]
mod etb_team_pump_batch;

#[path = "../dies_draw_batch.rs"]
mod dies_draw_batch;

#[path = "../etb_gain_four_life_batch.rs"]
mod etb_gain_four_life_batch;

#[path = "../aura_tap_lock_batch.rs"]
mod aura_tap_lock_batch;

#[path = "../destroy_all_enchantments_batch.rs"]
mod destroy_all_enchantments_batch;

#[path = "../draw_three_discard_batch.rs"]
mod draw_three_discard_batch;

#[path = "../pinger_batch.rs"]
mod pinger_batch;

#[path = "../exile_target_creature_batch.rs"]
mod exile_target_creature_batch;

#[path = "../etb_damage_any_target_batch.rs"]
mod etb_damage_any_target_batch;

#[path = "../combat_damage_draw_batch.rs"]
mod combat_damage_draw_batch;

#[path = "../dies_shrink_opponent_batch.rs"]
mod dies_shrink_opponent_batch;

#[path = "../aura_cant_attack_block_batch.rs"]
mod aura_cant_attack_block_batch;

#[path = "../put_creature_on_top_batch.rs"]
mod put_creature_on_top_batch;

#[path = "../etb_loot_batch.rs"]
mod etb_loot_batch;

#[path = "../flying_etb_loot_batch.rs"]
mod flying_etb_loot_batch;

#[path = "../mana_any_color_batch.rs"]
mod mana_any_color_batch;

#[path = "../filter_lands_batch.rs"]
mod filter_lands_batch;

#[path = "../pain_lands_batch.rs"]
mod pain_lands_batch;

#[path = "../cycling_lands_batch.rs"]
mod cycling_lands_batch;

#[path = "../cycling_color_lands_batch.rs"]
mod cycling_color_lands_batch;

#[path = "../vivid_lands_batch.rs"]
mod vivid_lands_batch;

#[path = "../explore_etb_batch.rs"]
mod explore_etb_batch;

#[path = "../dies_damage_any_target_batch.rs"]
mod dies_damage_any_target_batch;

#[path = "../dies_gain_life_batch.rs"]
mod dies_gain_life_batch;

#[path = "../team_pump_activated_batch.rs"]
mod team_pump_activated_batch;

#[path = "../refuge_lands_batch.rs"]
mod refuge_lands_batch;

#[path = "../talisman_cycle_batch.rs"]
mod talisman_cycle_batch;

#[path = "../islandwalk_batch.rs"]
mod islandwalk_batch;

#[path = "../mountainwalk_batch.rs"]
mod mountainwalk_batch;

#[path = "../etb_target_pump_batch.rs"]
mod etb_target_pump_batch;

#[path = "../aura_pump_values_batch.rs"]
mod aura_pump_values_batch;

#[path = "../sliver_anthem_batch.rs"]
mod sliver_anthem_batch;

#[path = "../attack_pump_values_batch.rs"]
mod attack_pump_values_batch;

#[path = "../self_pump_once_batch.rs"]
mod self_pump_once_batch;

#[path = "../swampwalk_batch.rs"]
mod swampwalk_batch;

#[path = "../protection_red_flying_batch.rs"]
mod protection_red_flying_batch;

#[path = "../signet_cycle_batch.rs"]
mod signet_cycle_batch;

#[path = "../filter_land_cycle_batch.rs"]
mod filter_land_cycle_batch;

#[path = "../damage_target_creature_batch.rs"]
mod damage_target_creature_batch;

#[path = "../pump_first_strike_batch.rs"]
mod pump_first_strike_batch;

#[path = "../tap_target_two_batch.rs"]
mod tap_target_two_batch;

#[path = "../landfall_pump_remnant_batch.rs"]
mod landfall_pump_remnant_batch;

#[path = "../battle_land_cycle_batch.rs"]
mod battle_land_cycle_batch;

#[path = "../tapped_dual_basic_batch.rs"]
mod tapped_dual_basic_batch;

#[path = "../snow_tapped_dual_basic_batch.rs"]
mod snow_tapped_dual_basic_batch;

#[path = "../all_creature_shrink_batch.rs"]
mod all_creature_shrink_batch;

#[path = "../targeted_evasion_batch.rs"]
mod targeted_evasion_batch;

#[path = "../loot_activated_batch.rs"]
mod loot_activated_batch;

#[path = "../put_land_from_hand_batch.rs"]
mod put_land_from_hand_batch;

#[path = "../pay_life_any_color_batch.rs"]
mod pay_life_any_color_batch;

#[path = "../any_color_mana_activation_batch.rs"]
mod any_color_mana_activation_batch;

#[path = "../self_pump_negative_batch.rs"]
mod self_pump_negative_batch;

#[path = "../cant_block_batch.rs"]
mod cant_block_batch;

#[path = "../cant_be_blocked_batch.rs"]
mod cant_be_blocked_batch;

#[path = "../protection_cycle_batch.rs"]
mod protection_cycle_batch;

#[path = "../flying_blocks_only_flying_batch.rs"]
mod flying_blocks_only_flying_batch;

#[path = "../cycling_two_batch.rs"]
mod cycling_two_batch;

#[path = "../flying_cycling_two_batch.rs"]
mod flying_cycling_two_batch;

#[path = "../etb_scry_two_batch.rs"]
mod etb_scry_two_batch;

#[path = "../flying_dies_draw_batch.rs"]
mod flying_dies_draw_batch;

#[path = "../etb_destroy_artifact_batch.rs"]
mod etb_destroy_artifact_batch;

#[path = "../tap_gain_life_batch.rs"]
mod tap_gain_life_batch;

#[path = "../tap_untap_land_batch.rs"]
mod tap_untap_land_batch;

#[path = "../destroy_attacking_batch.rs"]
mod destroy_attacking_batch;

#[path = "../etb_destroy_land_batch.rs"]
mod etb_destroy_land_batch;

#[path = "../wither_batch.rs"]
mod wither_batch;

#[path = "../flying_protection_batch.rs"]
mod flying_protection_batch;

#[path = "../vanilla_colorless_batch.rs"]
mod vanilla_colorless_batch;

#[path = "../vanilla_green_batch.rs"]
mod vanilla_green_batch;

#[path = "../vanilla_gold_batch.rs"]
mod vanilla_gold_batch;

#[path = "../attack_haste_batch.rs"]
mod attack_haste_batch;

#[path = "../bounce_draw_batch.rs"]
mod bounce_draw_batch;

#[path = "../tap_ping_self_batch.rs"]
mod tap_ping_self_batch;

#[path = "../regenerate_green_batch.rs"]
mod regenerate_green_batch;

#[path = "../red_pump_one_batch.rs"]
mod red_pump_one_batch;

#[path = "../red_pump_two_batch.rs"]
mod red_pump_two_batch;

#[path = "../vanilla_remnants_batch.rs"]
mod vanilla_remnants_batch;

#[path = "../blue_gain_flying_batch.rs"]
mod blue_gain_flying_batch;

#[path = "../white_gain_vigilance_batch.rs"]
mod white_gain_vigilance_batch;

#[path = "../vanilla_red_batch.rs"]
mod vanilla_red_batch;

#[path = "../first_strike_batch.rs"]
mod first_strike_batch;

#[path = "../trample_batch.rs"]
mod trample_batch;

#[path = "../self_pump_black_batch.rs"]
mod self_pump_black_batch;

#[path = "../self_pump_red_batch.rs"]
mod self_pump_red_batch;

#[path = "../self_pump_green_batch.rs"]
mod self_pump_green_batch;

#[path = "../convoke_batch.rs"]
mod convoke_batch;

#[path = "../prowess_batch.rs"]
mod prowess_batch;

#[path = "../protection_red_batch.rs"]
mod protection_red_batch;

#[path = "../flier_blocker_batch.rs"]
mod flier_blocker_batch;

#[path = "../flash_hexproof_aura_batch.rs"]
mod flash_hexproof_aura_batch;

#[path = "../etb_scry_value_batch.rs"]
mod etb_scry_value_batch;

#[path = "../aura_menace_batch.rs"]
mod aura_menace_batch;

#[path = "../first_strike_scry_batch.rs"]
mod first_strike_scry_batch;

#[path = "../shrink_draw_batch.rs"]
mod shrink_draw_batch;

#[path = "../landfall_trample_batch.rs"]
mod landfall_trample_batch;

#[path = "../etb_damage_creature_batch.rs"]
mod etb_damage_creature_batch;

#[path = "../team_pump_on_enter_batch.rs"]
mod team_pump_on_enter_batch;

#[path = "../wall_regenerate_batch.rs"]
mod wall_regenerate_batch;

#[path = "../flier_regenerate_batch.rs"]
mod flier_regenerate_batch;

#[path = "../wall_pump_batch.rs"]
mod wall_pump_batch;

#[path = "../flash_etb_pump_batch.rs"]
mod flash_etb_pump_batch;

#[path = "../equipment_vigilance_batch.rs"]
mod equipment_vigilance_batch;

#[path = "../vanilla_vehicles_batch.rs"]
mod vanilla_vehicles_batch;

#[path = "../soft_counter_cycling_batch.rs"]
mod soft_counter_cycling_batch;

#[path = "../fog_cycling_batch.rs"]
mod fog_cycling_batch;

#[path = "../artifact_sac_ping_batch.rs"]
mod artifact_sac_ping_batch;

#[path = "../regenerate_activated_batch.rs"]
mod regenerate_activated_batch;

#[path = "../self_pump_activated_batch.rs"]
mod self_pump_activated_batch;

#[path = "../tap_target_mana_batch.rs"]
mod tap_target_mana_batch;

#[path = "../grant_flying_activated_batch.rs"]
mod grant_flying_activated_batch;

#[path = "../gain_life_draw_batch.rs"]
mod gain_life_draw_batch;

#[path = "../vigilance_crew_batch.rs"]
mod vigilance_crew_batch;

#[path = "../cycling_keyword_batch.rs"]
mod cycling_keyword_batch;

#[path = "../tap_grant_haste_batch.rs"]
mod tap_grant_haste_batch;

#[path = "../changeling_keyword_batch.rs"]
mod changeling_keyword_batch;

#[path = "../dies_counter_target_batch.rs"]
mod dies_counter_target_batch;

#[path = "../dies_drain_batch.rs"]
mod dies_drain_batch;

#[path = "../sacrifice_draw_batch.rs"]
mod sacrifice_draw_batch;

#[path = "../etb_edict_batch.rs"]
mod etb_edict_batch;

#[path = "../etb_cant_block_batch.rs"]
mod etb_cant_block_batch;

#[path = "../etb_draw_lose_life_batch.rs"]
mod etb_draw_lose_life_batch;

#[path = "../etb_basic_search_tapped_batch.rs"]
mod etb_basic_search_tapped_batch;

#[path = "../flying_neg_pump_batch.rs"]
mod flying_neg_pump_batch;

#[path = "../aura_pump_haste_batch.rs"]
mod aura_pump_haste_batch;

#[path = "../flying_any_mana_batch.rs"]
mod flying_any_mana_batch;

#[path = "../equipment_haste_batch.rs"]
mod equipment_haste_batch;

#[path = "../mana_target_pump_batch.rs"]
mod mana_target_pump_batch;

#[path = "../haste_self_pump_batch.rs"]
mod haste_self_pump_batch;

#[path = "../trample_self_pump_batch.rs"]
mod trample_self_pump_batch;

#[path = "../naturalize_lifegain_batch.rs"]
mod naturalize_lifegain_batch;

#[path = "../damage_creature_lifegain_batch.rs"]
mod damage_creature_lifegain_batch;

#[path = "../prevention_shield_batch.rs"]
mod prevention_shield_batch;

#[path = "../etb_vanilla_tokens_batch.rs"]
mod etb_vanilla_tokens_batch;

#[path = "../sac_lands_cc_batch.rs"]
mod sac_lands_cc_batch;

#[path = "../investigate_lands_batch.rs"]
mod investigate_lands_batch;

#[path = "../artifact_lands_bridge_batch.rs"]
mod artifact_lands_bridge_batch;

#[path = "../tri_mana_obelisks_batch.rs"]
mod tri_mana_obelisks_batch;

#[path = "../sac_creature_three_batch.rs"]
mod sac_creature_three_batch;

#[path = "../blocks_pump_batch.rs"]
mod blocks_pump_batch;

#[path = "../becomes_target_sacrifice_batch.rs"]
mod becomes_target_sacrifice_batch;

#[path = "../flying_grant_activated_batch.rs"]
mod flying_grant_activated_batch;

#[path = "../unblockable_power_two_batch.rs"]
mod unblockable_power_two_batch;

#[path = "../horizon_canopy_cycle_batch.rs"]
mod horizon_canopy_cycle_batch;

#[path = "../slow_lands_batch.rs"]
mod slow_lands_batch;

#[path = "../fast_lands_batch.rs"]
mod fast_lands_batch;

#[path = "../abandoned_outpost_batch.rs"]
mod abandoned_outpost_batch;

#[path = "../desert_cycling_batch.rs"]
mod desert_cycling_batch;

#[path = "../botanical_plaza_batch.rs"]
mod botanical_plaza_batch;

#[path = "../sac_creature_self_pump_batch.rs"]
mod sac_creature_self_pump_batch;

#[path = "../cameo_artifacts_batch.rs"]
mod cameo_artifacts_batch;

#[path = "../sliver_anthem_controlled_batch.rs"]
mod sliver_anthem_controlled_batch;

#[path = "../pain_taplands_batch.rs"]
mod pain_taplands_batch;

#[path = "../cluestones_batch.rs"]
mod cluestones_batch;

#[path = "../lockets_batch.rs"]
mod lockets_batch;

#[path = "../banners_batch.rs"]
mod banners_batch;

#[path = "../other_anthems_batch.rs"]
mod other_anthems_batch;

#[path = "../flying_once_pump_batch.rs"]
mod flying_once_pump_batch;

#[path = "../etb_lose_life_flyers_batch.rs"]
mod etb_lose_life_flyers_batch;

#[path = "../etb_pump_flying_batch.rs"]
mod etb_pump_flying_batch;

#[path = "../mill_two_batch.rs"]
mod mill_two_batch;

#[path = "../blocked_by_power_two_batch.rs"]
mod blocked_by_power_two_batch;

#[path = "../max_one_blocker_batch.rs"]
mod max_one_blocker_batch;

#[path = "../counter_typed_spells_batch.rs"]
mod counter_typed_spells_batch;

#[path = "../graveyard_type_recursion_batch.rs"]
mod graveyard_type_recursion_batch;

#[path = "../bounce_spells_batch.rs"]
mod bounce_spells_batch;

#[path = "../etb_freeze_batch.rs"]
mod etb_freeze_batch;

#[path = "../dies_destroy_land_batch.rs"]
mod dies_destroy_land_batch;

#[path = "../graveyard_to_top_batch.rs"]
mod graveyard_to_top_batch;

#[path = "../look_three_batch.rs"]
mod look_three_batch;

#[path = "../team_counters_batch.rs"]
mod team_counters_batch;

#[path = "../keyword_only_batch.rs"]
mod keyword_only_batch;

#[path = "../multi_mana_producers_batch.rs"]
mod multi_mana_producers_batch;

#[path = "../double_strike_grants_batch.rs"]
mod double_strike_grants_batch;

#[path = "../x_pumps_batch.rs"]
mod x_pumps_batch;

#[path = "../draw_two_lose_two_batch.rs"]
mod draw_two_lose_two_batch;

#[path = "../attack_tap_batch.rs"]
mod attack_tap_batch;

#[path = "../untap_self_batch.rs"]
mod untap_self_batch;

#[path = "../sac_artifact_pump_batch.rs"]
mod sac_artifact_pump_batch;

#[path = "../q29_sac_artifact_ping_batch.rs"]
mod q29_sac_artifact_ping_batch;

#[path = "../q29_dies_another_counter_batch.rs"]
mod q29_dies_another_counter_batch;

#[path = "../q29_attack_may_tap_batch.rs"]
mod q29_attack_may_tap_batch;

#[path = "../q29_target_sacrifice_batch.rs"]
mod q29_target_sacrifice_batch;

#[path = "../q29_snow_basics_batch.rs"]
mod q29_snow_basics_batch;

#[path = "../q29_ally_counter_batch.rs"]
mod q29_ally_counter_batch;

#[path = "../q29_opponent_shrink_batch.rs"]
mod q29_opponent_shrink_batch;

#[path = "../q29_destroy_islands_batch.rs"]
mod q29_destroy_islands_batch;

#[path = "../q29_search_top_lose_two_batch.rs"]
mod q29_search_top_lose_two_batch;

#[path = "../q30_counter_draw_batch.rs"]
mod q30_counter_draw_batch;

#[path = "../q30_destroy_creature_filtered_batch.rs"]
mod q30_destroy_creature_filtered_batch;

#[path = "../q30_free_sac_outlets_batch.rs"]
mod q30_free_sac_outlets_batch;

#[path = "../q30_tap_looters_batch.rs"]
mod q30_tap_looters_batch;

#[path = "../q30_protection_remnants_batch.rs"]
mod q30_protection_remnants_batch;

#[path = "../q30_no_flying_block_batch.rs"]
mod q30_no_flying_block_batch;

#[path = "../q30_etb_naturalize_batch.rs"]
mod q30_etb_naturalize_batch;

#[path = "../q30_etb_bounce_batch.rs"]
mod q30_etb_bounce_batch;

#[path = "../q30_exile_creature_filtered_batch.rs"]
mod q30_exile_creature_filtered_batch;

#[path = "../q31_checklands_batch.rs"]
mod q31_checklands_batch;

#[path = "../q31_bounce_lands_batch.rs"]
mod q31_bounce_lands_batch;

#[path = "../q31_turbulent_lands_batch.rs"]
mod q31_turbulent_lands_batch;

#[path = "../q31_sphere_lands_batch.rs"]
mod q31_sphere_lands_batch;

#[path = "../q31_metalcraft_batch.rs"]
mod q31_metalcraft_batch;

#[path = "../q31_heroic_counters_batch.rs"]
mod q31_heroic_counters_batch;

#[path = "../q31_battle_cry_batch.rs"]
mod q31_battle_cry_batch;

#[path = "../q31_treasure_loot_batch.rs"]
mod q31_treasure_loot_batch;

#[path = "../q31_graveyard_top_draw_batch.rs"]
mod q31_graveyard_top_draw_batch;

#[path = "../q32_diamond_artifacts_batch.rs"]
mod q32_diamond_artifacts_batch;

#[path = "../q32_ramos_artifacts_batch.rs"]
mod q32_ramos_artifacts_batch;

#[path = "../q32_elder_lands_batch.rs"]
mod q32_elder_lands_batch;

#[path = "../q32_crystal_cycle_batch.rs"]
mod q32_crystal_cycle_batch;

#[path = "../q32_plus_minus_auras_batch.rs"]
mod q32_plus_minus_auras_batch;

#[path = "../q32_self_sac_ping_batch.rs"]
mod q32_self_sac_ping_batch;

#[path = "../q32_activated_lifegain_batch.rs"]
mod q32_activated_lifegain_batch;

#[path = "../q32_flying_untap_batch.rs"]
mod q32_flying_untap_batch;

#[path = "../q32_untap_pump_batch.rs"]
mod q32_untap_pump_batch;

#[path = "../q33_upkeep_damage_batch.rs"]
mod q33_upkeep_damage_batch;

#[path = "../q33_lifegain_counter_batch.rs"]
mod q33_lifegain_counter_batch;

#[path = "../q33_soul_warden_batch.rs"]
mod q33_soul_warden_batch;

#[path = "../q33_etb_team_counter_batch.rs"]
mod q33_etb_team_counter_batch;

#[path = "../q33_graveyard_self_return_batch.rs"]
mod q33_graveyard_self_return_batch;

#[path = "../q33_cost_reduction_batch.rs"]
mod q33_cost_reduction_batch;

#[path = "../q33_creature_mana_abilities_batch.rs"]
mod q33_creature_mana_abilities_batch;

#[path = "../q33_tapland_remnants_batch.rs"]
mod q33_tapland_remnants_batch;

#[path = "../q33_keyword_only_remnants_batch.rs"]
mod q33_keyword_only_remnants_batch;

#[path = "../q34_shrink_spells_batch.rs"]
mod q34_shrink_spells_batch;

#[path = "../q34_prevention_shields_batch.rs"]
mod q34_prevention_shields_batch;

#[path = "../q34_targeted_lifegain_batch.rs"]
mod q34_targeted_lifegain_batch;

#[path = "../q34_death_shrink_batch.rs"]
mod q34_death_shrink_batch;

#[path = "../q34_death_life_loss_batch.rs"]
mod q34_death_life_loss_batch;

#[path = "../q34_blocked_pump_batch.rs"]
mod q34_blocked_pump_batch;

#[path = "../q34_power_evasion_batch.rs"]
mod q34_power_evasion_batch;

#[path = "../q34_during_turn_pump_batch.rs"]
mod q34_during_turn_pump_batch;

#[path = "../q34_self_first_strike_batch.rs"]
mod q34_self_first_strike_batch;

#[path = "../q35_prevent_creature_batch.rs"]
mod q35_prevent_creature_batch;

#[path = "../q35_trample_pump_batch.rs"]
mod q35_trample_pump_batch;

#[path = "../q35_self_first_strike_batch.rs"]
mod q35_self_first_strike_batch;

#[path = "../q35_target_first_strike_batch.rs"]
mod q35_target_first_strike_batch;

#[path = "../q35_cant_block_batch.rs"]
mod q35_cant_block_batch;

#[path = "../q35_campus_lands_batch.rs"]
mod q35_campus_lands_batch;

#[path = "../q35_reach_pump_batch.rs"]
mod q35_reach_pump_batch;

#[path = "../q35_two_target_pump_batch.rs"]
mod q35_two_target_pump_batch;

#[path = "../q35_double_pump_shrink_batch.rs"]
mod q35_double_pump_shrink_batch;

#[path = "../q36_pump_scry_batch.rs"]
mod q36_pump_scry_batch;

#[path = "../q36_shrink_scry_batch.rs"]
mod q36_shrink_scry_batch;

#[path = "../q36_damage_scry_batch.rs"]
mod q36_damage_scry_batch;

#[path = "../q36_etb_mill_batch.rs"]
mod q36_etb_mill_batch;

#[path = "../q36_team_haste_batch.rs"]
mod q36_team_haste_batch;

#[path = "../q36_self_deathtouch_batch.rs"]
mod q36_self_deathtouch_batch;

#[path = "../q36_pump_fight_batch.rs"]
mod q36_pump_fight_batch;

#[path = "../q36_untap_reach_pump_batch.rs"]
mod q36_untap_reach_pump_batch;

#[path = "../q36_self_unblockable_batch.rs"]
mod q36_self_unblockable_batch;

#[path = "../q37_panorama_lands_batch.rs"]
mod q37_panorama_lands_batch;

#[path = "../q37_cycling_fetch_lands_batch.rs"]
mod q37_cycling_fetch_lands_batch;

#[path = "../q37_prevention_activated_batch.rs"]
mod q37_prevention_activated_batch;

#[path = "../q37_self_keyword_activated_batch.rs"]
mod q37_self_keyword_activated_batch;

#[path = "../q37_target_keyword_activated_batch.rs"]
mod q37_target_keyword_activated_batch;

#[path = "../q37_pump_keyword_batch.rs"]
mod q37_pump_keyword_batch;

#[path = "../q37_grant_keyword_batch.rs"]
mod q37_grant_keyword_batch;

#[path = "../q37_landfall_keyword_batch.rs"]
mod q37_landfall_keyword_batch;

#[path = "../q37_tap_keyword_activated_batch.rs"]
mod q37_tap_keyword_activated_batch;

#[path = "../q38_creature_player_sweep_batch.rs"]
mod q38_creature_player_sweep_batch;

#[path = "../q38_hand_damage_batch.rs"]
mod q38_hand_damage_batch;

#[path = "../q38_flyer_damage_batch.rs"]
mod q38_flyer_damage_batch;

#[path = "../q38_flyer_sweep_batch.rs"]
mod q38_flyer_sweep_batch;

#[path = "../q38_attacking_sweep_batch.rs"]
mod q38_attacking_sweep_batch;

#[path = "../q38_spiketail_counters_batch.rs"]
mod q38_spiketail_counters_batch;

#[path = "../q38_tap_power_batch.rs"]
mod q38_tap_power_batch;

#[path = "../q38_mountain_damage_batch.rs"]
mod q38_mountain_damage_batch;

#[path = "../q38_aura_pump_batch.rs"]
mod q38_aura_pump_batch;

#[path = "../q39_aura_static_keyword_batch.rs"]
mod q39_aura_static_keyword_batch;

#[path = "../q39_equipment_static_keyword_batch.rs"]
mod q39_equipment_static_keyword_batch;

#[path = "../q39_land_etb_target_batch.rs"]
mod q39_land_etb_target_batch;

#[path = "../q39_draw_discard_batch.rs"]
mod q39_draw_discard_batch;

#[path = "../q39_destroy_gain_life_batch.rs"]
mod q39_destroy_gain_life_batch;

#[path = "../q39_exile_simple_batch.rs"]
mod q39_exile_simple_batch;

#[path = "../q39_counter_rider_batch.rs"]
mod q39_counter_rider_batch;

#[path = "../q39_tap_ping_batch.rs"]
mod q39_tap_ping_batch;

#[path = "../q39_targeted_utility_lands_batch.rs"]
mod q39_targeted_utility_lands_batch;

#[path = "../q40_cohorts_batch.rs"]
mod q40_cohorts_batch;

#[path = "../q41_cohorts_batch.rs"]
mod q41_cohorts_batch;

#[path = "../q42_cohorts_batch.rs"]
mod q42_cohorts_batch;
#[path = "../q43_cohorts_batch.rs"]
mod q43_cohorts_batch;
#[path = "../q44_cohorts_batch.rs"]
mod q44_cohorts_batch;

#[path = "../q45_cohorts_batch.rs"]
mod q45_cohorts_batch;

#[path = "../q46_cohorts_batch.rs"]
mod q46_cohorts_batch;

#[path = "../q47_cohorts_batch.rs"]
mod q47_cohorts_batch;

#[path = "../q48_cohorts_batch.rs"]
mod q48_cohorts_batch;

#[path = "../q49_cohorts_batch.rs"]
mod q49_cohorts_batch;

#[path = "../q49_batch_b.rs"]
mod q49_batch_b;

#[path = "../q49_batch_c.rs"]
mod q49_batch_c;

#[path = "../q50_cohorts_batch.rs"]
mod q50_cohorts_batch;

#[path = "../q51_cohorts_batch.rs"]
mod q51_cohorts_batch;

#[path = "../q52_cohorts_batch.rs"]
mod q52_cohorts_batch;
#[path = "../q53_batch_a.rs"]
mod q53_batch_a;
#[path = "../q53_batch_b.rs"]
mod q53_batch_b;
#[path = "../q53_batch_c.rs"]
mod q53_batch_c;

#[path = "../q54_batch_a.rs"]
mod q54_batch_a;

#[path = "../q54_batch_b.rs"]
mod q54_batch_b;

#[path = "../q54_batch_c.rs"]
mod q54_batch_c;

#[path = "../q55_batch_a.rs"]
mod q55_batch_a;

#[path = "../q55_batch_b.rs"]
mod q55_batch_b;

#[path = "../q55_batch_c.rs"]
mod q55_batch_c;

#[path = "../q56_batch_a.rs"]
mod q56_batch_a;

#[path = "../q56_batch_b.rs"]
mod q56_batch_b;

#[path = "../q56_batch_c.rs"]
mod q56_batch_c;

#[path = "../q57_batch_a.rs"]
mod q57_batch_a;

#[path = "../q58_batch_a.rs"]
mod q58_batch_a;

#[path = "../q59_batch_a.rs"]
mod q59_batch_a;

#[path = "../q60_batch_a.rs"]
mod q60_batch_a;

#[path = "../q61_batch_a.rs"]
mod q61_batch_a;

#[path = "../q62_batch_a.rs"]
mod q62_batch_a;

#[path = "../q63_batch_a.rs"]
mod q63_batch_a;

#[path = "../q64_batch_a.rs"]
mod q64_batch_a;
