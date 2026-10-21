//! Currency Converter and Nautiloid Ship's source-incarnation linked Exile choices.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::registry;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, dev_command, ruled_command::Cmd, CostObjectRef, CostObjectRefs,
    CostSelection, DevCommand, DevMoveCard, DevZone, ResolutionChoiceDecision, TargetRef,
    TargetRefKind,
};

const CURRENCY_CONVERTER: &str = "currency_converter";
const NAUTILOID_SHIP: &str = "nautiloid_ship";

fn start_converter_multi_card_discard(seed: u64) -> (GameEngine, u32, u32, u32) {
    let mut engine =
        GameEngine::new(registry::global(), seed, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let converter = inject_permanent_on_battlefield(&mut engine, 0, CURRENCY_CONVERTER);
    let looting = inject_card_into_hand(&mut engine, 0, "faithless_looting");
    let land = inject_card_into_hand(&mut engine, 0, "forest");
    let nonland = inject_card_into_hand(&mut engine, 0, "hill_giant");
    inject_library_card(&mut engine, 0, "island");
    inject_library_card(&mut engine, 0, "plains");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, "faithless_looting");
    engine
        .apply_scenario_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Faithless Looting");
    pass_priority_round(&mut engine);
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![land, nonland]))
        .expect("discard the land and nonland together");
    assert_eq!(engine.state.objects[&looting].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&nonland].zone, Zone::Graveyard);
    (engine, converter, land, nonland)
}

fn pending_converter_discard_refs(
    engine: &GameEngine,
    converter: u32,
) -> std::collections::HashSet<(u32, u64)> {
    let pending_order = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("Currency Converter asks its controller to order both discard triggers");
    pending_order
        .candidates
        .iter()
        .filter(|trigger| trigger.source_permanent_id == converter)
        .map(|trigger| {
            let observed = trigger
                .trigger_context
                .observed_object
                .expect("each per-card discard trigger captures the discarded object");
            (observed.object_id, observed.zone_change_generation)
        })
        .collect()
}

fn currency_linked_program_key_and_offers(
    engine: &GameEngine,
    converter: u32,
) -> serde_json::Value {
    let snapshot = engine
        .diagnostic_snapshot()
        .expect("frozen Currency linked-choice snapshot");
    let continuation = &snapshot["state"]["pending_resolution"]["continuation"];
    assert_eq!(
        continuation["variant"].as_str(),
        Some("LinkedExileChoice"),
        "the linked object prompt owns the in-flight Program"
    );
    let value = &continuation["value"];
    let program = &value["stack"]["program"];
    assert!(
        program.is_object(),
        "the linked object continuation must retain the exact parked Program: {program}"
    );
    assert_eq!(
        program["owner"]["value"]["card_id"].as_str(),
        Some(CURRENCY_CONVERTER)
    );
    assert_eq!(
        program["owner"]["value"]["face_id"].as_str(),
        Some(CURRENCY_CONVERTER)
    );
    assert_eq!(
        program["owner"]["value"]["ability_path"][0].as_str(),
        Some("return_linked_card")
    );
    assert_eq!(
        program["in_flight"].as_array().map(Vec::len),
        Some(1),
        "the linked choice must retain the output slot being committed"
    );
    assert_eq!(program["in_flight"][0].as_u64(), Some(0));
    assert_eq!(
        program["frames"].as_array().map(Vec::len),
        Some(1),
        "the root Program cursor remains parked at its next instruction"
    );
    assert_eq!(program["frames"][0]["next_instruction"].as_u64(), Some(1));

    let key = &value["key"];
    assert_eq!(key["source_object_id"].as_u64(), Some(u64::from(converter)));
    assert_eq!(
        value["stack"]["item"]["source_permanent_id"].as_u64(),
        Some(u64::from(converter)),
        "the parked ability retains the physical Currency Converter as its source"
    );
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("pending linked choice")
            .presentation
            .source_object_id as u64,
        value["stack"]["item"]["id"]
            .as_u64()
            .expect("stack item id"),
        "the public offer names the exact resolving stack item"
    );
    assert_eq!(
        key["producer"]["card_id"].as_str(),
        Some(CURRENCY_CONVERTER)
    );
    assert_eq!(
        key["producer"]["face_id"].as_str(),
        Some(CURRENCY_CONVERTER)
    );
    assert_eq!(
        key["producer"]["ability_path"][0].as_str(),
        Some("discard_to_exile")
    );
    assert_eq!(key["ability_link_id"].as_str(), Some("exiled_cards"));
    assert_eq!(key["occurrence"].as_str(), Some("NativeOrTokenBase"));

    let offered = value["candidate_generations"]
        .as_array()
        .expect("the choice retains exact offered object generations");
    assert_eq!(offered.len(), 2);
    for (object_id, generation) in engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates
        .iter()
        .map(|object_id| {
            (
                *object_id,
                engine
                    .state
                    .zone_change_generation
                    .get(object_id)
                    .copied()
                    .unwrap_or(0),
            )
        })
    {
        let exact = serde_json::json!([object_id, generation.to_string()]);
        assert!(
            offered.contains(&exact),
            "the parked key retains candidate ({object_id}, {generation}): {offered:?}"
        );
    }

    serde_json::json!({
        "program": program,
        "key": key,
        "candidate_generations": offered,
    })
}

fn linked_choice_for_prompt(
    prompt_id: u64,
    chosen_object_ids: Vec<u32>,
) -> tricerules_proto::ruled::v1::RuledCommand {
    tricerules_proto::ruled::v1::RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(
            tricerules_proto::ruled::v1::SubmitResolutionChoice {
                resolution_prompt_id: prompt_id,
                chosen_object_ids,
                ..Default::default()
            },
        )),
    }
}

fn linked_delegate_for_prompt(
    prompt_id: u64,
    branch_index: u32,
) -> tricerules_proto::ruled::v1::RuledCommand {
    tricerules_proto::ruled::v1::RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(
            tricerules_proto::ruled::v1::SubmitResolutionChoice {
                resolution_prompt_id: prompt_id,
                decision: ResolutionChoiceDecision::SelectBranch as i32,
                selected_branch_index: branch_index,
                ..Default::default()
            },
        )),
    }
}

fn assert_rejected_atomically(
    engine: &mut GameEngine,
    actor: i32,
    command: &tricerules_proto::ruled::v1::RuledCommand,
    label: &str,
) {
    let before = engine
        .diagnostic_snapshot()
        .expect("capture state before rejected linked-choice answer");
    let seed = engine.state.seed;
    let command_index = engine.state.command_index;
    assert!(
        engine.apply_scenario_command(actor, command).is_err(),
        "{label}"
    );
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("capture state after rejected linked-choice answer"),
        before,
        "{label} must not alter the pending Program or published offer"
    );
    assert_eq!(
        engine.state.seed, seed,
        "{label} must not consume randomness"
    );
    assert_eq!(
        engine.state.command_index, command_index,
        "{label} must not advance the logged command index"
    );
}

fn resolve_converter_discard_triggers(
    engine: &mut GameEngine,
    converter: u32,
    land: u32,
    nonland: u32,
) {
    let mut exiled = std::collections::HashSet::new();
    for _ in 0..2 {
        answer_trigger_order_in_engine_order(engine);
        let top = engine.state.stack.last().expect("ordered discard trigger");
        assert_eq!(top.source_permanent_id, Some(converter));
        let expected = top
            .trigger_context
            .observed_object
            .expect("the stack trigger retains its discarded object")
            .object_id;
        assert!(
            [land, nonland].contains(&expected),
            "the trigger must refer to one of the two discarded cards"
        );
        assert!(exiled.insert(expected), "each trigger must resolve once");
        pass_priority_round(engine);
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .expect("the trigger asks whether to exile its discarded card")
                .presentation
                .choice_kind,
            ChoiceKind::ResolutionBranch
        );
        let deciding_player = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .deciding_player;
        engine
            .apply_scenario_command(
                deciding_player,
                &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
            )
            .expect("accept the optional linked-exile branch");
        assert_eq!(engine.state.objects[&expected].zone, Zone::Exile);
    }
    assert_eq!(exiled, [land, nonland].into_iter().collect());
}

fn currency_with_two_linked_cards(seed: u64) -> (GameEngine, u32, u32, u32) {
    let (mut engine, converter, land, nonland) = start_converter_multi_card_discard(seed);
    resolve_converter_discard_triggers(&mut engine, converter, land, nonland);
    (engine, converter, land, nonland)
}

fn three_player_currency_with_two_linked_cards(seed: u64) -> (GameEngine, u32, u32, u32) {
    currency_with_linked_cards_for_players(seed, &[0, 1, 2])
}

fn four_player_currency_with_two_linked_cards(seed: u64) -> (GameEngine, u32, u32, u32) {
    currency_with_linked_cards_for_players(seed, &[0, 1, 2, 3])
}

fn currency_with_linked_cards_for_players(
    seed: u64,
    player_ids: &[i32],
) -> (GameEngine, u32, u32, u32) {
    let mut engine = GameEngine::new(registry::global(), seed, player_ids, 20, None, true)
        .expect("new multiplayer game");
    advance_to_main1_from_game_start(&mut engine);
    // Keep the owner below cleanup hand size after the real looting/discard pipeline. Returning
    // the unused opening hand to the library is fixture setup, not a game-action substitute for
    // the linked producer path exercised below.
    let owner_index = engine.state.player_idx(0).expect("owner seat");
    let opening_hand = std::mem::take(&mut engine.state.players[owner_index].hand);
    for object_id in opening_hand {
        engine.state.players[owner_index]
            .library
            .push_back(object_id);
        engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Library;
    }
    let converter = inject_permanent_on_battlefield(&mut engine, 0, CURRENCY_CONVERTER);
    let _looting = inject_card_into_hand(&mut engine, 0, "faithless_looting");
    let land = inject_card_into_hand(&mut engine, 0, "forest");
    let nonland = inject_card_into_hand(&mut engine, 0, "hill_giant");
    inject_library_card(&mut engine, 0, "island");
    inject_library_card(&mut engine, 0, "plains");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );

    let looting_slot = hand_index_for_card(&engine, 0, "faithless_looting");
    engine
        .apply_scenario_command(0, &cast_spell(looting_slot, Vec::new()))
        .expect("cast Faithless Looting");
    pass_priority_round(&mut engine);
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![land, nonland]))
        .expect("discard the land and nonland together");
    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&nonland].zone, Zone::Graveyard);
    resolve_converter_discard_triggers(&mut engine, converter, land, nonland);
    (engine, converter, land, nonland)
}

fn activate_converter_return(engine: &mut GameEngine, converter: u32, chosen: u32) {
    let activation = activate_ability_for(engine, converter, 1, Vec::new());
    engine
        .apply_scenario_command(0, &activation)
        .expect("activate Currency Converter's linked-card return ability");
    pass_priority_round(engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the linked Exile choice is pending");
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::LinkedExileCards
    );
    assert!(pending.presentation.candidates.contains(&chosen));
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![chosen]))
        .expect("choose the exact linked Exile object");
}

fn answer_targeting_player(player_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id: player_id,
                group_index: 0,
                kind: TargetRefKind::Player as i32,
                ..Default::default()
            }],
        })),
    }
}

fn move_owned_card_with_dev(
    engine: &mut GameEngine,
    owner: i32,
    card: &str,
    zone: DevZone,
    ready: bool,
) {
    engine.enable_dev_commands();
    engine
        .apply_scenario_command(
            owner,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: owner,
                    dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                        card_name: registry::global().get(card).unwrap().name.clone(),
                        zone: zone as i32,
                        ready,
                    })),
                })),
            },
        )
        .expect("move the owned test card through the dev command");
}

fn enter_nautiloid_targeting(engine: &mut GameEngine, target_player_id: u32) -> u32 {
    inject_card_into_hand(engine, 0, NAUTILOID_SHIP);
    let ship = move_ready_to_battlefield(engine, 0, NAUTILOID_SHIP);
    engine
        .apply_scenario_command(0, &answer_targeting_player(target_player_id))
        .expect("choose the target player for Nautiloid Ship's ETB trigger");
    resolve_entire_stack_two_player(engine);
    ship
}

fn crew_nautiloid(engine: &mut GameEngine, ship: u32, creatures: &[u32]) {
    let generation = |object_id: u32| {
        engine
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0)
    };
    let selection = CostSelection {
        cost_index: 0,
        selection: Some(Selection::BattlefieldObjects(CostObjectRefs {
            objects: creatures
                .iter()
                .map(|object_id| CostObjectRef {
                    object_id: *object_id,
                    zone_change_generation: generation(*object_id),
                })
                .collect(),
        })),
    };
    let mut command = activate_ability_with_costs(ship, 0, Vec::new(), vec![selection]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation = generation(ship);
    engine
        .apply_scenario_command(0, &command)
        .expect("tap three power to crew Nautiloid Ship");
    resolve_entire_stack_two_player(engine);
    assert!(creatures
        .iter()
        .all(|creature| engine.state.objects[creature].tapped));
    assert!(engine.characteristics(ship).unwrap().is_creature());
}

fn advance_to_combat_damage(engine: &mut GameEngine, ship: u32) {
    engine
        .apply_scenario_command(0, &primitive_yield())
        .expect("begin combat");
    for _ in 0..12 {
        if engine.state.turn_step == tricerules_core::TurnStep::DeclareAttackers {
            break;
        }
        pass_priority_round(engine);
    }
    assert_eq!(
        engine.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers,
        "game did not reach attacker declaration"
    );
    engine
        .apply_scenario_command(0, &declare_attackers(vec![ship]))
        .expect("attack with the crewed Nautiloid Ship");
    for _ in 0..12 {
        if engine.state.turn_step == tricerules_core::TurnStep::CombatDamage {
            return;
        }
        pass_priority_round(engine);
    }
    panic!(
        "game did not reach combat damage: {:?}",
        engine.state.turn_step
    );
}

fn nautiloid_after_crewed_combat_hit(seed: u64) -> (GameEngine, u32, u32, u32, u32) {
    let mut engine =
        GameEngine::new(registry::global(), seed, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let linked_creature = inject_graveyard_card(&mut engine, 1, "storm_crow");
    let linked_noncreature = inject_graveyard_card(&mut engine, 1, "island");
    let ship = enter_nautiloid_targeting(&mut engine, 1);
    let crew_a = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let crew_b = inject_creature_on_battlefield(&mut engine, 0, "savannah_lions");
    crew_nautiloid(&mut engine, ship, &[crew_a, crew_b]);
    advance_to_combat_damage(&mut engine, ship);
    assert!(engine.characteristics(ship).unwrap().is_creature());
    assert_eq!(engine.state.players[1].life, 15);
    (engine, ship, crew_a, linked_creature, linked_noncreature)
}

#[test]
fn currency_converter_is_registered_with_its_exact_card_identity() {
    let card = registry::global()
        .get(CURRENCY_CONVERTER)
        .expect("Currency Converter must have a complete rules definition");

    assert_eq!(card.id, CURRENCY_CONVERTER);
    assert_eq!(card.name, "Currency Converter");
}

#[test]
fn nautiloid_ship_is_registered_with_its_exact_card_identity() {
    let card = registry::global()
        .get(NAUTILOID_SHIP)
        .expect("Nautiloid Ship must have a complete rules definition");

    assert_eq!(card.id, NAUTILOID_SHIP);
    assert_eq!(card.name, "Nautiloid Ship");
}

#[test]
fn nautiloid_ship_exiles_only_the_current_graveyard_of_its_target_player() {
    let mut engine =
        GameEngine::new(registry::global(), 20_261_109, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let owner_graveyard_card = inject_graveyard_card(&mut engine, 0, "forest");
    let targeted_creature = inject_graveyard_card(&mut engine, 1, "hill_giant");
    let targeted_noncreature = inject_graveyard_card(&mut engine, 1, "island");
    let ship = enter_nautiloid_targeting(&mut engine, 1);

    assert_eq!(
        engine.state.objects[&owner_graveyard_card].zone,
        Zone::Graveyard
    );
    assert_eq!(engine.state.objects[&targeted_creature].zone, Zone::Exile);
    assert_eq!(
        engine.state.objects[&targeted_noncreature].zone,
        Zone::Exile
    );
    assert!(engine.state.players[0]
        .graveyard
        .contains(&owner_graveyard_card));
    assert!(!engine.state.players[1]
        .graveyard
        .contains(&targeted_creature));
    assert!(!engine.state.players[1]
        .graveyard
        .contains(&targeted_noncreature));
    assert!(engine.state.players[1].exile.contains(&targeted_creature));
    assert!(engine.state.players[1]
        .exile
        .contains(&targeted_noncreature));
    assert_eq!(engine.state.objects[&ship].zone, Zone::Battlefield);
}

#[test]
fn nautiloid_ship_with_an_empty_target_graveyard_does_not_substitute_another_player() {
    let mut engine =
        GameEngine::new(registry::global(), 20_261_110, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let untargeted_card = inject_graveyard_card(&mut engine, 0, "forest");
    let ship = enter_nautiloid_targeting(&mut engine, 1);

    assert_eq!(engine.state.objects[&untargeted_card].zone, Zone::Graveyard);
    assert!(engine.state.players[1].exile.is_empty());
    assert_eq!(engine.state.objects[&ship].zone, Zone::Battlefield);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn nautiloid_ship_does_not_substitute_a_player_who_lost_before_resolution() {
    let mut engine =
        GameEngine::new(registry::global(), 20_261_111, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let active_player_card = inject_graveyard_card(&mut engine, 0, "forest");
    let lost_player_card = inject_graveyard_card(&mut engine, 1, "hill_giant");
    inject_card_into_hand(&mut engine, 0, NAUTILOID_SHIP);
    let ship = move_ready_to_battlefield(&mut engine, 0, NAUTILOID_SHIP);
    engine
        .apply_scenario_command(0, &answer_targeting_player(1))
        .expect("target the opponent's graveyard");
    engine.state.players[1].has_lost = true;

    for _ in 0..8 {
        if engine.state.stack.is_empty() {
            break;
        }
        pass_priority_round(&mut engine);
    }
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.objects[&active_player_card].zone,
        Zone::Graveyard
    );
    assert_eq!(
        engine.state.objects[&lost_player_card].zone,
        Zone::Graveyard
    );
    assert_eq!(engine.state.objects[&ship].zone, Zone::Battlefield);
}

#[test]
fn currency_converter_links_each_card_from_a_multi_card_discard_to_its_own_trigger() {
    let (mut engine, converter, land, nonland) = start_converter_multi_card_discard(20_261_101);
    let observed = pending_converter_discard_refs(&engine, converter);
    assert_eq!(
        observed,
        [
            (land, engine.state.zone_change_generation[&land]),
            (nonland, engine.state.zone_change_generation[&nonland]),
        ]
        .into_iter()
        .collect(),
        "each trigger must retain the identity of its own discard"
    );
    resolve_converter_discard_triggers(&mut engine, converter, land, nonland);
}

#[test]
fn currency_converter_does_not_link_a_discard_replaced_to_the_library() {
    let mut engine =
        GameEngine::new(registry::global(), 20_261_115, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let converter = inject_permanent_on_battlefield(&mut engine, 0, CURRENCY_CONVERTER);
    inject_permanent_on_battlefield(&mut engine, 0, "library_of_leng");
    let discarded = inject_card_into_hand(&mut engine, 0, "forest");
    inject_library_card(&mut engine, 0, "island");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );

    engine
        .apply_scenario_command(0, &activate_ability_for(&engine, converter, 0, Vec::new()))
        .expect("activate Currency Converter's draw-discard ability");
    pass_priority_round(&mut engine);
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("choose a card to discard")
            .presentation
            .choice_kind,
        ChoiceKind::HandCards
    );
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![discarded]))
        .expect("discard Forest");
    assert_eq!(
        engine
            .state
            .pending_resolution
            .as_ref()
            .expect("Library of Leng offers its replacement destination")
            .presentation
            .choice_kind,
        ChoiceKind::PrivateReplacement
    );
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![1]))
        .expect("replace the discard with putting Forest on top of the library");

    for _ in 0..6 {
        if engine.state.stack.is_empty()
            && engine.state.pending_triggers.is_empty()
            && engine.state.pending_trigger_order.is_none()
            && engine.state.pending_resolution.is_none()
        {
            break;
        }
        pass_priority_round(&mut engine);
    }
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&discarded].zone, Zone::Library);
    assert_eq!(engine.state.players[0].library.front(), Some(&discarded));

    engine.state.objects.get_mut(&converter).unwrap().tapped = false;
    engine.initial_response_batch();
    engine
        .apply_scenario_command(0, &activate_ability_for(&engine, converter, 1, Vec::new()))
        .expect("the linked-card return ability is legal even with no linked cards");
    pass_priority_round(&mut engine);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&discarded].zone, Zone::Library);
    assert!(battlefield_token_oids(&engine, 0, "treasure").is_empty());
    assert!(battlefield_token_oids(&engine, 0, "rogue_b_2_2").is_empty());
}

#[test]
fn currency_converter_returns_a_linked_land_to_its_owner_and_creates_treasure() {
    let (mut engine, converter, land, nonland) = currency_with_two_linked_cards(20_261_102);
    activate_converter_return(&mut engine, converter, land);

    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&nonland].zone, Zone::Exile);
    assert_eq!(battlefield_token_oids(&engine, 0, "treasure").len(), 1);
    assert!(battlefield_token_oids(&engine, 0, "rogue_b_2_2").is_empty());
}

#[test]
fn currency_converter_returns_a_linked_nonland_and_creates_a_rogue() {
    let (mut engine, converter, land, nonland) = currency_with_two_linked_cards(20_261_103);
    activate_converter_return(&mut engine, converter, nonland);

    assert_eq!(engine.state.objects[&nonland].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&land].zone, Zone::Exile);
    let rogues = battlefield_token_oids(&engine, 0, "rogue_b_2_2");
    assert_eq!(rogues.len(), 1);
    assert_eq!(engine.characteristics(rogues[0]).unwrap().power, Some(2));
    assert_eq!(
        engine.characteristics(rogues[0]).unwrap().toughness,
        Some(2)
    );
    assert!(battlefield_token_oids(&engine, 0, "treasure").is_empty());
}

#[test]
fn currency_converter_control_transfer_uses_the_exiled_cards_owner_for_its_type_result() {
    let (mut engine, converter, land, nonland) = currency_with_two_linked_cards(20_261_105);
    let previous_generation = engine
        .state
        .zone_change_generation
        .get(&converter)
        .copied()
        .unwrap_or(0);
    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != converter);
    engine.state.players[1].battlefield.push(converter);
    let source = engine.state.objects.get_mut(&converter).unwrap();
    source.controller = engine.state.players[1].id;
    source.base_controller = engine.state.players[1].id;
    engine.initial_response_batch();
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&converter)
            .copied()
            .unwrap_or(0),
        previous_generation
    );
    let current_priority = engine.state.priority_player_id();
    if current_priority != 1 {
        engine
            .apply_scenario_command(current_priority, &pass())
            .expect("active player passes priority to the new controller");
    }

    let activation = activate_ability_for(&engine, converter, 1, Vec::new());
    engine
        .apply_scenario_command(1, &activation)
        .expect("the new controller activates Currency Converter");
    pass_priority_round(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the new controller receives the linked-card choice");
    assert_eq!(pending.deciding_player, 1);
    assert!(pending.presentation.candidates.contains(&land));
    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![land]))
        .expect("choose the owner's linked land");

    assert_eq!(
        engine.state.objects[&land].owner,
        engine.state.players[0].id
    );
    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].graveyard.last(), Some(&land));
    assert_eq!(engine.state.objects[&nonland].zone, Zone::Exile);
    assert_eq!(battlefield_token_oids(&engine, 0, "treasure").len(), 0);
    let treasures = battlefield_token_oids(&engine, 1, "treasure");
    assert_eq!(treasures.len(), 1);
    assert_eq!(
        engine.state.objects[&treasures[0]].owner,
        engine.state.players[1].id
    );
}

#[test]
fn currency_converter_linked_choice_rejects_unrelated_player_fields_atomically() {
    let (mut engine, converter, land, _) = currency_with_two_linked_cards(20_261_121);
    let activation = activate_ability_for(&engine, converter, 1, Vec::new());
    engine
        .apply_scenario_command(0, &activation)
        .expect("activate Currency Converter");
    pass_priority_round(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("linked Exile choice");
    assert_eq!(pending.deciding_player, 0);
    assert!(pending.presentation.candidates.contains(&land));

    let mut malformed = submit_resolution_choice(vec![land]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = malformed.cmd.as_mut() else {
        unreachable!();
    };
    answer.chosen_player_ids.push(0);

    let before = engine
        .diagnostic_snapshot()
        .expect("state before malformed linked-card answer");
    let seed = engine.state.seed;
    let command_index = engine.state.command_index;
    assert!(
        engine.apply_scenario_command(0, &malformed).is_err(),
        "the linked-object prompt must reject an unrelated player selection"
    );
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("state after malformed linked-card answer"),
        before,
        "rejecting extra player fields must leave the full game state unchanged"
    );
    assert_eq!(engine.state.seed, seed);
    assert_eq!(engine.state.command_index, command_index);
}

#[test]
fn currency_converter_linked_program_rejects_every_unrelated_card_answer_field_atomically() {
    let (mut engine, converter, land, _) = currency_with_two_linked_cards(20_261_124);
    let activation = activate_ability_for(&engine, converter, 1, Vec::new());
    engine
        .apply_scenario_command(0, &activation)
        .expect("activate Currency Converter");
    pass_priority_round(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("mandatory Program linked-card choice");
    assert_eq!(pending.deciding_player, 0);
    assert!(pending.presentation.candidates.contains(&land));
    let prompt_id = pending.resolution_prompt_id.get();
    assert_ne!(prompt_id, 0);

    let mut malformed = linked_choice_for_prompt(prompt_id, vec![land]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = malformed.cmd.as_mut() else {
        unreachable!()
    };
    answer.decision = ResolutionChoiceDecision::Decline as i32;
    assert_rejected_atomically(&mut engine, 0, &malformed, "card menu rejects a decision");

    malformed = linked_choice_for_prompt(prompt_id, vec![land]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = malformed.cmd.as_mut() else {
        unreachable!()
    };
    answer.selected_branch_index = 1;
    assert_rejected_atomically(
        &mut engine,
        0,
        &malformed,
        "card menu rejects a branch index",
    );

    malformed = linked_choice_for_prompt(prompt_id, vec![land]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = malformed.cmd.as_mut() else {
        unreachable!()
    };
    answer.chosen_player_ids.push(1);
    assert_rejected_atomically(&mut engine, 0, &malformed, "card menu rejects player ids");

    malformed = linked_choice_for_prompt(prompt_id, vec![land]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = malformed.cmd.as_mut() else {
        unreachable!()
    };
    answer.cast_spell = Some(Default::default());
    assert_rejected_atomically(&mut engine, 0, &malformed, "card menu rejects cast data");

    malformed = linked_choice_for_prompt(prompt_id, vec![land]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = malformed.cmd.as_mut() else {
        unreachable!()
    };
    answer.chosen_combat_defender = Some(Default::default());
    assert_rejected_atomically(
        &mut engine,
        0,
        &malformed,
        "card menu rejects combat-defender data",
    );

    malformed = linked_choice_for_prompt(prompt_id, vec![land]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = malformed.cmd.as_mut() else {
        unreachable!()
    };
    answer.payment = Some(Default::default());
    assert_rejected_atomically(&mut engine, 0, &malformed, "card menu rejects payment data");

    malformed = linked_choice_for_prompt(prompt_id, vec![land]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = malformed.cmd.as_mut() else {
        unreachable!()
    };
    answer.restricted_mana.push(Default::default());
    assert_rejected_atomically(
        &mut engine,
        0,
        &malformed,
        "card menu rejects restricted-mana data",
    );

    malformed = linked_choice_for_prompt(prompt_id, vec![land]);
    let Some(Cmd::SubmitResolutionChoice(answer)) = malformed.cmd.as_mut() else {
        unreachable!()
    };
    answer.spell_cast_announcement = Some(Default::default());
    assert_rejected_atomically(
        &mut engine,
        0,
        &malformed,
        "card menu rejects spell-announcement data",
    );
}

#[test]
fn currency_converter_keeps_the_begun_linked_program_after_its_controller_concedes() {
    let (mut engine, converter, land, nonland) =
        three_player_currency_with_two_linked_cards(20_261_122);
    let source_generation = engine
        .state
        .zone_change_generation
        .get(&converter)
        .copied()
        .unwrap_or(0);
    let land_exile_generation = engine.state.zone_change_generation[&land];
    let nonland_exile_generation = engine.state.zone_change_generation[&nonland];

    end_active_turn(&mut engine, 0);
    assert!(
        engine.state.pending_resolution.is_none(),
        "the setup must leave the linked discard triggers completed before P1's turn: {:?}",
        engine.state.pending_resolution.as_ref().map(|pending| (
            pending.deciding_player,
            pending.presentation.choice_kind,
            pending.presentation.prompt.as_str(),
        ))
    );
    for _ in 0..3 {
        if engine.state.turn_step != tricerules_core::TurnStep::Cleanup {
            break;
        }
        pass_priority_round(&mut engine);
    }
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.active_player_id(), 1);
    inject_card_into_hand(&mut engine, 1, "confiscate");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            c: 4,
            ..Default::default()
        },
    );
    let confiscate_slot = hand_index_for_card(&engine, 1, "confiscate");
    engine
        .apply_scenario_command(1, &cast_spell(confiscate_slot, target_object(converter)))
        .expect("cast Confiscate on Currency Converter");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&converter].owner, 0);
    assert_eq!(engine.state.objects[&converter].controller, 1);
    assert_eq!(engine.state.objects[&land].owner, 0);
    assert_eq!(engine.state.objects[&nonland].owner, 0);

    let activation = activate_ability_for(&engine, converter, 1, Vec::new());
    engine
        .apply_scenario_command(1, &activation)
        .expect("B activates Currency Converter");
    pass_priority_round(&mut engine);
    let offered = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("B receives the original mandatory linked-card offer");
    assert_eq!(offered.deciding_player, 1);
    assert_eq!(
        offered.presentation.choice_kind,
        ChoiceKind::LinkedExileCards
    );
    assert_eq!(offered.presentation.min, 1);
    assert_eq!(offered.presentation.max, 1);
    assert!(offered.presentation.candidates.contains(&land));
    assert!(offered.presentation.candidates.contains(&nonland));
    let parked_resume_state = currency_linked_program_key_and_offers(&engine, converter);
    let snapshot = engine.diagnostic_snapshot().expect("frozen linked choice");
    let continuation = &snapshot["state"]["pending_resolution"]["continuation"]["value"];
    assert_eq!(
        continuation["key"]["source_object_id"].as_u64(),
        Some(u64::from(converter))
    );
    assert_eq!(
        continuation["key"]["source_zone_change"].as_str(),
        Some(source_generation.to_string().as_str())
    );
    assert_eq!(
        continuation["stack"]["item"]["controller"].as_i64(),
        Some(1),
        "the resolution remains attributed to its original controller"
    );
    let existing_object_ids = engine
        .state
        .objects
        .keys()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let next_object_id_before_resolution = engine.state.next_object_id;

    let concede_batch = engine
        .apply_scenario_command(1, &concede())
        .expect("the original controller concedes with the linked move begun");
    let mut resolution_events = concede_batch.events;
    assert!(engine.state.players[1].has_lost);
    let selector = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the begun linked Program must survive its controller leaving");
    assert_eq!(selector.deciding_player, 2);
    assert_eq!(
        selector.presentation.choice_kind,
        ChoiceKind::ResolutionBranch
    );
    assert_eq!(
        currency_linked_program_key_and_offers(&engine, converter),
        parked_resume_state,
        "concession delegation preserves the exact parked Program, linked key, and generation-bound offers"
    );
    let selector_prompt = selector.resolution_prompt_id.get();
    let selector_batch = engine
        .apply_scenario_command(
            2,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("the next live player selects a replacement chooser");
    resolution_events.extend(selector_batch.events.clone());
    assert!(
        selector_batch.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::Log(log))
                if log.text == "P2 chooses P0 to make P1's linked Exile choice."
        )),
        "the log identifies the live selector separately from the departed effect controller"
    );
    let delegated_offer = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the selected live delegate receives the original linked-card offer");
    assert_eq!(delegated_offer.deciding_player, 0);
    assert_eq!(
        delegated_offer.presentation.choice_kind,
        ChoiceKind::LinkedExileCards
    );
    assert_ne!(delegated_offer.resolution_prompt_id.get(), selector_prompt);
    assert_eq!(
        currency_linked_program_key_and_offers(&engine, converter),
        parked_resume_state,
        "delegate reselection preserves the exact Program cursor, result binding, linked key, and offered generations"
    );
    assert!(delegated_offer.presentation.candidates.contains(&land));
    assert!(delegated_offer.presentation.candidates.contains(&nonland));
    assert!(selector_batch.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::ResolutionChoiceRequired(choice)) if choice.deciding_player_id == 0
    )));

    let completed = engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![nonland]))
        .expect("A chooses its exact linked nonland card");
    resolution_events.extend(completed.events.clone());
    assert_eq!(engine.state.objects[&nonland].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].graveyard.last(), Some(&nonland));
    assert_eq!(engine.state.objects[&nonland].owner, 0);
    assert_eq!(
        engine.state.zone_change_generation[&nonland],
        nonland_exile_generation + 1,
        "the selected linked object moves exactly once into its owner's graveyard"
    );
    assert_eq!(engine.state.objects[&land].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&land].owner, 0);
    assert_eq!(
        engine.state.zone_change_generation[&land], land_exile_generation,
        "the unchosen linked object keeps its exact Exile incarnation"
    );
    assert_eq!(engine.state.objects[&converter].owner, 0);
    assert_eq!(engine.state.objects[&converter].controller, 0);
    assert_eq!(engine.state.objects[&converter].zone, Zone::Battlefield);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&converter)
            .copied()
            .unwrap_or(0),
        source_generation,
        "the source survives with its original incarnation after the control effect ends"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(
        !resolution_events
            .iter()
            .any(|event| matches!(&event.ev, Some(Ev::TokenCreated(_)))),
        "the delegate does not inherit a Rogue effect from the departed controller"
    );
    assert_eq!(
        engine.state.next_object_id, next_object_id_before_resolution,
        "the departed controller's token branch allocates no token object"
    );
    assert!(engine
        .state
        .objects
        .keys()
        .all(|object_id| existing_object_ids.contains(object_id)));
    assert!(engine.state.objects.values().all(|object| {
        !(object.is_token() && matches!(object.card_id.as_str(), "treasure" | "rogue_b_2_2"))
    }));
    let final_snapshot = engine
        .diagnostic_snapshot()
        .expect("completed linked choice");
    assert!(final_snapshot["state"]["pending_resolution"].is_null());
    assert!(final_snapshot["state"]["pending_replacement_event"].is_null());
    assert!(final_snapshot["state"]["pending_observer_return_batch"].is_null());
    assert!(final_snapshot["state"]["pending_aura_recipient"].is_null());
}

#[test]
fn currency_converter_program_continues_with_empty_cohort_when_source_and_candidates_owner_concede()
{
    let (mut engine, converter, land, nonland) =
        three_player_currency_with_two_linked_cards(20_261_125);
    let land_generation = engine.state.zone_change_generation[&land];
    let nonland_generation = engine.state.zone_change_generation[&nonland];

    end_active_turn(&mut engine, 0);
    for _ in 0..3 {
        if engine.state.turn_step != tricerules_core::TurnStep::Cleanup {
            break;
        }
        pass_priority_round(&mut engine);
    }
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.active_player_id(), 1);
    inject_card_into_hand(&mut engine, 1, "confiscate");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            c: 4,
            ..Default::default()
        },
    );
    let confiscate_slot = hand_index_for_card(&engine, 1, "confiscate");
    engine
        .apply_scenario_command(1, &cast_spell(confiscate_slot, target_object(converter)))
        .expect("B casts Confiscate on A's Currency Converter");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&converter].owner, 0);
    assert_eq!(engine.state.objects[&converter].controller, 1);

    engine
        .apply_scenario_command(1, &activate_ability_for(&engine, converter, 1, Vec::new()))
        .expect("B begins the exact mandatory linked Program");
    pass_priority_round(&mut engine);
    let offered = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("B receives the linked-card offer before A leaves");
    assert_eq!(offered.deciding_player, 1);
    assert_eq!(offered.presentation.candidates.len(), 2);
    assert!(offered.presentation.candidates.contains(&land));
    assert!(offered.presentation.candidates.contains(&nonland));
    let parked_resume_state = currency_linked_program_key_and_offers(&engine, converter);
    assert!(parked_resume_state["program"].is_object());
    let next_object_id_before_departure = engine.state.next_object_id;

    let departure = engine
        .apply_scenario_command(0, &concede())
        .expect("A leaves while B's linked Program choice is parked");

    assert!(engine.state.players[0].has_lost);
    assert!(
        !engine.state.objects.contains_key(&converter),
        "the departed physical source owner removes Currency Converter"
    );
    assert!(
        !engine.state.objects.contains_key(&land) && !engine.state.objects.contains_key(&nonland),
        "the departed owners remove both linked candidate objects"
    );
    assert_eq!(engine.state.zone_change_generation[&land], land_generation);
    assert_eq!(
        engine.state.zone_change_generation[&nonland],
        nonland_generation
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.next_object_id, next_object_id_before_departure);
    assert!(departure.events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::Log(log))
            if log.text.contains("No offered linked Exile card remains")
                && log.text.contains("empty cohort")
    )));
    assert!(departure
        .events
        .iter()
        .all(|event| !matches!(event.ev, Some(Ev::TokenCreated(_)))));
    assert!(battlefield_token_oids(&engine, 1, "treasure").is_empty());
    assert!(battlefield_token_oids(&engine, 1, "rogue_b_2_2").is_empty());
}

#[test]
fn currency_converter_reselects_a_four_player_delegate_with_stable_tombstones_and_prompt_ids() {
    let (mut engine, converter, land, nonland) =
        four_player_currency_with_two_linked_cards(20_261_123);
    let source_generation = engine
        .state
        .zone_change_generation
        .get(&converter)
        .copied()
        .unwrap_or(0);
    let land_exile_generation = engine.state.zone_change_generation[&land];
    let nonland_exile_generation = engine.state.zone_change_generation[&nonland];

    end_active_turn(&mut engine, 0);
    for _ in 0..3 {
        if engine.state.turn_step != tricerules_core::TurnStep::Cleanup {
            break;
        }
        pass_priority_round(&mut engine);
    }
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.active_player_id(), 1);
    inject_card_into_hand(&mut engine, 1, "confiscate");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            c: 4,
            ..Default::default()
        },
    );
    let confiscate_slot = hand_index_for_card(&engine, 1, "confiscate");
    engine
        .apply_scenario_command(1, &cast_spell(confiscate_slot, target_object(converter)))
        .expect("B casts Confiscate on Currency Converter");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&converter].controller, 1);

    engine
        .apply_scenario_command(1, &activate_ability_for(&engine, converter, 1, Vec::new()))
        .expect("B begins the linked Program");
    pass_priority_round(&mut engine);
    let initial_prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("B receives the original linked-card prompt");
    assert_eq!(initial_prompt.deciding_player, 1);
    assert_ne!(initial_prompt.resolution_prompt_id.get(), 0);
    let parked_resume_state = currency_linked_program_key_and_offers(&engine, converter);
    let next_object_id_before_resolution = engine.state.next_object_id;
    let mut resolution_events = Vec::new();

    let first_departure = engine
        .apply_scenario_command(1, &concede())
        .expect("B concedes with the linked Program parked");
    resolution_events.extend(first_departure.events.clone());
    assert!(engine.state.players[1].has_lost);
    let selector =
        find_resolution_choice(&first_departure).expect("C receives the delegate selector");
    assert_eq!(selector.deciding_player_id, 2);
    assert_ne!(selector.resolution_prompt_id, 0);
    let first_selector_prompt = selector.resolution_prompt_id;
    assert_eq!(
        selector
            .resolution_branches
            .iter()
            .map(|branch| (
                branch.branch_index,
                branch.label.as_str(),
                branch.selectable
            ))
            .collect::<Vec<_>>(),
        vec![
            (0, "P0 makes the choice", true),
            (1, "P2 makes the choice", true),
            (2, "P3 makes the choice", true),
        ]
    );
    assert_eq!(
        currency_linked_program_key_and_offers(&engine, converter),
        parked_resume_state,
        "the first selector sees the original exact Program and candidate generations"
    );

    let mut malformed = linked_delegate_for_prompt(first_selector_prompt, 2);
    let Some(Cmd::SubmitResolutionChoice(answer)) = malformed.cmd.as_mut() else {
        unreachable!()
    };
    answer.chosen_object_ids.push(land);
    assert_rejected_atomically(
        &mut engine,
        2,
        &malformed,
        "delegate menu rejects object ids",
    );

    malformed = linked_delegate_for_prompt(first_selector_prompt, 2);
    let Some(Cmd::SubmitResolutionChoice(answer)) = malformed.cmd.as_mut() else {
        unreachable!()
    };
    answer.chosen_player_ids.push(0);
    assert_rejected_atomically(
        &mut engine,
        2,
        &malformed,
        "delegate menu rejects player ids",
    );

    malformed = linked_delegate_for_prompt(first_selector_prompt, 2);
    let Some(Cmd::SubmitResolutionChoice(answer)) = malformed.cmd.as_mut() else {
        unreachable!()
    };
    answer.cast_spell = Some(Default::default());
    assert_rejected_atomically(
        &mut engine,
        2,
        &malformed,
        "delegate menu rejects cast data",
    );

    malformed = linked_delegate_for_prompt(first_selector_prompt, 2);
    let Some(Cmd::SubmitResolutionChoice(answer)) = malformed.cmd.as_mut() else {
        unreachable!()
    };
    answer.chosen_combat_defender = Some(Default::default());
    assert_rejected_atomically(
        &mut engine,
        2,
        &malformed,
        "delegate menu rejects combat-defender data",
    );

    malformed = linked_delegate_for_prompt(first_selector_prompt, 2);
    let Some(Cmd::SubmitResolutionChoice(answer)) = malformed.cmd.as_mut() else {
        unreachable!()
    };
    answer.payment = Some(Default::default());
    assert_rejected_atomically(
        &mut engine,
        2,
        &malformed,
        "delegate menu rejects payment data",
    );

    malformed = linked_delegate_for_prompt(first_selector_prompt, 2);
    let Some(Cmd::SubmitResolutionChoice(answer)) = malformed.cmd.as_mut() else {
        unreachable!()
    };
    answer.restricted_mana.push(Default::default());
    assert_rejected_atomically(
        &mut engine,
        2,
        &malformed,
        "delegate menu rejects restricted-mana data",
    );

    malformed = linked_delegate_for_prompt(first_selector_prompt, 2);
    let Some(Cmd::SubmitResolutionChoice(answer)) = malformed.cmd.as_mut() else {
        unreachable!()
    };
    answer.spell_cast_announcement = Some(Default::default());
    assert_rejected_atomically(
        &mut engine,
        2,
        &malformed,
        "delegate menu rejects spell-announcement data",
    );

    let first_delegation = engine
        .apply_scenario_command(2, &linked_delegate_for_prompt(first_selector_prompt, 2))
        .expect("C delegates the linked-card choice to D");
    resolution_events.extend(first_delegation.events.clone());
    let d_prompt =
        find_resolution_choice(&first_delegation).expect("D receives a new linked-card prompt");
    assert_eq!(d_prompt.deciding_player_id, 3);
    assert_ne!(d_prompt.resolution_prompt_id, 0);
    let d_linked_choice_prompt = d_prompt.resolution_prompt_id;
    assert_eq!(
        currency_linked_program_key_and_offers(&engine, converter),
        parked_resume_state
    );
    let stale_c_choice = linked_delegate_for_prompt(first_selector_prompt, 2);
    assert_rejected_atomically(
        &mut engine,
        2,
        &stale_c_choice,
        "C cannot replay the stale selector prompt",
    );
    let wrong_actor_choice = linked_choice_for_prompt(d_linked_choice_prompt, vec![land]);
    assert_rejected_atomically(
        &mut engine,
        2,
        &wrong_actor_choice,
        "C cannot answer D's prompt",
    );

    let second_departure = engine
        .apply_scenario_command(3, &concede())
        .expect("D concedes before choosing the linked card");
    resolution_events.extend(second_departure.events.clone());
    assert!(engine.state.players[3].has_lost);
    let refreshed_selector =
        find_resolution_choice(&second_departure).expect("C receives a fresh selector offer");
    assert_eq!(refreshed_selector.deciding_player_id, 2);
    assert_ne!(
        refreshed_selector.resolution_prompt_id,
        first_selector_prompt
    );
    assert_eq!(
        refreshed_selector
            .resolution_branches
            .iter()
            .map(|branch| (
                branch.branch_index,
                branch.label.as_str(),
                branch.selectable
            ))
            .collect::<Vec<_>>(),
        vec![
            (0, "P0 makes the choice", true),
            (1, "P2 makes the choice", true),
            (2, "Unavailable player", false),
        ],
        "D's departed slot remains a non-selectable tombstone instead of shifting C's index"
    );
    let stale_d_choice = linked_choice_for_prompt(d_linked_choice_prompt, vec![land]);
    assert_rejected_atomically(
        &mut engine,
        3,
        &stale_d_choice,
        "D cannot answer after conceding",
    );
    assert_rejected_atomically(
        &mut engine,
        2,
        &stale_c_choice,
        "C cannot replay the selector prompt superseded by D's departure",
    );
    assert_eq!(
        currency_linked_program_key_and_offers(&engine, converter),
        parked_resume_state,
        "reselection retains the exact same in-flight instruction and linked candidates"
    );

    let final_delegation = engine
        .apply_scenario_command(
            2,
            &linked_delegate_for_prompt(refreshed_selector.resolution_prompt_id, 0),
        )
        .expect("C delegates to A from the refreshed menu");
    resolution_events.extend(final_delegation.events.clone());
    let a_prompt = find_resolution_choice(&final_delegation)
        .expect("A receives the exact original linked-card offer");
    assert_eq!(a_prompt.deciding_player_id, 0);
    assert_eq!(a_prompt.choice_kind, ChoiceKind::LinkedExileCards as i32);
    assert_ne!(
        refreshed_selector.resolution_prompt_id,
        first_selector_prompt
    );
    assert_eq!(a_prompt.candidate_object_ids.len(), 2);
    assert!(a_prompt.candidate_object_ids.contains(&land));
    assert!(a_prompt.candidate_object_ids.contains(&nonland));
    let stale_d_after_reselection = linked_choice_for_prompt(d_linked_choice_prompt, vec![land]);
    assert_rejected_atomically(
        &mut engine,
        3,
        &stale_d_after_reselection,
        "a departed delegate cannot answer after another delegate is selected",
    );
    let stale_completion_prompt = a_prompt.resolution_prompt_id;
    let completion = engine
        .apply_scenario_command(
            0,
            &linked_choice_for_prompt(stale_completion_prompt, vec![land]),
        )
        .expect("A chooses the exact original land candidate");
    resolution_events.extend(completion.events.clone());
    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.zone_change_generation[&land],
        land_exile_generation + 1
    );
    assert_eq!(engine.state.objects[&nonland].zone, Zone::Exile);
    assert_eq!(
        engine.state.zone_change_generation[&nonland],
        nonland_exile_generation
    );
    assert_eq!(engine.state.objects[&converter].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&converter].owner, 0);
    assert_eq!(engine.state.objects[&converter].controller, 0);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&converter)
            .copied()
            .unwrap_or(0),
        source_generation
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine.state.next_object_id,
        next_object_id_before_resolution
    );
    assert!(resolution_events
        .iter()
        .all(|event| !matches!(event.ev, Some(Ev::TokenCreated(_)))));
    assert!(battlefield_token_oids(&engine, 0, "treasure").is_empty());
    assert!(battlefield_token_oids(&engine, 0, "rogue_b_2_2").is_empty());
    assert_rejected_atomically(
        &mut engine,
        0,
        &linked_choice_for_prompt(stale_completion_prompt, vec![land]),
        "A cannot replay the completed linked-card answer",
    );
}

#[test]
fn currency_converter_moves_selector_after_the_current_delegate_picker_concedes() {
    let (mut engine, converter, land, nonland) =
        four_player_currency_with_two_linked_cards(20_261_126);
    let source_generation = engine
        .state
        .zone_change_generation
        .get(&converter)
        .copied()
        .unwrap_or(0);

    end_active_turn(&mut engine, 0);
    for _ in 0..3 {
        if engine.state.turn_step != tricerules_core::TurnStep::Cleanup {
            break;
        }
        pass_priority_round(&mut engine);
    }
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.active_player_id(), 1);
    inject_card_into_hand(&mut engine, 1, "confiscate");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            c: 4,
            ..Default::default()
        },
    );
    let confiscate_slot = hand_index_for_card(&engine, 1, "confiscate");
    engine
        .apply_scenario_command(1, &cast_spell(confiscate_slot, target_object(converter)))
        .expect("B casts Confiscate on A's Currency Converter");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&converter].controller, 1);

    engine
        .apply_scenario_command(1, &activate_ability_for(&engine, converter, 1, Vec::new()))
        .expect("B begins the linked Program");
    pass_priority_round(&mut engine);
    let original_card_prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("B receives the original linked-card offer");
    assert_eq!(original_card_prompt.deciding_player, 1);
    let parked_resume_state = currency_linked_program_key_and_offers(&engine, converter);

    let controller_departure = engine
        .apply_scenario_command(1, &concede())
        .expect("B leaves while the linked Program is parked");
    assert!(engine.state.players[1].has_lost);
    let first_selector = find_resolution_choice(&controller_departure)
        .expect("C receives the first live delegate selector");
    assert_eq!(first_selector.deciding_player_id, 2);
    assert_ne!(first_selector.resolution_prompt_id, 0);
    assert_eq!(
        currency_linked_program_key_and_offers(&engine, converter),
        parked_resume_state
    );

    let selector_departure = engine
        .apply_scenario_command(2, &concede())
        .expect("C leaves while the delegate selector is active");
    assert!(engine.state.players[2].has_lost);
    let refreshed_selector = find_resolution_choice(&selector_departure)
        .expect("D receives the refreshed selector after C leaves");
    assert_eq!(refreshed_selector.deciding_player_id, 3);
    assert_ne!(refreshed_selector.resolution_prompt_id, 0);
    assert_eq!(
        refreshed_selector
            .resolution_branches
            .iter()
            .map(|branch| (
                branch.branch_index,
                branch.label.as_str(),
                branch.selectable
            ))
            .collect::<Vec<_>>(),
        vec![
            (0, "P0 makes the choice", true),
            (1, "Unavailable player", false),
            (2, "P3 makes the choice", true),
        ],
        "C's selector slot remains a tombstone after the current picker concedes"
    );
    assert_eq!(
        currency_linked_program_key_and_offers(&engine, converter),
        parked_resume_state,
        "both concessions preserve the original source incarnation and linked candidates"
    );
    assert_rejected_atomically(
        &mut engine,
        2,
        &linked_delegate_for_prompt(first_selector.resolution_prompt_id, 0),
        "C cannot replay the selector prompt after conceding",
    );

    let final_delegation = engine
        .apply_scenario_command(
            3,
            &linked_delegate_for_prompt(refreshed_selector.resolution_prompt_id, 0),
        )
        .expect("D delegates the pending choice to A");
    let a_prompt = find_resolution_choice(&final_delegation)
        .expect("A receives the original linked-card offer");
    assert_eq!(a_prompt.deciding_player_id, 0);
    assert_eq!(a_prompt.candidate_object_ids.len(), 2);
    assert!(a_prompt.candidate_object_ids.contains(&land));
    assert!(a_prompt.candidate_object_ids.contains(&nonland));
    let completion = engine
        .apply_scenario_command(
            0,
            &linked_choice_for_prompt(a_prompt.resolution_prompt_id, vec![land]),
        )
        .expect("A completes the linked choice after both selector departures");

    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&nonland].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&converter].owner, 0);
    assert_eq!(engine.state.objects[&converter].controller, 0);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&converter)
            .copied()
            .unwrap_or(0),
        source_generation
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert!(battlefield_token_oids(&engine, 0, "treasure").is_empty());
    assert!(
        battlefield_token_oids(&engine, 0, "rogue_b_2_2").is_empty(),
        "land result unexpectedly created Rogue: {:?}; completion events: {:?}",
        battlefield_token_oids(&engine, 0, "rogue_b_2_2"),
        completion.events
    );
}

#[test]
fn currency_converter_return_program_cannot_read_old_link_records_after_source_reentry() {
    let (mut engine, converter, land, nonland) = currency_with_two_linked_cards(20_261_127);
    let old_source_generation = engine
        .state
        .zone_change_generation
        .get(&converter)
        .copied()
        .unwrap_or(0);
    let old_linked_records = engine
        .diagnostic_snapshot()
        .expect("capture old source-incarnation links")["state"]["linked_exile_records"]
        .clone();
    let old_activation = activate_ability_for(&engine, converter, 1, Vec::new());
    engine
        .apply_scenario_command(0, &old_activation)
        .expect("activate before the physical source leaves and returns");
    let old_stack_item = engine
        .state
        .stack
        .last()
        .expect("the original activation waits on the Stack");
    assert_eq!(old_stack_item.source_permanent_id, Some(converter));
    assert_eq!(old_stack_item.source_zone_change, old_source_generation);

    // Exercise the engine's normal zone-change path through the logged DevCommand fixture rather
    // than mutating object zones, zone generations, or the library/battlefield indexes directly.
    move_owned_card_with_dev(
        &mut engine,
        0,
        CURRENCY_CONVERTER,
        DevZone::Graveyard,
        false,
    );
    move_owned_card_with_dev(
        &mut engine,
        0,
        CURRENCY_CONVERTER,
        DevZone::Battlefield,
        true,
    );
    assert_eq!(
        engine.state.zone_change_generation[&converter],
        old_source_generation + 2,
        "the same physical card has a new generation after leaving and returning"
    );
    assert_eq!(engine.state.objects[&converter].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&converter].controller, 0);

    pass_priority_round(&mut engine);
    let old_pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the begun old ability resolves using its frozen linked Exile key");
    assert_eq!(old_pending.deciding_player, 0);
    assert!(old_pending.presentation.candidates.contains(&land));
    assert!(old_pending.presentation.candidates.contains(&nonland));
    let old_offer_prompt_id = old_pending.resolution_prompt_id.get();
    assert_ne!(old_offer_prompt_id, 0);
    let old_offer_snapshot = engine
        .diagnostic_snapshot()
        .expect("capture the old activation's frozen key");
    let old_offer = &old_offer_snapshot["state"]["pending_resolution"]["continuation"]["value"];
    assert_eq!(
        old_offer["key"]["source_object_id"].as_u64(),
        Some(u64::from(converter))
    );
    assert_eq!(
        old_offer["key"]["source_zone_change"].as_str(),
        Some(old_source_generation.to_string().as_str()),
        "the old ability remains bound to the prior source incarnation"
    );
    assert_eq!(
        old_offer["stack"]["item"]["source_zone_change"].as_str(),
        Some(old_source_generation.to_string().as_str())
    );
    assert_eq!(
        old_linked_records, old_offer_snapshot["state"]["linked_exile_records"],
        "the normal zone moves leave old linked records stored under the old key"
    );

    let old_completion = engine
        .apply_scenario_command(
            0,
            &linked_choice_for_prompt(old_offer_prompt_id, vec![land]),
        )
        .expect("finish the begun old activation with its original linked candidate");
    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&nonland].zone, Zone::Exile);
    assert_eq!(battlefield_token_oids(&engine, 0, "treasure").len(), 1);
    assert!(battlefield_token_oids(&engine, 0, "rogue_b_2_2").is_empty());
    assert_eq!(
        old_completion
            .events
            .iter()
            .filter(|event| matches!(&event.ev, Some(Ev::TokenCreated(_))))
            .count(),
        1,
        "the old Program tail runs once after its linked choice"
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());

    let new_activation = activate_ability_for(&engine, converter, 1, Vec::new());
    engine
        .apply_scenario_command(0, &new_activation)
        .expect("activate the new Currency Converter incarnation");
    let new_stack_item = engine
        .state
        .stack
        .last()
        .expect("new activation is on the Stack");
    assert_eq!(new_stack_item.source_permanent_id, Some(converter));
    assert_eq!(
        new_stack_item.source_zone_change,
        old_source_generation + 2,
        "the new ability binds to the returned source incarnation"
    );
    let mut resolution_events = Vec::new();
    for _ in 0..engine.state.players.len() {
        if engine.state.stack.is_empty() && engine.state.pending_resolution.is_none() {
            break;
        }
        let actor = engine.state.priority_player_id();
        let batch = engine
            .apply_scenario_command(actor, &pass())
            .expect("pass priority through the new incarnation's Program");
        resolution_events.extend(batch.events);
    }

    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&nonland].zone, Zone::Exile);
    assert_eq!(
        engine
            .diagnostic_snapshot()
            .expect("capture new source-incarnation result")["state"]["linked_exile_records"],
        old_linked_records,
        "the old link remains stored under its old source key and is not rebound"
    );
    assert!(resolution_events.iter().all(|event| !matches!(
        &event.ev,
        Some(Ev::ResolutionChoiceRequired(_)) | Some(Ev::TokenCreated(_))
    )));
    assert_eq!(battlefield_token_oids(&engine, 0, "treasure").len(), 1);
    assert!(battlefield_token_oids(&engine, 0, "rogue_b_2_2").is_empty());
}

fn run_currency_saved_observer_stream(
    engine: &mut GameEngine,
    converter: u32,
    land: u32,
    nonland: u32,
    aura: u32,
    aura_target: u32,
) -> (
    Vec<(i32, RuledCommand)>,
    Vec<tricerules_proto::ruled::v1::RuledEventBatch>,
) {
    let mut stream = Vec::new();
    macro_rules! apply_recorded {
        ($actor:expr, $command:expr) => {{
            let actor = $actor;
            let command = $command;
            stream.push((actor, command.clone()));
            engine
                .apply_scenario_command(actor, &command)
                .expect("apply and record the accepted command")
        }};
    }

    let mut batches = Vec::new();
    let activation = activate_ability_for(engine, converter, 1, Vec::new());
    batches.push(apply_recorded!(1, activation));
    for _ in 0..engine.state.players.len() {
        let actor = engine.state.priority_player_id();
        batches.push(apply_recorded!(actor, pass()));
        if engine.state.pending_resolution.is_some() {
            break;
        }
    }
    let offered = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the actual return Program reaches its linked-card choice");
    assert_eq!(offered.deciding_player, 1);
    assert_eq!(
        offered.presentation.choice_kind,
        ChoiceKind::LinkedExileCards
    );
    assert!(offered.presentation.candidates.contains(&land));
    assert!(offered.presentation.candidates.contains(&nonland));
    let parked = currency_linked_program_key_and_offers(engine, converter);
    assert_eq!(
        parked["program"]["frames"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(
        parked["program"]["frames"][0]["next_instruction"].as_u64(),
        Some(1)
    );

    batches.push(apply_recorded!(1, concede()));
    assert!(engine.state.players[engine.state.player_idx(1).unwrap()].has_lost);
    let after_concede = engine
        .diagnostic_snapshot()
        .expect("inspect the suspended actual Program under the Aura return");
    let suspended =
        &after_concede["state"]["pending_observer_return_batch"]["suspended_resolution"];
    assert!(
        suspended.is_object(),
        "the actual linked Program is saved below the Aura return"
    );
    assert_eq!(
        suspended["continuation"]["value"]["stack"]["program"]["owner"]["value"]["card_id"]
            .as_str(),
        Some(CURRENCY_CONVERTER),
        "the observer batch retains Currency Converter's real Program"
    );
    let aura_choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the paired Aura return is above the saved Program");
    assert_eq!(
        aura_choice.presentation.choice_kind,
        ChoiceKind::AuraPermanent
    );
    assert_eq!(aura_choice.deciding_player, 0);
    assert_eq!(aura_choice.presentation.candidates, [aura_target]);

    let aura_prompt_id = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Aura target choice")
        .resolution_prompt_id
        .get();
    let mut aura_answer = submit_resolution_choice(vec![aura_target]);
    if let Some(Cmd::SubmitResolutionChoice(answer)) = aura_answer.cmd.as_mut() {
        answer.resolution_prompt_id = aura_prompt_id;
    }
    batches.push(apply_recorded!(0, aura_answer));
    let selector = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("restoration publishes a fresh delegate selector");
    assert_eq!(
        selector.presentation.choice_kind,
        ChoiceKind::ResolutionBranch
    );
    assert_eq!(selector.deciding_player, 2);
    assert_ne!(selector.resolution_prompt_id.get(), 0);
    let selector_prompt_id = selector.resolution_prompt_id.get();
    let mut delegate_answer = submit_resolution_decision(ResolutionChoiceDecision::SelectBranch);
    if let Some(Cmd::SubmitResolutionChoice(answer)) = delegate_answer.cmd.as_mut() {
        answer.resolution_prompt_id = selector_prompt_id;
    }
    batches.push(apply_recorded!(2, delegate_answer));
    let linked_offer = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("P0 receives a fresh offer from Currency's restored Program");
    assert_eq!(linked_offer.deciding_player, 0);
    assert_eq!(
        linked_offer.presentation.choice_kind,
        ChoiceKind::LinkedExileCards
    );
    assert!(linked_offer.presentation.candidates.contains(&land));
    assert!(linked_offer.presentation.candidates.contains(&nonland));
    assert_eq!(
        currency_linked_program_key_and_offers(engine, converter),
        parked,
        "Aura restoration and delegate selection preserve the exact Currency cursor, link key, and candidate generations"
    );
    let linked_prompt_id = linked_offer.resolution_prompt_id.get();
    batches.push(apply_recorded!(
        0,
        linked_choice_for_prompt(linked_prompt_id, vec![nonland])
    ));

    assert_eq!(engine.state.objects[&nonland].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&land].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&aura].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.objects[&aura].attached_to,
        Some(tricerules_core::state::AttachmentRecipient::Object(
            aura_target
        ))
    );
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(battlefield_token_oids(engine, 0, "treasure").len(), 0);
    assert_eq!(battlefield_token_oids(engine, 0, "rogue_b_2_2").len(), 0);
    assert!(
        batches.iter().all(|batch| batch
            .events
            .iter()
            .all(|event| !matches!(&event.ev, Some(Ev::TokenCreated(_))))),
        "the departed controller creates no tail token"
    );
    (stream, batches)
}

#[test]
fn currency_converter_full_saved_observer_accepted_stream_replays_exactly() {
    let (mut engine, converter, land, nonland) =
        three_player_currency_with_two_linked_cards(20_261_128);
    end_active_turn(&mut engine, 0);
    for _ in 0..3 {
        if engine.state.turn_step != tricerules_core::TurnStep::Cleanup {
            break;
        }
        pass_priority_round(&mut engine);
    }
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.active_player_id(), 1);
    inject_card_into_hand(&mut engine, 1, "confiscate");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            c: 4,
            ..Default::default()
        },
    );
    let confiscate_slot = hand_index_for_card(&engine, 1, "confiscate");
    engine
        .apply_scenario_command(1, &cast_spell(confiscate_slot, target_object(converter)))
        .expect("B casts Confiscate on A's Currency Converter");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&converter].owner, 0);
    assert_eq!(engine.state.objects[&converter].controller, 1);

    let watched = inject_permanent_on_battlefield(&mut engine, 1, "hill_giant");
    inject_card_into_hand(&mut engine, 0, "pacifism");
    move_owned_card_with_dev(&mut engine, 0, "pacifism", DevZone::Exile, false);
    let aura = engine
        .state
        .objects
        .values()
        .find(|object| {
            object.owner == 0 && object.card_id == "pacifism" && object.zone == Zone::Exile
        })
        .map(|object| object.id)
        .expect("move an actual Pacifism into Exile for the paired return observer");
    let aura_target = inject_permanent_on_battlefield(&mut engine, 2, "grizzly_bears");
    let aura_generation = engine.state.zone_change_generation[&aura];
    engine
        .state
        .active_event_observers
        .push(tricerules_core::state::ActiveEventObserver {
            watched: Some(tricerules_core::state::TriggerObjectRef {
                object_id: watched,
                zone_change_generation: engine
                    .state
                    .zone_change_generation
                    .get(&watched)
                    .copied()
                    .unwrap_or(0),
                controller_at_event: 1,
            }),
            matcher:
                tricerules_core::state::EventObserverMatcher::WhenWatchedObjectLeavesBattlefield,
            payload: tricerules_core::state::EventObserverPayload::ReturnExiledObject {
                exiled: tricerules_core::state::TriggerObjectRef {
                    object_id: aura,
                    zone_change_generation: aura_generation,
                    controller_at_event: 0,
                },
            },
        });

    // Clone the full accepted-command baseline before activation, not a parked continuation.
    let mut replay = engine.clone();
    let (accepted_stream, original_batches) = run_currency_saved_observer_stream(
        &mut engine,
        converter,
        land,
        nonland,
        aura,
        aura_target,
    );
    let replay_batches = accepted_stream
        .iter()
        .map(|(actor, command)| {
            replay
                .apply_scenario_command(*actor, command)
                .expect("replay the exact accepted actor/command stream")
        })
        .collect::<Vec<_>>();

    assert_eq!(
        original_batches, replay_batches,
        "every accepted command batch replays exactly"
    );
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap(),
        "the complete canonical engine state replays exactly"
    );
    assert_eq!(engine.state.seed, replay.state.seed);
    assert_eq!(engine.state.command_index, replay.state.command_index);
}

#[test]
fn currency_converter_declining_one_discard_trigger_does_not_exile_that_card() {
    let (mut engine, converter, land, nonland) = start_converter_multi_card_discard(20_261_106);
    answer_trigger_order_in_engine_order(&mut engine);
    answer_trigger_order_in_engine_order(&mut engine);
    let top = engine.state.stack.last().expect("ordered discard trigger");
    let declined = top
        .trigger_context
        .observed_object
        .expect("trigger retains its card")
        .object_id;
    assert!(top.source_permanent_id == Some(converter));
    assert!([land, nonland].contains(&declined));
    pass_priority_round(&mut engine);
    let chooser = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("optional discard branch")
        .deciding_player;
    engine
        .apply_scenario_command(
            chooser,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("decline the optional exile");
    assert_eq!(engine.state.objects[&declined].zone, Zone::Graveyard);

    pass_priority_round(&mut engine);
    let accepted = if declined == land { nonland } else { land };
    let chooser = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the other discard trigger remains on the stack")
        .deciding_player;
    engine
        .apply_scenario_command(
            chooser,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("accept the other discard trigger");
    assert_eq!(engine.state.objects[&accepted].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&declined].zone, Zone::Graveyard);
}

#[test]
fn currency_discard_to_exile_program_replays_accepted_commands_exactly() {
    let (mut engine, _converter, land, nonland) = start_converter_multi_card_discard(20_261_132);
    let mut replay = engine.clone();
    let mut accepted_stream = Vec::new();
    let mut original_batches = Vec::new();
    let discarded;
    {
        let mut record = |engine: &mut GameEngine, actor, command| {
            let batch = engine
                .apply_command(actor, &command)
                .expect("apply an accepted Currency discard Program command");
            accepted_stream.push((actor, command));
            original_batches.push(batch);
        };

        while let Some(pending) = engine.state.pending_trigger_order.as_ref() {
            let actor = pending.deciding_player;
            let trigger = pending.candidates[0].object_id;
            record(&mut engine, actor, submit_trigger_order(trigger));
        }
        let trigger = engine.state.stack.last().expect("ordered discard trigger");
        discarded = trigger
            .trigger_context
            .observed_object
            .expect("the trigger retains its exact discarded object")
            .object_id;
        assert!([land, nonland].contains(&discarded));

        let priority_count = engine
            .state
            .players
            .iter()
            .filter(|player| !player.has_lost)
            .count()
            .checked_sub(engine.state.passes_since_stack_change as usize)
            .expect("consistent priority-pass count");
        for _ in 0..priority_count {
            let actor = engine.state.priority_player_id();
            record(&mut engine, actor, pass());
        }
        let pending = engine
            .state
            .pending_resolution
            .as_ref()
            .expect("the discard Program suspends on its optional choice");
        assert_eq!(
            pending.presentation.choice_kind,
            ChoiceKind::ResolutionBranch
        );
        assert_eq!(pending.deciding_player, 0);
        let prompt_id = pending.resolution_prompt_id.get();
        let mut decision = submit_resolution_decision(ResolutionChoiceDecision::SelectBranch);
        if let Some(Cmd::SubmitResolutionChoice(answer)) = decision.cmd.as_mut() {
            answer.resolution_prompt_id = prompt_id;
        }
        record(&mut engine, 0, decision);
    }

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&discarded].zone, Zone::Exile);
    let left_in_graveyard = if discarded == land { nonland } else { land };
    assert_eq!(
        engine.state.objects[&left_in_graveyard].zone,
        Zone::Graveyard
    );

    let replay_batches = accepted_stream
        .iter()
        .map(|(actor, command)| {
            replay
                .apply_command(*actor, command)
                .expect("replay the same accepted Currency discard Program command")
        })
        .collect::<Vec<_>>();
    assert_eq!(original_batches, replay_batches);
    assert_eq!(
        engine.diagnostic_snapshot().unwrap(),
        replay.diagnostic_snapshot().unwrap()
    );
    assert_eq!(engine.state.seed, replay.state.seed);
    assert_eq!(engine.state.command_index, replay.state.command_index);
}

#[test]
fn currency_discard_to_exile_program_uses_lki_after_source_owner_concedes() {
    let mut engine = GameEngine::new(registry::global(), 20_261_133, &[0, 1, 2], 20, None, true)
        .expect("new three-player game");
    advance_to_main1_from_game_start(&mut engine);
    let owner_index = engine.state.player_idx(0).expect("source owner seat");
    let opening_hand = std::mem::take(&mut engine.state.players[owner_index].hand);
    for object_id in opening_hand {
        engine.state.players[owner_index]
            .library
            .push_back(object_id);
        engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Library;
    }
    engine.initial_response_batch();
    let converter = inject_permanent_on_battlefield(&mut engine, 0, CURRENCY_CONVERTER);

    // Give P1 control of P0's Converter through the ordinary control-changing spell. P1 then
    // discards P1-owned cards, so P0 can leave while the resolving object remains live.
    end_active_turn(&mut engine, 0);
    for _ in 0..3 {
        if engine.state.turn_step != tricerules_core::TurnStep::Cleanup {
            break;
        }
        pass_priority_round(&mut engine);
    }
    advance_to_main1_from_game_start(&mut engine);
    assert_eq!(engine.state.active_player_id(), 1);
    inject_card_into_hand(&mut engine, 1, "confiscate");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            c: 4,
            r: 1,
            ..Default::default()
        },
    );
    let confiscate_slot = hand_index_for_card(&engine, 1, "confiscate");
    engine
        .apply_scenario_command(1, &cast_spell(confiscate_slot, target_object(converter)))
        .expect("P1 takes control of Currency Converter");
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&converter].owner, 0);
    assert_eq!(engine.state.objects[&converter].controller, 1);

    inject_card_into_hand(&mut engine, 1, "faithless_looting");
    let land = inject_card_into_hand(&mut engine, 1, "forest");
    let nonland = inject_card_into_hand(&mut engine, 1, "hill_giant");
    inject_library_card(&mut engine, 1, "island");
    inject_library_card(&mut engine, 1, "plains");
    let looting_slot = hand_index_for_card(&engine, 1, "faithless_looting");
    engine
        .apply_scenario_command(1, &cast_spell(looting_slot, Vec::new()))
        .expect("P1 casts Faithless Looting");
    pass_priority_round(&mut engine);
    engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![land, nonland]))
        .expect("P1 discards two cards");
    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&nonland].zone, Zone::Graveyard);

    answer_trigger_order_in_engine_order(&mut engine);
    let trigger = engine.state.stack.last().expect("P1's discard trigger");
    let discarded = trigger
        .trigger_context
        .observed_object
        .expect("the resolving trigger keeps its exact discarded card")
        .object_id;
    assert!([land, nonland].contains(&discarded));
    let discarded_generation = engine.state.zone_change_generation[&discarded];
    for _ in 0..12 {
        if engine.state.pending_resolution.is_some() {
            break;
        }
        let actor = engine.state.priority_player_id();
        engine
            .apply_scenario_command(actor, &pass())
            .expect("pass until the discard Program publishes its optional choice");
    }
    let prompt = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("P1 receives the optional linked-Exile choice");
    assert_eq!(prompt.deciding_player, 1);
    assert_eq!(
        prompt.presentation.choice_kind,
        ChoiceKind::ResolutionBranch
    );

    engine
        .apply_scenario_command(0, &concede())
        .expect("P0 concedes while P1's trigger Program is suspended");
    assert!(engine.state.players[0].has_lost);
    assert!(
        !engine.state.objects.contains_key(&converter),
        "the departed source owner removes Currency Converter"
    );
    assert!(engine.state.objects.contains_key(&discarded));
    assert_eq!(engine.state.objects[&discarded].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.zone_change_generation[&discarded], discarded_generation,
        "source-owner departure leaves P1's exact discard incarnation available"
    );
    let resumed =
        engine.state.pending_resolution.as_ref().expect(
            "the already-begun discard ability keeps its choice after its source owner leaves",
        );
    assert_eq!(resumed.deciding_player, 1);
    assert_eq!(
        resumed.presentation.choice_kind,
        ChoiceKind::ResolutionBranch
    );
    engine
        .apply_scenario_command(
            1,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("P1 selects the still-applicable branch after P0 leaves");

    assert_eq!(engine.state.objects[&discarded].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&discarded].owner, 1);
    assert_eq!(
        engine.state.objects[&if discarded == land { nonland } else { land }].zone,
        Zone::Graveyard
    );
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn currency_converter_has_no_token_branch_when_no_linked_card_remains_in_exile() {
    let (mut engine, converter, land, nonland) = currency_with_two_linked_cards(20_261_107);
    activate_converter_return(&mut engine, converter, land);
    engine.state.objects.get_mut(&converter).unwrap().tapped = false;
    engine.initial_response_batch();
    activate_converter_return(&mut engine, converter, nonland);
    engine.state.objects.get_mut(&converter).unwrap().tapped = false;
    engine.initial_response_batch();

    engine
        .apply_scenario_command(0, &activate_ability_for(&engine, converter, 1, Vec::new()))
        .expect("activate with an empty linked Exile set");
    pass_priority_round(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(battlefield_token_oids(&engine, 0, "treasure").len(), 1);
    assert_eq!(battlefield_token_oids(&engine, 0, "rogue_b_2_2").len(), 1);
}

#[test]
fn stale_discard_trigger_cannot_exile_a_card_that_returned_to_its_graveyard() {
    let (mut engine, converter, land, nonland) = start_converter_multi_card_discard(20_261_108);
    let order = engine
        .state
        .pending_trigger_order
        .as_ref()
        .expect("both discard triggers wait for order");
    let first = order
        .candidates
        .iter()
        .find(|trigger| {
            trigger
                .trigger_context
                .observed_object
                .is_some_and(|observed| observed.object_id == nonland)
        })
        .expect("find nonland trigger");
    engine
        .apply_scenario_command(0, &submit_trigger_order(first.object_id))
        .expect("put the nonland trigger below the stale land trigger");
    assert!(engine.state.pending_trigger_order.is_none());
    assert_eq!(
        engine
            .state
            .stack
            .last()
            .and_then(|item| item.trigger_context.observed_object)
            .map(|observed| observed.object_id),
        Some(land),
        "the last remaining trigger is placed on top automatically"
    );

    let land_generation = engine.state.zone_change_generation[&land];
    move_owned_card_with_dev(&mut engine, 0, "forest", DevZone::Hand, false);
    move_owned_card_with_dev(&mut engine, 0, "forest", DevZone::Graveyard, false);
    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    assert!(engine.state.zone_change_generation[&land] > land_generation);

    pass_priority_round(&mut engine);
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.zone_change_generation[&land],
        land_generation + 2
    );

    pass_priority_round(&mut engine);
    let chooser = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the untouched nonland trigger remains selectable")
        .deciding_player;
    engine
        .apply_scenario_command(
            chooser,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("resolve the nonland discard trigger");
    assert_eq!(engine.state.objects[&land].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&nonland].zone, Zone::Exile);
    assert_eq!(
        engine.state.zone_change_generation[&land],
        land_generation + 2
    );
    assert_eq!(engine.state.objects[&converter].zone, Zone::Battlefield);
}

#[test]
fn currency_converter_rejects_a_stale_linked_exile_answer_without_consuming_the_choice() {
    let (mut engine, converter, land, nonland) = currency_with_two_linked_cards(20_261_104);
    let activation = activate_ability_for(&engine, converter, 1, Vec::new());
    engine
        .apply_scenario_command(0, &activation)
        .expect("activate Currency Converter");
    pass_priority_round(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("linked Exile choice");
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::LinkedExileCards
    );
    let selected_as_stale = pending.presentation.candidates[0];
    let other = if selected_as_stale == land {
        nonland
    } else {
        land
    };

    let before_unauthorized = format!("{:?}", engine.state);
    assert!(engine
        .apply_scenario_command(1, &submit_resolution_choice(vec![selected_as_stale]))
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before_unauthorized);
    let before_malformed = format!("{:?}", engine.state);
    assert!(engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![u32::MAX]))
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before_malformed);

    // Model an intervening state change after publication. The next logged answer must recheck
    // both the source zone and linked incarnation instead of trusting the old prompt candidate.
    engine.state.players[0]
        .exile
        .retain(|object_id| *object_id != selected_as_stale);
    engine.state.players[0].graveyard.push(selected_as_stale);
    engine
        .state
        .objects
        .get_mut(&selected_as_stale)
        .expect("linked card")
        .zone = Zone::Graveyard;
    let before_rejection = format!("{:?}", engine.state);
    assert!(engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![selected_as_stale]))
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before_rejection);
    assert_eq!(engine.state.objects[&other].zone, Zone::Exile);
}

#[test]
fn nautiloid_returns_only_a_linked_creature_on_combat_damage() {
    let (mut engine, _ship, _crew, linked_creature, linked_noncreature) =
        nautiloid_after_crewed_combat_hit(20_261_112);
    pass_priority_round(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Nautiloid's optional combat trigger offers its linked creature");
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::LinkedExileCards
    );
    assert_eq!(pending.presentation.min, 0);
    assert_eq!(pending.presentation.max, 1);
    assert_eq!(pending.presentation.candidates, [linked_creature]);
    assert!(!pending
        .presentation
        .candidates
        .contains(&linked_noncreature));
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![linked_creature]))
        .expect("return the linked creature");
    assert_eq!(
        engine.state.objects[&linked_creature].zone,
        Zone::Battlefield
    );
    assert_eq!(engine.state.objects[&linked_creature].controller, 0);
    assert_eq!(engine.state.objects[&linked_noncreature].zone, Zone::Exile);
}

#[test]
fn nautiloid_return_preserves_object_through_nested_etb_library_choice() {
    let mut engine =
        GameEngine::new(registry::global(), 20_261_115, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let solemn = inject_graveyard_card(&mut engine, 1, "solemn_simulacrum");
    let forest = inject_library_card(&mut engine, 0, "forest");
    let ship = enter_nautiloid_targeting(&mut engine, 1);

    assert_eq!(engine.state.objects[&solemn].zone, Zone::Exile);
    let crew_a = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let crew_b = inject_creature_on_battlefield(&mut engine, 0, "savannah_lions");
    crew_nautiloid(&mut engine, ship, &[crew_a, crew_b]);
    advance_to_combat_damage(&mut engine, ship);

    pass_priority_round(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Nautiloid's combat trigger offers the linked Solemn Simulacrum");
    assert_eq!(
        pending.presentation.choice_kind,
        ChoiceKind::LinkedExileCards
    );
    assert_eq!(pending.presentation.candidates, [solemn]);
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![solemn]))
        .expect("return the linked Solemn Simulacrum");

    // Returning the same physical object triggers its ETB search. Resolve that nested choice
    // before checking the final battlefield state, so this also exercises suspension/resume.
    assert_eq!(engine.state.objects[&solemn].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&solemn].controller, 0);
    pass_priority_round(&mut engine);
    let branch = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Solemn Simulacrum's optional ETB search branch");
    assert_eq!(
        branch.presentation.choice_kind,
        ChoiceKind::ResolutionBranch
    );
    assert_eq!(branch.deciding_player, 0);
    engine
        .apply_scenario_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("choose Solemn Simulacrum's ETB search");

    let search = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Solemn Simulacrum's basic-land search choice");
    assert_eq!(search.presentation.choice_kind, ChoiceKind::LibrarySearch);
    assert!(search.presentation.candidates.contains(&forest));
    engine
        .apply_scenario_command(0, &submit_resolution_choice(vec![forest]))
        .expect("put the chosen basic land onto the battlefield tapped");

    assert_eq!(engine.state.objects[&solemn].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&solemn].controller, 0);
    assert_eq!(engine.state.objects[&forest].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&forest].controller, 0);
    assert!(engine.state.objects[&forest].tapped);
    assert!(!engine.state.players[0].library.contains(&forest));
}

#[test]
fn nautiloid_combat_trigger_can_be_declined() {
    let (mut engine, _ship, _crew, linked_creature, linked_noncreature) =
        nautiloid_after_crewed_combat_hit(20_261_113);
    pass_priority_round(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Nautiloid's optional combat trigger offers its linked creature");
    assert_eq!(pending.presentation.candidates, [linked_creature]);
    engine
        .apply_scenario_command(0, &submit_resolution_choice(Vec::new()))
        .expect("decline to return an exiled creature");
    assert_eq!(engine.state.objects[&linked_creature].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&linked_noncreature].zone, Zone::Exile);
    assert!(engine.state.players[0]
        .battlefield
        .iter()
        .all(|object_id| *object_id != linked_creature));
}

#[test]
fn nautiloid_reentry_cannot_return_a_creature_linked_to_its_prior_incarnation() {
    let mut engine =
        GameEngine::new(registry::global(), 20_261_114, &[0, 1], 20, None, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let old_linked_creature = inject_graveyard_card(&mut engine, 1, "storm_crow");
    let ship = enter_nautiloid_targeting(&mut engine, 1);
    assert_eq!(engine.state.objects[&old_linked_creature].zone, Zone::Exile);
    let first_generation = engine.state.zone_change_generation[&ship];

    move_owned_card_with_dev(&mut engine, 0, NAUTILOID_SHIP, DevZone::Graveyard, false);
    move_owned_card_with_dev(&mut engine, 0, NAUTILOID_SHIP, DevZone::Battlefield, true);
    assert!(engine.state.zone_change_generation[&ship] > first_generation);
    engine
        .apply_scenario_command(0, &answer_targeting_player(1))
        .expect("choose the opponent again for the new entry trigger");
    resolve_entire_stack_two_player(&mut engine);

    let crew_a = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let crew_b = inject_creature_on_battlefield(&mut engine, 0, "savannah_lions");
    crew_nautiloid(&mut engine, ship, &[crew_a, crew_b]);
    advance_to_combat_damage(&mut engine, ship);
    pass_priority_round(&mut engine);

    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
    assert_eq!(engine.state.objects[&old_linked_creature].zone, Zone::Exile);
    assert!(engine.state.players[0]
        .battlefield
        .iter()
        .all(|object_id| *object_id != old_linked_creature));
}
