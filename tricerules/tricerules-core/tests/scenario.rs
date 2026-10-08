//! Scripted command sequences (M2), split into themed submodules.
//!
//! `tests/scenario.rs` is the crate root of the `scenario` integration-test binary, so a bare
//! `mod foo;` would resolve to `tests/foo.rs` (which Cargo would then compile as its own test
//! binary). The `#[path]` attributes keep every submodule under `tests/scenario/` while
//! preserving a single `scenario` test binary.

#[path = "scenario/deck_coverage_devotion_gods.rs"]
mod deck_coverage_devotion_gods;

#[path = "scenario/deck_coverage_doubling_season.rs"]
mod deck_coverage_doubling_season;

#[path = "scenario/deck_coverage_aligned_hedron_network.rs"]
mod deck_coverage_aligned_hedron_network;

#[path = "scenario/deck_coverage_lavabrink_floodgates.rs"]
mod deck_coverage_lavabrink_floodgates;

#[path = "scenario/deck_coverage_pyreswipe_hawk.rs"]
mod deck_coverage_pyreswipe_hawk;

#[path = "scenario/deck_coverage_thickest_in_the_thicket.rs"]
mod deck_coverage_thickest_in_the_thicket;

#[path = "scenario/deck_coverage_flux_abundance.rs"]
mod deck_coverage_flux_abundance;

#[path = "scenario/deck_coverage_combustible_gearhulk.rs"]
mod deck_coverage_combustible_gearhulk;

#[path = "scenario/deck_coverage_flameshadow_conjuring.rs"]
mod deck_coverage_flameshadow_conjuring;
#[path = "scenario/deck_coverage_mirrorworks.rs"]
mod deck_coverage_mirrorworks;

#[path = "scenario/deck_coverage_natures_will.rs"]
mod deck_coverage_natures_will;

#[path = "scenario/deck_coverage_graveyard_return.rs"]
mod deck_coverage_graveyard_return;

#[path = "scenario/deck_coverage_pupu_ufo.rs"]
mod deck_coverage_pupu_ufo;

#[path = "scenario/deck_coverage_sword_of_war_and_peace.rs"]
mod deck_coverage_sword_of_war_and_peace;

#[path = "scenario/deck_coverage_alhammarrets_archive.rs"]
mod deck_coverage_alhammarrets_archive;
#[path = "scenario/deck_coverage_boompile.rs"]
mod deck_coverage_boompile;
#[path = "scenario/deck_coverage_maze_of_ith.rs"]
mod deck_coverage_maze_of_ith;
#[path = "scenario/deck_coverage_mutational_advantage.rs"]
mod deck_coverage_mutational_advantage;
#[path = "scenario/deck_coverage_myr_battlesphere.rs"]
mod deck_coverage_myr_battlesphere;
#[path = "scenario/deck_coverage_standstill.rs"]
mod deck_coverage_standstill;
#[path = "scenario/deck_coverage_thassa_oracle.rs"]
mod deck_coverage_thassa_oracle;
#[path = "scenario/myr_damage_prerequisites.rs"]
mod myr_damage_prerequisites;
#[path = "scenario/myriad_search_prerequisites.rs"]
mod myriad_search_prerequisites;

#[path = "scenario/deploy_the_gatewatch.rs"]
mod deploy_the_gatewatch;

#[path = "scenario/chaos_warp.rs"]
mod chaos_warp;

#[path = "scenario/authoring_actions.rs"]
mod authoring_actions;

#[path = "scenario/replicating_ring_runtime.rs"]
mod replicating_ring_runtime;

#[path = "scenario/deck_coverage_chromatic_lantern.rs"]
mod deck_coverage_chromatic_lantern;
#[path = "scenario/deck_coverage_replicating_ring.rs"]
mod deck_coverage_replicating_ring;
#[path = "scenario/deck_coverage_scrap_mastery.rs"]
mod deck_coverage_scrap_mastery;

#[path = "scenario/deck_coverage_crawlspace.rs"]
mod deck_coverage_crawlspace;
#[path = "scenario/deck_coverage_phyrexian_metamorph.rs"]
mod deck_coverage_phyrexian_metamorph;
#[path = "scenario/deck_coverage_threefold_thunderhulk.rs"]
mod deck_coverage_threefold_thunderhulk;

#[path = "scenario/deck_coverage_entry_reveal.rs"]
mod deck_coverage_entry_reveal;

#[path = "scenario/deck_coverage_boseiju.rs"]
mod deck_coverage_boseiju;

#[path = "scenario/deck_coverage_pentad_prism.rs"]
mod deck_coverage_pentad_prism;

#[path = "scenario/deck_coverage_land_type_pair.rs"]
mod deck_coverage_land_type_pair;

#[path = "scenario/deck_coverage_zenith_chronicler.rs"]
mod deck_coverage_zenith_chronicler;

#[path = "scenario/deck_coverage_blue_suns_zenith.rs"]
mod deck_coverage_blue_suns_zenith;

#[path = "scenario/deck_coverage_propaganda.rs"]
mod deck_coverage_propaganda;

#[path = "scenario/deck_coverage_spine_of_ish_sah.rs"]
mod deck_coverage_spine_of_ish_sah;

#[path = "scenario/deck_coverage_unwinding_clock.rs"]
mod deck_coverage_unwinding_clock;

#[path = "scenario/deck_coverage_pentavus.rs"]
mod deck_coverage_pentavus;
#[path = "scenario/deck_coverage_titan.rs"]
mod deck_coverage_titan;

#[path = "scenario/deck_coverage_darksteel_forge.rs"]
mod deck_coverage_darksteel_forge;
#[path = "scenario/deck_coverage_psychosis_crawler.rs"]
mod deck_coverage_psychosis_crawler;

#[path = "scenario/deck_coverage_fanatic_of_rhonas.rs"]
mod deck_coverage_fanatic_of_rhonas;

#[path = "scenario/deck_coverage_skullclamp.rs"]
mod deck_coverage_skullclamp;

#[path = "scenario/deck_coverage_tolarian_winds.rs"]
mod deck_coverage_tolarian_winds;
#[path = "scenario/deck_coverage_trading_post.rs"]
mod deck_coverage_trading_post;

#[path = "scenario/deck_coverage_trailtracker_scout.rs"]
mod deck_coverage_trailtracker_scout;

#[path = "scenario/deck_coverage_raging_ravine.rs"]
mod deck_coverage_raging_ravine;

#[path = "scenario/deck_coverage_kenriths_transformation.rs"]
mod deck_coverage_kenriths_transformation;

#[path = "scenario/deck_coverage_staff_of_compleation.rs"]
mod deck_coverage_staff_of_compleation;

#[path = "scenario/deck_coverage_beacon_chandra.rs"]
mod deck_coverage_beacon_chandra;

#[path = "scenario/deck_coverage_teferi_temporal_pilgrim.rs"]
mod deck_coverage_teferi_temporal_pilgrim;

#[path = "scenario/deck_coverage_wizard_class.rs"]
mod deck_coverage_wizard_class;

#[path = "scenario/deck_coverage_minds_aglow.rs"]
mod deck_coverage_minds_aglow;

#[path = "scenario/deck_coverage_garruks_uprising.rs"]
mod deck_coverage_garruks_uprising;

#[path = "scenario/deck_coverage_basic_mana.rs"]
mod deck_coverage_basic_mana;

#[path = "scenario/semantic_fixtures.rs"]
mod semantic_fixtures;

#[path = "scenario/deck_coverage_fruition_tower.rs"]
mod deck_coverage_fruition_tower;

#[cfg(feature = "authoring")]
#[path = "scenario/authoring_drafts.rs"]
mod authoring_drafts;

#[path = "scenario/anti_venom.rs"]
mod anti_venom;

#[path = "scenario/issue_231_spell_filters.rs"]
mod issue_231_spell_filters;
#[path = "scenario/issue_236_hydro_man.rs"]
mod issue_236_hydro_man;
#[path = "scenario/issue_238_ral_crackling_wit.rs"]
mod issue_238_ral_crackling_wit;
#[path = "scenario/issue_242_heated_argument.rs"]
mod issue_242_heated_argument;
#[path = "scenario/issue_243_evils_thrall.rs"]
mod issue_243_evils_thrall;
#[path = "scenario/issue_248_prowess.rs"]
mod issue_248_prowess;
#[path = "scenario/issue_249_generated_cycling.rs"]
mod issue_249_generated_cycling;
#[path = "scenario/issue_250_etb_explore.rs"]
mod issue_250_etb_explore;
#[path = "scenario/issue_251_generated_naturalize.rs"]
mod issue_251_generated_naturalize;
#[path = "scenario/issue_252_generated_recipe_batch.rs"]
mod issue_252_generated_recipe_batch;
#[path = "scenario/issue_253_generated_lands.rs"]
mod issue_253_generated_lands;
#[path = "scenario/issue_254_generated_utility_lands.rs"]
mod issue_254_generated_utility_lands;
#[path = "scenario/issue_257_generated_recipe_batch.rs"]
mod issue_257_generated_recipe_batch;
#[path = "scenario/issue_258_generated_conditional_lands.rs"]
mod issue_258_generated_conditional_lands;

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
#[path = "scenario/deck_coverage_guardian_project.rs"]
mod deck_coverage_guardian_project;

#[path = "scenario/counters_and_pump.rs"]
mod counters_and_pump;
#[path = "scenario/current_standard_coverage_2026_09_10.rs"]
mod current_standard_coverage_2026_09_10;
#[path = "scenario/custom_resolution.rs"]
mod custom_resolution;
#[path = "scenario/damage_prevention.rs"]
mod damage_prevention;
#[path = "scenario/deck_coverage_ancient_grudge.rs"]
mod deck_coverage_ancient_grudge;
#[path = "scenario/deck_coverage_anvil.rs"]
mod deck_coverage_anvil;
#[path = "scenario/deck_coverage_astral_cornucopia.rs"]
mod deck_coverage_astral_cornucopia;
#[path = "scenario/deck_coverage_austere_decimate.rs"]
mod deck_coverage_austere_decimate;
#[path = "scenario/deck_coverage_battlelands.rs"]
mod deck_coverage_battlelands;
#[path = "scenario/deck_coverage_beast_within.rs"]
mod deck_coverage_beast_within;
#[path = "scenario/deck_coverage_berserkers_onslaught.rs"]
mod deck_coverage_berserkers_onslaught;
#[path = "scenario/deck_coverage_brokers_ascendancy.rs"]
mod deck_coverage_brokers_ascendancy;
#[path = "scenario/deck_coverage_buried_ruin.rs"]
mod deck_coverage_buried_ruin;
#[path = "scenario/deck_coverage_checklands.rs"]
mod deck_coverage_checklands;
#[path = "scenario/deck_coverage_chromatic_star.rs"]
mod deck_coverage_chromatic_star;
#[path = "scenario/deck_coverage_clock_of_omens.rs"]
mod deck_coverage_clock_of_omens;
#[path = "scenario/deck_coverage_codex_shredder.rs"]
mod deck_coverage_codex_shredder;
#[path = "scenario/deck_coverage_commander_mana.rs"]
mod deck_coverage_commander_mana;
#[path = "scenario/deck_coverage_copperline_prismatic.rs"]
mod deck_coverage_copperline_prismatic;
#[path = "scenario/deck_coverage_creature_board_wipes.rs"]
mod deck_coverage_creature_board_wipes;
#[path = "scenario/deck_coverage_cultivate.rs"]
mod deck_coverage_cultivate;
#[path = "scenario/deck_coverage_cycling_taplands.rs"]
mod deck_coverage_cycling_taplands;
#[path = "scenario/deck_coverage_darksteel_plate.rs"]
mod deck_coverage_darksteel_plate;
#[path = "scenario/deck_coverage_devoid.rs"]
mod deck_coverage_devoid;
#[path = "scenario/deck_coverage_everflowing_chalice.rs"]
mod deck_coverage_everflowing_chalice;
#[path = "scenario/deck_coverage_exotic_orchard.rs"]
mod deck_coverage_exotic_orchard;
#[path = "scenario/deck_coverage_fire_lit_thicket.rs"]
mod deck_coverage_fire_lit_thicket;
#[path = "scenario/deck_coverage_garruks_packleader.rs"]
mod deck_coverage_garruks_packleader;
#[path = "scenario/deck_coverage_goblin_engineer.rs"]
mod deck_coverage_goblin_engineer;
#[path = "scenario/deck_coverage_goblin_welder.rs"]
mod deck_coverage_goblin_welder;
#[path = "scenario/deck_coverage_gruul_war_chant.rs"]
mod deck_coverage_gruul_war_chant;
#[path = "scenario/deck_coverage_hand_size_mana.rs"]
mod deck_coverage_hand_size_mana;
#[path = "scenario/deck_coverage_ichor_wellspring.rs"]
mod deck_coverage_ichor_wellspring;
#[path = "scenario/deck_coverage_khalni_ambush.rs"]
mod deck_coverage_khalni_ambush;
#[path = "scenario/deck_coverage_krark_clan_ironworks.rs"]
mod deck_coverage_krark_clan_ironworks;
#[path = "scenario/deck_coverage_liquimetal_torque.rs"]
mod deck_coverage_liquimetal_torque;
#[path = "scenario/deck_coverage_looting.rs"]
mod deck_coverage_looting;
#[path = "scenario/deck_coverage_lotus_petal.rs"]
mod deck_coverage_lotus_petal;
#[path = "scenario/deck_coverage_mana_reflection.rs"]
mod deck_coverage_mana_reflection;
#[path = "scenario/deck_coverage_manifold_key.rs"]
mod deck_coverage_manifold_key;
#[path = "scenario/deck_coverage_maximum_hand_size.rs"]
mod deck_coverage_maximum_hand_size;
#[path = "scenario/deck_coverage_mh3_mdfcs.rs"]
mod deck_coverage_mh3_mdfcs;
#[path = "scenario/deck_coverage_mind_stone.rs"]
mod deck_coverage_mind_stone;
#[path = "scenario/deck_coverage_mithril_coat.rs"]
mod deck_coverage_mithril_coat;
#[path = "scenario/deck_coverage_mossfire_valley.rs"]
mod deck_coverage_mossfire_valley;
#[path = "scenario/deck_coverage_myr_retriever.rs"]
mod deck_coverage_myr_retriever;
#[path = "scenario/deck_coverage_nevinyrrals_disk.rs"]
mod deck_coverage_nevinyrrals_disk;
#[path = "scenario/deck_coverage_ohran_frostfang.rs"]
mod deck_coverage_ohran_frostfang;
#[path = "scenario/deck_coverage_ominous_seas.rs"]
mod deck_coverage_ominous_seas;
#[path = "scenario/deck_coverage_opponent_draw.rs"]
mod deck_coverage_opponent_draw;
#[path = "scenario/deck_coverage_pongify.rs"]
mod deck_coverage_pongify;
#[path = "scenario/deck_coverage_prized_statue.rs"]
mod deck_coverage_prized_statue;
#[path = "scenario/deck_coverage_proft_eidetic_memory.rs"]
mod deck_coverage_proft_eidetic_memory;
#[path = "scenario/deck_coverage_prosperity.rs"]
mod deck_coverage_prosperity;
#[path = "scenario/deck_coverage_quicksmith_genius.rs"]
mod deck_coverage_quicksmith_genius;
#[path = "scenario/deck_coverage_sacrifice_artifacts.rs"]
mod deck_coverage_sacrifice_artifacts;
#[path = "scenario/deck_coverage_sakura_tribe_elder.rs"]
mod deck_coverage_sakura_tribe_elder;
#[path = "scenario/deck_coverage_scavenger_grounds.rs"]
mod deck_coverage_scavenger_grounds;
#[path = "scenario/deck_coverage_search_lands.rs"]
mod deck_coverage_search_lands;
#[path = "scenario/deck_coverage_skarrg.rs"]
mod deck_coverage_skarrg;
#[path = "scenario/deck_coverage_sol_ring.rs"]
mod deck_coverage_sol_ring;
#[path = "scenario/deck_coverage_solemn_simulacrum.rs"]
mod deck_coverage_solemn_simulacrum;
#[path = "scenario/deck_coverage_stump_stomp.rs"]
mod deck_coverage_stump_stomp;
#[path = "scenario/deck_coverage_swan_song.rs"]
mod deck_coverage_swan_song;
#[path = "scenario/deck_coverage_thran_dynamo.rs"]
mod deck_coverage_thran_dynamo;
#[path = "scenario/deck_coverage_tri_lands.rs"]
mod deck_coverage_tri_lands;
#[path = "scenario/deck_coverage_urzas_cave.rs"]
mod deck_coverage_urzas_cave;
#[path = "scenario/deck_coverage_vision_skeins_mikokoro.rs"]
mod deck_coverage_vision_skeins_mikokoro;
#[path = "scenario/deck_coverage_vivid_grove.rs"]
mod deck_coverage_vivid_grove;
#[path = "scenario/deck_coverage_voltaic_key.rs"]
mod deck_coverage_voltaic_key;
#[path = "scenario/deck_coverage_waterlogged_grove.rs"]
mod deck_coverage_waterlogged_grove;
#[path = "scenario/deck_coverage_words_of_wisdom.rs"]
mod deck_coverage_words_of_wisdom;
#[path = "scenario/deck_coverage_zurs_weirding.rs"]
mod deck_coverage_zurs_weirding;
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
#[cfg(feature = "authoring")]
#[path = "scenario/entry_copy_auras.rs"]
mod entry_copy_auras;
#[path = "scenario/equipment.rs"]
mod equipment;
#[path = "scenario/font_of_mythos.rs"]
mod font_of_mythos;
#[path = "scenario/generator_servant.rs"]
mod generator_servant;
#[path = "scenario/hand_size_cohort_runtime.rs"]
mod hand_size_cohort_runtime;
#[path = "scenario/hand_size_upkeep_cohort.rs"]
mod hand_size_upkeep_cohort;
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
#[path = "scenario/issue_106_mobilize.rs"]
mod issue_106_mobilize;
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
#[path = "scenario/issue_113_characteristic_setting.rs"]
mod issue_113_characteristic_setting;
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
#[path = "scenario/issue_144_selectable_tap_payments.rs"]
mod issue_144_selectable_tap_payments;
#[path = "scenario/issue_145_convoke.rs"]
mod issue_145_convoke;
#[path = "scenario/issue_147_station.rs"]
mod issue_147_station;
#[path = "scenario/issue_148_warp_void.rs"]
mod issue_148_warp_void;
#[path = "scenario/issue_149_airbend.rs"]
mod issue_149_airbend;
#[path = "scenario/issue_151_firebending.rs"]
mod issue_151_firebending;
#[path = "scenario/issue_152_exhaust.rs"]
mod issue_152_exhaust;
#[path = "scenario/issue_153_blight.rs"]
mod issue_153_blight;
#[path = "scenario/issue_154_changeling.rs"]
mod issue_154_changeling;
#[path = "scenario/issue_155_attachment_actions.rs"]
mod issue_155_attachment_actions;
#[path = "scenario/issue_156_event_observers.rs"]
mod issue_156_event_observers;
#[path = "scenario/issue_158_predicates.rs"]
mod issue_158_predicates;
#[path = "scenario/issue_159_resolution_results.rs"]
mod issue_159_resolution_results;
#[path = "scenario/issue_160_blocker_constraints.rs"]
mod issue_160_blocker_constraints;
#[path = "scenario/issue_161_turn_boundaries.rs"]
mod issue_161_turn_boundaries;
#[path = "scenario/issue_162_tapped_tokens.rs"]
mod issue_162_tapped_tokens;
#[path = "scenario/issue_163_calibration_cards.rs"]
mod issue_163_calibration_cards;
#[path = "scenario/issue_169_actor_aware_taps.rs"]
mod issue_169_actor_aware_taps;
#[path = "scenario/issue_177_multi_option_cast_costs.rs"]
mod issue_177_multi_option_cast_costs;
#[path = "scenario/issue_178_aggregate_object_payments.rs"]
mod issue_178_aggregate_object_payments;
#[path = "scenario/issue_179_sneak.rs"]
mod issue_179_sneak;
#[path = "scenario/issue_180_preparation.rs"]
mod issue_180_preparation;
#[path = "scenario/issue_181_spell_mana_spent.rs"]
mod issue_181_spell_mana_spent;
#[path = "scenario/issue_182_teamwork.rs"]
mod issue_182_teamwork;
#[path = "scenario/issue_183_power_up.rs"]
mod issue_183_power_up;
#[path = "scenario/issue_184_storied.rs"]
mod issue_184_storied;
#[path = "scenario/issue_185_sagas.rs"]
mod issue_185_sagas;
#[path = "scenario/issue_186_amass.rs"]
mod issue_186_amass;
#[path = "scenario/issue_187_base_pt.rs"]
mod issue_187_base_pt;
#[path = "scenario/issue_188_chosen_equipment_attachment.rs"]
mod issue_188_chosen_equipment_attachment;
#[path = "scenario/issue_189_disappear.rs"]
mod issue_189_disappear;
#[path = "scenario/issue_190_restricted_mana_any_ability.rs"]
mod issue_190_restricted_mana_any_ability;
#[path = "scenario/issue_191_repartee.rs"]
mod issue_191_repartee;
#[path = "scenario/issue_192_calibration_cards.rs"]
mod issue_192_calibration_cards;
#[path = "scenario/issue_193_selected_counter_removal.rs"]
mod issue_193_selected_counter_removal;
#[path = "scenario/issue_194_dynamic_scope_combat_restrictions.rs"]
mod issue_194_dynamic_scope_combat_restrictions;
#[path = "scenario/issue_197_discard.rs"]
mod issue_197_discard;
#[path = "scenario/issue_199_alternative_additional_costs.rs"]
mod issue_199_alternative_additional_costs;
#[path = "scenario/issue_200_enduring_curiosity.rs"]
mod issue_200_enduring_curiosity;
#[path = "scenario/issue_201_multi_object_library.rs"]
mod issue_201_multi_object_library;
#[path = "scenario/issue_202_hidden_lair.rs"]
mod issue_202_hidden_lair;
#[path = "scenario/issue_203_kaito.rs"]
mod issue_203_kaito;
#[path = "scenario/issue_204_land_animation.rs"]
mod issue_204_land_animation;
#[path = "scenario/issue_205_stack_spell_filters.rs"]
mod issue_205_stack_spell_filters;
#[path = "scenario/issue_206_explore.rs"]
mod issue_206_explore;
#[path = "scenario/issue_207_stack_abilities.rs"]
mod issue_207_stack_abilities;
#[path = "scenario/issue_208_library_search.rs"]
mod issue_208_library_search;
#[path = "scenario/issue_209_entry_life_payment.rs"]
mod issue_209_entry_life_payment;
#[path = "scenario/issue_210_demolition_field.rs"]
mod issue_210_demolition_field;
#[path = "scenario/issue_211_reflexive_counter_placement.rs"]
mod issue_211_reflexive_counter_placement;
#[path = "scenario/issue_212_passages.rs"]
mod issue_212_passages;
#[path = "scenario/issue_213_esper_origins.rs"]
mod issue_213_esper_origins;
#[path = "scenario/issue_214_graveyard_land_play.rs"]
mod issue_214_graveyard_land_play;
#[path = "scenario/issue_215_crew.rs"]
mod issue_215_crew;
#[path = "scenario/issue_216_attachment_scoped_blocking_restrictions.rs"]
mod issue_216_attachment_scoped_blocking_restrictions;
#[path = "scenario/issue_217_target_power_scale.rs"]
mod issue_217_target_power_scale;
#[path = "scenario/issue_218_battlefield_exile_cost.rs"]
mod issue_218_battlefield_exile_cost;
#[path = "scenario/issue_219_surrak.rs"]
mod issue_219_surrak;
#[path = "scenario/issue_233_multiversal_passage.rs"]
mod issue_233_multiversal_passage;
#[path = "scenario/issue_234_ghost_vacuum.rs"]
mod issue_234_ghost_vacuum;
#[path = "scenario/issue_235_soul_guide_lantern.rs"]
mod issue_235_soul_guide_lantern;
#[path = "scenario/issue_237_token_copies.rs"]
mod issue_237_token_copies;
#[path = "scenario/issue_255_generated_lands.rs"]
mod issue_255_generated_lands;
#[path = "scenario/issue_256_generated_triggers.rs"]
mod issue_256_generated_triggers;
#[path = "scenario/issue_259_generated_creature_etbs.rs"]
mod issue_259_generated_creature_etbs;
#[path = "scenario/issue_260_generated_creature_triggers.rs"]
mod issue_260_generated_creature_triggers;
#[path = "scenario/issue_261_generated_attachments.rs"]
mod issue_261_generated_attachments;
#[path = "scenario/issue_262_generated_combat_tricks.rs"]
mod issue_262_generated_combat_tricks;
#[path = "scenario/issue_263_generated_creature_triggers.rs"]
mod issue_263_generated_creature_triggers;
#[path = "scenario/issue_264_generated_activated_abilities.rs"]
mod issue_264_generated_activated_abilities;
#[path = "scenario/issue_265_generated_modal_spells.rs"]
mod issue_265_generated_modal_spells;
#[path = "scenario/issue_266_generated_permanent_triggers.rs"]
mod issue_266_generated_permanent_triggers;
#[path = "scenario/issue_267_generated_static_templates.rs"]
mod issue_267_generated_static_templates;
#[path = "scenario/issue_269_generated_modal_spells.rs"]
mod issue_269_generated_modal_spells;
#[path = "scenario/issue_270_generated_utility_permanents.rs"]
mod issue_270_generated_utility_permanents;
#[path = "scenario/issue_277_affinity_artifacts.rs"]
mod issue_277_affinity_artifacts;
#[path = "scenario/issue_278_raid_etb_draw.rs"]
mod issue_278_raid_etb_draw;
#[path = "scenario/issue_279_landfall_damage.rs"]
mod issue_279_landfall_damage;
#[path = "scenario/issue_280_increment.rs"]
mod issue_280_increment;
#[path = "scenario/issue_281_generated_etb_recursion.rs"]
mod issue_281_generated_etb_recursion;
#[path = "scenario/issue_282_self_tap_loot.rs"]
mod issue_282_self_tap_loot;
#[path = "scenario/issue_283_generated_graveyard_to_library_bottom.rs"]
mod issue_283_generated_graveyard_to_library_bottom;
#[path = "scenario/issue_284_generated_optional_basic_land_to_top.rs"]
mod issue_284_generated_optional_basic_land_to_top;
#[path = "scenario/issue_285_generated_attack_pump_indestructible.rs"]
mod issue_285_generated_attack_pump_indestructible;
#[path = "scenario/issue_286_generated_equipment_defending_tap.rs"]
mod issue_286_generated_equipment_defending_tap;
#[path = "scenario/issue_288_generated_opponent_hand_exile.rs"]
mod issue_288_generated_opponent_hand_exile;
#[path = "scenario/issue_289_discard_batch.rs"]
mod issue_289_discard_batch;
#[path = "scenario/issue_290_generated_library_partition.rs"]
mod issue_290_generated_library_partition;
#[path = "scenario/issue_291_generated_mutagen.rs"]
mod issue_291_generated_mutagen;
#[path = "scenario/issue_297_city_pigeon.rs"]
mod issue_297_city_pigeon;
#[path = "scenario/issue_298_generated_aura_etb_keywords.rs"]
mod issue_298_generated_aura_etb_keywords;
#[path = "scenario/issue_299_generated_mercenary.rs"]
mod issue_299_generated_mercenary;
#[path = "scenario/issue_300_generated_land_sacrifice_draw.rs"]
mod issue_300_generated_land_sacrifice_draw;
#[path = "scenario/issue_341_static_spell_cost_filters.rs"]
mod issue_341_static_spell_cost_filters;
#[path = "scenario/issue_478_targeted_spell_controller.rs"]
mod issue_478_targeted_spell_controller;
#[path = "scenario/issue_479_land_comparison.rs"]
mod issue_479_land_comparison;
#[path = "scenario/issue_480_tainted_treats.rs"]
mod issue_480_tainted_treats;
#[path = "scenario/issue_481_conditional_library_choices.rs"]
mod issue_481_conditional_library_choices;
#[path = "scenario/issue_483_feed_the_swarm.rs"]
mod issue_483_feed_the_swarm;
#[path = "scenario/issue_487_red_guardian.rs"]
mod issue_487_red_guardian;
#[path = "scenario/issue_490_beza_comparison.rs"]
mod issue_490_beza_comparison;
#[path = "scenario/issue_494_damage_mana.rs"]
mod issue_494_damage_mana;
#[path = "scenario/issue_499_storage_counter_lands.rs"]
mod issue_499_storage_counter_lands;
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
#[path = "scenario/issue_72_planeswalkers_battles.rs"]
mod issue_72_planeswalkers_battles;
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
#[path = "scenario/maximum_hand_size_twenty.rs"]
mod maximum_hand_size_twenty;
#[path = "scenario/mill_results.rs"]
mod mill_results;
#[path = "scenario/modal_spells.rs"]
mod modal_spells;
#[path = "scenario/multi_attacker_triggers.rs"]
mod multi_attacker_triggers;
#[path = "scenario/multi_face.rs"]
mod multi_face;
#[path = "scenario/multiplayer_milestone.rs"]
mod multiplayer_milestone;
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
#[path = "scenario/standard_deck_mainboards.rs"]
mod standard_deck_mainboards;
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

#[path = "scenario/ability_logs.rs"]
mod ability_logs;
#[path = "scenario/issue_227_whole_hand_discard.rs"]
mod issue_227_whole_hand_discard;
#[path = "scenario/spellementals.rs"]
mod spellementals;

#[path = "scenario/issue_229_sunderflock.rs"]
mod issue_229_sunderflock;

#[path = "scenario/issue_230_winternight.rs"]
mod issue_230_winternight;

#[path = "scenario/issue_240_hand_bottom_cost.rs"]
mod issue_240_hand_bottom_cost;

#[path = "scenario/issue_241_koya.rs"]
mod issue_241_koya;
#[path = "scenario/issue_268_generated_one_shot_spells.rs"]
mod issue_268_generated_one_shot_spells;
#[path = "scenario/issue_271_generated_targeted_ordered.rs"]
mod issue_271_generated_targeted_ordered;
#[path = "scenario/issue_272_manifest_dread_equipment.rs"]
mod issue_272_manifest_dread_equipment;
#[path = "scenario/issue_276_generated_power_damage.rs"]
mod issue_276_generated_power_damage;
#[path = "scenario/issue_287_generated_recruit_etb.rs"]
mod issue_287_generated_recruit_etb;
#[path = "scenario/issue_294_direct_ron.rs"]
mod issue_294_direct_ron;
#[path = "scenario/issue_296_every_draw.rs"]
mod issue_296_every_draw;
#[path = "scenario/issue_301_generated_beginning_of_combat.rs"]
mod issue_301_generated_beginning_of_combat;
#[path = "scenario/issue_309_generated_station.rs"]
mod issue_309_generated_station;
#[path = "scenario/issue_310_generated_station.rs"]
mod issue_310_generated_station;
#[path = "scenario/issue_311_generated_station.rs"]
mod issue_311_generated_station;
#[path = "scenario/issue_313_generated_station.rs"]
mod issue_313_generated_station;
#[path = "scenario/issue_314_generated_activated_draw_two.rs"]
mod issue_314_generated_activated_draw_two;
#[path = "scenario/issue_315_generated_creature_tappers.rs"]
mod issue_315_generated_creature_tappers;
#[path = "scenario/issue_316_generated_single_clause_cohort.rs"]
mod issue_316_generated_single_clause_cohort;
#[path = "scenario/issue_317_generated_six_clause_batch.rs"]
mod issue_317_generated_six_clause_batch;
#[path = "scenario/issue_318_generated_placement_cost_trigger_batch.rs"]
mod issue_318_generated_placement_cost_trigger_batch;
#[path = "scenario/issue_323_generated_eight_clause_batch.rs"]
mod issue_323_generated_eight_clause_batch;
#[path = "scenario/issue_327_generated_eight_clause_batch.rs"]
mod issue_327_generated_eight_clause_batch;
#[path = "scenario/issue_328_generated_eight_clause_batch.rs"]
mod issue_328_generated_eight_clause_batch;
#[path = "scenario/issue_329_generated_warp_flashback.rs"]
mod issue_329_generated_warp_flashback;
#[path = "scenario/issue_333_generated_hand_landfall_mana_batch.rs"]
mod issue_333_generated_hand_landfall_mana_batch;
#[path = "scenario/issue_335_generated_eight_clause_batch.rs"]
mod issue_335_generated_eight_clause_batch;
#[path = "scenario/issue_336_generated_eight_clause_batch.rs"]
mod issue_336_generated_eight_clause_batch;
#[path = "scenario/issue_337_generated_eight_clause_batch.rs"]
mod issue_337_generated_eight_clause_batch;
#[path = "scenario/issue_338_additional_costs.rs"]
mod issue_338_additional_costs;

#[path = "scenario/issue_322_bargain.rs"]
mod issue_322_bargain;
#[path = "scenario/issue_338_direct_ron.rs"]
mod issue_338_direct_ron;
#[path = "scenario/issue_338_direct_ron_batch2.rs"]
mod issue_338_direct_ron_batch2;
#[path = "scenario/issue_338_direct_ron_batch3.rs"]
mod issue_338_direct_ron_batch3;
#[path = "scenario/issue_343_generated_eight_clause_batch.rs"]
mod issue_343_generated_eight_clause_batch;
#[path = "scenario/issue_344_generated_eight_clause_batch.rs"]
mod issue_344_generated_eight_clause_batch;
#[path = "scenario/issue_350_generated_harmonize.rs"]
mod issue_350_generated_harmonize;
#[path = "scenario/issue_351_generated_eight_clause_batch.rs"]
mod issue_351_generated_eight_clause_batch;
#[path = "scenario/issue_352_generated_eight_clause_batch.rs"]
mod issue_352_generated_eight_clause_batch;
#[path = "scenario/issue_358_generated_five_clause_batch.rs"]
mod issue_358_generated_five_clause_batch;
#[path = "scenario/issue_359_direct_ron.rs"]
mod issue_359_direct_ron;
#[path = "scenario/issue_359_direct_ron_batch2.rs"]
mod issue_359_direct_ron_batch2;
#[path = "scenario/issue_359_direct_ron_batch3.rs"]
mod issue_359_direct_ron_batch3;
#[path = "scenario/issue_360_tiered_cards.rs"]
mod issue_360_tiered_cards;
#[path = "scenario/issue_363_generated_dragonstorm_cohort.rs"]
mod issue_363_generated_dragonstorm_cohort;
#[path = "scenario/issue_370_graveyard_count_cost_reduction.rs"]
mod issue_370_graveyard_count_cost_reduction;
#[path = "scenario/issue_371_graveyard_count_tokens.rs"]
mod issue_371_graveyard_count_tokens;
#[path = "scenario/issue_372_static_count_scaled_pt.rs"]
mod issue_372_static_count_scaled_pt;
#[path = "scenario/issue_373_graveyard_count_resolution.rs"]
mod issue_373_graveyard_count_resolution;
#[path = "scenario/issue_374_graveyard_condition_triggers.rs"]
mod issue_374_graveyard_condition_triggers;
#[path = "scenario/issue_375_graveyard_return.rs"]
mod issue_375_graveyard_return;
#[path = "scenario/issue_377_graveyard_static_conditions.rs"]
mod issue_377_graveyard_static_conditions;
#[path = "scenario/issue_412_modal_modes.rs"]
mod issue_412_modal_modes;
#[path = "scenario/issue_413_cycling_reminder.rs"]
mod issue_413_cycling_reminder;
#[path = "scenario/issue_414_enters_with_minus_counters.rs"]
mod issue_414_enters_with_minus_counters;
#[path = "scenario/issue_415_clue_equipment.rs"]
mod issue_415_clue_equipment;
#[path = "scenario/issue_416_roads_entry.rs"]
mod issue_416_roads_entry;
#[path = "scenario/issue_417_self_sacrifice_mana.rs"]
mod issue_417_self_sacrifice_mana;
#[path = "scenario/issue_423_station_thresholds.rs"]
mod issue_423_station_thresholds;
#[path = "scenario/issue_424_triggered_modal_etb.rs"]
mod issue_424_triggered_modal_etb;
#[path = "scenario/issue_425_choose_one_or_both.rs"]
mod issue_425_choose_one_or_both;
#[path = "scenario/issue_426_one_mode_modal.rs"]
mod issue_426_one_mode_modal;
#[path = "scenario/issue_427_firebending.rs"]
mod issue_427_firebending;
#[path = "scenario/issue_428_choose_two.rs"]
mod issue_428_choose_two;
#[path = "scenario/issue_429_aura_restriction.rs"]
mod issue_429_aura_restriction;
#[path = "scenario/issue_430_direct_ron.rs"]
mod issue_430_direct_ron;
#[path = "scenario/issue_444_commands.rs"]
mod issue_444_commands;
#[path = "scenario/issue_452_direct_ron.rs"]
mod issue_452_direct_ron;
#[path = "scenario/issue_452_fixed_damage_family.rs"]
mod issue_452_fixed_damage_family;
#[path = "scenario/issue_453_newly_eligible_standard.rs"]
mod issue_453_newly_eligible_standard;
#[path = "scenario/issue_454_activated_surveil.rs"]
mod issue_454_activated_surveil;
#[path = "scenario/issue_455_etb_opponent_pump.rs"]
mod issue_455_etb_opponent_pump;
#[path = "scenario/issue_456_conditional_dual_lands.rs"]
mod issue_456_conditional_dual_lands;
#[path = "scenario/issue_457_power_up_exhaust.rs"]
mod issue_457_power_up_exhaust;
#[path = "scenario/issue_458_pump_and_seven_lands.rs"]
mod issue_458_pump_and_seven_lands;
#[path = "scenario/issue_459_devotee_endure_mobilize.rs"]
mod issue_459_devotee_endure_mobilize;
#[path = "scenario/issue_460_reviewed_instances.rs"]
mod issue_460_reviewed_instances;
#[path = "scenario/issue_465_token_blockers.rs"]
mod issue_465_token_blockers;
#[path = "scenario/issue_500_proliferate_cohort.rs"]
mod issue_500_proliferate_cohort;
#[path = "scenario/issue_combat_tricks_batch.rs"]
mod issue_combat_tricks_batch;
#[path = "scenario/issue_dies_batch.rs"]
mod issue_dies_batch;
#[path = "scenario/issue_dmg2_batch.rs"]
mod issue_dmg2_batch;
#[path = "scenario/issue_etb_batch.rs"]
mod issue_etb_batch;
#[path = "scenario/issue_look_batch.rs"]
mod issue_look_batch;
#[path = "scenario/issue_misc10_batch.rs"]
mod issue_misc10_batch;
#[path = "scenario/issue_misc11_batch.rs"]
mod issue_misc11_batch;
#[path = "scenario/issue_misc12_batch.rs"]
mod issue_misc12_batch;
#[path = "scenario/issue_misc13_batch.rs"]
mod issue_misc13_batch;
#[path = "scenario/issue_misc14_batch.rs"]
mod issue_misc14_batch;
#[path = "scenario/issue_misc15_batch.rs"]
mod issue_misc15_batch;
#[path = "scenario/issue_misc16_batch.rs"]
mod issue_misc16_batch;
#[path = "scenario/issue_misc17_batch.rs"]
mod issue_misc17_batch;
#[path = "scenario/issue_misc18_batch.rs"]
mod issue_misc18_batch;
#[path = "scenario/issue_misc19_batch.rs"]
mod issue_misc19_batch;
#[path = "scenario/issue_misc1_batch.rs"]
mod issue_misc1_batch;
#[path = "scenario/issue_misc20_batch.rs"]
mod issue_misc20_batch;
#[path = "scenario/issue_misc21_batch.rs"]
mod issue_misc21_batch;
#[path = "scenario/issue_misc22_batch.rs"]
mod issue_misc22_batch;
#[path = "scenario/issue_misc23_batch.rs"]
mod issue_misc23_batch;
#[path = "scenario/issue_misc24_batch.rs"]
mod issue_misc24_batch;
#[path = "scenario/issue_misc25_batch.rs"]
mod issue_misc25_batch;
#[path = "scenario/issue_misc26_batch.rs"]
mod issue_misc26_batch;
#[path = "scenario/issue_misc27_batch.rs"]
mod issue_misc27_batch;
#[path = "scenario/issue_misc28_batch.rs"]
mod issue_misc28_batch;
#[path = "scenario/issue_misc29_batch.rs"]
mod issue_misc29_batch;
#[path = "scenario/issue_misc2_batch.rs"]
mod issue_misc2_batch;
#[path = "scenario/issue_misc30_batch.rs"]
mod issue_misc30_batch;
#[path = "scenario/issue_misc31_batch.rs"]
mod issue_misc31_batch;
#[path = "scenario/issue_misc32_batch.rs"]
mod issue_misc32_batch;
#[path = "scenario/issue_misc33_batch.rs"]
mod issue_misc33_batch;
#[path = "scenario/issue_misc34_batch.rs"]
mod issue_misc34_batch;
#[path = "scenario/issue_misc35_batch.rs"]
mod issue_misc35_batch;
#[path = "scenario/issue_misc36_batch.rs"]
mod issue_misc36_batch;
#[path = "scenario/issue_misc37_batch.rs"]
mod issue_misc37_batch;
#[path = "scenario/issue_misc38_batch.rs"]
mod issue_misc38_batch;
#[path = "scenario/issue_misc39_batch.rs"]
mod issue_misc39_batch;
#[path = "scenario/issue_misc3_batch.rs"]
mod issue_misc3_batch;
#[path = "scenario/issue_misc40_batch.rs"]
mod issue_misc40_batch;
#[path = "scenario/issue_misc41_batch.rs"]
mod issue_misc41_batch;
#[path = "scenario/issue_misc42_batch.rs"]
mod issue_misc42_batch;
#[path = "scenario/issue_misc43_batch.rs"]
mod issue_misc43_batch;
#[path = "scenario/issue_misc44_batch.rs"]
mod issue_misc44_batch;
#[path = "scenario/issue_misc45_batch.rs"]
mod issue_misc45_batch;

#[path = "scenario/issue_misc46_batch.rs"]
mod issue_misc46_batch;

#[path = "scenario/issue_misc47_batch.rs"]
mod issue_misc47_batch;
#[path = "scenario/issue_misc48_batch.rs"]
mod issue_misc48_batch;
#[path = "scenario/issue_misc4_batch.rs"]
mod issue_misc4_batch;
#[path = "scenario/issue_misc5_batch.rs"]
mod issue_misc5_batch;
#[path = "scenario/issue_misc6_batch.rs"]
mod issue_misc6_batch;
#[path = "scenario/issue_misc7_batch.rs"]
mod issue_misc7_batch;
#[path = "scenario/issue_misc8_batch.rs"]
mod issue_misc8_batch;
#[path = "scenario/issue_misc9_batch.rs"]
mod issue_misc9_batch;
#[path = "scenario/issue_removal_batch.rs"]
mod issue_removal_batch;

#[path = "scenario/deck_coverage_anarchomancer_mastermind.rs"]
mod deck_coverage_anarchomancer_mastermind;

#[path = "scenario/deck_coverage_fair_steel.rs"]
mod deck_coverage_fair_steel;

#[path = "scenario/deck_coverage_artifact_tutors.rs"]
mod deck_coverage_artifact_tutors;

#[path = "scenario/deck_coverage_greater_good.rs"]
mod deck_coverage_greater_good;

#[path = "scenario/deck_coverage_flusterstorm.rs"]
mod deck_coverage_flusterstorm;

#[path = "scenario/deck_coverage_copy_aura_cards.rs"]
mod deck_coverage_copy_aura_cards;

#[path = "scenario/deck_coverage_forgemaster.rs"]
mod deck_coverage_forgemaster;

#[path = "scenario/deck_coverage_glorious_sunrise.rs"]
mod deck_coverage_glorious_sunrise;

#[path = "scenario/deck_coverage_summoning_station.rs"]
mod deck_coverage_summoning_station;

#[path = "scenario/deck_coverage_frantic_search.rs"]
mod deck_coverage_frantic_search;

#[path = "scenario/deck_coverage_wash_out.rs"]
mod deck_coverage_wash_out;

#[path = "scenario/deck_coverage_anger.rs"]
mod deck_coverage_anger;

#[path = "scenario/deck_coverage_ghalta.rs"]
mod deck_coverage_ghalta;

#[path = "scenario/deck_coverage_all_is_dust.rs"]
mod deck_coverage_all_is_dust;

#[path = "scenario/deck_coverage_deepglow_skate.rs"]
mod deck_coverage_deepglow_skate;

#[path = "scenario/deck_coverage_farewell.rs"]
mod deck_coverage_farewell;

#[path = "scenario/deck_coverage_draw_replacements.rs"]
mod deck_coverage_draw_replacements;

#[path = "scenario/deck_coverage_empty_draw_win.rs"]
mod deck_coverage_empty_draw_win;

#[path = "scenario/deck_coverage_windfall.rs"]
mod deck_coverage_windfall;

#[path = "scenario/deck_coverage_black_vise.rs"]
mod deck_coverage_black_vise;

#[path = "scenario/deck_coverage_war_room.rs"]
mod deck_coverage_war_room;

#[path = "scenario/into_the_wilds.rs"]
mod into_the_wilds;

#[path = "scenario/deck_coverage_arena.rs"]
mod deck_coverage_arena;

#[path = "scenario/deck_coverage_unnatural_growth.rs"]
mod deck_coverage_unnatural_growth;

#[path = "scenario/deck_coverage_chasm_skulker.rs"]
mod deck_coverage_chasm_skulker;
#[path = "scenario/deck_coverage_metalwork_colossus.rs"]
mod deck_coverage_metalwork_colossus;

#[path = "scenario/deck_coverage_arcane_denial.rs"]
mod deck_coverage_arcane_denial;

#[path = "scenario/deck_coverage_ascend_from_avernus.rs"]
mod deck_coverage_ascend_from_avernus;

#[path = "scenario/deck_coverage_weapons_manufacturing.rs"]
mod deck_coverage_weapons_manufacturing;

#[path = "scenario/deck_coverage_esikas_chariot.rs"]
mod deck_coverage_esikas_chariot;

#[path = "scenario/deck_coverage_valakut.rs"]
mod deck_coverage_valakut;

#[path = "scenario/deck_coverage_monument_to_endurance.rs"]
mod deck_coverage_monument_to_endurance;
