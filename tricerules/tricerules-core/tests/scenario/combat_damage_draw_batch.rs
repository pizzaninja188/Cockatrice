//! Actual-card scenarios for the reviewed combat-damage draw family.
//!
//! Expectations are printed Oracle values. CR 510.1b and 510.2 assign and deal
//! combat damage to the player only when the attacker is unblocked; CR 603.2
//! triggers on that event. Noncombat damage does not satisfy the trigger.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    power: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "nine-tail_white_fox",
            power: 2,
        },
        Case {
            id: "soulknife_spy",
            power: 3,
        },
        Case {
            id: "stealer_of_secrets",
            power: 2,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &[card]),
        deck_with("forest", &["grizzly_bears", "lightning_bolt"]),
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

fn enter_combat(e: &mut GameEngine) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_scenario_command(0, &pass())
        .expect("ap pass begin combat");
    e.apply_scenario_command(1, &pass())
        .expect("nap pass begin combat");
}

#[test]
fn combat_damage_draw_batch_cards_draw_when_they_deal_combat_damage_to_a_player() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_220_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        enter_combat(&mut e);
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        let opponent_hand_before = e.state.players[1].hand.len();
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the attacker");
        e.apply_scenario_command(0, &pass()).expect("ap pass");
        e.apply_scenario_command(1, &pass()).expect("nap pass");
        e.apply_scenario_command(0, &pass()).expect("ap pass");
        e.apply_scenario_command(1, &pass()).expect("nap pass");
        assert_eq!(
            e.state.players[1].life,
            20 - case.power as i32,
            "{}: the unblocked attacker deals its printed power",
            case.id
        );
        assert_eq!(
            e.state.stack.len(),
            1,
            "{}: the combat-damage trigger is on the stack",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before + 1,
            "{}: its controller draws one card",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            library_before - 1,
            "{}: the draw comes from the controller's library",
            case.id
        );
        assert_eq!(
            e.state.players[1].hand.len(),
            opponent_hand_before,
            "{}: the defending player draws nothing",
            case.id
        );
    }
}

#[test]
fn combat_damage_draw_batch_cards_do_not_draw_when_blocked() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_221_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        let blocker = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        enter_combat(&mut e);
        let hand_before = e.state.players[0].hand.len();
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the attacker");
        e.apply_scenario_command(0, &pass()).expect("ap pass");
        e.apply_scenario_command(1, &pass()).expect("nap pass");
        e.apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .expect("declare the blocker");
        e.apply_scenario_command(0, &pass()).expect("ap pass");
        e.apply_scenario_command(1, &pass()).expect("nap pass");
        assert_eq!(
            e.state.players[1].life, 20,
            "{}: the blocked attacker deals no damage to the player",
            case.id
        );
        assert!(
            e.state.stack.is_empty(),
            "{}: no combat-damage-to-player trigger fires",
            case.id
        );
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before,
            "{}: its controller draws nothing",
            case.id
        );
    }
}

#[test]
fn combat_damage_draw_batch_cards_do_not_draw_from_noncombat_damage() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_222_000 + index as u64, case.id);
        let target = move_ready_to_battlefield(&mut e, 0, case.id);
        let hand_before = e.state.players[0].hand.len();
        e.apply_scenario_command(0, &pass())
            .expect("the active player passes priority");
        ensure_in_hand(&mut e, 1, "lightning_bolt");
        give_mana(
            &mut e,
            1,
            ManaGift {
                r: 1,
                ..Default::default()
            },
        );
        let bolt = hand_index_for_card(&e, 1, "lightning_bolt");
        e.apply_scenario_command(1, &cast_spell(bolt, target_object(target)))
            .expect("cast noncombat damage");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Graveyard,
            "{}: the 3-damage spell destroys the creature",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before,
            "{}: noncombat damage does not draw",
            case.id
        );
    }
}
