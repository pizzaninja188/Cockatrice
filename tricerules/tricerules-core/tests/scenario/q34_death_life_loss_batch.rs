//! Actual-card scenarios for the queue-v34 dies, each player loses life cohort.
//!
//! Expectations are printed Oracle values. CR 700.4/603.6c trigger on leaving
//! the battlefield for a graveyard; CR 119.3 makes each player lose the printed
//! life. The instruction is untargeted.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    amount: i32,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "caustic_hound",
            amount: 4,
            power: 4,
            toughness: 4,
        },
        Case {
            id: "deathcurse_ogre",
            amount: 3,
            power: 3,
            toughness: 3,
        },
        Case {
            id: "soulcage_fiend",
            amount: 3,
            power: 3,
            toughness: 2,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("swamp", &[card, "murder", "unsummon"]),
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

fn kill_source(e: &mut GameEngine, source: u32) {
    inject_card_into_hand(e, 0, "murder");
    give_mana(
        e,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let murder = hand_index_for_card(e, 0, "murder");
    e.apply_scenario_command(0, &cast_spell(murder, target_object(source)))
        .expect("destroy the source");
    e.apply_scenario_command(0, &pass()).expect("p0 pass");
    e.apply_scenario_command(1, &pass())
        .expect("Murder resolves");
}

#[test]
fn q34_death_life_loss_canary_simplest() {
    let case = &cases()[2];
    let mut e = engine(34_050_001, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    kill_source(&mut e, source);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.state.players[0].life, e.state.players[1].life),
        (17, 17),
        "Soulcage Fiend makes each player lose 3 life"
    );
}

#[test]
fn q34_death_life_loss_canary_hardest() {
    let case = &cases()[0];
    let mut e = engine(34_050_002, case.id);
    let source = move_ready_to_battlefield(&mut e, 0, case.id);
    kill_source(&mut e, source);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.state.players[0].life, e.state.players[1].life),
        (16, 16),
        "Caustic Hound makes each player lose 4 life"
    );
}

#[test]
fn q34_death_life_loss_cards_hit_both_players_for_the_printed_amount() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(34_051_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        let bystander = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        kill_source(&mut e, source);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.state.players[0].life, e.state.players[1].life),
            (20 - case.amount, 20 - case.amount),
            "{}: each player loses exactly the printed life",
            case.id
        );
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(2), Some(2)),
            "{}: no creature is affected",
            case.id
        );
    }
}

#[test]
fn q34_death_life_loss_cards_do_not_trigger_on_other_zone_changes() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(34_052_000 + index as u64, case.id);
        let bounced = move_ready_to_battlefield(&mut e, 0, case.id);
        inject_card_into_hand(&mut e, 0, "unsummon");
        give_mana(
            &mut e,
            0,
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
        let unsummon = hand_index_for_card(&e, 0, "unsummon");
        e.apply_scenario_command(0, &cast_spell(unsummon, target_object(bounced)))
            .expect("return the source to its owner's hand");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&bounced].zone, Zone::Hand, "{}", case.id);
        assert!(
            e.state.pending_triggers.is_empty(),
            "{}: leaving for hand is not dying",
            case.id
        );
        assert_eq!(
            (e.state.players[0].life, e.state.players[1].life),
            (20, 20),
            "{}: no life is lost",
            case.id
        );
    }
}
