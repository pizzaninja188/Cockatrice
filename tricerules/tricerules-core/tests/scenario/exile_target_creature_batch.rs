//! Actual-card scenarios for the reviewed "Exile target creature" family.
//!
//! Expectations are printed Oracle values. CR 701.13 exiles the chosen creature;
//! CR 608.2b leaves a departed target in its new zone and lets the spell finish
//! without exiling anything else. A land is not a legal creature target.
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
            id: "final_death",
            mana: ManaGift {
                b: 1,
                c: 4,
                ..Default::default()
            },
        },
        Case {
            id: "final_reward",
            mana: ManaGift {
                b: 1,
                c: 4,
                ..Default::default()
            },
        },
        Case {
            id: "unmake",
            mana: ManaGift {
                w: 3,
                ..Default::default()
            },
        },
    ]
}

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("swamp", &[card]),
        deck_with("forest", &["grizzly_bears", "unsummon"]),
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

fn cast_named(e: &mut GameEngine, card_id: &str, mana: ManaGift, target: u32) {
    ensure_in_hand(e, 0, card_id);
    give_mana(e, 0, mana);
    let slot = hand_index_for_card(e, 0, card_id);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {card_id}: {error}"));
    resolve_entire_stack_two_player(e);
}

#[test]
fn exile_target_creature_batch_cards_exile_the_target_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_200_000 + index as u64, case.id);
        let target = relocate_to_battlefield(&mut e, 1, "grizzly_bears", false);
        let opponent_hand_before = e.state.players[1].hand.len();
        let opponent_library_before = e.state.players[1].library.len();
        cast_named(&mut e, case.id, case.mana, target);

        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Exile,
            "{}: the chosen creature is exiled",
            case.id
        );
        assert!(
            !e.state.players[1].battlefield.contains(&target),
            "{}: the exiled creature left the battlefield",
            case.id
        );
        let spell = e.state.players[0]
            .graveyard
            .iter()
            .copied()
            .find(|oid| e.state.objects[oid].card_id == case.id)
            .expect("the spell went to its owner's graveyard");
        assert_eq!(
            e.state.objects[&spell].zone,
            Zone::Graveyard,
            "{}: the resolving spell is in the graveyard",
            case.id
        );
        assert_eq!(
            e.state.players[1].hand.len(),
            opponent_hand_before,
            "{}: the opponent's hand is unchanged",
            case.id
        );
        assert_eq!(
            e.state.players[1].library.len(),
            opponent_library_before,
            "{}: the opponent's library is unchanged",
            case.id
        );
    }
}

#[test]
fn exile_target_creature_batch_cards_do_not_exile_a_land() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_201_000 + index as u64, case.id);
        let land = inject_permanent_on_battlefield(&mut e, 1, "forest");
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, case.mana);
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(land)))
                .is_err(),
            "{}: a land is not a legal creature target",
            case.id
        );
        assert_eq!(
            e.state.objects[&land].zone,
            Zone::Battlefield,
            "{}: the rejected spell leaves the land on the battlefield",
            case.id
        );
        assert_eq!(
            e.state.objects[&e.state.players[0].hand[slot]].zone,
            Zone::Hand,
            "{}: the rejected spell stays in hand",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn exile_target_creature_batch_cards_fizzle_when_the_target_leaves() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_202_000 + index as u64, case.id);
        let target = relocate_to_battlefield(&mut e, 1, "grizzly_bears", false);
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, case.mana);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .expect("cast the exile spell");
        e.apply_scenario_command(0, &pass())
            .expect("the caster passes priority");
        ensure_in_hand(&mut e, 1, "unsummon");
        give_mana(
            &mut e,
            1,
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
        let unsummon = hand_index_for_card(&e, 1, "unsummon");
        e.apply_scenario_command(1, &cast_spell(unsummon, target_object(target)))
            .expect("return the target to its owner's hand in response");
        resolve_entire_stack_two_player(&mut e);

        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Hand,
            "{}: the target was returned to hand by the response",
            case.id
        );
        assert!(
            e.state.players[1].hand.contains(&target),
            "{}: the target is in its owner's hand",
            case.id
        );
        let spell = e.state.players[0]
            .graveyard
            .iter()
            .copied()
            .find(|oid| e.state.objects[oid].card_id == case.id)
            .expect("the fizzled spell is in the graveyard");
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard, "{}", case.id);
        assert!(
            !e.state.players[1].battlefield.contains(&target),
            "{}: the stale spell exiled nothing else",
            case.id
        );
    }
}
