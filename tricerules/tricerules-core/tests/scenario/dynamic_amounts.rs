use crate::helpers::*;

fn quantity_game(card: &str) -> GameEngine {
    let mut engine = GameEngine::new(
        165_100,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[card]), forest_only_deck()]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, card);
    engine
}

fn quantity_cast(engine: &mut GameEngine, card: &str, targets: Vec<TargetRef>) {
    if !engine.state.players[0]
        .hand
        .iter()
        .any(|id| engine.state.objects[id].card_id == card)
    {
        inject_card_into_hand(engine, 0, card);
    }
    grant_pool(engine, 0);
    engine
        .apply_command(
            0,
            &cast_spell(hand_index_for_card(engine, 0, card), targets),
        )
        .unwrap();
    pass_both_players(engine);
}

fn quantity_land(engine: &mut GameEngine, player: usize, subtype: &str) -> u32 {
    // A copied test land exercises derived subtype selection without adding unrelated card data.
    let id = inject_permanent_on_battlefield(engine, player, "forest");
    let definition = tricerules_cards::CardRegistry::global()
        .get("forest")
        .unwrap();
    let mut face = definition.primary_face().clone();
    face.types = vec!["Land".into(), subtype.into()];
    face.supertypes.clear();
    engine.state.objects.get_mut(&id).unwrap().copiable_values =
        Some(tricerules_core::state::CopiableValues {
            source_card_id: "forest".into(),
            source_face_index: 0,
            face,
            room_faces: None,
            display_name: "Test land".into(),
        });
    id
}

fn quantity_trigger_target(engine: &mut GameEngine, target: u32) {
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    targets: target_object(target),
                    ..Default::default()
                })),
            },
        )
        .unwrap();
}

#[test]
fn issue_165_flow_draws_for_islands_then_discards_even_at_zero() {
    for islands in [0, 3] {
        let mut engine = quantity_game("flow_of_knowledge");
        for _ in 0..islands {
            inject_permanent_on_battlefield(&mut engine, 0, "island");
        }
        inject_permanent_on_battlefield(&mut engine, 1, "island");
        let library = engine.state.players[0].library.len();
        let hand = engine.state.players[0].hand.len();
        quantity_cast(&mut engine, "flow_of_knowledge", vec![]);
        assert_eq!(engine.state.players[0].library.len(), library - islands);
        let choices = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates[..2]
            .to_vec();
        engine
            .apply_command(0, &submit_resolution_choice(choices))
            .unwrap();
        assert_eq!(engine.state.players[0].hand.len(), hand + islands - 3);
        assert!(engine.state.pending_resolution.is_none());
    }
}

#[test]
fn issue_165_keepguard_counts_enchantments_at_activation_resolution() {
    let mut engine = quantity_game("slumbering_keepguard");
    quantity_cast(&mut engine, "slumbering_keepguard", vec![]);
    let source = battlefield_object_for_card(&engine, 0, "slumbering_keepguard");
    inject_permanent_on_battlefield(&mut engine, 0, "glorious_anthem");
    grant_pool(&mut engine, 0);
    apply_ability(&mut engine, 0, source, 0, vec![]).unwrap();
    inject_permanent_on_battlefield(&mut engine, 0, "glorious_anthem");
    inject_permanent_on_battlefield(&mut engine, 1, "glorious_anthem");
    pass_both_players(&mut engine);
    assert_eq!(engine.effective_power(source), Some(3));
    quantity_cast(&mut engine, "holy_strength", target_object(source));
    pass_both_players(&mut engine);
    assert!(
        engine.state.pending_resolution.is_some(),
        "controlled enchantment entry triggers scry"
    );
    engine
        .apply_command(0, &submit_resolution_choice(vec![]))
        .unwrap();
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn issue_165_deserts_due_scales_negative_pump_from_controlled_deserts() {
    let mut engine = quantity_game("deserts_due");
    let target = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 8, 8);
    quantity_land(&mut engine, 0, "Desert");
    quantity_land(&mut engine, 0, "Desert");
    quantity_land(&mut engine, 1, "Desert");
    quantity_cast(&mut engine, "deserts_due", target_object(target));
    assert_eq!(engine.effective_power(target), Some(4));
    quantity_land(&mut engine, 0, "Desert");
    assert_eq!(
        engine.effective_power(target),
        Some(4),
        "resolved bonus is fixed"
    );
}

#[test]
fn issue_165_gold_rush_counts_new_treasure_and_respects_optional_target_fizzle() {
    for mode in [0, 1, 2] {
        let mut engine = quantity_game("gold_rush");
        let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
        grant_pool(&mut engine, 0);
        engine
            .apply_command(
                0,
                &cast_spell(
                    hand_index_for_card(&engine, 0, "gold_rush"),
                    if mode == 0 {
                        vec![]
                    } else {
                        target_object(target)
                    },
                ),
            )
            .unwrap();
        if mode == 2 {
            engine.state.players[0]
                .battlefield
                .retain(|id| *id != target);
            engine.state.players[0].hand.push(target);
            engine.state.objects.get_mut(&target).unwrap().zone = tricerules_core::Zone::Hand;
            engine.state.zone_change_generation.insert(target, 1);
        }
        pass_both_players(&mut engine);
        let treasures = engine.state.players[0]
            .battlefield
            .iter()
            .filter(|id| engine.state.objects[id].card_id == "treasure")
            .count();
        assert_eq!(treasures, usize::from(mode != 2));
        if mode == 1 {
            assert_eq!(engine.effective_power(target), Some(4));
            inject_permanent_on_battlefield(&mut engine, 0, "treasure");
            assert_eq!(engine.effective_power(target), Some(4));
        }
    }
}

#[test]
fn issue_165_outcaster_searches_and_continuously_counts_deserts() {
    let mut engine = quantity_game("outcaster_greenblade");
    let desert = quantity_land(&mut engine, 0, "Desert");
    quantity_land(&mut engine, 1, "Desert");
    quantity_cast(&mut engine, "outcaster_greenblade", vec![]);
    let source = battlefield_object_for_card(&engine, 0, "outcaster_greenblade");
    assert_eq!(engine.effective_power(source), Some(2));
    pass_both_players(&mut engine);
    let choice = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates[0];
    engine
        .apply_command(0, &submit_resolution_choice(vec![choice]))
        .unwrap();
    assert!(engine.state.players[0].hand.contains(&choice));
    engine
        .state
        .objects
        .get_mut(&desert)
        .unwrap()
        .copiable_values = None;
    assert_eq!(
        engine.effective_power(source),
        Some(1),
        "losing the subtype removes the bonus"
    );
}

#[test]
fn issue_165_brambleguard_uses_power_when_combat_trigger_resolves() {
    let mut engine = quantity_game("brambleguard_captain");
    quantity_cast(&mut engine, "brambleguard_captain", vec![]);
    let source = battlefield_object_for_card(&engine, 0, "brambleguard_captain");
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    engine.apply_command(0, &primitive_yield()).unwrap();
    quantity_trigger_target(&mut engine, target);
    quantity_cast(&mut engine, "giant_growth", target_object(source));
    pass_both_players(&mut engine);
    assert_eq!(engine.effective_power(target), Some(7));
}

#[test]
fn issue_165_cave_in_uses_derived_caves_and_damages_each_creature() {
    let mut engine = quantity_game("calamitous_cave-in");
    quantity_land(&mut engine, 0, "Cave");
    quantity_land(&mut engine, 0, "Cave");
    quantity_land(&mut engine, 1, "Cave");
    let a = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 4, 4);
    let b = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 4, 4);
    quantity_cast(&mut engine, "calamitous_cave-in", vec![]);
    assert_eq!(engine.state.objects[&a].damage, 2);
    assert_eq!(engine.state.objects[&b].damage, 2);
}

#[test]
fn issue_165_cave_in_freezes_whole_damage_batch_across_prevention_choice() {
    let mut engine = quantity_game("calamitous_cave-in");
    for _ in 0..3 {
        quantity_land(&mut engine, 0, "Cave");
    }
    let protected = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 5, 5);
    let other = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 5, 5);
    let walker = inject_permanent_on_battlefield(&mut engine, 1, "jace_beleren");
    engine
        .state
        .objects
        .get_mut(&walker)
        .unwrap()
        .counters
        .insert(tricerules_cards::CounterKind::Loyalty, 5);
    engine.state.add_damage_prevention_shield(protected, 1);
    engine.state.add_damage_prevention_shield(protected, 1);
    quantity_cast(&mut engine, "calamitous_cave-in", vec![]);
    let choice = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .presentation
        .candidates[0];
    assert_eq!(
        engine.state.objects[&other].damage, 0,
        "the whole batch is parked"
    );
    let index = engine.state.command_index;
    assert!(engine
        .apply_command(1, &submit_resolution_choice(vec![choice]))
        .is_err());
    assert_eq!(engine.state.command_index, index);
    // State changed after computation must not cause a second quantity evaluation on resume.
    quantity_land(&mut engine, 0, "Cave");
    engine
        .apply_command(0, &submit_resolution_choice(vec![choice]))
        .unwrap();
    assert_eq!(engine.state.objects[&protected].damage, 1);
    assert_eq!(engine.state.objects[&other].damage, 3);
    assert_eq!(
        engine.state.objects[&walker].counter_count(tricerules_cards::CounterKind::Loyalty),
        2
    );
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn issue_165_chupacabra_counts_permanent_cards_not_instant_or_tokens() {
    let mut engine = quantity_game("chupacabra_echo");
    inject_graveyard_card(&mut engine, 0, "forest");
    inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    inject_graveyard_card(&mut engine, 0, "lightning_bolt");
    inject_graveyard_card(&mut engine, 1, "forest");
    let target = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 5, 5);
    quantity_cast(&mut engine, "chupacabra_echo", vec![]);
    quantity_trigger_target(&mut engine, target);
    pass_both_players(&mut engine);
    assert_eq!(engine.effective_power(target), Some(3));
}

#[test]
fn dwarven_priest_counts_controlled_creatures_when_its_trigger_resolves() {
    let decks = Some(vec![
        deck_with("plains", &["dwarven_priest"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(51_001, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "dwarven_priest");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 3,
            ..Default::default()
        },
    );

    let priest = hand_index_for_card(&engine, 0, "dwarven_priest");
    engine
        .apply_command(0, &cast_spell(priest, vec![]))
        .expect("cast Dwarven Priest");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.stack.len(), 1, "ETB trigger is waiting");

    inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.players[0].life, 23,
        "the Priest and both creatures its controller owns are counted at resolution"
    );
}

#[test]
fn aerial_assault_counts_derived_flying_after_a_legal_destroy_attempt() {
    let decks = Some(vec![
        deck_with("plains", &["aerial_assault", "flight"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(84_001, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);

    let enchanted = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    inject_creature_on_battlefield(&mut engine, 0, "wind_drake");
    ensure_in_hand(&mut engine, 0, "flight");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let flight = hand_index_for_card(&engine, 0, "flight");
    engine
        .apply_command(
            0,
            &cast_spell(
                flight,
                vec![TargetRef {
                    object_id: enchanted,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
                }],
            ),
        )
        .expect("cast Flight");
    pass_both_players(&mut engine);

    let indestructible = inject_creature_on_battlefield(&mut engine, 1, "darksteel_myr");
    engine
        .state
        .objects
        .get_mut(&indestructible)
        .expect("Darksteel Myr")
        .tapped = true;
    ensure_in_hand(&mut engine, 0, "aerial_assault");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let assault = hand_index_for_card(&engine, 0, "aerial_assault");
    engine
        .apply_command(
            0,
            &cast_spell(
                assault,
                vec![TargetRef {
                    object_id: indestructible,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
                }],
            ),
        )
        .expect("cast Aerial Assault");
    pass_both_players(&mut engine);

    assert_eq!(engine.state.players[0].life, 22);
    assert_eq!(
        engine.state.objects[&indestructible].zone,
        tricerules_core::Zone::Battlefield,
        "a legal indestructible target survives while the later life-gain instruction still happens"
    );
}

#[test]
fn aerial_assault_fizzles_entirely_when_its_only_target_becomes_illegal() {
    let decks = Some(vec![
        deck_with("plains", &["aerial_assault"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(84_005, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_creature_on_battlefield(&mut engine, 0, "wind_drake");
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine
        .state
        .objects
        .get_mut(&target)
        .expect("target")
        .tapped = true;

    ensure_in_hand(&mut engine, 0, "aerial_assault");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );
    let assault = hand_index_for_card(&engine, 0, "aerial_assault");
    engine
        .apply_command(
            0,
            &cast_spell(
                assault,
                vec![TargetRef {
                    object_id: target,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
                }],
            ),
        )
        .expect("cast Aerial Assault");

    engine
        .state
        .objects
        .get_mut(&target)
        .expect("target")
        .tapped = false;
    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.players[0].life, 20,
        "a fizzled spell gains no life"
    );
    assert_eq!(
        engine.state.objects[&target].zone,
        tricerules_core::Zone::Battlefield
    );
}

#[test]
fn growth_cycle_counts_only_its_controllers_graveyard_and_locks_the_bonus() {
    let decks = Some(vec![
        deck_with("forest", &["growth_cycle"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(84_002, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_graveyard_card(&mut engine, 0, "growth_cycle");
    inject_graveyard_card(&mut engine, 0, "growth_cycle");
    inject_graveyard_card(&mut engine, 1, "growth_cycle");
    let target = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");

    ensure_in_hand(&mut engine, 0, "growth_cycle");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let growth = hand_index_for_card(&engine, 0, "growth_cycle");
    engine
        .apply_command(
            0,
            &cast_spell(
                growth,
                vec![TargetRef {
                    object_id: target,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
                }],
            ),
        )
        .expect("cast Growth Cycle");
    pass_both_players(&mut engine);

    assert_eq!(engine.effective_power(target), Some(9));
    assert_eq!(engine.effective_toughness(target), Some(9));
    assert_eq!(
        count_card_id_in_graveyard(&engine, 0, "growth_cycle"),
        3,
        "the resolving spell moves to the graveyard only after its locked-in bonus is determined"
    );
}

#[test]
fn lavakin_brawler_counts_controlled_elementals_when_its_attack_trigger_resolves() {
    let decks = Some(vec![
        deck_with("mountain", &["lavakin_brawler"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(84_003, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let brawler = relocate_to_battlefield(&mut engine, 0, "lavakin_brawler", false);
    inject_creature_on_battlefield(&mut engine, 0, "fire_elemental");
    inject_creature_on_battlefield(&mut engine, 1, "air_elemental");

    engine
        .apply_command(0, &primitive_yield())
        .expect("move to beginning of combat");
    engine
        .apply_command(0, &pass())
        .expect("active player pass");
    engine.apply_command(1, &pass()).expect("defender pass");
    engine
        .apply_command(0, &declare_attackers(vec![brawler]))
        .expect("attack with Lavakin Brawler");
    assert_eq!(engine.state.stack.len(), 1, "attack trigger is waiting");

    inject_creature_on_battlefield(&mut engine, 0, "air_elemental");
    pass_both_players(&mut engine);

    assert_eq!(engine.effective_power(brawler), Some(5));
    assert_eq!(engine.effective_toughness(brawler), Some(4));
}

#[test]
fn undead_servant_dying_before_its_etb_trigger_resolves_counts_itself() {
    let decks = Some(vec![
        deck_with("swamp", &["undead_servant", "lightning_bolt"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(84_004, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_graveyard_card(&mut engine, 0, "undead_servant");
    inject_graveyard_card(&mut engine, 0, "undead_servant");
    inject_graveyard_card(&mut engine, 1, "undead_servant");

    ensure_in_hand(&mut engine, 0, "undead_servant");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let servant = hand_index_for_card(&engine, 0, "undead_servant");
    engine
        .apply_command(0, &cast_spell(servant, vec![]))
        .expect("cast Undead Servant");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.stack.len(), 1, "ETB trigger is waiting");
    let servant_oid = battlefield_object_for_card(&engine, 0, "undead_servant");

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
                    object_id: servant_oid,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
                }],
            ),
        )
        .expect("cast Lightning Bolt in response");
    pass_both_players(&mut engine);
    assert_eq!(
        count_card_id_in_graveyard(&engine, 0, "undead_servant"),
        3,
        "the source card is in its owner's graveyard before the ETB trigger resolves"
    );

    pass_both_players(&mut engine);
    let zombies = engine.state.players[0]
        .battlefield
        .iter()
        .filter(|oid| {
            engine
                .state
                .objects
                .get(oid)
                .is_some_and(|o| o.card_id == "zombie_b_2_2")
        })
        .count();
    assert_eq!(zombies, 3);
}

#[test]
fn undead_servant_zero_count_resolves_without_creating_or_parking() {
    let decks = Some(vec![
        deck_with("swamp", &["undead_servant"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(84_006, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "undead_servant");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let servant = hand_index_for_card(&engine, 0, "undead_servant");
    engine
        .apply_command(0, &cast_spell(servant, vec![]))
        .expect("cast Undead Servant");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.stack.len(), 1, "ETB trigger is waiting");
    pass_both_players(&mut engine);

    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_resolution.is_none());
    assert!(engine.state.players[0].battlefield.iter().all(|oid| {
        engine
            .state
            .objects
            .get(oid)
            .is_none_or(|o| o.card_id != "zombie_b_2_2")
    }));
}
