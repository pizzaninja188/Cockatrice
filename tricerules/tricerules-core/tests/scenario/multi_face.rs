use crate::helpers::*;

/// CR 709: Fire // Ice is a split card. Each half is an independently castable instant chosen by
/// `CastSpell.face_index`. Casting face 0 (Fire) deals 2 damage and shows the half's own name.
#[test]
fn fire_ice_fire_half_deals_two_and_shows_face_name() {
    let decks = Some(vec![
        vec![
            "fire_ice".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(21, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // {1}{R}: two red pays the colored pip and the generic.
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 2,
            ..Default::default()
        },
    );

    let idx = hand_index_for_card(&e, 0, "fire_ice");
    let pushed = e
        .apply_command(0, &cast_spell_face(idx, target_player(1), 0))
        .expect("cast Fire");
    let spell_oid = e.state.stack.last().expect("spell on stack").id;
    let push = pushed
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::StackPushed(s)) => Some(s),
            _ => None,
        })
        .expect("stack pushed");
    // The cast half's own name is on the stack card; the card id is the whole-card id.
    assert_eq!(push.description, "Fire");
    assert_eq!(push.card_id, "fire_ice");
    // CR 709: a multi-face spell's stack card is annotated with the cast face name so the player
    // sees which half resolves (the physical card still shows "Fire // Ice").
    assert_eq!(push.ability_annotation, "Fire");

    e.apply_command(0, &pass()).expect("caster pass");
    e.apply_command(1, &pass()).expect("opponent pass");
    assert!(e.state.stack.is_empty());
    assert_eq!(e.state.players[1].life, 18, "Fire deals 2 to the player");
    // An instant half resolves to its owner's graveyard, not the battlefield.
    assert!(e.state.players[0].graveyard.contains(&spell_oid));
}

/// CR 709: casting the other half (Ice, face 1) of the same split card taps a target permanent
/// and draws — different cost, effect, and name from Fire, resolved from the same physical card.
#[test]
fn fire_ice_ice_half_taps_and_draws() {
    let mut p0_deck: Vec<String> = vec!["fire_ice".into(), "mountain".into()];
    p0_deck.extend(std::iter::repeat_n("island".to_string(), 10));
    let decks = Some(vec![p0_deck, vec!["forest".into(); 12]]);
    let mut e = GameEngine::new(22, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Place the card and an untapped land deterministically (shuffle-independent).
    let card_oid = relocate_to_hand(&mut e, 0, "fire_ice");
    let land_oid = relocate_to_battlefield(&mut e, 0, "mountain", false);
    let idx = e.state.players[0]
        .hand
        .iter()
        .position(|&o| o == card_oid)
        .expect("fire_ice in hand");
    assert!(!e.state.objects.get(&land_oid).unwrap().tapped);

    // {1}{U}: two blue pays the colored pip and the generic.
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 2,
            ..Default::default()
        },
    );

    let lib_before = e.state.players[0].library.len();
    let pushed = e
        .apply_command(
            0,
            &cast_spell_face(
                idx,
                vec![TargetRef {
                    object_id: land_oid,
                }],
                1,
            ),
        )
        .expect("cast Ice");
    let push = pushed
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::StackPushed(s)) => Some(s),
            _ => None,
        })
        .expect("stack pushed");
    assert_eq!(push.description, "Ice");
    assert_eq!(push.ability_annotation, "Ice");

    e.apply_command(0, &pass()).expect("caster pass");
    e.apply_command(1, &pass()).expect("opponent pass");
    assert!(e.state.stack.is_empty());
    assert!(
        e.state.objects.get(&land_oid).unwrap().tapped,
        "Ice taps the target permanent"
    );
    assert_eq!(
        e.state.players[0].library.len(),
        lib_before - 1,
        "Ice draws a card"
    );
}

// ---------------------------------------------------------------------------
// MDFC Pathway lands (CR 712): choose one face on play; that face's abilities apply.
// ---------------------------------------------------------------------------

/// CR 712.2c / 712.4: a Modal DFC land played as face 0 (Cragcrown Pathway) enters the
/// battlefield with face_up_index = 0 and exposes only that face's activated ability
/// ({T}: Add {R}). Tapping it produces red, not green.
#[test]
fn mdfc_pathway_enter_as_face_0_taps_for_red() {
    let decks = Some(vec![
        deck_with("mountain", &["cragcrown_pathway_timbercrown_pathway"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(40, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let card_oid = relocate_to_hand(&mut e, 0, "cragcrown_pathway_timbercrown_pathway");
    let idx = e.state.players[0]
        .hand
        .iter()
        .position(|&o| o == card_oid)
        .expect("pathway in hand");

    e.apply_command(0, &play_land_face(idx, 0))
        .expect("play as Cragcrown (face 0)");
    let land_oid = *e.state.players[0]
        .battlefield
        .last()
        .expect("land on battlefield");

    assert_eq!(
        e.state.objects[&land_oid].face_up_index, 0,
        "enters as face 0"
    );

    e.apply_command(0, &activate_ability(land_oid, 0, vec![]))
        .expect("tap for R");
    assert_eq!(e.state.players[0].mana_pool.red, 1, "produced {{R}}");
    assert_eq!(
        e.state.players[0].mana_pool.green, 0,
        "no {{G}} from face 0"
    );
    assert!(e.state.objects[&land_oid].tapped, "land tapped as cost");
}

/// CR 712.2c / 712.4: the same MDFC card played as face 1 (Timbercrown Pathway) enters with
/// face_up_index = 1 and exposes only that face's ability ({T}: Add {G}).
#[test]
fn mdfc_pathway_enter_as_face_1_taps_for_green() {
    let decks = Some(vec![
        deck_with("forest", &["cragcrown_pathway_timbercrown_pathway"]),
        vec!["mountain".into(); 20],
    ]);
    let mut e = GameEngine::new(41, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let card_oid = relocate_to_hand(&mut e, 0, "cragcrown_pathway_timbercrown_pathway");
    let idx = e.state.players[0]
        .hand
        .iter()
        .position(|&o| o == card_oid)
        .expect("pathway in hand");

    e.apply_command(0, &play_land_face(idx, 1))
        .expect("play as Timbercrown (face 1)");
    let land_oid = *e.state.players[0]
        .battlefield
        .last()
        .expect("land on battlefield");

    assert_eq!(
        e.state.objects[&land_oid].face_up_index, 1,
        "enters as face 1"
    );

    e.apply_command(0, &activate_ability(land_oid, 0, vec![]))
        .expect("tap for G");
    assert_eq!(e.state.players[0].mana_pool.green, 1, "produced {{G}}");
    assert_eq!(e.state.players[0].mana_pool.red, 0, "no {{R}} from face 1");
    assert!(e.state.objects[&land_oid].tapped, "land tapped as cost");
}

/// CR 712.2c: Modal DFCs are not double-faced cards that transform; TransformPermanent on a
/// ModalDfc permanent is illegal (only Transform/Flip layouts may transform).
#[test]
fn transform_permanent_rejected_for_mdfc_land() {
    let decks = Some(vec![
        deck_with("mountain", &["cragcrown_pathway_timbercrown_pathway"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(42, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let land_oid =
        inject_permanent_on_battlefield(&mut e, 0, "cragcrown_pathway_timbercrown_pathway");

    let result = e.apply_command(0, &transform_permanent_cmd(land_oid));
    assert!(
        result.is_err(),
        "TransformPermanent on a ModalDfc card must be rejected"
    );
}

/// CR 709/115.4: each half of a split card offers only its own legal targets. Fire targets "any
/// target" (creatures/players, never lands); Ice targets "any permanent" (including lands). The
/// engine emits per-face target sets keyed by (hand_slot << 8 | face_index) so the UI cannot offer
/// a land for Fire and then waste mana on an illegal cast the engine would reject.
#[test]
fn fire_ice_target_sets_are_per_face() {
    let decks = Some(vec![
        deck_with("mountain", &["fire_ice", "grizzly_bears"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(23, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // A creature (legal for both halves) and a land (legal for Ice only) on the battlefield.
    let bears = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);
    let land = relocate_to_battlefield(&mut e, 0, "mountain", false);
    let card_oid = relocate_to_hand(&mut e, 0, "fire_ice");
    let slot = e.state.players[0]
        .hand
        .iter()
        .position(|&o| o == card_oid)
        .expect("fire_ice in hand") as u32;

    // fill_legal emits a target set for every targeting face regardless of affordability, so no
    // mana is needed; read the legal actions for the current state directly.
    let batch = e.initial_response_batch();

    let legal = batch.legal_by_player.get(&0).expect("legal for P0");
    let fire_key = slot << 8; // face 0
    let ice_key = (slot << 8) | 1; // face 1
    let fire = legal
        .valid_targets_by_hand_slot
        .get(&fire_key)
        .expect("Fire face target set");
    let ice = legal
        .valid_targets_by_hand_slot
        .get(&ice_key)
        .expect("Ice face target set");

    assert!(
        fire.valid_permanent_ids.contains(&bears),
        "Fire can target a creature"
    );
    assert!(
        !fire.valid_permanent_ids.contains(&land),
        "Fire cannot target a land (any target excludes lands)"
    );
    assert!(
        fire.can_target_opponent,
        "Fire can target a player (any target)"
    );

    assert!(
        ice.valid_permanent_ids.contains(&bears),
        "Ice can target a creature permanent"
    );
    assert!(
        ice.valid_permanent_ids.contains(&land),
        "Ice can target a land permanent"
    );
    assert!(
        !ice.can_target_opponent,
        "Ice targets a permanent, not a player"
    );
}
