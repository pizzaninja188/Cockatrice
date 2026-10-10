//! Currency Converter and Nautiloid Ship's source-incarnation linked Exile choices.
use super::helpers::*;
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
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Faithless Looting");
    pass_priority_round(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![land, nonland]))
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
            .apply_command(
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

fn activate_converter_return(engine: &mut GameEngine, converter: u32, chosen: u32) {
    let activation = activate_ability_for(engine, converter, 1, Vec::new());
    engine
        .apply_command(0, &activation)
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
        .apply_command(0, &submit_resolution_choice(vec![chosen]))
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
        .apply_command(
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
        .apply_command(0, &answer_targeting_player(target_player_id))
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
        .apply_command(0, &command)
        .expect("tap three power to crew Nautiloid Ship");
    resolve_entire_stack_two_player(engine);
    assert!(creatures
        .iter()
        .all(|creature| engine.state.objects[creature].tapped));
    assert!(engine.characteristics(ship).unwrap().is_creature());
}

fn advance_to_combat_damage(engine: &mut GameEngine, ship: u32) {
    engine
        .apply_command(0, &primitive_yield())
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
        .apply_command(0, &declare_attackers(vec![ship]))
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
        .apply_command(0, &answer_targeting_player(1))
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
        .apply_command(0, &activate_ability_for(&engine, converter, 0, Vec::new()))
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
        .apply_command(0, &submit_resolution_choice(vec![discarded]))
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
        .apply_command(0, &submit_resolution_choice(vec![1]))
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
        .apply_command(0, &activate_ability_for(&engine, converter, 1, Vec::new()))
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
            .apply_command(current_priority, &pass())
            .expect("active player passes priority to the new controller");
    }

    let activation = activate_ability_for(&engine, converter, 1, Vec::new());
    engine
        .apply_command(1, &activation)
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
        .apply_command(1, &submit_resolution_choice(vec![land]))
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
        .apply_command(
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
        .apply_command(
            chooser,
            &submit_resolution_decision(ResolutionChoiceDecision::SelectBranch),
        )
        .expect("accept the other discard trigger");
    assert_eq!(engine.state.objects[&accepted].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&declined].zone, Zone::Graveyard);
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
        .apply_command(0, &activate_ability_for(&engine, converter, 1, Vec::new()))
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
        .apply_command(0, &submit_trigger_order(first.object_id))
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
        .apply_command(
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
        .apply_command(0, &activation)
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
        .apply_command(1, &submit_resolution_choice(vec![selected_as_stale]))
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before_unauthorized);
    let before_malformed = format!("{:?}", engine.state);
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![u32::MAX]))
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
        .apply_command(0, &submit_resolution_choice(vec![selected_as_stale]))
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
        .apply_command(0, &submit_resolution_choice(vec![linked_creature]))
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
        .apply_command(0, &submit_resolution_choice(vec![solemn]))
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
        .apply_command(
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
        .apply_command(0, &submit_resolution_choice(vec![forest]))
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
        .apply_command(0, &submit_resolution_choice(Vec::new()))
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
        .apply_command(0, &answer_targeting_player(1))
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
