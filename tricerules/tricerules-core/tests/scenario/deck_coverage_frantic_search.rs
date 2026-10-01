//! Actual Frantic Search: private draw/discard, then untargeted simultaneous chosen-land untap.

use super::helpers::*;
use tricerules_cards::{CardRegistry, ContinuousEffectKind, CounterKind, EffectDuration, Keyword};
use tricerules_core::{AffectedScope, ContinuousEffect, GameEngine, Zone};
use tricerules_proto::ruled::v1::{ChoiceKind, RuledEventBatch};

const SEARCH: &str = "frantic_search";

fn setup() -> GameEngine {
    let deck = deck_with("island", &[]);
    let mut engine = GameEngine::new(
        26_100_703,
        &[10, 20, 30],
        20,
        Some(vec![deck.clone(), deck.clone(), deck]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_and_discard(engine: &mut GameEngine) -> RuledEventBatch {
    inject_card_into_hand(engine, 0, SEARCH);
    give_mana(
        engine,
        10,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, SEARCH);
    engine.apply_command(10, &cast_spell(slot, vec![])).unwrap();
    let batch = resolve_one(engine);
    let discard = find_resolution_choice(&batch).expect("discard before land choice");
    assert_eq!(discard.choice_kind(), ChoiceKind::HandCards);
    engine
        .apply_command(
            10,
            &submit_resolution_choice(discard.candidate_object_ids[..2].to_vec()),
        )
        .unwrap()
}

fn reject(engine: &mut GameEngine, actor: i32, chosen: Vec<u32>) {
    let before = format!("{:?}", engine.state);
    engine
        .apply_command(actor, &submit_resolution_choice(chosen))
        .unwrap_err();
    assert_eq!(format!("{:?}", engine.state), before);
}

fn modify(engine: &mut GameEngine, object: u32, kind: ContinuousEffectKind) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(object),
        kind,
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
}

fn resolve_one(e: &mut GameEngine) -> RuledEventBatch {
    let mut batch = RuledEventBatch::default();
    for _ in 0..e.state.players.len() {
        let actor = e.state.priority_player_id();
        batch = e.apply_command(actor, &pass()).unwrap();
    }
    batch
}

#[test]
fn frantic_search_rejects_illegal_and_stale_land_choices_without_mutation() {
    let mut engine = setup();
    let lands = (0..4)
        .map(|index| inject_permanent_on_battlefield(&mut engine, index % 3, "island"))
        .collect::<Vec<_>>();
    for oid in &lands {
        engine.state.objects.get_mut(oid).unwrap().tapped = true;
    }
    let nonland = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let batch = cast_and_discard(&mut engine);
    let choice = find_resolution_choice(&batch).unwrap();
    assert!(!choice.candidate_object_ids.contains(&nonland));
    reject(&mut engine, 20, vec![lands[1]]);
    reject(&mut engine, 10, vec![lands[0], lands[0]]);
    reject(&mut engine, 10, vec![nonland]);
    reject(&mut engine, 10, lands.clone());
    reject(&mut engine, 10, vec![u32::MAX]);
    let oid = lands[0];
    *engine.state.zone_change_generation.entry(oid).or_default() += 1;
    reject(&mut engine, 10, vec![oid]);
    engine
        .apply_command(10, &submit_resolution_choice(vec![lands[1]]))
        .unwrap();
    assert!(engine.state.objects[&lands[0]].tapped);
    assert!(!engine.state.objects[&lands[1]].tapped);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn frantic_search_untargeted_lands_include_opponents_and_respect_stun_and_prohibition() {
    let mut engine = setup();
    let protected = inject_permanent_on_battlefield(&mut engine, 1, "island");
    let stunned = inject_permanent_on_battlefield(&mut engine, 2, "forest");
    let prohibited = inject_permanent_on_battlefield(&mut engine, 0, "mountain");
    let untapped = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    for oid in [protected, stunned, prohibited] {
        engine.state.objects.get_mut(&oid).unwrap().tapped = true;
    }
    engine
        .state
        .objects
        .get_mut(&stunned)
        .unwrap()
        .set_counter(CounterKind::Stun, 2);
    engine
        .state
        .objects
        .get_mut(&prohibited)
        .unwrap()
        .set_counter(CounterKind::Stun, 2);
    modify(
        &mut engine,
        protected,
        ContinuousEffectKind::Layer6AddKeyword(Keyword::Hexproof),
    );
    modify(
        &mut engine,
        protected,
        ContinuousEffectKind::Layer6AddKeyword(Keyword::Shroud),
    );
    modify(&mut engine, prohibited, ContinuousEffectKind::ProhibitUntap);
    let batch = cast_and_discard(&mut engine);
    let choice = find_resolution_choice(&batch).unwrap();
    for oid in [protected, stunned, prohibited, untapped] {
        assert!(choice.candidate_object_ids.contains(&oid));
    }
    engine
        .apply_command(
            10,
            &submit_resolution_choice(vec![protected, stunned, prohibited]),
        )
        .unwrap();
    assert!(!engine.state.objects[&protected].tapped);
    assert!(engine.state.objects[&stunned].tapped);
    assert_eq!(
        engine.state.objects[&stunned].counter_count(CounterKind::Stun),
        1
    );
    assert!(engine.state.objects[&prohibited].tapped);
    assert_eq!(
        engine.state.objects[&prohibited].counter_count(CounterKind::Stun),
        2
    );
    assert!(!engine.state.objects[&untapped].tapped);
    // An already untapped land is legal and contributes no untap edge.
    let batch = cast_and_discard(&mut engine);
    assert!(find_resolution_choice(&batch)
        .unwrap()
        .candidate_object_ids
        .contains(&untapped));
    engine
        .apply_command(10, &submit_resolution_choice(vec![untapped]))
        .unwrap();
    assert!(!engine.state.objects[&untapped].tapped);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn frantic_search_no_lands_finishes_after_private_discard() {
    let mut engine = setup();
    let hand_before = engine.state.players[0].hand.len();
    let library_before = engine.state.players[0].library.len();
    let batch = cast_and_discard(&mut engine);
    assert!(find_resolution_choice(&batch).is_none());
    assert_eq!(engine.state.players[0].hand.len(), hand_before);
    assert_eq!(engine.state.players[0].library.len(), library_before - 2);
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.stack.is_empty());
}

#[test]
fn frantic_search_with_only_the_spell_in_hand_draws_and_discards_both_cards() {
    let mut engine = setup();
    let old_hand = std::mem::take(&mut engine.state.players[0].hand);
    for oid in old_hand {
        engine.state.objects.get_mut(&oid).unwrap().zone = Zone::Graveyard;
        engine.state.players[0].graveyard.push(oid);
    }
    let library_before = engine.state.players[0].library.len();
    let batch = cast_and_discard(&mut engine);
    assert!(find_resolution_choice(&batch).is_none());
    assert!(engine.state.players[0].hand.is_empty());
    assert_eq!(engine.state.players[0].library.len(), library_before - 2);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn frantic_search_paid_cast_and_both_choices_replay_deterministically() {
    fn prepared() -> (GameEngine, Vec<u32>, usize) {
        let mut engine = setup();
        let lands = (0..3)
            .map(|seat| inject_permanent_on_battlefield(&mut engine, seat, "island"))
            .collect::<Vec<_>>();
        for oid in &lands {
            engine.state.objects.get_mut(oid).unwrap().tapped = true;
        }
        inject_card_into_hand(&mut engine, 0, SEARCH);
        give_mana(
            &mut engine,
            10,
            ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&engine, 0, SEARCH);
        (engine, lands, slot)
    }
    let (mut engine, lands, slot) = prepared();
    let mut log = Vec::new();
    let mut record =
        |engine: &mut GameEngine, actor, command: tricerules_proto::ruled::v1::RuledCommand| {
            let batch = engine.apply_command(actor, &command).unwrap();
            log.push((actor, command, batch.clone()));
            batch
        };
    record(&mut engine, 10, cast_spell(slot, vec![]));
    let mut batch = RuledEventBatch::default();
    for _ in 0..3 {
        let actor = engine.state.priority_player_id();
        batch = record(&mut engine, actor, pass());
    }
    let discard = find_resolution_choice(&batch).unwrap();
    batch = record(
        &mut engine,
        10,
        submit_resolution_choice(discard.candidate_object_ids[..2].to_vec()),
    );
    assert_eq!(
        find_resolution_choice(&batch).unwrap().choice_kind(),
        ChoiceKind::PermanentObjects
    );
    record(&mut engine, 10, submit_resolution_choice(lands));
    let (mut replay, _, _) = prepared();
    for (actor, command, expected) in log {
        assert_eq!(replay.apply_command(actor, &command).unwrap(), expected);
    }
    assert_eq!(
        serde_json::to_value(&replay.state).unwrap(),
        serde_json::to_value(&engine.state).unwrap()
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn frantic_search_paid_cast_private_discard_then_zero_one_three_lands() {
    let definition = CardRegistry::global()
        .get(SEARCH)
        .expect("exact original missing Frantic Search registered");
    assert_eq!(definition.name, "Frantic Search");
    assert_eq!(definition.face_count(), 1);
    assert_eq!(definition.primary_face().mana_cost.to_string(), "{2}{U}");
    assert_eq!(definition.primary_face().types, ["Instant"]);

    for count in [0, 1, 3] {
        let deck = deck_with("island", &[]);
        let mut e = GameEngine::new(
            26_100_702,
            &[10, 20, 30],
            20,
            Some(vec![deck.clone(), deck.clone(), deck]),
            true,
        )
        .unwrap();
        advance_to_main1_from_game_start(&mut e);
        let lands = (0..3)
            .map(|seat| inject_permanent_on_battlefield(&mut e, seat, "island"))
            .collect::<Vec<_>>();
        let unselected = inject_permanent_on_battlefield(&mut e, 0, "mountain");
        for oid in lands.iter().chain([&unselected]) {
            e.state.objects.get_mut(oid).unwrap().tapped = true;
        }
        let spell = inject_card_into_hand(&mut e, 0, SEARCH);
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        give_mana(
            &mut e,
            10,
            ManaGift {
                u: 1,
                c: 2,
                ..Default::default()
            },
        );
        let slot = hand_index_for_card(&e, 0, SEARCH);
        e.apply_command(10, &cast_spell(slot, vec![])).unwrap();
        assert_eq!(e.state.players[0].mana_pool, Default::default());
        let batch = resolve_one(&mut e);
        let discard =
            find_resolution_choice(&batch).expect("private discard interrupts resolution");
        assert_eq!(discard.deciding_player_id, 10);
        assert_eq!(discard.choice_kind(), ChoiceKind::HandCards);
        assert_eq!((discard.min, discard.max), (2, 2));
        assert_eq!(e.state.players[0].library.len(), library_before - 2);
        assert_eq!(e.state.players[0].hand.len(), hand_before + 1);
        assert!(e.apply_command(20, &pass()).is_err());
        let discarded = discard.candidate_object_ids[..2].to_vec();
        let batch = e
            .apply_command(10, &submit_resolution_choice(discarded.clone()))
            .unwrap();
        assert!(discarded
            .iter()
            .all(|oid| e.state.objects[oid].zone == Zone::Graveyard));
        let choice =
            find_resolution_choice(&batch).expect("public land choice follows private discard");
        assert_eq!(choice.choice_kind(), ChoiceKind::PermanentObjects);
        assert_eq!(choice.deciding_player_id, 10);
        assert_eq!((choice.min, choice.max), (0, 3));
        assert!(choice.prompt_text.contains("lands"));
        assert!(lands
            .iter()
            .all(|oid| choice.candidate_object_ids.contains(oid)));
        assert!(e.apply_command(30, &pass()).is_err());
        e.apply_command(10, &submit_resolution_choice(lands[..count].to_vec()))
            .unwrap();
        for (index, oid) in lands.iter().enumerate() {
            assert_eq!(e.state.objects[oid].tapped, index >= count);
        }
        assert!(e.state.objects[&unselected].tapped);
        assert_eq!(e.state.players[0].hand.len(), hand_before - 1);
        assert_eq!(e.state.objects[&spell].zone, Zone::Graveyard);
        assert!(e.state.pending_resolution.is_none());
        assert!(e.state.stack.is_empty());
    }
}
