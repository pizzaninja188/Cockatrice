use crate::helpers::*;
use tricerules_cards::primitives::{ContinuousEffectKind, CounterKind, EffectDuration, Keyword};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};

fn bite_targets(source: u32, recipient: u32) -> Vec<TargetRef> {
    vec![
        TargetRef {
            object_id: source,
            group_index: 0,
            kind: TargetRefKind::Permanent as i32,
            ..Default::default()
        },
        TargetRef {
            object_id: recipient,
            group_index: 1,
            kind: TargetRefKind::Permanent as i32,
            ..Default::default()
        },
    ]
}

#[test]
fn rabid_bite_publishes_independent_groups_and_rejects_forged_roles_atomically() {
    let decks = Some(vec![
        deck_with("forest", &["rabid_bite", "grizzly_bears"]),
        deck_with("forest", &["hill_giant"]),
    ]);
    let mut engine = GameEngine::new(85_100, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let mine = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let theirs = relocate_to_battlefield(&mut engine, 1, "hill_giant", false);
    ensure_in_hand(&mut engine, 0, "rabid_bite");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 2,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, "rabid_bite");
    let batch = engine.initial_response_batch();
    let targets = &batch.legal_by_player[&0].valid_targets_by_hand_slot[&((slot as u32) << 8)];
    assert_eq!(targets.groups.len(), 2);
    assert_eq!(targets.groups[0].valid_permanent_ids, [mine]);
    assert_eq!(targets.groups[1].valid_permanent_ids, [theirs]);
    assert_eq!(targets.groups[1].distinct_from_group_indices, [0]);

    let hand_before = engine.state.players[0].hand.clone();
    let mana_before = engine.state.players[0].mana_pool.green;
    for invalid in [
        bite_targets(theirs, mine),
        vec![bite_targets(mine, theirs)[0]],
    ] {
        assert!(engine.apply_command(0, &cast_spell(slot, invalid)).is_err());
        assert_eq!(engine.state.players[0].hand, hand_before);
        assert_eq!(engine.state.players[0].mana_pool.green, mana_before);
    }
}

#[test]
fn rabid_bite_uses_the_sources_current_power() {
    let decks = Some(vec![
        deck_with("forest", &["rabid_bite", "grizzly_bears"]),
        deck_with("forest", &["colossal_dreadmaw"]),
    ]);
    let mut engine = GameEngine::new(85_101, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let source = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let recipient = relocate_to_battlefield(&mut engine, 1, "colossal_dreadmaw", false);
    ensure_in_hand(&mut engine, 0, "rabid_bite");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 2,
            ..Default::default()
        },
    );

    let spell = hand_index_for_card(&engine, 0, "rabid_bite");
    engine
        .apply_command(0, &cast_spell(spell, bite_targets(source, recipient)))
        .expect("cast Rabid Bite");
    engine
        .state
        .objects
        .get_mut(&source)
        .expect("source")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&recipient].damage, 3);
}

#[test]
fn hunters_edge_applies_its_counter_before_calculating_damage() {
    let decks = Some(vec![
        deck_with("forest", &["hunters_edge", "grizzly_bears"]),
        deck_with("forest", &["hill_giant"]),
    ]);
    let mut engine = GameEngine::new(85_102, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let source = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let recipient = relocate_to_battlefield(&mut engine, 1, "hill_giant", false);
    ensure_in_hand(&mut engine, 0, "hunters_edge");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 4,
            ..Default::default()
        },
    );

    let spell = hand_index_for_card(&engine, 0, "hunters_edge");
    engine
        .apply_command(0, &cast_spell(spell, bite_targets(source, recipient)))
        .expect("cast Hunter's Edge");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(engine.state.objects[&recipient].zone, Zone::Graveyard);
}

#[test]
fn hunters_edge_still_adds_the_counter_when_only_the_recipient_is_illegal() {
    let decks = Some(vec![
        deck_with("forest", &["hunters_edge", "grizzly_bears"]),
        deck_with("forest", &["hill_giant"]),
    ]);
    let mut engine = GameEngine::new(85_103, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let source = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let recipient = relocate_to_battlefield(&mut engine, 1, "hill_giant", false);
    ensure_in_hand(&mut engine, 0, "hunters_edge");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 4,
            ..Default::default()
        },
    );

    let spell = hand_index_for_card(&engine, 0, "hunters_edge");
    engine
        .apply_command(0, &cast_spell(spell, bite_targets(source, recipient)))
        .expect("cast Hunter's Edge");
    engine.state.players[1]
        .battlefield
        .retain(|object_id| *object_id != recipient);
    engine.state.players[1].hand.push(recipient);
    engine
        .state
        .objects
        .get_mut(&recipient)
        .expect("recipient")
        .zone = Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(recipient)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        engine.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
}

#[test]
fn bite_target_that_leaves_and_returns_is_a_new_object() {
    let decks = Some(vec![
        deck_with("forest", &["rabid_bite", "grizzly_bears"]),
        deck_with("forest", &["hill_giant"]),
    ]);
    let mut engine = GameEngine::new(85_104, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let source = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let recipient = relocate_to_battlefield(&mut engine, 1, "hill_giant", false);
    ensure_in_hand(&mut engine, 0, "rabid_bite");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 2,
            ..Default::default()
        },
    );

    let spell = hand_index_for_card(&engine, 0, "rabid_bite");
    engine
        .apply_command(0, &cast_spell(spell, bite_targets(source, recipient)))
        .expect("cast Rabid Bite");
    *engine
        .state
        .zone_change_generation
        .entry(recipient)
        .or_default() += 2;
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.objects[&recipient].damage, 0);
}

#[test]
fn prodigal_sorcerer_lifelink_uses_the_shared_noncombat_damage_pipeline() {
    let mut engine = GameEngine::new(85_105, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let source = inject_creature_on_battlefield(&mut engine, 0, "prodigal_sorcerer");
    engine.state.players[0].life = 10;
    engine.state.continuous_effects.push(ContinuousEffect {
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::Layer6AddKeyword(Keyword::Lifelink),
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });

    engine
        .apply_command(0, &activate_ability(source, 0, target_player(1)))
        .expect("activate Prodigal Sorcerer");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[0].life, 11);
    assert_eq!(engine.state.players[1].life, 19);
}

#[test]
fn prodigal_damage_feeds_general_damage_triggers_only_when_damage_is_dealt() {
    for (seed, prevented) in [(85_107, false), (85_108, true)] {
        let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("new");
        advance_to_main1_from_game_start(&mut engine);
        let source = inject_creature_on_battlefield(&mut engine, 0, "prodigal_sorcerer");
        if prevented {
            engine.state.add_damage_prevention_shield(1, 1);
        }

        engine
            .apply_command(0, &activate_ability(source, 0, target_player(1)))
            .expect("activate Prodigal Sorcerer");
        // Model the source acquiring Thieving Magpie's copiable characteristics while the
        // activated ability is on the stack. The ability still resolves from its Prodigal stack
        // snapshot, while damage triggers inspect the physical source that actually dealt it.
        engine
            .state
            .objects
            .get_mut(&source)
            .expect("source")
            .card_id = "thieving_magpie".into();
        let library_before = engine.state.players[0].library.len();
        resolve_entire_stack_two_player(&mut engine);

        assert_eq!(
            engine.state.players[0].library.len(),
            library_before - usize::from(!prevented),
            "only actual post-prevention damage triggers Thieving Magpie"
        );
    }
}

#[test]
fn rabid_bite_uses_creature_deathtouch_and_lifelink_after_prevention() {
    let decks = Some(vec![
        deck_with("forest", &["rabid_bite", "grizzly_bears"]),
        deck_with("forest", &["colossal_dreadmaw"]),
    ]);
    let mut engine = GameEngine::new(85_106, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let source = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let recipient = relocate_to_battlefield(&mut engine, 1, "colossal_dreadmaw", false);
    engine.state.players[0].life = 10;
    for keyword in [Keyword::Deathtouch, Keyword::Lifelink] {
        engine.state.continuous_effects.push(ContinuousEffect {
            source_id: None,
            affected: AffectedScope::Single(source),
            kind: ContinuousEffectKind::Layer6AddKeyword(keyword),
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: engine.state.command_index,
        });
    }
    engine.state.add_damage_prevention_shield(recipient, 1);
    ensure_in_hand(&mut engine, 0, "rabid_bite");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 2,
            ..Default::default()
        },
    );

    let spell = hand_index_for_card(&engine, 0, "rabid_bite");
    engine
        .apply_command(0, &cast_spell(spell, bite_targets(source, recipient)))
        .expect("cast Rabid Bite");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[0].life, 11);
    assert_eq!(engine.state.objects[&recipient].zone, Zone::Graveyard);
}
