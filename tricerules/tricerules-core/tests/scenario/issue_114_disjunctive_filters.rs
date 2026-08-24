//! Issue #114: recursive OR filters use the same authoritative predicates at publication,
//! submission, and resolution.

use super::helpers::*;
use tricerules_cards::primitives::{
    CardTypeFilter, ContinuousEffectKind, CounterKind, EffectDuration, GraveyardFilter,
    SpellEffectKind, TriggerCondition, TriggeredAbilityDef,
};
use tricerules_cards::CardRegistry;
use tricerules_core::state::PendingTrigger;
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};

fn published_hand_targets(engine: &mut GameEngine, player: i32, card_id: &str) -> Vec<u32> {
    let slot = hand_index_for_card(engine, player as usize, card_id);
    engine.initial_response_batch().legal_by_player[&player].valid_targets_by_hand_slot
        [&((slot as u32) << 8)]
        .groups[0]
        .valid_permanent_ids
        .clone()
}

fn graveyard_fixture_ability(filter: GraveyardFilter) -> TriggeredAbilityDef {
    let mut ability = CardRegistry::global()
        .get("gravedigger")
        .expect("Gravedigger definition")
        .primary_face()
        .triggered_abilities[0]
        .clone();
    ability.trigger = TriggerCondition::WhenSelfEntersBattlefield;
    let SpellEffectKind::MoveGraveyardCards {
        filter: ability_filter,
        ..
    } = &mut ability.effect[0]
    else {
        panic!("Gravedigger fixture must return a graveyard card")
    };
    *ability_filter = filter;
    ability
}

fn publish_graveyard_fixture(engine: &mut GameEngine, filter: GraveyardFilter) -> (u32, Vec<u32>) {
    let source = inject_creature_on_battlefield(engine, 0, "grizzly_bears");
    let ability = graveyard_fixture_ability(filter);
    let trigger_id = engine.state.next_object_id;
    engine.state.next_object_id += 1;
    engine.state.pending_triggers.push_back(PendingTrigger {
        object_id: trigger_id,
        source_permanent_id: source,
        source_face_index: 0,
        source_zone_change: engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0),
        source_face_change: 0,
        ability_index: 0,
        ability: ability.clone(),
        ability_text: ability.text.clone(),
        card_id: "grizzly_bears".into(),
        controller: 0,
        may: ability.may,
        trigger_context: Default::default(),
    });
    let key = u64::from(source) << 32;
    let candidates = engine.initial_response_batch().legal_by_player[&0].valid_targets_by_ability
        [&key]
        .groups[0]
        .valid_graveyard_ids
        .clone();
    (source, candidates)
}

#[test]
fn cards_publish_the_exact_union_of_recursive_branches_without_duplicates() {
    let decks = Some(vec![
        deck_with("plains", &["make_your_move", "broken_wings"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(114_001, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);

    let artifact = inject_permanent_on_battlefield(&mut engine, 1, "short_sword");
    let enchantment = inject_permanent_on_battlefield(&mut engine, 1, "glorious_anthem");
    let power_four = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&power_four)
        .expect("power target")
        .counters
        .insert(CounterKind::PlusOnePlusOne, 2);
    let derived_flyer = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.state.continuous_effects.push(ContinuousEffect {
        source_id: None,
        affected: AffectedScope::Single(derived_flyer),
        kind: ContinuousEffectKind::Layer6AddKeyword(tricerules_cards::primitives::Keyword::Flying),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    let overlapping_artifact_flyer = inject_creature_on_battlefield(&mut engine, 1, "ornithopter");
    let ground_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    ensure_in_hand(&mut engine, 0, "make_your_move");
    ensure_in_hand(&mut engine, 0, "broken_wings");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            g: 1,
            c: 4,
            ..Default::default()
        },
    );

    assert_eq!(
        published_hand_targets(&mut engine, 0, "make_your_move"),
        vec![
            artifact,
            enchantment,
            power_four,
            overlapping_artifact_flyer
        ]
    );
    assert_eq!(
        published_hand_targets(&mut engine, 0, "broken_wings"),
        vec![
            artifact,
            enchantment,
            derived_flyer,
            overlapping_artifact_flyer
        ]
    );
    assert_ne!(ground_creature, land);
}

#[test]
fn forged_nonmatching_target_is_rejected_before_mana_or_card_movement() {
    let decks = Some(vec![
        deck_with("plains", &["make_your_move"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(114_002, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let illegal = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "make_your_move");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "make_your_move");
    let hand_before = engine.state.players[0].hand.clone();
    let mana_before = engine.state.players[0].mana_pool;

    assert!(engine
        .apply_command(0, &cast_spell(slot, target_object(illegal)))
        .is_err());
    assert_eq!(engine.state.players[0].hand, hand_before);
    let mana_after = engine.state.players[0].mana_pool;
    assert_eq!(
        (
            mana_after.white,
            mana_after.blue,
            mana_after.black,
            mana_after.red,
            mana_after.green,
            mana_after.colorless,
        ),
        (
            mana_before.white,
            mana_before.blue,
            mana_before.black,
            mana_before.red,
            mana_before.green,
            mana_before.colorless,
        )
    );
    assert_eq!(engine.state.objects[&illegal].zone, Zone::Battlefield);
}

#[test]
fn power_branch_is_revalidated_against_current_derived_power() {
    let decks = Some(vec![
        deck_with("plains", &["make_your_move"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(114_003, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&target)
        .expect("target")
        .counters
        .insert(CounterKind::PlusOnePlusOne, 2);
    ensure_in_hand(&mut engine, 0, "make_your_move");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "make_your_move");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast at a derived power-four creature");
    engine
        .state
        .objects
        .get_mut(&target)
        .expect("target")
        .counters
        .clear();

    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&target].zone,
        Zone::Battlefield,
        "CR 608.2b: the now-illegal sole target makes the spell fail to resolve"
    );
}

#[test]
fn graveyard_or_and_exclusion_fixtures_publish_and_revalidate_exact_candidates() {
    let mut say_engine =
        GameEngine::new(114_004, &[0, 1], 20, None, true).expect("Say Its Name fixture");
    advance_to_main1_from_game_start(&mut say_engine);
    let creature = inject_graveyard_card(&mut say_engine, 0, "grizzly_bears");
    let land = inject_graveyard_card(&mut say_engine, 0, "forest");
    inject_graveyard_card(&mut say_engine, 0, "short_sword");
    let (_, candidates) = publish_graveyard_fixture(
        &mut say_engine,
        GraveyardFilter {
            any_of: Some(vec![
                GraveyardFilter {
                    card_type: Some(CardTypeFilter::Creature),
                    ..Default::default()
                },
                GraveyardFilter {
                    card_type: Some(CardTypeFilter::Land),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        },
    );
    assert_eq!(candidates, vec![creature, land]);

    let mut messenger_engine =
        GameEngine::new(114_005, &[0, 1], 20, None, true).expect("Messenger fixture");
    advance_to_main1_from_game_start(&mut messenger_engine);
    inject_graveyard_card(&mut messenger_engine, 0, "grizzly_bears");
    inject_graveyard_card(&mut messenger_engine, 0, "forest");
    let artifact = inject_graveyard_card(&mut messenger_engine, 0, "short_sword");
    let instant = inject_graveyard_card(&mut messenger_engine, 0, "lightning_bolt");
    let (_, candidates) = publish_graveyard_fixture(
        &mut messenger_engine,
        GraveyardFilter {
            card_type: Some(CardTypeFilter::Noncreature),
            excluded_card_types: vec![CardTypeFilter::Land],
            ..Default::default()
        },
    );
    assert_eq!(candidates, vec![artifact, instant]);

    messenger_engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: target_object(instant),
                })),
            },
        )
        .expect("choose noncreature, nonland graveyard card");
    *messenger_engine
        .state
        .zone_change_generation
        .entry(instant)
        .or_default() += 1;
    pass_both_players(&mut messenger_engine);
    assert_eq!(
        messenger_engine.state.objects[&instant].zone,
        Zone::Graveyard
    );
}

#[test]
fn battlefield_target_generation_is_revalidated_through_the_recursive_filter() {
    let decks = Some(vec![
        deck_with("forest", &["broken_wings"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(114_006, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let target = inject_creature_on_battlefield(&mut engine, 1, "wind_drake");
    ensure_in_hand(&mut engine, 0, "broken_wings");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "broken_wings");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Broken Wings");
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 2;

    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Battlefield);
}
