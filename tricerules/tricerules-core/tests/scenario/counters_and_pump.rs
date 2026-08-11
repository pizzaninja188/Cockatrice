use crate::helpers::*;

#[test]
fn non_active_player_with_priority_pays_mana_for_counterspell() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "island".into(),
            "counterspell".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
    ]);
    let mut e = GameEngine::new(144, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("p0 play mountain");

    for _ in 0..2 {
        let island_idx = hand_index_for_card(&e, 1, "island");
        let island_oid = e.state.players[1].hand.remove(island_idx);
        e.state.players[1].battlefield.push(island_oid);
        e.state
            .objects
            .get_mut(&island_oid)
            .expect("seeded island")
            .zone = tricerules_core::Zone::Battlefield;
    }

    let p1_island_a = battlefield_object_for_card(&e, 1, "island");
    assert!(!e.state.objects.get(&p1_island_a).expect("p1 island").tapped);

    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, target_player(1)))
        .expect("p0 cast bolt");
    let bolt_oid = e.state.stack.last().expect("bolt on stack").id;
    e.apply_command(0, &pass())
        .expect("p0 pass to give p1 priority");

    // Manually tap an island (simulates client-side land tap for mana).
    e.state
        .objects
        .get_mut(&p1_island_a)
        .expect("p1 island")
        .tapped = true;
    give_mana(
        &mut e,
        1,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let counter_idx = hand_index_for_card(&e, 1, "counterspell");
    e.apply_command(
        1,
        &cast_spell(
            counter_idx,
            vec![TargetRef {
                object_id: bolt_oid,
                damage_amount: 0,
            }],
        ),
    )
    .expect("NAP with priority should cast counterspell");
    assert!(
        e.state.objects.get(&p1_island_a).expect("p1 island").tapped,
        "an island should tap to help pay UU"
    );
    assert_eq!(e.state.stack.len(), 2, "bolt and counterspell on stack");

    e.apply_command(1, &pass())
        .expect("p1 pass after casting counter");
    e.apply_command(0, &pass())
        .expect("p0 pass resolves counterspell");
    assert!(e.state.stack.is_empty(), "stack empty after counter");
    assert_eq!(e.state.active_player_id(), 0, "AP is P0 in this test");
    assert_eq!(
        e.state.priority_player_id(),
        0,
        "with empty stack, priority should return to active player (CR 117.3c)"
    );
    assert_eq!(
        e.state.passes_since_stack_change, 0,
        "pass counter should reset after stack closed"
    );
}

#[test]
fn giant_growth_pump_expires_after_active_turn_ends() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "giant_growth".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(904, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let forest_idx = hand_index_for_card(&e, 0, "forest");
    let forest_oid = e.state.players[0].hand.remove(forest_idx);
    e.state.players[0].battlefield.push(forest_oid);
    e.state.objects.get_mut(&forest_oid).expect("forest").zone = tricerules_core::Zone::Battlefield;

    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let growth_idx = hand_index_for_card(&e, 0, "giant_growth");
    e.apply_command(
        0,
        &cast_spell(
            growth_idx,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast growth");
    pass_both_players(&mut e);

    assert_eq!(
        e.effective_power(bear),
        Some(5),
        "pumped bear should have 5 effective power"
    );
    assert_eq!(
        e.effective_toughness(bear),
        Some(5),
        "pumped bear should have 5 effective toughness"
    );

    end_active_turn(&mut e, 0);

    assert_eq!(
        e.effective_power(bear),
        Some(2),
        "Giant Growth should expire at end of turn"
    );
    assert_eq!(e.effective_toughness(bear), Some(2));
}

/// CR 113/606: Fiery Hellhound's firebreathing (`{R}: this creature gets +1/+0`) is the first
/// card to drive a `PumpTarget` with `EffectSubject::Source` from an *activated* ability — the effect
/// auto-binds to the source (not a chosen target, CR 115) and is repeatable. Each activation goes
/// on the stack (non-mana ability), resolves to a layer-7c `UntilEndOfTurn` P/T bump, and the
/// bumps stack; they all drain at cleanup (CR 514.2 / 611.2g — independent of the source).
#[test]
fn fiery_hellhound_self_firebreathing_pumps_and_expires() {
    let decks = Some(vec![
        {
            let mut d = vec!["fiery_hellhound".to_string()];
            d.extend(std::iter::repeat_n("mountain".to_string(), 6));
            d
        },
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(7311, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let hound = put_creature_on_battlefield(&mut e, 0, "fiery_hellhound");
    assert_eq!(e.effective_power(hound), Some(2), "printed 2/2 power");
    assert_eq!(
        e.effective_toughness(hound),
        Some(2),
        "printed 2/2 toughness"
    );

    // Fund two activations of the {R} firebreathing cost.
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 2,
            ..Default::default()
        },
    );

    // First activation: cost paid on activation (pool 2 -> 1), ability on the stack; resolves to +1/+0.
    e.apply_command(0, &activate_ability(hound, 0, vec![]))
        .expect("activate firebreathing #1");
    assert_eq!(
        e.state.players[0].mana_pool.red, 1,
        "first {{R}} paid on activation"
    );
    pass_both_players(&mut e);
    assert_eq!(e.effective_power(hound), Some(3), "+1/+0 after first pump");
    assert_eq!(e.effective_toughness(hound), Some(2), "toughness unchanged");

    // Second activation stacks another +1/+0.
    e.apply_command(0, &activate_ability(hound, 0, vec![]))
        .expect("activate firebreathing #2");
    assert_eq!(
        e.state.players[0].mana_pool.red, 0,
        "second {{R}} paid on activation"
    );
    pass_both_players(&mut e);
    assert_eq!(
        e.effective_power(hound),
        Some(4),
        "+2/+0 after two pumps stack"
    );
    assert_eq!(
        e.effective_toughness(hound),
        Some(2),
        "toughness still unchanged"
    );

    // CR 514.2: both until-end-of-turn pumps drain at cleanup; back to printed 2/2.
    end_active_turn(&mut e, 0);
    assert_eq!(
        e.effective_power(hound),
        Some(2),
        "firebreathing expires at end of turn"
    );
    assert_eq!(
        e.effective_toughness(hound),
        Some(2),
        "toughness back to printed"
    );
}

/// A source-bound effect is untargeted: the engine must reject a client that attaches a target
/// instead of silently accepting targeting vocabulary for the source permanent.
#[test]
fn fiery_hellhound_source_pump_rejects_supplied_target() {
    let decks = Some(vec![
        {
            let mut d = vec!["fiery_hellhound".to_string()];
            d.extend(std::iter::repeat_n("mountain".to_string(), 6));
            d
        },
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(7312, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let hound = put_creature_on_battlefield(&mut e, 0, "fiery_hellhound");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );

    let err = e
        .apply_command(
            0,
            &activate_ability(
                hound,
                0,
                vec![TargetRef {
                    object_id: hound,
                    damage_amount: 0,
                }],
            ),
        )
        .expect_err("source-bound pump must reject a supplied target");
    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));
    assert!(
        e.state.stack.is_empty(),
        "rejected activation stays off stack"
    );
    assert_eq!(
        e.state.players[0].mana_pool.red, 1,
        "rejected activation pays no mana"
    );
}

/// Two Giant Growths on the same creature stack: effective P/T = base + both deltas.
#[test]
fn two_giant_growths_stack_correctly() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "forest".into(),
            "giant_growth".into(),
            "giant_growth".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(9050, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    // Tap two forests and cast both Giant Growths.
    for _ in 0..2 {
        let forest_idx = hand_index_for_card(&e, 0, "forest");
        let foid = e.state.players[0].hand.remove(forest_idx);
        e.state.players[0].battlefield.push(foid);
        e.state.objects.get_mut(&foid).expect("forest").zone = tricerules_core::Zone::Battlefield;
        give_mana(
            &mut e,
            0,
            ManaGift {
                g: 1,
                ..Default::default()
            },
        );
        let growth_idx = hand_index_for_card(&e, 0, "giant_growth");
        e.apply_command(
            0,
            &cast_spell(
                growth_idx,
                vec![TargetRef {
                    object_id: bear,
                    damage_amount: 0,
                }],
            ),
        )
        .expect("cast growth");
        pass_both_players(&mut e);
    }

    assert_eq!(
        e.effective_power(bear),
        Some(8),
        "two Giant Growths should give +6/+6 total"
    );
    assert_eq!(e.effective_toughness(bear), Some(8));
    assert_eq!(
        e.state.continuous_effects.len(),
        2,
        "two active ContinuousEffects expected"
    );

    end_active_turn(&mut e, 0);

    assert_eq!(e.effective_power(bear), Some(2), "pump expires at cleanup");
    assert_eq!(e.effective_toughness(bear), Some(2));
    assert!(
        e.state.continuous_effects.is_empty(),
        "continuous_effects must be empty after cleanup"
    );
}

/// CR 122 + CR 613.4 layer 7d: a +1/+1 counter from Battlegrowth raises a creature's P/T, and
/// unlike a Giant Growth pump it persists past the end of the turn (counters are not
/// until-end-of-turn continuous effects).
#[test]
fn battlegrowth_counter_raises_pt_and_persists() {
    use tricerules_cards::CounterKind;
    let decks = Some(vec![
        vec![
            "battlegrowth".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(1221, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "battlegrowth");
    e.apply_command(
        0,
        &cast_spell(
            idx,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast battlegrowth");
    pass_both_players(&mut e);

    assert_eq!(e.effective_power(bear), Some(3), "2/2 + one +1/+1 counter");
    assert_eq!(e.effective_toughness(bear), Some(3));
    assert_eq!(
        e.state
            .objects
            .get(&bear)
            .unwrap()
            .counter_count(CounterKind::PlusOnePlusOne),
        1
    );

    end_active_turn(&mut e, 0);

    assert_eq!(
        e.effective_power(bear),
        Some(3),
        "counter persists past end of turn (not a continuous effect)"
    );
    assert_eq!(e.effective_toughness(bear), Some(3));
    assert_eq!(
        e.state
            .objects
            .get(&bear)
            .unwrap()
            .counter_count(CounterKind::PlusOnePlusOne),
        1,
        "counter survives cleanup"
    );
}

/// A creature with counters exposes a human-readable counter annotation in the zone view so the
/// client can overlay it on the card (e.g. "1 +1/+1 counter(s)"); a counter-free creature reports
/// an empty annotation.
#[test]
fn zone_view_reports_counter_annotation() {
    let decks = Some(vec![
        vec![
            "battlegrowth".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(1313, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    // A second creature stays counter-free so we can assert the empty-annotation case.
    let plain = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let bg = hand_index_for_card(&e, 0, "battlegrowth");
    e.apply_command(
        0,
        &cast_spell(
            bg,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast battlegrowth");
    let first = e.state.priority_player_id();
    e.apply_command(first, &pass()).expect("first pass");
    let second = if first == 0 { 1 } else { 0 };
    let b = e
        .apply_command(second, &pass())
        .expect("second pass resolves");

    let zone_view = b
        .events
        .iter()
        .rev()
        .find_map(|ev| match &ev.ev {
            Some(Ev::ZoneView(zv)) => Some(zv.clone()),
            _ => None,
        })
        .expect("zone view in batch");
    let p0 = zone_view
        .per_player
        .iter()
        .find(|p| p.player_id == 0)
        .expect("p0 view");
    let bear_pos = p0
        .battlefield_objects
        .iter()
        .position(|object| object.object_id == bear)
        .expect("bear in view");
    assert_eq!(
        p0.battlefield_objects[bear_pos].counters_annotation, "1 +1/+1 counter(s)",
        "bear with one +1/+1 counter is annotated"
    );
    let plain_pos = p0
        .battlefield_objects
        .iter()
        .position(|object| object.object_id == plain)
        .expect("plain creature in view");
    assert!(
        p0.battlefield_objects[plain_pos]
            .counters_annotation
            .is_empty(),
        "counter-free permanent has no annotation"
    );
}

/// CR 122.3: when a creature has both +1/+1 and -1/-1 counters, equal numbers annihilate as a
/// state-based action. Battlegrowth (+1/+1) then Instill Infection (-1/-1) net back to base P/T.
#[test]
fn plus_and_minus_counters_annihilate() {
    let decks = Some(vec![
        vec![
            "battlegrowth".into(),
            "instill_infection".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(1222, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    // Battlegrowth: +1/+1 counter -> 3/3.
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let bg = hand_index_for_card(&e, 0, "battlegrowth");
    e.apply_command(
        0,
        &cast_spell(
            bg,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast battlegrowth");
    pass_both_players(&mut e);
    assert_eq!(e.effective_toughness(bear), Some(3));

    // Instill Infection also draws a card; give the (opening-hand-emptied) library something.
    inject_library_card(&mut e, 0, "forest");
    // Instill Infection: -1/-1 counter; the SBA annihilates the +1/+1/-1/-1 pair -> back to 2/2.
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let ii = hand_index_for_card(&e, 0, "instill_infection");
    e.apply_command(
        0,
        &cast_spell(
            ii,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast instill infection");
    pass_both_players(&mut e);

    assert_eq!(
        e.effective_power(bear),
        Some(2),
        "counters annihilated to net 0"
    );
    assert_eq!(e.effective_toughness(bear), Some(2));
    assert!(
        e.state.objects.get(&bear).unwrap().counters.is_empty(),
        "no counters remain after annihilation"
    );
}

/// CR 704.5f via CR 122: a -1/-1 counter dropping a 1/1's toughness to 0 kills it as an SBA.
#[test]
fn minus_counter_to_zero_toughness_kills_via_sba() {
    let decks = Some(vec![
        vec![
            "prodigal_sorcerer".into(),
            "instill_infection".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(1223, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Prodigal Sorcerer is a 1/1.
    let sorc = put_creature_on_battlefield(&mut e, 0, "prodigal_sorcerer");
    assert_eq!(e.effective_toughness(sorc), Some(1));

    // Instill Infection also draws a card; give the (opening-hand-emptied) library something.
    inject_library_card(&mut e, 0, "swamp");

    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 3,
            ..Default::default()
        },
    );
    let ii = hand_index_for_card(&e, 0, "instill_infection");
    e.apply_command(
        0,
        &cast_spell(
            ii,
            vec![TargetRef {
                object_id: sorc,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast instill infection");
    pass_both_players(&mut e);

    assert!(
        !e.state.players[0].battlefield.contains(&sorc),
        "0-toughness creature left the battlefield"
    );
    assert!(
        e.state.players[0].graveyard.contains(&sorc),
        "dead creature is in its owner's graveyard"
    );
}

#[test]
fn marked_damage_clears_at_cleanup() {
    let decks = Some(vec![
        {
            let mut d = vec![
                "forest".into(),
                "giant_growth".into(),
                "grizzly_bears".into(),
            ];
            d.extend(std::iter::repeat_n("forest".into(), 17));
            d
        },
        vec!["mountain".into(); 20],
    ]);
    let mut e = GameEngine::new(906, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let forest_idx = hand_index_for_card(&e, 0, "forest");
    let forest_oid = e.state.players[0].hand.remove(forest_idx);
    e.state.players[0].battlefield.push(forest_oid);
    e.state.objects.get_mut(&forest_oid).expect("forest").zone = tricerules_core::Zone::Battlefield;

    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let growth_idx = hand_index_for_card(&e, 0, "giant_growth");
    e.apply_command(
        0,
        &cast_spell(
            growth_idx,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast growth");
    pass_both_players(&mut e);

    assert_eq!(e.state.objects.get(&bear).expect("bear").damage, 0);

    if let Some(o) = e.state.objects.get_mut(&bear) {
        o.damage = 1;
    }
    assert_eq!(e.state.objects.get(&bear).expect("bear").damage, 1);

    end_active_turn(&mut e, 0);

    assert_eq!(
        e.state
            .objects
            .get(&bear)
            .expect("bear after cleanup")
            .damage,
        0,
        "marked damage should clear during cleanup"
    );
}

/// CR 400.7 / 121.2: counters and marked damage do not survive a zone change. A creature with a
/// +1/+1 counter and combat damage that is bounced to hand becomes a new object — the returned
/// card must carry neither, so a recast is a fresh 2/2 rather than a damaged 3/3.
#[test]
fn bounce_clears_counters_and_marked_damage() {
    use tricerules_cards::CounterKind;
    let decks = Some(vec![
        vec![
            "island".into(),
            "unsummon".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
        forest_only_deck(),
    ]);
    let mut e = GameEngine::new(2611, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    {
        let o = e.state.objects.get_mut(&bear).expect("bear");
        o.set_counter(CounterKind::PlusOnePlusOne, 1);
        o.damage = 1;
    }
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "unsummon");
    e.apply_command(
        0,
        &cast_spell(
            idx,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast unsummon");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");
    let returned = e.state.objects.get(&bear).expect("bear");
    assert_eq!(returned.zone, tricerules_core::Zone::Hand);
    assert_eq!(
        returned.counter_count(CounterKind::PlusOnePlusOne),
        0,
        "counters must not survive the bounce"
    );
    assert_eq!(
        returned.damage, 0,
        "marked damage must not survive the bounce"
    );
}

/// P1: Glorious Anthem ("Creatures you control get +1/+1") buffs only the controller's creatures.
/// A creature an opponent controls is untouched (controller-filtered scope).
#[test]
fn glorious_anthem_buffs_only_controllers_creatures() {
    let mut e = anthem_engine(5001, "glorious_anthem");
    let mine = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
    let theirs = inject_creature_on_battlefield(&mut e, 1, "savannah_lions");

    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 3,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "glorious_anthem");
    e.apply_command(0, &cast_spell(idx, vec![]))
        .expect("cast anthem");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(e.effective_power(mine), Some(3), "your creature gets +1/+1");
    assert_eq!(e.effective_toughness(mine), Some(3));
    assert_eq!(
        e.effective_power(theirs),
        Some(2),
        "opponent's creature is not affected by your anthem"
    );
}

/// P1 dynamic scope (not a snapshot): a creature entering *after* Glorious Anthem resolves is
/// still buffed, because `CreaturesMatching` is evaluated on each P/T query.
#[test]
fn anthem_scope_is_dynamic_for_creatures_entering_later() {
    let mut e = anthem_engine(5002, "glorious_anthem");
    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 3,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "glorious_anthem");
    e.apply_command(0, &cast_spell(idx, vec![]))
        .expect("cast anthem");
    resolve_entire_stack_two_player(&mut e);

    // Enters after the anthem is already on the battlefield.
    let late = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
    assert_eq!(
        e.effective_power(late),
        Some(3),
        "a creature entering after the anthem is buffed (dynamic scope)"
    );
}

/// P1 LTB drain: bouncing Glorious Anthem off the battlefield removes its continuous effect
/// (CR 604.3/611.3) — the buff disappears the moment the source leaves.
#[test]
fn anthem_buff_drains_when_source_leaves_battlefield() {
    let decks = Some(vec![
        vec![
            "glorious_anthem".into(),
            "boomerang".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
    ]);
    let mut e = GameEngine::new(5003, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let mine = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");

    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 3,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "glorious_anthem");
    e.apply_command(0, &cast_spell(idx, vec![]))
        .expect("cast anthem");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.effective_power(mine),
        Some(3),
        "buffed while anthem in play"
    );

    let anthem = battlefield_object_for_card(&e, 0, "glorious_anthem");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let b_idx = hand_index_for_card(&e, 0, "boomerang");
    e.apply_command(
        0,
        &cast_spell(
            b_idx,
            vec![TargetRef {
                object_id: anthem,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast boomerang at own anthem");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.effective_power(mine),
        Some(2),
        "buff drains when the anthem leaves the battlefield"
    );
}

/// P1 color-filtered, symmetrical lord: Crusade ("White creatures get +1/+1") buffs every white
/// creature regardless of controller, and leaves non-white creatures alone.
#[test]
fn crusade_buffs_white_creatures_of_either_player() {
    let mut e = anthem_engine(5004, "crusade");
    let my_white = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
    let my_green = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let their_white = inject_creature_on_battlefield(&mut e, 1, "savannah_lions");

    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 2,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "crusade");
    e.apply_command(0, &cast_spell(idx, vec![]))
        .expect("cast crusade");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.effective_power(my_white),
        Some(3),
        "your white creature buffed"
    );
    assert_eq!(
        e.effective_power(their_white),
        Some(3),
        "opponent's white creature is also buffed (symmetrical color lord)"
    );
    assert_eq!(
        e.effective_power(my_green),
        Some(2),
        "a non-white creature is not affected"
    );
}

/// P1 subtype + exclude_self: Captain of the Watch ("Other Soldier creatures you control get
/// +1/+1") buffs only your other Soldiers — not itself, not your non-Soldiers, not the opponent's.
#[test]
fn captain_of_the_watch_buffs_other_soldiers_you_control() {
    let mut e = anthem_engine(5005, "captain_of_the_watch");
    let my_soldier = inject_creature_on_battlefield(&mut e, 0, "squire"); // Human Soldier
    let my_nonsoldier = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let their_soldier = inject_creature_on_battlefield(&mut e, 1, "squire");

    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 6,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "captain_of_the_watch");
    e.apply_command(0, &cast_spell(idx, vec![]))
        .expect("cast captain");
    // Resolve the spell and its ETB token-making trigger to a quiet board.
    while !e.state.stack.is_empty() || !e.state.pending_triggers.is_empty() {
        pass_both_players(&mut e);
    }

    let captain = battlefield_object_for_card(&e, 0, "captain_of_the_watch");
    assert_eq!(
        e.effective_power(captain),
        Some(3),
        "Captain does not buff itself (exclude_self / 'other')"
    );
    assert_eq!(
        e.effective_power(my_soldier),
        Some(3),
        "your other Soldier gets +1/+1"
    );
    assert_eq!(
        e.effective_power(my_nonsoldier),
        Some(2),
        "your non-Soldier is unaffected (subtype filter)"
    );
    assert_eq!(
        e.effective_power(their_soldier),
        Some(2),
        "opponent's Soldier is unaffected (controller filter)"
    );
}

/// P1 one-shot sibling: Glorious Charge ("Creatures you control get +1/+1 until end of turn")
/// pumps your team and expires at cleanup (CR 514.2).
#[test]
fn glorious_charge_pumps_team_until_cleanup() {
    let mut e = anthem_engine(5006, "glorious_charge");
    let mine = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 2,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "glorious_charge");
    e.apply_command(0, &cast_spell(idx, vec![]))
        .expect("cast glorious charge");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(mine), Some(3), "+1/+1 this turn");
    assert_eq!(e.effective_toughness(mine), Some(3));

    end_active_turn(&mut e, 0);
    assert_eq!(
        e.effective_power(mine),
        Some(2),
        "the until-end-of-turn pump expires at cleanup"
    );
}

/// Issue #75: an opponents-only one-shot scope snapshots the affected objects at resolution.
/// Later entrants are unaffected, while a snapshotted object stays affected after control changes.
#[test]
fn issue_75_uncomfortable_chill_snapshots_opponents_and_draws() {
    let decks = Some(vec![
        vec![
            "uncomfortable_chill".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(75_001, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let mine = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let theirs = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    inject_library_card(&mut e, 0, "island");
    let hand_before = e.state.players[0].hand.len();

    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 3,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "uncomfortable_chill");
    e.apply_command(0, &cast_spell(idx, vec![]))
        .expect("cast Uncomfortable Chill");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.effective_power(mine),
        Some(2),
        "your creature is unchanged"
    );
    assert_eq!(
        e.effective_power(theirs),
        Some(0),
        "opponent's creature gets -2/-0"
    );
    assert_eq!(e.effective_toughness(theirs), Some(2));
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "casting one card and drawing one card leaves the same hand size"
    );

    let late = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    assert_eq!(
        e.effective_power(late),
        Some(2),
        "a creature entering after resolution was not in the snapshot"
    );

    e.state.players[1].battlefield.retain(|oid| *oid != theirs);
    e.state.players[0].battlefield.push(theirs);
    let changed = e.state.objects.get_mut(&theirs).expect("affected creature");
    changed.base_controller = 0;
    changed.controller = 0;
    assert_eq!(
        e.effective_power(theirs),
        Some(0),
        "an affected object remains affected after its controller changes"
    );

    end_active_turn(&mut e, 0);
    assert_eq!(
        e.effective_power(theirs),
        Some(2),
        "the debuff expires at cleanup"
    );
}

/// Issue #75: the -1/-1 scope excludes the caster's creature and normal SBAs remove an opposing
/// creature whose toughness becomes zero.
#[test]
fn issue_75_make_obsolete_only_kills_opposing_creatures() {
    let decks = Some(vec![
        vec![
            "make_obsolete".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
        ],
        vec!["plains".into(); 7],
    ]);
    let mut e = GameEngine::new(75_002, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let mine = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
    let theirs = inject_creature_on_battlefield(&mut e, 1, "savannah_lions");
    e.state.objects.get_mut(&mine).expect("own Lion").toughness = Some(1);
    e.state
        .objects
        .get_mut(&theirs)
        .expect("opposing Lion")
        .toughness = Some(1);

    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 3,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "make_obsolete");
    e.apply_command(0, &cast_spell(idx, vec![]))
        .expect("cast Make Obsolete");
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.state.objects[&mine].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(e.effective_power(mine), Some(2));
    assert_eq!(e.effective_toughness(mine), Some(1));
    assert_eq!(
        e.state.objects[&theirs].zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(e.state.players[1].graveyard.contains(&theirs));
}
