//! Psychosis Crawler: all-zone hand-size CDA and one life-loss trigger per successful draw.
use crate::helpers::*;
use tricerules_cards::primitives::{
    ContinuousEffectKind, EffectDuration, PermanentTypeFilter, TypeLineAddition,
};
use tricerules_cards::CounterKind;
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};

const CRAWLER: &str = "psychosis_crawler";

fn crawler_engine(players: &[i32]) -> GameEngine {
    assert!(
        tricerules_cards::registry::global().get(CRAWLER).is_some(),
        "missing exact Psychosis Crawler"
    );
    let deck = deck_with("forest", &[CRAWLER, "divination", "cackling_counterpart"]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        2026093011,
        players,
        20,
        Some(vec![deck; players.len()]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn hand_size(engine: &mut GameEngine, player: usize, count: usize) {
    for object in std::mem::take(&mut engine.state.players[player].hand) {
        engine.state.objects.get_mut(&object).unwrap().zone = Zone::Graveyard;
        engine.state.players[player].graveyard.push(object);
    }
    for _ in 0..count {
        inject_card_into_hand(engine, player, "forest");
    }
}

fn assert_pt(engine: &GameEngine, source: u32, expected: (u32, u32)) {
    let value = engine.characteristics(source).unwrap();
    assert_eq!(
        (value.power, value.toughness),
        (Some(expected.0), Some(expected.1))
    );
}

fn add_effect(engine: &mut GameEngine, source: u32, kind: ContinuousEffectKind) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
}

#[test]
fn crawler_cda_is_live_outside_battlefield_and_uses_current_controller_on_battlefield() {
    let mut engine = crawler_engine(&[0, 1]);
    let source = take_oid_from_library_or_hand(&mut engine, 0, CRAWLER);
    hand_size(&mut engine, 0, 3);
    hand_size(&mut engine, 1, 5);
    for zone in [Zone::Library, Zone::Graveyard, Zone::Exile, Zone::Command] {
        engine.state.objects.get_mut(&source).unwrap().zone = zone;
        assert_pt(&engine, source, (3, 3));
    }
    engine.state.objects.get_mut(&source).unwrap().zone = Zone::Hand;
    engine.state.players[0].hand.push(source);
    assert_pt(&engine, source, (4, 4));
    engine.state.players[0]
        .hand
        .retain(|object| *object != source);
    engine.state.objects.get_mut(&source).unwrap().zone = Zone::Battlefield;
    engine.state.players[0].battlefield.push(source);
    assert_pt(&engine, source, (3, 3));
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .base_controller = 1;
    assert_pt(&engine, source, (5, 5));
    engine.state.objects.get_mut(&source).unwrap().zone = Zone::Graveyard;
    assert_pt(&engine, source, (3, 3));
    engine.state.objects.get_mut(&source).unwrap().zone = Zone::Battlefield;
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .base_controller = 0;
    hand_size(&mut engine, 0, 1);
    assert_pt(&engine, source, (1, 1));
    add_effect(
        &mut engine,
        source,
        ContinuousEffectKind::Layer7bSetPt {
            power: 7,
            toughness: 7,
        },
    );
    add_effect(
        &mut engine,
        source,
        ContinuousEffectKind::PtModify {
            delta_power: 2,
            delta_toughness: 1,
        },
    );
    let timestamp = engine.state.command_index;
    engine.state.objects.get_mut(&source).unwrap().add_counters(
        CounterKind::PlusOnePlusOne,
        1,
        timestamp,
    );
    assert_pt(&engine, source, (10, 9));
}

#[test]
fn crawler_draws_create_separate_life_loss_triggers_and_opponents_draws_do_not() {
    let mut engine = crawler_engine(&[0, 1]);
    let source = move_ready_to_battlefield(&mut engine, 0, CRAWLER);
    ensure_card_in_hand(&mut engine, 0, "divination");
    authoring_fixture::library_top(&mut engine, 0, &["island", "mountain"]);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let hand_before = engine.state.players[0].hand.len();
    let slot = hand_index_for_card(&engine, 0, "divination");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    engine.apply_command(0, &pass()).unwrap();
    engine.apply_command(1, &pass()).unwrap();
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(
        engine.state.stack.len(),
        2,
        "two successful draws create two triggers"
    );
    assert_eq!(engine.state.players[1].life, 20);
    assert_eq!(engine.state.players[0].hand.len(), hand_before + 1);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[1].life, 18);
    assert_pt(
        &engine,
        source,
        ((hand_before + 1) as u32, (hand_before + 1) as u32),
    );
    end_active_turn(&mut engine, 0);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.turn_history.current.player(1).cards_drawn, 1);
    assert_eq!(engine.state.players[1].life, 18);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn crawler_dies_with_no_cards_at_the_next_state_based_action_check() {
    let mut engine = crawler_engine(&[0, 1]);
    let source = relocate_to_battlefield(&mut engine, 0, CRAWLER, false);
    hand_size(&mut engine, 0, 0);
    assert_pt(&engine, source, (0, 0));
    engine.apply_command(0, &pass()).unwrap();
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
}

#[test]
fn crawler_projects_live_size_and_rejects_an_unoffered_activation_without_mutation() {
    let mut engine = crawler_engine(&[0, 1]);
    let source = move_ready_to_battlefield(&mut engine, 0, CRAWLER);
    hand_size(&mut engine, 0, 3);
    let batch = engine.initial_response_batch();
    let projected = batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => view
                .per_player
                .iter()
                .flat_map(|player| &player.battlefield_objects)
                .find(|object| object.object_id == source),
            _ => None,
        })
        .unwrap();
    assert_eq!((projected.power, projected.toughness), (3, 3));
    assert!(projected.activated_abilities.is_empty());
    let before = serde_json::to_value(&engine.state).unwrap();
    engine
        .apply_command(0, &activate_ability_for(&engine, source, 0, vec![]))
        .expect_err("Crawler has no activated ability");
    assert_eq!(serde_json::to_value(&engine.state).unwrap(), before);
}

#[test]
fn crawler_cast_counts_the_smaller_hand_and_copy_cda_is_not_a_frozen_size() {
    let mut engine = crawler_engine(&[0, 1]);
    hand_size(&mut engine, 0, 3);
    let source = inject_card_into_hand(&mut engine, 0, CRAWLER);
    assert_pt(&engine, source, (4, 4));
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 5,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, CRAWLER);
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    assert_eq!(engine.state.objects[&source].zone, Zone::Stack);
    assert_pt(&engine, source, (3, 3));
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_pt(&engine, source, (3, 3));
    ensure_card_in_hand(&mut engine, 0, "cackling_counterpart");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "cackling_counterpart");
    engine
        .apply_command(0, &cast_spell(slot, target_object(source)))
        .unwrap();
    engine.apply_command(0, &pass()).unwrap();
    let batch = engine.apply_command(1, &pass()).unwrap();
    let copy = token_created_events(&batch)[0].object_id;
    assert_pt(&engine, copy, (3, 3));
    hand_size(&mut engine, 1, 5);
    engine.state.objects.get_mut(&copy).unwrap().base_controller = 1;
    assert_pt(&engine, copy, (5, 5));
    assert_pt(&engine, source, (3, 3));
    engine.state.objects.get_mut(&copy).unwrap().face_down = true;
    assert_pt(&engine, copy, (2, 2));
    engine.state.objects.get_mut(&copy).unwrap().face_down = false;
    add_effect(
        &mut engine,
        copy,
        ContinuousEffectKind::Layer6RemoveAllAbilities,
    );
    assert_pt(&engine, copy, (0, 0));
    let timestamp = engine.state.command_index;
    engine.state.objects.get_mut(&copy).unwrap().add_counters(
        CounterKind::PlusOnePlusOne,
        1,
        timestamp,
    );
    assert_pt(&engine, copy, (1, 1));
}

#[test]
fn crawler_stack_cda_uses_the_spell_controller_instead_of_the_card_owner() {
    let mut engine = crawler_engine(&[0, 1]);
    hand_size(&mut engine, 0, 2);
    let source = inject_card_into_hand(&mut engine, 0, CRAWLER);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 5,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, CRAWLER);
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    hand_size(&mut engine, 1, 5);
    // Fixture an other-player spell controller independently of the owner/base controller.
    engine.state.stack.last_mut().unwrap().controller = 1;
    assert_eq!(engine.state.objects[&source].owner, 0);
    assert_pt(&engine, source, (5, 5));
}

#[test]
fn undefined_creature_stats_are_zero_and_counters_apply_while_noncreatures_have_no_pt() {
    let mut engine = crawler_engine(&[0, 1]);
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let original = engine.characteristics(land).unwrap();
    assert_eq!((original.power, original.toughness), (None, None));
    let animation = ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
        land_types: Vec::new(),
        card_types: vec![PermanentTypeFilter::Creature],
        creature_types: vec![],
    });
    add_effect(&mut engine, land, animation.clone());
    assert_pt(&engine, land, (0, 0));
    let timestamp = engine.state.command_index;
    engine.state.objects.get_mut(&land).unwrap().add_counters(
        CounterKind::PlusOnePlusOne,
        1,
        timestamp,
    );
    assert_pt(&engine, land, (1, 1));
    let zero = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    add_effect(&mut engine, zero, animation);
    engine.apply_command(0, &pass()).unwrap();
    assert_eq!(engine.state.objects[&zero].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&land].zone, Zone::Battlefield);
}

#[test]
fn crawler_reserved_cast_cda_uses_the_pending_caster_before_stack_publication() {
    let mut engine = crawler_engine(&[0, 1]);
    hand_size(&mut engine, 0, 2);
    let source = inject_card_into_hand(&mut engine, 0, CRAWLER);
    let slot = hand_index_for_card(&engine, 0, CRAWLER);
    let Some(Cmd::CastSpell(cast)) = cast_spell(slot, vec![]).cmd else {
        unreachable!()
    };
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::BeginSpellCast(
                    tricerules_proto::ruled::v1::BeginSpellCast {
                        announcement: Some(tricerules_proto::ruled::v1::SpellCastAnnouncement {
                            targets: cast.targets,
                            x_value: cast.x_value,
                            flex_payments: cast.flex_payments,
                            face_index: cast.face_index,
                            selected_modes: cast.selected_modes,
                            source: cast.source,
                            cost_selections: cast.cost_selections,
                            cast_cost_group_selections: cast.cast_cost_group_selections,
                            cast_method: cast.cast_method,
                            casting_permission_id: cast.casting_permission_id,
                        }),
                    },
                )),
            },
        )
        .unwrap();
    assert_eq!(engine.state.objects[&source].zone, Zone::Stack);
    assert!(engine.state.stack.is_empty());
    assert_pt(&engine, source, (2, 2));
    hand_size(&mut engine, 1, 5);
    // Distinguish the reserved spell's caster from its physical owner/base controller.
    engine.state.pending_spell_cast.as_mut().unwrap().caster = 1;
    assert_eq!(engine.state.objects[&source].owner, 0);
    assert_pt(&engine, source, (5, 5));
}

#[test]
fn crawler_survives_a_resolving_discard_then_draw_with_a_temporarily_empty_hand() {
    let mut engine = crawler_engine(&[0, 1]);
    let source = move_ready_to_battlefield(&mut engine, 0, CRAWLER);
    hand_size(&mut engine, 0, 2);
    inject_card_into_hand(&mut engine, 0, "tolarian_winds");
    authoring_fixture::library_top(&mut engine, 0, &["island", "mountain"]);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "tolarian_winds");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_priority_round(&mut engine);
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(engine.state.objects[&source].zone, Zone::Battlefield);
    assert_pt(&engine, source, (2, 2));
    assert_eq!(engine.state.stack.len(), 2);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[1].life, 18);
}

#[test]
fn casting_the_last_hand_card_kills_crawler_before_the_later_draw_spell_resolves() {
    let mut engine = crawler_engine(&[0, 1]);
    let source = move_ready_to_battlefield(&mut engine, 0, CRAWLER);
    hand_size(&mut engine, 0, 0);
    inject_card_into_hand(&mut engine, 0, "divination");
    authoring_fixture::library_top(&mut engine, 0, &["island", "mountain"]);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "divination");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    assert_eq!(engine.state.objects[&source].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].hand.len(), 2);
    assert_eq!(engine.state.players[1].life, 20);
}

#[test]
fn failed_draw_creates_no_crawler_life_loss_trigger() {
    let mut engine = crawler_engine(&[0, 1]);
    move_ready_to_battlefield(&mut engine, 0, CRAWLER);
    ensure_card_in_hand(&mut engine, 0, "divination");
    engine.state.players[0].library.clear();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "divination");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_priority_round(&mut engine);
    assert_eq!(engine.state.turn_history.current.player(0).cards_drawn, 0);
    assert_eq!(engine.state.players[1].life, 20);
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_trigger_order.is_none());
}

#[test]
fn captured_crawler_triggers_keep_their_controller_after_source_departure_in_multiplayer() {
    let mut engine = crawler_engine(&[0, 1, 2, 3]);
    let source = move_ready_to_battlefield(&mut engine, 0, CRAWLER);
    ensure_card_in_hand(&mut engine, 0, "divination");
    authoring_fixture::library_top(&mut engine, 0, &["island", "mountain"]);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "divination");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_priority_round(&mut engine);
    answer_trigger_order_in_engine_order(&mut engine);
    assert_eq!(engine.state.stack.len(), 2);
    // Source departure after trigger capture does not remove these source-independent effects.
    engine.state.players[0]
        .battlefield
        .retain(|object| *object != source);
    engine.state.players[0].graveyard.push(source);
    engine.state.objects.get_mut(&source).unwrap().zone = Zone::Graveyard;
    engine
        .state
        .objects
        .get_mut(&source)
        .unwrap()
        .base_controller = 1;
    *engine
        .state
        .zone_change_generation
        .entry(source)
        .or_default() += 1;
    for _ in 0..2 {
        pass_priority_round(&mut engine);
    }
    assert!(engine.state.stack.is_empty());
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|player| player.life)
            .collect::<Vec<_>>(),
        [20, 18, 18, 18]
    );
}
