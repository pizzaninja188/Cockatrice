use super::helpers::*;
use tricerules_cards::primitives::CounterKind;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::permanent_moved::Destination;

#[test]
fn totally_lost_places_nonland_permanent_on_owners_library_top() {
    let decks = Some(vec![
        {
            let mut deck = vec!["totally_lost".to_string()];
            deck.extend(std::iter::repeat_n("island".to_string(), 29));
            deck
        },
        std::iter::repeat_n("forest".to_string(), 30).collect(),
    ]);
    let mut engine = GameEngine::new(8901, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);

    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.state.players[1]
        .battlefield
        .retain(|&oid| oid != target);
    engine.state.players[0].battlefield.push(target);
    let controlled_object = engine.state.objects.get_mut(&target).expect("target");
    controlled_object.base_controller = 0;
    controlled_object.controller = 0;
    controlled_object.tapped = true;
    controlled_object.damage = 1;
    controlled_object
        .counters
        .insert(CounterKind::PlusOnePlusOne, 1);
    let generation_before = engine
        .state
        .zone_change_generation
        .get(&target)
        .copied()
        .unwrap_or(0);
    ensure_in_hand(&mut engine, 0, "totally_lost");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 4,
            ..Default::default()
        },
    );

    let spell = hand_index_for_card(&engine, 0, "totally_lost");
    engine
        .apply_command(0, &cast_spell(spell, target_object(target)))
        .expect("cast Totally Lost");
    engine.apply_command(0, &pass()).expect("caster passes");
    let batch = engine
        .apply_command(1, &pass())
        .expect("opponent passes and spell resolves");

    let object = engine.state.objects.get(&target).expect("target");
    assert_eq!(object.zone, Zone::Library);
    assert_eq!(
        object.owner, 1,
        "the owner determines the destination library"
    );
    assert_eq!(
        object.controller, 1,
        "nonbattlefield objects reset to their owner"
    );
    assert!(!object.tapped);
    assert_eq!(object.damage, 0);
    assert!(object.counters.is_empty());
    assert_eq!(
        engine.state.zone_change_generation.get(&target).copied(),
        Some(generation_before + 1)
    );
    assert_eq!(
        engine.state.players[1].library.front().copied(),
        Some(target),
        "the permanent is the top card of its owner's library"
    );
    assert!(!engine.state.players[0].battlefield.contains(&target));
    assert!(permanents_moved_in(&batch).iter().any(|moved| {
        moved.object_id == target
            && moved.owner_player_id == 1
            && moved.destination == Destination::Library as i32
    }));
}

#[test]
fn totally_lost_rejects_land_targets() {
    let decks = Some(vec![
        {
            let mut deck = vec!["totally_lost".to_string()];
            deck.extend(std::iter::repeat_n("island".to_string(), 29));
            deck
        },
        std::iter::repeat_n("forest".to_string(), 30).collect(),
    ]);
    let mut engine = GameEngine::new(8902, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    ensure_in_hand(&mut engine, 0, "totally_lost");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 4,
            ..Default::default()
        },
    );

    let spell = hand_index_for_card(&engine, 0, "totally_lost");
    assert!(
        engine
            .apply_command(0, &cast_spell(spell, target_object(land)))
            .is_err(),
        "Totally Lost cannot target a land"
    );
    assert_eq!(
        engine.state.objects.get(&land).expect("land").zone,
        Zone::Battlefield
    );
}

fn resolve_deglamer(seed: u64) -> (GameEngine, u32) {
    let decks = Some(vec![
        {
            let mut deck = vec!["deglamer".to_string()];
            deck.extend(std::iter::repeat_n("forest".to_string(), 29));
            deck
        },
        {
            let mut deck = vec!["bonesplitter".to_string()];
            deck.extend(std::iter::repeat_n("island".to_string(), 29));
            deck
        },
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let target = deploy_to_battlefield(&mut engine, 1, "bonesplitter", false);
    ensure_in_hand(&mut engine, 0, "deglamer");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );

    let spell = hand_index_for_card(&engine, 0, "deglamer");
    engine
        .apply_command(0, &cast_spell(spell, target_object(target)))
        .expect("cast Deglamer");
    engine.apply_command(0, &pass()).expect("caster passes");
    engine
        .apply_command(1, &pass())
        .expect("opponent passes and spell resolves");
    (engine, target)
}

#[test]
fn deglamer_includes_the_permanent_in_a_deterministic_owner_library_shuffle() {
    let (first, first_target) = resolve_deglamer(8903);
    let (replay, replay_target) = resolve_deglamer(8903);

    assert_eq!(first_target, replay_target);
    assert_eq!(
        first.state.players[1].library, replay.state.players[1].library,
        "the same seed and command log reproduce the same shuffled order"
    );
    assert!(first.state.players[1].library.contains(&first_target));
    let object = first.state.objects.get(&first_target).expect("target");
    assert_eq!(object.zone, Zone::Library);
    assert_eq!(object.owner, 1);
    assert_eq!(object.controller, 1);
}

#[test]
fn griptide_fizzles_after_its_target_changes_zones() {
    let decks = Some(vec![
        {
            let mut deck = vec!["griptide".to_string()];
            deck.extend(std::iter::repeat_n("island".to_string(), 29));
            deck
        },
        std::iter::repeat_n("forest".to_string(), 30).collect(),
    ]);
    let mut engine = GameEngine::new(8904, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "griptide");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );

    let spell = hand_index_for_card(&engine, 0, "griptide");
    engine
        .apply_command(0, &cast_spell(spell, target_object(target)))
        .expect("cast Griptide");
    engine.state.players[1]
        .battlefield
        .retain(|&oid| oid != target);
    engine.state.players[1].graveyard.push(target);
    engine.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;

    engine.apply_command(0, &pass()).expect("caster passes");
    engine.apply_command(1, &pass()).expect("spell fizzles");
    assert_eq!(
        engine.state.objects.get(&target).expect("target").zone,
        Zone::Graveyard
    );
    assert!(!engine.state.players[1].library.contains(&target));
}

#[test]
fn a_token_put_into_a_library_ceases_to_exist() {
    let decks = Some(vec![
        {
            let mut deck = vec!["raise_the_alarm".to_string(), "totally_lost".to_string()];
            deck.extend(std::iter::repeat_n("plains".to_string(), 28));
            deck
        },
        std::iter::repeat_n("forest".to_string(), 30).collect(),
    ]);
    let mut engine = GameEngine::new(8905, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "raise_the_alarm");
    ensure_in_hand(&mut engine, 0, "totally_lost");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            u: 1,
            c: 5,
            ..Default::default()
        },
    );

    let raise = hand_index_for_card(&engine, 0, "raise_the_alarm");
    engine
        .apply_command(0, &cast_spell(raise, vec![]))
        .expect("cast Raise the Alarm");
    engine.apply_command(0, &pass()).expect("caster passes");
    engine.apply_command(1, &pass()).expect("tokens resolve");
    let token = battlefield_token_oids(&engine, 0, "soldier_w_1_1")[0];

    let totally_lost = hand_index_for_card(&engine, 0, "totally_lost");
    engine
        .apply_command(0, &cast_spell(totally_lost, target_object(token)))
        .expect("cast Totally Lost targeting the token");
    engine.apply_command(0, &pass()).expect("caster passes");
    engine.apply_command(1, &pass()).expect("spell resolves");

    assert!(!engine.state.objects.contains_key(&token));
    assert!(!engine.state.players[0].library.contains(&token));
}
