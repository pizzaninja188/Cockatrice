//! Pentavus entry counters and two ordinary composite activation costs.
use super::helpers::*;
use tricerules_cards::{ContinuousEffectKind, CounterKind, EffectDuration, Keyword};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ruled_command::Cmd, CostChoiceKind, CostSelection,
    CounterRemovalSelection,
};

fn cast_pentavus(seed: u64) -> (GameEngine, u32) {
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck; 2]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_card_into_hand(&mut engine, 0, "pentavus");
    let slot = hand_index_for_card(&engine, 0, "pentavus");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 7,
            ..Default::default()
        },
    );
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_priority_round(&mut engine);
    (engine, source)
}

fn effect(engine: &mut GameEngine, oid: u32, kind: ContinuousEffectKind) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(oid),
        kind,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
}

fn remove_command(engine: &mut GameEngine, source: u32) -> RuledCommand {
    let batch = engine.initial_response_batch();
    let choices = &batch.legal_by_player[&0].cost_choices_by_ability[&(u64::from(source) << 32)];
    assert!(choices.non_mana_costs_payable);
    let choice = choices
        .choices
        .iter()
        .find(|choice| choice.kind() == CostChoiceKind::RemoveCounters)
        .unwrap();
    let removal = choice.counter_removal.as_ref().unwrap();
    assert_eq!(removal.count, 1);
    assert_eq!(removal.options.len(), 1, "only +1/+1 counters are accepted");
    let reference = removal.source.unwrap();
    assert_eq!(reference.object_id, source);
    let mut command = activate_ability_for(engine, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.cost_selections = vec![CostSelection {
        cost_index: choice.cost_index,
        selection: Some(Selection::CounterRemoval(CounterRemovalSelection {
            source: Some(reference),
            option_id: removal.options[0].option_id,
        })),
    }];
    command
}

fn sacrifice_command(engine: &GameEngine, source: u32, payment: u32) -> RuledCommand {
    let mut command = activate_ability_for(engine, source, 1, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.cost_selections = vec![permanent_cost_selection(1, payment)];
    command
}

fn rejected_unchanged(engine: &mut GameEngine, actor: i32, command: &RuledCommand) {
    let before = serde_json::to_value(&engine.state).unwrap();
    assert!(engine.apply_command(actor, command).is_err());
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
}

fn pentavites(engine: &GameEngine) -> Vec<u32> {
    engine
        .state
        .objects
        .values()
        .filter(|object| {
            object.zone == Zone::Battlefield && object.card_id == "pentavite_c_1_1_flying"
        })
        .map(|object| object.id)
        .collect()
}

#[test]
fn pentavus_actual_cast_enters_as_five_five_with_five_counters() {
    assert!(
        tricerules_cards::registry::global()
            .get("pentavus")
            .is_some(),
        "missing exact Pentavus"
    );
    let deck = deck_with("forest", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        2026100101,
        &[10, 20],
        20,
        Some(vec![deck; 2]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_card_into_hand(&mut engine, 0, "pentavus");
    let slot = hand_index_for_card(&engine, 0, "pentavus");
    give_mana(
        &mut engine,
        10,
        ManaGift {
            c: 7,
            ..Default::default()
        },
    );
    engine.apply_command(10, &cast_spell(slot, vec![])).unwrap();
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        5
    );
    let stats = engine.characteristics(source).unwrap();
    assert_eq!((stats.power, stats.toughness), (Some(5), Some(5)));
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
}

#[test]
fn pentavus_removes_any_plus_counter_as_cost_then_creates_exact_token() {
    let (mut engine, source) = cast_pentavus(2026100102);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 6);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Stun, 2);
    let command = remove_command(&mut engine, source);
    rejected_unchanged(&mut engine, 1, &command);
    rejected_unchanged(&mut engine, 0, &command); // no mana
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let mut stale = command.clone();
    let Some(Cmd::ActivateAbility(activation)) = stale.cmd.as_mut() else {
        unreachable!()
    };
    let Some(Selection::CounterRemoval(selection)) =
        activation.cost_selections[0].selection.as_mut()
    else {
        unreachable!()
    };
    selection.source.as_mut().unwrap().zone_change_generation += 1;
    rejected_unchanged(&mut engine, 0, &stale);
    engine.apply_command(0, &command).unwrap();
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        5
    );
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::Stun),
        2
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert!(
        pentavites(&engine).is_empty(),
        "token creation waits for resolution"
    );
    pass_priority_round(&mut engine);
    let tokens = pentavites(&engine);
    assert_eq!(tokens.len(), 1);
    let token = &engine.state.objects[&tokens[0]];
    assert!(token.is_token());
    assert_eq!(token.controller, 0);
    let stats = engine.characteristics(tokens[0]).unwrap();
    assert_eq!((stats.power, stats.toughness), (Some(1), Some(1)));
    assert!(stats.colors.is_empty() && stats.is_artifact() && stats.is_creature());
    assert!(stats.types.iter().any(|subtype| subtype == "Pentavite"));
    assert!(stats.keywords.contains(&Keyword::Flying));
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let command = sacrifice_command(&engine, source, tokens[0]);
    engine.apply_command(0, &command).unwrap();
    assert!(!engine.state.players[0].battlefield.contains(&tokens[0]));
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        5
    );
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        6
    );
    assert!(pentavites(&engine).is_empty());
}

#[test]
fn pentavus_sacrifices_current_controlled_nontoken_pentavite_and_rejects_other_costs() {
    let (mut engine, source) = cast_pentavus(2026100103);
    let ordinary = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    for oid in [own, opponent] {
        effect(
            &mut engine,
            oid,
            ContinuousEffectKind::Layer4SetCreatureTypes(vec!["Pentavite".into()]),
        );
    }
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    for oid in [ordinary, opponent] {
        let command = sacrifice_command(&engine, source, oid);
        rejected_unchanged(&mut engine, 0, &command);
    }
    assert!(!engine.state.objects[&own].is_token());
    let command = sacrifice_command(&engine, source, own);
    rejected_unchanged(&mut engine, 1, &command);
    engine.state.players[0].mana_pool.colorless = 0;
    rejected_unchanged(&mut engine, 0, &command);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    engine.apply_command(0, &command).unwrap();
    assert_eq!(engine.state.objects[&own].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        5
    );
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        6
    );
    assert_eq!(engine.state.objects[&opponent].zone, Zone::Battlefield);
}

#[test]
fn pentavus_cannot_pay_with_stun_and_paid_creation_survives_source_death() {
    let (mut engine, source) = cast_pentavus(2026100104);
    let command = remove_command(&mut engine, source);
    effect(
        &mut engine,
        source,
        ContinuousEffectKind::Layer7bSetPt {
            power: 1,
            toughness: 1,
        },
    );
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 0);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::Stun, 3);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    rejected_unchanged(&mut engine, 0, &command);
    let legal = engine.initial_response_batch();
    assert!(
        !legal.legal_by_player[&0].cost_choices_by_ability[&(u64::from(source) << 32)]
            .non_mana_costs_payable
    );
    engine.state.continuous_effects.clear();
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    engine.apply_command(0, &command).unwrap();
    assert_eq!(
        engine.state.objects[&source].zone,
        Zone::Graveyard,
        "zero toughness after cost payment"
    );
    pass_priority_round(&mut engine);
    assert_eq!(
        pentavites(&engine).len(),
        1,
        "the paid ability remains independent of its source"
    );
}
