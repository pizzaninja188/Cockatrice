use super::helpers::*;
use tricerules_cards::primitives::{ActivationLimit, ContinuousEffectKind, EffectDuration};
use tricerules_cards::{AbilityId, CardRegistry};
use tricerules_core::{AffectedScope, ContinuousEffect};
use tricerules_proto::ruled::v1::ResolutionChoiceDecision;

fn mana_state(engine: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let pool = &engine.state.players[player].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

fn grant_exhaust_ability(engine: &mut GameEngine, source: u32, ability_id: &str) {
    let mut ability = CardRegistry::global()
        .get("temur_devotee")
        .expect("Temur Devotee definition")
        .primary_face()
        .activated_abilities[0]
        .clone();
    ability.activation_limit = Some(ActivationLimit::PerObject { max_activations: 1 });
    ability.ability_id = AbilityId::new(ability_id).unwrap();
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::GrantActivatedAbility(Box::new(ability)),
        condition: None,
        duration: EffectDuration::WhileSourceOnBattlefield,
        timestamp: engine.state.command_index,
    });
}

#[test]
fn exhaust_persists_across_turns_and_control_but_resets_for_a_new_object() {
    let mut engine = anthem_engine(15_201, "mountain");
    let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    grant_exhaust_ability(&mut engine, source, "exhaust_granted");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );

    engine
        .apply_command(0, &activate_ability(source, 0, vec![]))
        .expect("the first Exhaust activation is legal");
    assert_eq!(zone_view_ability_flags(&mut engine, 0, source), [false]);
    assert_eq!(
        zone_view_ability_flags(&mut engine, 0, source),
        [false],
        "a reconnect-style snapshot publishes the spent ability as unavailable"
    );

    let starting_turn = engine.state.turn_instance;
    for _ in 0..40 {
        if engine.state.turn_instance != starting_turn {
            break;
        }
        resolve_cleanup_discards_if_any(&mut engine);
        let priority = engine.state.priority_player_id();
        engine
            .apply_command(priority, &pass())
            .expect("advance to the next turn");
    }
    assert!(
        engine.state.turn_instance > starting_turn,
        "the turn advanced"
    );
    assert_eq!(
        zone_view_ability_flags(&mut engine, 0, source),
        [false],
        "turn cleanup must not restore an Exhaust ability"
    );

    engine.state.players[0]
        .battlefield
        .retain(|object_id| *object_id != source);
    engine.state.players[1].battlefield.push(source);
    let object = engine.state.objects.get_mut(&source).expect("source");
    object.base_controller = 1;
    object.controller = 1;
    engine.state.priority_idx = 1;
    assert_eq!(zone_view_ability_flags(&mut engine, 1, source), [false]);

    *engine
        .state
        .face_change_generation
        .entry(source)
        .or_insert(0) += 1;
    assert_eq!(
        zone_view_ability_flags(&mut engine, 1, source),
        [false],
        "an in-place face-status change does not create a new object"
    );

    *engine
        .state
        .zone_change_generation
        .entry(source)
        .or_insert(0) += 1;
    assert_eq!(
        zone_view_ability_flags(&mut engine, 1, source),
        [true],
        "a CR 400.7 new object receives a fresh allowance"
    );
}

#[test]
fn separate_exhaust_abilities_on_one_object_have_independent_allowances() {
    let mut engine = anthem_engine(15_202, "mountain");
    let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    grant_exhaust_ability(&mut engine, source, "exhaust_granted_01");
    grant_exhaust_ability(&mut engine, source, "exhaust_granted_02");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );

    engine
        .apply_command(0, &activate_ability(source, 0, vec![]))
        .expect("activate the first Exhaust ability");
    assert_eq!(
        zone_view_ability_flags(&mut engine, 0, source),
        [false, true]
    );

    engine
        .apply_command(0, &activate_ability(source, 1, vec![]))
        .expect("the second Exhaust ability remains independent");
    assert_eq!(
        zone_view_ability_flags(&mut engine, 0, source),
        [false, false]
    );
}

#[test]
fn failed_and_duplicate_commands_do_not_partially_change_exhaust_state() {
    let mut engine = anthem_engine(15_203, "mountain");
    let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    grant_exhaust_ability(&mut engine, source, "exhaust_granted");
    let command = activate_ability(source, 0, vec![]);

    engine
        .apply_command(0, &command)
        .expect_err("an unaffordable activation is illegal");
    assert!(engine.state.activation_uses_per_object.is_empty());
    assert_eq!(zone_view_ability_flags(&mut engine, 0, source), [true]);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let batch = engine
        .apply_command(0, &command)
        .expect("the funded activation succeeds");
    let key = (u64::from(source)) << 32;
    assert!(
        !batch.legal_by_player[&0].cost_choices_by_ability[&key].non_mana_costs_payable,
        "the same authoritative legality check disables further cost collection"
    );

    let mana_before = mana_state(&engine, 0);
    let uses_before = engine.state.activation_uses_per_object.clone();
    engine
        .apply_command(0, &command)
        .expect_err("the stale duplicate activation is rejected before payment");
    assert_eq!(mana_state(&engine, 0), mana_before);
    assert_eq!(engine.state.activation_uses_per_object, uses_before);
}

#[test]
fn a_countered_exhaust_ability_remains_spent() {
    let decks = vec![
        deck_with("island", &["prodigal_sorcerer"]),
        deck_with("island", &["dirgur_island_dragon_skimming_strike"]),
    ];
    let mut engine = GameEngine::new(15_204, &[0, 1], 20, Some(decks), true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let source = relocate_to_battlefield(&mut engine, 0, "prodigal_sorcerer", false);
    let warded = relocate_to_battlefield(
        &mut engine,
        1,
        "dirgur_island_dragon_skimming_strike",
        false,
    );
    let mut ability = CardRegistry::global()
        .get("prodigal_sorcerer")
        .expect("Prodigal Sorcerer definition")
        .primary_face()
        .activated_abilities[0]
        .clone();
    ability.activation_limit = Some(ActivationLimit::PerObject { max_activations: 1 });
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::GrantActivatedAbility(Box::new(ability)),
        condition: None,
        duration: EffectDuration::WhileSourceOnBattlefield,
        timestamp: engine.state.command_index,
    });

    engine
        .apply_command(0, &activate_ability(source, 1, target_object(warded)))
        .expect("activate the granted Exhaust ability");
    assert_eq!(
        engine.state.stack.len(),
        2,
        "Ward triggers above the ability"
    );
    pass_both_players(&mut engine);
    engine
        .apply_command(
            0,
            &submit_resolution_decision(ResolutionChoiceDecision::Decline),
        )
        .expect("declining Ward counters the Exhaust ability");

    engine
        .state
        .objects
        .get_mut(&source)
        .expect("source")
        .tapped = false;
    assert_eq!(
        zone_view_ability_flags(&mut engine, 0, source),
        [true, false],
        "countering resolution does not undo the completed activation"
    );
}

#[test]
fn same_seed_and_commands_replay_the_same_exhaust_state() {
    fn replay() -> ((u32, u64, Vec<AbilityId>, u32), Vec<bool>) {
        let mut engine = anthem_engine(15_205, "mountain");
        let source = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        grant_exhaust_ability(&mut engine, source, "exhaust_granted");
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        engine
            .apply_command(0, &activate_ability(source, 0, vec![]))
            .expect("accepted command");
        let (key, count) = engine
            .state
            .activation_uses_per_object
            .iter()
            .next()
            .expect("one persistent activation record");
        (
            (
                key.object_id,
                key.zone_change_generation,
                key.definition.ability_path.clone(),
                *count,
            ),
            zone_view_ability_flags(&mut engine, 0, source),
        )
    }

    assert_eq!(replay(), replay());
}
