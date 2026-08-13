use crate::helpers::*;

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
