//! Actual-card scenarios for the reviewed `Destroy target attacking creature.` family.
//!
//! Expectations are printed Oracle values. CR 115.1a and 601.2c require one
//! attacking-creature target as the spell is cast; CR 608.2b revalidates it; CR
//! 701.8 moves the destroyed creature to its owner's graveyard.
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
            id: "immolating_glare",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "rebuke",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "kill_shot",
            mana: ManaGift {
                w: 1,
                c: 2,
                ..Default::default()
            },
        },
    ]
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

fn declare_an_attacker(e: &mut GameEngine) -> u32 {
    let attacker = inject_creature_on_battlefield(e, 0, "grizzly_bears");
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_scenario_command(0, &pass())
        .expect("active pass begin combat");
    e.apply_scenario_command(1, &pass())
        .expect("defender pass begin combat");
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare the attacker");
    attacker
}

#[test]
fn destroy_attacking_batch_cards_destroy_only_an_attacking_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_220_000 + index as u64, case.id);
        let attacker = declare_an_attacker(&mut e);
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(attacker)))
            .unwrap_or_else(|error| panic!("{}: cast at the attacker: {error}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&attacker].zone,
            Zone::Graveyard,
            "{}: the attacking creature is destroyed",
            case.id
        );
        assert!(
            e.state.players[0].graveyard.contains(&attacker),
            "{}: it is in its owner's graveyard",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn destroy_attacking_batch_cards_reject_a_nonattacking_creature() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_221_000 + index as u64, case.id);
        let bystander = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_object(bystander)))
                .is_err(),
            "{}: a creature that is not attacking is not a legal target",
            case.id
        );
        assert_eq!(
            e.state.objects[&bystander].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(
            e.state.stack.is_empty(),
            "{}: rejected cast leaves no spell",
            case.id
        );
    }
}

#[test]
fn destroy_attacking_batch_cards_fizzle_a_stale_attacker() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_222_000 + index as u64, case.id);
        let attacker = declare_an_attacker(&mut e);
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(attacker)))
            .expect("cast at the attacker");
        e.state.players[0]
            .battlefield
            .retain(|oid| *oid != attacker);
        e.state.objects.get_mut(&attacker).expect("attacker").zone = Zone::Exile;
        e.state.players[0].exile.push(attacker);
        *e.state.zone_change_generation.entry(attacker).or_insert(0) += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&attacker].zone,
            Zone::Exile,
            "{}: the stale attacker stays exiled",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn destroy_attacking_batch_cards_destroy_an_opponents_attacker() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_223_000 + index as u64, case.id);
        end_active_turn(&mut e, 0);
        pass_priority_round(&mut e);
        pass_priority_round(&mut e);
        assert_eq!(
            e.state.turn_step,
            tricerules_core::TurnStep::Main1,
            "{}",
            case.id
        );
        let attacker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        e.apply_scenario_command(1, &primitive_yield())
            .expect("opponent main1 to begin combat");
        e.apply_scenario_command(1, &pass())
            .expect("opponent pass begin combat");
        e.apply_scenario_command(0, &pass())
            .expect("defender pass begin combat");
        e.apply_scenario_command(1, &declare_attackers(vec![attacker]))
            .expect("declare the opponent's attacker");
        e.apply_scenario_command(1, &pass())
            .expect("the active player passes priority");
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(attacker)))
            .unwrap_or_else(|error| {
                panic!("{}: cast at the opponent's attacker: {error}", case.id)
            });
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&attacker].zone,
            Zone::Graveyard,
            "{}: an opponent's attacking creature is a legal target",
            case.id
        );
        assert!(
            e.state.players[1].graveyard.contains(&attacker),
            "{}: it is in its owner's graveyard",
            case.id
        );
    }
}

#[test]
fn destroy_attacking_batch_cards_leave_a_creature_that_stopped_attacking() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_224_000 + index as u64, case.id);
        let attacker = declare_an_attacker(&mut e);
        ensure_in_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        e.apply_scenario_command(0, &cast_spell(slot, target_object(attacker)))
            .expect("cast at the attacker");
        let combat = e.state.combat.as_mut().expect("combat");
        combat.attacking.retain(|oid| *oid != attacker);
        combat.attack_assignments.remove(&attacker);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&attacker].zone,
            Zone::Battlefield,
            "{}: a creature that stopped attacking is not destroyed",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}
