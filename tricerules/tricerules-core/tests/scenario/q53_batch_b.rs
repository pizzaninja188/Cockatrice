//! Actual-card scenarios for the queue-v53 batch B cohorts.
//!
//! Expectations are printed Oracle values. CR 602.2/118.3 govern activation and costs,
//! CR 115.1 governs the one creature target, CR 701.21a governs sacrifice, CR 701.9 governs
//! discard, CR 611.2c/613.4c govern until-end-of-turn penalties, and CR 120.3 governs damage.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::GameEngine;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{CostSelection, RuledCommand};

struct ShrinkCase {
    id: &'static str,
    mana: ManaGift,
    power: i32,
    toughness: i32,
    sacrifice_self: bool,
    taps_source: bool,
    discard: bool,
    food: bool,
    sacrifice_creature: bool,
}

fn shrink_cases() -> [ShrinkCase; 7] {
    [
        ShrinkCase {
            id: "aquus_steed",
            mana: ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
            power: -2,
            toughness: 0,
            sacrifice_self: false,
            taps_source: true,
            discard: false,
            food: false,
            sacrifice_creature: false,
        },
        ShrinkCase {
            id: "grandmother_sengir",
            mana: ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
            power: -1,
            toughness: -1,
            sacrifice_self: false,
            taps_source: true,
            discard: false,
            food: false,
            sacrifice_creature: false,
        },
        ShrinkCase {
            id: "plague_witch",
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
            power: -1,
            toughness: -1,
            sacrifice_self: false,
            taps_source: true,
            discard: true,
            food: false,
            sacrifice_creature: false,
        },
        ShrinkCase {
            id: "phyrexian_debaser",
            mana: ManaGift::default(),
            power: -2,
            toughness: -2,
            sacrifice_self: true,
            taps_source: true,
            discard: false,
            food: false,
            sacrifice_creature: false,
        },
        ShrinkCase {
            id: "nim_replica",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            power: -1,
            toughness: -1,
            sacrifice_self: true,
            taps_source: false,
            discard: false,
            food: false,
            sacrifice_creature: false,
        },
        ShrinkCase {
            id: "bog_naughty",
            mana: ManaGift {
                b: 1,
                c: 2,
                ..Default::default()
            },
            power: -3,
            toughness: -3,
            sacrifice_self: false,
            taps_source: false,
            discard: false,
            food: true,
            sacrifice_creature: false,
        },
        ShrinkCase {
            id: "plagued_rusalka",
            mana: ManaGift {
                b: 1,
                ..Default::default()
            },
            power: -1,
            toughness: -1,
            sacrifice_self: false,
            taps_source: false,
            discard: false,
            food: false,
            sacrifice_creature: true,
        },
    ]
}

struct DamageCase {
    id: &'static str,
    mana: ManaGift,
    creature_damage: u32,
    player_damage: u32,
}

fn damage_cases() -> [DamageCase; 3] {
    [
        DamageCase {
            id: "lunge",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            creature_damage: 2,
            player_damage: 2,
        },
        DamageCase {
            id: "hungry_flames",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            creature_damage: 3,
            player_damage: 2,
        },
        DamageCase {
            id: "shower_of_sparks",
            mana: ManaGift {
                r: 1,
                ..Default::default()
            },
            creature_damage: 1,
            player_damage: 1,
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
    let decks = Some(vec![
        deck_with("island", &[card]),
        deck_with("forest", &["grizzly_bears", "sol_ring"]),
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

fn player_target_in_group(pid: i32, group: u32) -> Vec<tricerules_proto::ruled::v1::TargetRef> {
    let mut target = target_player(pid);
    target[0].group_index = group;
    target
}

fn activation_with_targets(
    e: &GameEngine,
    source: u32,
    targets: Vec<tricerules_proto::ruled::v1::TargetRef>,
    selections: Vec<CostSelection>,
) -> RuledCommand {
    let mut command = activate_ability_for(e, source, 0, targets);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.cost_selections = selections;
    command
}

fn activation_with_costs(
    e: &GameEngine,
    source: u32,
    target: u32,
    selections: Vec<CostSelection>,
) -> RuledCommand {
    activation_with_targets(e, source, target_object(target), selections)
}

#[test]
fn q53_b_canary_simplest_aquus_steed_leaves_toughness_alone() {
    let case = &shrink_cases()[0];
    let mut e = engine(53_060_001, case.id);
    let source = inject_creature_on_battlefield(&mut e, 0, case.id);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(&mut e, 0, gift(&case.mana));
    let command = activation_with_costs(&e, source, target, vec![]);
    e.apply_scenario_command(0, &command)
        .unwrap_or_else(|error| panic!("activate {}: {error:?}", case.id));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(0), Some(2)),
        "Aquus Steed reduces power only"
    );
    assert!(e.state.objects[&source].tapped, "the tap cost was paid");
}

#[test]
fn q53_b_canary_hardest_bog_naughty_sacrifices_food_and_kills() {
    let case = &shrink_cases()[5];
    let mut e = engine(53_060_002, case.id);
    let source = inject_creature_on_battlefield(&mut e, 0, case.id);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let food = inject_permanent_on_battlefield(&mut e, 0, "food");
    give_mana(&mut e, 0, gift(&case.mana));
    let command =
        activation_with_costs(&e, source, target, vec![permanent_cost_selection(1, food)]);
    e.apply_scenario_command(0, &command)
        .unwrap_or_else(|error| panic!("activate {}: {error:?}", case.id));
    resolve_entire_stack_two_player(&mut e);
    assert!(
        !e.state.players[0].battlefield.contains(&food),
        "the Food was sacrificed"
    );
    assert_eq!(
        e.state.objects[&target].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0));
}

#[test]
fn q53_b_shrink_cards_apply_their_printed_values_and_costs() {
    for (index, case) in shrink_cases().iter().enumerate() {
        let mut e = engine(53_061_000 + index as u64, case.id);
        let source = inject_creature_on_battlefield(&mut e, 0, case.id);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let mut selections = Vec::new();
        let mut sacrificed_food = None;
        let mut sacrificed_creature = None;
        if case.food {
            let food = inject_permanent_on_battlefield(&mut e, 0, "food");
            sacrificed_food = Some(food);
            selections.push(permanent_cost_selection(1, food));
        }
        if case.sacrifice_creature {
            let other = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
            sacrificed_creature = Some(other);
            selections.push(permanent_cost_selection(1, other));
        }
        if case.discard {
            inject_card_into_hand(&mut e, 0, "grizzly_bears");
            let hand = hand_index_for_card(&e, 0, "grizzly_bears");
            selections.push(hand_cost_selection(2, hand as u32));
        }
        let hand_before = e.state.players[0].hand.len();
        give_mana(&mut e, 0, gift(&case.mana));
        let command = activation_with_costs(&e, source, target, selections);
        e.apply_scenario_command(0, &command)
            .unwrap_or_else(|error| panic!("activate {}: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: exact mana", case.id);
        if case.discard {
            assert_eq!(
                e.state.players[0].hand.len(),
                hand_before - 1,
                "{}: discarded one",
                case.id
            );
        }
        if case.sacrifice_self {
            assert_eq!(
                e.state.objects[&source].zone,
                tricerules_core::Zone::Graveyard,
                "{}: sacrificed itself",
                case.id
            );
        }
        if let Some(food) = sacrificed_food {
            assert!(
                e.state
                    .objects
                    .get(&food)
                    .is_none_or(|object| { object.zone == tricerules_core::Zone::Graveyard }),
                "{}: the Food was sacrificed",
                case.id
            );
        }
        if let Some(other) = sacrificed_creature {
            assert_eq!(
                e.state.objects[&other].zone,
                tricerules_core::Zone::Graveyard,
                "{}: the chosen creature was sacrificed",
                case.id
            );
        }
        if !case.sacrifice_self {
            if case.taps_source {
                assert!(e.state.objects[&source].tapped, "{}: tap cost", case.id);
            } else {
                assert!(!e.state.objects[&source].tapped, "{}: no tap cost", case.id);
            }
        }
        let expected_power = 2 + case.power;
        let expected_toughness = 2 + case.toughness;
        if expected_toughness <= 0 {
            assert_eq!(
                e.state.objects[&target].zone,
                tricerules_core::Zone::Graveyard,
                "{}: lethal penalty",
                case.id
            );
        } else {
            assert_eq!(
                (e.effective_power(target), e.effective_toughness(target)),
                (Some(expected_power as u32), Some(expected_toughness as u32)),
                "{}: printed penalty",
                case.id
            );
        }
    }
}

#[test]
fn q53_b_two_target_damage_cards_hit_both_printed_targets() {
    for (index, case) in damage_cases().iter().enumerate() {
        let mut e = engine(53_062_000 + index as u64, case.id);
        let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        let mut targets = target_object(creature);
        targets.extend(player_target_in_group(1, 1));
        e.apply_scenario_command(0, &cast_spell(slot, targets))
            .unwrap_or_else(|error| panic!("cast {}: {error:?}", case.id));
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: exact cost", case.id);
        resolve_entire_stack_two_player(&mut e);
        if case.creature_damage >= 2 {
            assert_eq!(
                e.state.objects[&creature].zone,
                tricerules_core::Zone::Graveyard,
                "{}: lethal creature damage",
                case.id
            );
        } else {
            assert_eq!(
                e.state.objects[&creature].damage, case.creature_damage,
                "{}: creature damage",
                case.id
            );
        }
        assert_eq!(
            e.state.players[1].life,
            20 - case.player_damage as i32,
            "{}: player damage",
            case.id
        );
        assert_eq!(e.state.players[0].life, 20, "{}: caster unchanged", case.id);
    }
}

#[test]
fn q53_b_two_target_damage_still_resolves_against_a_legal_player() {
    for (index, case) in damage_cases().iter().enumerate() {
        let mut e = engine(53_063_000 + index as u64, case.id);
        let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        let mut targets = target_object(creature);
        targets.extend(player_target_in_group(1, 1));
        e.apply_scenario_command(0, &cast_spell(slot, targets))
            .expect("cast two-target damage");
        inject_card_into_hand(&mut e, 0, "doom_blade");
        give_mana(
            &mut e,
            0,
            ManaGift {
                b: 1,
                c: 1,
                ..Default::default()
            },
        );
        let doom = hand_index_for_card(&e, 0, "doom_blade");
        e.apply_scenario_command(0, &cast_spell(doom, target_object(creature)))
            .expect("destroy the creature target in response");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&creature].zone,
            tricerules_core::Zone::Graveyard
        );
        assert_eq!(
            e.state.players[1].life,
            20 - case.player_damage as i32,
            "{}: the legal player target still takes damage",
            case.id
        );
    }
}

#[test]
fn q53_b_activated_shrink_rejects_invalid_targets_and_cost_sources() {
    // Noncreature target.
    let mut e = engine(53_064_001, "aquus_steed");
    let source = inject_creature_on_battlefield(&mut e, 0, "aquus_steed");
    let artifact = inject_permanent_on_battlefield(&mut e, 0, "sol_ring");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let mana_before = pool(&e);
    let command = activation_with_costs(&e, source, artifact, vec![]);
    assert!(e.apply_scenario_command(0, &command).is_err());
    assert_eq!(pool(&e), mana_before);
    assert_eq!(
        e.state.objects[&artifact].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        e.state.objects[&source].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(!e.state.objects[&source].tapped);

    // No target.
    let mut e = engine(53_064_002, "aquus_steed");
    let source = inject_creature_on_battlefield(&mut e, 0, "aquus_steed");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let mana_before = pool(&e);
    let command = activation_with_targets(&e, source, vec![], vec![]);
    assert!(e.apply_scenario_command(0, &command).is_err());
    assert_eq!(pool(&e), mana_before);
    assert_eq!(
        e.state.objects[&source].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(!e.state.objects[&source].tapped);

    // Bog Naughty without Food.
    let mut e = engine(53_064_003, "bog_naughty");
    let source = inject_creature_on_battlefield(&mut e, 0, "bog_naughty");
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let mana_before = pool(&e);
    let command = activation_with_costs(&e, source, target, vec![]);
    assert!(e.apply_scenario_command(0, &command).is_err());
    assert_eq!(pool(&e), mana_before);
    assert_eq!(
        e.state.objects[&source].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        e.state.objects[&target].zone,
        tricerules_core::Zone::Battlefield
    );

    // Bog Naughty cannot sacrifice a non-Food artifact.
    let mut e = engine(53_064_004, "bog_naughty");
    let source = inject_creature_on_battlefield(&mut e, 0, "bog_naughty");
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let artifact = inject_permanent_on_battlefield(&mut e, 0, "sol_ring");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let mana_before = pool(&e);
    let command = activation_with_costs(
        &e,
        source,
        target,
        vec![permanent_cost_selection(1, artifact)],
    );
    assert!(e.apply_scenario_command(0, &command).is_err());
    assert_eq!(pool(&e), mana_before);
    assert_eq!(
        e.state.objects[&source].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        e.state.objects[&artifact].zone,
        tricerules_core::Zone::Battlefield
    );

    // Plagued Rusalka cannot sacrifice an opponent's creature.
    let mut e = engine(53_064_005, "plagued_rusalka");
    let source = inject_creature_on_battlefield(&mut e, 0, "plagued_rusalka");
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let mana_before = pool(&e);
    let command = activation_with_costs(
        &e,
        source,
        target,
        vec![permanent_cost_selection(1, target)],
    );
    assert!(e.apply_scenario_command(0, &command).is_err());
    assert_eq!(pool(&e), mana_before);
    assert_eq!(
        e.state.objects[&source].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        e.state.objects[&target].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(2), Some(2)),
        "the rejected Rusalka activation leaves the target unchanged"
    );
}

#[test]
fn q53_b_bog_naughty_sacrifices_a_non_token_food() {
    let mut e = engine(53_065_001, "bog_naughty");
    let source = inject_creature_on_battlefield(&mut e, 0, "bog_naughty");
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let food = inject_permanent_on_battlefield(&mut e, 0, "spicy_oatmeal_pizza");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    let command =
        activation_with_costs(&e, source, target, vec![permanent_cost_selection(1, food)]);
    e.apply_scenario_command(0, &command)
        .expect("sacrifice a non-token Food");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&food].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        e.state.objects[&target].zone,
        tricerules_core::Zone::Graveyard
    );
}

#[test]
fn q53_b_two_target_damage_rejects_a_single_target() {
    for (index, case) in damage_cases().iter().enumerate() {
        for (label, target) in [
            ("creature only", target_object(9999)),
            ("player only", player_target_in_group(1, 1)),
        ] {
            let mut e = engine(53_066_000 + index as u64, case.id);
            let creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
            let target = if label == "creature only" {
                target_object(creature)
            } else {
                target
            };
            let card = inject_card_into_hand(&mut e, 0, case.id);
            give_mana(&mut e, 0, gift(&case.mana));
            let mana_before = pool(&e);
            let hand_before = e.state.players[0].hand.len();
            let slot = hand_index_for_card(&e, 0, case.id);
            assert!(
                e.apply_scenario_command(0, &cast_spell(slot, target))
                    .is_err(),
                "{}: {label} is insufficient",
                case.id
            );
            assert_eq!(pool(&e), mana_before, "{}: mana unchanged", case.id);
            assert_eq!(
                e.state.players[0].hand.len(),
                hand_before,
                "{}: card unchanged",
                case.id
            );
            assert_eq!(e.state.players[1].life, 20);
            assert_eq!(e.state.players[0].life, 20);
            assert_eq!(
                e.state.objects[&creature].zone,
                tricerules_core::Zone::Battlefield
            );
            assert_eq!(
                e.state.objects[&creature].damage, 0,
                "{}: no partial creature damage",
                case.id
            );
            assert!(
                e.state.players[0].hand.contains(&card),
                "{}: exact card object remains in hand",
                case.id
            );
        }
    }
}
