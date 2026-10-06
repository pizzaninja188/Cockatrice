//! Exact Ascend from Avernus card behavior and its X-bound graveyard return.

use super::helpers::*;
use tricerules_cards::primitives::{CardTypeFilter, SpellEffectKind};
use tricerules_cards::{CardRegistry, ManaCost};
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{ruled_event::Ev, StackResolveDestination};

fn ascend_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[4, 9], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

#[test]
fn ascend_from_avernus_registers_its_exact_card_identity() {
    let face = CardRegistry::global()
        .get("ascend_from_avernus")
        .expect("complete Ascend from Avernus definition")
        .primary_face();

    assert_eq!(face.mana_cost, ManaCost::parse("{X}{W}{W}{W}").unwrap());
    assert_eq!(face.types, vec!["Sorcery".to_string()]);
    assert_eq!(face.spell_effect.len(), 2);
    let SpellEffectKind::ReturnAllGraveyardPermanentsWithManaValueXOrLess { filter } =
        &face.spell_effect[0]
    else {
        panic!("the first instruction returns the X-bounded permanent cohort");
    };
    let branches = filter
        .any_of
        .as_ref()
        .expect("Creature or Planeswalker filter");
    assert_eq!(branches.len(), 2);
    assert_eq!(branches[0].card_type, Some(CardTypeFilter::Creature));
    assert_eq!(branches[1].card_type, Some(CardTypeFilter::Planeswalker));
    for card_id in ["grizzly_bears", "jace_beleren"] {
        let candidate = CardRegistry::global().get(card_id).expect("candidate card");
        assert!(candidate.matches_zone_card_filter(filter));
    }
    assert_eq!(face.spell_effect[1], SpellEffectKind::ExileResolvingSpell);
}

#[test]
fn ascend_is_untargeted_and_rejects_extra_targets_atomically() {
    let mut engine = ascend_engine(20_261_005);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    inject_card_into_hand(&mut engine, 0, "ascend_from_avernus");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            w: 3,
            c: 1,
            ..Default::default()
        },
    );
    let hand_index = hand_index_for_card(&engine, 0, "ascend_from_avernus");
    let before = serde_json::to_vec(&engine.state).expect("serializable state");

    let result = engine.apply_command(4, &cast_spell_x(hand_index, target_object(target), 1));

    assert!(result.is_err(), "Ascend has no targets");
    assert_eq!(
        serde_json::to_vec(&engine.state).expect("serializable state"),
        before,
        "a forged target cannot pay for or alter the untargeted spell"
    );
}

#[test]
fn ascend_returns_each_eligible_card_at_the_cast_x_then_exiles_the_spell() {
    let mut engine = ascend_engine(20_261_006);
    let bear = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let planeswalker = inject_graveyard_card(&mut engine, 0, "jace_beleren");
    let too_large = inject_graveyard_card(&mut engine, 0, "hill_giant");
    let nonpermanent = inject_graveyard_card(&mut engine, 0, "opt");
    let other_player_card = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    let source = inject_card_into_hand(&mut engine, 0, "ascend_from_avernus");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            w: 3,
            c: 3,
            ..Default::default()
        },
    );
    assert!(engine.state.players[0].graveyard.contains(&bear));
    assert!(engine.state.players[0].graveyard.contains(&planeswalker));

    let hand_index = hand_index_for_card(&engine, 0, "ascend_from_avernus");
    semantic::accepted(&mut engine, 4, &cast_spell_x(hand_index, Vec::new(), 3));
    assert_eq!(engine.state.stack.last().unwrap().chosen_x, 3);
    let mut saw_exile_exit = false;
    let mut saw_simultaneous_entry_order = false;
    for _ in 0..2 {
        let actor = engine.state.priority_player_id();
        let batch = engine
            .apply_command(actor, &pass())
            .expect("pass after Ascend cast");
        saw_exile_exit |= batch.events.iter().any(|event| {
            matches!(
                event.ev,
                Some(Ev::StackResolved(ref resolved))
                    if resolved.object_id == source
                        && resolved.destination == StackResolveDestination::Exile as i32
            )
        });
        if engine
            .state
            .pending_resolution
            .as_ref()
            .is_some_and(|pending| {
                pending.presentation.choice_kind == ChoiceKind::SimultaneousEntryOrder
            })
        {
            saw_simultaneous_entry_order = true;
            for (_, _, choice_batch) in answer_simultaneous_entry_order_in_engine_order(&mut engine)
            {
                saw_exile_exit |= choice_batch.events.iter().any(|event| {
                    matches!(
                        event.ev,
                        Some(Ev::StackResolved(ref resolved))
                            if resolved.object_id == source
                                && resolved.destination == StackResolveDestination::Exile as i32
                    )
                });
            }
        }
    }

    assert!(saw_simultaneous_entry_order);
    assert_eq!(engine.state.objects[&bear].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&planeswalker].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&too_large].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&nonpermanent].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.objects[&other_player_card].zone,
        Zone::Graveyard
    );
    assert_eq!(engine.state.objects[&source].zone, Zone::Exile);
    assert!(engine.state.stack.is_empty());
    assert!(
        saw_exile_exit,
        "the resolving spell leaves the stack for exile"
    );
}

#[test]
fn ascend_with_x_zero_returns_only_zero_mana_value_permanents_and_still_exiles() {
    let mut engine = ascend_engine(20_261_007);
    let zero_value_creature = inject_graveyard_card(&mut engine, 0, "ornithopter");
    // X in a mana cost is zero outside the stack (CR 107.3g, 202.3e). Endless One is returned,
    // then its zero toughness makes it die to state-based actions after entering.
    let x_cost_creature = inject_graveyard_card(&mut engine, 0, "endless_one");
    let creature = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let instant = inject_graveyard_card(&mut engine, 0, "opt");
    let x_cost_generation = engine
        .state
        .zone_change_generation
        .get(&x_cost_creature)
        .copied()
        .unwrap_or_default();
    let source = inject_card_into_hand(&mut engine, 0, "ascend_from_avernus");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            w: 3,
            ..Default::default()
        },
    );

    let hand_index = hand_index_for_card(&engine, 0, "ascend_from_avernus");
    semantic::accepted(&mut engine, 4, &cast_spell_x(hand_index, Vec::new(), 0));
    assert_eq!(engine.state.stack.last().unwrap().chosen_x, 0);
    let mut saw_exile_exit = false;
    for _ in 0..2 {
        let actor = engine.state.priority_player_id();
        let batch = engine
            .apply_command(actor, &pass())
            .expect("pass after Ascend cast");
        saw_exile_exit |= batch.events.iter().any(|event| {
            matches!(
                event.ev,
                Some(Ev::StackResolved(ref resolved))
                    if resolved.object_id == source
                        && resolved.destination == StackResolveDestination::Exile as i32
            )
        });
        if engine
            .state
            .pending_resolution
            .as_ref()
            .is_some_and(|pending| {
                pending.presentation.choice_kind == ChoiceKind::SimultaneousEntryOrder
            })
        {
            for (_, _, choice_batch) in answer_simultaneous_entry_order_in_engine_order(&mut engine)
            {
                saw_exile_exit |= choice_batch.events.iter().any(|event| {
                    matches!(
                        event.ev,
                        Some(Ev::StackResolved(ref resolved))
                            if resolved.object_id == source
                                && resolved.destination == StackResolveDestination::Exile as i32
                    )
                });
            }
        }
    }

    assert_eq!(
        engine.state.objects[&zero_value_creature].zone,
        Zone::Battlefield
    );
    assert_eq!(engine.state.objects[&x_cost_creature].zone, Zone::Graveyard);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&x_cost_creature)
            .copied()
            .unwrap_or_default(),
        x_cost_generation + 2,
        "Endless One entered before state-based actions put the 0/0 back into its owner's graveyard"
    );
    assert_eq!(engine.state.objects[&creature].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&instant].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&source].zone, Zone::Exile);
    assert!(engine.state.stack.is_empty());
    assert!(saw_exile_exit);
}

#[test]
fn ascend_exiles_after_an_empty_x_bounded_return_cohort() {
    let mut engine = ascend_engine(20_261_008);
    let creature = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let source = inject_card_into_hand(&mut engine, 0, "ascend_from_avernus");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            w: 3,
            ..Default::default()
        },
    );

    let hand_index = hand_index_for_card(&engine, 0, "ascend_from_avernus");
    semantic::accepted(&mut engine, 4, &cast_spell_x(hand_index, Vec::new(), 0));
    pass_priority_round(&mut engine);

    assert_eq!(engine.state.objects[&creature].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&source].zone, Zone::Exile);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn ascend_copy_uses_its_controllers_graveyard_and_exiles_only_the_copy() {
    let mut engine = ascend_engine(20_261_009);
    let original_card = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let copy_card = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    let source = inject_card_into_hand(&mut engine, 0, "ascend_from_avernus");
    give_mana(
        &mut engine,
        4,
        ManaGift {
            w: 3,
            c: 2,
            ..Default::default()
        },
    );

    let source_slot = hand_index_for_card(&engine, 0, "ascend_from_avernus");
    semantic::accepted(&mut engine, 4, &cast_spell_x(source_slot, Vec::new(), 2));
    inject_card_into_hand(&mut engine, 1, "twincast");
    give_mana(
        &mut engine,
        9,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    for _ in 0..engine.state.players.len() {
        if engine.state.priority_player_id() == 9 {
            break;
        }
        let priority = engine.state.priority_player_id();
        engine
            .apply_command(priority, &pass())
            .expect("pass priority to the Twincast player");
    }
    assert_eq!(engine.state.priority_player_id(), 9);
    let twincast_slot = hand_index_for_card(&engine, 1, "twincast");
    semantic::accepted(
        &mut engine,
        9,
        &cast_spell(twincast_slot, target_object(source)),
    );
    let twincast_stack_id = engine.state.stack.last().unwrap().id;
    pass_priority_round(&mut engine);
    assert!(!engine
        .state
        .stack
        .iter()
        .any(|item| item.id == twincast_stack_id));

    let copy = engine.state.stack.last().unwrap().clone();
    assert!(copy.is_copy);
    assert_eq!(copy.controller, 9);
    assert_eq!(copy.chosen_x, 2);
    assert!(!engine.state.objects.contains_key(&copy.id));
    let mut copy_events = Vec::new();
    for _ in 0..engine.state.players.len() {
        let actor = engine.state.priority_player_id();
        copy_events.extend(
            engine
                .apply_command(actor, &pass())
                .expect("pass to resolve the Ascend copy")
                .events,
        );
    }
    let copy_exits: Vec<_> = copy_events
        .iter()
        .filter_map(|event| match &event.ev {
            Some(Ev::StackResolved(exit)) if exit.object_id == copy.id => Some(exit),
            _ => None,
        })
        .collect();
    assert_eq!(copy_exits.len(), 1);
    assert_eq!(
        copy_exits[0].destination,
        StackResolveDestination::Exile as i32
    );
    assert_eq!(copy_exits[0].owner_player_id, None);
    assert_eq!(engine.state.objects[&copy_card].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&original_card].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&source].zone, Zone::Stack);

    let mut original_events = Vec::new();
    for _ in 0..engine.state.players.len() {
        let actor = engine.state.priority_player_id();
        original_events.extend(
            engine
                .apply_command(actor, &pass())
                .expect("pass to resolve physical Ascend")
                .events,
        );
    }
    assert_eq!(engine.state.objects[&original_card].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&copy_card].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&source].zone, Zone::Exile);
    assert!(original_events.iter().any(|event| matches!(
        &event.ev,
        Some(Ev::StackResolved(exit)) if exit.object_id == source
            && exit.destination == StackResolveDestination::Exile as i32
            && exit.owner_player_id == Some(4)
    )));
    assert!(engine.state.stack.is_empty());
}
