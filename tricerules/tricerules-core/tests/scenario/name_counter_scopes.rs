use crate::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_proto::ruled::v1::dev_command::Dev;
use tricerules_proto::ruled::v1::{DevCommand, DevMoveCard, DevZone};

fn move_card(target: i32, zone: DevZone, card_name: &str) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: target,
            dev: Some(Dev::MoveCard(DevMoveCard {
                card_name: card_name.to_string(),
                zone: zone as i32,
                ready: false,
            })),
        })),
    }
}

#[test]
fn pack_mastiff_matches_copiable_names_and_snapshots_objects() {
    let decks = Some(vec![
        vec![
            "clone".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut engine = GameEngine::new(79_001, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);

    let mastiff = inject_creature_on_battlefield(&mut engine, 0, "pack_mastiff");
    let other = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opposing_mastiff = inject_creature_on_battlefield(&mut engine, 1, "pack_mastiff");

    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let clone_in_hand = hand_index_for_card(&engine, 0, "clone");
    engine
        .apply_command(0, &cast_spell(clone_in_hand, vec![]))
        .expect("cast Clone");
    pass_both_players(&mut engine);
    engine
        .apply_command(0, &submit_resolution_choice(vec![mastiff]))
        .expect("copy Pack Mastiff");
    let clone = battlefield_object_for_card(&engine, 0, "clone");
    assert_eq!(
        engine.state.objects[&clone]
            .copiable_values
            .as_ref()
            .expect("copy values")
            .face
            .name,
        "Pack Mastiff"
    );

    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            c: 1,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &activate_ability(mastiff, 0, vec![]))
        .expect("activate Pack Mastiff");
    pass_both_players(&mut engine);

    assert_eq!(engine.effective_power(mastiff), Some(3));
    assert_eq!(engine.effective_power(clone), Some(3));
    assert_eq!(engine.effective_power(other), Some(2));
    assert_eq!(engine.effective_power(opposing_mastiff), Some(2));

    engine.enable_dev_commands();
    engine
        .apply_command(0, &move_card(0, DevZone::Hand, "Pack Mastiff"))
        .expect("bounce Pack Mastiff");
    engine
        .apply_command(0, &move_card(0, DevZone::Battlefield, "Pack Mastiff"))
        .expect("return Pack Mastiff");
    assert_eq!(
        engine.effective_power(mastiff),
        Some(2),
        "a new zone-change generation must not retain the snapshot pump"
    );

    let late_mastiff = inject_creature_on_battlefield(&mut engine, 0, "pack_mastiff");
    assert_eq!(
        engine.effective_power(late_mastiff),
        Some(2),
        "a later entrant was not in the resolved effect's snapshot"
    );
}

#[test]
fn pridemalkin_rechecks_controller_and_counter_presence_continuously() {
    let decks = Some(vec![
        vec![
            "pridemalkin".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec!["mountain".into(); 7],
    ]);
    let mut engine = GameEngine::new(79_002, &[0, 1], 20, decks, true).expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    let ally = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opponent = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let card = hand_index_for_card(&engine, 0, "pridemalkin");
    engine
        .apply_command(0, &cast_spell(card, vec![]))
        .expect("cast Pridemalkin");
    pass_both_players(&mut engine);
    let pridemalkin = battlefield_object_for_card(&engine, 0, "pridemalkin");
    assert_eq!(engine.state.pending_triggers.len(), 1);
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                    decline: false,
                    targets: vec![TargetRef {
                        object_id: pridemalkin,
                        damage_amount: 0,
                        group_index: 0,
                        kind: 0,
                    }],
                })),
            },
        )
        .expect("Pridemalkin may target itself");
    pass_both_players(&mut engine);

    assert_eq!(
        engine.state.objects[&pridemalkin].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert!(engine.effective_has_keyword(pridemalkin, Keyword::Trample));
    assert!(!engine.effective_has_keyword(ally, Keyword::Trample));

    engine
        .state
        .objects
        .get_mut(&ally)
        .expect("ally")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    engine
        .state
        .objects
        .get_mut(&opponent)
        .expect("opponent")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    assert!(engine.effective_has_keyword(ally, Keyword::Trample));
    assert!(!engine.effective_has_keyword(opponent, Keyword::Trample));

    engine
        .state
        .objects
        .get_mut(&ally)
        .expect("ally")
        .set_counter(CounterKind::PlusOnePlusOne, 0);
    assert!(!engine.effective_has_keyword(ally, Keyword::Trample));
}
