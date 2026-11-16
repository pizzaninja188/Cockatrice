//! Actual-card scenarios for the queue-v22 "sacrifice a creature, draw two or
//! three" cohort.
//!
//! Expectations are printed Oracle values. CR 118.8 and 601.2f-h make the
//! sacrifice an additional cost paid before the spell is cast; CR 701.21 moves
//! the chosen creature to the graveyard; CR 121.1 draws the printed count.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::HandActionKind;

struct Case {
    id: &'static str,
    name: &'static str,
    generic: u32,
    black: u32,
    blue: u32,
    draw: u32,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "blood_divination",
            name: "Blood Divination",
            generic: 3,
            black: 1,
            blue: 0,
            draw: 3,
        },
        Case {
            id: "vivisection",
            name: "Vivisection",
            generic: 3,
            black: 0,
            blue: 1,
            draw: 3,
        },
        Case {
            id: "altars_reap",
            name: "Altar's Reap",
            generic: 1,
            black: 1,
            blue: 0,
            draw: 2,
        },
        Case {
            id: "skulltap",
            name: "Skulltap",
            generic: 1,
            black: 1,
            blue: 0,
            draw: 2,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("swamp", &[card, "grizzly_bears", "hill_giant", "bone_saw"]),
        deck_with("forest", &["grizzly_bears"]),
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

fn fund(e: &mut GameEngine, case: &Case) {
    give_mana(
        e,
        0,
        ManaGift {
            b: case.black,
            u: case.blue,
            c: case.generic,
            ..Default::default()
        },
    );
}

fn cast_sacrificing(e: &mut GameEngine, case: &Case, victim: u32) {
    ensure_in_hand(e, 0, case.id);
    fund(e, case);
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(
        0,
        &cast_spell_with_costs(slot, vec![], vec![permanent_cost_selection(0, victim)]),
    )
    .expect("sacrifice the chosen creature and cast the spell");
}

#[test]
fn sacrifice_draw_batch_canary_cards_sacrifice_the_chosen_creature_then_draw() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "altars_reap" | "vivisection"))
    {
        let mut e = engine(11_100_000, case.id);
        let victim = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        cast_sacrificing(&mut e, &case, victim);
        assert_eq!(
            e.state.objects[&victim].zone,
            Zone::Graveyard,
            "{}: the chosen creature is sacrificed as a cost",
            case.id
        );
        assert_eq!(e.state.stack.len(), 1, "{}", case.id);
        let hand_after_payment = e.state.players[0].hand.len();
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_after_payment + case.draw as usize,
            "{}: exactly the printed draw",
            case.id
        );
        assert_eq!(
            e.state
                .objects
                .values()
                .filter(|object| object.zone == Zone::Graveyard)
                .count(),
            2,
            "{}: the sacrificed creature and the resolved spell",
            case.id
        );
    }
}

#[test]
fn sacrifice_draw_batch_cards_draw_their_printed_count() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_110_000 + index as u64, case.id);
        let victim = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        cast_sacrificing(&mut e, &case, victim);
        let hand_after_payment = e.state.players[0].hand.len();
        let opponent_hand = e.state.players[1].hand.len();
        let opponent_library = e.state.players[1].library.len();
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_after_payment + case.draw as usize,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.players[1].hand.len(),
            opponent_hand,
            "{}: only the caster draws",
            case.id
        );
        assert_eq!(
            e.state.players[1].library.len(),
            opponent_library,
            "{}: the opponent's library is untouched",
            case.id
        );
        assert_eq!(
            e.state.objects[&victim].zone,
            Zone::Graveyard,
            "{}",
            case.id
        );
    }
}

#[test]
fn sacrifice_draw_batch_cards_require_a_creature_you_control() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_120_000 + index as u64, case.id);
        ensure_in_hand(&mut e, 0, case.id);
        fund(&mut e, &case);
        let slot = hand_index_for_card(&e, 0, case.id);
        let hand_before = e.state.players[0].hand.len();
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &cast_spell_with_costs(slot, vec![], vec![]))
                .is_err(),
            "{}: the additional creature sacrifice is mandatory",
            case.id
        );
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        assert_eq!(e.state.players[0].hand.len(), hand_before, "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);

        let swamp = relocate_to_battlefield(&mut e, 0, "swamp", false);
        let batch = e
            .apply_scenario_command(0, &activate_ability(swamp, 0, vec![]))
            .expect("activate the land to publish legal actions");
        assert!(
            !batch.legal_by_player[&0].hand_actions.iter().any(|action| {
                action.kind == HandActionKind::HandActionCastSpell as i32
                    && action.card_name == case.name
            }),
            "{}: no creature means the spell is not published as castable",
            case.id
        );
    }
}

#[test]
fn sacrifice_draw_batch_cards_reject_a_noncreature_permanent() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_125_000 + index as u64, case.id);
        let artifact = move_ready_to_battlefield(&mut e, 0, "bone_saw");
        ensure_in_hand(&mut e, 0, case.id);
        fund(&mut e, &case);
        let slot = hand_index_for_card(&e, 0, case.id);
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(
                0,
                &cast_spell_with_costs(slot, vec![], vec![permanent_cost_selection(0, artifact)]),
            )
            .is_err(),
            "{}: an artifact is not a creature",
            case.id
        );
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        assert_eq!(
            e.state.objects[&artifact].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn sacrifice_draw_batch_cards_reject_an_opponents_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_130_000 + index as u64, case.id);
        let victim = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        ensure_in_hand(&mut e, 0, case.id);
        fund(&mut e, &case);
        let slot = hand_index_for_card(&e, 0, case.id);
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(
                0,
                &cast_spell_with_costs(slot, vec![], vec![permanent_cost_selection(0, victim)]),
            )
            .is_err(),
            "{}: only the caster's creature may be sacrificed",
            case.id
        );
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        assert_eq!(
            e.state.objects[&victim].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn sacrifice_draw_batch_altars_reap_can_be_cast_at_instant_speed() {
    let mut e = engine(11_140_000, "altars_reap");
    let victim = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
    end_active_turn(&mut e, 0);
    advance_to_main1_from_game_start(&mut e);
    assert_eq!(e.state.active_player_id(), 1);
    e.apply_scenario_command(1, &pass())
        .expect("the active player passes priority");
    ensure_in_hand(&mut e, 0, "altars_reap");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "altars_reap");
    e.apply_scenario_command(
        0,
        &cast_spell_with_costs(slot, vec![], vec![permanent_cost_selection(0, victim)]),
    )
    .expect("cast Altar's Reap during the opponent's main phase");
    let hand_after_payment = e.state.players[0].hand.len();
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].hand.len(), hand_after_payment + 2);
    assert_eq!(e.state.objects[&victim].zone, Zone::Graveyard);
}
