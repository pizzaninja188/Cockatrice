//! Actual-card scenarios for the reviewed tap-and-lock Aura family.
//!
//! Expectations are printed Oracle values. CR 303.4a makes the Aura spell target
//! the creature it enchants; CR 603.6a stages the entry trigger that taps it;
//! CR 611.3 and CR 502.3 keep it from untapping. A target that is already tapped
//! remains a legal target (2017-09-29 ruling), while an illegal target at
//! resolution keeps the Aura from entering the battlefield.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::{AttachmentRecipient, Zone};

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 4] {
    [
        Case {
            id: "castaways_despair",
            mana: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "containment_protocol",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "sleep_paralysis",
            mana: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "waterknot",
            mana: ManaGift {
                u: 2,
                c: 1,
                ..Default::default()
            },
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("island", &[card]),
        deck_with("forest", &["grizzly_bears", "swords_to_plowshares"]),
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

fn cast_aura(e: &mut GameEngine, card_id: &str, mana: ManaGift, target: u32) -> u32 {
    ensure_in_hand(e, 0, card_id);
    give_mana(e, 0, mana);
    let slot = hand_index_for_card(e, 0, card_id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {card_id}: {error}"));
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, card_id)
}

#[test]
fn aura_tap_lock_batch_cards_tap_and_lock_the_enchanted_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_100_000 + index as u64, case.id);
        let target = relocate_to_battlefield(&mut e, 1, "grizzly_bears", false);
        let aura = cast_aura(&mut e, case.id, case.mana, target);
        assert_eq!(
            e.state.objects[&aura].attached_to,
            Some(AttachmentRecipient::Object(target)),
            "{}: the Aura is attached to the target",
            case.id
        );
        assert!(
            e.state.objects[&target].tapped,
            "{}: the entry trigger taps the creature",
            case.id
        );

        end_active_turn(&mut e, 0);
        assert!(
            e.state.objects[&target].tapped,
            "{}: the creature does not untap during its controller's untap step",
            case.id
        );
    }
}

#[test]
fn aura_tap_lock_batch_cards_can_target_an_already_tapped_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_101_000 + index as u64, case.id);
        let target = relocate_to_battlefield(&mut e, 1, "grizzly_bears", true);
        assert!(e.state.objects[&target].tapped, "{}", case.id);
        let aura = cast_aura(&mut e, case.id, case.mana, target);
        assert_eq!(
            e.state.objects[&aura].attached_to,
            Some(AttachmentRecipient::Object(target)),
            "{}: an already-tapped creature is a legal Aura target",
            case.id
        );
        assert!(
            e.state.objects[&target].tapped,
            "{}: it remains tapped",
            case.id
        );
    }
}

#[test]
fn aura_tap_lock_batch_cards_do_not_enter_when_the_target_is_illegal() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_102_000 + index as u64, case.id);
        let target = relocate_to_battlefield(&mut e, 1, "grizzly_bears", false);
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, case.mana);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .expect("cast the Aura");
        e.apply_scenario_command(0, &pass())
            .expect("the Aura's controller passes");
        ensure_in_hand(&mut e, 1, "swords_to_plowshares");
        give_mana(
            &mut e,
            1,
            ManaGift {
                w: 1,
                ..Default::default()
            },
        );
        let swords = hand_index_for_card(&e, 1, "swords_to_plowshares");
        e.apply_scenario_command(1, &cast_spell(swords, target_object(target)))
            .expect("exile the Aura's target in response");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Exile,
            "{}: the target left the battlefield",
            case.id
        );
        let aura = e.state.players[0]
            .graveyard
            .iter()
            .copied()
            .find(|oid| e.state.objects[oid].card_id == case.id)
            .expect("the Aura went to its owner's graveyard");
        assert_eq!(e.state.objects[&aura].zone, Zone::Graveyard, "{}", case.id);
        assert_eq!(
            e.state.objects[&aura].attached_to, None,
            "{}: the Aura never entered attached",
            case.id
        );
        assert!(
            !e.state.players[0]
                .battlefield
                .iter()
                .any(|oid| e.state.objects[oid].card_id == case.id),
            "{}: the Aura is not on the battlefield",
            case.id
        );
    }
}
