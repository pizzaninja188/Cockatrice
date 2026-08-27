use crate::helpers::*;

#[test]
fn issue_174_fae_court_draws_then_creates_a_restricted_faerie_and_clone_keeps_it() {
    let decks = Some(vec![
        deck_with("island", &["into_the_fae_court", "clone"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(174_008, &[0, 1], 20, decks, true).unwrap();
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "into_the_fae_court");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "into_the_fae_court");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    let before_draw = engine.state.players[0].hand.len();
    engine.apply_command(0, &pass()).unwrap();
    let batch = engine.apply_command(1, &pass()).unwrap();
    assert_eq!(engine.state.players[0].hand.len(), before_draw + 3);
    let created = token_created_events(&batch);
    assert_eq!(created.len(), 1);
    let faerie = created[0].object_id;
    let identity = created[0].identity.as_ref().unwrap();
    assert_eq!(
        (&*identity.name, &*identity.pt, &*identity.color),
        ("Faerie", "1/1", "u")
    );
    assert_eq!(identity.keywords, ["Flying"]);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, faerie),
        ["Can't block creatures without flying"]
    );
    ensure_in_hand(&mut engine, 0, "clone");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "clone");
    engine.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![faerie]))
        .unwrap();
    let clone = battlefield_object_for_card(&engine, 0, "clone");
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, clone),
        ["Can't block creatures without flying"]
    );
    end_active_turn(&mut engine, 0);
    advance_to_main1_from_game_start(&mut engine);
    let flying = inject_creature_on_battlefield(&mut engine, 1, "storm_crow");
    let reach = inject_creature_on_battlefield(&mut engine, 1, "giant_spider");
    let ground = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    engine.apply_command(1, &primitive_yield()).unwrap();
    pass_both_players(&mut engine);
    engine
        .apply_command(1, &declare_attackers(vec![flying, reach, ground]))
        .unwrap();
    pass_both_players(&mut engine);
    let legal = &engine.initial_response_batch().legal_by_player[&0];
    for blocker in [faerie, clone] {
        assert_eq!(
            legal
                .legal_block_pairs
                .iter()
                .filter(|pair| pair.blocker_id == blocker)
                .map(|pair| pair.attacker_id)
                .collect::<Vec<_>>(),
            [flying]
        );
    }
    assert!(engine
        .apply_command(
            0,
            &declare_blockers(vec![BlockPair {
                attacker_id: reach,
                blocker_id: faerie
            }])
        )
        .is_err());
    engine
        .apply_command(
            0,
            &declare_blockers(vec![BlockPair {
                attacker_id: flying,
                blocker_id: faerie,
            }]),
        )
        .unwrap();
}

/// Raise the Alarm makes exactly two 1/1 white Soldier tokens under the caster, summoning-sick,
/// on the battlefield, and emits a self-describing TokenCreated for each (CR 111.1/111.4).
#[test]
fn raise_the_alarm_creates_two_soldier_tokens() {
    let decks = Some(vec![
        vec![
            "raise_the_alarm".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(21, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    grant_pool(&mut e, 0);
    let idx = hand_index_for_card(&e, 0, "raise_the_alarm");
    e.apply_command(0, &cast_spell(idx, vec![]))
        .expect("cast raise the alarm");
    e.apply_command(0, &pass()).expect("caster pass");
    let resolved = e.apply_command(1, &pass()).expect("opponent pass");

    let soldiers = battlefield_token_oids(&e, 0, "soldier_w_1_1");
    assert_eq!(soldiers.len(), 2, "two soldier tokens created");
    for oid in &soldiers {
        let o = e.state.objects.get(oid).expect("token object");
        assert_eq!(o.owner, 0, "token controlled by caster");
        assert_eq!(o.zone, tricerules_core::Zone::Battlefield);
        assert_eq!((o.power, o.toughness), (Some(1), Some(1)), "1/1");
        assert!(o.summoning_sick, "entering token is summoning sick");
        assert!(!o.tapped, "ordinary tokens remain untapped by default");
    }
    // P1 received no tokens (Controller, not EachPlayer).
    assert!(battlefield_token_oids(&e, 1, "soldier_w_1_1").is_empty());

    let created = token_created_events(&resolved);
    assert_eq!(created.len(), 2, "one TokenCreated per token");
    for tc in &created {
        assert_eq!(tc.controller_player_id, 0);
        assert_eq!(tc.card_id, "soldier_w_1_1");
        let id = tc.identity.as_ref().expect("identity");
        assert_eq!(id.name, "Soldier");
        assert_eq!(id.pt, "1/1");
        assert_eq!(id.color, "w");
        assert!(id.is_creature);
        assert!(!tc.enters_tapped, "default token event remains untapped");
        // Vanilla token: no keyword abilities feed to the client art matcher.
        assert!(id.keywords.is_empty());
    }
}

/// A keyword-bearing token (Call the Cavalry → 2/2 white Knight with vigilance) feeds its keyword
/// abilities through TokenIdentity so the client can pick the matching Oracle token art among
/// same-name/P/T variants (vanilla vs. vigilance Knight).
#[test]
fn call_the_cavalry_token_identity_carries_keywords() {
    let decks = Some(vec![
        vec!["call_the_cavalry".into(); 7],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(22, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    grant_pool(&mut e, 0);
    let idx = hand_index_for_card(&e, 0, "call_the_cavalry");
    e.apply_command(0, &cast_spell(idx, vec![]))
        .expect("cast call the cavalry");
    e.apply_command(0, &pass()).expect("caster pass");
    let resolved = e.apply_command(1, &pass()).expect("opponent pass");

    let created = token_created_events(&resolved);
    assert_eq!(created.len(), 2, "two knight tokens created");
    for tc in &created {
        assert_eq!(tc.card_id, "knight_w_2_2_vigilance");
        let id = tc.identity.as_ref().expect("identity");
        assert_eq!(id.name, "Knight");
        assert_eq!(id.pt, "2/2");
        assert_eq!(id.color, "w");
        // The keyword feed uses canonical MTG spelling (Keyword::as_str).
        assert_eq!(id.keywords, vec!["Vigilance".to_string()]);
    }
}

/// Goblin Wizardry creates two independent 1/1 red Goblin Wizard tokens whose prowess ability is
/// both mechanically active and included in the public token identity used by the client.
#[test]
fn goblin_wizardry_tokens_carry_and_trigger_prowess() {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &["goblin_wizardry", "lightning_bolt", "grizzly_bears"],
        ),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(66, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    ensure_in_hand(&mut e, 0, "goblin_wizardry");
    grant_pool(&mut e, 0);
    let idx = hand_index_for_card(&e, 0, "goblin_wizardry");
    e.apply_command(0, &cast_spell(idx, vec![]))
        .expect("cast Goblin Wizardry");
    e.apply_command(0, &pass()).expect("caster pass");
    let resolved = e.apply_command(1, &pass()).expect("opponent pass");

    let wizards = battlefield_token_oids(&e, 0, "goblin_wizard_r_1_1_prowess");
    assert_eq!(wizards.len(), 2);
    for oid in &wizards {
        assert_eq!(e.effective_power(*oid), Some(1));
        assert_eq!(e.effective_toughness(*oid), Some(1));
    }
    let created = token_created_events(&resolved);
    assert_eq!(created.len(), 2);
    for event in created {
        let identity = event.identity.as_ref().expect("token identity");
        assert_eq!(identity.name, "Goblin Wizard");
        assert_eq!(identity.pt, "1/1");
        assert_eq!(identity.color, "r");
        assert_eq!(
            identity.triggered_ability_texts,
            vec!["Prowess (Whenever you cast a noncreature spell, this creature gets +1/+1 until end of turn.)"]
        );
    }

    ensure_in_hand(&mut e, 0, "lightning_bolt");
    grant_pool(&mut e, 0);
    let bolt = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt, target_player(1)))
        .expect("cast Lightning Bolt");
    let prowess_order = e
        .state
        .pending_trigger_order
        .as_ref()
        .expect("the two prowess triggers need a deterministic controller-chosen order");
    assert_eq!(prowess_order.candidates.len(), 2);
    let mut prowess_sources: Vec<_> = prowess_order
        .candidates
        .iter()
        .map(|trigger| trigger.source_permanent_id)
        .collect();
    prowess_sources.sort_unstable();
    let mut expected_sources = wizards.clone();
    expected_sources.sort_unstable();
    assert_eq!(
        prowess_sources, expected_sources,
        "one prowess trigger per token"
    );
    resolve_entire_stack_two_player(&mut e);
    for oid in &wizards {
        assert_eq!(e.effective_power(*oid), Some(2));
        assert_eq!(e.effective_toughness(*oid), Some(2));
    }

    ensure_in_hand(&mut e, 0, "grizzly_bears");
    grant_pool(&mut e, 0);
    let bears = hand_index_for_card(&e, 0, "grizzly_bears");
    e.apply_command(0, &cast_spell(bears, vec![]))
        .expect("cast creature spell");
    assert!(
        e.state.pending_triggers.is_empty() && e.state.pending_trigger_order.is_none(),
        "prowess must not trigger for a creature spell"
    );
}

/// One spell with several CreateTokens effects (Bestial Menace) mints each distinct token type
/// from its own registry definition — proving the ordered `spell_effect` Vec resolves tokens of
/// different characteristics in a single resolution.
#[test]
fn bestial_menace_creates_three_distinct_tokens() {
    let decks = Some(vec![
        vec![
            "bestial_menace".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(25, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    grant_pool(&mut e, 0);
    let idx = hand_index_for_card(&e, 0, "bestial_menace");
    e.apply_command(0, &cast_spell(idx, vec![])).expect("cast");
    e.apply_command(0, &pass()).expect("pass");
    e.apply_command(1, &pass()).expect("pass");

    let snake = battlefield_token_oids(&e, 0, "snake_g_1_1");
    let wolf = battlefield_token_oids(&e, 0, "wolf_g_2_2");
    let elephant = battlefield_token_oids(&e, 0, "elephant_g_3_3");
    assert_eq!(snake.len(), 1);
    assert_eq!(wolf.len(), 1);
    assert_eq!(elephant.len(), 1);
    assert_eq!(
        e.state.objects[&snake[0]]
            .power
            .zip(e.state.objects[&snake[0]].toughness),
        Some((1, 1))
    );
    assert_eq!(
        e.state.objects[&wolf[0]]
            .power
            .zip(e.state.objects[&wolf[0]].toughness),
        Some((2, 2))
    );
    assert_eq!(
        e.state.objects[&elephant[0]]
            .power
            .zip(e.state.objects[&elephant[0]].toughness),
        Some((3, 3))
    );
}

/// CR 111.7/704.5: a token that dies leaves the battlefield, then ceases to exist as an SBA —
/// the object is gone entirely (not stranded in the graveyard). Other tokens are unaffected.
#[test]
fn token_dies_and_ceases_to_exist() {
    let decks = Some(vec![
        vec![
            "raise_the_alarm".into(),
            "lightning_bolt".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(22, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    grant_pool(&mut e, 0);
    let idx = hand_index_for_card(&e, 0, "raise_the_alarm");
    e.apply_command(0, &cast_spell(idx, vec![])).expect("cast");
    e.apply_command(0, &pass()).expect("pass");
    e.apply_command(1, &pass()).expect("pass");

    let soldiers = battlefield_token_oids(&e, 0, "soldier_w_1_1");
    assert_eq!(soldiers.len(), 2);
    let victim = soldiers[0];
    let survivor = soldiers[1];

    grant_pool(&mut e, 0);
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(
        0,
        &cast_spell(
            bolt_idx,
            vec![TargetRef {
                object_id: victim,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
            }],
        ),
    )
    .expect("bolt the token");
    e.apply_command(0, &pass()).expect("pass");
    e.apply_command(1, &pass()).expect("pass");

    // CR 111.7: the dead token object no longer exists in any zone.
    assert!(
        !e.state.objects.contains_key(&victim),
        "dead token ceased to exist"
    );
    assert_eq!(
        e.state.turn_history.current.creatures_died, 1,
        "a creature token still counts as having died before it ceases to exist"
    );
    assert!(
        !e.state.players[0].graveyard.contains(&victim),
        "token must not linger in the graveyard"
    );
    // The other token is untouched.
    assert!(e.state.objects.contains_key(&survivor));
    assert_eq!(
        battlefield_token_oids(&e, 0, "soldier_w_1_1"),
        vec![survivor]
    );
}

/// CR 111.7: a token returned to its owner's hand ceases to exist rather than becoming a hand
/// card — it must not show up in the hand zone afterward.
#[test]
fn bounced_token_ceases_to_exist() {
    let decks = Some(vec![
        vec![
            "raise_the_alarm".into(),
            "unsummon".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(23, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    grant_pool(&mut e, 0);
    let idx = hand_index_for_card(&e, 0, "raise_the_alarm");
    e.apply_command(0, &cast_spell(idx, vec![])).expect("cast");
    e.apply_command(0, &pass()).expect("pass");
    e.apply_command(1, &pass()).expect("pass");

    let victim = battlefield_token_oids(&e, 0, "soldier_w_1_1")[0];
    let hand_before = e.state.players[0].hand.len();

    grant_pool(&mut e, 0);
    let uns_idx = hand_index_for_card(&e, 0, "unsummon");
    e.apply_command(
        0,
        &cast_spell(
            uns_idx,
            vec![TargetRef {
                object_id: victim,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
            }],
        ),
    )
    .expect("unsummon the token");
    e.apply_command(0, &pass()).expect("pass");
    e.apply_command(1, &pass()).expect("pass");

    assert!(
        !e.state.objects.contains_key(&victim),
        "bounced token ceased to exist"
    );
    assert!(
        !e.state.players[0].hand.contains(&victim),
        "token must not enter the hand"
    );
    // Unsummon itself left hand (cast) and the token didn't join it.
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before - 1,
        "only the cast spell left the hand"
    );
}

/// An anthem ("creatures you control get +1/+1") modeled as an AllCreatures continuous effect
/// buffs a token through the same base-P/T + continuous-effects path used for cards — proving
/// the engine never special-cases token-ness for characteristic queries.
#[test]
fn anthem_buffs_token_via_shared_pt_path() {
    use tricerules_cards::primitives::{ContinuousEffectKind, EffectDuration};

    let decks = Some(vec![
        vec![
            "raise_the_alarm".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(24, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    grant_pool(&mut e, 0);
    let idx = hand_index_for_card(&e, 0, "raise_the_alarm");
    e.apply_command(0, &cast_spell(idx, vec![])).expect("cast");
    e.apply_command(0, &pass()).expect("pass");
    e.apply_command(1, &pass()).expect("pass");

    let token = battlefield_token_oids(&e, 0, "soldier_w_1_1")[0];
    assert_eq!(e.effective_power(token), Some(1));
    assert_eq!(e.effective_toughness(token), Some(1));

    e.state
        .continuous_effects
        .push(tricerules_core::ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: tricerules_core::AffectedScope::AllCreatures,
            kind: ContinuousEffectKind::PtModify {
                delta_power: 1,
                delta_toughness: 1,
            },
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 0,
        });

    assert_eq!(
        e.effective_power(token),
        Some(2),
        "anthem buffs token power"
    );
    assert_eq!(
        e.effective_toughness(token),
        Some(2),
        "anthem buffs token toughness"
    );
}
