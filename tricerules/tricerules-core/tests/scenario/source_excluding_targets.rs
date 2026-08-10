use crate::helpers::*;
use tricerules_cards::Keyword;

fn choose_trigger_target(target_object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            target_object_id,
            decline: false,
        })),
    }
}

fn target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id,
        damage_amount: 0,
    }]
}

#[test]
fn pegasus_courser_excludes_itself_and_grants_flying_to_the_other_attacker() {
    let decks = Some(vec![
        deck_with("plains", &["pegasus_courser"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(97001, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut engine);
    let other_attacker = battlefield_object_for_card(&engine, 0, "grizzly_bears");
    let courser = inject_creature_on_battlefield(&mut engine, 0, "pegasus_courser");

    let batch = engine
        .apply_command(0, &declare_attackers(vec![courser, other_attacker]))
        .expect("declare Pegasus Courser and Grizzly Bears as attackers");
    let key = (courser as u64) << 32;
    let targets = batch
        .legal_by_player
        .get(&0)
        .expect("active player legal actions")
        .valid_targets_by_ability
        .get(&key)
        .expect("Pegasus Courser trigger target set");
    assert_eq!(
        targets.valid_permanent_ids,
        vec![other_attacker],
        "the trigger must publish only the other attacking creature"
    );

    let err = engine
        .apply_command(0, &choose_trigger_target(courser))
        .expect_err("Pegasus Courser cannot target itself");
    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));
    assert_eq!(
        engine.state.pending_triggers.len(),
        1,
        "a rejected source target must leave the trigger pending"
    );

    engine
        .apply_command(0, &choose_trigger_target(other_attacker))
        .expect("choose the other attacker");
    pass_both_players(&mut engine);
    assert!(engine.effective_has_keyword(other_attacker, Keyword::Flying));
}

#[test]
fn legion_guildmage_rejects_itself_before_costs_and_taps_another_creature() {
    let decks = Some(vec![
        deck_with("plains", &["legion_guildmage"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(97002, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    let guildmage = inject_creature_on_battlefield(&mut engine, 0, "legion_guildmage");
    let other_creature = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 2,
            ..Default::default()
        },
    );

    let err = engine
        .apply_command(0, &activate_ability(guildmage, 1, target(guildmage)))
        .expect_err("Legion Guildmage cannot target itself");
    assert!(matches!(err, tricerules_core::EngineError::Illegal(_)));
    assert!(
        !engine
            .state
            .objects
            .get(&guildmage)
            .expect("guildmage")
            .tapped
    );
    assert_eq!(engine.state.players[0].mana_pool.white, 1);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 2);
    assert!(engine.state.stack.is_empty());

    engine
        .apply_command(0, &activate_ability(guildmage, 1, target(other_creature)))
        .expect("target another creature");
    assert!(
        engine
            .state
            .objects
            .get(&guildmage)
            .expect("guildmage")
            .tapped
    );
    assert_eq!(engine.state.players[0].mana_pool.white, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert_eq!(engine.state.stack.len(), 1);

    pass_both_players(&mut engine);
    assert!(
        engine
            .state
            .objects
            .get(&other_creature)
            .expect("target creature")
            .tapped
    );
}

#[test]
fn legion_guildmage_damages_the_opponent_not_its_controller() {
    let decks = Some(vec![
        deck_with("mountain", &["legion_guildmage"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(97003, &[0, 1], 20, decks, true).expect("new");
    let guildmage = inject_creature_on_battlefield(&mut engine, 0, "legion_guildmage");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 5,
            ..Default::default()
        },
    );

    engine
        .apply_command(0, &activate_ability(guildmage, 0, vec![]))
        .expect("activate each-opponent damage ability");
    pass_both_players(&mut engine);

    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[1].life, 17);
}
