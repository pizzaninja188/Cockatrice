//! Issue #454 — the activated mana-and-tap Surveil 1 family through the command boundary.
//!
//! Every expectation is the reviewed printed Oracle behavior, not a copy of generator output.
//! Exact Scryfall records and `rulings_uri` responses were fetched 2026-09-19 against pinned
//! snapshot `27bf3214-1271-490b-bdfe-c0be6c23d02e`; none returned a ruling. Governing CR
//! concepts: CR 701.25 (surveil), CR 602.2b (activated-ability costs), CR 605 (mana abilities),
//! CR 614.1c (enters tapped), and CR 601.2h (paying a cost before the ability resolves).

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};

/// `(w, u, b, r, g, c)`; a named alias keeps the reviewed fixtures readable.
type ManaOption = (u32, u32, u32, u32, u32, u32);

struct SurveilCase {
    card: &'static str,
    ability_index: u32,
    is_land: bool,
    mana: ManaOption,
}

fn gift((w, u, b, r, g, c): ManaOption) -> ManaGift {
    ManaGift { w, u, b, r, g, c }
}

fn surveil_cases() -> [SurveilCase; 3] {
    [
        SurveilCase {
            card: "titans_grave",
            ability_index: 1,
            is_land: true,
            mana: (0, 0, 1, 0, 1, 2),
        },
        SurveilCase {
            card: "wretched_doll",
            ability_index: 0,
            is_land: false,
            mana: (0, 0, 1, 0, 0, 0),
        },
        SurveilCase {
            card: "tocasias_dig_site",
            ability_index: 1,
            is_land: true,
            mana: (0, 0, 0, 0, 0, 3),
        },
    ]
}

fn land_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn place_source(engine: &mut GameEngine, case: &SurveilCase) -> u32 {
    if case.is_land {
        inject_permanent_on_battlefield(engine, 0, case.card)
    } else {
        inject_creature_on_battlefield(engine, 0, case.card)
    }
}

fn activate_mana_option(
    engine: &GameEngine,
    permanent: u32,
    ability_index: u32,
    option_index: u32,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: permanent,
            ability_index,
            mana_option_index: option_index,
            expected_zone_change_generation: engine
                .state
                .zone_change_generation
                .get(&permanent)
                .copied()
                .unwrap_or(0),
            ..Default::default()
        })),
    }
}

/// Put `card_ids` on top of `player`'s library, first entry on top, and return their OIDs.
fn seat_on_top(engine: &mut GameEngine, player: usize, card_ids: &[&str]) -> Vec<u32> {
    let oids: Vec<u32> = card_ids
        .iter()
        .map(|card_id| inject_library_card(engine, player, card_id))
        .collect();
    engine.state.players[player]
        .library
        .retain(|oid| !oids.contains(oid));
    for &oid in oids.iter().rev() {
        engine.state.players[player].library.push_front(oid);
    }
    oids
}

#[test]
fn issue_454_activations_tap_the_source_and_partition_the_controller_library() {
    for (index, case) in surveil_cases().into_iter().enumerate() {
        for (path_index, keep) in [false, true].into_iter().enumerate() {
            let mut engine = land_engine(454_100 + (index * 2 + path_index) as u64);
            let source = place_source(&mut engine, &case);
            // A distinct opponent top card proves the surveil touches only the controller's library.
            inject_library_card(&mut engine, 1, "hill_giant");
            let opposing_library: Vec<u32> =
                engine.state.players[1].library.iter().copied().collect();

            let top = seat_on_top(&mut engine, 0, &["grizzly_bears", "storm_crow"]);
            let hand_before: Vec<u32> = engine.state.players[0].hand.to_vec();
            let library_before: Vec<u32> =
                engine.state.players[0].library.iter().copied().collect();
            give_mana(&mut engine, 0, gift(case.mana));

            let activation = activate_ability_for(&engine, source, case.ability_index, vec![]);
            semantic::accepted(&mut engine, 0, &activation);
            assert!(
                engine.state.objects[&source].tapped,
                "{} taps as part of the cost",
                case.card
            );

            // Resolve the ability and park the private surveil choice.
            let first = engine.state.priority_player_id();
            semantic::accepted(&mut engine, first, &pass());
            let second = engine.state.priority_player_id();
            let parked = semantic::accepted(&mut engine, second, &pass());
            let choice = find_resolution_choice(&parked).expect("surveil choice");
            assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
            assert_eq!(choice.deciding_player_id, 0);
            assert_eq!(choice.candidate_object_ids, vec![top[0]]);
            assert!(choice.public_reveal.is_none(), "surveil stays private");

            let chosen = if keep { vec![] } else { vec![top[0]] };
            semantic::complete(&mut engine, 4, |_| {
                Some((0, submit_resolution_choice(chosen.clone())))
            })
            .require_exercised();

            let mut expected_library = library_before.clone();
            if keep {
                assert_eq!(engine.state.objects[&top[0]].zone, Zone::Library);
                assert_eq!(engine.state.players[0].library.front(), Some(&top[0]));
                assert!(!engine.state.players[0].graveyard.contains(&top[0]));
            } else {
                assert_eq!(engine.state.objects[&top[0]].zone, Zone::Graveyard);
                assert!(engine.state.players[0].graveyard.contains(&top[0]));
                expected_library.remove(0);
            }
            assert_eq!(
                engine.state.players[0]
                    .library
                    .iter()
                    .copied()
                    .collect::<Vec<_>>(),
                expected_library,
                "{} exact controller library after the partition",
                case.card
            );
            assert_eq!(
                engine.state.players[0].hand, hand_before,
                "{} surveil never moves a card to hand",
                case.card
            );
            assert_eq!(
                engine.state.players[1]
                    .library
                    .iter()
                    .copied()
                    .collect::<Vec<_>>(),
                opposing_library,
                "{} leaves the opponent library untouched",
                case.card
            );
            assert_eq!(engine.state.players[0].life, 20, "{}", case.card);
            assert!(engine.state.objects[&source].tapped, "{}", case.card);
        }
    }
}

#[test]
fn issue_454_activation_is_rejected_without_mana_and_when_already_tapped() {
    for (index, case) in surveil_cases().into_iter().enumerate() {
        // CR 601.2h: the exact mana cost is checked before the ability is activated.
        let mut no_mana = land_engine(454_200 + index as u64);
        let source = place_source(&mut no_mana, &case);
        let top = seat_on_top(&mut no_mana, 0, &["grizzly_bears"]);
        let before = no_mana.state.command_index;
        let activation = activate_ability_for(&no_mana, source, case.ability_index, vec![]);
        no_mana
            .apply_command(0, &activation)
            .expect_err("the exact mana cost must be paid");
        assert_eq!(no_mana.state.command_index, before);
        assert!(!no_mana.state.objects[&source].tapped);
        assert!(no_mana.state.stack.is_empty());
        assert_eq!(no_mana.state.players[0].library.front(), Some(&top[0]));

        // CR 602.2b: an already-tapped source cannot pay the tap cost.
        let mut tapped = land_engine(454_300 + index as u64);
        let source = place_source(&mut tapped, &case);
        seat_on_top(&mut tapped, 0, &["grizzly_bears"]);
        give_mana(&mut tapped, 0, gift(case.mana));
        tapped
            .state
            .objects
            .get_mut(&source)
            .expect("source")
            .tapped = true;
        let before = tapped.state.command_index;
        let activation = activate_ability_for(&tapped, source, case.ability_index, vec![]);
        tapped
            .apply_command(0, &activation)
            .expect_err("a tapped source cannot pay the tap cost");
        assert_eq!(tapped.state.command_index, before);
        assert!(tapped.state.stack.is_empty());
    }
}

const GUILD_MANA: [(&str, [ManaOption; 2]); 5] = [
    ("titans_grave", [(0, 0, 1, 0, 0, 0), (0, 0, 0, 0, 1, 0)]),
    ("paradox_gardens", [(0, 0, 0, 0, 1, 0), (0, 1, 0, 0, 0, 0)]),
    ("fields_of_strife", [(0, 0, 0, 1, 0, 0), (1, 0, 0, 0, 0, 0)]),
    ("spectacle_summit", [(0, 1, 0, 0, 0, 0), (0, 0, 0, 1, 0, 0)]),
    ("forum_of_amity", [(1, 0, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)]),
];

#[test]
fn issue_454_guild_lands_enter_tapped_and_produce_both_colors() {
    for (index, (card_id, options)) in GUILD_MANA.into_iter().enumerate() {
        for (option_index, expected) in options.into_iter().enumerate() {
            let mut engine = land_engine(454_400 + (index * 2 + option_index) as u64);
            let source = inject_card_into_hand(&mut engine, 0, card_id);
            let slot = hand_index_for_card(&engine, 0, card_id);
            semantic::accepted(&mut engine, 0, &play_land(slot));
            let land = battlefield_object_for_card(&engine, 0, card_id);
            assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
            assert!(
                engine.state.objects[&land].tapped,
                "{card_id} enters tapped"
            );

            // Untap through the fixture so the printed mana ability is a real command.
            engine.state.objects.get_mut(&land).expect("land").tapped = false;
            let activation = activate_mana_option(&engine, land, 0, option_index as u32);
            semantic::accepted(&mut engine, 0, &activation);
            let pool = &engine.state.players[0].mana_pool;
            assert_eq!(
                (
                    pool.white,
                    pool.blue,
                    pool.black,
                    pool.red,
                    pool.green,
                    pool.colorless
                ),
                expected,
                "{card_id} option {option_index}"
            );
            assert!(
                engine.state.objects[&land].tapped,
                "{card_id} tapping is the mana cost"
            );
        }
    }
}

#[test]
fn issue_454_tocasia_dig_site_produces_colorless_without_entering_tapped() {
    let mut engine = land_engine(454_500);
    let source = inject_card_into_hand(&mut engine, 0, "tocasias_dig_site");
    let slot = hand_index_for_card(&engine, 0, "tocasias_dig_site");
    semantic::accepted(&mut engine, 0, &play_land(slot));
    let land = battlefield_object_for_card(&engine, 0, "tocasias_dig_site");
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert!(
        !engine.state.objects[&land].tapped,
        "Tocasia's Dig Site does not enter tapped"
    );
    let activation = activate_mana_option(&engine, land, 0, 0);
    semantic::accepted(&mut engine, 0, &activation);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
    assert!(engine.state.objects[&land].tapped);
}
