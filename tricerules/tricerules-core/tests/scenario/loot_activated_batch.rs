//! Actual-card scenarios for the queue-v21 activated loot cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 activates the ability and
//! pays its mana plus tap cost; CR 121.1 draws first, then CR 701.9 applies the
//! mandatory private discard.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    activation: ManaGift,
    body: Option<(u32, u32)>,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "captain_of_umbar",
            activation: ManaGift {
                u: 1,
                ..Default::default()
            },
            body: Some((2, 3)),
        },
        Case {
            id: "dragonborn_looter",
            activation: ManaGift {
                u: 1,
                ..Default::default()
            },
            body: Some((1, 2)),
        },
        Case {
            id: "facet_reader",
            activation: ManaGift {
                u: 1,
                ..Default::default()
            },
            body: Some((1, 2)),
        },
        Case {
            id: "jalum_tome",
            activation: ManaGift {
                c: 2,
                ..Default::default()
            },
            body: None,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[card]), deck_with("forest", &[])]);
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

fn activate(e: &mut GameEngine, source: u32, case: &Case) {
    give_mana(e, 0, gift(&case.activation));
    e.apply_scenario_command(0, &activate_ability_for(e, source, 0, vec![]))
        .unwrap_or_else(|error| panic!("activate {}: {error:?}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
    pass_both_players(e);
    assert!(
        e.state.pending_resolution.is_some(),
        "{}: the discard choice is pending",
        case.id
    );
}

fn draw_then_discard_once(case: &Case, seed: u64) {
    let mut e = engine(seed, case.id);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let hand_before = e.state.players[0].hand.len();
    let library_before = e.state.players[0].library.len();
    activate(&mut e, source, case);
    assert!(
        e.state.objects[&source].tapped,
        "{}: tap cost paid",
        case.id
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 1,
        "{}: one card drawn",
        case.id
    );
    assert_eq!(
        e.state.players[0].library.len(),
        library_before - 1,
        "{}: the draw comes from the library",
        case.id
    );
    let pending = e
        .state
        .pending_resolution
        .as_ref()
        .expect("pending discard");
    assert_eq!(
        pending.deciding_player, e.state.players[0].id,
        "{}: the controller chooses the discard",
        case.id
    );
    let discarded = pending.presentation.candidates[0];
    e.apply_scenario_command(0, &submit_resolution_choice(vec![discarded]))
        .expect("discard one card");
    assert_eq!(
        e.state.objects[&discarded].zone,
        Zone::Graveyard,
        "{}: the discarded card is in the graveyard",
        case.id
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "{}: draw one then discard one is net zero",
        case.id
    );
    assert!(e.state.pending_resolution.is_none(), "{}", case.id);
}

#[test]
fn loot_activated_batch_canary_cards_draw_then_discard() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "dragonborn_looter" | "jalum_tome"))
    {
        draw_then_discard_once(&case, 9_998_000);
    }
}

#[test]
fn loot_activated_batch_cards_draw_then_discard_for_every_card() {
    for (index, case) in cases().into_iter().enumerate() {
        draw_then_discard_once(&case, 9_998_100 + index as u64);
    }
}

#[test]
fn loot_activated_batch_cards_apply_the_printed_body_and_mana_cost() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_999_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        activate(&mut e, source, &case);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            match case.body {
                Some((power, toughness)) => (Some(power), Some(toughness)),
                None => (None, None),
            },
            "{}: printed body",
            case.id
        );
        assert!(e.state.objects[&source].tapped, "{}", case.id);
        let discarded = e
            .state
            .pending_resolution
            .as_ref()
            .expect("pending discard")
            .presentation
            .candidates[0];
        e.apply_scenario_command(0, &submit_resolution_choice(vec![discarded]))
            .expect("complete the discard");
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
    }
}

#[test]
fn loot_activated_batch_cards_make_the_discard_mandatory_and_private() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_000_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let opponent_hand = e.state.players[1].hand.len();
        activate(&mut e, source, &case);
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &submit_resolution_choice(vec![]))
                .is_err(),
            "{}: an empty discard choice is rejected",
            case.id
        );
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        assert!(
            e.state.pending_resolution.is_some(),
            "{}: the discarded choice remains pending",
            case.id
        );
        assert!(
            e.apply_scenario_command(1, &submit_resolution_choice(vec![]))
                .is_err(),
            "{}: the opponent cannot answer the controller's discard",
            case.id
        );
        assert_eq!(
            e.state.players[1].hand.len(),
            opponent_hand,
            "{}: the opponent's hand is unchanged",
            case.id
        );
        let discarded = e
            .state
            .pending_resolution
            .as_ref()
            .expect("pending discard")
            .presentation
            .candidates[0];
        e.apply_scenario_command(0, &submit_resolution_choice(vec![discarded]))
            .expect("a legal discard completes the ability");
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn loot_activated_batch_cards_reject_activation_when_tapped_or_unfunded() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_001_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        activate(&mut e, source, &case);
        let pending = e
            .state
            .pending_resolution
            .as_ref()
            .expect("pending discard")
            .presentation
            .candidates[0];
        e.apply_scenario_command(0, &submit_resolution_choice(vec![pending]))
            .expect("complete the first activation");
        give_mana(&mut e, 0, gift(&case.activation));
        let before = pool(&e);
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                .is_err(),
            "{}: a tapped source cannot pay the tap cost",
            case.id
        );
        assert_eq!(pool(&e), before, "{}: no mana is spent", case.id);
        assert_eq!(e.state.command_index, command_index, "{}", case.id);

        let mut e = engine(10_002_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                .is_err(),
            "{}: the printed mana is required",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
        assert!(!e.state.objects[&source].tapped, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
    }
}
