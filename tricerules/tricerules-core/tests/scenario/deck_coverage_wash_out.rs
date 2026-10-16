//! Wash Out chooses a color during resolution, then simultaneously returns that cohort.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::ProtectionQuality;
use tricerules_cards::{Color, ContinuousEffectKind, ControllerReference, EffectDuration, Keyword};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::{ChoiceKind, ResolutionChoiceDecision, SubmitResolutionChoice};

fn branch(index: u32) -> RuledCommand {
    branch_at(0, index)
}

fn branch_at(prompt_id: u64, index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: prompt_id,
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn setup() -> GameEngine {
    let deck = deck_with("island", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        26_100_801,
        &[10, 20, 30],
        20,
        Some(vec![deck.clone(), deck.clone(), deck]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast(engine: &mut GameEngine) -> RuledEventBatch {
    inject_card_into_hand(engine, 0, "wash_out");
    give_mana(
        engine,
        10,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "wash_out");
    engine
        .apply_scenario_command(10, &cast_spell(slot, vec![]))
        .unwrap();
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].card_id, "wash_out");
    assert!(engine.state.stack[0].targets.is_empty());
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    let mut batch = RuledEventBatch::default();
    for _ in 0..engine.state.players.len() {
        if engine.state.pending_resolution.is_some() {
            break;
        }
        batch = engine
            .apply_scenario_command(engine.state.priority_player_id(), &pass())
            .unwrap();
        if engine.state.pending_resolution.is_some() {
            break;
        }
    }
    batch
}

fn setup_with_blue_declared_commander() -> (GameEngine, u32) {
    use tricerules_core::EngineDeck;
    use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

    let empty = EngineDeck {
        mainboard: deck_with("island", &[]),
        commanders: Vec::new(),
    };
    let mut decks = vec![empty; 3];
    decks[1].commanders = vec!["kami_of_the_crescent_moon".into()];
    let mut engine = GameEngine::new_with_commander_decks(
        tricerules_cards::registry::global(),
        26_101_015,
        &[10, 20, 30],
        20,
        Some(decks),
        true,
    )
    .expect("three-player Commander fixture");
    advance_to_main1_from_game_start(&mut engine);

    let commander = engine.state.players[1].command_zone[0];
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(
            20,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: 20,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: "Kami of the Crescent Moon".into(),
                        zone: DevZone::Battlefield as i32,
                        ready: false,
                    })),
                })),
            },
        )
        .expect("move the exact declared commander from Command to the battlefield");
    assert_eq!(engine.state.objects[&commander].owner, 20);
    assert_eq!(engine.state.objects[&commander].zone, Zone::Battlefield);
    assert!(engine.state.players[1]
        .declared_commander_object_ids
        .contains(&commander));
    (engine, commander)
}

fn setup_with_two_partner_commander_owners() -> (GameEngine, Vec<u32>, Vec<u32>) {
    use tricerules_cards::CardRegistry;
    use tricerules_core::EngineDeck;
    use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

    // Both fixture commanders have Partner, so each player's test-only declaration is a legal
    // two-commander pair without adding a production card identity.
    const PARTNER_A: &str = r#"(
        id: "wash_out_test_partner_a",
        name: "Wash Out Test Partner A",
        face_id: "wash_out_test_partner_a",
        mana_cost: "{U}",
        supertypes: ["Legendary"],
        types: ["Creature"],
        power: 2,
        toughness: 2,
        keywords: [Partner],
    )"#;
    const PARTNER_B: &str = r#"(
        id: "wash_out_test_partner_b",
        name: "Wash Out Test Partner B",
        face_id: "wash_out_test_partner_b",
        mana_cost: "{U}",
        supertypes: ["Legendary"],
        types: ["Creature"],
        power: 2,
        toughness: 2,
        keywords: [Partner],
    )"#;

    let registry = Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(
            &[
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!(
                    "../../../tricerules-cards/data/generated/m/merfolk_of_the_pearl_trident.ron"
                ),
                include_str!("../../../tricerules-cards/data/wash_out.ron"),
                PARTNER_A,
                PARTNER_B,
            ],
            &[],
        )
        .expect("test-only Partner pair and Wash Out definitions compile"),
    ));
    let empty = EngineDeck {
        mainboard: deck_with("island", &[]),
        commanders: Vec::new(),
    };
    let mut decks = vec![empty; 3];
    let commanders = vec![
        "wash_out_test_partner_a".into(),
        "wash_out_test_partner_b".into(),
    ];
    decks[1].commanders = commanders.clone();
    decks[2].commanders = commanders;
    let mut engine = GameEngine::new_with_commander_decks(
        registry,
        26_101_016,
        &[10, 20, 30],
        20,
        Some(decks),
        true,
    )
    .expect("three-player fixture with two Partner commanders per owner");
    advance_to_main1_from_game_start(&mut engine);
    let p20_commanders = engine.state.players[1].command_zone.clone();
    let p30_commanders = engine.state.players[2].command_zone.clone();
    engine.enable_dev_commands();
    for (owner, player_index) in [(20, 1), (30, 2)] {
        for card_name in ["Wash Out Test Partner A", "Wash Out Test Partner B"] {
            engine
                .apply_scenario_command(
                    owner,
                    &RuledCommand {
                        cmd: Some(Cmd::DevCommand(DevCommand {
                            target_player_id: owner,
                            dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                                card_name: card_name.into(),
                                zone: DevZone::Battlefield as i32,
                                ready: false,
                            })),
                        })),
                    },
                )
                .expect("move an exact test Partner from Command to the battlefield");
        }
        let declared = &engine.state.players[player_index].declared_commander_object_ids;
        let expected = if owner == 20 {
            &p20_commanders
        } else {
            &p30_commanders
        };
        assert!(expected
            .iter()
            .all(|object_id| declared.contains(object_id)));
    }
    (engine, p20_commanders, p30_commanders)
}

fn ordered(objects: Vec<u32>) -> RuledCommand {
    ordered_at(0, objects)
}

fn ordered_at(prompt_id: u64, objects: Vec<u32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            resolution_prompt_id: prompt_id,
            chosen_object_ids: objects,
            ..Default::default()
        })),
    }
}

fn modify(engine: &mut GameEngine, object: u32, kind: ContinuousEffectKind) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(object),
        kind,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
}

fn set_fixture_control(engine: &mut GameEngine, object_id: u32, controller: i32, base: i32) {
    for player in &mut engine.state.players {
        player
            .battlefield
            .retain(|candidate| *candidate != object_id);
    }
    let controller_index = engine.state.player_idx(controller).unwrap();
    engine.state.players[controller_index]
        .battlefield
        .push(object_id);
    let object = engine.state.objects.get_mut(&object_id).unwrap();
    object.controller = controller;
    object.base_controller = base;
}

const SOURCE_DEPARTURE_TAIL_SPELL: &str = r#"(
  id: "wash_out_test_source_departure_tail",
  name: "Wash Out Source Departure Tail Fixture",
  face_id: "wash_out_test_source_departure_tail",
  mana_cost: "{3}{U}",
  types: ["Sorcery"],
  spell_effect: [],
  spell_program: Some((body: (steps: [
    (id: "choose_blue", kind: Choose(
      choice: "choose_blue",
      chooser: Controller,
      optional: false,
      branches: [
        (branch_id: "blue", presentation: Fallback, cost: None, requirement: Always,
         body: (steps: [
           (id: "return_blue", kind: Perform(ReturnAllToOwnersHand(kind: (
             kind: AnyPermanent, is_color: Some(Blue)
           )))),
         ], yields: [])),
      ],
      otherwise: (steps: [], yields: []),
    )),
    (id: "damage_tail", kind: Perform(DamageTarget(amount: 1, target: (kind: AnyPlayer)))),
  ], yields: []), target_bindings: [
    (instruction_id: "damage_tail", role: Target, target_group_index: 0),
  ])),
  targeting: Some((groups: [(min: 1, max: 1, prompt: "Choose a player", effect_indices: [])])),
)"#;

#[test]
fn wash_out_each_color_returns_only_matching_permanents_across_all_players() {
    let cards = [
        "serra_angel",
        "merfolk_of_the_pearl_trident",
        "scathe_zombies",
        "hill_giant",
        "grizzly_bears",
    ];
    for selected in 0..5 {
        let mut engine = setup();
        let objects = cards
            .into_iter()
            .enumerate()
            .map(|(index, card)| inject_creature_on_battlefield(&mut engine, index % 3, card))
            .collect::<Vec<_>>();
        let colorless = inject_permanent_on_battlefield(&mut engine, 2, "sol_ring");
        cast(&mut engine);
        engine
            .apply_scenario_command(10, &branch(selected as u32))
            .unwrap();
        for (index, oid) in objects.into_iter().enumerate() {
            assert_eq!(
                engine.state.objects[&oid].zone,
                if index == selected {
                    Zone::Hand
                } else {
                    Zone::Battlefield
                }
            );
            if index == selected {
                assert!(engine.state.players[index % 3].hand.contains(&oid));
            }
        }
        assert_eq!(engine.state.objects[&colorless].zone, Zone::Battlefield);
        assert!(engine.state.stack.is_empty());
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn wash_out_waits_for_a_selected_commanders_owner_before_moving_the_cohort() {
    let (mut engine, commander) = setup_with_blue_declared_commander();
    let ordinary_blue =
        inject_creature_on_battlefield(&mut engine, 2, "merfolk_of_the_pearl_trident");
    let commander_generation = engine
        .state
        .zone_change_generation
        .get(&commander)
        .copied()
        .unwrap_or(0);
    let ordinary_generation = engine
        .state
        .zone_change_generation
        .get(&ordinary_blue)
        .copied()
        .unwrap_or(0);

    let cast_batch = cast(&mut engine);
    assert!(
        find_resolution_choice(&cast_batch).is_some(),
        "Wash Out offers its color choice"
    );
    let blue_batch = engine
        .apply_scenario_command(10, &branch(1))
        .expect("the controller chooses blue");
    let replacement = find_resolution_choice(&blue_batch)
        .expect("a selected declared commander asks its owner before the move batch");

    assert_eq!(replacement.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!(replacement.deciding_player_id, 20);
    assert_eq!(replacement.resolution_branches.len(), 2);
    assert_eq!(replacement.resolution_branches[0].label, "Command zone");
    assert_eq!(replacement.resolution_branches[1].label, "Hand");
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        20
    );
    assert_eq!(engine.state.objects[&commander].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&ordinary_blue].zone, Zone::Battlefield);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&commander)
            .copied()
            .unwrap_or(0),
        commander_generation
    );
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&ordinary_blue)
            .copied()
            .unwrap_or(0),
        ordinary_generation
    );

    engine
        .apply_scenario_command(20, &branch(0))
        .expect("the commander owner chooses the Command zone");
    assert_eq!(engine.state.objects[&commander].zone, Zone::Command);
    assert!(engine.state.players[1].command_zone.contains(&commander));
    assert_eq!(engine.state.objects[&ordinary_blue].zone, Zone::Hand);
    assert!(engine.state.players[2].hand.contains(&ordinary_blue));
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&commander)
            .copied()
            .unwrap_or(0),
        commander_generation + 1
    );
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&ordinary_blue)
            .copied()
            .unwrap_or(0),
        ordinary_generation + 1
    );
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn unsummon_uses_the_shared_commander_return_replacement_before_moving_its_target() {
    let (mut engine, commander) = setup_with_blue_declared_commander();
    let commander_generation = engine
        .state
        .zone_change_generation
        .get(&commander)
        .copied()
        .unwrap_or(0);
    let caster = engine.state.priority_player_id();
    let caster_index = engine
        .state
        .players
        .iter()
        .position(|player| player.id == caster)
        .expect("the active priority holder is seated");
    inject_card_into_hand(&mut engine, caster_index, "unsummon");
    give_mana(
        &mut engine,
        caster,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, caster_index, "unsummon");
    engine
        .apply_scenario_command(
            caster,
            &cast_spell(
                slot,
                vec![tricerules_proto::ruled::v1::TargetRef {
                    object_id: commander,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
                    expected_zone_change_generation: None,
                }],
            ),
        )
        .expect("cast Unsummon at the exact declared Commander");

    let mut choice_batch = RuledEventBatch::default();
    for _ in 0..engine.state.players.len() {
        if engine.state.pending_resolution.is_some() {
            break;
        }
        let actor = engine.state.priority_player_id();
        choice_batch = engine
            .apply_scenario_command(actor, &pass())
            .expect("pass toward Unsummon's owner replacement choice");
    }
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the exact Commander owner receives the replacement choice");
    assert_eq!(pending.deciding_player, 20);
    assert_eq!(
        pending
            .continuation
            .stack()
            .expect("targeted return retains the resolving Unsummon")
            .item
            .card_id,
        "unsummon"
    );
    let replacement =
        find_resolution_choice(&choice_batch).expect("the owner replacement prompt is published");
    assert_eq!(replacement.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!(replacement.resolution_branches[0].label, "Command zone");
    assert_eq!(replacement.resolution_branches[1].label, "Hand");
    assert_eq!(engine.state.objects[&commander].zone, Zone::Battlefield);

    engine
        .apply_scenario_command(20, &branch_at(replacement.resolution_prompt_id, 1))
        .expect("the owner applies Unsummon's Hand destination");
    assert_eq!(engine.state.objects[&commander].zone, Zone::Hand);
    assert!(engine.state.players[1].hand.contains(&commander));
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&commander)
            .copied()
            .unwrap_or(0),
        commander_generation + 1
    );
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn twincast_wash_out_copy_has_an_independent_color_and_commander_pause() {
    let (mut engine, blue_commander) = setup_with_blue_declared_commander();
    let white_permanent = inject_permanent_on_battlefield(&mut engine, 2, "serra_angel");
    let original_controller = engine.state.priority_player_id();
    let original_controller_index = engine
        .state
        .players
        .iter()
        .position(|player| player.id == original_controller)
        .expect("the current priority holder is seated");
    let twincast_controller_index = (original_controller_index + 1) % engine.state.players.len();
    let twincast_controller = engine.state.players[twincast_controller_index].id;
    let commander_generation = engine
        .state
        .zone_change_generation
        .get(&blue_commander)
        .copied()
        .unwrap_or(0);

    // P20 casts Wash Out first. P10 then copies it with the real Twincast path; the copy has a
    // separate stack item and executes before the original.
    inject_card_into_hand(&mut engine, original_controller_index, "wash_out");
    give_mana(
        &mut engine,
        original_controller,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let original_slot = hand_index_for_card(&engine, original_controller_index, "wash_out");
    engine
        .apply_scenario_command(original_controller, &cast_spell(original_slot, vec![]))
        .expect("the active priority holder casts the original Wash Out");
    let original_id = engine.state.stack.last().expect("original is on stack").id;
    assert!(!engine.state.stack.last().unwrap().is_copy);

    for _ in 0..engine.state.players.len() {
        if engine.state.priority_player_id() == twincast_controller {
            break;
        }
        let actor = engine.state.priority_player_id();
        engine
            .apply_scenario_command(actor, &pass())
            .expect("pass priority to Twincast's controller");
    }
    assert_eq!(engine.state.priority_player_id(), twincast_controller);
    inject_card_into_hand(&mut engine, twincast_controller_index, "twincast");
    give_mana(
        &mut engine,
        twincast_controller,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let twincast_slot = hand_index_for_card(&engine, twincast_controller_index, "twincast");
    engine
        .apply_scenario_command(
            twincast_controller,
            &cast_spell(
                twincast_slot,
                vec![tricerules_proto::ruled::v1::TargetRef {
                    object_id: original_id,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
                    expected_zone_change_generation: None,
                }],
            ),
        )
        .expect("P10 casts Twincast at the original Wash Out");

    for _ in 0..12 {
        if engine.state.pending_resolution.is_some() {
            break;
        }
        let actor = engine.state.priority_player_id();
        engine
            .apply_scenario_command(actor, &pass())
            .expect("pass toward the copied Wash Out's color choice");
    }
    let copy_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the copy reaches its independent color choice");
    assert_eq!(copy_choice.deciding_player, twincast_controller);
    let parked_copy = copy_choice
        .continuation
        .stack()
        .expect("the copied Wash Out is parked with its own continuation");
    let copy_id = parked_copy.item.id;
    assert!(
        parked_copy.item.is_copy,
        "parked stack item: {:?}",
        (
            copy_id,
            parked_copy.item.card_id.as_str(),
            parked_copy.item.controller
        )
    );
    assert_eq!(parked_copy.item.controller, twincast_controller);
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].id, original_id);

    let blue_choice = engine
        .apply_scenario_command(twincast_controller, &branch(1))
        .expect("Twincast's controller independently selects Blue for the copy");
    let copy_commander_prompt = find_resolution_choice(&blue_choice)
        .expect("the copied blue cohort pauses for its declared Commander owner");
    assert_eq!(copy_commander_prompt.deciding_player_id, 20);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .continuation
            .stack()
            .unwrap()
            .item
            .id,
        copy_id
    );
    assert_eq!(
        engine.state.objects[&blue_commander].zone,
        Zone::Battlefield
    );

    engine
        .apply_scenario_command(
            20,
            &branch_at(copy_commander_prompt.resolution_prompt_id, 0),
        )
        .expect("P20 sends the copy's exact Commander to Command");
    assert_eq!(engine.state.stack.len(), 1);
    assert_eq!(engine.state.stack[0].id, original_id);
    assert!(!engine.state.stack[0].is_copy);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&blue_commander].zone, Zone::Command);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&blue_commander)
            .copied()
            .unwrap_or(0),
        commander_generation + 1
    );
    assert_eq!(
        engine.state.objects[&white_permanent].zone,
        Zone::Battlefield
    );

    for _ in 0..12 {
        if engine.state.pending_resolution.is_some() {
            break;
        }
        let actor = engine.state.priority_player_id();
        engine
            .apply_scenario_command(actor, &pass())
            .expect("pass toward the original Wash Out's separate color choice");
    }
    let original_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the original retains its own color choice after the copy completes");
    assert_eq!(original_choice.deciding_player, original_controller);
    let parked_original = original_choice
        .continuation
        .stack()
        .expect("the original has its own parked color-choice continuation");
    assert_eq!(parked_original.item.id, original_id);
    assert!(!parked_original.item.is_copy);
    let original_menu = engine
        .apply_scenario_command(original_controller, &branch(0))
        .expect("the original controller selects White independently");
    assert!(find_resolution_choice(&original_menu).is_none());
    assert_eq!(engine.state.objects[&white_permanent].zone, Zone::Hand);
    assert!(engine.state.players[2].hand.contains(&white_permanent));
    assert_eq!(engine.state.objects[&blue_commander].zone, Zone::Command);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn wash_out_finishes_each_apnap_commander_groups_order_and_destinations_before_the_next_owner() {
    let (mut engine, p20, p30) = setup_with_two_partner_commander_owners();
    let ordinary_blue =
        inject_creature_on_battlefield(&mut engine, 0, "merfolk_of_the_pearl_trident");
    let cast_batch = cast(&mut engine);
    assert!(find_resolution_choice(&cast_batch).is_some());

    let order20_batch = engine.apply_scenario_command(10, &branch(1)).unwrap();
    let order20 = find_resolution_choice(&order20_batch).unwrap();
    assert_eq!(order20.choice_kind(), ChoiceKind::SimultaneousEntryOrder);
    assert_eq!(order20.deciding_player_id, 20);
    assert_eq!(order20.candidate_object_ids.len(), 2);
    assert!(p20
        .iter()
        .all(|object_id| order20.candidate_object_ids.contains(object_id)));
    assert!(p20
        .iter()
        .chain(p30.iter())
        .chain([&ordinary_blue])
        .all(|object_id| engine.state.objects[object_id].zone == Zone::Battlefield));

    let p20_first = p20[1];
    let order20_answer = engine
        .apply_scenario_command(20, &ordered(vec![p20_first, p20[0]]))
        .unwrap();
    let first_destination = find_resolution_choice(&order20_answer).unwrap();
    assert_eq!(
        first_destination.choice_kind(),
        ChoiceKind::ResolutionBranch
    );
    assert_eq!(first_destination.deciding_player_id, 20);
    assert!(first_destination.prompt_text.contains("Partner B"));

    let p20_first_answer = engine.apply_scenario_command(20, &branch(0)).unwrap();
    assert!(p20
        .iter()
        .chain(p30.iter())
        .chain([&ordinary_blue])
        .all(|object_id| engine.state.objects[object_id].zone == Zone::Battlefield));
    let second_destination = find_resolution_choice(&p20_first_answer).unwrap();
    assert_eq!(second_destination.deciding_player_id, 20);
    assert!(second_destination.prompt_text.contains("Partner A"));

    let p20_second_answer = engine.apply_scenario_command(20, &branch(1)).unwrap();
    let order30 = find_resolution_choice(&p20_second_answer).unwrap();
    assert_eq!(order30.choice_kind(), ChoiceKind::SimultaneousEntryOrder);
    assert_eq!(order30.deciding_player_id, 30);
    assert_eq!(order30.candidate_object_ids.len(), 2);
    assert!(p30
        .iter()
        .all(|object_id| order30.candidate_object_ids.contains(object_id)));
    assert!(p20
        .iter()
        .chain(p30.iter())
        .chain([&ordinary_blue])
        .all(|object_id| engine.state.objects[object_id].zone == Zone::Battlefield));

    let order30_answer = engine
        .apply_scenario_command(30, &ordered(vec![p30[0], p30[1]]))
        .unwrap();
    let p30_first_answer = engine.apply_scenario_command(30, &branch(1)).unwrap();
    assert!(find_resolution_choice(&p30_first_answer)
        .unwrap()
        .prompt_text
        .contains("Partner B"));
    assert!(p20
        .iter()
        .chain(p30.iter())
        .chain([&ordinary_blue])
        .all(|object_id| engine.state.objects[object_id].zone == Zone::Battlefield));
    let final_batch = engine.apply_scenario_command(30, &branch(0)).unwrap();

    assert_eq!(engine.state.objects[&p20[1]].zone, Zone::Command);
    assert_eq!(engine.state.objects[&p20[0]].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&p30[0]].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&p30[1]].zone, Zone::Command);
    assert_eq!(engine.state.objects[&ordinary_blue].zone, Zone::Hand);
    let cohort = p20
        .iter()
        .chain(p30.iter())
        .copied()
        .chain([ordinary_blue])
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        final_batch
            .events
            .iter()
            .filter(|event| matches!(&event.ev,
                Some(Ev::PermanentMoved(moved)) if cohort.contains(&moved.object_id)))
            .count(),
        cohort.len(),
        "the frozen cohort commits in one response only after every owner's replacement settles"
    );
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(order30_answer
        .events
        .iter()
        .all(|event| !matches!(&event.ev,
        Some(Ev::PermanentMoved(moved)) if cohort.contains(&moved.object_id))));
}

#[test]
fn wash_out_departure_rebases_borrowed_commander_and_prunes_exact_stranded_exile() {
    let (mut engine, p20, p30) = setup_with_two_partner_commander_owners();
    let borrowed_commander = p30[1];
    // Keep P20's second Partner in Command so P20 never controls two copies of one legendary
    // name when the live P30 commander is borrowed.
    engine.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != p20[1]);
    engine.state.players[1].command_zone.push(p20[1]);
    engine.state.objects.get_mut(&p20[1]).unwrap().zone = Zone::Command;
    modify(
        &mut engine,
        p30[0],
        ContinuousEffectKind::Layer5SetColors(vec![Color::Red]),
    );
    let borrowed_generation = engine
        .state
        .zone_change_generation
        .get(&borrowed_commander)
        .copied()
        .unwrap_or(0);
    set_fixture_control(&mut engine, borrowed_commander, 20, 30);
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(borrowed_commander),
        kind: ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(20),
        },
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    let stranded = inject_creature_on_battlefield(&mut engine, 2, "merfolk_of_the_pearl_trident");
    set_fixture_control(&mut engine, stranded, 20, 20);
    let survivor = inject_creature_on_battlefield(&mut engine, 0, "merfolk_of_the_pearl_trident");
    let stranded_generation = engine
        .state
        .zone_change_generation
        .get(&stranded)
        .copied()
        .unwrap_or(0);

    let color_choice = cast(&mut engine);
    assert!(
        find_resolution_choice(&color_choice).is_some(),
        "pending={:?}, events={:?}, priority={}, passes={}",
        engine.state.pending_resolution.as_ref().map(|pending| (
            pending.deciding_player,
            pending.presentation.prompt.clone(),
            pending.presentation.choice_kind
        )),
        color_choice
            .events
            .iter()
            .map(|event| &event.ev)
            .collect::<Vec<_>>(),
        engine.state.priority_player_id(),
        engine.state.passes_since_stack_change,
    );
    let p20_choice = engine.apply_scenario_command(10, &branch(1)).unwrap();
    let first_owner_choice = find_resolution_choice(&p20_choice).unwrap();
    assert_eq!(first_owner_choice.deciding_player_id, 20);
    assert!(first_owner_choice.prompt_text.contains("Partner A"));

    let departure = engine.apply_scenario_command(20, &concede()).unwrap();
    assert!(engine.state.players[1].has_lost);
    assert!(!engine.state.objects.contains_key(&p20[0]));
    assert_eq!(engine.state.objects[&borrowed_commander].owner, 30);
    assert_eq!(engine.state.objects[&borrowed_commander].controller, 30);
    assert_eq!(
        engine.state.objects[&borrowed_commander].zone,
        Zone::Battlefield
    );
    assert!(engine.state.players[2]
        .battlefield
        .contains(&borrowed_commander));
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&borrowed_commander)
            .copied()
            .unwrap_or(0),
        borrowed_generation,
        "control reversion preserves the borrowed commander's exact battlefield incarnation"
    );
    assert_eq!(engine.state.objects[&stranded].owner, 30);
    assert_eq!(engine.state.objects[&stranded].zone, Zone::Exile);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&stranded)
            .copied()
            .unwrap_or(0),
        stranded_generation + 1,
        "an owner-surviving object still controlled by the departing player leaves under CR 800.4a"
    );
    let refreshed = find_resolution_choice(&departure)
        .expect("the surviving commander owner receives a refreshed replacement prompt");
    assert_eq!(refreshed.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!(refreshed.deciding_player_id, 30);
    assert!(refreshed.prompt_text.contains("Partner B"));

    engine
        .apply_scenario_command(30, &branch(0))
        .expect("the live borrowed commander's owner chooses the command zone");
    assert_eq!(
        engine.state.objects[&borrowed_commander].zone,
        Zone::Command
    );
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&borrowed_commander)
            .copied()
            .unwrap_or(0),
        borrowed_generation + 1
    );
    assert_eq!(engine.state.objects[&survivor].zone, Zone::Hand);
    assert!(engine.state.players[0].hand.contains(&survivor));
    assert_eq!(engine.state.objects[&stranded].zone, Zone::Exile);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn wash_out_departure_reduces_commander_order_to_singleton_before_destination_choice() {
    let (mut engine, p20, p30) = setup_with_two_partner_commander_owners();
    for object_id in &p30 {
        engine.state.players[2]
            .battlefield
            .retain(|candidate| candidate != object_id);
        engine.state.players[2].command_zone.push(*object_id);
        engine.state.objects.get_mut(object_id).unwrap().zone = Zone::Command;
    }

    let stranded = p20[1];
    set_fixture_control(&mut engine, stranded, 10, 10);
    let survivor = p20[0];
    let survivor_generation = engine
        .state
        .zone_change_generation
        .get(&survivor)
        .copied()
        .unwrap_or(0);
    let stranded_generation = engine
        .state
        .zone_change_generation
        .get(&stranded)
        .copied()
        .unwrap_or(0);

    let color_choice = cast(&mut engine);
    assert!(find_resolution_choice(&color_choice).is_some());
    let order_batch = engine
        .apply_scenario_command(10, &branch(1))
        .expect("the Wash Out controller chooses blue");
    let order_prompt = find_resolution_choice(&order_batch)
        .expect("P20 must order both of their blue declared Commanders");
    assert_eq!(
        order_prompt.choice_kind(),
        ChoiceKind::SimultaneousEntryOrder
    );
    assert_eq!(order_prompt.deciding_player_id, 20);
    assert_eq!(order_prompt.candidate_object_ids.len(), 2);
    assert!(p20
        .iter()
        .all(|object_id| order_prompt.candidate_object_ids.contains(object_id)));
    let old_prompt_id = order_prompt.resolution_prompt_id;

    let departure = engine
        .apply_scenario_command(10, &concede())
        .expect("the departing controller's cleanup refreshes the return plan");
    assert!(engine.state.players[0].has_lost);
    assert_eq!(engine.state.objects[&stranded].zone, Zone::Exile);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&stranded)
            .copied()
            .unwrap_or(0),
        stranded_generation + 1,
        "the surviving owner's commander controlled by the departing player leaves exactly once"
    );
    assert_eq!(engine.state.objects[&survivor].zone, Zone::Battlefield);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&survivor)
            .copied()
            .unwrap_or(0),
        survivor_generation,
        "the surviving commander's exact battlefield incarnation remains in the cohort"
    );

    let destination = find_resolution_choice(&departure)
        .expect("the singleton group advances directly to the surviving owner's destination");
    assert_eq!(destination.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!(destination.deciding_player_id, 20);
    assert!(destination.prompt_text.contains("Partner A"));
    assert!(!destination.prompt_text.contains("Partner B"));
    assert_eq!(destination.resolution_branches.len(), 2);
    assert_eq!(destination.resolution_branches[0].label, "Command zone");
    assert_eq!(destination.resolution_branches[1].label, "Hand");
    assert_eq!(destination.min, 1);
    assert_eq!(destination.max, 1);
    assert!(!destination.ordered);
    assert_ne!(destination.resolution_prompt_id, old_prompt_id);

    let before_stale = engine.diagnostic_snapshot().unwrap();
    assert!(engine
        .apply_scenario_command(20, &ordered_at(old_prompt_id, vec![survivor, stranded]),)
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before_stale);

    let final_batch = engine
        .apply_scenario_command(20, &branch_at(destination.resolution_prompt_id, 1))
        .expect("the live owner chooses Hand for the remaining commander");
    assert_eq!(engine.state.objects[&survivor].zone, Zone::Hand);
    assert!(engine.state.players[1].hand.contains(&survivor));
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&survivor)
            .copied()
            .unwrap_or(0),
        survivor_generation + 1
    );
    assert_eq!(engine.state.objects[&stranded].zone, Zone::Exile);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&stranded)
            .copied()
            .unwrap_or(0),
        stranded_generation + 1
    );
    assert_eq!(
        final_batch
            .events
            .iter()
            .filter(|event| matches!(
                &event.ev,
                Some(Ev::PermanentMoved(moved)) if moved.object_id == survivor
            ))
            .count(),
        1,
        "the surviving cohort member moves once when the batch commits"
    );
    assert!(final_batch.events.iter().all(|event| !matches!(
        &event.ev,
        Some(Ev::PermanentMoved(moved)) if moved.object_id == stranded
    )));
    let stranded_commander_choice = find_resolution_choice(&final_batch)
        .expect("the stranded declared Commander gets its separate post-return SBA choice");
    assert_eq!(stranded_commander_choice.deciding_player_id, 20);
    assert!(stranded_commander_choice.prompt_text.contains("Partner B"));
    assert_eq!(engine.state.objects[&stranded].zone, Zone::Exile);
    engine
        .apply_scenario_command(
            20,
            &branch_at(stranded_commander_choice.resolution_prompt_id, 1),
        )
        .expect("P20 leaves the already exiled Commander in Exile");
    assert_eq!(engine.state.objects[&stranded].zone, Zone::Exile);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&stranded)
            .copied()
            .unwrap_or(0),
        stranded_generation + 1
    );
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn wash_out_source_controller_departure_preserves_borrowed_commander_and_program_tail_once() {
    use tricerules_cards::CardRegistry;
    use tricerules_core::EngineDeck;
    use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone};

    let registry = Box::leak(Box::new(
        CardRegistry::from_chunks_and_tokens(
            &[
                include_str!("../../../tricerules-cards/data/island.ron"),
                include_str!("../../../tricerules-cards/data/kami_of_the_crescent_moon.ron"),
                SOURCE_DEPARTURE_TAIL_SPELL,
            ],
            &[],
        )
        .expect("test-only Choose plus resultless return and draw tail compiles"),
    ));
    let empty = EngineDeck {
        mainboard: deck_with("island", &[]),
        commanders: Vec::new(),
    };
    let mut decks = vec![empty; 3];
    for player in decks.iter_mut().skip(1) {
        player.commanders = vec!["kami_of_the_crescent_moon".into()];
    }
    let mut engine = GameEngine::new_with_commander_decks(
        registry,
        26_101_017,
        &[10, 20, 30],
        20,
        Some(decks),
        true,
    )
    .expect("three-player source-departure fixture");
    advance_to_main1_from_game_start(&mut engine);
    let p20_commander = engine.state.players[1].command_zone[0];
    let p30_commander = engine.state.players[2].command_zone[0];
    engine.enable_dev_commands();
    for (owner, commander) in [(20, p20_commander), (30, p30_commander)] {
        engine
            .apply_scenario_command(
                owner,
                &RuledCommand {
                    cmd: Some(Cmd::DevCommand(DevCommand {
                        target_player_id: owner,
                        dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                            card_name: "Kami of the Crescent Moon".into(),
                            zone: DevZone::Battlefield as i32,
                            ready: false,
                        })),
                    })),
                },
            )
            .expect("move the exact declared commander to the battlefield");
        assert!(
            engine.state.players[engine.state.player_idx(owner).unwrap()]
                .declared_commander_object_ids
                .contains(&commander)
        );
    }
    let p10_land = inject_permanent_on_battlefield(&mut engine, 0, "island");
    let borrowed_generation = engine
        .state
        .zone_change_generation
        .get(&p30_commander)
        .copied()
        .unwrap_or(0);
    set_fixture_control(&mut engine, p30_commander, 10, 30);
    let p20_life_before = engine.state.players[1].life;

    let source = inject_card_into_hand(&mut engine, 0, "wash_out_test_source_departure_tail");
    engine.state.players[0].mana_pool.blue = 1;
    engine.state.players[0].mana_pool.colorless = 3;
    let source_slot = hand_index_for_card(&engine, 0, "wash_out_test_source_departure_tail");
    engine
        .apply_scenario_command(
            10,
            &cast_spell(
                source_slot,
                vec![tricerules_proto::ruled::v1::TargetRef {
                    object_id: 20,
                    damage_amount: 0,
                    group_index: 0,
                    kind: tricerules_proto::ruled::v1::TargetRefKind::Player as i32,
                    expected_zone_change_generation: None,
                }],
            ),
        )
        .expect("cast the test-only Choose card");
    for actor in [10, 20, 30] {
        if engine.state.pending_resolution.is_some() {
            break;
        }
        engine
            .apply_scenario_command(actor, &pass())
            .expect("pass toward the source's Choose instruction");
    }
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("P10 chooses the test color")
            .deciding_player,
        10
    );
    let color_result = engine
        .apply_scenario_command(10, &branch(0))
        .expect("choose the one test color branch");
    let initial_owner_prompt = find_resolution_choice(&color_result)
        .expect("the frozen blue cohort asks P20 about their declared Commander first");
    assert_eq!(initial_owner_prompt.deciding_player_id, 20);
    let initial_prompt_id = initial_owner_prompt.resolution_prompt_id;

    let departure = engine
        .apply_scenario_command(10, &concede())
        .expect("the resolving source controller concedes during P20's Commander choice");
    let refreshed = find_resolution_choice(&departure)
        .expect("P20 receives a fresh prompt while the outer program remains parked");
    assert_eq!(refreshed.deciding_player_id, 20);
    assert_ne!(refreshed.resolution_prompt_id, initial_prompt_id);
    assert!(!engine.state.objects.contains_key(&source));
    assert!(!engine.state.objects.contains_key(&p10_land));
    assert_eq!(engine.state.objects[&p20_commander].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&p30_commander].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&p30_commander].controller, 30);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&p30_commander)
            .copied()
            .unwrap_or(0),
        borrowed_generation,
        "the exact borrowed Commander survives source-controller departure and control reverts"
    );

    engine
        .apply_scenario_command(20, &branch_at(refreshed.resolution_prompt_id, 1))
        .expect("P20 chooses Hand for their Commander");
    let p30_prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("P30's still-live borrowed Commander reaches its owner")
        .deciding_player;
    assert_eq!(p30_prompt, 30);
    engine
        .apply_scenario_command(30, &branch(0))
        .expect("P30 chooses Command for their borrowed Commander");

    assert_eq!(engine.state.objects[&p20_commander].zone, Zone::Hand);
    assert!(engine.state.players[1].hand.contains(&p20_commander));
    assert_eq!(engine.state.objects[&p30_commander].zone, Zone::Command);
    assert!(engine.state.players[2]
        .command_zone
        .contains(&p30_commander));
    assert_eq!(engine.state.players[1].life, p20_life_before - 1);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn wash_out_same_owner_departure_refresh_rejects_the_superseded_prompt_atomically() {
    let (mut engine, p20, p30) = setup_with_two_partner_commander_owners();
    for object_id in [p20[1], p30[1]] {
        engine.state.players.iter_mut().for_each(|player| {
            player
                .battlefield
                .retain(|candidate| *candidate != object_id)
        });
        let owner = engine.state.objects[&object_id].owner;
        let owner_index = engine.state.player_idx(owner).unwrap();
        engine.state.players[owner_index]
            .command_zone
            .push(object_id);
        engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Command;
    }
    let cast_batch = cast(&mut engine);
    assert!(find_resolution_choice(&cast_batch).is_some());
    let p20_batch = engine.apply_scenario_command(10, &branch(1)).unwrap();
    let first_prompt = find_resolution_choice(&p20_batch).unwrap();
    assert_eq!(first_prompt.deciding_player_id, 20);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player,
        20
    );
    let old_prompt_id = first_prompt.resolution_prompt_id;

    let p30_departure = engine.apply_scenario_command(30, &concede()).unwrap();
    let refreshed = find_resolution_choice(&p30_departure)
        .expect("P30's departure republishes P20's still-current replacement question");
    assert_eq!(refreshed.deciding_player_id, 20);
    assert_ne!(
        refreshed.resolution_prompt_id, old_prompt_id,
        "a same-owner cohort refresh supersedes that owner's old prompt ID"
    );
    let current_prompt_id = refreshed.resolution_prompt_id;
    assert!(p30
        .iter()
        .all(|object_id| !engine.state.objects.contains_key(object_id)));
    assert_eq!(engine.state.objects[&p20[0]].zone, Zone::Battlefield);

    let before_stale = engine.diagnostic_snapshot().unwrap();
    assert!(engine
        .apply_scenario_command(20, &branch_at(old_prompt_id, 0))
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before_stale);
    assert!(engine
        .apply_scenario_command(10, &branch_at(current_prompt_id, 0))
        .is_err());
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before_stale);

    engine
        .apply_scenario_command(20, &branch_at(current_prompt_id, 1))
        .expect("P20's current hand answer resumes the surviving cohort");
    assert_eq!(engine.state.objects[&p20[0]].zone, Zone::Hand);
    assert!(engine.state.players[1].hand.contains(&p20[0]));
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn wash_out_uses_current_colors_and_returns_untargetable_borrowed_and_token_cohort() {
    let mut engine = setup();
    let own = inject_creature_on_battlefield(&mut engine, 0, "merfolk_of_the_pearl_trident");
    let protected = inject_creature_on_battlefield(&mut engine, 1, "merfolk_of_the_pearl_trident");
    let borrowed = inject_creature_on_battlefield(&mut engine, 0, "merfolk_of_the_pearl_trident");
    engine.state.objects.get_mut(&borrowed).unwrap().owner = 30;
    let multicolor = inject_permanent_on_battlefield(&mut engine, 2, "sol_ring");
    let became_red = inject_creature_on_battlefield(&mut engine, 1, "merfolk_of_the_pearl_trident");
    let green = inject_creature_on_battlefield(&mut engine, 2, "grizzly_bears");
    let colorless = inject_permanent_on_battlefield(&mut engine, 1, "island");
    let token = inject_creature_on_battlefield(&mut engine, 2, "soldier_w_1_1");
    for kind in [
        ContinuousEffectKind::Layer6AddKeyword(Keyword::Hexproof),
        ContinuousEffectKind::Layer6AddKeyword(Keyword::Shroud),
        ContinuousEffectKind::Layer6AddProtection(ProtectionQuality::Color(Color::Blue)),
    ] {
        modify(&mut engine, protected, kind);
    }
    cast(&mut engine);
    // Color is read when the selected instruction executes, not when the spell is cast.
    modify(
        &mut engine,
        multicolor,
        ContinuousEffectKind::Layer5SetColors(vec![Color::Blue, Color::Green]),
    );
    modify(
        &mut engine,
        became_red,
        ContinuousEffectKind::Layer5SetColors(vec![Color::Red]),
    );
    modify(
        &mut engine,
        token,
        ContinuousEffectKind::Layer5SetColors(vec![Color::Blue]),
    );
    let batch = engine.apply_scenario_command(10, &branch(1)).unwrap();
    for (oid, owner) in [(own, 10), (protected, 20), (borrowed, 30), (multicolor, 30)] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Hand);
        assert!(engine
            .state
            .players
            .iter()
            .find(|p| p.id == owner)
            .unwrap()
            .hand
            .contains(&oid));
        assert!(batch.events.iter().any(|event| matches!(&event.ev,
            Some(Ev::PermanentMoved(moved)) if moved.object_id == oid && moved.controller_player_id == owner
        )));
    }
    assert!(!engine.state.players[0].hand.contains(&borrowed));
    for oid in [became_red, green, colorless] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Battlefield);
    }
    assert!(engine
        .state
        .objects
        .get(&token)
        .is_none_or(|object| object.zone != Zone::Hand));
    assert!(!engine
        .state
        .players
        .iter()
        .any(|player| player.hand.contains(&token)));
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn wash_out_snapshots_the_color_cohort_before_any_departure_changes_colors() {
    let mut engine = setup();
    let source = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let affected = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    modify(
        &mut engine,
        source,
        ContinuousEffectKind::Layer5SetColors(vec![Color::Blue]),
    );
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: Some(source),
        affected: AffectedScope::Single(affected),
        kind: ContinuousEffectKind::Layer5SetColors(vec![Color::Blue]),
        condition: None,
        duration: EffectDuration::WhileSourceOnBattlefield,
        timestamp: engine.state.command_index,
    });
    cast(&mut engine);
    engine.apply_scenario_command(10, &branch(1)).unwrap();
    for (index, oid) in [(0, source), (1, affected)] {
        assert_eq!(engine.state.objects[&oid].zone, Zone::Hand);
        assert!(engine.state.players[index].hand.contains(&oid));
    }
    assert!(engine.state.stack.is_empty());
}

#[test]
fn wash_out_paid_cast_and_color_choice_replay_deterministically() {
    let play = || {
        let mut engine = setup();
        inject_creature_on_battlefield(&mut engine, 1, "merfolk_of_the_pearl_trident");
        inject_permanent_on_battlefield(&mut engine, 2, "sol_ring");
        let first = cast(&mut engine);
        let second = engine.apply_scenario_command(10, &branch(1)).unwrap();
        assert!(engine.state.stack.is_empty());
        // GameState has tuple-keyed LKI maps, so JSON cannot serialize it as an object.
        // Compare public responses plus every player/object and the identity/history state.
        let objects = engine
            .state
            .objects
            .iter()
            .map(|(&oid, object)| (oid, serde_json::to_value(object).unwrap()))
            .collect::<std::collections::BTreeMap<_, _>>();
        let generations = engine
            .state
            .zone_change_generation
            .iter()
            .map(|(&oid, &generation)| (oid, generation))
            .collect::<std::collections::BTreeMap<_, _>>();
        (
            first,
            second,
            engine.initial_response_batch(),
            serde_json::to_value(&engine.state.players).unwrap(),
            objects,
            generations,
            engine.state.command_index,
            engine.state.turn_history.clone(),
        )
    };
    assert_eq!(play(), play());
}

#[test]
fn wash_out_paid_cast_offers_all_five_colors_even_when_no_permanents_match() {
    let mut engine = setup();
    let land = inject_permanent_on_battlefield(&mut engine, 1, "island");
    let batch = cast(&mut engine);
    let choice = find_resolution_choice(&batch).unwrap();
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!(choice.deciding_player_id, 10);
    assert_eq!((choice.min, choice.max), (1, 1));
    assert_eq!(choice.resolution_branches.len(), 5);
    for (index, color) in ["white", "blue", "black", "red", "green"]
        .into_iter()
        .enumerate()
    {
        let option = &choice.resolution_branches[index];
        assert_eq!(option.branch_index, index as u32);
        assert!(option.selectable);
        assert_eq!(
            option.label,
            format!("Return all {color} permanents to their owners' hands.")
        );
    }
    for (actor, command) in [
        (20, branch(0)),
        (10, branch(5)),
        (
            10,
            RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: 0,
                    decision: ResolutionChoiceDecision::Decline as i32,
                    ..Default::default()
                })),
            },
        ),
    ] {
        let before = format!("{:?}", engine.state);
        assert!(engine.apply_scenario_command(actor, &command).is_err());
        assert_eq!(format!("{:?}", engine.state), before);
    }
    engine.apply_scenario_command(10, &branch(0)).unwrap();
    assert_eq!(engine.state.objects[&land].zone, Zone::Battlefield);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}
