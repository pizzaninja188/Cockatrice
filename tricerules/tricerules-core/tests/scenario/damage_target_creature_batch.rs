//! Actual-card scenarios for the reviewed creature-only damage spell family.
//!
//! Expectations are printed Oracle values. CR 120.2b keeps the resolving spell
//! as the damage source; CR 115.1a requires a declared creature target; CR 608.2b
//! rechecks target legality on resolution; CR 704.5g handles lethal damage.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    amount: u32,
    mana: ManaGift,
}

fn cases() -> [Case; 6] {
    [
        Case {
            id: "bathe_in_dragonfire",
            amount: 4,
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
        },
        Case {
            id: "breath_of_fire",
            amount: 2,
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "electrify",
            amount: 4,
            mana: ManaGift {
                r: 1,
                c: 3,
                ..Default::default()
            },
        },
        Case {
            id: "explosive_shot",
            amount: 4,
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
        },
        Case {
            id: "fiery_finish",
            amount: 7,
            mana: ManaGift {
                r: 2,
                c: 4,
                ..Default::default()
            },
        },
        Case {
            id: "flame_slash",
            amount: 4,
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
        },
    ]
}

fn spell_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("mountain", &[]), deck_with("mountain", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn pool(engine: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let pool = &engine.state.players[0].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

fn prepare_spell(engine: &mut GameEngine, case: &Case) -> usize {
    give_mana(
        engine,
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
    let slot = inject_card_into_hand(engine, 0, case.id);
    engine.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == slot)
        .expect("the injected card is in hand")
}

#[test]
fn damage_target_creature_batch_cards_deal_exact_damage_from_the_resolving_spell() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut engine = spell_engine(452_100 + index as u64);
        let target = inject_creature_with_stats(&mut engine, 1, "hill_giant", 5, 10);
        let bystander = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 2);
        let artifact = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
        let slot = prepare_spell(&mut engine, &case);
        let source = engine.state.players[0].hand[slot];
        let generation = semantic::generation(&engine, source);

        semantic::accepted(&mut engine, 0, &cast_spell(slot, target_object(target)));
        assert_eq!(engine.state.stack.len(), 1);
        assert_eq!(
            pool(&engine),
            (0, 0, 0, 0, 0, 0),
            "{}: exact cost consumed",
            case.id
        );
        semantic::complete(&mut engine, 8, |_| None).require_exercised();

        semantic::assert_object(
            &engine,
            source,
            case.id,
            0,
            0,
            Zone::Graveyard,
            generation + 2,
        );
        assert_eq!(
            engine.state.objects[&target].damage, case.amount,
            "{}",
            case.id
        );
        assert_eq!(engine.state.objects[&bystander].damage, 0, "{}", case.id);
        assert_eq!(engine.state.objects[&artifact].damage, 0, "{}", case.id);
        assert_eq!(engine.state.players[0].life, 20, "{}", case.id);
        assert_eq!(engine.state.players[1].life, 20, "{}", case.id);
    }
}

#[test]
fn damage_target_creature_batch_cards_reject_players_and_noncreatures_as_targets() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut engine = spell_engine(452_200 + index as u64);
        let artifact = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
        let slot = prepare_spell(&mut engine, &case);
        let before_pool = pool(&engine);
        for illegal in [target_player(1), target_object(artifact)] {
            let before = engine.state.command_index;
            engine
                .apply_scenario_command(0, &cast_spell(slot, illegal.clone()))
                .expect_err("only creature targets are legal");
            assert_eq!(
                engine.state.command_index, before,
                "{}: rejected cast must not mutate state",
                case.id
            );
            assert_eq!(
                pool(&engine),
                before_pool,
                "{}: rejected cast spends no mana",
                case.id
            );
            assert!(engine.state.stack.is_empty(), "{}", case.id);
        }
        let target = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 2);
        semantic::accepted(&mut engine, 0, &cast_spell(slot, target_object(target)));
    }
}

#[test]
fn damage_target_creature_batch_cards_reject_a_cast_without_the_printed_mana() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut engine = spell_engine(452_250 + index as u64);
        let target = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 2);
        let slot = inject_card_into_hand(&mut engine, 0, case.id);
        let hand_slot = engine.state.players[0]
            .hand
            .iter()
            .position(|oid| *oid == slot)
            .expect("the injected card is in hand");
        let before = engine.state.command_index;
        engine
            .apply_scenario_command(0, &cast_spell(hand_slot, target_object(target)))
            .expect_err("the printed mana cost is required");
        assert_eq!(engine.state.command_index, before, "{}", case.id);
        assert_eq!(engine.state.players[0].hand[hand_slot], slot, "{}", case.id);
        assert!(engine.state.stack.is_empty(), "{}", case.id);
        assert_eq!(engine.state.objects[&target].damage, 0, "{}", case.id);
    }
}

#[test]
fn damage_target_creature_batch_cards_fizzle_after_the_target_leaves() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut engine = spell_engine(452_300 + index as u64);
        let target = inject_creature_with_stats(&mut engine, 1, "hill_giant", 5, 10);
        let slot = prepare_spell(&mut engine, &case);
        let source = engine.state.players[0].hand[slot];
        let generation = semantic::generation(&engine, source);
        semantic::accepted(&mut engine, 0, &cast_spell(slot, target_object(target)));

        give_mana(
            &mut engine,
            0,
            ManaGift {
                u: 1,
                ..Default::default()
            },
        );
        let unsummon_oid = inject_card_into_hand(&mut engine, 0, "unsummon");
        let unsummon = engine.state.players[0]
            .hand
            .iter()
            .position(|oid| *oid == unsummon_oid)
            .expect("unsummon in hand");
        semantic::accepted(&mut engine, 0, &cast_spell(unsummon, target_object(target)));
        resolve_entire_stack_two_player(&mut engine);

        assert_eq!(
            engine.state.objects[&target].zone,
            Zone::Hand,
            "{}",
            case.id
        );
        assert_eq!(engine.state.objects[&target].damage, 0, "{}", case.id);
        semantic::assert_object(
            &engine,
            source,
            case.id,
            0,
            0,
            Zone::Graveyard,
            generation + 2,
        );
        assert_eq!(engine.state.players[1].life, 20, "{}", case.id);
    }
}
