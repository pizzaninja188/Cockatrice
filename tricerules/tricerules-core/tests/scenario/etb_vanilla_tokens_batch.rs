//! Actual-card scenarios for the queue-v23 entry vanilla-token cohort.
//!
//! Expectations are printed Oracle values. CR 603.6a stages the entry trigger,
//! CR 111.1 creates tokens, CR 608.2c uses the controller, and the token
//! identity is the shipped registered template.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;

struct Case {
    id: &'static str,
    mana: ManaGift,
    body: (u32, u32),
    token: &'static str,
    token_name: &'static str,
    count: u32,
    color: &'static str,
    types: &'static [&'static str],
}

fn cases() -> [Case; 12] {
    [
        Case {
            id: "goblin_instigator",
            mana: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            body: (1, 1),
            token: "goblin_r_1_1",
            token_name: "Goblin",
            count: 1,
            color: "r",
            types: &["Creature", "Goblin"],
        },
        Case {
            id: "goblin_gang_leader",
            mana: ManaGift {
                r: 2,
                c: 2,
                ..Default::default()
            },
            body: (2, 2),
            token: "goblin_r_1_1",
            token_name: "Goblin",
            count: 2,
            color: "r",
            types: &["Creature", "Goblin"],
        },
        Case {
            id: "beetleback_chief",
            mana: ManaGift {
                r: 2,
                c: 2,
                ..Default::default()
            },
            body: (2, 2),
            token: "goblin_r_1_1",
            token_name: "Goblin",
            count: 2,
            color: "r",
            types: &["Creature", "Goblin"],
        },
        Case {
            id: "ambassador_oak",
            mana: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            body: (3, 3),
            token: "elf_warrior_g_1_1",
            token_name: "Elf Warrior",
            count: 1,
            color: "g",
            types: &["Creature", "Elf", "Warrior"],
        },
        Case {
            id: "elderleaf_mentor",
            mana: ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            body: (3, 2),
            token: "elf_warrior_g_1_1",
            token_name: "Elf Warrior",
            count: 1,
            color: "g",
            types: &["Creature", "Elf", "Warrior"],
        },
        Case {
            id: "yavimaya_sapherd",
            mana: ManaGift {
                g: 1,
                c: 2,
                ..Default::default()
            },
            body: (2, 2),
            token: "saproling_g_1_1",
            token_name: "Saproling",
            count: 1,
            color: "g",
            types: &["Creature", "Saproling"],
        },
        Case {
            id: "clarion_cathars",
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
            body: (3, 3),
            token: "human_w_1_1",
            token_name: "Human",
            count: 1,
            color: "w",
            types: &["Creature", "Human"],
        },
        Case {
            id: "watchful_giant",
            mana: ManaGift {
                w: 1,
                c: 5,
                ..Default::default()
            },
            body: (3, 6),
            token: "human_w_1_1",
            token_name: "Human",
            count: 1,
            color: "w",
            types: &["Creature", "Human"],
        },
        Case {
            id: "daysquad_marshal",
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
            body: (3, 3),
            token: "human_soldier_w_1_1",
            token_name: "Human Soldier",
            count: 1,
            color: "w",
            types: &["Creature", "Human", "Soldier"],
        },
        Case {
            id: "protector_of_gondor",
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
            body: (3, 3),
            token: "human_soldier_w_1_1",
            token_name: "Human Soldier",
            count: 1,
            color: "w",
            types: &["Creature", "Human", "Soldier"],
        },
        Case {
            id: "aviation_pioneer",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            body: (1, 2),
            token: "thopter_c_1_1_flying",
            token_name: "Thopter",
            count: 1,
            color: "",
            types: &["Artifact", "Creature", "Thopter"],
        },
        Case {
            id: "ghirapur_gearcrafter",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            body: (2, 1),
            token: "thopter_c_1_1_flying",
            token_name: "Thopter",
            count: 1,
            color: "",
            types: &["Artifact", "Creature", "Thopter"],
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

fn engine(seed: u64, card: &'static str) -> GameEngine {
    let decks = Some(vec![
        deck_with("plains", &[card, card]),
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn resolve_top_stack(engine: &mut GameEngine) -> tricerules_proto::ruled::v1::RuledEventBatch {
    let first = engine.state.priority_player_id();
    let second = if first == engine.state.players[0].id {
        engine.state.players[1].id
    } else {
        engine.state.players[0].id
    };
    engine
        .apply_scenario_command(first, &pass())
        .expect("first pass");
    let mut batch = engine
        .apply_scenario_command(second, &pass())
        .expect("second pass resolves stack item");
    for (_, _, answered) in answer_simultaneous_entry_order_in_engine_order(engine) {
        batch.events.extend(answered.events);
    }
    batch
}

fn cast_source(e: &mut GameEngine, case: &Case) -> u32 {
    inject_card_into_hand(e, 0, case.id);
    give_mana(e, 0, gift(&case.mana));
    let slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(
        pool(e),
        (0, 0, 0, 0, 0, 0),
        "{}: exact printed cost",
        case.id
    );
    resolve_entire_stack_two_player(e);
    if !e.state.stack.is_empty() {
        resolve_top_stack(e);
    }
    answer_simultaneous_entry_order_in_engine_order(e);
    battlefield_object_for_card(e, 0, case.id)
}

#[test]
fn etb_vanilla_tokens_batch_canary_cards_create_the_printed_token() {
    for case in cases()
        .into_iter()
        .filter(|case| matches!(case.id, "goblin_instigator" | "aviation_pioneer"))
    {
        let mut e = engine(13_000_000, case.id);
        let entered = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(
            (e.effective_power(entered), e.effective_toughness(entered)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        assert_eq!(e.state.stack.len(), 1, "{}: one entry trigger", case.id);
        let resolved = resolve_top_stack(&mut e);
        let tokens = battlefield_token_oids(&e, 0, case.token);
        assert_eq!(tokens.len(), case.count as usize, "{}", case.id);
        let created = token_created_events(&resolved);
        assert_eq!(created.len(), case.count as usize, "{}", case.id);
        assert_eq!(created[0].card_id, case.token, "{}", case.id);
        let identity = created[0].identity.as_ref().expect("public token identity");
        assert_eq!(identity.name, case.token_name, "{}", case.id);
        assert!(identity.is_creature, "{}", case.id);
    }
}

#[test]
fn etb_vanilla_tokens_batch_cards_create_only_their_printed_token() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(13_010_000 + index as u64, case.id);
        move_ready_to_battlefield(&mut e, 0, case.id);
        let resolved = resolve_top_stack(&mut e);
        let tokens = battlefield_token_oids(&e, 0, case.token);
        assert_eq!(tokens.len(), case.count as usize, "{}: count", case.id);
        let created = token_created_events(&resolved);
        assert_eq!(
            created.len(),
            case.count as usize,
            "{}: token events",
            case.id
        );
        for event in created {
            let identity = event.identity.as_ref().expect("public token identity");
            assert_eq!(identity.color, case.color, "{}: token color", case.id);
            assert_eq!(identity.types, case.types, "{}: token types", case.id);
        }
        assert_eq!(
            e.state.players[1].battlefield.len(),
            0,
            "{}: opponent tokens",
            case.id
        );
        for token in tokens {
            assert_eq!(
                (e.effective_power(token), e.effective_toughness(token)),
                (Some(1), Some(1)),
                "{}: token body",
                case.id
            );
            assert_eq!(
                e.state.objects[&token].controller, e.state.players[0].id,
                "{}",
                case.id
            );
            let flying = e.effective_has_keyword(token, Keyword::Flying);
            assert_eq!(flying, case.token == "thopter_c_1_1_flying", "{}", case.id);
        }
    }
}

#[test]
fn etb_vanilla_tokens_batch_cards_cast_with_exact_cost_and_body() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(13_020_000 + index as u64, case.id);
        let entered = cast_source(&mut e, &case);
        assert_eq!(
            (e.effective_power(entered), e.effective_toughness(entered)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        assert_eq!(
            battlefield_token_oids(&e, 0, case.token).len(),
            case.count as usize,
            "{}: trigger resolved after the cast",
            case.id
        );
        assert_eq!(
            e.state.objects[&entered].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
    }
}

#[test]
fn etb_vanilla_tokens_batch_cards_create_tokens_once_per_entry() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(13_030_000 + index as u64, case.id);
        move_ready_to_battlefield(&mut e, 0, case.id);
        resolve_top_stack(&mut e);
        move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(
            e.state.stack.len(),
            1,
            "{}: the second entry triggers",
            case.id
        );
        resolve_top_stack(&mut e);
        assert_eq!(
            battlefield_token_oids(&e, 0, case.token).len(),
            (case.count * 2) as usize,
            "{}: one printed cohort per entry",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn etb_vanilla_tokens_batch_count_two_cards_present_simultaneous_entry_order() {
    for (index, case) in cases().into_iter().enumerate() {
        if case.count != 2 {
            continue;
        }
        let mut e = engine(13_050_000 + index as u64, case.id);
        move_ready_to_battlefield(&mut e, 0, case.id);
        let first = e.state.priority_player_id();
        let second = if first == e.state.players[0].id {
            e.state.players[1].id
        } else {
            e.state.players[0].id
        };
        e.apply_scenario_command(first, &pass())
            .expect("first pass");
        e.apply_scenario_command(second, &pass())
            .expect("second pass resolves trigger");
        let pending = e
            .state
            .pending_resolution
            .as_ref()
            .expect("two tokens require an entry order");
        assert_eq!(
            pending.presentation.choice_kind,
            tricerules_proto::ruled::v1::ChoiceKind::SimultaneousEntryOrder,
            "{}: simultaneous entry order",
            case.id
        );
        assert_eq!(
            pending.presentation.candidates.len(),
            2,
            "{}: two token candidates",
            case.id
        );
        let answers = answer_simultaneous_entry_order_in_engine_order(&mut e);
        assert_eq!(answers.len(), 1, "{}: one order answer", case.id);
        assert_eq!(
            battlefield_token_oids(&e, 0, case.token).len(),
            2,
            "{}: both tokens enter",
            case.id
        );
    }
}

#[test]
fn etb_vanilla_tokens_batch_cards_source_effect_is_controller_scoped() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(13_040_000 + index as u64, case.id);
        move_ready_to_battlefield(&mut e, 0, case.id);
        let resolved = resolve_top_stack(&mut e);
        let created = token_created_events(&resolved);
        assert_eq!(created.len(), case.count as usize, "{}", case.id);
        assert!(
            created.iter().all(|event| event.controller_player_id == 0),
            "{}",
            case.id
        );
        assert!(
            battlefield_token_oids(&e, 1, case.token).is_empty(),
            "{}",
            case.id
        );
    }
}
