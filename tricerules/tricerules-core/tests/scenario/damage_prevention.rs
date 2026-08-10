use crate::helpers::*;
use tricerules_cards::primitives::CounterKind;

/// CR 615.12: Stomp makes the damage unpreventable, so an existing prevention shield neither
/// reduces the damage nor loses any of its remaining capacity.
#[test]
fn stomp_bypasses_prevention_shield_without_consuming_it() {
    let decks = Some(vec![
        deck_with("mountain", &["bonecrusher_giant_stomp"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(4801, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);

    engine.state.add_damage_prevention_shield(1, 3);
    ensure_in_hand(&mut engine, 0, "bonecrusher_giant_stomp");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&engine, 0, "bonecrusher_giant_stomp");
    engine
        .apply_command(0, &cast_spell_face(slot, target_player(1), 1))
        .expect("cast Stomp");
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.players[1].life, 18,
        "Stomp deals its full 2 damage"
    );
    assert_eq!(
        engine.state.remaining_damage_prevention(1),
        3,
        "unpreventable damage does not consume the shield"
    );
}

#[test]
fn anti_venom_prevents_direct_damage_and_gets_attempted_damage_counters() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &["anti-venom,_horrifying_healer", "lightning_bolt"],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(4802, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);

    ensure_in_hand(&mut engine, 0, "anti-venom,_horrifying_healer");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 5,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "anti-venom,_horrifying_healer");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Anti-Venom");
    pass_both_players(&mut engine);
    let anti_venom = battlefield_object_for_card(&engine, 0, "anti-venom,_horrifying_healer");

    ensure_in_hand(&mut engine, 0, "lightning_bolt");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt = hand_index_for_card(&engine, 0, "lightning_bolt");
    engine
        .apply_command(
            0,
            &cast_spell(
                bolt,
                vec![TargetRef {
                    object_id: anti_venom,
                    damage_amount: 0,
                }],
            ),
        )
        .expect("cast Bolt");
    pass_both_players(&mut engine);

    let object = &engine.state.objects[&anti_venom];
    assert_eq!(object.damage, 0);
    assert_eq!(object.counter_count(CounterKind::PlusOnePlusOne), 3);
}

fn anti_venom_with_shield_awaiting_five_damage(seed: u64) -> (GameEngine, u32, u32, u32) {
    let decks = Some(vec![
        deck_with("plains", &["anti-venom,_horrifying_healer", "blaze"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "anti-venom,_horrifying_healer");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 5,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "anti-venom,_horrifying_healer");
    engine
        .apply_command(0, &cast_spell(slot, vec![]))
        .expect("cast Anti-Venom");
    pass_both_players(&mut engine);
    let anti_venom = battlefield_object_for_card(&engine, 0, "anti-venom,_horrifying_healer");
    engine.state.add_damage_prevention_shield(anti_venom, 3);
    let anti_effect = engine
        .state
        .damage_prevention_effects
        .iter()
        .find(|effect| effect.source_label == "Anti-Venom, Horrifying Healer")
        .expect("Anti-Venom prevention")
        .id;
    let shield_effect = engine
        .state
        .damage_prevention_effects
        .iter()
        .find(|effect| effect.source_label == "Prevention shield")
        .expect("shield prevention")
        .id;

    ensure_in_hand(&mut engine, 0, "blaze");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 5,
            ..Default::default()
        },
    );
    let blaze = hand_index_for_card(&engine, 0, "blaze");
    engine
        .apply_command(
            0,
            &cast_spell_x(
                blaze,
                vec![TargetRef {
                    object_id: anti_venom,
                    damage_amount: 0,
                }],
                5,
            ),
        )
        .expect("cast Blaze");
    pass_both_players(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("CR 616 prompt");
    assert_eq!(pending.choice_kind, ChoiceKind::ReplacementEffect);
    let anti_application = pending.candidates[pending
        .scratch
        .iter()
        .position(|effect_id| *effect_id == anti_effect)
        .expect("Anti-Venom application")];
    let shield_application = pending.candidates[pending
        .scratch
        .iter()
        .position(|effect_id| *effect_id == shield_effect)
        .expect("shield application")];
    (engine, anti_venom, anti_application, shield_application)
}

#[test]
fn anti_venom_then_shield_keeps_the_shield_and_gets_full_counters() {
    let (mut engine, anti_venom, anti_effect, _) =
        anti_venom_with_shield_awaiting_five_damage(4803);
    engine
        .apply_command(0, &submit_resolution_choice(vec![anti_effect]))
        .expect("choose Anti-Venom first");

    let object = &engine.state.objects[&anti_venom];
    assert_eq!(object.damage, 0);
    assert_eq!(object.counter_count(CounterKind::PlusOnePlusOne), 5);
    assert_eq!(engine.state.remaining_damage_prevention(anti_venom), 3);
}

#[test]
fn shield_then_anti_venom_consumes_shield_and_counts_only_remaining_damage() {
    let (mut engine, anti_venom, _, shield_effect) =
        anti_venom_with_shield_awaiting_five_damage(4804);
    engine
        .apply_command(0, &submit_resolution_choice(vec![shield_effect]))
        .expect("choose shield first");

    let object = &engine.state.objects[&anti_venom];
    assert_eq!(object.damage, 0);
    assert_eq!(object.counter_count(CounterKind::PlusOnePlusOne), 2);
    assert_eq!(engine.state.remaining_damage_prevention(anti_venom), 0);
}

#[test]
fn unpreventable_ordered_damage_applies_every_effect_without_consuming_the_shield() {
    let (mut engine, anti_venom, anti_application, _) =
        anti_venom_with_shield_awaiting_five_damage(4813);
    engine
        .state
        .damage_prevention_prohibitions
        .push(tricerules_core::state::DamagePreventionProhibition { source_id: None });

    engine
        .apply_command(0, &submit_resolution_choice(vec![anti_application]))
        .expect("apply every prevention effect under the prohibition");

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&anti_venom].damage, 5);
    assert_eq!(
        engine.state.objects[&anti_venom].counter_count(CounterKind::PlusOnePlusOne),
        5
    );
    assert_eq!(engine.state.remaining_damage_prevention(anti_venom), 3);
}

#[test]
fn three_applications_prompt_again_for_the_next_prevention_effect() {
    let (mut engine, anti_venom, _, first_shield_application) =
        anti_venom_with_shield_awaiting_five_damage(4811);
    let second_shield_effect = engine.state.next_damage_prevention_effect_id;
    engine.state.add_damage_prevention_shield(anti_venom, 1);

    engine
        .apply_command(0, &submit_resolution_choice(vec![first_shield_application]))
        .expect("choose the three-point shield first");

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("the remaining two applications need another CR 616 choice");
    assert_eq!(pending.candidates.len(), 2);
    let anti_application = pending.candidates[pending
        .scratch
        .iter()
        .position(|effect_id| *effect_id != second_shield_effect)
        .expect("Anti-Venom remains a candidate")];
    engine
        .apply_command(0, &submit_resolution_choice(vec![anti_application]))
        .expect("choose Anti-Venom next");

    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(engine.state.objects[&anti_venom].damage, 0);
    assert_eq!(
        engine.state.objects[&anti_venom].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert_eq!(engine.state.remaining_damage_prevention(anti_venom), 1);
}

#[test]
fn invalid_prevention_order_answers_preserve_the_parked_damage() {
    let (mut engine, anti_venom, anti_effect, _) =
        anti_venom_with_shield_awaiting_five_damage(4805);

    assert!(engine
        .apply_command(1, &submit_resolution_choice(vec![anti_effect]))
        .is_err());
    assert!(engine.state.pending_resolution.is_some());
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![u32::MAX]))
        .is_err());
    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(engine.state.objects[&anti_venom].damage, 0);

    engine
        .apply_command(0, &submit_resolution_choice(vec![anti_effect]))
        .expect("valid retry");
    assert!(engine.state.pending_resolution.is_none());
    assert_eq!(
        engine.state.objects[&anti_venom].counter_count(CounterKind::PlusOnePlusOne),
        5
    );
}

#[test]
fn lethal_damage_runs_state_based_actions_after_the_ordering_choice() {
    let decks = Some(vec![
        deck_with("mountain", &["blaze"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(4812, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let bears = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    engine.state.add_damage_prevention_shield(bears, 1);
    engine.state.add_damage_prevention_shield(bears, 1);
    ensure_in_hand(&mut engine, 0, "blaze");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 5,
            ..Default::default()
        },
    );
    let blaze = hand_index_for_card(&engine, 0, "blaze");
    engine
        .apply_command(
            0,
            &cast_spell_x(
                blaze,
                vec![TargetRef {
                    object_id: bears,
                    damage_amount: 0,
                }],
                5,
            ),
        )
        .expect("cast Blaze");
    pass_both_players(&mut engine);
    let application = engine.state.pending_resolution.as_ref().unwrap().candidates[0];
    engine
        .apply_command(1, &submit_resolution_choice(vec![application]))
        .expect("choose either shield first");

    assert_eq!(
        engine.state.objects[&bears].zone,
        tricerules_core::Zone::Graveyard
    );
}

#[test]
fn combat_prevention_choice_parks_and_then_commits_the_entire_damage_batch() {
    let decks = Some(vec![
        deck_with("plains", &["anti-venom,_horrifying_healer"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(4806, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "anti-venom,_horrifying_healer");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 5,
            ..Default::default()
        },
    );
    let anti_slot = hand_index_for_card(&engine, 0, "anti-venom,_horrifying_healer");
    engine
        .apply_command(0, &cast_spell(anti_slot, vec![]))
        .expect("cast Anti-Venom");
    pass_both_players(&mut engine);
    let anti_venom = battlefield_object_for_card(&engine, 0, "anti-venom,_horrifying_healer");
    engine
        .state
        .objects
        .get_mut(&anti_venom)
        .unwrap()
        .summoning_sick = false;
    let blocker = relocate_to_battlefield(&mut engine, 1, "grizzly_bears", false);
    engine.state.add_damage_prevention_shield(anti_venom, 3);
    let anti_effect = engine
        .state
        .damage_prevention_effects
        .iter()
        .find(|effect| effect.source_label == "Anti-Venom, Horrifying Healer")
        .unwrap()
        .id;

    engine
        .apply_command(0, &primitive_yield())
        .expect("main phase ends");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![anti_venom]))
        .expect("attack");
    pass_both_players(&mut engine);
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: anti_venom,
                blocker_id: blocker,
            }]),
        )
        .expect("block");
    pass_both_players(&mut engine);

    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("CR 616 prompt");
    let anti_index = pending
        .scratch
        .iter()
        .position(|effect_id| *effect_id == anti_effect)
        .expect("Anti-Venom application");
    let anti_application = pending.candidates[anti_index];
    engine
        .apply_command(0, &submit_resolution_choice(vec![anti_application]))
        .expect("choose Anti-Venom first");

    assert_eq!(engine.state.objects[&anti_venom].damage, 0);
    assert_eq!(
        engine.state.objects[&anti_venom].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert_eq!(engine.state.remaining_damage_prevention(anti_venom), 3);
    assert_eq!(
        engine.state.objects[&blocker].zone,
        tricerules_core::Zone::Graveyard
    );
}

#[test]
fn stomp_damage_still_gives_anti_venom_attempted_damage_counters() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &["anti-venom,_horrifying_healer", "bonecrusher_giant_stomp"],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(4807, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "anti-venom,_horrifying_healer");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 5,
            ..Default::default()
        },
    );
    let anti_slot = hand_index_for_card(&engine, 0, "anti-venom,_horrifying_healer");
    engine
        .apply_command(0, &cast_spell(anti_slot, vec![]))
        .expect("cast Anti-Venom");
    pass_both_players(&mut engine);
    let anti_venom = battlefield_object_for_card(&engine, 0, "anti-venom,_horrifying_healer");

    ensure_in_hand(&mut engine, 0, "bonecrusher_giant_stomp");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let stomp = hand_index_for_card(&engine, 0, "bonecrusher_giant_stomp");
    engine
        .apply_command(
            0,
            &cast_spell_face(
                stomp,
                vec![TargetRef {
                    object_id: anti_venom,
                    damage_amount: 0,
                }],
                1,
            ),
        )
        .expect("cast Stomp");
    pass_both_players(&mut engine);

    let object = &engine.state.objects[&anti_venom];
    assert_eq!(object.damage, 2);
    assert_eq!(object.counter_count(CounterKind::PlusOnePlusOne), 2);
}

#[test]
fn first_strike_damage_grows_anti_venom_before_normal_combat_damage() {
    let decks = Some(vec![
        deck_with("plains", &["anti-venom,_horrifying_healer"]),
        deck_with("forest", &["youthful_knight"]),
    ]);
    let mut engine = GameEngine::new(4808, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "anti-venom,_horrifying_healer");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 5,
            ..Default::default()
        },
    );
    let anti_slot = hand_index_for_card(&engine, 0, "anti-venom,_horrifying_healer");
    engine
        .apply_command(0, &cast_spell(anti_slot, vec![]))
        .expect("cast Anti-Venom");
    pass_both_players(&mut engine);
    let anti_venom = battlefield_object_for_card(&engine, 0, "anti-venom,_horrifying_healer");
    engine
        .state
        .objects
        .get_mut(&anti_venom)
        .unwrap()
        .summoning_sick = false;
    let knight = relocate_to_battlefield(&mut engine, 1, "youthful_knight", false);

    engine.apply_command(0, &primitive_yield()).unwrap();
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![anti_venom]))
        .unwrap();
    pass_both_players(&mut engine);
    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: anti_venom,
                blocker_id: knight,
            }]),
        )
        .unwrap();
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.turn_step,
        tricerules_core::TurnStep::FirstStrikeDamage
    );
    assert_eq!(
        engine.state.objects[&anti_venom].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert_eq!(engine.state.objects[&anti_venom].damage, 0);
    assert_eq!(
        engine.state.objects[&knight].zone,
        tricerules_core::Zone::Battlefield
    );

    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&knight].zone,
        tricerules_core::Zone::Graveyard
    );
}

#[test]
fn cleanup_clears_stomps_prevention_prohibition() {
    let decks = Some(vec![
        deck_with("mountain", &["bonecrusher_giant_stomp"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(4809, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "bonecrusher_giant_stomp");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let stomp = hand_index_for_card(&engine, 0, "bonecrusher_giant_stomp");
    engine
        .apply_command(0, &cast_spell_face(stomp, target_player(1), 1))
        .expect("cast Stomp");
    pass_both_players(&mut engine);
    assert!(!engine.state.damage_prevention_prohibitions.is_empty());

    for _ in 0..5 {
        engine
            .apply_command(0, &primitive_yield())
            .expect("advance toward cleanup");
    }
    assert!(engine.state.damage_prevention_prohibitions.is_empty());
}

#[test]
fn stomp_bypasses_fog_for_later_combat_damage() {
    let decks = Some(vec![
        deck_with("mountain", &["bonecrusher_giant_stomp", "grizzly_bears"]),
        deck_with("forest", &["fog"]),
    ]);
    let mut engine = GameEngine::new(4810, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "bonecrusher_giant_stomp");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    let stomp = hand_index_for_card(&engine, 0, "bonecrusher_giant_stomp");
    engine
        .apply_command(0, &cast_spell_face(stomp, target_player(1), 1))
        .unwrap();
    pass_both_players(&mut engine);

    ensure_in_hand(&mut engine, 1, "fog");
    give_mana(
        &mut engine,
        1,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    engine.apply_command(0, &pass()).unwrap();
    let fog = hand_index_for_card(&engine, 1, "fog");
    engine.apply_command(1, &cast_spell(fog, vec![])).unwrap();
    pass_both_players(&mut engine);

    let attacker = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    engine.apply_command(0, &primitive_yield()).unwrap();
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .unwrap();
    pass_both_players(&mut engine);
    engine.apply_command(1, &declare_blockers(vec![])).unwrap();
    pass_both_players(&mut engine);

    assert_eq!(engine.state.players[1].life, 16);
}
