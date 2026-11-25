//! Actual-card scenarios for the queue-v81 03-untap-team-rider cohort.
//!
//! Expectations are printed Oracle values. CR 701.26 untaps each creature the spell's controller
//! controls at resolution; CR 613.4c applies the pumps in layer 7c; CR 611.2c/514.2 fix and end the
//! grants. The 2018-10-05 ruling requires already-untapped creatures to gain hexproof and
//! indestructible, and the 2019-05-03 ruling limits Rally of Wings to creatures controlled at
//! resolution.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "call_to_glory",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "rally_of_wings",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "join_shields",
            mana: ManaGift {
                w: 1,
                g: 1,
                c: 3,
                ..Default::default()
            },
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "plains",
            &[
                "grizzly_bears",
                "universal_automaton",
                "cloud_elemental",
                "cloud_elemental",
            ],
        ),
        deck_with("island", &["cloud_elemental"]),
    ]);
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

fn cast_case(e: &mut GameEngine, case: &Case) {
    give_mana(e, 0, gift(&case.mana));
    inject_card_into_hand(e, 0, case.id);
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
}

fn is_tapped(e: &GameEngine, id: u32) -> bool {
    e.state.objects[&id].tapped
}

#[test]
fn q81_untap_rider_canary_simplest_call_to_glory_untaps_and_pumps_only_samurai() {
    let case = &cases()[0];
    let mut e = engine(81_300_001);
    let samurai = move_ready_to_battlefield(&mut e, 0, "universal_automaton");
    let bears = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
    let opponent = move_ready_to_battlefield(&mut e, 1, "cloud_elemental");
    for id in [samurai, bears, opponent] {
        e.state.objects.get_mut(&id).expect("creature").tapped = true;
    }
    cast_case(&mut e, case);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "the printed cost is spent");
    assert!(!is_tapped(&e, samurai), "the Samurai is untapped");
    assert!(!is_tapped(&e, bears), "the non-Samurai is untapped");
    assert!(
        is_tapped(&e, opponent),
        "the opponent's creature stays tapped"
    );
    assert_eq!(
        (e.effective_power(samurai), e.effective_toughness(samurai)),
        (Some(2), Some(2)),
        "the changeling is a Samurai and gets +1/+1"
    );
    assert_eq!(
        (e.effective_power(bears), e.effective_toughness(bears)),
        (Some(2), Some(2)),
        "the non-Samurai is not pumped"
    );
    assert_eq!(
        (e.effective_power(opponent), e.effective_toughness(opponent)),
        (Some(2), Some(3)),
        "the opponent's creature is not pumped"
    );
}

#[test]
fn q81_untap_rider_canary_hardest_join_shields_grants_hexproof_and_indestructible() {
    let case = &cases()[2];
    let mut e = engine(81_300_002);
    let bears = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
    let automaton = move_ready_to_battlefield(&mut e, 0, "universal_automaton");
    e.state
        .objects
        .get_mut(&automaton)
        .expect("creature")
        .tapped = true;
    cast_case(&mut e, case);
    assert!(!is_tapped(&e, automaton), "the tapped creature is untapped");
    assert!(
        !is_tapped(&e, bears),
        "the already-untapped creature stays untapped"
    );
    for id in [bears, automaton] {
        assert!(
            e.effective_has_keyword(id, Keyword::Hexproof),
            "each creature gains hexproof"
        );
        assert!(
            e.effective_has_keyword(id, Keyword::Indestructible),
            "each creature gains indestructible"
        );
    }

    give_mana(
        &mut e,
        1,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 1, "murder");
    let murder = hand_index_for_card(&e, 1, "murder");
    let rejected = e.apply_scenario_command(1, &cast_spell(murder, target_object(bears)));
    assert!(
        rejected.is_err(),
        "an opponent cannot target a hexproof creature"
    );
    assert!(e.state.stack.is_empty(), "the targeted spell was not cast");

    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 2,
            c: 2,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "wrath_of_god");
    let wrath = hand_index_for_card(&e, 0, "wrath_of_god");
    e.apply_scenario_command(0, &cast_spell(wrath, vec![]))
        .expect("cast Wrath of God");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&bears].zone,
        Zone::Battlefield,
        "indestructible survives a destroy-all effect"
    );
    assert_eq!(
        e.state.objects[&automaton].zone,
        Zone::Battlefield,
        "the second indestructible creature survives"
    );
}

#[test]
fn q81_untap_rider_cards_untap_only_your_creatures() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(81_301_000 + index as u64);
        let mine = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        let other = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let opponent = move_ready_to_battlefield(&mut e, 1, "cloud_elemental");
        e.state.objects.get_mut(&other).expect("mine").tapped = true;
        e.state.objects.get_mut(&opponent).expect("theirs").tapped = true;
        cast_case(&mut e, &case);
        assert!(!is_tapped(&e, mine), "{}: untapped", case.id);
        assert!(!is_tapped(&e, other), "{}: untapped", case.id);
        assert!(
            is_tapped(&e, opponent),
            "{}: the opponent's creature stays tapped",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    }
}

#[test]
fn q81_untap_rider_rally_of_wings_pumps_only_flyers_at_resolution() {
    let case = &cases()[1];
    let mut e = engine(81_302_001);
    let flier = move_ready_to_battlefield(&mut e, 0, "cloud_elemental");
    let bears = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
    e.state.objects.get_mut(&flier).expect("flier").tapped = true;
    e.state.objects.get_mut(&bears).expect("bear").tapped = true;
    cast_case(&mut e, case);
    assert_eq!(
        (e.effective_power(flier), e.effective_toughness(flier)),
        (Some(4), Some(5)),
        "the flying creature gets +2/+2"
    );
    assert_eq!(
        (e.effective_power(bears), e.effective_toughness(bears)),
        (Some(2), Some(2)),
        "the nonflying creature is not pumped"
    );
    let later = move_ready_to_battlefield(&mut e, 0, "cloud_elemental");
    assert_eq!(
        (e.effective_power(later), e.effective_toughness(later)),
        (Some(2), Some(3)),
        "a creature that enters later is not pumped"
    );
}

#[test]
fn q81_untap_rider_join_shields_already_untapped_creatures_still_gain_keywords() {
    let case = &cases()[2];
    let mut e = engine(81_303_001);
    let bears = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
    assert!(!is_tapped(&e, bears), "the creature starts untapped");
    cast_case(&mut e, case);
    assert!(
        e.effective_has_keyword(bears, Keyword::Hexproof),
        "the 2018-10-05 ruling: already-untapped creatures still gain hexproof"
    );
    assert!(
        e.effective_has_keyword(bears, Keyword::Indestructible),
        "the 2018-10-05 ruling: already-untapped creatures still gain indestructible"
    );
}
