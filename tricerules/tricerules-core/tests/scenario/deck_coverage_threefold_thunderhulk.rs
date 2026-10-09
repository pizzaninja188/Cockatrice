//! Exact Threefold Thunderhulk coverage; pinned Oracle b8020a8b-557b-465d-865d-b59fecd7abc1.

use super::helpers::*;
use tricerules_cards::primitives::*;
use tricerules_cards::{AbilityPresentation, Layout, PermanentTypeFilter};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::{ruled_command::Cmd, RuledCommand};

fn setup(seed: u64) -> GameEngine {
    semantic::main_phase(seed)
}

fn tokens(engine: &GameEngine) -> Vec<u32> {
    let mut ids: Vec<_> = engine
        .state
        .objects
        .values()
        .filter(|object| object.zone == Zone::Battlefield && object.card_id == "gnome_c_1_1")
        .map(|object| object.id)
        .collect();
    ids.sort_unstable();
    ids
}

fn assert_gnomes(engine: &GameEngine, count: usize, controller: i32) {
    let ids = tokens(engine);
    assert_eq!(ids.len(), count);
    for id in ids {
        let object = &engine.state.objects[&id];
        assert_eq!((object.owner, object.controller), (controller, controller));
        assert!(object.token_origin.is_some());
        let face = engine.characteristics(id).unwrap();
        assert!(face.is_artifact() && face.is_creature() && face.has_type("Gnome"));
        assert!(face.colors.is_empty());
        assert_eq!((face.power, face.toughness), (Some(1), Some(1)));
    }
}

fn finish(engine: &mut GameEngine) {
    for _ in 0..32 {
        answer_simultaneous_entry_order_in_engine_order(engine);
        answer_trigger_order_in_engine_order(engine);
        if engine.state.stack.is_empty() && engine.state.blocking_choice().is_none() {
            return;
        }
        pass_both_players(engine);
    }
    panic!("Thunderhulk resolution exceeded 32 pass cycles");
}

fn cast_until_etb(engine: &mut GameEngine) -> u32 {
    let source = inject_card_into_hand(engine, 0, "threefold_thunderhulk");
    let generation = semantic::generation(engine, source);
    give_mana(
        engine,
        0,
        ManaGift {
            c: 7,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "threefold_thunderhulk");
    semantic::accepted(engine, 0, &cast_spell(slot, vec![]));
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.objects[&source].zone, Zone::Stack);
    pass_both_players(engine);
    semantic::assert_object(
        engine,
        source,
        "threefold_thunderhulk",
        0,
        0,
        Zone::Battlefield,
        generation + 2,
    );
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        3
    );
    let face = engine.characteristics(source).unwrap();
    assert_eq!((face.power, face.toughness), (Some(3), Some(3)));
    assert_eq!(
        engine.state.stack.len(),
        1,
        "ETB is a separate stack ability"
    );
    assert!(tokens(engine).is_empty(), "tokens await ETB resolution");
    source
}

fn sacrifice(engine: &GameEngine, source: u32, artifact: u32) -> RuledCommand {
    let mut command = activate_ability_for(engine, source, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.cost_selections = vec![permanent_cost_selection(1, artifact)];
    command
}

fn ready_attack(engine: &mut GameEngine, source: u32) {
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .summoning_sick = false;
    semantic::assert_main_priority(engine, 0);
    semantic::accepted(engine, 0, &primitive_yield());
    pass_both_players(engine);
    assert_eq!(
        engine.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
    semantic::accepted(engine, 0, &declare_attackers(vec![source]));
    assert_eq!(engine.state.stack.len(), 1, "one attack trigger");
}

#[test]
fn threefold_thunderhulk_registers_exact_identity_and_all_clauses() {
    let card = tricerules_cards::registry::global()
        .get("threefold_thunderhulk")
        .expect("complete Threefold Thunderhulk definition");
    assert_eq!(card.name, "Threefold Thunderhulk");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{7}");
    assert_eq!(face.types, ["Artifact", "Creature", "Gnome"]);
    assert!(face.colors().is_empty());
    assert_eq!(face.power, Some(0));
    assert_eq!(face.toughness, Some(0));
    assert_eq!(face.static_abilities.len(), 1);
    assert_eq!(face.triggered_abilities.len(), 2);
    assert_eq!(face.activated_abilities.len(), 1);
    assert_eq!(
        face.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        face.static_abilities[0].definition,
        StaticAbilityDef::EntersWithCounters {
            affected: EntersWithCountersAffected::Self_,
            counter: CounterKind::PlusOnePlusOne,
            amount: Amount::Fixed(3),
            cast_cost_condition: None
        }
    ));
    assert_eq!(
        face.triggered_abilities[0].trigger,
        TriggerCondition::WhenSelfEntersBattlefield
    );
    assert_eq!(
        face.triggered_abilities[1].trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0
        }
    );
    for trigger in &face.triggered_abilities {
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert!(trigger.targeting.is_none());
        assert!(
            matches!(trigger.effect.as_slice(), [SpellEffectKind::CreateTokens { token, count: Amount::Count(CountExpression::SourcePower), .. }] if token == "gnome_c_1_1")
        );
    }
    let ability = &face.activated_abilities[0];
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(ability.timing, ActivationTiming::Normal);
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    assert!(ability.targeting.is_none());
    let [AbilityCost::Mana(mana), AbilityCost::SacrificePermanent { filter, count }] =
        ability.costs.as_slice()
    else {
        panic!("mana and another-artifact sacrifice costs")
    };
    assert_eq!(*count, 1);
    assert_eq!(mana.to_string(), "{2}");
    assert_eq!(filter.kind, TargetKind::AnyPermanent);
    assert_eq!(filter.controller, TargetController::You);
    assert_eq!(filter.permanent_types, [PermanentTypeFilter::Artifact]);
    assert_eq!(filter.excluded_objects, [TargetObjectExclusion::Source]);
    assert!(matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Fixed(1),
            subject: EffectSubject::Source
        }]
    ));
}

#[test]
fn threefold_thunderhulk_paid_cast_enters_with_counters_then_creates_exact_gnomes() {
    let mut engine = setup(49701);
    cast_until_etb(&mut engine);
    finish(&mut engine);
    assert_gnomes(&engine, 3, 0);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn threefold_thunderhulk_sacrifices_another_artifact_before_counter_resolution() {
    for (seed, use_token) in [(49702, false), (49703, true)] {
        let mut engine = setup(seed);
        let source = cast_until_etb(&mut engine);
        finish(&mut engine);
        let artifact = if use_token {
            tokens(&engine)[0]
        } else {
            inject_permanent_on_battlefield(&mut engine, 0, "sol_ring")
        };
        give_mana(
            &mut engine,
            0,
            ManaGift {
                c: 2,
                ..Default::default()
            },
        );
        let command = sacrifice(&engine, source, artifact);
        semantic::accepted(&mut engine, 0, &command);
        assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
        assert!(engine
            .state
            .objects
            .get(&artifact)
            .is_none_or(|object| object.zone != Zone::Battlefield));
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            3
        );
        assert_eq!(engine.state.stack.len(), 1);
        assert!(
            !engine.state.objects[&source].tapped,
            "no tap cost, even with summoning sickness"
        );
        finish(&mut engine);
        assert_eq!(
            engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            4
        );
        assert_eq!(engine.characteristics(source).unwrap().power, Some(4));
        assert_gnomes(&engine, if use_token { 2 } else { 3 }, 0);
    }
}

#[test]
fn threefold_thunderhulk_rejects_invalid_costs_wrong_actor_and_insufficient_mana_atomically() {
    let mut engine = setup(49704);
    let source = cast_until_etb(&mut engine);
    finish(&mut engine);
    let own_artifact = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let nonartifact = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let enemy_artifact = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    for (actor, selected) in [
        (0, source),
        (0, nonartifact),
        (0, enemy_artifact),
        (1, own_artifact),
    ] {
        let before = engine.diagnostic_snapshot().unwrap();
        let command = sacrifice(&engine, source, selected);
        engine
            .apply_command(actor, &command)
            .expect_err("invalid actor or sacrifice selection");
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    }
    engine.state.players[0].mana_pool.colorless = 1;
    let before = engine.diagnostic_snapshot().unwrap();
    let command = sacrifice(&engine, source, own_artifact);
    engine
        .apply_command(0, &command)
        .expect_err("two generic mana required");
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
}

#[test]
fn threefold_thunderhulk_attack_uses_power_after_a_resolved_activation() {
    let mut engine = setup(49705);
    let source = cast_until_etb(&mut engine);
    finish(&mut engine);
    ready_attack(&mut engine, source);
    assert_gnomes(&engine, 3, 0);
    let artifact = tokens(&engine)[0];
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let command = sacrifice(&engine, source, artifact);
    semantic::accepted(&mut engine, 0, &command);
    pass_both_players(&mut engine);
    assert_eq!(engine.characteristics(source).unwrap().power, Some(4));
    assert_eq!(
        engine.state.stack.len(),
        1,
        "attack trigger remains pending"
    );
    assert_gnomes(&engine, 2, 0);
    finish(&mut engine);
    assert_gnomes(&engine, 6, 0);
}

#[test]
fn threefold_thunderhulk_departed_trigger_uses_old_generation_power_after_return() {
    let mut engine = setup(49706);
    let source = cast_until_etb(&mut engine);
    let old_generation = semantic::generation(&engine, source);
    // Seed the independently expected old power; the sacrifice behavior is tested separately.
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 5);
    inject_card_into_hand(&mut engine, 0, "unsummon");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "unsummon");
    semantic::accepted(&mut engine, 0, &cast_spell(slot, target_object(source)));
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Hand);
    assert_eq!(engine.state.stack.len(), 1);
    // Logged dev entry supplies an interrupt/reentry fixture; this is not a sorcery-speed recast.
    let returned = move_ready_to_battlefield(&mut engine, 0, "threefold_thunderhulk");
    assert_eq!(returned, source);
    assert!(semantic::generation(&engine, source) > old_generation);
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 9);
    assert_eq!(engine.characteristics(source).unwrap().power, Some(9));
    finish(&mut engine);
    assert_gnomes(&engine, 14, 0); // New ETB uses 9; old departed ETB must still use 5.
}

#[test]
fn threefold_thunderhulk_zero_and_negative_power_create_no_tokens() {
    for (seed, base_power) in [(49707, -3), (49708, -5)] {
        let mut engine = setup(seed);
        let source = cast_until_etb(&mut engine);
        // Structural P/T layer fixture preserves positive toughness while varying power.
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(source),
            kind: ContinuousEffectKind::Layer7bSetPt {
                power: base_power,
                toughness: 0,
            },
            condition: None,
            duration: tricerules_cards::EffectDuration::Indefinite,
            timestamp: engine.state.command_index,
        });
        // Public wire P/T clamps negative values; the fixture's signed power is 0 or -2.
        let face = engine.characteristics(source).unwrap();
        assert_eq!((face.power, face.toughness), (Some(0), Some(3)));
        finish(&mut engine);
        assert_gnomes(&engine, 0, 0);
        assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    }
}

#[test]
fn threefold_thunderhulk_resolved_counter_does_not_follow_source_reentry() {
    let mut engine = setup(49709);
    let source = cast_until_etb(&mut engine);
    finish(&mut engine);
    let artifact = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 2,
            u: 1,
            ..Default::default()
        },
    );
    let command = sacrifice(&engine, source, artifact);
    semantic::accepted(&mut engine, 0, &command);
    assert_eq!(engine.state.objects[&artifact].zone, Zone::Graveyard);
    let generation = semantic::generation(&engine, source);
    inject_card_into_hand(&mut engine, 0, "unsummon");
    let slot = hand_index_for_card(&engine, 0, "unsummon");
    semantic::accepted(&mut engine, 0, &cast_spell(slot, target_object(source)));
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Hand);
    // Logged dev reentry fixture interrupts the pending activation; no ordinary recast is claimed.
    assert_eq!(
        move_ready_to_battlefield(&mut engine, 0, "threefold_thunderhulk"),
        source
    );
    assert!(semantic::generation(&engine, source) > generation);
    finish(&mut engine);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        3
    );
    assert_gnomes(&engine, 6, 0);
}

#[test]
fn threefold_thunderhulk_four_player_tokens_and_costs_use_actual_controller_ids() {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        49710,
        &[7, 20, 42, 91],
        20,
        Some(vec![deck_with("island", &[]); 4]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_card_into_hand(&mut engine, 0, "threefold_thunderhulk");
    give_mana(
        &mut engine,
        7,
        ManaGift {
            c: 7,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "threefold_thunderhulk");
    semantic::accepted(&mut engine, 7, &cast_spell(slot, vec![]));
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.objects[&source].controller, 7);
    pass_priority_round(&mut engine);
    assert_gnomes(&engine, 3, 7);
    let foreign = inject_permanent_on_battlefield(&mut engine, 2, "sol_ring");
    assert_eq!(engine.state.objects[&foreign].controller, 42);
    give_mana(
        &mut engine,
        7,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let before = engine.diagnostic_snapshot().unwrap();
    let invalid = sacrifice(&engine, source, foreign);
    engine
        .apply_command(7, &invalid)
        .expect_err("opponent 42's artifact cannot pay controller 7's cost");
    assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
    let own = tokens(&engine)[0];
    let valid = sacrifice(&engine, source, own);
    semantic::accepted(&mut engine, 7, &valid);
    pass_priority_round(&mut engine);
    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        4
    );
    assert_gnomes(&engine, 2, 7);
    assert_eq!(engine.state.objects[&foreign].zone, Zone::Battlefield);
}
