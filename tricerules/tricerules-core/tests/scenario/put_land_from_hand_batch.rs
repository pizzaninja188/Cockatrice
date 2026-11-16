//! Actual-card scenarios for the queue-v21 tap-to-put-land cohort.
//!
//! Expectations are printed Oracle values. CR 602.1/602.2 activates the ability and
//! pays the tap; CR 305.1 distinguishes putting a land onto the battlefield from
//! playing it; the choice is optional and private to the controller.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ChoiceKind;

struct Case {
    id: &'static str,
    body: Option<(u32, u32)>,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "llanowar_scout",
            body: Some((1, 3)),
        },
        Case {
            id: "sakura-tribe_scout",
            body: Some((1, 1)),
        },
        Case {
            id: "scaled_herbalist",
            body: Some((1, 3)),
        },
        Case {
            id: "walking_atlas",
            body: Some((1, 1)),
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("island", &[])]);
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

fn activate_to_choice(e: &mut GameEngine, source: u32) -> ResolutionChoiceRequired {
    semantic::accepted(e, 0, &activate_ability_for(e, source, 0, vec![]));
    assert!(e.state.objects[&source].tapped, "the tap cost is paid");
    semantic::accepted(e, 0, &pass());
    let parked = semantic::accepted(e, 1, &pass());
    find_resolution_choice(&parked).expect("private optional land choice")
}

#[test]
fn put_land_from_hand_batch_canary_cards_put_a_land_onto_the_battlefield() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "sakura-tribe_scout" | "walking_atlas"))
    {
        let mut e = engine(10_010_000, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let land = inject_card_into_hand(&mut e, 0, "forest");
        let choice = activate_to_choice(&mut e, source);
        assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
        assert_eq!(choice.deciding_player_id, 0);
        assert_eq!((choice.min, choice.max), (0, 1));
        let index = choice
            .candidate_object_ids
            .iter()
            .position(|object_id| *object_id == land)
            .expect("the Forest is a candidate");
        assert!(choice.candidate_selectable[index]);
        e.apply_scenario_command(0, &submit_resolution_choice(vec![land]))
            .expect("put the land onto the battlefield");
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(e.state.objects[&land].controller, 0, "{}", case.id);
        assert!(
            !e.state.objects[&land].tapped,
            "{}: a Forest enters untapped",
            case.id
        );
        assert_eq!(
            e.state.lands_played_this_turn, 0,
            "{}: putting a land onto the battlefield is not playing it",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
        let (power, toughness) = case.body.expect("printed body");
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(power), Some(toughness)),
            "{}: printed body",
            case.id
        );
    }
}

#[test]
fn put_land_from_hand_batch_cards_offer_no_nonland_and_reject_a_forced_nonland() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_011_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let land = inject_card_into_hand(&mut e, 0, "forest");
        let nonland = inject_card_into_hand(&mut e, 0, "ornithopter");
        let choice = activate_to_choice(&mut e, source);
        let land_index = choice
            .candidate_object_ids
            .iter()
            .position(|object_id| *object_id == land)
            .expect("the Forest is listed");
        let nonland_index = choice
            .candidate_object_ids
            .iter()
            .position(|object_id| *object_id == nonland)
            .expect("the nonland is listed but not selectable");
        assert!(choice.candidate_selectable[land_index], "{}", case.id);
        assert!(!choice.candidate_selectable[nonland_index], "{}", case.id);
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &submit_resolution_choice(vec![nonland]))
                .is_err(),
            "{}: a nonland cannot be chosen",
            case.id
        );
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        assert_eq!(e.state.objects[&land].zone, Zone::Hand, "{}", case.id);
        assert_eq!(e.state.objects[&nonland].zone, Zone::Hand, "{}", case.id);
        e.apply_scenario_command(0, &submit_resolution_choice(vec![land]))
            .expect("a legal land choice completes the ability");
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(e.state.objects[&nonland].zone, Zone::Hand, "{}", case.id);
    }
}

#[test]
fn put_land_from_hand_batch_cards_allow_declining_and_bound_a_no_land_choice() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_012_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let land = inject_card_into_hand(&mut e, 0, "forest");
        activate_to_choice(&mut e, source);
        e.apply_scenario_command(0, &submit_resolution_choice(vec![]))
            .expect("decline the optional land");
        assert_eq!(e.state.objects[&land].zone, Zone::Hand, "{}", case.id);
        assert!(e.state.objects[&source].tapped, "{}", case.id);

        let mut e = engine(10_013_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        e.state.players[0].hand.clear();
        let _nonland = inject_card_into_hand(&mut e, 0, "ornithopter");
        let choice = activate_to_choice(&mut e, source);
        assert_eq!((choice.min, choice.max), (0, 1), "{}", case.id);
        assert!(
            choice
                .candidate_selectable
                .iter()
                .all(|selectable| !selectable),
            "{}: no nonland is selectable",
            case.id
        );
        e.apply_scenario_command(0, &submit_resolution_choice(vec![]))
            .expect("decline when no land qualifies");
        assert!(e.state.pending_resolution.is_none(), "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn put_land_from_hand_batch_cards_keep_the_choice_private_and_reject_a_tapped_source() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(10_014_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let land = inject_card_into_hand(&mut e, 0, "forest");
        let opponent_hand = e.state.players[1].hand.len();
        let choice = activate_to_choice(&mut e, source);
        assert_eq!(choice.deciding_player_id, 0, "{}", case.id);
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(1, &submit_resolution_choice(vec![land]))
                .is_err(),
            "{}: the opponent cannot choose the controller's land",
            case.id
        );
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        assert_eq!(e.state.players[1].hand.len(), opponent_hand, "{}", case.id);
        e.apply_scenario_command(0, &submit_resolution_choice(vec![land]))
            .expect("the controller completes the choice");
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );

        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                .is_err(),
            "{}: the tapped source cannot pay the tap cost again",
            case.id
        );
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
