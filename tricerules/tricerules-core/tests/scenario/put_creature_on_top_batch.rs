//! Actual-card scenarios for the reviewed "put target creature on top of its
//! owner's library" family.
//!
//! Expectations are printed Oracle values. CR 400.3 moves the card to its
//! owner's library; the instruction places it on top. CR 608.2b leaves a
//! departed target in its new zone and lets the spell finish without moving it.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::permanent_moved::Destination;

struct Case {
    id: &'static str,
    mana: ManaGift,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "excommunicate",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "forced_retreat",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "time_ebb",
            mana: ManaGift {
                u: 1,
                c: 2,
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

#[test]
fn put_creature_on_top_batch_cards_put_the_target_on_top_of_its_owners_library() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_250_000 + index as u64, case.id);
        let target = relocate_to_battlefield(&mut e, 1, "grizzly_bears", true);
        e.state.objects.get_mut(&target).expect("target").damage = 1;
        let generation_before = e
            .state
            .zone_change_generation
            .get(&target)
            .copied()
            .unwrap_or(0);
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, case.mana);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .expect("cast the placement spell");
        e.apply_scenario_command(0, &pass())
            .expect("the caster passes");
        let batch = e
            .apply_scenario_command(1, &pass())
            .expect("the opponent passes and the spell resolves");

        let object = e.state.objects.get(&target).expect("target");
        assert_eq!(object.zone, Zone::Library, "{}", case.id);
        assert_eq!(
            object.owner, 1,
            "{}: the owner chooses the library",
            case.id
        );
        assert_eq!(
            object.controller, 1,
            "{}: a nonbattlefield object resets to its owner",
            case.id
        );
        assert!(!object.tapped, "{}", case.id);
        assert_eq!(object.damage, 0, "{}", case.id);
        assert_eq!(
            e.state.zone_change_generation.get(&target).copied(),
            Some(generation_before + 1),
            "{}: one zone change",
            case.id
        );
        assert_eq!(
            e.state.players[1].library.front().copied(),
            Some(target),
            "{}: the permanent is the top card of its owner's library",
            case.id
        );
        assert!(
            !e.state.players[0].battlefield.contains(&target),
            "{}: the permanent left the battlefield",
            case.id
        );
        assert!(
            permanents_moved_in(&batch).iter().any(|moved| {
                moved.object_id == target
                    && moved.owner_player_id == 1
                    && moved.destination == Destination::Library as i32
            }),
            "{}: the move event names the library destination",
            case.id
        );
    }
}

#[test]
fn put_creature_on_top_batch_cards_do_not_target_a_land() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_251_000 + index as u64, case.id);
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
            "{}: the rejected spell leaves the land alone",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn put_creature_on_top_batch_cards_fizzle_when_the_target_leaves() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_252_000 + index as u64, case.id);
        let target = relocate_to_battlefield(&mut e, 1, "grizzly_bears", false);
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, case.mana);
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
            .expect("cast the placement spell");
        e.apply_scenario_command(0, &pass())
            .expect("the caster passes");
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
            .expect("exile the target in response");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Exile,
            "{}: the target left the battlefield",
            case.id
        );
        assert!(
            !e.state.players[1].library.contains(&target),
            "{}: the stale spell did not move the card to the library",
            case.id
        );
        let spell = e.state.players[0]
            .graveyard
            .iter()
            .copied()
            .find(|oid| e.state.objects[oid].card_id == case.id)
            .expect("the fizzled spell is in the graveyard");
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard, "{}", case.id);
    }
}
