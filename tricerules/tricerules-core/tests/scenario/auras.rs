use crate::helpers::*;
use tricerules_cards::Keyword;
use tricerules_proto::ruled::v1::attachment_recipient::Recipient;
use tricerules_proto::ruled::v1::dev_command::Dev;
use tricerules_proto::ruled::v1::TargetRef;
use tricerules_proto::ruled::v1::{DevCommand, DevMoveCard, DevZone};

fn aura_deck(aura: &str) -> Vec<String> {
    (0..60).map(|_| aura.to_string()).collect()
}

fn cast_and_resolve_aura(e: &mut GameEngine, card_id: &str, target: u32, mana: ManaGift) -> u32 {
    ensure_card_in_hand(e, 0, card_id);
    give_mana(e, 0, mana);
    let idx = hand_index_for_card(e, 0, card_id);
    e.apply_command(
        0,
        &cast_spell(
            idx,
            vec![TargetRef {
                object_id: target,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
            }],
        ),
    )
    .expect("cast aura");
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, card_id)
}

fn cast_and_resolve_player_aura(
    e: &mut GameEngine,
    card_id: &str,
    player_id: i32,
    mana: ManaGift,
) -> (u32, RuledEventBatch) {
    ensure_card_in_hand(e, 0, card_id);
    give_mana(e, 0, mana);
    let slot = hand_index_for_card(e, 0, card_id);
    e.apply_command(0, &cast_spell(slot, target_player(player_id)))
        .expect("cast player Aura");
    e.apply_command(0, &pass()).expect("caster pass");
    let batch = e.apply_command(1, &pass()).expect("opponent pass");
    (battlefield_object_for_card(e, 0, card_id), batch)
}

#[test]
fn wingspan_stride_returns_itself_and_removes_its_modifier() {
    let decks = Some(vec![
        deck_with("island", &["wingspan_stride", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(4220, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);
    let aura = cast_and_resolve_aura(
        &mut e,
        "wingspan_stride",
        bear,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    assert_eq!(e.effective_power(bear), Some(3));
    assert_eq!(e.effective_toughness(bear), Some(3));
    assert!(e.effective_has_keyword(bear, Keyword::Flying));

    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut e, 0, aura, 0, vec![])
        .expect("activate the untargeted source-return ability");
    e.apply_command(0, &pass()).expect("controller pass");
    let resolution = e.apply_command(1, &pass()).expect("opponent pass");

    assert_eq!(e.state.objects[&aura].zone, tricerules_core::Zone::Hand);
    assert!(e.state.players[0].hand.contains(&aura));
    assert_eq!(e.state.objects[&aura].attached_to, None);
    assert_eq!(e.effective_power(bear), Some(2));
    assert_eq!(e.effective_toughness(bear), Some(2));
    assert!(!e.effective_has_keyword(bear, Keyword::Flying));
    let moves = permanents_moved_in(&resolution);
    assert!(moves.iter().any(|m| {
        m.object_id == aura
            && m.destination
                == tricerules_proto::ruled::v1::permanent_moved::Destination::Hand as i32
    }));
}

#[test]
fn wingspan_stride_does_not_return_a_new_source_generation() {
    let decks = Some(vec![
        deck_with("island", &["wingspan_stride", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(4221, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);
    let aura = cast_and_resolve_aura(
        &mut e,
        "wingspan_stride",
        bear,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut e, 0, aura, 0, vec![]).expect("activate source-return ability");

    *e.state.zone_change_generation.entry(aura).or_default() += 1;
    resolve_entire_stack_two_player(&mut e);

    assert_eq!(
        e.state.objects[&aura].zone,
        tricerules_core::Zone::Battlefield,
        "the old ability must not act on the new CR 400.7 object"
    );
}

#[test]
fn wingspan_stride_in_graveyard_is_not_returned_by_its_old_ability() {
    let decks = Some(vec![
        deck_with("island", &["wingspan_stride", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut e = GameEngine::new(4222, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);
    let aura = cast_and_resolve_aura(
        &mut e,
        "wingspan_stride",
        bear,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    give_mana(
        &mut e,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    apply_ability(&mut e, 0, aura, 0, vec![]).expect("activate source-return ability");

    e.state.players[0].battlefield.retain(|&id| id != bear);
    e.state.players[0].graveyard.push(bear);
    e.state.objects.get_mut(&bear).expect("bear").zone = tricerules_core::Zone::Graveyard;
    *e.state.zone_change_generation.entry(bear).or_default() += 1;
    e.apply_command(0, &pass())
        .expect("state-based actions move the orphaned Aura");
    assert_eq!(
        e.state.objects[&aura].zone,
        tricerules_core::Zone::Graveyard
    );

    e.apply_command(1, &pass())
        .expect("resolve the old source ability");
    assert_eq!(
        e.state.objects[&aura].zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(!e.state.players[0].hand.contains(&aura));
}

fn dev_move(player_id: i32, zone: DevZone, card_name: &str) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: player_id,
            dev: Some(Dev::MoveCard(DevMoveCard {
                card_name: card_name.to_string(),
                zone: zone as i32,
                ready: false,
            })),
        })),
    }
}

#[test]
fn curse_of_disturbance_can_target_a_player() {
    let decks = Some(vec![
        deck_with("swamp", &["curse_of_disturbance"]),
        vec!["swamp".into(); 20],
    ]);
    let mut e = GameEngine::new(4219, &[0, 1], 20, decks, true)
        .expect("player-enchanting Aura card data must validate");
    advance_to_main1_from_game_start(&mut e);
    ensure_card_in_hand(&mut e, 0, "curse_of_disturbance");
    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );

    let slot = hand_index_for_card(&e, 0, "curse_of_disturbance");
    e.apply_command(0, &cast_spell(slot, target_player(1)))
        .expect("cast Curse of Disturbance targeting opponent");
    e.apply_command(0, &pass()).expect("caster pass");
    let resolution = e.apply_command(1, &pass()).expect("opponent pass");
    let curse = battlefield_object_for_card(&e, 0, "curse_of_disturbance");

    assert_eq!(
        e.state.objects.get(&curse).map(|object| object.zone),
        Some(tricerules_core::Zone::Battlefield)
    );
    assert_eq!(
        e.state.objects[&curse].attached_to,
        Some(AttachmentRecipient::Player(1))
    );
    let aura_event = resolution.events.iter().find_map(|event| match &event.ev {
        Some(Ev::AuraAttached(attached)) => Some(attached),
        _ => None,
    });
    assert!(matches!(
        aura_event
            .and_then(|event| event.attachment_recipient.as_ref())
            .and_then(|recipient| recipient.recipient.as_ref()),
        Some(Recipient::PlayerId(1))
    ));

    let snapshot = e.initial_response_batch();
    let public_recipient = snapshot.events.iter().find_map(|event| match &event.ev {
        Some(Ev::ZoneView(view)) => view
            .per_player
            .iter()
            .flat_map(|player| &player.battlefield_objects)
            .find(|object| object.object_id == curse)
            .and_then(|object| object.attachment_recipient.as_ref()),
        _ => None,
    });
    assert!(matches!(
        public_recipient.and_then(|recipient| recipient.recipient.as_ref()),
        Some(Recipient::PlayerId(1))
    ));
}

#[test]
fn curse_of_opulence_can_enchant_its_controller() {
    let decks = Some(vec![
        aura_deck("curse_of_opulence"),
        aura_deck("curse_of_opulence"),
    ]);
    let mut e = GameEngine::new(4220, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let (curse, _) = cast_and_resolve_player_aura(
        &mut e,
        "curse_of_opulence",
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    assert_eq!(
        e.state.objects[&curse].attached_to,
        Some(AttachmentRecipient::Player(0))
    );
}

#[test]
fn player_aura_rejects_a_forged_permanent_target() {
    let decks = Some(vec![
        aura_deck("curse_of_opulence"),
        aura_deck("curse_of_opulence"),
    ]);
    let mut e = GameEngine::new(4221, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    ensure_card_in_hand(&mut e, 0, "curse_of_opulence");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "curse_of_opulence");
    assert!(
        e.apply_command(
            0,
            &cast_spell(
                slot,
                vec![TargetRef {
                    object_id: bear,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
                }]
            )
        )
        .is_err(),
        "AnyPlayer Aura must reject a forged permanent target"
    );
}

#[test]
fn player_aura_sba_cleans_up_a_lost_recipient() {
    let decks = Some(vec![
        aura_deck("curse_of_opulence"),
        aura_deck("curse_of_opulence"),
    ]);
    let mut e = GameEngine::new(4223, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let (curse, _) = cast_and_resolve_player_aura(
        &mut e,
        "curse_of_opulence",
        1,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    e.state.players[1].has_lost = true;
    e.apply_command(0, &pass()).expect("pass applies SBAs");
    assert_eq!(
        e.state.objects[&curse].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(e.state.objects[&curse].attached_to, None);
}

#[test]
fn player_aura_survives_control_change_and_clears_on_zone_change() {
    let decks = Some(vec![
        deck_with("swamp", &["curse_of_disturbance"]),
        vec!["swamp".into(); 20],
    ]);
    let mut e = GameEngine::new(4224, &[0, 1], 20, decks, true).expect("new");
    e.enable_dev_commands();
    advance_to_main1_from_game_start(&mut e);
    let (curse, _) = cast_and_resolve_player_aura(
        &mut e,
        "curse_of_disturbance",
        1,
        ManaGift {
            b: 1,
            c: 2,
            ..Default::default()
        },
    );
    e.state.objects.get_mut(&curse).expect("Curse").controller = 1;
    e.state
        .objects
        .get_mut(&curse)
        .expect("Curse")
        .base_controller = 1;
    e.state.players[0].battlefield.retain(|&id| id != curse);
    e.state.players[1].battlefield.push(curse);
    e.apply_command(0, &pass()).expect("pass applies SBAs");
    assert_eq!(
        e.state.objects[&curse].attached_to,
        Some(AttachmentRecipient::Player(1)),
        "AnyPlayer remains legal after the Aura changes control"
    );

    e.apply_command(0, &dev_move(1, DevZone::Graveyard, "Curse of Disturbance"))
        .expect("move Curse to graveyard");
    assert_eq!(
        e.state.objects[&curse].zone,
        tricerules_core::Zone::Graveyard
    );
    assert_eq!(
        e.state.objects[&curse].attached_to, None,
        "CR 400.7 exit funnel clears typed recipient"
    );
}

#[test]
fn holy_strength_buffs_enchanted_creature() {
    // Happy path: Holy Strength (+1/+2) on Grizzly Bears (2/2) → effective 3/4.
    let decks = Some(vec![
        vec![
            "plains".into(),
            "holy_strength".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(4201, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let hs_idx = hand_index_for_card(&e, 0, "holy_strength");
    e.apply_command(
        0,
        &cast_spell(
            hs_idx,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
            }],
        ),
    )
    .expect("cast Holy Strength targeting bear");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");

    // Bear is now enchanted with Holy Strength.
    assert_eq!(
        e.effective_power(bear),
        Some(3),
        "Holy Strength grants +1 power (2+1=3)"
    );
    assert_eq!(
        e.effective_toughness(bear),
        Some(4),
        "Holy Strength grants +2 toughness (2+2=4)"
    );

    // The aura itself should be on the battlefield, attached to the bear.
    let hs_oid = battlefield_object_for_card(&e, 0, "holy_strength");
    assert_eq!(
        e.state.objects.get(&hs_oid).and_then(|o| o.attached_to),
        Some(AttachmentRecipient::Object(bear)),
        "aura.attached_to must point at the enchanted creature"
    );
}

#[test]
fn unholy_strength_buffs_enchanted_creature() {
    // Unholy Strength (+2/+1) on Walking Corpse (2/2) → effective 4/3.
    let decks = Some(vec![
        vec![
            "swamp".into(),
            "unholy_strength".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
        ],
        vec!["swamp".into(); 7],
    ]);
    let mut e = GameEngine::new(4202, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let corpse = inject_creature_on_battlefield(&mut e, 0, "walking_corpse");

    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            ..Default::default()
        },
    );
    let us_idx = hand_index_for_card(&e, 0, "unholy_strength");
    e.apply_command(
        0,
        &cast_spell(
            us_idx,
            vec![TargetRef {
                object_id: corpse,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
            }],
        ),
    )
    .expect("cast Unholy Strength");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");

    assert_eq!(e.effective_power(corpse), Some(4), "2+2=4 power");
    assert_eq!(e.effective_toughness(corpse), Some(3), "2+1=3 toughness");
}

#[test]
fn aura_pt_buff_removed_when_aura_leaves_battlefield() {
    // The WhileSourceOnBattlefield continuous effect drains when the aura is destroyed.
    let decks = Some(vec![
        vec![
            "plains".into(),
            "holy_strength".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(4205, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let hs_idx = hand_index_for_card(&e, 0, "holy_strength");
    e.apply_command(
        0,
        &cast_spell(
            hs_idx,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
            }],
        ),
    )
    .expect("cast Holy Strength");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");

    let hs_oid = battlefield_object_for_card(&e, 0, "holy_strength");
    assert_eq!(
        e.effective_power(bear),
        Some(3),
        "buffed before aura removed"
    );

    // Directly remove the aura from the battlefield (simulates it being destroyed/bounced).
    {
        let obj = e.state.objects.get_mut(&hs_oid).expect("hs obj");
        obj.zone = tricerules_core::Zone::Graveyard;
        obj.attached_to = None;
    }
    e.state.players[0].battlefield.retain(|&id| id != hs_oid);
    e.state.players[0].graveyard.push(hs_oid);
    // Drain all continuous effects sourced from the aura (WhileSourceOnBattlefield).
    e.state
        .continuous_effects
        .retain(|ce| ce.source_id != Some(hs_oid));

    // Bear should return to base stats once the aura's effect is drained.
    assert_eq!(e.effective_power(bear), Some(2), "power back to base 2");
    assert_eq!(
        e.effective_toughness(bear),
        Some(2),
        "toughness back to base 2"
    );
}

#[test]
fn aura_dies_when_enchanted_creature_dies_sba() {
    // CR 704.5m: aura on battlefield with no valid enchanted permanent goes to graveyard on SBA.
    let decks = Some(vec![
        vec![
            "plains".into(),
            "holy_strength".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(4203, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let hs_idx = hand_index_for_card(&e, 0, "holy_strength");
    e.apply_command(
        0,
        &cast_spell(
            hs_idx,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
            }],
        ),
    )
    .expect("cast Holy Strength");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");

    let hs_oid = battlefield_object_for_card(&e, 0, "holy_strength");
    assert_eq!(
        e.state.objects.get(&hs_oid).map(|o| o.zone),
        Some(tricerules_core::Zone::Battlefield),
        "aura on battlefield before creature dies"
    );

    // Move the bear to the graveyard (simulating lethal damage SBA or destroy effect).
    // This mimics what move_object_to_zone does without going through the full engine path.
    {
        let obj = e.state.objects.get_mut(&bear).expect("bear obj");
        obj.zone = tricerules_core::Zone::Graveyard;
    }
    e.state.players[0].battlefield.retain(|&id| id != bear);
    e.state.players[0].graveyard.push(bear);

    // Passing priority triggers SBA check: orphaned aura should be destroyed.
    e.apply_command(0, &pass()).expect("pass triggers SBA");

    assert_eq!(
        e.state.objects.get(&hs_oid).map(|o| o.zone),
        Some(tricerules_core::Zone::Graveyard),
        "aura must go to graveyard (CR 704.5m) when enchanted creature leaves battlefield"
    );
}

#[test]
fn aura_spell_fizzles_when_target_leaves_before_resolution() {
    // CR 303.4f: aura goes to graveyard (not battlefield) when target leaves before resolution.
    let decks = Some(vec![
        vec![
            "plains".into(),
            "holy_strength".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(4204, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    give_mana(
        &mut e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let hs_idx = hand_index_for_card(&e, 0, "holy_strength");
    e.apply_command(
        0,
        &cast_spell(
            hs_idx,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
            }],
        ),
    )
    .expect("cast Holy Strength");

    // Aura is on the stack; now remove the target creature before resolution.
    let hs_stack_id = e.state.stack.last().expect("hs on stack").id;
    {
        let obj = e.state.objects.get_mut(&bear).expect("bear obj");
        obj.zone = tricerules_core::Zone::Graveyard;
    }
    e.state.players[0].battlefield.retain(|&id| id != bear);
    e.state.players[0].graveyard.push(bear);

    // Both players pass — aura resolves but target is gone, must go to graveyard.
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");

    assert_eq!(
        e.state.objects.get(&hs_stack_id).map(|o| o.zone),
        Some(tricerules_core::Zone::Graveyard),
        "aura goes to graveyard when target left before resolution (CR 303.4f)"
    );
    // No continuous effect should have been created.
    let aura_effects: Vec<_> = e
        .state
        .continuous_effects
        .iter()
        .filter(|ce| ce.source_id == Some(hs_stack_id))
        .collect();
    assert!(
        aura_effects.is_empty(),
        "no P/T buff should linger when aura fizzled"
    );
}

#[test]
fn flight_grants_flying_only_while_attached() {
    let decks = Some(vec![aura_deck("flight"), aura_deck("flight")]);
    let mut e = GameEngine::new(4210, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let flight = cast_and_resolve_aura(
        &mut e,
        "flight",
        bear,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );

    assert!(e.effective_has_keyword(bear, Keyword::Flying));
    assert_eq!(
        zone_view_rules_annotation_labels(&mut e, 0, bear),
        vec!["Flying"],
        "Flight's static ability is visible in the granted-ability feed"
    );
    e.state.objects.get_mut(&flight).expect("Flight").zone = tricerules_core::Zone::Graveyard;
    assert!(
        !e.effective_has_keyword(bear, Keyword::Flying),
        "the attached keyword stops applying as soon as its source leaves"
    );
    assert!(
        zone_view_rules_annotation_labels(&mut e, 0, bear).is_empty(),
        "the annotation disappears with the granting source"
    );
}

#[test]
fn pacifism_restriction_overrides_must_attack() {
    let decks = Some(vec![aura_deck("pacifism"), aura_deck("pacifism")]);
    let mut e = GameEngine::new(4211, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let goblin = inject_creature_on_battlefield(&mut e, 0, "crazed_goblin");
    e.state
        .objects
        .get_mut(&goblin)
        .expect("goblin")
        .must_attack_if_able = true;
    let pacifism = cast_and_resolve_aura(
        &mut e,
        "pacifism",
        goblin,
        ManaGift {
            w: 2,
            ..Default::default()
        },
    );
    assert_eq!(
        zone_view_rules_annotation_labels(&mut e, 0, goblin),
        vec!["Can't attack", "Can't block"],
        "Pacifism's two combat restrictions are visible on the enchanted creature"
    );
    let legal_bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap pass begin combat");
    let declare_batch = e.apply_command(1, &pass()).expect("nap pass begin combat");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
    let legal = declare_batch
        .legal_by_player
        .get(&0)
        .expect("active-player legal actions");
    assert_eq!(legal.selectable_attacker_ids, vec![legal_bear]);
    assert!(
        e.apply_command(0, &declare_attackers(vec![goblin]))
            .is_err(),
        "Pacifism must make its creature an illegal attacker"
    );
    e.apply_command(0, &declare_attackers(vec![]))
        .expect("a Pacified must-attack creature is not able and may be omitted");

    e.state.objects.get_mut(&pacifism).expect("Pacifism").zone = tricerules_core::Zone::Graveyard;
    assert!(
        zone_view_rules_annotation_labels(&mut e, 0, goblin).is_empty(),
        "the restriction labels disappear with their source"
    );
}

#[test]
fn pacifism_prevents_blocking() {
    let decks = Some(vec![aura_deck("pacifism"), aura_deck("pacifism")]);
    let mut e = GameEngine::new(4214, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let legal_blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast_and_resolve_aura(
        &mut e,
        "pacifism",
        blocker,
        ManaGift {
            w: 2,
            ..Default::default()
        },
    );

    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap pass begin combat");
    e.apply_command(1, &pass()).expect("nap pass begin combat");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("ap pass after attackers");
    let blockers_batch = e.apply_command(1, &pass()).expect("nap pass to blockers");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers
    );
    let legal = blockers_batch
        .legal_by_player
        .get(&1)
        .expect("defending-player legal actions");
    let legal_pairs: Vec<_> = legal
        .legal_block_pairs
        .iter()
        .map(|pair| (pair.blocker_id, pair.attacker_id))
        .collect();
    assert_eq!(legal_pairs, [(legal_blocker, attacker)]);
    assert!(
        e.apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .is_err(),
        "Pacifism must make its creature an illegal blocker"
    );
    e.apply_command(1, &declare_blockers(vec![]))
        .expect("the Pacified creature is not required or allowed to block");
}

#[test]
fn aura_dies_when_host_stops_matching_enchant_filter() {
    let decks = Some(vec![aura_deck("flight"), aura_deck("flight")]);
    let mut e = GameEngine::new(4212, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let flight = cast_and_resolve_aura(
        &mut e,
        "flight",
        bear,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );

    e.state.objects.get_mut(&bear).expect("bear").card_id = "plains".into();
    e.apply_command(0, &pass()).expect("pass triggers SBAs");
    assert_eq!(
        e.state.objects.get(&flight).map(|object| object.zone),
        Some(tricerules_core::Zone::Graveyard),
        "Enchant creature becomes illegal when the host stops being a creature"
    );
}

#[test]
fn existing_aura_ignores_shroud_gained_after_resolution() {
    let decks = Some(vec![aura_deck("flight"), aura_deck("flight")]);
    let mut e = GameEngine::new(4218, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let flight = cast_and_resolve_aura(
        &mut e,
        "flight",
        bear,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );

    e.state.objects.get_mut(&bear).expect("bear").card_id = "argothian_enchantress".into();
    assert!(e.effective_has_keyword(bear, Keyword::Shroud));
    e.apply_command(0, &pass()).expect("pass triggers SBAs");
    assert_eq!(
        e.state.objects.get(&flight).map(|object| object.zone),
        Some(tricerules_core::Zone::Battlefield),
        "shroud affects targeting, not an Aura that is already attached"
    );
}

#[test]
fn indestructibility_can_enchant_a_land() {
    let decks = Some(vec![
        vec!["indestructibility".into()]
            .into_iter()
            .chain((0..59).map(|_| "plains".into()))
            .collect(),
        vec!["plains".into(); 60],
    ]);
    let mut e = GameEngine::new(4213, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let land = relocate_to_battlefield(&mut e, 0, "plains", false);
    cast_and_resolve_aura(
        &mut e,
        "indestructibility",
        land,
        ManaGift {
            w: 1,
            c: 3,
            ..Default::default()
        },
    );
    assert!(e.effective_has_keyword(land, Keyword::Indestructible));
}

#[test]
fn oakenform_grants_its_printed_pt_bonus() {
    let decks = Some(vec![aura_deck("oakenform"), aura_deck("oakenform")]);
    let mut e = GameEngine::new(4215, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    cast_and_resolve_aura(
        &mut e,
        "oakenform",
        bear,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    assert_eq!(e.effective_power(bear), Some(5));
    assert_eq!(e.effective_toughness(bear), Some(5));
}

#[test]
fn guard_duty_grants_defender() {
    let decks = Some(vec![aura_deck("guard_duty"), aura_deck("guard_duty")]);
    let mut e = GameEngine::new(4216, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    cast_and_resolve_aura(
        &mut e,
        "guard_duty",
        bear,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    assert!(e.effective_has_keyword(bear, Keyword::Defender));
}

#[test]
fn indestructibility_prevents_destroy_effects() {
    let decks = Some(vec![aura_deck("indestructibility"), aura_deck("murder")]);
    let mut e = GameEngine::new(4217, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    cast_and_resolve_aura(
        &mut e,
        "indestructibility",
        bear,
        ManaGift {
            w: 1,
            c: 3,
            ..Default::default()
        },
    );

    give_mana(
        &mut e,
        1,
        ManaGift {
            b: 3,
            ..Default::default()
        },
    );
    e.apply_command(0, &pass())
        .expect("active player passes priority");
    let murder_idx = hand_index_for_card(&e, 1, "murder");
    e.apply_command(
        1,
        &cast_spell(
            murder_idx,
            vec![TargetRef {
                object_id: bear,
                damage_amount: 0,
                group_index: 0,
                kind: 0,
            }],
        ),
    )
    .expect("cast Murder");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects.get(&bear).map(|object| object.zone),
        Some(tricerules_core::Zone::Battlefield),
        "the enchanted creature survives a destroy effect"
    );
}
