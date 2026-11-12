//! Actual-card scenarios for the reviewed Viashino Racketeer optional-discard family.
//!
//! Expectations are printed Oracle values. CR 603.6a puts the entry trigger on the
//! stack (CR 603.6a/603.3); CR 701.9a moves the chosen card from hand to its owner's
//! graveyard; CR 608.2c resolves the optional instruction; the saved Fissure Wizard ruling
//! caps the optional discard at one card; CR 121.1 draws only when the discard happened.
use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "fissure_wizard",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "immersturm_raider",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "viashino_racketeer",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]);
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

fn enter(e: &mut GameEngine, case: &Case) -> (RuledEventBatch, usize, usize) {
    give_mana(e, 0, gift(&case.mana));
    let slot = inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("{}: cast: {error:?}", case.id));
    let mut batch = pass_both_players_with_batch(e);
    if find_resolution_choice(&batch).is_none() {
        e.apply_scenario_command(0, &pass()).expect("caster pass");
        batch = e
            .apply_scenario_command(1, &pass())
            .expect("trigger resolves");
    }
    let library_before = e.state.players[0].library.len();
    let hand_before = e.state.players[0].hand.len();
    assert_eq!(
        e.state.objects[&slot].zone,
        Zone::Battlefield,
        "{}",
        case.id
    );
    (batch, library_before, hand_before)
}

#[test]
fn viashino_racketeer_batch_cards_decline_does_not_draw() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(780_000 + index as u64);
        let (batch, library_before, hand_before) = enter(&mut e, &case);
        let choice = find_resolution_choice(&batch).expect("optional discard choice");
        e.apply_scenario_command(0, &submit_resolution_choice(vec![]))
            .expect("decline");
        assert_eq!(
            e.state.players[0].library.len(),
            library_before,
            "{}",
            case.id
        );
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{}", case.id);
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
        assert_eq!(choice.deciding_player_id, 0, "{}", case.id);
    }
}

#[test]
fn viashino_racketeer_batch_cards_discard_then_draws() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(781_000 + index as u64);
        let (batch, library_before, hand_before) = enter(&mut e, &case);
        let choice = find_resolution_choice(&batch).expect("optional discard choice");
        let discarded = choice.candidate_object_ids[0];
        e.apply_scenario_command(0, &submit_resolution_choice(vec![discarded]))
            .expect("discard then draw");
        assert_eq!(
            e.state.players[0].library.len(),
            library_before - 1,
            "{}",
            case.id
        );
        assert!(
            e.state.players[0].graveyard.contains(&discarded),
            "{}",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before,
            "{}: discard one then draw one",
            case.id
        );
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}

#[test]
fn viashino_racketeer_batch_cards_offer_only_one_discard() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(782_000 + index as u64);
        let (batch, _, _) = enter(&mut e, &case);
        let choice = find_resolution_choice(&batch).expect("optional discard choice");
        assert_eq!((choice.min, choice.max), (0, 1), "{}", case.id);
        assert!(!choice.candidate_object_ids.is_empty(), "{}", case.id);
    }
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
