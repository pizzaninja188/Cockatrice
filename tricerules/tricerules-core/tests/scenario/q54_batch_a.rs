//! Actual-card scenarios for the queue-v54 batch A cohorts.
//!
//! Expectations are printed Oracle values. CR 400.2/404.1 govern public graveyard targets,
//! CR 701.6 countering a spell, CR 701.22 scry, CR 121.1 drawing, CR 701.9 discarding,
//! CR 602.2 activated abilities, CR 608.2b stale targets, CR 611.2c/613.4c fixed-set pumps and
//! CR 514.2 until-end-of-turn expiry.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::TargetRef;

fn gift(m: &ManaGift) -> ManaGift {
    ManaGift {
        w: m.w,
        u: m.u,
        b: m.b,
        r: m.r,
        g: m.g,
        c: m.c,
    }
}

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn engine(seed: u64, p0: &[&str], p1: &[&str]) -> GameEngine {
    let decks = Some(vec![deck_with("island", p0), deck_with("forest", p1)]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

// ---------------------------------------------------------------------------
// 01-gy-to-library-bottom

#[test]
fn q54_canary_simplest_epitaph_golem_moves_own_graveyard_card_to_bottom() {
    let mut e = engine(54_010_001, &["epitaph_golem"], &["grizzly_bears"]);
    let golem = relocate_to_battlefield(&mut e, 0, "epitaph_golem", false);
    let doomed = inject_graveyard_card(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut e, 0, golem, 0, target_object(doomed)).expect("activate Epitaph Golem");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact {{2}} cost");
    assert_eq!(e.state.objects[&doomed].zone, Zone::Library);
    assert_eq!(
        e.state.players[0].library.back().copied(),
        Some(doomed),
        "the chosen card is on the bottom"
    );
    assert_eq!(count_card_id_in_graveyard(&e, 0, "grizzly_bears"), 0);
    assert!(
        !e.state.objects[&golem].tapped,
        "Epitaph Golem has no {{T}} cost"
    );
}

#[test]
fn q54_canary_hardest_cogwork_archivist_taps_and_moves_any_graveyard_card() {
    let mut e = engine(54_010_002, &["cogwork_archivist"], &["grizzly_bears"]);
    let archivist = relocate_to_battlefield(&mut e, 0, "cogwork_archivist", false);
    let doomed = inject_graveyard_card(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut e, 0, archivist, 0, target_object(doomed))
        .expect("activate Cogwork Archivist");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact {{2}} cost");
    assert!(
        e.state.objects[&archivist].tapped,
        "the {{T}} cost was paid"
    );
    assert_eq!(
        e.state.players[1].library.back().copied(),
        Some(doomed),
        "another player's card moves to its owner's bottom"
    );
    assert_eq!(count_card_id_in_graveyard(&e, 1, "grizzly_bears"), 0);
}

#[test]
fn q54_phyrexian_archivist_moves_any_graveyard_card_to_the_owners_bottom() {
    let mut e = engine(54_010_003, &["phyrexian_archivist"], &["grizzly_bears"]);
    let archivist = relocate_to_battlefield(&mut e, 0, "phyrexian_archivist", false);
    let doomed = inject_graveyard_card(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut e, 0, archivist, 0, target_object(doomed))
        .expect("activate Phyrexian Archivist");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact {{2}} cost");
    assert!(
        e.state.objects[&archivist].tapped,
        "the {{T}} cost was paid"
    );
    assert_eq!(
        e.state.players[1].library.back().copied(),
        Some(doomed),
        "the card returns to its owner's library bottom"
    );
}

#[test]
fn q54_epitaph_golem_rejects_another_players_graveyard() {
    let mut e = engine(54_011_001, &["epitaph_golem"], &["grizzly_bears"]);
    let golem = relocate_to_battlefield(&mut e, 0, "epitaph_golem", false);
    let foreign = inject_graveyard_card(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    assert!(
        apply_ability(&mut e, 0, golem, 0, target_object(foreign)).is_err(),
        "Epitaph Golem targets only its controller's graveyard"
    );
    assert_eq!(
        pool(&e),
        (0, 0, 0, 0, 0, 2),
        "rejected activation keeps mana"
    );
    assert_eq!(count_card_id_in_graveyard(&e, 1, "grizzly_bears"), 1);
    assert!(!e.state.objects[&golem].tapped);
}

#[test]
fn q54_second_archivist_fizzles_when_the_shared_graveyard_target_is_gone() {
    let mut e = engine(
        54_012_001,
        &["cogwork_archivist", "phyrexian_archivist"],
        &["grizzly_bears"],
    );
    let cogwork = relocate_to_battlefield(&mut e, 0, "cogwork_archivist", false);
    let phyrexian = relocate_to_battlefield(&mut e, 0, "phyrexian_archivist", false);
    let doomed = inject_graveyard_card(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    apply_ability(&mut e, 0, cogwork, 0, target_object(doomed)).expect("first activation");
    apply_ability(&mut e, 0, phyrexian, 0, target_object(doomed)).expect("second activation");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "both costs were paid");
    assert_eq!(
        e.state.players[1].library.back().copied(),
        Some(doomed),
        "the card moved exactly once"
    );
    assert_eq!(e.state.objects[&doomed].zone, Zone::Library);
}

// ---------------------------------------------------------------------------
// 02-counter-spell-riders

fn bolt_at_opponent(e: &mut GameEngine) -> u32 {
    inject_card_into_hand(e, 0, "lightning_bolt");
    give_mana(
        e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, 0, "lightning_bolt");
    e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
        .expect("cast Lightning Bolt");
    e.apply_scenario_command(0, &pass())
        .expect("pass priority to the opponent");
    e.state.stack.last().expect("bolt on stack").id
}

fn counter_bolt(e: &mut GameEngine, card: &str, mana: ManaGift) -> u32 {
    inject_card_into_hand(e, 1, card);
    give_mana(e, 1, mana);
    let slot = hand_index_for_card(e, 1, card);
    let bolt = e.state.stack.last().expect("bolt on stack").id;
    e.apply_scenario_command(
        1,
        &cast_spell(
            slot,
            vec![TargetRef {
                object_id: bolt,
                ..Default::default()
            }],
        ),
    )
    .unwrap_or_else(|error| panic!("cast {card}: {error:?}"));
    bolt
}

#[test]
fn q54_canary_simplest_mental_misstep_counters_a_one_mana_spell() {
    let mut e = engine(54_020_001, &["lightning_bolt"], &["mental_misstep"]);
    let bolt = bolt_at_opponent(&mut e);
    counter_bolt(
        &mut e,
        "mental_misstep",
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let batch = {
        e.apply_scenario_command(1, &pass())
            .expect("counter player passes");
        e.apply_scenario_command(0, &pass()).expect("resolve")
    };
    assert!(find_resolution_choice(&batch).is_none(), "no rider choice");
    assert_eq!(e.state.objects[&bolt].zone, Zone::Graveyard);
    assert_eq!(e.state.players[1].life, 20, "the bolt never resolves");
    assert_eq!(e.state.players[0].life, 20);
}

#[test]
fn q54_canary_hardest_failed_inspection_counters_then_loots() {
    let mut e = engine(54_020_002, &["lightning_bolt"], &["failed_inspection"]);
    let bolt = bolt_at_opponent(&mut e);
    let library_before_cast = e.state.players[1].library.len();
    counter_bolt(
        &mut e,
        "failed_inspection",
        ManaGift {
            u: 2,
            c: 2,
            ..Default::default()
        },
    );
    e.apply_scenario_command(1, &pass())
        .expect("counter player passes");
    let batch = e
        .apply_scenario_command(0, &pass())
        .expect("resolve to the loot");
    let request = find_resolution_choice(&batch).expect("loot parks for a discard choice");
    assert_eq!(request.min, 1);
    assert_eq!(request.max, 1);
    assert_eq!(e.state.objects[&bolt].zone, Zone::Graveyard);
    assert_eq!(e.state.players[1].life, 20);
    let hand_before_discard = e.state.players[1].hand.len();
    assert_eq!(
        e.state.players[1].library.len(),
        library_before_cast - 1,
        "the rider drew one card before the discard choice"
    );
    let discarded = e.state.players[1].hand[0];
    e.apply_scenario_command(1, &submit_resolution_choice(vec![discarded]))
        .expect("discard the drawn card");
    assert_eq!(
        e.state.players[1].hand.len(),
        hand_before_discard - 1,
        "draw one, then discard one"
    );
    assert_eq!(e.state.objects[&discarded].zone, Zone::Graveyard);
}

#[test]
fn q54_dissolve_counters_then_scries_one() {
    let mut e = engine(54_020_003, &["lightning_bolt"], &["dissolve"]);
    let bolt = bolt_at_opponent(&mut e);
    counter_bolt(
        &mut e,
        "dissolve",
        ManaGift {
            u: 2,
            c: 1,
            ..Default::default()
        },
    );
    let top = e.state.players[1].library[0];
    e.apply_scenario_command(1, &pass())
        .expect("counter player passes");
    let batch = e
        .apply_scenario_command(0, &pass())
        .expect("resolve to the scry");
    let request = find_resolution_choice(&batch).expect("scry parks for a choice");
    assert_eq!(
        request.choice_kind(),
        tricerules_proto::ruled::v1::ChoiceKind::LibraryTop
    );
    assert_eq!((request.min, request.max), (0, 1), "scry 1");
    assert_eq!(e.state.objects[&bolt].zone, Zone::Graveyard);
    e.apply_scenario_command(1, &submit_resolution_choice(vec![top]))
        .expect("put the looked-at card on the bottom");
    assert_eq!(
        e.state.players[1].library.back().copied(),
        Some(top),
        "the scried card moved to the bottom"
    );
}

#[test]
fn q54_mental_misstep_rejects_a_two_mana_spell() {
    let mut e = engine(54_021_001, &["grizzly_bears"], &["mental_misstep"]);
    inject_card_into_hand(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast Grizzly Bears");
    e.apply_scenario_command(0, &pass()).expect("pass priority");
    let bears = e.state.stack.last().expect("bears on stack").id;
    inject_card_into_hand(&mut e, 1, "mental_misstep");
    give_mana(
        &mut e,
        1,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let counter_slot = hand_index_for_card(&e, 1, "mental_misstep");
    let before = e.state.players[1].hand.len();
    assert!(
        e.apply_scenario_command(
            1,
            &cast_spell(
                counter_slot,
                vec![TargetRef {
                    object_id: bears,
                    ..Default::default()
                }]
            )
        )
        .is_err(),
        "mana value 2 is not a legal Mental Misstep target"
    );
    assert_eq!(
        e.state.players[1].hand.len(),
        before,
        "rejected cast keeps the card"
    );
    assert_eq!(
        e.state.players[1].mana_pool.blue, 1,
        "rejected cast keeps mana"
    );
}

#[test]
fn q54_dissolve_rider_does_not_happen_when_the_counter_is_countered() {
    let mut e = engine(
        54_022_001,
        &["lightning_bolt", "counterspell"],
        &["dissolve"],
    );
    let bolt = bolt_at_opponent(&mut e);
    counter_bolt(
        &mut e,
        "dissolve",
        ManaGift {
            u: 2,
            c: 1,
            ..Default::default()
        },
    );
    // P0 answers the Dissolve with Counterspell before it resolves.
    inject_card_into_hand(&mut e, 0, "counterspell");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    e.apply_scenario_command(1, &pass())
        .expect("counter player passes priority to the active player");
    let dissolve = e.state.stack.last().expect("dissolve on stack").id;
    let slot = hand_index_for_card(&e, 0, "counterspell");
    e.apply_scenario_command(
        0,
        &cast_spell(
            slot,
            vec![TargetRef {
                object_id: dissolve,
                ..Default::default()
            }],
        ),
    )
    .expect("counter the Dissolve");
    let library_before = e.state.players[1].library.clone();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&bolt].zone,
        Zone::Graveyard,
        "the bolt resolved"
    );
    assert_eq!(e.state.players[1].life, 17, "Lightning Bolt dealt 3");
    assert_eq!(
        e.state.players[1].library, library_before,
        "a countered Dissolve never scries"
    );
}

// ---------------------------------------------------------------------------
// 03-team-pump-activated

fn pump_case(seed: u64, card: &str, mana: ManaGift, power: u32, toughness: u32) {
    let mut e = engine(seed, &[card], &["grizzly_bears"]);
    let source = relocate_to_battlefield(&mut e, 0, card, false);
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let other = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(&mut e, 0, gift(&mana));
    apply_ability(&mut e, 0, source, 0, vec![])
        .unwrap_or_else(|error| panic!("activate {card}: {error:?}"));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{card}: exact cost");
    assert_eq!(e.effective_power(own), Some(2 + power), "{card}: own power");
    assert_eq!(
        e.effective_toughness(own),
        Some(2 + toughness),
        "{card}: own toughness"
    );
    assert_eq!(
        e.effective_power(other),
        Some(2),
        "{card}: opponent's creature is unaffected"
    );
    assert_eq!(e.effective_toughness(other), Some(2));
}

#[test]
fn q54_canary_simplest_gerrards_battle_cry_pumps_only_your_creatures() {
    pump_case(
        54_030_001,
        "gerrards_battle_cry",
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
        1,
        1,
    );
}

#[test]
fn q54_canary_hardest_screeching_phoenix_pumps_the_team() {
    pump_case(
        54_030_002,
        "screeching_phoenix",
        ManaGift {
            r: 1,
            c: 2,
            ..Default::default()
        },
        1,
        0,
    );
}

#[test]
fn q54_team_pump_cards_apply_their_printed_offsets() {
    pump_case(
        54_031_001,
        "leonin_sun_standard",
        ManaGift {
            w: 1,
            c: 1,
            ..Default::default()
        },
        1,
        1,
    );
    pump_case(
        54_031_002,
        "lavafume_invoker",
        ManaGift {
            c: 8,
            ..Default::default()
        },
        3,
        0,
    );
}

#[test]
fn q54_team_pump_rejects_activation_without_the_printed_mana() {
    let mut e = engine(54_032_001, &["gerrards_battle_cry"], &["grizzly_bears"]);
    let source = relocate_to_battlefield(&mut e, 0, "gerrards_battle_cry", false);
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    assert!(apply_ability(&mut e, 0, source, 0, vec![]).is_err());
    assert_eq!(e.effective_power(own), Some(2));
    assert!(!e.state.objects[&source].tapped);
}

#[test]
fn q54_team_pump_expires_at_cleanup() {
    let mut e = engine(54_033_001, &["gerrards_battle_cry"], &["grizzly_bears"]);
    let source = relocate_to_battlefield(&mut e, 0, "gerrards_battle_cry", false);
    let own = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut e, 0, source, 0, vec![]).expect("activate");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(own), Some(3));
    end_active_turn(&mut e, 0);
    assert_eq!(e.effective_power(own), Some(2), "the pump expired");
    assert_eq!(e.effective_toughness(own), Some(2));
}
