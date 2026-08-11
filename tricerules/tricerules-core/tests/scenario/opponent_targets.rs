use crate::helpers::*;
use tricerules_core::Zone;

fn choose_trigger_target(target_object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            target_object_id,
            decline: false,
        })),
    }
}

fn cast_glaring_aegis(e: &mut GameEngine, enchanted_creature: u32) {
    ensure_in_hand(e, 0, "glaring_aegis");
    give_mana(
        e,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(e, 0, "glaring_aegis");
    e.apply_command(
        0,
        &cast_spell(
            index,
            vec![TargetRef {
                object_id: enchanted_creature,
                damage_amount: 0,
            }],
        ),
    )
    .expect("cast Glaring Aegis");
    pass_both_players(e);
}

fn cast_rambunctious_mutt(e: &mut GameEngine) {
    ensure_in_hand(e, 0, "rambunctious_mutt");
    give_mana(
        e,
        0,
        ManaGift {
            w: 2,
            c: 3,
            ..Default::default()
        },
    );
    let index = hand_index_for_card(e, 0, "rambunctious_mutt");
    e.apply_command(0, &cast_spell(index, vec![]))
        .expect("cast Rambunctious Mutt");
    pass_both_players(e);
}

fn opponent_target_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[card]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

#[test]
fn glaring_aegis_rejects_own_trigger_target_and_taps_opponents_creature() {
    let mut e = opponent_target_engine(96901, "glaring_aegis");
    let own_bear = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opponent_bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    cast_glaring_aegis(&mut e, own_bear);

    let aegis = battlefield_object_for_card(&e, 0, "glaring_aegis");
    assert_eq!(
        e.state.objects.get(&aegis).expect("Aegis").attached_to,
        Some(own_bear)
    );
    let enchanted = e.characteristics(own_bear).expect("enchanted bear");
    assert_eq!((enchanted.power, enchanted.toughness), (Some(3), Some(5)));
    assert_eq!(e.state.pending_triggers.len(), 1);

    let err = e
        .apply_command(0, &choose_trigger_target(own_bear))
        .expect_err("the trigger cannot target its controller's creature");
    assert!(err.to_string().contains("target"), "unexpected: {err}");
    assert_eq!(e.state.pending_triggers.len(), 1);

    e.apply_command(0, &choose_trigger_target(opponent_bear))
        .expect("choose opponent's creature");
    pass_both_players(&mut e);
    assert!(
        e.state
            .objects
            .get(&opponent_bear)
            .expect("opponent bear")
            .tapped,
        "Glaring Aegis must tap the opponent-controlled creature"
    );
}

#[test]
fn rambunctious_mutt_combines_opponent_and_permanent_type_filters() {
    for (seed, target_card) in [(96902, "icy_manipulator"), (96903, "exploration")] {
        let mut e = opponent_target_engine(seed, "rambunctious_mutt");
        let own_artifact = inject_permanent_on_battlefield(&mut e, 0, "icy_manipulator");
        let opponent_creature = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        let legal_target = inject_permanent_on_battlefield(&mut e, 1, target_card);

        cast_rambunctious_mutt(&mut e);
        assert_eq!(e.state.pending_triggers.len(), 1);

        assert!(
            e.apply_command(0, &choose_trigger_target(own_artifact))
                .is_err(),
            "an artifact controlled by the trigger controller is illegal"
        );
        assert!(
            e.apply_command(0, &choose_trigger_target(opponent_creature))
                .is_err(),
            "an opponent-controlled creature is still the wrong permanent type"
        );
        assert_eq!(e.state.pending_triggers.len(), 1);

        e.apply_command(0, &choose_trigger_target(legal_target))
            .expect("choose opponent-controlled artifact or enchantment");
        pass_both_players(&mut e);
        assert_eq!(
            e.state.objects.get(&legal_target).expect("target").zone,
            Zone::Graveyard,
            "Rambunctious Mutt must destroy {target_card}"
        );
    }
}

#[test]
fn rambunctious_mutt_trigger_is_removed_when_no_legal_target_exists() {
    let mut e = opponent_target_engine(96904, "rambunctious_mutt");
    inject_permanent_on_battlefield(&mut e, 0, "icy_manipulator");
    inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    cast_rambunctious_mutt(&mut e);

    assert!(e.state.pending_triggers.is_empty());
    assert!(e.state.stack.is_empty());
    let mutt = battlefield_object_for_card(&e, 0, "rambunctious_mutt");
    assert_eq!(
        e.state.objects.get(&mutt).expect("Mutt").zone,
        Zone::Battlefield
    );
}

#[test]
fn opponent_controlled_target_becomes_illegal_after_control_changes() {
    let mut e = opponent_target_engine(96905, "rambunctious_mutt");
    let artifact = inject_permanent_on_battlefield(&mut e, 1, "icy_manipulator");

    cast_rambunctious_mutt(&mut e);
    e.apply_command(0, &choose_trigger_target(artifact))
        .expect("choose opponent-controlled artifact");

    e.state.players[1]
        .battlefield
        .retain(|oid| *oid != artifact);
    e.state.players[0].battlefield.push(artifact);
    let artifact_object = e.state.objects.get_mut(&artifact).expect("artifact");
    artifact_object.base_controller = 0;
    artifact_object.controller = 0;

    pass_both_players(&mut e);
    assert_eq!(
        e.state.objects.get(&artifact).expect("artifact").zone,
        Zone::Battlefield
    );
    assert!(e.state.players[0].battlefield.contains(&artifact));
    assert!(e.state.stack.is_empty());
}
