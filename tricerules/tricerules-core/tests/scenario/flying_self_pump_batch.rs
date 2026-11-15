//! Actual-card scenarios for the queue-v18 flying self-pump cohort.
//!
//! Expectations are printed Oracle values. CR 702.9a-b grants flying, CR 602.2
//! governs the activated ability, CR 611.2a/613.4c apply the source-only pump,
//! and CR 514.2 ends it.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::BlockPair;

struct Case {
    id: &'static str,
    mana: ManaGift,
    activation: ManaGift,
    body: (u32, u32),
    pump: (i32, i32),
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            id: "mesa_falcon",
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            activation: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            body: (1, 1),
            pump: (0, 1),
        },
        Case {
            id: "nightwing_shade",
            mana: ManaGift {
                b: 1,
                c: 4,
                ..Default::default()
            },
            activation: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            body: (2, 2),
            pump: (1, 1),
        },
        Case {
            id: "pearl_dragon",
            mana: ManaGift {
                w: 2,
                c: 4,
                ..Default::default()
            },
            activation: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
            body: (4, 4),
            pump: (0, 1),
        },
        Case {
            id: "tattered_apparition",
            mana: ManaGift {
                b: 1,
                c: 3,
                ..Default::default()
            },
            activation: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            body: (2, 2),
            pump: (1, 1),
        },
        Case {
            id: "torch_drake",
            mana: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
            activation: ManaGift {
                r: 1,
                c: 1,
                ..Default::default()
            },
            body: (2, 2),
            pump: (1, 0),
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("mountain", &[])]);
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn cast_source(e: &mut GameEngine, case: &Case) -> u32 {
    give_mana(e, 0, gift(&case.mana));
    let _slot = inject_card_into_hand(e, 0, case.id);
    let hand_slot = hand_index_for_card(e, 0, case.id);
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
    assert_eq!(
        pool(e),
        (0, 0, 0, 0, 0, 0),
        "{}: exact printed cost",
        case.id
    );
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, case.id)
}

#[test]
fn flying_self_pump_batch_cards_cast_with_exact_characteristics_and_flying() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_800_000 + index as u64);
        let source = cast_source(&mut e, &case);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        assert!(
            e.effective_has_keyword(source, Keyword::Flying),
            "{}: flying",
            case.id
        );
        assert_eq!(e.state.players[0].life, 20, "{}", case.id);
    }
}

#[test]
fn flying_self_pump_batch_cards_activate_only_the_source_with_the_printed_pump() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_801_000 + index as u64);
        let source = cast_source(&mut e, &case);
        let bystander = inject_creature_with_stats(&mut e, 0, "hill_giant", 3, 3);
        give_mana(&mut e, 0, gift(&case.activation));
        apply_ability(&mut e, 0, source, 0, vec![])
            .unwrap_or_else(|error| panic!("{}: activate the printed pump: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            pool(&e),
            (0, 0, 0, 0, 0, 0),
            "{}: exact activation cost",
            case.id
        );
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (
                Some((case.body.0 as i32 + case.pump.0) as u32),
                Some((case.body.1 as i32 + case.pump.1) as u32)
            ),
            "{}: printed pump",
            case.id
        );
        assert_eq!(
            (
                e.effective_power(bystander),
                e.effective_toughness(bystander)
            ),
            (Some(3), Some(3)),
            "{}: source only",
            case.id
        );
    }
}

#[test]
fn flying_self_pump_batch_cards_stack_repeated_activations_in_one_turn() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_802_000 + index as u64);
        let source = cast_source(&mut e, &case);
        for _ in 0..2 {
            give_mana(&mut e, 0, gift(&case.activation));
            apply_ability(&mut e, 0, source, 0, vec![])
                .unwrap_or_else(|error| panic!("{}: repeat activation: {error:?}", case.id));
            resolve_entire_stack_two_player(&mut e);
        }
        assert_eq!(
            e.effective_power(source),
            Some((case.body.0 as i32 + 2 * case.pump.0) as u32),
            "{}: two activations",
            case.id
        );
        assert_eq!(
            e.effective_toughness(source),
            Some((case.body.1 as i32 + 2 * case.pump.1) as u32),
            "{}: two activations",
            case.id
        );
    }
}

#[test]
fn flying_self_pump_batch_cards_reject_activation_without_the_exact_cost() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_803_000 + index as u64);
        let source = cast_source(&mut e, &case);
        let before = (e.effective_power(source), e.effective_toughness(source));
        assert!(
            apply_ability(&mut e, 0, source, 0, vec![]).is_err(),
            "{}: no mana means no activation",
            case.id
        );
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            before,
            "{}: rejected activation leaves the body unchanged",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}", case.id);
    }
}

#[test]
fn flying_self_pump_batch_cards_expire_the_pump_at_cleanup() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_804_000 + index as u64);
        let source = cast_source(&mut e, &case);
        give_mana(&mut e, 0, gift(&case.activation));
        apply_ability(&mut e, 0, source, 0, vec![]).expect("activate");
        resolve_entire_stack_two_player(&mut e);
        end_active_turn(&mut e, 0);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: CR 514.2 restores the printed body",
            case.id
        );
    }
}

#[test]
fn flying_self_pump_batch_cards_block_ground_creatures_only_when_they_have_flying() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_805_000 + index as u64);
        let attacker = inject_creature_on_battlefield(&mut e, 0, case.id);
        let ground = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        e.apply_scenario_command(0, &primitive_yield())
            .expect("main1 to beginning of combat");
        pass_both_players(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare the flier");
        e.apply_scenario_command(0, &pass()).expect("active pass");
        e.apply_scenario_command(1, &pass()).expect("defender pass");
        assert!(
            e.apply_scenario_command(
                1,
                &declare_blockers(vec![BlockPair {
                    attacker_id: attacker,
                    blocker_id: ground,
                }]),
            )
            .is_err(),
            "{}: a ground creature cannot block a flier",
            case.id
        );
    }
}

#[test]
fn flying_self_pump_batch_cards_resolved_creature_stays_on_the_battlefield() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(9_806_000 + index as u64);
        let _source = cast_source(&mut e, &case);
        assert_eq!(
            count_card_id_in_graveyard(&e, 0, case.id),
            0,
            "{}: a resolved permanent is not in the graveyard",
            case.id
        );
        assert_eq!(e.state.players[1].life, 20, "{}", case.id);
        assert_eq!(
            e.state.objects[&_source].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
    }
}
