//! Actual Luxior integration scenarios. Prepared state is called out where the command path is
//! not a legally reachable card sequence; the registered Luxior definition is used throughout.
use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_cards::primitives::CounterKind;
use tricerules_cards::{ContinuousEffectKind, EffectDuration};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1 as rv1;
use tricerules_proto::ruled::v1::attachment_recipient::Recipient;

const LUXIOR: &str = "luxior,_giadas_gift";
const AJANI: &str = "ajani_steadfast";
const ORNITHOPTER: &str = "ornithopter";

fn game(seed: u64) -> GameEngine {
    let registry = tricerules_cards::registry::global();
    assert!(
        registry.get(LUXIOR).is_some(),
        "actual Luxior must be admitted"
    );
    let mut engine = GameEngine::new(
        registry,
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("plains", &[]), deck_with("plains", &[])]),
        true,
    )
    .expect("create two-player actual-card game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_luxior(engine: &mut GameEngine) -> u32 {
    let object = inject_card_into_hand(engine, 0, LUXIOR);
    give_mana(
        engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, LUXIOR);
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast actual Luxior for {1}");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    pass_both_players(engine);
    assert_eq!(engine.state.objects[&object].zone, Zone::Battlefield);
    object
}

fn cast_actual_ornithopter(engine: &mut GameEngine) -> u32 {
    let object = inject_card_into_hand(engine, 0, ORNITHOPTER);
    let slot = hand_index_for_card(engine, 0, ORNITHOPTER);
    engine
        .apply_scenario_command(0, &cast_spell(slot, vec![]))
        .expect("cast an actual zero-cost Ornithopter to reach the priority checkpoint");
    assert_eq!(engine.state.objects[&object].zone, Zone::Stack);
    object
}

fn add_ajani(engine: &mut GameEngine, player: usize, loyalty: u32) -> u32 {
    let object = inject_permanent_on_battlefield(engine, player, AJANI);
    engine.state.zone_change_generation.insert(object, 0);
    if loyalty > 0 {
        engine
            .state
            .objects
            .get_mut(&object)
            .unwrap()
            .set_counter(CounterKind::Loyalty, loyalty);
        engine.initial_response_batch();
    }
    object
}

fn ability_index(card_id: &str, stable_id: &str) -> u32 {
    tricerules_cards::registry::global()
        .get(card_id)
        .unwrap_or_else(|| panic!("missing actual card {card_id}"))
        .primary_face()
        .activated_abilities
        .iter()
        .position(|ability| ability.ability_id.as_str() == stable_id)
        .unwrap_or_else(|| panic!("missing {stable_id} on {card_id}")) as u32
}

fn target_ref(engine: &GameEngine, object_id: u32) -> rv1::TargetRef {
    rv1::TargetRef {
        object_id,
        group_index: 0,
        kind: rv1::TargetRefKind::Permanent as i32,
        expected_zone_change_generation: engine
            .state
            .zone_change_generation
            .get(&object_id)
            .copied(),
        ..Default::default()
    }
}

fn apply_targeted_ability(
    engine: &mut GameEngine,
    player: i32,
    source: u32,
    ability_index: u32,
    target: u32,
) -> Result<RuledEventBatch, tricerules_core::EngineError> {
    let target = target_ref(engine, target);
    apply_ability(engine, player, source, ability_index, vec![target])
}

fn equip_actual_planeswalker(engine: &mut GameEngine, source: u32, recipient: u32) {
    give_mana(
        engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    apply_targeted_ability(
        engine,
        0,
        source,
        ability_index(LUXIOR, "activated_01"),
        recipient,
    )
    .expect("activate the actual Equip planeswalker ability");
    resolve_entire_stack_two_player(engine);
}

fn add_prepared_effect(
    engine: &mut GameEngine,
    object_id: u32,
    kind: ContinuousEffectKind,
) -> ContinuousEffect {
    let effect = ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(object_id),
        kind,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    };
    engine.state.continuous_effects.push(effect.clone());
    engine.initial_response_batch();
    effect
}

fn change_controller(engine: &mut GameEngine, object_id: u32, from: usize, to: usize) {
    engine.state.players[from]
        .battlefield
        .retain(|existing| *existing != object_id);
    engine.state.players[to].battlefield.push(object_id);
    let controller = engine.state.players[to].id;
    let object = engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("permanent to change control");
    object.base_controller = controller;
    object.controller = controller;
    engine.initial_response_batch();
}

fn published_battlefield_object(
    engine: &mut GameEngine,
    player: usize,
    object_id: u32,
) -> rv1::BattlefieldObject {
    let batch = engine.initial_response_batch();
    batch
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => view
                .per_player
                .get(player)
                .and_then(|player| {
                    player
                        .battlefield_objects
                        .iter()
                        .find(|object| object.object_id == object_id)
                })
                .cloned(),
            _ => None,
        })
        .expect("the public battlefield view includes this permanent")
}

fn published_attachment_id(object: &rv1::BattlefieldObject) -> Option<u32> {
    object
        .attachment_recipient
        .as_ref()
        .and_then(|recipient| recipient.recipient.as_ref())
        .and_then(|recipient| match recipient {
            Recipient::ObjectId(object_id) => Some(*object_id),
            _ => None,
        })
}

fn prepared_return_same_ajani(engine: &mut GameEngine, ajani: u32) {
    // The same-ObjectId/new-generation return is prepared state: the admitted corpus has no
    // immediate recipient blink effect to perform it within one normal resolution.
    let owner = engine.state.objects[&ajani].owner;
    let old_controller = engine.state.objects[&ajani].controller;
    let owner_seat = engine
        .state
        .players
        .iter()
        .position(|player| player.id == owner)
        .expect("owner seat");
    let old_controller_seat = engine
        .state
        .players
        .iter()
        .position(|player| player.id == old_controller)
        .expect("controller seat");
    let next_generation = engine
        .state
        .zone_change_generation
        .get(&ajani)
        .copied()
        .unwrap_or(0)
        .saturating_add(1);

    engine.state.players[old_controller_seat]
        .battlefield
        .retain(|existing| *existing != ajani);
    engine.state.players[owner_seat]
        .graveyard
        .retain(|existing| *existing != ajani);
    engine.state.players[owner_seat].battlefield.push(ajani);
    {
        let object = engine.state.objects.get_mut(&ajani).unwrap();
        object.zone = Zone::Battlefield;
        object.base_controller = owner;
        object.controller = owner;
        object.attached_to = None;
        object.counters.clear();
        object.counter_timestamps.clear();
        object.tapped = false;
        object.damage = 0;
        object.phase_status = None;
        object.set_counter(CounterKind::Loyalty, 1);
    }
    engine
        .state
        .zone_change_generation
        .insert(ajani, next_generation);
    engine.initial_response_batch();
}

#[test]
fn luxior_casts_and_resolves_both_printed_equips() {
    let mut engine = game(261_031_001);
    let luxior = cast_luxior(&mut engine);
    let ajani = add_ajani(&mut engine, 0, 4);
    let opponent_ajani = add_ajani(&mut engine, 1, 4);
    let ornithopter = inject_permanent_on_battlefield(&mut engine, 0, ORNITHOPTER);
    let planeswalker_equip = ability_index(LUXIOR, "activated_01");
    let creature_equip = ability_index(LUXIOR, "activated_02");

    assert!(apply_targeted_ability(&mut engine, 0, luxior, planeswalker_equip, ajani).is_err());
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert!(engine.state.stack.is_empty());

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let before = engine.state.players[0].mana_pool.colorless;
    assert!(
        apply_targeted_ability(&mut engine, 0, luxior, planeswalker_equip, ornithopter).is_err()
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, before);
    assert!(engine.state.stack.is_empty());

    let before = engine.state.players[0].mana_pool.colorless;
    assert!(
        apply_targeted_ability(&mut engine, 0, luxior, planeswalker_equip, opponent_ajani).is_err()
    );
    assert_eq!(engine.state.players[0].mana_pool.colorless, before);
    assert!(engine.state.stack.is_empty());

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let before = engine.state.players[0].mana_pool.colorless;
    assert!(apply_targeted_ability(&mut engine, 0, luxior, creature_equip, ajani).is_err());
    assert_eq!(engine.state.players[0].mana_pool.colorless, before);
    assert!(engine.state.stack.is_empty());

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let mana_before_planeswalker_equip = engine.state.players[0].mana_pool.colorless;
    apply_targeted_ability(&mut engine, 0, luxior, planeswalker_equip, ajani)
        .expect("pay the actual {1} Equip planeswalker ability");
    assert_eq!(
        engine.state.players[0].mana_pool.colorless,
        mana_before_planeswalker_equip - 1
    );
    assert_eq!(engine.state.stack.len(), 1);

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let before_timing_rejection = engine.state.players[0].mana_pool.colorless;
    assert!(apply_targeted_ability(&mut engine, 0, luxior, creature_equip, ornithopter).is_err());
    assert_eq!(
        engine.state.players[0].mana_pool.colorless, before_timing_rejection,
        "the second Equip is rejected at the stack window before paying {{3}}"
    );
    assert_eq!(engine.state.stack.len(), 1);

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&luxior].attached_to,
        Some(AttachmentRecipient::Object(ajani))
    );
    let attached = engine.characteristics(ajani).expect("actual Ajani");
    assert!(attached.is_creature());
    assert!(!attached.has_type("Planeswalker"));
    assert_eq!((attached.power, attached.toughness), (Some(4), Some(4)));

    apply_ability(
        &mut engine,
        0,
        ajani,
        ability_index(AJANI, "activated_01"),
        vec![],
    )
    .expect("Ajani's actual loyalty ability remains usable while equipped");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&ajani].counter_count(CounterKind::Loyalty),
        5
    );
    assert_eq!(engine.effective_power(ajani), Some(5));
    assert_eq!(engine.effective_toughness(ajani), Some(5));

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    let mana_before_creature_equip = engine.state.players[0].mana_pool.colorless;
    apply_targeted_ability(&mut engine, 0, luxior, creature_equip, ornithopter)
        .expect("pay the actual {3} creature Equip ability");
    assert_eq!(
        engine.state.players[0].mana_pool.colorless,
        mana_before_creature_equip - 3
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&luxior].attached_to,
        Some(AttachmentRecipient::Object(ornithopter))
    );
    let restored = engine.characteristics(ajani).expect("restored Ajani");
    assert!(!restored.is_creature());
    assert!(restored.has_type("Planeswalker"));
    assert_eq!(
        engine.state.objects[&ajani].counter_count(CounterKind::Loyalty),
        5
    );
    assert!(
        apply_ability(
            &mut engine,
            0,
            ajani,
            ability_index(AJANI, "activated_01"),
            vec![]
        )
        .is_err(),
        "Ajani's once-per-turn loyalty activation stays used after re-equipping"
    );
}

#[test]
fn luxior_counts_live_counter_kinds_without_setting_base_stats() {
    let mut engine = game(261_031_002);
    let luxior = cast_luxior(&mut engine);
    let ajani = add_ajani(&mut engine, 0, 4);
    engine
        .state
        .objects
        .get_mut(&ajani)
        .unwrap()
        .set_counter(CounterKind::Charge, 2);
    equip_actual_planeswalker(&mut engine, luxior, ajani);

    let initial = engine
        .characteristics(ajani)
        .expect("actual equipped Ajani");
    assert!(initial.is_creature());
    assert!(!initial.has_type("Planeswalker"));
    assert!(initial.is_legendary());
    assert!(!initial.types.iter().any(|kind| kind == "Ajani"));
    assert_eq!((initial.power, initial.toughness), (Some(6), Some(6)));

    engine
        .state
        .objects
        .get_mut(&ajani)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    engine.initial_response_batch();
    assert_eq!(
        (
            engine.effective_power(ajani),
            engine.effective_toughness(ajani)
        ),
        (Some(8), Some(8)),
        "a +1/+1 counter contributes its own delta and Luxior's all-counter bonus"
    );

    {
        let object = engine.state.objects.get_mut(&ajani).unwrap();
        object.counters.remove(&CounterKind::PlusOnePlusOne);
        object
            .counter_timestamps
            .remove(&CounterKind::PlusOnePlusOne);
        object.set_counter(CounterKind::MinusOneMinusOne, 1);
    }
    engine.initial_response_batch();
    assert_eq!(
        (
            engine.effective_power(ajani),
            engine.effective_toughness(ajani)
        ),
        (Some(6), Some(6)),
        "a -1/-1 counter applies its ordinary delta separately from Luxior"
    );

    {
        let object = engine.state.objects.get_mut(&ajani).unwrap();
        object.counters.remove(&CounterKind::MinusOneMinusOne);
        object
            .counter_timestamps
            .remove(&CounterKind::MinusOneMinusOne);
    }
    add_prepared_effect(
        &mut engine,
        ajani,
        ContinuousEffectKind::Layer7bSetPt {
            power: 2,
            toughness: 3,
        },
    );
    assert_eq!(
        (
            engine.effective_power(ajani),
            engine.effective_toughness(ajani)
        ),
        (Some(8), Some(9)),
        "Luxior modifies an independent 2/3 base instead of setting 0/0"
    );

    engine
        .state
        .objects
        .get_mut(&ajani)
        .unwrap()
        .set_counter(CounterKind::Charge, 1);
    engine.initial_response_batch();
    assert_eq!(
        (
            engine.effective_power(ajani),
            engine.effective_toughness(ajani)
        ),
        (Some(7), Some(8)),
        "the attached all-counter quantity updates after counter removal"
    );

    add_prepared_effect(
        &mut engine,
        luxior,
        ContinuousEffectKind::Layer6RemoveAllAbilities,
    );
    let after_source_ability_loss = engine.characteristics(ajani).expect("Ajani remains");
    assert!(after_source_ability_loss.is_creature());
    assert!(!after_source_ability_loss.has_type("Planeswalker"));
    assert!(after_source_ability_loss.is_legendary());
    assert!(!after_source_ability_loss
        .types
        .iter()
        .any(|kind| kind == "Ajani"));
    assert_eq!(
        (
            after_source_ability_loss.power,
            after_source_ability_loss.toughness
        ),
        (Some(2), Some(3)),
        "the started layer-4 type ability continues while the distinct layer-7 count bonus stops"
    );
}

#[test]
fn luxior_separate_statics_preserve_layer_and_loyalty_sba_distinctions() {
    // The previous counter test covers separate ability loss. Here the actual definition's
    // type static is checked against its planeswalker subtype and retained Legendary supertype.
    let mut engine = game(261_031_003);
    let luxior = cast_luxior(&mut engine);
    let ajani = add_ajani(&mut engine, 0, 2);
    engine
        .state
        .objects
        .get_mut(&ajani)
        .unwrap()
        .set_counter(CounterKind::Charge, 1);
    equip_actual_planeswalker(&mut engine, luxior, ajani);

    let attached = engine.characteristics(ajani).expect("actual Ajani");
    assert!(attached.is_creature());
    assert!(!attached.has_type("Planeswalker"));
    assert!(attached.is_legendary());
    assert!(!attached.types.iter().any(|kind| kind == "Ajani"));
    assert_eq!((attached.power, attached.toughness), (Some(3), Some(3)));

    // Prepared direct counter removal leaves the source and attachment in place while Ajani's
    // base P/T effect keeps the creature alive, exposing the different static origins.
    add_prepared_effect(
        &mut engine,
        ajani,
        ContinuousEffectKind::Layer7bSetPt {
            power: 1,
            toughness: 1,
        },
    );
    add_prepared_effect(
        &mut engine,
        luxior,
        ContinuousEffectKind::Layer6RemoveAllAbilities,
    );
    let after_loss = engine.characteristics(ajani).expect("actual Ajani remains");
    assert!(after_loss.is_creature());
    assert!(!after_loss.has_type("Planeswalker"));
    assert!(after_loss.is_legendary());
    assert_eq!((after_loss.power, after_loss.toughness), (Some(1), Some(1)));

    let mut loyalty_sba = game(261_031_013);
    let loyalty_luxior = cast_luxior(&mut loyalty_sba);
    let loyalty_ajani = add_ajani(&mut loyalty_sba, 0, 1);
    loyalty_sba
        .state
        .objects
        .get_mut(&loyalty_ajani)
        .unwrap()
        .set_counter(CounterKind::Charge, 1);
    let ornithopter = inject_permanent_on_battlefield(&mut loyalty_sba, 0, ORNITHOPTER);
    loyalty_sba
        .state
        .zone_change_generation
        .insert(ornithopter, 0);
    equip_actual_planeswalker(&mut loyalty_sba, loyalty_luxior, loyalty_ajani);
    give_mana(
        &mut loyalty_sba,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    apply_targeted_ability(
        &mut loyalty_sba,
        0,
        loyalty_luxior,
        ability_index(LUXIOR, "activated_02"),
        ornithopter,
    )
    .expect("activate ordinary Equip to detach the one-loyalty planeswalker");
    loyalty_sba
        .state
        .objects
        .get_mut(&loyalty_ajani)
        .unwrap()
        .counters
        .remove(&CounterKind::Loyalty);
    loyalty_sba
        .state
        .objects
        .get_mut(&loyalty_ajani)
        .unwrap()
        .counter_timestamps
        .remove(&CounterKind::Loyalty);
    resolve_entire_stack_two_player(&mut loyalty_sba);
    assert_eq!(
        loyalty_sba.state.objects[&loyalty_ajani].zone,
        Zone::Graveyard,
        "without Luxior's type ability, the actual Equip resolution checkpoint processes the zero-loyalty SBA"
    );
    assert_eq!(
        loyalty_sba.state.objects[&loyalty_luxior].attached_to,
        Some(AttachmentRecipient::Object(ornithopter))
    );
}

#[test]
fn luxior_equip_resolution_preserves_controller_and_incarnation_boundaries() {
    let planeswalker_equip = ability_index(LUXIOR, "activated_01");

    let mut source_control = game(261_031_004);
    let luxior = cast_luxior(&mut source_control);
    let ajani = add_ajani(&mut source_control, 0, 4);
    give_mana(
        &mut source_control,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    apply_targeted_ability(&mut source_control, 0, luxior, planeswalker_equip, ajani)
        .expect("player A activates actual Equip planeswalker");
    change_controller(&mut source_control, luxior, 0, 1);
    resolve_entire_stack_two_player(&mut source_control);
    assert_eq!(
        source_control.state.objects[&luxior].attached_to,
        Some(AttachmentRecipient::Object(ajani)),
        "source-only control change does not counter the stacked ability"
    );
    assert_eq!(source_control.state.objects[&luxior].controller, 1);
    assert_eq!(source_control.state.objects[&ajani].controller, 0);

    let mut target_control = game(261_031_005);
    let luxior = cast_luxior(&mut target_control);
    let ajani = add_ajani(&mut target_control, 0, 4);
    give_mana(
        &mut target_control,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    apply_targeted_ability(&mut target_control, 0, luxior, planeswalker_equip, ajani)
        .expect("player A activates actual Equip planeswalker");
    change_controller(&mut target_control, ajani, 0, 1);
    resolve_entire_stack_two_player(&mut target_control);
    assert_eq!(target_control.state.objects[&luxior].attached_to, None);
    assert_eq!(target_control.state.objects[&ajani].controller, 1);
}

#[test]
fn luxior_recipient_departure_clears_public_link_and_statics_before_prepared_return() {
    let mut engine = game(261_031_006);
    let luxior = cast_luxior(&mut engine);
    let ajani = add_ajani(&mut engine, 0, 1);
    engine
        .state
        .objects
        .get_mut(&ajani)
        .unwrap()
        .set_counter(CounterKind::Charge, 1);
    engine.initial_response_batch();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    apply_targeted_ability(
        &mut engine,
        0,
        luxior,
        ability_index(LUXIOR, "activated_01"),
        ajani,
    )
    .expect("attach the actual Luxior by its Oracle-order stable role");
    resolve_entire_stack_two_player(&mut engine);

    let source_generation = engine.state.zone_change_generation[&luxior];
    let recipient_generation = engine.state.zone_change_generation[&ajani];
    assert_eq!(engine.state.objects[&luxior].card_id, LUXIOR);
    assert_eq!(
        engine.state.objects[&luxior].attached_to,
        Some(AttachmentRecipient::Object(ajani))
    );
    let before_departure = published_battlefield_object(&mut engine, 0, luxior);
    assert_eq!(published_attachment_id(&before_departure), Some(ajani));

    // Removing both counters makes the newly animated, undefined-P/T Ajani a 0/0. Its actual
    // SBA departure runs through the engine's central zone-move path.
    {
        let object = engine.state.objects.get_mut(&ajani).unwrap();
        object.counters.clear();
        object.counter_timestamps.clear();
    }
    let ornithopter = cast_actual_ornithopter(&mut engine);
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&ornithopter].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&ajani].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.zone_change_generation[&ajani],
        recipient_generation + 1
    );
    assert_eq!(engine.state.objects[&luxior].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.zone_change_generation[&luxior],
        source_generation
    );
    assert_eq!(engine.state.objects[&luxior].attached_to, None);
    let after_departure = published_battlefield_object(&mut engine, 0, luxior);
    assert_eq!(
        published_attachment_id(&after_departure),
        None,
        "the actual Luxior public source field clears when its recipient leaves"
    );

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    apply_targeted_ability(
        &mut engine,
        0,
        luxior,
        ability_index(LUXIOR, "activated_02"),
        ornithopter,
    )
    .expect("equip the resolved actual Ornithopter after the departure proof");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&luxior].attached_to,
        Some(AttachmentRecipient::Object(ornithopter))
    );

    prepared_return_same_ajani(&mut engine, ajani);
    assert_eq!(engine.state.objects[&ajani].zone, Zone::Battlefield);
    assert_eq!(
        engine.state.zone_change_generation[&ajani],
        recipient_generation + 2
    );
    assert_eq!(
        engine.state.objects[&luxior].attached_to,
        Some(AttachmentRecipient::Object(ornithopter))
    );
    let returned = engine.characteristics(ajani).expect("returned Ajani");
    assert!(returned.has_type("Planeswalker"));
    assert!(!returned.is_creature());
    assert_eq!((returned.power, returned.toughness), (None, None));

    let before_reequip = engine.state.command_index;
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    apply_targeted_ability(
        &mut engine,
        0,
        luxior,
        ability_index(LUXIOR, "activated_01"),
        ajani,
    )
    .expect("a fresh explicit Equip is needed for the returned incarnation");
    resolve_entire_stack_two_player(&mut engine);
    assert!(engine.state.command_index > before_reequip);
    assert_eq!(
        engine.state.objects[&luxior].attached_to,
        Some(AttachmentRecipient::Object(ajani))
    );
    assert_eq!(
        (
            engine.effective_power(ajani),
            engine.effective_toughness(ajani)
        ),
        (Some(1), Some(1))
    );
}
