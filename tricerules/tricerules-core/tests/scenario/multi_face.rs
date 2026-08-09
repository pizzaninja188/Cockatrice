use crate::helpers::*;
use tricerules_proto::ruled::v1 as rv1;
use tricerules_proto::ruled::v1::dev_command::Dev;
use tricerules_proto::ruled::v1::{DevCommand, DevMoveCard, DevZone};

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
    // Fire is a DamageTargets effect; damage_amount must be specified (all 2 to player 1).
    let pushed = e
        .apply_command(0, &cast_spell_face(idx, target_player_damage(1, 2), 0))
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
                    damage_amount: 0,
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
    let face_actions: Vec<_> = legal
        .hand_actions
        .iter()
        .filter(|action| action.hand_index == slot)
        .collect();
    assert_eq!(face_actions.len(), 2, "one structured cast action per face");
    assert_eq!(
        face_actions[0].kind,
        rv1::HandActionKind::HandActionCastSpell as i32
    );
    assert_eq!(face_actions[0].face_index, 0);
    assert_eq!(face_actions[0].card_name, "Fire");
    assert!(face_actions[0].needs_target);
    assert_eq!(
        face_actions[1].kind,
        rv1::HandActionKind::HandActionCastSpell as i32
    );
    assert_eq!(face_actions[1].face_index, 1);
    assert_eq!(face_actions[1].card_name, "Ice");
    assert!(face_actions[1].needs_target);
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

// ---------------------------------------------------------------------------
// Adventure (CR 715): resolve the Adventure half into exile, then cast the permanent face.
// ---------------------------------------------------------------------------

/// CR 715.3d: an Adventure spell that resolves is exiled instead of going to its owner's
/// graveyard. The later cast permission is covered separately once the source-zone command is
/// wired; this regression first pins the destination decision that creates that permission.
#[test]
fn stomp_resolves_to_exile() {
    let decks = Some(vec![
        deck_with("mountain", &["bonecrusher_giant_stomp"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(43, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let card_oid = relocate_to_hand(&mut e, 0, "bonecrusher_giant_stomp");
    let hand_index = e.state.players[0]
        .hand
        .iter()
        .position(|&oid| oid == card_oid)
        .expect("Bonecrusher Giant in hand");
    let hand_legal = e.initial_response_batch();
    let offered_faces: Vec<_> = hand_legal.legal_by_player[&0]
        .hand_actions
        .iter()
        .filter(|action| action.hand_index == hand_index as u32)
        .map(|action| {
            (
                action.face_index,
                action.card_name.as_str(),
                action.cost.as_str(),
            )
        })
        .collect();
    assert_eq!(
        offered_faces,
        vec![(0, "Bonecrusher Giant", "{2}{R}"), (1, "Stomp", "{1}{R}"),],
        "both hand-cast faces carry their engine names and costs"
    );
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 2,
            ..Default::default()
        },
    );

    e.apply_command(0, &cast_spell_face(hand_index, target_player(1), 1))
        .expect("cast Stomp");
    e.apply_command(0, &pass()).expect("caster pass");
    e.apply_command(1, &pass()).expect("opponent pass");

    assert!(
        e.state.players[0].exile.contains(&card_oid),
        "a successfully resolved Adventure spell is exiled"
    );
    assert!(
        !e.state.players[0].graveyard.contains(&card_oid),
        "a successfully resolved Adventure spell does not go to the graveyard"
    );
}

#[test]
fn bonecrusher_giant_casts_normally_from_hand() {
    let decks = Some(vec![
        deck_with("mountain", &["bonecrusher_giant_stomp"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(49, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let oid = relocate_to_hand(&mut e, 0, "bonecrusher_giant_stomp");
    let slot = e.state.players[0]
        .hand
        .iter()
        .position(|&candidate| candidate == oid)
        .unwrap();
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 3,
            ..Default::default()
        },
    );
    let pushed = e
        .apply_command(0, &cast_spell_face(slot, vec![], 0))
        .expect("cast Bonecrusher Giant from hand");
    let stack_push = pushed
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(push)) => Some(push),
            _ => None,
        })
        .expect("stack push");
    assert_eq!(
        stack_push.ability_annotation, "Bonecrusher Giant",
        "two castable hand faces still identify the chosen face"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&oid].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(e.state.objects[&oid].face_up_index, 0);
}

#[test]
fn temporarily_sole_hand_cast_face_has_no_annotation() {
    let decks = Some(vec![
        deck_with("mountain", &["bonecrusher_giant_stomp"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(50, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let oid = relocate_to_hand(&mut e, 0, "bonecrusher_giant_stomp");
    let slot = e.state.players[0]
        .hand
        .iter()
        .position(|&candidate| candidate == oid)
        .unwrap();
    e.state.turn_step = tricerules_core::TurnStep::Upkeep;
    let legal = e.initial_response_batch();
    let actions: Vec<_> = legal.legal_by_player[&0]
        .hand_actions
        .iter()
        .filter(|action| {
            action.hand_index == slot as u32
                && action.kind == rv1::HandActionKind::HandActionCastSpell as i32
        })
        .collect();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].card_name, "Stomp");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 2,
            ..Default::default()
        },
    );

    let pushed = e
        .apply_command(0, &cast_spell_face(slot, target_player(1), 1))
        .expect("cast the only currently available face");
    let stack_push = pushed
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(push)) => Some(push),
            _ => None,
        })
        .expect("stack push");
    assert!(stack_push.ability_annotation.is_empty());
}

/// CR 715.3d: the same object may be cast from exile only as its permanent face, and moving it to
/// the stack consumes the permission. LegalActions supplies the exact object, face, and cost.
#[test]
fn bonecrusher_giant_casts_once_from_adventure_exile() {
    let decks = Some(vec![
        deck_with("mountain", &["bonecrusher_giant_stomp"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(44, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let oid = relocate_to_hand(&mut e, 0, "bonecrusher_giant_stomp");
    let hand_index = e.state.players[0]
        .hand
        .iter()
        .position(|&candidate| candidate == oid)
        .unwrap();
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 5,
            ..Default::default()
        },
    );
    e.apply_command(0, &cast_spell_face(hand_index, target_player(1), 1))
        .expect("cast Stomp");
    e.apply_command(0, &pass()).unwrap();
    e.apply_command(1, &pass()).unwrap();

    let legal = e.initial_response_batch();
    let actions = &legal.legal_by_player[&0].zone_cast_actions;
    assert_eq!(actions.len(), 1, "one Adventure exile action");
    assert_eq!(actions[0].source_zone, rv1::CastSourceZone::Exile as i32);
    assert_eq!(actions[0].object_id, oid);
    assert_eq!(actions[0].face_index, 0);
    assert_eq!(actions[0].card_name, "Bonecrusher Giant");
    assert_eq!(actions[0].cost, "{2}{R}");

    let cast = RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            source: Some(exile_cast_source(oid)),
            face_index: 0,
            ..Default::default()
        })),
    };
    let pushed = e.apply_command(0, &cast).expect("cast creature from exile");
    let stack_push = pushed
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(push)) => Some(push),
            _ => None,
        })
        .expect("stack push");
    assert!(
        stack_push.ability_annotation.is_empty(),
        "the sole legal exile cast needs no face annotation"
    );
    assert_eq!(e.state.objects[&oid].zone, tricerules_core::Zone::Stack);
    assert!(e.state.objects[&oid].adventure_cast_permission.is_none());
    e.apply_command(0, &pass()).unwrap();
    e.apply_command(1, &pass()).unwrap();
    assert_eq!(
        e.state.objects[&oid].zone,
        tricerules_core::Zone::Battlefield
    );
    assert_eq!(e.state.objects[&oid].face_up_index, 0);
    assert!(e.initial_response_batch().legal_by_player[&0]
        .zone_cast_actions
        .is_empty());
}

#[test]
fn adventure_exile_permission_rejects_wrong_source_player_face_and_unpaid_cost() {
    let decks = Some(vec![
        deck_with("mountain", &["bonecrusher_giant_stomp"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(45, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let oid = relocate_to_hand(&mut e, 0, "bonecrusher_giant_stomp");
    let slot = e.state.players[0]
        .hand
        .iter()
        .position(|&candidate| candidate == oid)
        .unwrap();
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 2,
            ..Default::default()
        },
    );
    e.apply_command(0, &cast_spell_face(slot, target_player(1), 1))
        .unwrap();
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&oid].adventure_cast_permission.is_some());

    let exile_cast = |object_id, face_index| RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            source: Some(exile_cast_source(object_id)),
            face_index,
            ..Default::default()
        })),
    };
    assert!(e.apply_command(0, &exile_cast(oid, 1)).is_err());
    assert!(e.apply_command(1, &exile_cast(oid, 0)).is_err());
    assert!(e.apply_command(0, &exile_cast(u32::MAX, 0)).is_err());
    assert!(e
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CastSpell(CastSpell {
                    source: None,
                    face_index: 0,
                    ..Default::default()
                })),
            },
        )
        .is_err());
    assert!(
        e.apply_command(0, &exile_cast(oid, 0)).is_err(),
        "ordinary mana payment is still required"
    );
    e.state.turn_step = tricerules_core::TurnStep::Upkeep;
    assert!(e.initial_response_batch().legal_by_player[&0]
        .zone_cast_actions
        .is_empty());
    assert!(
        e.apply_command(0, &exile_cast(oid, 0)).is_err(),
        "the permanent face keeps its ordinary sorcery timing"
    );
    assert_eq!(e.state.objects[&oid].zone, tricerules_core::Zone::Exile);
    assert!(
        e.state.objects[&oid].adventure_cast_permission.is_some(),
        "rejected casts do not consume permission"
    );
}

#[test]
fn adventure_permission_does_not_return_when_the_card_leaves_and_reenters_exile() {
    let decks = Some(vec![
        deck_with("mountain", &["bonecrusher_giant_stomp"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(48, &[0, 1], 20, decks, true).expect("new");
    e.enable_dev_commands();
    advance_to_main1_from_game_start(&mut e);
    let oid = relocate_to_hand(&mut e, 0, "bonecrusher_giant_stomp");
    let slot = e.state.players[0]
        .hand
        .iter()
        .position(|&candidate| candidate == oid)
        .unwrap();
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 2,
            ..Default::default()
        },
    );
    e.apply_command(0, &cast_spell_face(slot, target_player(1), 1))
        .unwrap();
    resolve_entire_stack_two_player(&mut e);

    let move_to = |zone| RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: 0,
            dev: Some(Dev::MoveCard(DevMoveCard {
                card_name: "Bonecrusher Giant // Stomp".to_string(),
                zone: zone as i32,
                ready: false,
            })),
        })),
    };
    e.apply_command(0, &move_to(DevZone::Hand)).unwrap();
    assert!(e.state.objects[&oid].adventure_cast_permission.is_none());
    e.apply_command(0, &move_to(DevZone::Exile)).unwrap();
    assert_eq!(e.state.objects[&oid].zone, tricerules_core::Zone::Exile);
    assert!(e.state.objects[&oid].adventure_cast_permission.is_none());
    assert!(e.initial_response_batch().legal_by_player[&0]
        .zone_cast_actions
        .is_empty());
}

#[test]
fn stomp_with_no_legal_target_goes_to_graveyard_without_permission() {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &["bonecrusher_giant_stomp", "lightning_bolt", "grizzly_bears"],
        ),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(46, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);
    let adventure = relocate_to_hand(&mut e, 0, "bonecrusher_giant_stomp");
    relocate_to_hand(&mut e, 0, "lightning_bolt");
    let slot = e.state.players[0]
        .hand
        .iter()
        .position(|&candidate| candidate == adventure)
        .unwrap();
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 3,
            ..Default::default()
        },
    );
    e.apply_command(
        0,
        &cast_spell_face(
            slot,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
            1,
        ),
    )
    .unwrap();
    let bolt = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(
        0,
        &cast_spell(
            bolt,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
            }],
        ),
    )
    .unwrap();
    resolve_entire_stack_two_player(&mut e);

    assert!(
        e.state.damage_prevention_prohibitions.is_empty(),
        "a fully fizzled Stomp creates no turn-scoped prohibition"
    );
    assert_eq!(
        e.state.objects[&adventure].zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(e.state.objects[&adventure]
        .adventure_cast_permission
        .is_none());
    assert!(e.initial_response_batch().legal_by_player[&0]
        .zone_cast_actions
        .is_empty());
}

#[test]
fn countered_stomp_goes_to_graveyard_without_permission() {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &[
                "bonecrusher_giant_stomp",
                "counterspell",
                "island",
                "island",
            ],
        ),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(47, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let adventure = relocate_to_hand(&mut e, 0, "bonecrusher_giant_stomp");
    relocate_to_hand(&mut e, 0, "counterspell");
    let slot = e.state.players[0]
        .hand
        .iter()
        .position(|&candidate| candidate == adventure)
        .unwrap();
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 2,
            u: 3,
            ..Default::default()
        },
    );
    e.apply_command(0, &cast_spell_face(slot, target_player(1), 1))
        .unwrap();
    let stomp_on_stack = e.state.stack.last().unwrap().id;
    let counter = hand_index_for_card(&e, 0, "counterspell");
    e.apply_command(
        0,
        &cast_spell(
            counter,
            vec![TargetRef {
                object_id: stomp_on_stack,
                damage_amount: 0,
            }],
        ),
    )
    .unwrap();
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.state.objects[&adventure].zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(e.state.objects[&adventure]
        .adventure_cast_permission
        .is_none());
}

/// CR 712.11 and CR 710.2: transforming double-faced cards and flip cards have only their
/// front/top face available in hand. Their alternate permanent face is neither a separately
/// castable spell nor a zero-cost spell, even if authored with an empty mana cost.
#[test]
fn transform_and_flip_back_faces_cannot_be_cast_from_hand() {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &[
                "reckless_waif_merciless_predator",
                "akki_lavarunner_tok-tok,_volcano_born",
            ],
        ),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(87, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    for card_id in [
        "reckless_waif_merciless_predator",
        "akki_lavarunner_tok-tok,_volcano_born",
    ] {
        let oid = relocate_to_hand(&mut e, 0, card_id);
        let slot = e.state.players[0]
            .hand
            .iter()
            .position(|&candidate| candidate == oid)
            .expect("card in hand");

        let legal = e.initial_response_batch();
        let face_actions: Vec<_> = legal.legal_by_player[&0]
            .hand_actions
            .iter()
            .filter(|action| action.hand_index == slot as u32)
            .collect();
        assert_eq!(face_actions.len(), 1, "only the front face is offered");
        assert_eq!(face_actions[0].face_index, 0);

        let stack_len = e.state.stack.len();
        let error = e
            .apply_command(0, &cast_spell_face(slot, vec![], 1))
            .expect_err("the alternate permanent face is not castable from hand");
        assert!(matches!(error, tricerules_core::EngineError::Illegal(_)));
        assert_eq!(e.state.stack.len(), stack_len);
        assert!(e.state.players[0].hand.contains(&oid));
    }
}

#[test]
fn sole_transform_front_cast_has_no_redundant_face_annotation() {
    let decks = Some(vec![
        deck_with("mountain", &["reckless_waif_merciless_predator"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(86, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let oid = relocate_to_hand(&mut e, 0, "reckless_waif_merciless_predator");
    let slot = e.state.players[0]
        .hand
        .iter()
        .position(|&candidate| candidate == oid)
        .expect("Waif in hand");
    let legal = e.initial_response_batch();
    assert_eq!(
        legal.legal_by_player[&0]
            .hand_actions
            .iter()
            .filter(|action| {
                action.hand_index == slot as u32
                    && action.kind == rv1::HandActionKind::HandActionCastSpell as i32
            })
            .count(),
        1,
        "only Reckless Waif is a cast option"
    );
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );

    let pushed = e
        .apply_command(0, &cast_spell_face(slot, vec![], 0))
        .expect("cast Reckless Waif");
    let stack_push = pushed
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::StackPushed(push)) => Some(push),
            _ => None,
        })
        .expect("stack push");
    assert!(stack_push.ability_annotation.is_empty());
}

/// Classic Innistrad werewolves use an intervening-if upkeep trigger: after a turn in which no
/// spells were cast, the front face transforms in place and the triggered ability itself resolves
/// from the face that was showing when it triggered.
#[test]
fn reckless_waif_transforms_after_a_spell_free_turn() {
    let mut e = GameEngine::new(88, &[0, 1], 20, None, true).expect("new");
    let waif = inject_permanent_on_battlefield(&mut e, 0, "reckless_waif_merciless_predator");
    let original_owner = e.state.objects[&waif].owner;

    // Finish P0's current turn without casting a spell. The trigger is placed on the stack at
    // P1's upkeep, before that player receives priority.
    for _ in 0..64 {
        if e.state.active_player_id() == 1
            && e.state.turn_step == tricerules_core::TurnStep::Upkeep
            && !e.state.stack.is_empty()
        {
            break;
        }
        let priority = e.state.priority_player_id();
        let command = if e.state.cleanup_discard_player == Some(priority) {
            discard_cleanup(
                (e.state.players[e.state.player_idx(priority).unwrap()]
                    .hand
                    .len()
                    - 1) as u32,
            )
        } else {
            pass()
        };
        e.apply_command(priority, &command).expect("advance turn");
    }
    assert!(
        !e.state.stack.is_empty(),
        "werewolf trigger reached the stack"
    );
    resolve_entire_stack_two_player(&mut e);

    let object = &e.state.objects[&waif];
    assert_eq!(object.face_up_index, 1, "the back face is now active");
    assert_eq!(
        object.owner, original_owner,
        "physical identity was preserved"
    );
    assert_eq!(object.zone, tricerules_core::Zone::Battlefield);
    let characteristics = e
        .characteristics(waif)
        .expect("active face characteristics");
    assert_eq!(characteristics.power, Some(3));
    assert_eq!(characteristics.toughness, Some(2));
    assert_eq!(characteristics.types, vec!["Creature", "Werewolf"]);
    assert_eq!(characteristics.colors, vec![tricerules_cards::Color::Red]);
}

fn advance_to_next_upkeep_trigger(e: &mut GameEngine) {
    let starting_active = e.state.active_player_id();
    for _ in 0..96 {
        if e.state.active_player_id() != starting_active
            && e.state.turn_step == tricerules_core::TurnStep::Upkeep
        {
            return;
        }
        let priority = e.state.priority_player_id();
        let command = if e.state.cleanup_discard_player == Some(priority) {
            let player = e.state.player_idx(priority).unwrap();
            discard_cleanup((e.state.players[player].hand.len() - 1) as u32)
        } else {
            pass()
        };
        e.apply_command(priority, &command)
            .expect("advance to upkeep");
    }
    panic!("did not reach next upkeep");
}

#[test]
fn classic_werewolf_spell_count_thresholds_are_face_aware() {
    for card_id in [
        "reckless_waif_merciless_predator",
        "village_ironsmith_ironfang",
    ] {
        let mut e = GameEngine::new(89, &[0, 1], 20, None, true).expect("new");
        let oid = inject_permanent_on_battlefield(&mut e, 0, card_id);

        e.state.spells_cast_this_turn = 1;
        advance_to_next_upkeep_trigger(&mut e);
        assert!(e.state.stack.is_empty(), "one spell fires neither face");
        assert_eq!(e.state.objects[&oid].face_up_index, 0);

        e.state.objects.get_mut(&oid).unwrap().face_up_index = 1;
        let back = e.characteristics(oid).expect("back-face characteristics");
        assert_eq!(back.power, Some(3));
        assert!(
            card_id != "village_ironsmith_ironfang"
                || back.has_keyword(tricerules_cards::Keyword::FirstStrike)
        );
        e.state.spells_cast_this_turn = 2;
        advance_to_next_upkeep_trigger(&mut e);
        assert!(!e.state.stack.is_empty(), "two spells fire the back face");
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(e.state.objects[&oid].face_up_index, 0);
    }
}

#[test]
fn werewolf_intervening_if_is_rechecked_at_resolution() {
    let mut e = GameEngine::new(90, &[0, 1], 20, None, true).expect("new");
    let oid = inject_permanent_on_battlefield(&mut e, 0, "reckless_waif_merciless_predator");
    advance_to_next_upkeep_trigger(&mut e);
    assert!(!e.state.stack.is_empty());

    // A state mutation stands in for a future spell-count-changing effect and proves the generic
    // CR 603.4 resolution check reads live predicate state, not the trigger-time result.
    e.state.spells_cast_last_turn = 1;
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&oid].face_up_index, 0);
}

#[test]
fn older_transform_instruction_is_ignored_after_an_intervening_face_change() {
    let mut e = GameEngine::new(92, &[0, 1], 20, None, true).expect("new");
    let oid = inject_permanent_on_battlefield(&mut e, 0, "reckless_waif_merciless_predator");
    advance_to_next_upkeep_trigger(&mut e);
    assert!(!e.state.stack.is_empty());

    // Model another resolving transform instruction changing the permanent before this one.
    e.state.objects.get_mut(&oid).unwrap().face_up_index = 1;
    *e.state.face_change_generation.entry(oid).or_insert(0) += 1;
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&oid].face_up_index, 1,
        "CR 701.27f prevents the older instruction from toggling it back"
    );
}

#[test]
fn transform_preserves_battlefield_identity_and_does_not_fire_etb() {
    let decks = Some(vec![
        deck_with("plains", &["soul_warden"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(91, &[0, 1], 20, decks, true).expect("new");
    let _warden = inject_permanent_on_battlefield(&mut e, 0, "soul_warden");
    resolve_entire_stack_two_player(&mut e);
    let waif = inject_permanent_on_battlefield(&mut e, 0, "reckless_waif_merciless_predator");
    let attachment = inject_permanent_on_battlefield(&mut e, 0, "forest");
    let life_before = e.state.players[0].life;

    advance_to_next_upkeep_trigger(&mut e);
    {
        let object = e.state.objects.get_mut(&waif).unwrap();
        object.tapped = true;
        object.damage = 1;
        object
            .counters
            .insert(tricerules_cards::CounterKind::PlusOnePlusOne, 2);
    }
    e.state.objects.get_mut(&attachment).unwrap().attached_to = Some(waif);
    resolve_entire_stack_two_player(&mut e);

    let object = &e.state.objects[&waif];
    assert_eq!(object.face_up_index, 1);
    assert!(object.tapped);
    assert_eq!(object.damage, 1);
    assert_eq!(
        object.counter_count(tricerules_cards::CounterKind::PlusOnePlusOne),
        2
    );
    assert_eq!(object.controller, 0);
    assert_eq!(e.state.objects[&attachment].attached_to, Some(waif));
    assert_eq!(e.state.players[0].life, life_before, "no ETB event fired");
}

#[test]
fn akki_lavarunner_flips_after_damaging_an_opponent() {
    let mut e = GameEngine::new(93, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let akki = inject_permanent_on_battlefield(&mut e, 0, "akki_lavarunner_tok-tok,_volcano_born");
    e.state.objects.get_mut(&akki).unwrap().summoning_sick = false;

    e.apply_command(0, &declare_attackers(vec![akki])).unwrap();
    e.apply_command(0, &pass()).unwrap();
    e.apply_command(1, &pass()).unwrap();
    e.apply_command(0, &pass()).unwrap();
    e.apply_command(1, &pass()).unwrap();
    assert!(
        !e.state.stack.is_empty(),
        "damage trigger reached the stack"
    );
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(e.state.objects[&akki].face_up_index, 1);
    let tok_tok = e.characteristics(akki).unwrap();
    assert_eq!(tok_tok.power, Some(2));
    assert!(tok_tok.is_legendary());
    assert_eq!(tok_tok.colors, vec![tricerules_cards::Color::Red]);
}
