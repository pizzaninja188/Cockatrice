//! Actual-card scenarios for the queue-v22 aura cohort granting `+X/+X` and
//! haste.
//!
//! Expectations are printed Oracle values. CR 303.4 attaches the Aura;
//! CR 613.4c applies the P/T bonus; CR 702.10 grants haste; CR 704.5m sends the
//! Aura to the graveyard when its creature leaves.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    bonus: u32,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "goblin_war_paint",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            bonus: 2,
        },
        Case {
            id: "primal_visitation",
            mana: ManaGift {
                r: 1,
                g: 1,
                c: 3,
                ..Default::default()
            },
            bonus: 3,
        },
        Case {
            id: "swashbuckling",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            bonus: 2,
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("mountain", &[card, "grizzly_bears", "bone_saw", "murder"]),
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
            w: case.mana.w,
            u: case.mana.u,
            b: case.mana.b,
            r: case.mana.r,
            g: case.mana.g,
            c: case.mana.c,
        },
    );
}

fn cast_bear(e: &mut GameEngine) -> u32 {
    ensure_in_hand(e, 0, "grizzly_bears");
    give_mana(
        e,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast the creature");
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, "grizzly_bears")
}

fn cast_aura(e: &mut GameEngine, case: &Case, target: u32) {
    ensure_in_hand(e, 0, case.id);
    fund(e, case);
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast the Aura");
    resolve_entire_stack_two_player(e);
}

#[test]
fn aura_pump_haste_batch_canary_cards_attach_and_grant_the_printed_bonus() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "goblin_war_paint" | "primal_visitation"))
    {
        let mut e = engine(11_700_000, case.id);
        let bear = cast_bear(&mut e);
        cast_aura(&mut e, &case, bear);
        assert_eq!(
            e.state.objects[&bear].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        let aura = e
            .state
            .objects
            .values()
            .find(|object| object.card_id == case.id && object.zone == Zone::Battlefield)
            .expect("the Aura is on the battlefield");
        assert_eq!(
            aura.attached_to,
            Some(tricerules_core::AttachmentRecipient::Object(bear)),
            "{}: the Aura is attached",
            case.id
        );
        assert_eq!(
            (e.effective_power(bear), e.effective_toughness(bear)),
            (Some(2 + case.bonus), Some(2 + case.bonus)),
            "{}: exact printed bonus",
            case.id
        );
    }
}

#[test]
fn aura_pump_haste_batch_cards_apply_every_printed_bonus() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_710_000 + index as u64, case.id);
        let bear = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        cast_aura(&mut e, &case, bear);
        assert_eq!(
            (e.effective_power(bear), e.effective_toughness(bear)),
            (Some(2 + case.bonus), Some(2 + case.bonus)),
            "{}",
            case.id
        );
    }
}

#[test]
fn aura_pump_haste_batch_cards_grant_haste_immediately() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_720_000 + index as u64, case.id);
        let bear = cast_bear(&mut e);
        assert!(e.state.objects[&bear].summoning_sick, "{}", case.id);
        cast_aura(&mut e, &case, bear);
        e.apply_scenario_command(0, &primitive_yield())
            .expect("main1 to beginning of combat");
        pass_both_players(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![bear]))
            .expect("haste lets the just-cast creature attack");
    }
}

#[test]
fn aura_pump_haste_batch_cards_reject_a_noncreature_target() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_730_000 + index as u64, case.id);
        let artifact = move_ready_to_battlefield(&mut e, 0, "bone_saw");
        ensure_in_hand(&mut e, 0, case.id);
        fund(&mut e, &case);
        let slot = hand_index_for_card(&e, 0, case.id);
        let command_index = e.state.command_index;
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(artifact)))
                .is_err(),
            "{}: enchant creature cannot target an artifact",
            case.id
        );
        assert_eq!(e.state.command_index, command_index, "{}", case.id);
        assert!(!e.state.players[0].hand.is_empty(), "{}", case.id);
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn aura_pump_haste_batch_cards_go_to_the_graveyard_with_their_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(11_740_000 + index as u64, case.id);
        let bear = move_ready_to_battlefield(&mut e, 0, "grizzly_bears");
        cast_aura(&mut e, &case, bear);
        ensure_in_hand(&mut e, 0, "murder");
        give_mana(
            &mut e,
            0,
            ManaGift {
                b: 2,
                c: 1,
                ..Default::default()
            },
        );
        let murder = hand_index_for_card(&e, 0, "murder");
        e.apply_scenario_command(0, &cast_spell(murder, target_object(bear)))
            .expect("destroy the enchanted creature");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&bear].zone, Zone::Graveyard, "{}", case.id);
        assert!(
            e.state
                .objects
                .values()
                .any(|object| object.card_id == case.id && object.zone == Zone::Graveyard),
            "{}: the Aura fell off",
            case.id
        );
    }
}
