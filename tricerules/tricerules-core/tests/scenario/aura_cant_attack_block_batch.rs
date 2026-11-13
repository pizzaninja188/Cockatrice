//! Actual-card scenarios for the reviewed "can't attack or block" Aura family.
//!
//! Expectations are printed Oracle values. CR 303.4a makes the Aura spell target
//! the creature it enchants; CR 508.1 and 509.1 enforce the attack and block
//! restrictions. An illegal target at resolution keeps the Aura off the
//! battlefield.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "bound_in_silence",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "detained_by_legionnaires",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "luminous_bonds",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &[card, "savannah_lions"]),
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

fn enter_combat(e: &mut GameEngine) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_scenario_command(0, &pass())
        .expect("ap pass begin combat");
    e.apply_scenario_command(1, &pass())
        .expect("nap pass begin combat");
}

#[test]
fn aura_cant_attack_block_batch_cards_prevent_the_enchanted_creature_from_attacking() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_240_000 + index as u64, case.id);
        let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let legal_attacker = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
        let aura = cast_aura(&mut e, case.id, case.mana, creature);
        assert_eq!(
            e.state.objects[&aura].attached_to,
            Some(tricerules_core::AttachmentRecipient::Object(creature)),
            "{}: the Aura is attached",
            case.id
        );
        assert_eq!(
            zone_view_rules_annotation_labels(&mut e, 0, creature),
            vec!["Can't attack", "Can't block"],
            "{}: both restrictions are visible",
            case.id
        );
        enter_combat(&mut e);
        assert!(
            e.apply_scenario_command(0, &declare_attackers(vec![creature]))
                .is_err(),
            "{}: the enchanted creature is not a legal attacker",
            case.id
        );
        e.apply_scenario_command(0, &declare_attackers(vec![legal_attacker]))
            .expect("the unrestricted creature may attack");
    }
}

#[test]
fn aura_cant_attack_block_batch_cards_prevent_the_enchanted_creature_from_blocking() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_241_000 + index as u64, case.id);
        let attacker = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
        let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let legal_blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        cast_aura(&mut e, case.id, case.mana, blocker);
        enter_combat(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the attacker");
        e.apply_scenario_command(0, &pass()).expect("ap pass");
        let blockers_batch = e
            .apply_scenario_command(1, &pass())
            .expect("nap pass to blockers");
        assert_eq!(
            e.state.turn_step,
            tricerules_core::TurnStep::DeclareBlockers,
            "{}",
            case.id
        );
        let legal = blockers_batch
            .legal_by_player
            .get(&1)
            .expect("defending-player legal actions");
        let legal_pairs: Vec<_> = legal
            .legal_block_pairs
            .iter()
            .map(|pair| (pair.blocker_id, pair.attacker_id))
            .collect();
        assert_eq!(
            legal_pairs,
            [(legal_blocker, attacker)],
            "{}: only the unrestricted creature may block",
            case.id
        );
        assert!(
            e.apply_scenario_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: attacker,
                    blocker_id: blocker,
                }]),
            )
            .is_err(),
            "{}: the enchanted creature is not a legal blocker",
            case.id
        );
        e.apply_scenario_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: legal_blocker,
            }]),
        )
        .expect("the unrestricted creature blocks");
    }
}

#[test]
fn aura_cant_attack_block_batch_cards_do_not_enter_when_the_target_is_illegal() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_242_000 + index as u64, case.id);
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
