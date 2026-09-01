use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChoiceKind, ChooseTriggerTarget, RuledCommand, TargetRef, TargetRefKind,
};

fn choose_trigger_target(object_id: u32, kind: TargetRefKind) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: vec![TargetRef {
                object_id,
                kind: kind as i32,
                ..Default::default()
            }],
        })),
    }
}

fn repartee_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "plains",
            &[card, "giant_growth", "grizzly_bears", "storm_crow"],
        ),
        deck_with("island", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn cast_giant_growth_at(engine: &mut GameEngine, target: u32) -> RuledEventBatch {
    ensure_in_hand(engine, 0, "giant_growth");
    grant_pool(engine, 0);
    let slot = hand_index_for_card(engine, 0, "giant_growth");
    engine
        .apply_command(0, &cast_spell(slot, target_object(target)))
        .expect("cast Giant Growth at a creature")
}

#[test]
fn graduation_day_publishes_and_revalidates_its_own_creature_target() {
    let mut engine = repartee_engine(191_001, "graduation_day");
    relocate_to_battlefield(&mut engine, 0, "graduation_day", false);
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    let batch = cast_giant_growth_at(&mut engine, opposing);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    let candidates = batch.legal_by_player[&0]
        .valid_targets_by_ability
        .values()
        .next()
        .expect("Graduation Day target candidates");
    assert_eq!(candidates.groups[0].valid_permanent_ids, [own]);
    assert!(engine
        .apply_command(
            0,
            &choose_trigger_target(opposing, TargetRefKind::Permanent),
        )
        .is_err());
    engine
        .apply_command(0, &choose_trigger_target(own, TargetRefKind::Permanent))
        .expect("target own creature");

    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&own].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "the Repartee trigger resolves before Giant Growth"
    );
    assert_eq!(engine.state.stack.len(), 1, "the originating spell remains");
}

#[test]
fn forum_necroscribe_returns_only_a_current_own_graveyard_creature() {
    let mut engine = repartee_engine(191_002, "forum_necroscribe");
    relocate_to_battlefield(&mut engine, 0, "forum_necroscribe", false);
    let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    let departed = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let surviving = inject_graveyard_card(&mut engine, 0, "storm_crow");
    let opposing = inject_graveyard_card(&mut engine, 1, "grizzly_bears");

    let batch = cast_giant_growth_at(&mut engine, target);
    let candidates = batch.legal_by_player[&0]
        .valid_targets_by_ability
        .values()
        .next()
        .expect("Forum Necroscribe target candidates");
    assert_eq!(
        candidates.groups[0].valid_graveyard_ids,
        [departed, surviving]
    );
    assert!(engine
        .apply_command(
            0,
            &choose_trigger_target(opposing, TargetRefKind::Graveyard),
        )
        .is_err());

    engine.state.players[0]
        .graveyard
        .retain(|object_id| *object_id != departed);
    engine.state.players[0].hand.push(departed);
    engine.state.objects.get_mut(&departed).unwrap().zone = Zone::Hand;
    assert!(engine
        .apply_command(
            0,
            &choose_trigger_target(departed, TargetRefKind::Graveyard),
        )
        .is_err());
    engine
        .apply_command(
            0,
            &choose_trigger_target(surviving, TargetRefKind::Graveyard),
        )
        .expect("choose a current own creature card");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&surviving].zone, Zone::Battlefield);
}

#[test]
fn forum_necroscribe_ward_uses_the_existing_private_discard_payment() {
    let decks = Some(vec![
        deck_with("island", &["unsummon", "grizzly_bears"]),
        deck_with("swamp", &["forum_necroscribe"]),
    ]);
    let mut engine = GameEngine::new(191_003, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let forum = relocate_to_battlefield(&mut engine, 1, "forum_necroscribe", false);
    let discard = relocate_to_hand(&mut engine, 0, "grizzly_bears");
    ensure_in_hand(&mut engine, 0, "unsummon");
    grant_pool(&mut engine, 0);
    let unsummon = hand_index_for_card(&engine, 0, "unsummon");
    let spell_id = engine.state.players[0].hand[unsummon];
    engine
        .apply_command(0, &cast_spell(unsummon, target_object(forum)))
        .expect("target Forum Necroscribe");
    assert_eq!(engine.state.stack.len(), 2, "Ward is above Unsummon");

    pass_both_players(&mut engine);
    let pending = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("Ward payment");
    assert_eq!(pending.presentation.choice_kind, ChoiceKind::HandCards);
    assert!(pending.presentation.candidates.contains(&discard));
    engine
        .apply_command(0, &submit_resolution_choice(vec![discard]))
        .expect("discard to pay Ward");
    assert_eq!(engine.state.objects[&discard].zone, Zone::Graveyard);
    assert!(engine.state.stack.iter().any(|item| item.id == spell_id));
}
