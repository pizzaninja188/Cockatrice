//! Actual-card scenarios for the reviewed Soothsayer Adept draw-then-discard family.
//!
//! Expectations are printed Oracle values. CR 602.1, 602.2, 602.2b and 601.2h cover
//! activation and its costs; CR 121.1 draws one card; CR 701.9b makes the affected player
//! choose the mandatory discard immediately after the draw (the saved Erratic Visionary
//! ruling confirms no action may occur between drawing and discarding).
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "bloodfire_mentor",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "erratic_visionary",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "research_assistant",
            mana: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "soothsayer_adept",
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
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

fn ready_creature(e: &mut GameEngine, id: &str) -> u32 {
    let creature = inject_creature_on_battlefield(e, 0, id);
    e.state
        .objects
        .get_mut(&creature)
        .expect("creature")
        .summoning_sick = false;
    creature
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

#[test]
fn soothsayer_adept_batch_cards_draw_then_discard_mandatorily() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(830_000 + index as u64);
        let creature = ready_creature(&mut e, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        let drawn_before = e.state.turn_history.current.player(0).cards_drawn;
        assert!(
            e.apply_scenario_command(0, &activate_ability(creature, 0, vec![]))
                .is_ok(),
            "{}: activate",
            case.id
        );
        e.apply_scenario_command(0, &pass()).expect("p0 pass");
        let batch = e
            .apply_scenario_command(1, &pass())
            .expect("p1 pass parks after the draw");
        let choice = find_resolution_choice(&batch).expect("discard choice");
        assert_eq!(choice.deciding_player_id, 0, "{}", case.id);
        assert_eq!(
            choice.choice_kind(),
            tricerules_proto::ruled::v1::ChoiceKind::HandCards,
            "{}",
            case.id
        );
        assert_eq!((choice.min, choice.max), (1, 1), "{}", case.id);
        assert_eq!(
            e.state.players[0].library.len(),
            library_before - 1,
            "{}: the draw precedes the discard choice",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before + 1,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.turn_history.current.player(0).cards_drawn,
            drawn_before + 1,
            "{}",
            case.id
        );
        assert!(
            e.state.objects[&creature].tapped,
            "{}: tap is a cost",
            case.id
        );

        let discarded = choice.candidate_object_ids[0];
        e.apply_scenario_command(0, &submit_resolution_choice(vec![discarded]))
            .expect("discard the chosen card");
        assert!(
            e.state.players[0].graveyard.contains(&discarded),
            "{}: the chosen card is discarded",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before,
            "{}: draw one then discard one",
            case.id
        );
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn soothsayer_adept_batch_cards_require_mana_and_an_untapped_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(831_000 + index as u64);
        let creature = ready_creature(&mut e, case.id);
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        let pool_before = mana_pool(&e);
        let error = e
            .apply_scenario_command(0, &activate_ability(creature, 0, vec![]))
            .expect_err("activation requires mana");
        assert!(
            matches!(error, tricerules_core::EngineError::Illegal(_)),
            "{}: {error:?}",
            case.id
        );
        assert!(!e.state.objects[&creature].tapped, "{}", case.id);
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{}", case.id);
        assert_eq!(
            e.state.players[0].library.len(),
            library_before,
            "{}",
            case.id
        );
        assert_eq!(mana_pool(&e), pool_before, "{}", case.id);

        give_mana(&mut e, 0, gift(&case.mana));
        e.apply_scenario_command(0, &activate_ability(creature, 0, vec![]))
            .expect("first activation");
        e.apply_scenario_command(0, &pass()).expect("p0 pass");
        let batch = e.apply_scenario_command(1, &pass()).expect("p1 pass");
        let choice = find_resolution_choice(&batch).expect("discard choice");
        e.apply_scenario_command(
            0,
            &submit_resolution_choice(vec![choice.candidate_object_ids[0]]),
        )
        .expect("discard");

        give_mana(&mut e, 0, gift(&case.mana));
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        let pool_before = mana_pool(&e);
        let error = e
            .apply_scenario_command(0, &activate_ability(creature, 0, vec![]))
            .expect_err("the tapped creature cannot pay the tap cost again");
        assert!(
            matches!(error, tricerules_core::EngineError::Illegal(_)),
            "{}: {error:?}",
            case.id
        );
        assert!(e.state.objects[&creature].tapped, "{}", case.id);
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{}", case.id);
        assert_eq!(
            e.state.players[0].library.len(),
            library_before,
            "{}",
            case.id
        );
        assert_eq!(mana_pool(&e), pool_before, "{}", case.id);
    }
}

fn mana_pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}
