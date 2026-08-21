use crate::helpers::*;
use tricerules_cards::primitives::{
    Color, ContinuousEffectKind, CounterKind, EffectDuration, Keyword,
};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::{dev_command, DevCommand, DevMoveCard, DevZone, TargetRef};

fn resolving_clone_with_source(source_card_id: &str, seed: u64) -> (GameEngine, u32) {
    let decks = Some(vec![
        vec![
            "clone".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
        vec![
            source_card_id.into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = put_creature_on_battlefield(&mut engine, 1, source_card_id);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let clone = hand_index_for_card(&engine, 0, "clone");
    engine
        .apply_command(0, &cast_spell(clone, vec![]))
        .expect("cast Clone");
    pass_both_players(&mut engine);
    (engine, source)
}

#[test]
fn clone_chooses_its_copy_source_during_resolution_not_casting() {
    let decks = Some(vec![
        vec![
            "clone".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
        vec![
            "serra_angel".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut engine = GameEngine::new(45_001, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = put_creature_on_battlefield(&mut engine, 1, "serra_angel");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );

    let clone = hand_index_for_card(&engine, 0, "clone");
    let cast = engine
        .apply_command(0, &cast_spell(clone, vec![]))
        .expect("cast Clone without a target");
    assert!(
        find_resolution_choice(&cast).is_none(),
        "casting Clone must not choose what it will copy"
    );

    pass_both_players(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Clone resolution must park before battlefield entry");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::CopySource);
    assert_eq!(pending.deciding_player, 0);
    assert_eq!(pending.presentation.min, 0);
    assert_eq!(pending.presentation.max, 1);
    assert_eq!(pending.presentation.candidates, vec![source]);
    assert!(matches!(
        &pending.continuation,
        ResolutionContinuation::EntryCopySource { .. }
    ));
    assert!(
        engine.state.players[0]
            .battlefield
            .iter()
            .all(|oid| engine.state.objects[oid].card_id != "clone"),
        "Clone is not committed to the battlefield until its entry choice finishes"
    );
}

#[test]
fn countered_clone_never_emits_a_copy_source_choice() {
    let decks = Some(vec![
        vec![
            "clone".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
        vec![
            "essence_scatter".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
    ]);
    let mut engine = GameEngine::new(45_011, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    give_mana(
        &mut engine,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );

    let clone = hand_index_for_card(&engine, 0, "clone");
    engine
        .apply_command(0, &cast_spell(clone, vec![]))
        .expect("cast Clone");
    let clone_spell = engine.state.stack.last().expect("Clone on stack").id;
    engine.apply_command(0, &pass()).expect("pass to opponent");
    let scatter = hand_index_for_card(&engine, 1, "essence_scatter");
    engine
        .apply_command(
            1,
            &cast_spell(
                scatter,
                vec![TargetRef {
                    object_id: clone_spell,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
                }],
            ),
        )
        .expect("counter Clone");
    resolve_entire_stack_two_player(&mut engine);

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(count_card_id_in_graveyard(&engine, 0, "clone"), 1);
    assert!(engine.state.players[0]
        .battlefield
        .iter()
        .all(|oid| engine.state.objects[oid].card_id != "clone"));
}

#[test]
fn clone_copies_printed_values_but_not_source_status_counters_damage_or_pump() {
    let (mut engine, source) = resolving_clone_with_source("serra_angel", 45_002);
    {
        let source_object = engine.state.objects.get_mut(&source).expect("source");
        source_object.tapped = true;
        source_object.damage = 3;
        source_object.add_counters(CounterKind::PlusOnePlusOne, 2, 1);
        source_object.add_counters(CounterKind::Keyword(Keyword::Menace), 1, 2);
    }
    engine.state.continuous_effects.push(ContinuousEffect {
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::PtModify {
            delta_power: 3,
            delta_toughness: 3,
        },
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });

    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .expect("copy Serra Angel");
    let clone = battlefield_object_for_card(&engine, 0, "clone");
    let object = &engine.state.objects[&clone];
    let values = object.copiable_values.as_ref().expect("copy snapshot");
    assert_eq!(values.face.name, "Serra Angel");
    assert_eq!(values.display_name, "Serra Angel");
    assert_eq!(values.source_card_id, "serra_angel");
    assert_eq!(object.copy_revision, 1);
    assert!(!object.tapped);
    assert_eq!(object.damage, 0);
    assert!(object.counters.is_empty());
    assert!(object.counter_timestamps.is_empty());

    let characteristics = engine
        .characteristics(clone)
        .expect("copied characteristics");
    assert_eq!(characteristics.power, Some(4));
    assert_eq!(characteristics.toughness, Some(4));
    assert!(characteristics.types.contains(&"Angel".to_string()));
    assert_eq!(characteristics.colors, vec![Color::White]);
    assert!(characteristics.has_keyword(Keyword::Flying));
    assert!(characteristics.has_keyword(Keyword::Vigilance));
    assert!(
        !characteristics.has_keyword(Keyword::Menace),
        "keyword counters are status, not copiable values"
    );

    engine
        .state
        .objects
        .get_mut(&clone)
        .expect("Clone")
        .counters
        .insert(CounterKind::PlusOnePlusOne, 1);
    assert_eq!(engine.effective_power(clone), Some(5));
    assert_eq!(engine.effective_toughness(clone), Some(5));
}

#[test]
fn conditional_characteristics_clone_inherits_the_copied_static_ability() {
    let (mut engine, source) = resolving_clone_with_source("gearsmith_guardian", 78_008);
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .expect("copy Gearsmith Guardian");
    let clone = battlefield_object_for_card(&engine, 0, "clone");

    assert_eq!(engine.effective_power(clone), Some(3));
    inject_creature_on_battlefield(&mut engine, 0, "air_elemental");
    assert_eq!(
        engine.effective_power(clone),
        Some(5),
        "the copied conditional static ability tracks its new controller's battlefield"
    );
}

#[test]
fn clone_re_evaluates_entry_replacements_from_the_copied_face() {
    let (mut engine, source) = resolving_clone_with_source("diregraf_ghoul", 45_003);
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .expect("copy Diregraf Ghoul");

    let clone = battlefield_object_for_card(&engine, 0, "clone");
    assert!(engine.state.objects[&clone].tapped);
    assert_eq!(engine.effective_power(clone), Some(2));
    assert_eq!(
        engine.state.objects[&clone]
            .copiable_values
            .as_ref()
            .expect("copy snapshot")
            .face
            .name,
        "Diregraf Ghoul"
    );
}

#[test]
fn clone_decline_finishes_entry_and_zero_toughness_sba() {
    let (mut engine, _) = resolving_clone_with_source("grizzly_bears", 45_004);
    let clone = engine
        .state
        .objects
        .values()
        .find(|object| object.card_id == "clone")
        .expect("Clone object")
        .id;
    engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .expect("decline copy");

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&clone].zone, Zone::Graveyard);
    assert!(engine.state.objects[&clone].copiable_values.is_none());
    assert!(engine.state.players[0].graveyard.contains(&clone));
}

#[test]
fn stale_copy_source_is_rejected_without_clearing_the_choice() {
    let (mut engine, source) = resolving_clone_with_source("grizzly_bears", 45_005);
    engine.state.players[1]
        .battlefield
        .retain(|oid| *oid != source);
    engine.state.objects.get_mut(&source).expect("source").zone = Zone::Graveyard;

    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .is_err());
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("stale answer preserves pending choice");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::CopySource);
    assert_eq!(pending.presentation.candidates, vec![source]);
}

#[test]
fn copying_an_already_copied_clone_uses_its_layer_one_values() {
    let decks = Some(vec![
        vec![
            "clone".into(),
            "clone".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
        vec![
            "serra_angel".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut engine = GameEngine::new(45_006, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let angel = put_creature_on_battlefield(&mut engine, 1, "serra_angel");

    for source in [angel, 0] {
        grant_pool(&mut engine, 0);
        let hand_clone = hand_index_for_card(&engine, 0, "clone");
        engine
            .apply_command(0, &cast_spell(hand_clone, vec![]))
            .expect("cast Clone");
        pass_both_players(&mut engine);
        let actual_source = if source == 0 {
            engine.state.players[0]
                .battlefield
                .iter()
                .copied()
                .find(|oid| engine.state.objects[oid].card_id == "clone")
                .expect("first Clone")
        } else {
            source
        };
        engine
            .apply_command(0, &submit_resolution_choice(vec![actual_source]))
            .expect("choose copy source");
    }

    let clones: Vec<_> = engine.state.players[0]
        .battlefield
        .iter()
        .copied()
        .filter(|oid| engine.state.objects[oid].card_id == "clone")
        .collect();
    assert_eq!(clones.len(), 2);
    for clone in clones {
        let values = engine.state.objects[&clone]
            .copiable_values
            .as_ref()
            .expect("copy snapshot");
        assert_eq!(values.source_card_id, "serra_angel");
        assert_eq!(values.face.name, "Serra Angel");
        assert_eq!(engine.effective_power(clone), Some(4));
    }
}

#[test]
fn clone_can_copy_a_registry_backed_token() {
    let decks = Some(vec![
        vec![
            "clone".into(),
            "raise_the_alarm".into(),
            "island".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut engine = GameEngine::new(45_007, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    grant_pool(&mut engine, 0);
    let alarm = hand_index_for_card(&engine, 0, "raise_the_alarm");
    engine
        .apply_command(0, &cast_spell(alarm, vec![]))
        .expect("cast Raise the Alarm");
    pass_both_players(&mut engine);
    let soldier = battlefield_token_oids(&engine, 0, "soldier_w_1_1")[0];

    grant_pool(&mut engine, 0);
    let clone = hand_index_for_card(&engine, 0, "clone");
    engine
        .apply_command(0, &cast_spell(clone, vec![]))
        .expect("cast Clone");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![soldier]))
        .expect("copy Soldier token");

    let clone = battlefield_object_for_card(&engine, 0, "clone");
    let values = engine.state.objects[&clone]
        .copiable_values
        .as_ref()
        .expect("copy snapshot");
    assert_eq!(values.source_card_id, "soldier_w_1_1");
    assert_eq!(values.face.name, "Soldier");
    assert_eq!(engine.effective_power(clone), Some(1));
    assert_eq!(engine.effective_toughness(clone), Some(1));
}

#[test]
fn copied_activated_ability_uses_the_effective_face() {
    let (mut engine, source) = resolving_clone_with_source("prodigal_sorcerer", 45_008);
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .expect("copy Prodigal Sorcerer");
    let clone = battlefield_object_for_card(&engine, 0, "clone");
    engine
        .state
        .objects
        .get_mut(&clone)
        .expect("Clone")
        .summoning_sick = false;

    apply_ability(&mut engine, 0, clone, 0, target_player(1)).expect("activate copied ability");
    let ability = engine.state.stack.last().expect("ability on stack");
    assert_eq!(ability.card_id, "prodigal_sorcerer");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[1].life, 19);
}

#[test]
fn copied_enters_trigger_is_put_on_the_stack_from_the_effective_face() {
    let (mut engine, source) = resolving_clone_with_source("elvish_visionary", 45_010);
    inject_library_card(&mut engine, 0, "forest");
    let hand_before = engine.state.players[0].hand.len();
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .expect("copy Elvish Visionary");

    let trigger = engine.state.stack.last().expect("copied ETB trigger");
    assert!(trigger.is_triggered);
    assert_eq!(trigger.card_id, "elvish_visionary");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
}

#[test]
fn leaving_the_battlefield_clears_copy_values_and_restores_clone() {
    let (mut engine, source) = resolving_clone_with_source("serra_angel", 45_009);
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .expect("copy Serra Angel");
    let clone = battlefield_object_for_card(&engine, 0, "clone");
    engine.enable_dev_commands();
    let command = RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: 0,
            dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                zone: DevZone::Graveyard as i32,
                card_name: "Clone".to_string(),
                ready: false,
            })),
        })),
    };
    engine.apply_command(0, &command).expect("move Clone");

    let object = &engine.state.objects[&clone];
    assert_eq!(object.zone, Zone::Graveyard);
    assert!(object.copiable_values.is_none());
    assert_eq!(object.copy_revision, 0);
    assert_eq!(engine.effective_power(clone), Some(0));
    assert_eq!(engine.effective_toughness(clone), Some(0));
}
