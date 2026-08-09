//! Scenario tests for the Equipment mechanic (CR 301.5 / 702.6).
//!
//! Oracle refs: Bonesplitter (+2/+0, Equip {1}), Vulshok Morningstar (+2/+2, Equip {2}).
//! Happy paths: equip, re-equip. Illegal paths: opponent's creature.
//! SBA: equipment falls off when attached creature leaves the battlefield (CR 704.5p).

use crate::helpers::*;
use tricerules_cards::Keyword;
use tricerules_proto::ruled::v1::TargetRef;

fn equipment_deck(equipment: &str) -> Vec<String> {
    (0..20)
        .map(|_| equipment.to_string())
        .chain((0..40).map(|_| "grizzly_bears".to_string()))
        .take(60)
        .collect()
}

/// Cast equipment and bring it onto the battlefield, returning its ObjectId.
fn cast_and_resolve_equipment(
    e: &mut GameEngine,
    player: usize,
    card_id: &str,
    mana: ManaGift,
) -> u32 {
    take_card_from_library_to_hand(e, player, card_id);
    give_mana(e, player as i32, mana);
    let idx = hand_index_for_card(e, player, card_id);
    e.apply_command(player as i32, &cast_spell(idx, vec![]))
        .expect("cast equipment");
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, player, card_id)
}

/// Equip Bonesplitter to a creature you control — creature gets +2/+0 bonus (layer 7c).
#[test]
fn bonesplitter_equip_adds_bonus() {
    let decks = Some(vec![
        equipment_deck("bonesplitter"),
        equipment_deck("grizzly_bears"),
    ]);
    let mut e = GameEngine::new(5001, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Cast Bonesplitter ({1}) so emit_static_abilities_on_enter fires.
    let splitter = cast_and_resolve_equipment(
        &mut e,
        0,
        "bonesplitter",
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let creature = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    // Before equip: unattached equipment gives no bonus.
    assert_eq!(e.effective_power(creature), Some(2));
    assert_eq!(e.effective_toughness(creature), Some(2));
    assert_eq!(e.state.objects.get(&splitter).unwrap().attached_to, None);

    // Activate equip ability (index 0) targeting the creature; costs {1}.
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    e.apply_command(
        0,
        &activate_ability(
            splitter,
            0,
            vec![TargetRef {
                object_id: creature,
                damage_amount: 0,
            }],
        ),
    )
    .expect("equip Bonesplitter");

    // Both players pass → ability resolves.
    pass_both_players(&mut e);
    assert!(e.state.stack.is_empty(), "equip resolves off the stack");

    assert_eq!(
        e.state
            .objects
            .get(&splitter)
            .expect("splitter")
            .attached_to,
        Some(creature),
        "Bonesplitter attached_to should be the creature"
    );
    // Creature now has +2/+0 from the attached modifier (layer 7c).
    assert_eq!(e.effective_power(creature), Some(4), "+2 from Bonesplitter");
    assert_eq!(
        e.effective_toughness(creature),
        Some(2),
        "toughness unchanged"
    );
}

/// Re-equip to a different creature: old creature loses the bonus, new creature gains it.
/// CR 702.6: the equipment detaches from the previous host automatically.
#[test]
fn bonesplitter_reequip_shifts_bonus() {
    let decks = Some(vec![
        equipment_deck("bonesplitter"),
        equipment_deck("grizzly_bears"),
    ]);
    let mut e = GameEngine::new(5002, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let splitter = cast_and_resolve_equipment(
        &mut e,
        0,
        "bonesplitter",
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let bear1 = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let bear2 = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    // First equip to bear1.
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    e.apply_command(
        0,
        &activate_ability(
            splitter,
            0,
            vec![TargetRef {
                object_id: bear1,
                damage_amount: 0,
            }],
        ),
    )
    .expect("first equip");
    pass_both_players(&mut e);

    assert_eq!(e.effective_power(bear1), Some(4), "bear1 has +2");
    assert_eq!(e.effective_power(bear2), Some(2), "bear2 unaffected");

    // Re-equip to bear2.
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    e.apply_command(
        0,
        &activate_ability(
            splitter,
            0,
            vec![TargetRef {
                object_id: bear2,
                damage_amount: 0,
            }],
        ),
    )
    .expect("re-equip");
    pass_both_players(&mut e);

    assert_eq!(
        e.state
            .objects
            .get(&splitter)
            .expect("splitter")
            .attached_to,
        Some(bear2),
        "now attached to bear2"
    );
    assert_eq!(e.effective_power(bear1), Some(2), "bear1 bonus gone");
    assert_eq!(e.effective_power(bear2), Some(4), "bear2 gains +2");
}

/// SBA: equipment falls off when the attached creature dies (CR 704.5p).
/// Lethal damage → SBAs run on the next priority pass → bear dies → equipment unattaches.
#[test]
fn equipment_falls_off_when_creature_dies() {
    let decks = Some(vec![
        equipment_deck("bonesplitter"),
        equipment_deck("grizzly_bears"),
    ]);
    let mut e = GameEngine::new(5003, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let splitter = cast_and_resolve_equipment(
        &mut e,
        0,
        "bonesplitter",
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    // Attach Bonesplitter (bear is now 4/2 effective, but bear's base toughness is 2).
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    e.apply_command(
        0,
        &activate_ability(
            splitter,
            0,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .expect("equip");
    pass_both_players(&mut e);
    assert_eq!(
        e.state.objects.get(&splitter).unwrap().attached_to,
        Some(bear)
    );

    // Mark lethal damage on the bear (base 2/2 toughness, 2 damage = lethal per CR 704.5g).
    // Bonesplitter's attached modifier adds power only (+2/+0), not toughness.
    e.state.objects.get_mut(&bear).unwrap().damage = 2;

    // SBAs run inside pass_priority (CR 704.3); passing priority flushes them.
    pass_both_players(&mut e);

    // Creature should have died.
    assert_eq!(
        e.state.objects.get(&bear).map(|o| o.zone),
        Some(tricerules_core::Zone::Graveyard),
        "bear dies to lethal damage"
    );
    // Equipment should have unattached (SBA: CR 704.5p analogue — equipment falls off).
    assert_eq!(
        e.state
            .objects
            .get(&splitter)
            .expect("splitter")
            .attached_to,
        None,
        "Bonesplitter detaches when its host dies"
    );
    // Equipment stays on battlefield.
    assert_eq!(
        e.state.objects.get(&splitter).map(|o| o.zone),
        Some(tricerules_core::Zone::Battlefield),
        "equipment remains on battlefield"
    );
}

/// CR 702.6a: equip can only target "a creature you control" — opponent's creature is illegal.
#[test]
fn equip_cannot_target_opponent_creature() {
    let decks = Some(vec![
        equipment_deck("bonesplitter"),
        equipment_deck("grizzly_bears"),
    ]);
    let mut e = GameEngine::new(5004, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let splitter = cast_and_resolve_equipment(
        &mut e,
        0,
        "bonesplitter",
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let opp_bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );

    let err = e.apply_command(
        0,
        &activate_ability(
            splitter,
            0,
            vec![TargetRef {
                object_id: opp_bear,
                damage_amount: 0,
            }],
        ),
    );
    assert!(err.is_err(), "equip cannot target opponent's creature");
    assert!(e.state.stack.is_empty(), "nothing on the stack");
    assert_eq!(
        e.state.objects.get(&splitter).unwrap().attached_to,
        None,
        "not attached"
    );
}

/// Vulshok Morningstar grants +2/+2 (layer 7c).
#[test]
fn vulshok_morningstar_adds_power_and_toughness() {
    let decks = Some(vec![
        equipment_deck("vulshok_morningstar"),
        equipment_deck("grizzly_bears"),
    ]);
    let mut e = GameEngine::new(5005, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let star = cast_and_resolve_equipment(
        &mut e,
        0,
        "vulshok_morningstar",
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    // Vulshok Morningstar equip costs {2} generic.
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    e.apply_command(
        0,
        &activate_ability(
            star,
            0,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .expect("equip Vulshok Morningstar");
    pass_both_players(&mut e);

    assert_eq!(e.state.objects.get(&star).unwrap().attached_to, Some(bear));
    assert_eq!(
        e.effective_power(bear),
        Some(4),
        "+2 power from Morningstar"
    );
    assert_eq!(
        e.effective_toughness(bear),
        Some(4),
        "+2 toughness from Morningstar"
    );
}

/// CR 702.6a: "Equip only as a sorcery" is built into the keyword. Activating it while the stack
/// is not empty — or on the opponent's turn — is illegal, even with the mana available.
#[test]
fn equip_is_rejected_at_instant_speed() {
    let decks = Some(vec![
        equipment_deck("bonesplitter"),
        equipment_deck("grizzly_bears"),
    ]);
    let mut e = GameEngine::new(5099, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let splitter = cast_and_resolve_equipment(
        &mut e,
        0,
        "bonesplitter",
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let bears = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 5,
            ..Default::default()
        },
    );

    // Hand priority to P1 and advance into P1's turn, where P0 has no sorcery-speed window.
    end_active_turn(&mut e, 0);
    let err = e.apply_command(
        0,
        &activate_ability(
            splitter,
            0,
            vec![TargetRef {
                object_id: bears,
                damage_amount: 0,
            }],
        ),
    );
    assert!(
        err.is_err(),
        "equip must be rejected outside a sorcery-speed window"
    );
    assert!(
        e.state
            .objects
            .get(&splitter)
            .expect("bonesplitter")
            .attached_to
            .is_none(),
        "the rejected equip must not have attached anything"
    );
}

/// The client greys the ability menu from `AbilityInfo.activatable`, so the engine must report
/// equip as unavailable outside a sorcery-speed window — otherwise the menu opens, collects mana,
/// and the engine rejects the command it produced.
#[test]
fn equip_is_reported_unactivatable_at_instant_speed() {
    let decks = Some(vec![
        equipment_deck("bonesplitter"),
        equipment_deck("grizzly_bears"),
    ]);
    let mut e = GameEngine::new(5098, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let splitter = cast_and_resolve_equipment(
        &mut e,
        0,
        "bonesplitter",
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );

    let activatable_now = |e: &mut GameEngine| -> bool {
        zone_view_ability_flags(e, 0, splitter)
            .first()
            .copied()
            .expect("bonesplitter has an equip ability")
    };
    assert!(
        activatable_now(&mut e),
        "equip is available in our own main phase with an empty stack"
    );

    end_active_turn(&mut e, 0);
    assert!(
        !activatable_now(&mut e),
        "equip must report as unavailable on the opponent's turn (CR 702.6a)"
    );
}

#[test]
fn swiftfoot_boots_moves_both_keywords_on_reequip() {
    let decks = Some(vec![
        equipment_deck("swiftfoot_boots"),
        equipment_deck("grizzly_bears"),
    ]);
    let mut e = GameEngine::new(5010, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let boots = cast_and_resolve_equipment(
        &mut e,
        0,
        "swiftfoot_boots",
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
    let bear1 = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let bear2 = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.state
        .objects
        .get_mut(&bear1)
        .expect("bear1")
        .summoning_sick = true;
    e.state
        .objects
        .get_mut(&bear2)
        .expect("bear2")
        .summoning_sick = true;

    for target in [bear1, bear2] {
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        e.apply_command(
            0,
            &activate_ability(
                boots,
                0,
                vec![TargetRef {
                    object_id: target,
                    damage_amount: 0,
                }],
            ),
        )
        .expect("equip Swiftfoot Boots");
        pass_both_players(&mut e);
    }

    assert!(!e.effective_has_keyword(bear1, Keyword::Hexproof));
    assert!(!e.effective_has_keyword(bear1, Keyword::Haste));
    assert!(e.effective_has_keyword(bear2, Keyword::Hexproof));
    assert!(e.effective_has_keyword(bear2, Keyword::Haste));
    assert!(
        zone_view_granted_ability_labels(&mut e, 0, bear1).is_empty(),
        "re-equipping removes both granted labels from the old bearer"
    );
    assert_eq!(
        zone_view_granted_ability_labels(&mut e, 0, bear2),
        vec!["Hexproof", "Haste"],
        "multiple granted keywords retain their effective-characteristics order"
    );

    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap pass begin combat");
    e.apply_command(1, &pass()).expect("nap pass begin combat");
    assert!(
        e.apply_command(0, &declare_attackers(vec![bear1])).is_err(),
        "the previously equipped summoning-sick creature lost haste"
    );
    e.apply_command(0, &declare_attackers(vec![bear2]))
        .expect("the currently equipped creature can attack with haste");
}

#[test]
fn equipment_unattaches_when_host_stops_being_a_creature() {
    let decks = Some(vec![
        equipment_deck("bonesplitter"),
        equipment_deck("grizzly_bears"),
    ]);
    let mut e = GameEngine::new(5011, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let splitter = cast_and_resolve_equipment(
        &mut e,
        0,
        "bonesplitter",
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    e.apply_command(
        0,
        &activate_ability(
            splitter,
            0,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .expect("equip Bonesplitter");
    pass_both_players(&mut e);

    e.state.objects.get_mut(&bear).expect("bear").card_id = "plains".into();
    e.apply_command(0, &pass()).expect("pass triggers SBAs");
    assert_eq!(
        e.state
            .objects
            .get(&splitter)
            .and_then(|object| object.attached_to),
        None
    );
    assert_eq!(
        e.state.objects.get(&splitter).map(|object| object.zone),
        Some(tricerules_core::Zone::Battlefield),
        "illegal Equipment detaches instead of going to the graveyard"
    );
}

#[test]
fn short_sword_grants_its_printed_pt_bonus() {
    let decks = Some(vec![
        equipment_deck("short_sword"),
        equipment_deck("grizzly_bears"),
    ]);
    let mut e = GameEngine::new(5012, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let sword = cast_and_resolve_equipment(
        &mut e,
        0,
        "short_sword",
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    e.apply_command(
        0,
        &activate_ability(
            sword,
            0,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .expect("equip Short Sword");
    pass_both_players(&mut e);
    assert_eq!(e.effective_power(bear), Some(3));
    assert_eq!(e.effective_toughness(bear), Some(3));
}
