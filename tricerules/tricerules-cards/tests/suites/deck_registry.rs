//! Registry regressions share one corpus initialization and executable.

#[path = "../common/mod.rs"]
mod common;

#[path = "../deck_coverage_ajani_steadfast.rs"]
mod deck_coverage_ajani_steadfast;
#[path = "../deck_coverage_alhammarrets_archive.rs"]
mod deck_coverage_alhammarrets_archive;
#[path = "../deck_coverage_all_is_dust.rs"]
mod deck_coverage_all_is_dust;
#[path = "../deck_coverage_anger.rs"]
mod deck_coverage_anger;
#[path = "../deck_coverage_arena.rs"]
mod deck_coverage_arena;
#[path = "../deck_coverage_ashiok_dream_render.rs"]
mod deck_coverage_ashiok_dream_render;
#[path = "../deck_coverage_beast_within.rs"]
mod deck_coverage_beast_within;
#[path = "../deck_coverage_berserkers_onslaught.rs"]
mod deck_coverage_berserkers_onslaught;
#[path = "../deck_coverage_black_vise.rs"]
mod deck_coverage_black_vise;
#[path = "../deck_coverage_boseiju.rs"]
mod deck_coverage_boseiju;
#[path = "../deck_coverage_brokers_ascendancy.rs"]
mod deck_coverage_brokers_ascendancy;
#[path = "../deck_coverage_buried_ruin.rs"]
mod deck_coverage_buried_ruin;
#[path = "../deck_coverage_carth_the_lion.rs"]
mod deck_coverage_carth_the_lion;
#[path = "../deck_coverage_chasm_skulker.rs"]
mod deck_coverage_chasm_skulker;
#[path = "../deck_coverage_clock_of_omens.rs"]
mod deck_coverage_clock_of_omens;
#[path = "../deck_coverage_codex_shredder.rs"]
mod deck_coverage_codex_shredder;
#[path = "../deck_coverage_combustible_gearhulk.rs"]
mod deck_coverage_combustible_gearhulk;
#[path = "../deck_coverage_commander_mana.rs"]
mod deck_coverage_commander_mana;
#[path = "../deck_coverage_commander_setup.rs"]
mod deck_coverage_commander_setup;
#[path = "../deck_coverage_coveted_jewel.rs"]
mod deck_coverage_coveted_jewel;
#[path = "../deck_coverage_creature_board_wipes.rs"]
mod deck_coverage_creature_board_wipes;
#[path = "../deck_coverage_daretti.rs"]
mod deck_coverage_daretti;
#[path = "../deck_coverage_deepglow_skate.rs"]
mod deck_coverage_deepglow_skate;
#[path = "../deck_coverage_devotion_gods.rs"]
mod deck_coverage_devotion_gods;
#[path = "../deck_coverage_draw_replacements.rs"]
mod deck_coverage_draw_replacements;
#[path = "../deck_coverage_duplicant.rs"]
mod deck_coverage_duplicant;
#[path = "../deck_coverage_eerie_ultimatum.rs"]
mod deck_coverage_eerie_ultimatum;
#[path = "../deck_coverage_everflowing_chalice.rs"]
mod deck_coverage_everflowing_chalice;
#[path = "../deck_coverage_farewell.rs"]
mod deck_coverage_farewell;
#[path = "../deck_coverage_fire_lit_thicket.rs"]
mod deck_coverage_fire_lit_thicket;
#[path = "../deck_coverage_flameshadow_conjuring.rs"]
mod deck_coverage_flameshadow_conjuring;
#[path = "../deck_coverage_frantic_search.rs"]
mod deck_coverage_frantic_search;
#[path = "../deck_coverage_garruk_cursed_huntsman.rs"]
mod deck_coverage_garruk_cursed_huntsman;
#[path = "../deck_coverage_garruks_packleader.rs"]
mod deck_coverage_garruks_packleader;
#[path = "../deck_coverage_ghalta.rs"]
mod deck_coverage_ghalta;
#[path = "../deck_coverage_gratuitous_violence.rs"]
mod deck_coverage_gratuitous_violence;
#[path = "../deck_coverage_graveyard_return.rs"]
mod deck_coverage_graveyard_return;
#[path = "../deck_coverage_gruul_war_chant.rs"]
mod deck_coverage_gruul_war_chant;
#[path = "../deck_coverage_ichor_wellspring.rs"]
mod deck_coverage_ichor_wellspring;
#[path = "../deck_coverage_jace_unraveler_of_secrets.rs"]
mod deck_coverage_jace_unraveler_of_secrets;
#[path = "../deck_coverage_khalni_ambush.rs"]
mod deck_coverage_khalni_ambush;
#[path = "../deck_coverage_koya.rs"]
mod deck_coverage_koya;
#[path = "../deck_coverage_krark_clan_ironworks.rs"]
mod deck_coverage_krark_clan_ironworks;
#[path = "../deck_coverage_land_type_pair.rs"]
mod deck_coverage_land_type_pair;
#[path = "../deck_coverage_liquimetal_torque.rs"]
mod deck_coverage_liquimetal_torque;
#[path = "../deck_coverage_lotus_petal.rs"]
mod deck_coverage_lotus_petal;
#[path = "../deck_coverage_manifold_key.rs"]
mod deck_coverage_manifold_key;
#[path = "../deck_coverage_memory_jar.rs"]
mod deck_coverage_memory_jar;
#[path = "../deck_coverage_oath_of_teferi.rs"]
mod deck_coverage_oath_of_teferi;
#[path = "../deck_coverage_onakke_oathkeeper.rs"]
mod deck_coverage_onakke_oathkeeper;
#[path = "../deck_coverage_teferi_master_of_time.rs"]
mod deck_coverage_teferi_master_of_time;
#[path = "../deck_coverage_teferi_temporal_archmage.rs"]
mod deck_coverage_teferi_temporal_archmage;
#[path = "../deck_coverage_teferi_temporal_archmage_minus_one.rs"]
mod deck_coverage_teferi_temporal_archmage_minus_one;
#[path = "../deck_coverage_the_chain_veil.rs"]
mod deck_coverage_the_chain_veil;
#[path = "../deck_coverage_vorinclex_monstrous_raider.rs"]
mod deck_coverage_vorinclex_monstrous_raider;

#[path = "../deck_coverage_aetheric_amplifier.rs"]
mod deck_coverage_aetheric_amplifier;
#[path = "../deck_coverage_innkeepers_talent.rs"]
mod deck_coverage_innkeepers_talent;
#[path = "../deck_coverage_metalwork_colossus.rs"]
mod deck_coverage_metalwork_colossus;
#[path = "../deck_coverage_mh3_mdfcs_registry.rs"]
mod deck_coverage_mh3_mdfcs_registry;
#[path = "../deck_coverage_mimic_vat.rs"]
mod deck_coverage_mimic_vat;
#[path = "../deck_coverage_mind_stone.rs"]
mod deck_coverage_mind_stone;
#[path = "../deck_coverage_mirrorworks.rs"]
mod deck_coverage_mirrorworks;
#[path = "../deck_coverage_mithril_coat.rs"]
mod deck_coverage_mithril_coat;
#[path = "../deck_coverage_molten_echoes.rs"]
mod deck_coverage_molten_echoes;
#[path = "../deck_coverage_mycosynth_gardens.rs"]
mod deck_coverage_mycosynth_gardens;
#[path = "../deck_coverage_mycosynth_lattice.rs"]
mod deck_coverage_mycosynth_lattice;
#[path = "../deck_coverage_myr_battlesphere.rs"]
mod deck_coverage_myr_battlesphere;
#[path = "../deck_coverage_myr_retriever.rs"]
mod deck_coverage_myr_retriever;
#[path = "../deck_coverage_natures_will.rs"]
mod deck_coverage_natures_will;
#[path = "../deck_coverage_nevinyrrals_disk.rs"]
mod deck_coverage_nevinyrrals_disk;
#[path = "../deck_coverage_ohran_frostfang.rs"]
mod deck_coverage_ohran_frostfang;
#[path = "../deck_coverage_ominous_seas.rs"]
mod deck_coverage_ominous_seas;
#[path = "../deck_coverage_pongify.rs"]
mod deck_coverage_pongify;
#[path = "../deck_coverage_prized_statue.rs"]
mod deck_coverage_prized_statue;
#[path = "../deck_coverage_prosperity.rs"]
mod deck_coverage_prosperity;
#[path = "../deck_coverage_quicksmith_genius.rs"]
mod deck_coverage_quicksmith_genius;
#[path = "../deck_coverage_rangers_guile.rs"]
mod deck_coverage_rangers_guile;
#[path = "../deck_coverage_rings_of_brighthearth.rs"]
mod deck_coverage_rings_of_brighthearth;
#[path = "../deck_coverage_sakura_tribe_elder.rs"]
mod deck_coverage_sakura_tribe_elder;
#[path = "../deck_coverage_scavenger_grounds.rs"]
mod deck_coverage_scavenger_grounds;
#[path = "../deck_coverage_scrap_trawler.rs"]
mod deck_coverage_scrap_trawler;
#[path = "../deck_coverage_search_lands.rs"]
mod deck_coverage_search_lands;
#[path = "../deck_coverage_solemn_simulacrum.rs"]
mod deck_coverage_solemn_simulacrum;
#[path = "../deck_coverage_swan_song.rs"]
mod deck_coverage_swan_song;
#[path = "../deck_coverage_sword_of_war_and_peace.rs"]
mod deck_coverage_sword_of_war_and_peace;
#[path = "../deck_coverage_unnatural_growth.rs"]
mod deck_coverage_unnatural_growth;
#[path = "../deck_coverage_urzas_cave.rs"]
mod deck_coverage_urzas_cave;
#[path = "../deck_coverage_voltaic_key.rs"]
mod deck_coverage_voltaic_key;
#[path = "../deck_coverage_wash_out.rs"]
mod deck_coverage_wash_out;
#[path = "../deck_coverage_waterlogged_grove.rs"]
mod deck_coverage_waterlogged_grove;
#[path = "../deck_coverage_wildsear.rs"]
mod deck_coverage_wildsear;
#[path = "../deck_coverage_wizard_class.rs"]
mod deck_coverage_wizard_class;
#[path = "../deck_coverage_words_of_wisdom.rs"]
mod deck_coverage_words_of_wisdom;
#[path = "../deck_coverage_zurs_weirding.rs"]
mod deck_coverage_zurs_weirding;
#[path = "../issue_507_replicating_ring_program.rs"]
mod issue_507_replicating_ring_program;
#[path = "../structural_choose_later_ports.rs"]
mod structural_choose_later_ports;
