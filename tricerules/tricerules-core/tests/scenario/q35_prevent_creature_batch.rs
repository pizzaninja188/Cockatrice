//! Actual-card scenarios for the queue-v35 prevention-shield creatures.
//!
//! Expectations are printed Oracle values. CR 614.1a/615.1 create a finite
//! prevention shield; CR 611.2a/514.2 end it at cleanup. A shield absorbs only
//! damage dealt to the chosen target and only until its amount is consumed.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;

struct Case {
    id: &'static str,
    amount: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "kei_takahashi",
            amount: 2,
        },
        Case {
            id: "oasis",
            amount: 1,
        },
        Case {
            id: "squees_toy",
            amount: 1,
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &["shock", "shock", "kei_takahashi", "oasis", "squees_toy"],
        ),
        deck_with("forest", &[]),
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

fn shield(e: &mut GameEngine, case: &Case, target: u32) -> u32 {
    let source = move_ready_to_battlefield(e, 0, case.id);
    apply_ability(e, 0, source, 0, target_object(target))
        .unwrap_or_else(|error| panic!("{}: activate: {error:?}", case.id));
    resolve_entire_stack_two_player(e);
    source
}

fn shock_creature(e: &mut GameEngine, target: u32) {
    inject_card_into_hand(e, 0, "shock");
    give_mana(
        e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, 0, "shock");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Shock");
    resolve_entire_stack_two_player(e);
}

fn damage(e: &GameEngine, target: u32) -> u32 {
    e.state.objects[&target].damage
}

#[test]
fn q35_prevent_creature_canary_simplest() {
    let case = &cases()[1];
    let mut e = engine(35_010_001);
    let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 5);
    shield(&mut e, case, target);
    shock_creature(&mut e, target);
    assert_eq!(damage(&e, target), 1, "Oasis prevents 1 of the 2 damage");
}

#[test]
fn q35_prevent_creature_canary_hardest() {
    let case = &cases()[0];
    let mut e = engine(35_010_002);
    let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 5);
    shield(&mut e, case, target);
    shock_creature(&mut e, target);
    assert_eq!(
        damage(&e, target),
        0,
        "Kei Takahashi prevents both points of the first Shock"
    );
    shock_creature(&mut e, target);
    assert_eq!(
        damage(&e, target),
        2,
        "the shield is exhausted and the second Shock is unprevented"
    );
}

#[test]
fn q35_prevent_creature_cards_absorb_exactly_their_printed_amount() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(35_011_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 5);
        shield(&mut e, case, target);
        shock_creature(&mut e, target);
        assert_eq!(
            damage(&e, target),
            2 - case.amount,
            "{}: exactly {} of the 2 damage is prevented",
            case.id,
            case.amount
        );
    }
}

#[test]
fn q35_prevent_creature_cards_protect_only_the_chosen_creature() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(35_012_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 5);
        let bystander = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 5);
        shield(&mut e, case, target);
        shock_creature(&mut e, bystander);
        assert_eq!(
            damage(&e, target),
            0,
            "{}: the chosen creature is untouched",
            case.id
        );
        assert_eq!(
            damage(&e, bystander),
            2,
            "{}: the shield does not protect another creature",
            case.id
        );
    }
}

#[test]
fn q35_prevent_creature_cards_can_shield_an_opponents_creature() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(35_013_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 5);
        let source = shield(&mut e, case, target);
        assert_eq!(
            e.state.remaining_damage_prevention(target),
            case.amount,
            "{}: the shield is created on the opponent's creature",
            case.id
        );
        shock_creature(&mut e, target);
        assert_eq!(
            damage(&e, target),
            2 - case.amount,
            "{}: the opponent's creature is shielded",
            case.id
        );
        assert!(
            e.state.objects[&source].tapped,
            "{}: the tap cost was paid",
            case.id
        );
    }
}

#[test]
fn q35_prevent_creature_cards_reject_illegal_targets_without_paying_the_cost() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(35_014_000 + index as u64);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);

        let land = inject_permanent_on_battlefield(&mut e, 0, "forest");
        assert!(
            apply_ability(&mut e, 0, source, 0, target_object(land)).is_err(),
            "{}: a land is not a legal target",
            case.id
        );
        assert!(
            !e.state.objects[&source].tapped,
            "{}: a rejected activation does not tap the source",
            case.id
        );

        assert!(
            apply_ability(&mut e, 0, source, 0, target_player(0)).is_err(),
            "{}: a player is not a legal target",
            case.id
        );
        assert!(
            !e.state.objects[&source].tapped,
            "{}: the second rejection also pays nothing",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}: nothing resolved", case.id);
    }
}

#[test]
fn q35_prevent_creature_cards_reject_foreign_controllers() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(35_015_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 5);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &pass())
            .expect("controller passes");
        assert!(
            apply_ability(&mut e, 1, source, 0, target_object(target)).is_err(),
            "{}: another player cannot activate it",
            case.id
        );
        assert_eq!(
            e.state.remaining_damage_prevention(target),
            0,
            "{}: no shield was created",
            case.id
        );
    }
}

#[test]
fn q35_prevent_creature_cards_expire_at_cleanup() {
    for (index, case) in cases().iter().enumerate() {
        let mut e = engine(35_016_000 + index as u64);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 5);
        shield(&mut e, case, target);
        assert_eq!(
            e.state.remaining_damage_prevention(target),
            case.amount,
            "{}: the shield exists during the turn",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert_eq!(
            e.state.remaining_damage_prevention(target),
            0,
            "{}: the shield ends with the turn",
            case.id
        );
    }
}
