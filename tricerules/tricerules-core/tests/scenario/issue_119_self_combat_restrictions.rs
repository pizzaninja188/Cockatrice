use super::helpers::*;
use tricerules_cards::primitives::ContinuousEffectKind;
use tricerules_proto::ruled::v1::dev_command::Dev;
use tricerules_proto::ruled::v1::{DevCommand, DevMoveCard, DevPutCardInZone, DevZone};

fn put_ready_on_battlefield(target: i32, card_name: &str) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: target,
            dev: Some(Dev::PutCardInZone(DevPutCardInZone {
                card_name: card_name.to_string(),
                zone: DevZone::Battlefield as i32,
                ready: true,
            })),
        })),
    }
}

fn move_card(target: i32, zone: DevZone, card_name: &str) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: target,
            dev: Some(Dev::MoveCard(DevMoveCard {
                card_name: card_name.to_string(),
                zone: zone as i32,
                ready: true,
            })),
        })),
    }
}

fn dev_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("mountain", &[]), deck_with("swamp", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new engine");
    engine.enable_dev_commands();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn self_restriction_count(engine: &GameEngine, source_id: u32) -> usize {
    engine
        .state
        .continuous_effects
        .iter()
        .filter(|effect| {
            effect.source_id == Some(source_id)
                && matches!(
                    &effect.kind,
                    ContinuousEffectKind::CombatRestriction(restriction)
                        if restriction.cant_block && !restriction.cant_attack
                )
        })
        .count()
}

#[test]
fn vampire_soulcaller_cannot_block() {
    let mut engine = dev_engine(119_001);

    engine
        .apply_command(0, &put_ready_on_battlefield(1, "Vampire Soulcaller"))
        .expect("put Vampire Soulcaller onto the battlefield");
    let soulcaller = battlefield_object_for_card(&engine, 1, "vampire_soulcaller");
    engine
        .state
        .objects
        .get_mut(&soulcaller)
        .expect("Soulcaller")
        .must_block_if_able = true;
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 1, soulcaller),
        vec!["Can't block"],
        "the public battlefield view reports the restriction"
    );
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");

    engine
        .apply_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    engine.apply_command(0, &pass()).expect("attacker passes");
    engine.apply_command(1, &pass()).expect("defender passes");
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    engine
        .apply_command(0, &pass())
        .expect("attacker passes after declaration");
    let blockers = engine
        .apply_command(1, &pass())
        .expect("defender passes after declaration");

    let legal = blockers
        .legal_by_player
        .get(&1)
        .expect("defender legal actions");
    assert!(
        legal
            .legal_block_pairs
            .iter()
            .all(|pair| pair.blocker_id != soulcaller),
        "Vampire Soulcaller must not be published as a legal blocker"
    );
    assert!(
        !legal.required_blocker_ids.contains(&soulcaller),
        "a prohibition makes the blocking requirement impossible rather than mandatory"
    );
    let err = engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: soulcaller,
            }]),
        )
        .expect_err("Vampire Soulcaller cannot block");
    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));
}

#[test]
fn vampire_soulcaller_mandatory_etb_returns_a_creature_card() {
    let mut engine = dev_engine(119_002);
    let bears = inject_graveyard_card(&mut engine, 0, "grizzly_bears");

    let entry = engine
        .apply_command(0, &put_ready_on_battlefield(0, "Vampire Soulcaller"))
        .expect("put Vampire Soulcaller onto the battlefield");
    let pending = engine
        .state
        .pending_triggers
        .front()
        .expect("mandatory ETB target choice");
    assert!(!pending.may, "Vampire Soulcaller's trigger is mandatory");
    let key = (pending.source_permanent_id as u64) << 32 | pending.ability_index as u64;
    assert_eq!(
        entry.legal_by_player[&0].valid_targets_by_ability[&key].groups[0].valid_graveyard_ids,
        vec![bears]
    );

    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    selected_modes: Vec::new(),
                    targets: target_object(bears),
                })),
            },
        )
        .expect("choose the graveyard creature");
    pass_both_players(&mut engine);

    assert!(engine.state.players[0].hand.contains(&bears));
    assert!(!engine.state.players[0].graveyard.contains(&bears));
}

#[test]
fn copied_and_controlled_soulcallers_keep_the_self_restriction() {
    let decks = Some(vec![
        deck_with("island", &["mind_control"]),
        deck_with("swamp", &[]),
    ]);
    let mut engine = GameEngine::new(119_003, &[0, 1], 20, decks, true).expect("new engine");
    engine.enable_dev_commands();
    advance_to_main1_from_game_start(&mut engine);
    engine
        .apply_command(0, &put_ready_on_battlefield(1, "Vampire Soulcaller"))
        .expect("put Vampire Soulcaller onto the battlefield");
    let soulcaller = battlefield_object_for_card(&engine, 1, "vampire_soulcaller");

    engine
        .apply_command(0, &put_ready_on_battlefield(0, "Clone"))
        .expect("put Clone and request a copy source");
    engine
        .apply_command(0, &submit_resolution_choice(vec![soulcaller]))
        .expect("copy Vampire Soulcaller");
    let clone = battlefield_object_for_card(&engine, 0, "clone");
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, clone),
        vec!["Can't block"],
        "the copied face carries the static restriction"
    );

    ensure_in_hand(&mut engine, 0, "mind_control");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 2,
            c: 3,
            ..Default::default()
        },
    );
    let mind_control = hand_index_for_card(&engine, 0, "mind_control");
    engine
        .apply_command(0, &cast_spell(mind_control, target_object(soulcaller)))
        .expect("cast Mind Control");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&soulcaller].controller, 0);
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, soulcaller),
        vec!["Can't block"],
        "the object-scoped restriction follows a control change"
    );
}

#[test]
fn self_restriction_is_removed_on_leave_and_recreated_on_return() {
    let mut engine = dev_engine(119_004);
    engine
        .apply_command(0, &put_ready_on_battlefield(1, "Vampire Soulcaller"))
        .expect("put Vampire Soulcaller onto the battlefield");
    let soulcaller = battlefield_object_for_card(&engine, 1, "vampire_soulcaller");
    let generation = engine.state.zone_change_generation[&soulcaller];
    assert_eq!(self_restriction_count(&engine, soulcaller), 1);

    engine
        .apply_command(0, &move_card(1, DevZone::Hand, "Vampire Soulcaller"))
        .expect("move Vampire Soulcaller to hand");
    assert_eq!(self_restriction_count(&engine, soulcaller), 0);

    engine
        .apply_command(0, &move_card(1, DevZone::Battlefield, "Vampire Soulcaller"))
        .expect("return Vampire Soulcaller to the battlefield");
    assert_eq!(
        battlefield_object_for_card(&engine, 1, "vampire_soulcaller"),
        soulcaller
    );
    assert!(engine.state.zone_change_generation[&soulcaller] > generation);
    assert_eq!(self_restriction_count(&engine, soulcaller), 1);
}
