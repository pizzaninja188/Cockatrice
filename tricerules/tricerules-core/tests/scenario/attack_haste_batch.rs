//! Actual-card scenarios for the reviewed haste and must-attack creature cohort.
//!
//! Expectations are printed Oracle values. CR 702.10b lets a creature with haste
//! attack the turn it enters; CR 508.1d requires an able must-attack creature to
//! be declared as an attacker. The requirement does not apply while tapped.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "ashen_monstrosity",
            mana: ManaGift {
                r: 2,
                c: 5,
                ..Default::default()
            },
            power: 7,
            toughness: 4,
        },
        Case {
            id: "flameborn_hellion",
            mana: ManaGift {
                r: 1,
                c: 5,
                ..Default::default()
            },
            power: 5,
            toughness: 4,
        },
        Case {
            id: "valley_dasher",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            power: 2,
            toughness: 2,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card]),
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

fn combat_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card]),
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
    advance_to_declare_attackers(&mut e);
    e
}

fn cast_creature(e: &mut GameEngine, case: &Case) -> u32 {
    let slot = inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let hand_slot = e.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == slot)
        .expect("injected card is in hand");
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .expect("cast the haste creature");
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, case.id)
}

#[test]
fn attack_haste_batch_cards_cast_with_exact_printed_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_300_000 + index as u64, case.id);
        let creature = cast_creature(&mut e, &case);
        assert_eq!(
            e.state.objects[&creature].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(e.effective_power(creature), Some(case.power), "{}", case.id);
        assert_eq!(
            e.effective_toughness(creature),
            Some(case.toughness),
            "{}",
            case.id
        );
        assert!(
            e.effective_has_keyword(creature, Keyword::Haste),
            "{}: printed haste",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn attack_haste_batch_cards_attack_the_turn_they_enter() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = combat_engine(9_301_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, case.id);
        e.state
            .objects
            .get_mut(&creature)
            .expect("creature")
            .summoning_sick = true;
        e.initial_response_batch();
        assert!(
            e.state.objects[&creature].summoning_sick,
            "{}: the creature entered this turn",
            case.id
        );
        e.apply_scenario_command(0, &declare_attackers(vec![creature]))
            .expect("haste ignores summoning sickness");
        assert!(
            e.state
                .combat
                .as_ref()
                .expect("combat")
                .attacking
                .contains(&creature),
            "{}: attacking",
            case.id
        );
    }
}

#[test]
fn attack_haste_batch_cards_must_attack_when_able() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = combat_engine(9_302_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, case.id);
        assert!(
            e.state.objects[&creature].must_attack_if_able,
            "{}: the must-attack characteristic is loaded",
            case.id
        );
        assert!(
            e.apply_scenario_command(0, &declare_attackers(vec![]))
                .is_err(),
            "{}: omitting an able must-attack creature is illegal",
            case.id
        );
        assert!(
            e.state
                .combat
                .as_ref()
                .is_none_or(|combat| !combat.attacking.contains(&creature)),
            "{}: the rejected declaration leaves no attacker",
            case.id
        );
        e.apply_scenario_command(0, &declare_attackers(vec![creature]))
            .expect("the legal declaration is accepted");
    }
}

#[test]
fn attack_haste_batch_cards_may_be_omitted_when_unable() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = combat_engine(9_303_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, case.id);
        e.state.objects.get_mut(&creature).expect("creature").tapped = true;
        e.initial_response_batch();
        e.apply_scenario_command(0, &declare_attackers(vec![]))
            .expect("a tapped must-attack creature cannot attack");
        assert!(
            e.state
                .combat
                .as_ref()
                .is_none_or(|combat| combat.attacking.is_empty()),
            "{}: no creature attacks",
            case.id
        );
    }
}
