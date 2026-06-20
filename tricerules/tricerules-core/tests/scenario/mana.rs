use crate::helpers::*;

/// CR 605: a basic land's `{T}: Add {C}` mana ability is engine-owned. Activating it taps the
/// source and adds the mana immediately — no stack, no priority change (CR 605.3a–b) — and the
/// engine emits the authoritative pool.
#[test]
fn mana_ability_taps_land_and_fills_pool() {
    let decks = Some(vec![
        {
            let mut d = vec!["mountain".to_string(), "lightning_bolt".to_string()];
            d.extend(std::iter::repeat_n("mountain".to_string(), 10));
            d
        },
        vec!["forest".into(); 12],
    ]);
    let mut e = GameEngine::new(7, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");
    let land = *e.state.players[0].battlefield.last().expect("land on bf");
    assert!(!e.state.objects[&land].tapped, "land starts untapped");

    let priority_before = e.state.priority_player_id();
    let batch = e
        .apply_command(0, &activate_ability(land, 0, vec![]))
        .expect("activate mountain mana ability");

    // Mana produced into the pool; source tapped.
    assert_eq!(e.state.players[0].mana_pool.red, 1, "produced {{R}}");
    assert!(e.state.objects[&land].tapped, "source tapped as cost");
    // CR 605.3a–b: no stack, no priority change.
    assert!(
        e.state.stack.is_empty(),
        "mana ability never uses the stack"
    );
    assert_eq!(
        e.state.priority_player_id(),
        priority_before,
        "priority unchanged by a mana ability"
    );
    assert!(
        !batch
            .events
            .iter()
            .any(|ev| matches!(ev.ev, Some(Ev::StackPushed(_)))),
        "no StackPushed for a mana ability"
    );
    let pool_ev = batch
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::ManaPoolUpdated(m)) if m.player_id == 0 => Some(m),
            _ => None,
        })
        .expect("ManaPoolUpdated for the active player");
    assert_eq!(pool_ev.r, 1, "pool event reflects produced red");

    // The produced mana actually pays for a spell.
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, target_player(1)))
        .expect("cast bolt off the tapped land's mana");
    assert_eq!(e.state.players[0].mana_pool.red, 0, "pool drained by cast");
}

/// CR 605.1a: a tapped source can't pay its own {T} cost again (CR 602.5 / 302.6); a second
/// activation in the same turn is illegal and produces no extra mana.
#[test]
fn cannot_activate_mana_ability_when_already_tapped() {
    let mut e = GameEngine::new(8, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let land = inject_permanent_on_battlefield(&mut e, 0, "mountain");
    e.apply_command(0, &activate_ability(land, 0, vec![]))
        .expect("first tap produces mana");
    assert_eq!(e.state.players[0].mana_pool.red, 1);
    let err = e
        .apply_command(0, &activate_ability(land, 0, vec![]))
        .expect_err("already-tapped land cannot tap again");
    assert!(
        format!("{err:?}").contains("already tapped"),
        "unexpected error: {err:?}"
    );
    assert_eq!(e.state.players[0].mana_pool.red, 1, "no extra mana");
}

/// CR 601.2h: an activated ability's cost is paid atomically. Activating a `TapAndMana` ability
/// (Jayemdae Tome's "{4}, {T}: Draw a card.") whose source is already tapped must be rejected
/// *without* draining the mana — `apply_command` does not roll back partial state mutations on an
/// Illegal result, so paying the {4} first and then failing the tap would burn the player's pool.
#[test]
fn tap_and_mana_ability_rejected_when_tapped_leaves_pool_intact() {
    let mut e = GameEngine::new(11, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let tome = inject_permanent_on_battlefield(&mut e, 0, "jayemdae_tome");
    // Source already tapped (e.g. used earlier this turn); the player has the {4} ready in pool.
    e.state.objects.get_mut(&tome).unwrap().tapped = true;
    e.state.players[0].mana_pool.colorless = 4;

    let err = e
        .apply_command(0, &activate_ability(tome, 0, vec![]))
        .expect_err("cannot activate a TapAndMana ability on an already-tapped source");
    assert!(
        format!("{err:?}").contains("already tapped"),
        "unexpected error: {err:?}"
    );
    assert_eq!(
        e.state.players[0].mana_pool.colorless, 4,
        "mana pool must be untouched when the tap precondition fails"
    );
}

/// CR 605 float courtesy: a freshly activated pure-`{T}` mana ability is undoable while still
/// inconsequential. `UndoManaAbility` untaps the source and removes exactly the produced mana, and
/// the controller's `LegalActions` advertises the undoable count.
#[test]
fn undo_mana_ability_untaps_source_and_removes_float() {
    let mut e = GameEngine::new(31, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let land = inject_permanent_on_battlefield(&mut e, 0, "mountain");

    let batch = e
        .apply_command(0, &activate_ability(land, 0, vec![]))
        .expect("tap mountain for {R}");
    assert_eq!(e.state.players[0].mana_pool.red, 1, "floated {{R}}");
    assert!(
        e.state.objects[&land].tapped,
        "source tapped as the {{T}} cost"
    );
    assert_eq!(
        batch.legal_by_player[&0].undoable_mana_abilities, 1,
        "one undoable float advertised to the controller"
    );

    let undo = e
        .apply_command(0, &undo_mana_ability())
        .expect("undo the float");
    assert_eq!(e.state.players[0].mana_pool.red, 0, "floated mana removed");
    assert!(
        !e.state.objects[&land].tapped,
        "source untapped by the undo"
    );
    assert_eq!(
        undo.legal_by_player[&0].undoable_mana_abilities, 0,
        "nothing left to undo after rewinding"
    );

    let err = e
        .apply_command(0, &undo_mana_ability())
        .expect_err("no float remains to undo");
    assert!(
        format!("{err:?}").contains("no mana ability to undo"),
        "unexpected error: {err:?}"
    );
}

/// The undo courtesy ends the instant the float is spent: casting a spell off the floated mana
/// clears the undo history, so a later `UndoManaAbility` is rejected (it would otherwise let a
/// player re-float spent mana / untap a land that paid for a spell on the stack).
#[test]
fn undo_mana_ability_cleared_once_float_is_spent() {
    let decks = Some(vec![
        {
            let mut d = vec!["mountain".to_string(), "lightning_bolt".to_string()];
            d.extend(std::iter::repeat_n("mountain".to_string(), 10));
            d
        },
        vec!["forest".into(); 12],
    ]);
    let mut e = GameEngine::new(13, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");
    let land = *e.state.players[0].battlefield.last().expect("land on bf");
    e.apply_command(0, &activate_ability(land, 0, vec![]))
        .expect("tap mountain for {R}");

    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, target_player(1)))
        .expect("cast bolt off the floated red");

    let err = e
        .apply_command(0, &undo_mana_ability())
        .expect_err("float was spent on the bolt; nothing to undo");
    assert!(
        format!("{err:?}").contains("no mana ability to undo"),
        "unexpected error: {err:?}"
    );
}

/// Passing priority makes a float consequential (the opponent gets a window), so the undo history
/// is dropped: the floated mana stays in the pool but can no longer be rewound.
#[test]
fn undo_mana_ability_cleared_by_passing_priority() {
    let mut e = GameEngine::new(14, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let land = inject_permanent_on_battlefield(&mut e, 0, "mountain");
    e.apply_command(0, &activate_ability(land, 0, vec![]))
        .expect("tap mountain for {R}");

    e.apply_command(0, &pass()).expect("pass priority");
    // Stack cleared by the pass and it is no longer player 0's priority.
    let err = e
        .apply_command(0, &undo_mana_ability())
        .expect_err("cannot undo after passing priority");
    assert!(
        format!("{err:?}").contains("not your priority"),
        "unexpected error: {err:?}"
    );
    assert_eq!(
        e.state.players[0].mana_pool.red, 1,
        "floated mana persists (only the undo affordance is gone)"
    );
}

/// CR 605: a multi-option mana ability (Tropical Island, "{T}: Add {G} or {U}") produces the
/// option the player chose via `mana_option_index`; an out-of-range index is rejected.
#[test]
fn dual_land_produces_chosen_color_option() {
    let mut e = GameEngine::new(9, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Option index 1 is {U}.
    let land = inject_permanent_on_battlefield(&mut e, 0, "tropical_island");
    let cmd = RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            permanent_id: land,
            ability_index: 0,
            mana_option_index: 1,
            ..Default::default()
        })),
    };
    e.apply_command(0, &cmd).expect("produce blue option");
    assert_eq!(e.state.players[0].mana_pool.blue, 1, "chose {{U}}");
    assert_eq!(e.state.players[0].mana_pool.green, 0, "did not make {{G}}");

    // An out-of-range option on a fresh source is illegal and produces nothing.
    let land2 = inject_permanent_on_battlefield(&mut e, 0, "tropical_island");
    let bad = RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            permanent_id: land2,
            ability_index: 0,
            mana_option_index: 5,
            ..Default::default()
        })),
    };
    let err = e.apply_command(0, &bad).expect_err("bad option index");
    assert!(
        format!("{err:?}").contains("invalid mana option"),
        "unexpected error: {err:?}"
    );
    assert!(!e.state.objects[&land2].tapped, "rejected: source untapped");
}

#[test]
fn cast_1u_creature_pays_from_mana_pool_without_tapping_extra_island() {
    let decks = Some(vec![
        vec![
            "island".into(),
            "island".into(),
            "mountain".into(),
            "coral_merfolk".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
        vec!["mountain".into(); 7],
    ]);
    let mut e = GameEngine::new(202, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Two islands + mountain on the battlefield (no land drop this turn).
    for _ in 0..2 {
        let idx = hand_index_for_card(&e, 0, "island");
        let oid = e.state.players[0].hand.remove(idx);
        e.state.players[0].battlefield.push(oid);
        e.state.objects.get_mut(&oid).expect("obj").zone = tricerules_core::Zone::Battlefield;
    }
    {
        let idx = hand_index_for_card(&e, 0, "mountain");
        let oid = e.state.players[0].hand.remove(idx);
        e.state.players[0].battlefield.push(oid);
        e.state.objects.get_mut(&oid).expect("obj").zone = tricerules_core::Zone::Battlefield;
    }
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            r: 1,
            ..Default::default()
        },
    );
    let merfolk_idx = hand_index_for_card(&e, 0, "coral_merfolk");
    e.apply_command(0, &cast_spell(merfolk_idx, vec![]))
        .expect("cast");
    let tapped_islands = e.state.players[0]
        .battlefield
        .iter()
        .filter(|oid| {
            e.state
                .objects
                .get(*oid)
                .map(|o| o.card_id == "island" && o.tapped)
                .unwrap_or(false)
        })
        .count();
    assert_eq!(
        tapped_islands, 0,
        "1U paid from pool; no extra island should auto-tap"
    );
    let mountain_oid = battlefield_object_for_card(&e, 0, "mountain");
    assert!(
        !e.state.objects.get(&mountain_oid).expect("mountain").tapped,
        "mountain should not be tapped by engine payment"
    );
}

#[test]
fn cast_grizzly_bears_resolves_to_battlefield_and_taps_two_forests() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "forest".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec![
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
    ]);
    let mut e = GameEngine::new(22, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Simulate one untapped Forest that was played on a previous turn.
    let seeded_forest_idx = hand_index_for_card(&e, 0, "forest");
    let seeded_forest_oid = e.state.players[0].hand.remove(seeded_forest_idx);
    e.state.players[0].battlefield.push(seeded_forest_oid);
    e.state
        .objects
        .get_mut(&seeded_forest_oid)
        .expect("seeded forest")
        .zone = tricerules_core::Zone::Battlefield;

    // Play the second Forest this turn.
    let forest_to_play_idx = hand_index_for_card(&e, 0, "forest");
    e.apply_command(0, &play_land(forest_to_play_idx))
        .expect("play second forest");

    // Tap both forests to pay for grizzly bears (simulating player tapping lands for mana).
    for &oid in &e.state.players[0].battlefield.clone() {
        if e.state.objects.get(&oid).map(|o| o.card_id.as_str()) == Some("forest") {
            e.state.objects.get_mut(&oid).expect("forest").tapped = true;
        }
    }
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            c: 1,
            ..Default::default()
        },
    );
    let bears_idx = hand_index_for_card(&e, 0, "grizzly_bears");
    e.apply_command(0, &cast_spell(bears_idx, vec![]))
        .expect("cast bears");
    let bears_oid = e.state.stack.first().expect("bears stack item").id;

    let untapped_before_resolve = e.state.players[0]
        .battlefield
        .iter()
        .filter(|oid| e.state.objects.get(oid).map(|o| !o.tapped).unwrap_or(false))
        .count();
    assert_eq!(untapped_before_resolve, 0, "both forests are tapped for 1G");

    e.apply_command(0, &pass()).expect("p0 pass");
    let resolved = e.apply_command(1, &pass()).expect("p1 pass");

    assert!(e.state.players[0].battlefield.contains(&bears_oid));
    assert!(resolved.events.iter().any(|ev| {
        matches!(
            ev.ev,
            Some(Ev::StackResolved(ref r))
                if r.object_id == bears_oid
                    && r.destination
                        == tricerules_proto::ruled::v1::StackResolveDestination::Battlefield as i32
        )
    }));
}

/// CR 107.4d: a hybrid card ({R/W} Boros Recruit) casts identically whether the pip is paid
/// with red or with white — both runs resolve the creature onto the battlefield.
#[test]
fn hybrid_creature_castable_with_either_color() {
    for (red, white) in [(1, 0), (0, 1)] {
        let decks = Some(vec![
            vec![
                "boros_recruit".into(),
                "mountain".into(),
                "mountain".into(),
                "mountain".into(),
                "mountain".into(),
                "mountain".into(),
                "mountain".into(),
            ],
            vec!["forest".into(); 7],
        ]);
        let mut e = GameEngine::new(71, &[0, 1], 20, decks, true).expect("new");
        advance_to_main1_from_game_start(&mut e);

        // Add exactly one R (first run) or one W (second run) — the {R/W} pip takes whichever.
        give_mana(
            &mut e,
            0,
            ManaGift {
                r: red,
                w: white,
                ..Default::default()
            },
        );

        let idx = hand_index_for_card(&e, 0, "boros_recruit");
        e.apply_command(0, &cast_spell(idx, vec![]))
            .expect("cast Boros Recruit");
        let oid = e.state.stack.last().expect("on stack").id;
        pass_both_players(&mut e);
        assert!(
            e.state.players[0].battlefield.contains(&oid),
            "Boros Recruit resolves to battlefield (red={red}, white={white})"
        );
    }
}

/// CR 107.4e: Flame Javelin ({2/R}{2/R}{2/R}) paid entirely with generic mana (6 colorless)
/// deals 4 damage to a player.
#[test]
fn mono_hybrid_flame_javelin_paid_with_generic() {
    let decks = Some(vec![
        vec![
            "flame_javelin".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(72, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Six generic (colorless) mana covers three {2/R} pips at two generic each.
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 6,
            ..Default::default()
        },
    );
    let idx = hand_index_for_card(&e, 0, "flame_javelin");
    e.apply_command(0, &cast_spell(idx, target_player(1)))
        .expect("cast Flame Javelin");
    pass_both_players(&mut e);
    assert_eq!(
        e.state.players[1].life, 16,
        "Flame Javelin deals 4 to the targeted player"
    );
}

/// CR 107.4f: Mutagenic Growth ({G/P}) cast by paying 2 life pumps the target +2/+2 and
/// reduces the caster's life by 2 without using any mana.
#[test]
fn phyrexian_mutagenic_growth_paid_with_life() {
    let decks = Some(vec![
        vec![
            "mutagenic_growth".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(73, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let life_before = e.state.players[0].life;
    let idx = hand_index_for_card(&e, 0, "mutagenic_growth");
    // No mana added: pay the {G/P} pip (pip index 0) with 2 life.
    let batch = e
        .apply_command(
            0,
            &cast_spell_flex(
                idx,
                vec![TargetRef { object_id: bear }],
                vec![FlexPipPayment {
                    pip_index: 0,
                    pay_life: true,
                }],
            ),
        )
        .expect("cast Mutagenic Growth paying life");
    assert_eq!(
        e.state.players[0].life,
        life_before - 2,
        "paying Phyrexian mana costs 2 life"
    );
    // The life payment must be surfaced as a LifeChanged event so the client updates the total.
    let life_events = life_changes_in(&batch);
    assert!(
        life_events
            .iter()
            .any(|lc| lc.player_id == 0 && lc.delta == -2 && lc.new_total == life_before - 2),
        "casting with Phyrexian life emits a LifeChanged event: {life_events:?}"
    );
    pass_both_players(&mut e);
    assert_eq!(e.effective_power(bear), Some(4), "2/2 + 2/2 = 4/4");
    assert_eq!(e.effective_toughness(bear), Some(4));
}
