use crate::helpers::*;

#[test]
fn new_with_custom_deck_length() {
    let decks = Some(vec![vec!["mountain".into(); 30], vec!["forest".into(); 30]]);
    let e = GameEngine::new(1, &[0, 1], 20, decks, true).expect("new");
    assert_eq!(
        e.state.players[0].library.len() + e.state.players[0].hand.len(),
        30
    );
}

#[test]
fn play_land_moves_card_from_hand_to_battlefield() {
    let decks = Some(vec![vec!["mountain".into(); 7], vec!["forest".into(); 7]]);
    let mut e = GameEngine::new(7, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let hand_before = e.state.players[0].hand.len();
    let battlefield_before = e.state.players[0].battlefield.len();

    e.apply_command(0, &play_land(0)).expect("play land");

    assert_eq!(e.state.players[0].hand.len(), hand_before - 1);
    assert_eq!(e.state.players[0].battlefield.len(), battlefield_before + 1);
    let mountain = battlefield_object_for_card(&e, 0, "mountain");
    assert_eq!(
        e.state.objects.get(&mountain).expect("mountain").card_id,
        "mountain"
    );
}

#[test]
fn cast_lightning_bolt_resolves_to_graveyard_after_double_pass() {
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
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(13, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");

    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    let pushed = e
        .apply_command(0, &cast_spell(bolt_idx, target_player(1)))
        .expect("cast bolt");
    let bolt_oid = e.state.stack.last().expect("spell on stack").id;
    let stack_push = pushed
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::StackPushed(s)) => Some(s),
            _ => None,
        })
        .expect("stack pushed");
    assert_eq!(stack_push.targets.len(), 1);
    assert_eq!(stack_push.targets[0].object_id, 1);
    // Spells carry their engine card id so the relay can bind the physical stack card
    // through the CardCatalog instead of guessing from the display description.
    assert_eq!(stack_push.card_id, "lightning_bolt");
    // A non-X spell carries no X annotation (the client overlays nothing).
    assert!(stack_push.ability_annotation.is_empty());

    e.apply_command(0, &pass()).expect("caster pass");
    let resolved = e.apply_command(1, &pass()).expect("opponent pass");
    assert!(e.state.stack.is_empty());
    assert!(e.state.players[0].graveyard.contains(&bolt_oid));
    assert!(resolved.events.iter().any(|ev| {
        matches!(
            ev.ev,
            Some(Ev::StackResolved(ref r))
                if r.object_id == bolt_oid
                    && r.destination
                        == tricerules_proto::ruled::v1::StackResolveDestination::Graveyard as i32
        )
    }));
}

/// CR 107.3: Blaze ({X}{R}, "deals X damage to any target") is the canonical single-target
/// X spell. With X=3 it pays 3 generic + {R} and deals 3 to the chosen target.
#[test]
fn blaze_deals_chosen_x_damage_to_target() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "blaze".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(13, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");
    // {X}{R} with X=3 needs 4 mana; the {R} is paid red, the 3 generic from the rest.
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 4,
            ..Default::default()
        },
    );

    let blaze_idx = hand_index_for_card(&e, 0, "blaze");
    let cast = e
        .apply_command(0, &cast_spell_x(blaze_idx, target_player(1), 3))
        .expect("cast blaze with X=3");
    let blaze_oid = e.state.stack.last().expect("spell on stack").id;
    assert_eq!(e.state.stack.last().unwrap().chosen_x, 3);
    assert_eq!(e.state.players[0].mana_pool.red, 0, "X mana fully paid");
    // CR 107.3: the chosen X is surfaced on the stack card as an annotation for the client.
    let blaze_push = cast
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::StackPushed(s)) => Some(s),
            _ => None,
        })
        .expect("blaze stack pushed");
    assert_eq!(blaze_push.ability_annotation, "X = 3");
    assert_eq!(
        blaze_push.card_id, "blaze",
        "spell carries its engine card id"
    );

    e.apply_command(0, &pass()).expect("caster pass");
    e.apply_command(1, &pass()).expect("opponent pass");
    assert!(e.state.stack.is_empty());
    assert!(e.state.players[0].graveyard.contains(&blaze_oid));
    assert_eq!(e.state.players[1].life, 17, "X=3 dealt 3 damage");
}

/// X=0 is a legal choice (CR 107.3); Blaze then deals 0 and the opponent's life is unchanged.
#[test]
fn blaze_x_zero_deals_no_damage() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "blaze".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(13, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );

    let blaze_idx = hand_index_for_card(&e, 0, "blaze");
    let cast = e
        .apply_command(0, &cast_spell_x(blaze_idx, target_player(1), 0))
        .expect("cast blaze with X=0");
    // X=0 is still an X spell, so the stack card is annotated "X = 0".
    let blaze_push = cast
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::StackPushed(s)) => Some(s),
            _ => None,
        })
        .expect("blaze stack pushed");
    assert_eq!(blaze_push.ability_annotation, "X = 0");
    e.apply_command(0, &pass()).expect("caster pass");
    e.apply_command(1, &pass()).expect("opponent pass");
    assert!(e.state.stack.is_empty());
    assert_eq!(e.state.players[1].life, 20, "X=0 dealt no damage");
}

/// Passing an x_value on a spell whose cost has no {X} is rejected (CR 107.3 strictness).
#[test]
fn x_value_on_non_x_spell_rejected() {
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
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(13, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );

    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    let err = e.apply_command(0, &cast_spell_x(bolt_idx, target_player(1), 5));
    assert!(err.is_err(), "x_value on a non-X spell must be rejected");
    // The bolt stays in hand; nothing was paid or pushed.
    assert!(e.state.stack.is_empty());
}

#[test]
fn casting_spell_keeps_priority_with_caster() {
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
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(13, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    let pushed = e
        .apply_command(0, &cast_spell(bolt_idx, target_player(1)))
        .expect("cast bolt");
    assert!(
        priority_changes_in(&pushed).contains(&0),
        "caster should keep priority after casting"
    );
}

#[test]
fn caster_can_cast_second_spell_before_passing_priority() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "mountain".into(),
            "lightning_bolt".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(333, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let mountain_a = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_a))
        .expect("play first mountain");
    // Seed a second untapped mountain to allow casting another bolt while holding priority.
    let mountain_b = hand_index_for_card(&e, 0, "mountain");
    let mountain_b_oid = e.state.players[0].hand.remove(mountain_b);
    e.state.players[0].battlefield.push(mountain_b_oid);
    e.state
        .objects
        .get_mut(&mountain_b_oid)
        .expect("second mountain")
        .zone = tricerules_core::Zone::Battlefield;

    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_one = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_one, target_player(1)))
        .expect("cast first bolt");
    assert_eq!(
        e.state.priority_player_id(),
        0,
        "caster should keep priority after casting first spell"
    );

    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let bolt_two = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_two, target_player(1)))
        .expect("cast second bolt while holding priority");
    assert_eq!(
        e.state.stack.len(),
        2,
        "both spells should be on the stack before any opponent pass"
    );
}

#[test]
fn nonactive_player_cannot_play_land_in_opponents_main() {
    let decks = Some(vec![vec!["mountain".into(); 10], vec!["forest".into(); 10]]);
    let mut e = GameEngine::new(905, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &pass()).expect("active passes");
    assert_eq!(e.state.priority_player_id(), 1);
    let forest_idx = hand_index_for_card(&e, 1, "forest");
    let err = e
        .apply_command(1, &play_land(forest_idx))
        .expect_err("NAP cannot play land during AP main");
    assert!(
        err.to_string().contains("sorcery speed"),
        "unexpected: {err}"
    );
}

#[test]
fn can_cast_new_vanilla_creature_with_swamp() {
    let decks = Some(vec![
        vec![
            "swamp".into(),
            "walking_corpse".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
        ],
        vec![
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(905, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let seeded_swamp_idx = hand_index_for_card(&e, 0, "swamp");
    let seeded_swamp = e.state.players[0].hand.remove(seeded_swamp_idx);
    e.state.players[0].battlefield.push(seeded_swamp);
    e.state
        .objects
        .get_mut(&seeded_swamp)
        .expect("seeded swamp")
        .zone = tricerules_core::Zone::Battlefield;

    let swamp_to_play_idx = hand_index_for_card(&e, 0, "swamp");
    e.apply_command(0, &play_land(swamp_to_play_idx))
        .expect("play second swamp");

    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 1,
            ..Default::default()
        },
    );
    let corpse_idx = hand_index_for_card(&e, 0, "walking_corpse");
    e.apply_command(0, &cast_spell(corpse_idx, vec![]))
        .expect("cast walking corpse");
    let corpse_oid = e.state.stack.first().expect("corpse on stack").id;
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");

    assert!(e.state.players[0].battlefield.contains(&corpse_oid));
}

// ---------------------------------------------------------------------------
// Exploration — extra land per turn (CR 305.2b / layer 5)
// ---------------------------------------------------------------------------

/// With Exploration in play, the active player may play a second land this turn.
#[test]
fn exploration_allows_second_land_per_turn() {
    let decks = Some(vec![
        vec![
            "exploration".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec!["island".into(); 7],
    ]);
    let mut e = GameEngine::new(42, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Cast and resolve Exploration ({G}).
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let exp_idx = hand_index_for_card(&e, 0, "exploration");
    e.apply_command(0, &cast_spell(exp_idx, vec![]))
        .expect("cast exploration");
    resolve_entire_stack_two_player(&mut e);
    assert!(
        e.state.players[0].battlefield.iter().any(|oid| e
            .state
            .objects
            .get(oid)
            .map(|o| o.card_id.as_str())
            == Some("exploration")),
        "Exploration should be on the battlefield"
    );

    // First land: allowed normally.
    let f1 = hand_index_for_card(&e, 0, "forest");
    e.apply_command(0, &play_land(f1))
        .expect("play first forest");
    assert_eq!(e.state.lands_played_this_turn, 1);

    // Second land: allowed because Exploration grants +1 land play.
    let f2 = hand_index_for_card(&e, 0, "forest");
    e.apply_command(0, &play_land(f2))
        .expect("play second forest with Exploration");
    assert_eq!(e.state.lands_played_this_turn, 2);

    // Third land: rejected — max is 2 (1 base + 1 from Exploration).
    let f3 = hand_index_for_card(&e, 0, "forest");
    e.apply_command(0, &play_land(f3))
        .expect_err("third land must be rejected even with Exploration");
}

/// Without Exploration, the second land play is rejected as normal.
#[test]
fn without_exploration_second_land_is_rejected() {
    let decks = Some(vec![vec!["forest".into(); 7], vec!["island".into(); 7]]);
    let mut e = GameEngine::new(43, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let f1 = hand_index_for_card(&e, 0, "forest");
    e.apply_command(0, &play_land(f1))
        .expect("play first forest");

    let f2 = hand_index_for_card(&e, 0, "forest");
    e.apply_command(0, &play_land(f2))
        .expect_err("second land without Exploration must be rejected");
}

/// When Exploration leaves the battlefield, the extra land play is revoked immediately (CR 611.3).
/// After Exploration is bounced back to hand, the second land play is no longer available.
#[test]
fn exploration_leaving_revokes_extra_land_play() {
    // P0: Exploration + Boomerang (to bounce Exploration) + forests + plains (needed for W blue?
    // Boomerang is {1}{U}, so give mana directly in tests).
    let decks = Some(vec![
        vec![
            "exploration".into(),
            "boomerang".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "island".into(),
            "island".into(),
        ],
        vec!["island".into(); 7],
    ]);
    let mut e = GameEngine::new(44, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Cast Exploration.
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let exp_idx = hand_index_for_card(&e, 0, "exploration");
    e.apply_command(0, &cast_spell(exp_idx, vec![]))
        .expect("cast exploration");
    resolve_entire_stack_two_player(&mut e);

    // Play first land.
    let f1 = hand_index_for_card(&e, 0, "forest");
    e.apply_command(0, &play_land(f1))
        .expect("play first forest");
    assert_eq!(e.state.lands_played_this_turn, 1);

    // Bounce Exploration back to hand with Boomerang (targets the Exploration permanent).
    let exploration_oid = battlefield_object_for_card(&e, 0, "exploration");
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );
    let boom_idx = hand_index_for_card(&e, 0, "boomerang");
    e.apply_command(
        0,
        &cast_spell(
            boom_idx,
            vec![TargetRef {
                object_id: exploration_oid,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast boomerang targeting Exploration");
    resolve_entire_stack_two_player(&mut e);

    // Exploration is now in P0's hand; its continuous effect is drained.
    assert!(
        e.state.players[0].battlefield.iter().all(|oid| e
            .state
            .objects
            .get(oid)
            .map(|o| o.card_id.as_str())
            != Some("exploration")),
        "Exploration should be back in hand"
    );
    assert!(
        e.state.continuous_effects.is_empty(),
        "ExtraLandPlays effect must be drained after Exploration leaves"
    );

    // Second land play is now rejected (max is 1 again).
    let f2 = hand_index_for_card(&e, 0, "forest");
    e.apply_command(0, &play_land(f2))
        .expect_err("second land must be rejected after Exploration leaves");
}

/// Two Explorations in play: the player may play 3 lands this turn (1 base + 2 extra).
#[test]
fn two_explorations_allow_three_lands_per_turn() {
    let decks = Some(vec![
        vec![
            "exploration".into(),
            "exploration".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec!["island".into(); 7],
    ]);
    let mut e = GameEngine::new(45, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Cast both Explorations.
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 2,
            ..Default::default()
        },
    );
    let exp1_idx = hand_index_for_card(&e, 0, "exploration");
    e.apply_command(0, &cast_spell(exp1_idx, vec![]))
        .expect("cast first exploration");
    resolve_entire_stack_two_player(&mut e);

    let exp2_idx = hand_index_for_card(&e, 0, "exploration");
    e.apply_command(0, &cast_spell(exp2_idx, vec![]))
        .expect("cast second exploration");
    resolve_entire_stack_two_player(&mut e);

    // Play three lands — all allowed.
    let f1 = hand_index_for_card(&e, 0, "forest");
    e.apply_command(0, &play_land(f1))
        .expect("play land 1 of 3");
    let f2 = hand_index_for_card(&e, 0, "forest");
    e.apply_command(0, &play_land(f2))
        .expect("play land 2 of 3");
    let f3 = hand_index_for_card(&e, 0, "forest");
    e.apply_command(0, &play_land(f3))
        .expect("play land 3 of 3");
    assert_eq!(e.state.lands_played_this_turn, 3);

    // Fourth land is rejected (max is 3).
    let f4 = hand_index_for_card(&e, 0, "forest");
    e.apply_command(0, &play_land(f4))
        .expect_err("fourth land must be rejected with two Explorations");
}
