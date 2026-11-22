//! Actual-card scenarios for the reviewed white vigilance-grant cohort.
//!
//! Expectations are printed Oracle values. CR 702.20a-b make vigilance remove the
//! tap-on-attack requirement, CR 602.2 activates the printed `{W}` ability, and
//! CR 611.2a/514.2 apply and end the until-end-of-turn grant.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    cast: ManaGift,
    power: u32,
    toughness: u32,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "bladed_sentinel",
            cast: ManaGift {
                c: 4,
                ..Default::default()
            },
            power: 2,
            toughness: 4,
        },
        Case {
            id: "llanowar_cavalry",
            cast: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            power: 1,
            toughness: 4,
        },
        Case {
            id: "bastion_mastodon",
            cast: ManaGift {
                c: 5,
                ..Default::default()
            },
            power: 4,
            toughness: 5,
        },
        Case {
            id: "towering_thunderfist",
            cast: ManaGift {
                r: 1,
                c: 4,
                ..Default::default()
            },
            power: 4,
            toughness: 4,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[card]), deck_with("forest", &[])]);
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

fn advance_main1_to_declare(e: &mut GameEngine) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    inject_creature_on_battlefield(e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &pass())
        .expect("active player passes begin combat");
    e.apply_scenario_command(1, &pass())
        .expect("defending player passes begin combat");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
}

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let pool = &e.state.players[0].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
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

fn cast_creature(e: &mut GameEngine, id: &str, mana: ManaGift) -> u32 {
    give_mana(e, 0, mana);
    let slot = inject_card_into_hand(e, 0, id);
    let hand_slot = e.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == slot)
        .expect("the injected card is in hand");
    semantic::accepted(e, 0, &cast_spell(hand_slot, vec![]));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{id}: exact cast cost");
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.state.objects[&slot].zone,
        Zone::Battlefield,
        "{id}: resolves onto the battlefield"
    );
    slot
}

fn activate_vigilance(e: &mut GameEngine, creature: u32, id: &str) {
    give_mana(
        e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    apply_ability(e, 0, creature, 0, vec![]).expect("activate vigilance");
    resolve_entire_stack_two_player(e);
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{id}: exact cost consumed");
}

#[test]
fn white_gain_vigilance_batch_cards_cast_with_exact_printed_characteristics() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_380_000 + index as u64, case.id);
        let creature = cast_creature(&mut e, case.id, gift(&case.cast));
        assert_eq!(e.effective_power(creature), Some(case.power), "{}", case.id);
        assert_eq!(
            e.effective_toughness(creature),
            Some(case.toughness),
            "{}",
            case.id
        );
        assert!(
            !e.effective_has_keyword(creature, Keyword::Vigilance),
            "{}: no printed vigilance",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn white_gain_vigilance_batch_cards_gain_vigilance_only_after_activation() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_381_000 + index as u64, case.id);
        let creature = cast_creature(&mut e, case.id, gift(&case.cast));
        assert!(
            !e.effective_has_keyword(creature, Keyword::Vigilance),
            "{}: no vigilance before activation",
            case.id
        );
        activate_vigilance(&mut e, creature, case.id);
        assert!(
            e.effective_has_keyword(creature, Keyword::Vigilance),
            "{}: vigilance after activation",
            case.id
        );
        end_active_turn(&mut e, 0);
        assert!(
            !e.effective_has_keyword(creature, Keyword::Vigilance),
            "{}: CR 514.2 ends the grant",
            case.id
        );
    }
}

#[test]
fn white_gain_vigilance_batch_cards_stay_untapped_when_attacking_with_vigilance() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_382_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        activate_vigilance(&mut e, attacker, case.id);
        advance_main1_to_declare(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the vigilant attacker");
        assert!(
            e.state
                .combat
                .as_ref()
                .expect("combat")
                .attacking
                .contains(&attacker),
            "{}: attacking",
            case.id
        );
        assert!(
            !e.state.objects[&attacker].tapped,
            "{}: vigilance does not tap the attacker",
            case.id
        );
    }
}

#[test]
fn white_gain_vigilance_batch_cards_tap_when_attacking_without_activation() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_383_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        advance_main1_to_declare(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the ordinary attacker");
        assert!(
            e.state.objects[&attacker].tapped,
            "{}: attacking without vigilance taps",
            case.id
        );
    }
}

#[test]
fn white_gain_vigilance_batch_cards_stay_tapped_when_vigilance_is_granted_after_attack() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_383_500 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        advance_main1_to_declare(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the attacker without vigilance");
        assert!(
            e.state.objects[&attacker].tapped,
            "{}: attacking without vigilance taps",
            case.id
        );
        activate_vigilance(&mut e, attacker, case.id);
        assert!(
            e.effective_has_keyword(attacker, Keyword::Vigilance),
            "{}: vigilance is granted after the attack",
            case.id
        );
        assert!(
            e.state.objects[&attacker].tapped,
            "{}: a later vigilance grant does not untap the attacking creature",
            case.id
        );
    }
}

#[test]
fn white_gain_vigilance_batch_cards_reject_activation_when_the_cost_cannot_be_paid() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_384_000 + index as u64, case.id);
        let creature = cast_creature(&mut e, case.id, gift(&case.cast));
        let before = pool(&e);
        assert!(
            apply_ability(&mut e, 0, creature, 0, vec![]).is_err(),
            "{}: vigilance needs its exact mana",
            case.id
        );
        assert_eq!(pool(&e), before, "{}: no mana is spent", case.id);
        assert!(
            !e.effective_has_keyword(creature, Keyword::Vigilance),
            "{}: the rejected activation grants nothing",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
