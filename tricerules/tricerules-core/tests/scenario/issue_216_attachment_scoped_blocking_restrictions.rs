use super::helpers::*;
use tricerules_core::{AttachmentRecipient, TurnStep, Zone};
use tricerules_proto::ruled::v1::{BlockPair, ChooseTriggerTarget, RuledCommand};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &["meltstriders_resolve"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("issue #216 engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_and_resolve_aura_spell(engine: &mut GameEngine, creature: u32) -> u32 {
    ensure_card_in_hand(engine, 0, "meltstriders_resolve");
    give_mana(
        engine,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(engine, 0, "meltstriders_resolve");
    engine
        .apply_command(0, &cast_spell(slot, target_object(creature)))
        .expect("cast Meltstrider's Resolve");
    resolve_entire_stack_two_player(engine);
    battlefield_object_for_card(engine, 0, "meltstriders_resolve")
}

fn choose_fight_target(engine: &mut GameEngine, target: Option<u32>) {
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    targets: target.map(target_object).unwrap_or_default(),
                    ..Default::default()
                })),
            },
        )
        .expect("choose fight target");
    resolve_entire_stack_two_player(engine);
}

fn cast_resolve_and_skip_fight(engine: &mut GameEngine, creature: u32) -> u32 {
    let aura = cast_and_resolve_aura_spell(engine, creature);
    choose_fight_target(engine, None);
    aura
}

fn advance_to_declare_attackers(engine: &mut GameEngine) {
    engine
        .apply_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    engine.apply_command(0, &pass()).expect("attacker passes");
    engine.apply_command(1, &pass()).expect("defender passes");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
}

fn pass_to_declare_blockers(engine: &mut GameEngine) {
    engine
        .apply_command(0, &pass())
        .expect("attacker passes after declaration");
    engine
        .apply_command(1, &pass())
        .expect("defender passes after declaration");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareBlockers);
}

#[test]
fn attached_maximum_blockers_is_public_and_engine_authoritative() {
    let mut engine = engine(216_001);
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let blocker_a = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let blocker_b = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    cast_resolve_and_skip_fight(&mut engine, attacker);

    assert_eq!(engine.effective_toughness(attacker), Some(4));
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, attacker),
        vec!["Can't be blocked by more than 1 creature"]
    );

    advance_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare enchanted attacker");
    pass_to_declare_blockers(&mut engine);

    let before_index = engine.state.command_index;
    let err = engine
        .apply_command(
            1,
            &declare_blockers(vec![
                BlockPair {
                    attacker_id: attacker,
                    blocker_id: blocker_a,
                },
                BlockPair {
                    attacker_id: attacker,
                    blocker_id: blocker_b,
                },
            ]),
        )
        .expect_err("two creatures cannot block the enchanted attacker");
    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));
    assert_eq!(engine.state.command_index, before_index);
    assert!(engine
        .state
        .combat
        .as_ref()
        .expect("combat")
        .blockers
        .is_empty());

    engine
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker_a,
            }]),
        )
        .expect("one creature may block the enchanted attacker");
}

#[test]
fn enters_trigger_is_optional_and_only_targets_an_opponents_creature() {
    let mut engine = engine(216_002);
    let enchanted = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    cast_and_resolve_aura_spell(&mut engine, enchanted);

    let before_index = engine.state.command_index;
    let err = engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    targets: target_object(enchanted),
                    ..Default::default()
                })),
            },
        )
        .expect_err("the enchanted creature is not controlled by an opponent");
    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));
    assert_eq!(engine.state.command_index, before_index);

    choose_fight_target(&mut engine, Some(opponent));
    assert_eq!(engine.state.objects[&enchanted].damage, 2);
    assert_eq!(engine.state.objects[&enchanted].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&opponent].zone, Zone::Graveyard);
}

#[test]
fn fight_does_nothing_when_the_aura_is_detached_before_resolution() {
    let mut engine = engine(216_003);
    let enchanted = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let aura = cast_and_resolve_aura_spell(&mut engine, enchanted);
    engine
        .state
        .objects
        .get_mut(&aura)
        .expect("aura")
        .attached_to = None;

    choose_fight_target(&mut engine, Some(opponent));
    assert_eq!(engine.state.objects[&enchanted].damage, 0);
    assert_eq!(engine.state.objects[&opponent].damage, 0);
}

#[test]
fn modifier_follows_the_attachment_and_ends_when_the_source_leaves() {
    let mut engine = engine(216_004);
    let first = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let second = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let aura = cast_resolve_and_skip_fight(&mut engine, first);

    engine
        .state
        .objects
        .get_mut(&aura)
        .expect("aura")
        .attached_to = Some(AttachmentRecipient::Object(second));
    assert_eq!(engine.effective_toughness(first), Some(2));
    assert_eq!(engine.effective_toughness(second), Some(4));
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, first).is_empty());
    assert_eq!(
        zone_view_rules_annotation_labels(&mut engine, 0, second),
        vec!["Can't be blocked by more than 1 creature"]
    );

    engine.state.objects.get_mut(&aura).expect("aura").zone = Zone::Graveyard;
    assert_eq!(engine.effective_toughness(second), Some(2));
    assert!(zone_view_rules_annotation_labels(&mut engine, 0, second).is_empty());
}

#[test]
fn maximum_one_blocker_combined_with_menace_means_unblockable() {
    let mut engine = engine(216_005);
    let attacker = inject_creature_on_battlefield(&mut engine, 0, "goblin_trailblazer");
    let blocker_a = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let blocker_b = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    cast_resolve_and_skip_fight(&mut engine, attacker);

    advance_to_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare enchanted menace attacker");
    pass_to_declare_blockers(&mut engine);

    for blockers in [
        vec![BlockPair {
            attacker_id: attacker,
            blocker_id: blocker_a,
        }],
        vec![
            BlockPair {
                attacker_id: attacker,
                blocker_id: blocker_a,
            },
            BlockPair {
                attacker_id: attacker,
                blocker_id: blocker_b,
            },
        ],
    ] {
        let before_index = engine.state.command_index;
        assert!(engine
            .apply_command(1, &declare_blockers(blockers))
            .is_err());
        assert_eq!(engine.state.command_index, before_index);
    }
    engine
        .apply_command(1, &declare_blockers(Vec::new()))
        .expect("the menace attacker may remain unblocked");
}
