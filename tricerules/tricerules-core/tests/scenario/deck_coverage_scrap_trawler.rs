//! Actual-card coverage for Scrap Trawler's event-object mana-value comparison.
use super::helpers::*;
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, DevCommand, DevMoveCard, DevZone, RuledCommand,
    TargetRef, TargetRefKind,
};

fn game(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("island", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    grant_pool(&mut engine, 0);
    grant_pool(&mut engine, 1);
    engine
}

fn game_with_cursed_mirror(seed: u64) -> GameEngine {
    let mirror_deck = deck_with("mountain", &["cursed_mirror", "clone"]);
    let base_deck = deck_with("mountain", &[]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![mirror_deck, base_deck]),
        true,
    )
    .expect("new game with copy fixtures");
    advance_to_main1_from_game_start(&mut engine);
    grant_pool(&mut engine, 0);
    grant_pool(&mut engine, 1);
    engine
}

fn choose_graveyard_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets: vec![TargetRef {
                object_id,
                group_index: 0,
                kind: TargetRefKind::Graveyard as i32,
                ..Default::default()
            }],
            ..Default::default()
        })),
    }
}

fn cast_wrath_of_god(engine: &mut GameEngine) {
    inject_card_into_hand(engine, 0, "wrath_of_god");
    let slot = hand_index_for_card(engine, 0, "wrath_of_god");
    engine
        .apply_command(0, &cast_spell(slot, Vec::new()))
        .expect("cast Wrath of God");
    pass_both_players(engine);
}

fn dev_move_card(engine: &mut GameEngine, target_player: i32, card_name: &str, zone: DevZone) {
    engine.enable_dev_commands();
    let actor = engine.state.priority_player_id();
    engine
        .apply_command(
            actor,
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: target_player,
                    dev: Some(tricerules_proto::ruled::v1::dev_command::Dev::MoveCard(
                        DevMoveCard {
                            card_name: card_name.into(),
                            zone: zone as i32,
                            ready: false,
                        },
                    )),
                })),
            },
        )
        .expect("move the exact existing card through the logged dev command");
}

fn graveyard_candidates(engine: &mut GameEngine, player: i32, key: u64) -> Vec<u32> {
    engine.initial_response_batch().legal_by_player[&player].valid_targets_by_ability[&key].groups
        [0]
    .valid_graveyard_ids
    .clone()
}

fn assert_ids_equal(mut actual: Vec<u32>, mut expected: Vec<u32>, reason: &str) {
    actual.sort_unstable();
    expected.sort_unstable();
    assert_eq!(actual, expected, "{reason}");
}

#[test]
fn scrap_trawler_self_death_uses_its_exact_event_mana_value() {
    let mut engine = game(2_026_100_901);
    let trawler = inject_creature_on_battlefield(&mut engine, 0, "scrap_trawler");
    let lower = inject_graveyard_card(&mut engine, 0, "myr_retriever");
    let equal = inject_graveyard_card(&mut engine, 0, "sculpting_steel");
    let greater = inject_graveyard_card(&mut engine, 0, "solemn_simulacrum");
    let nonartifact = inject_graveyard_card(&mut engine, 0, "island");
    let opponent_artifact = inject_graveyard_card(&mut engine, 1, "chromatic_star");

    let old_generation = engine
        .state
        .zone_change_generation
        .get(&trawler)
        .copied()
        .unwrap_or(0);
    cast_wrath_of_god(&mut engine);

    assert_eq!(engine.state.objects[&trawler].zone, Zone::Graveyard);
    assert_eq!(
        engine.state.zone_change_generation[&trawler],
        old_generation + 1
    );
    assert_eq!(engine.state.pending_triggers.len(), 1);
    let observed = engine.state.pending_triggers[0]
        .trigger_context
        .observed_object
        .expect("self-death trigger carries the dying incarnation");
    assert_eq!(observed.object_id, trawler);
    assert_eq!(observed.zone_change_generation, old_generation);
    let legal = engine.initial_response_batch();
    let ability_key = u64::from(trawler) << 32;
    let candidates = &legal.legal_by_player[&0].valid_targets_by_ability[&ability_key].groups[0]
        .valid_graveyard_ids;
    let mut actual = candidates.clone();
    actual.sort_unstable();
    let mut expected = vec![lower];
    expected.sort_unstable();
    assert_eq!(
        actual, expected,
        "only a lower-value artifact in your graveyard is legal"
    );
    assert!(
        !candidates.contains(&equal),
        "equal mana value is not lower"
    );
    assert!(
        !candidates.contains(&greater),
        "greater mana value is not lower"
    );
    assert!(
        !candidates.contains(&trawler),
        "its own event has equal mana value"
    );
    assert!(!candidates.contains(&nonartifact));
    assert!(!candidates.contains(&opponent_artifact));
}

#[test]
fn scrap_trawler_observes_noncreature_artifacts_and_uses_the_event_value() {
    let mut engine = game(2_026_100_902);
    let trawler = inject_creature_on_battlefield(&mut engine, 0, "scrap_trawler");
    let event_artifact = inject_permanent_on_battlefield(&mut engine, 0, "trading_post");
    let x_target = inject_graveyard_card(&mut engine, 0, "astral_cornucopia");
    let one_target = inject_graveyard_card(&mut engine, 0, "chromatic_star");
    let two_target = inject_graveyard_card(&mut engine, 0, "myr_retriever");
    let three_target = inject_graveyard_card(&mut engine, 0, "sculpting_steel");
    let higher_target = inject_graveyard_card(&mut engine, 0, "solemn_simulacrum");
    let nonartifact = inject_graveyard_card(&mut engine, 0, "island");
    let opponent_artifact = inject_graveyard_card(&mut engine, 1, "chromatic_star");

    dev_move_card(&mut engine, 0, "Trading Post", DevZone::Graveyard);
    assert_eq!(engine.state.objects[&event_artifact].zone, Zone::Graveyard);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    assert_eq!(
        engine.state.pending_triggers[0]
            .trigger_context
            .observed_object
            .unwrap()
            .object_id,
        event_artifact
    );

    let key = (u64::from(trawler) << 32) | 1;
    let candidates = graveyard_candidates(&mut engine, 0, key);
    assert_ids_equal(
        candidates.clone(),
        vec![x_target, one_target, two_target, three_target],
        "Trading Post's mana value 4 permits a target that is not below Scrap Trawler's 3",
    );
    assert!(
        !candidates.contains(&event_artifact),
        "equal event value is illegal"
    );
    assert!(!candidates.contains(&higher_target));
    assert!(!candidates.contains(&nonartifact));
    assert!(!candidates.contains(&opponent_artifact));

    engine
        .apply_command(0, &choose_graveyard_target(three_target))
        .expect("choose the artifact below the event object's value");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&three_target].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&trawler].zone, Zone::Battlefield);
}

#[test]
fn scrap_trawler_tracks_the_controller_of_an_opponent_owned_artifact_event() {
    let mut engine = game(2_026_100_903);
    let trawler = inject_creature_on_battlefield(&mut engine, 0, "scrap_trawler");
    let foreign_artifact =
        inject_creature_under_foreign_control(&mut engine, 1, 0, "solemn_simulacrum");
    let controller_target = inject_graveyard_card(&mut engine, 0, "sculpting_steel");
    let owner_target = inject_graveyard_card(&mut engine, 1, "myr_retriever");

    dev_move_card(&mut engine, 0, "Solemn Simulacrum", DevZone::Graveyard);
    assert_eq!(
        engine.state.objects[&foreign_artifact].zone,
        Zone::Graveyard
    );
    assert!(engine.state.players[1]
        .graveyard
        .contains(&foreign_artifact));
    assert_eq!(engine.state.pending_triggers.len(), 1);

    let key = (u64::from(trawler) << 32) | 1;
    let candidates = graveyard_candidates(&mut engine, 0, key);
    assert_eq!(candidates, [controller_target]);
    assert!(!candidates.contains(&owner_target));
    engine
        .apply_command(0, &choose_graveyard_target(controller_target))
        .expect("the controller chooses a card from their own graveyard");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&controller_target].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&owner_target].zone, Zone::Graveyard);
}

#[test]
fn scrap_trawler_simultaneous_events_keep_three_independent_thresholds() {
    let mut engine = game(2_026_100_904);
    let trawler = inject_creature_on_battlefield(&mut engine, 0, "scrap_trawler");
    // Wrath of God destroys creatures, so the MV-4 event must also be an artifact creature.
    // Trading Post's noncreature departure is covered by the preceding scenario.
    let four_mana_event = inject_creature_on_battlefield(&mut engine, 0, "phyrexian_metamorph");
    let battlesphere = inject_creature_on_battlefield(&mut engine, 0, "myr_battlesphere");
    let x_target = inject_graveyard_card(&mut engine, 0, "astral_cornucopia");
    let one_target = inject_graveyard_card(&mut engine, 0, "chromatic_star");
    let two_target = inject_graveyard_card(&mut engine, 0, "myr_retriever");
    let three_target = inject_graveyard_card(&mut engine, 0, "sculpting_steel");

    cast_wrath_of_god(&mut engine);
    let pending_order = engine.state.pending_trigger_order.as_ref().unwrap();
    assert_eq!(
        pending_order.candidates.len(),
        3,
        "expected one threshold per departing artifact; staged candidates: {:?}",
        pending_order.candidates
    );
    assert_eq!(engine.state.pending_triggers.len(), 0);

    let cases = [
        (
            trawler,
            3,
            one_target,
            vec![x_target, one_target, two_target],
        ),
        (
            four_mana_event,
            4,
            three_target,
            vec![x_target, one_target, two_target, trawler, three_target],
        ),
        (
            battlesphere,
            7,
            two_target,
            vec![
                x_target,
                one_target,
                two_target,
                trawler,
                three_target,
                four_mana_event,
            ],
        ),
    ];
    for (event_object, threshold, selected_target, expected_candidates) in cases {
        if engine.state.pending_triggers.is_empty() {
            let order = engine.state.pending_trigger_order.as_ref().unwrap();
            let staged = order
                .candidates
                .iter()
                .find(|candidate| {
                    candidate
                        .trigger_context
                        .observed_object
                        .is_some_and(|observed| observed.object_id == event_object)
                })
                .expect("the expected event has an ordering candidate");
            engine
                .apply_command(
                    order.deciding_player,
                    &submit_trigger_order(staged.object_id),
                )
                .expect("order the next event's trigger");
        }
        let pending = engine.state.pending_triggers.front().unwrap();
        let observed = pending
            .trigger_context
            .observed_object
            .expect("each trigger retains its own event object");
        assert_eq!(observed.object_id, event_object);
        assert_eq!(
            engine.state.zone_change_generation[&event_object],
            observed.zone_change_generation + 1
        );
        let key = (u64::from(trawler) << 32) | pending.ability_index as u64;
        assert_ids_equal(
            graveyard_candidates(&mut engine, 0, key),
            expected_candidates,
            &format!("event threshold is {threshold}"),
        );
        engine
            .apply_command(0, &choose_graveyard_target(selected_target))
            .expect("choose a target legal for this event only");
    }

    assert!(engine.state.pending_triggers.is_empty());
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&one_target].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&three_target].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&two_target].zone, Zone::Hand);
}

#[test]
fn scrap_trawler_uses_copied_event_value_and_zero_x_graveyard_value_after_reentry() {
    let mut engine = game_with_cursed_mirror(2_026_100_905);
    let trawler = inject_creature_on_battlefield(&mut engine, 0, "scrap_trawler");
    let battlesphere = inject_creature_on_battlefield(&mut engine, 0, "myr_battlesphere");
    let ornithopter = inject_creature_on_battlefield(&mut engine, 0, "ornithopter");
    let mirror = relocate_to_hand(&mut engine, 0, "cursed_mirror");
    let lower_than_copy = inject_graveyard_card(&mut engine, 0, "solemn_simulacrum");
    let x_target = inject_graveyard_card(&mut engine, 0, "astral_cornucopia");
    grant_pool(&mut engine, 0);

    engine
        .apply_command(
            0,
            &cast_spell(hand_index_for_card(&engine, 0, "cursed_mirror"), Vec::new()),
        )
        .expect("cast Cursed Mirror");
    pass_both_players(&mut engine);
    let deciding_player = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .deciding_player;
    engine
        .apply_command(
            deciding_player,
            &submit_resolution_choice(vec![battlesphere]),
        )
        .expect("Cursed Mirror copies Myr Battlesphere");
    assert_eq!(engine.characteristics(mirror).unwrap().mana_value, 7);

    let old_generation = engine.state.zone_change_generation[&mirror];
    dev_move_card(&mut engine, 0, "Cursed Mirror", DevZone::Graveyard);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    let pending = engine.state.pending_triggers.front().unwrap();
    assert_eq!(
        pending.trigger_context.observed_object.unwrap().object_id,
        mirror
    );
    assert_eq!(
        pending
            .trigger_context
            .observed_object
            .unwrap()
            .zone_change_generation,
        old_generation
    );
    let key = (u64::from(trawler) << 32) | 1;
    let candidates = graveyard_candidates(&mut engine, 0, key);
    assert!(
        candidates.contains(&lower_than_copy),
        "mana value 4 is below the copied 7"
    );
    assert!(candidates.contains(&x_target), "X is zero in the graveyard");
    engine
        .apply_command(0, &choose_graveyard_target(lower_than_copy))
        .expect("choose the card below the copied event value");

    dev_move_card(&mut engine, 0, "Cursed Mirror", DevZone::Battlefield);
    let deciding_player = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .deciding_player;
    engine
        .apply_command(
            deciding_player,
            &submit_resolution_choice(vec![ornithopter]),
        )
        .expect("the new Cursed Mirror incarnation copies Ornithopter");
    assert_eq!(engine.characteristics(mirror).unwrap().mana_value, 0);
    assert_eq!(
        engine.state.zone_change_generation[&mirror],
        old_generation + 2
    );

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&lower_than_copy].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&x_target].zone, Zone::Graveyard);
}

#[test]
fn scrap_trawler_rechecks_the_exact_target_generation_at_resolution() {
    let mut engine = game(2_026_100_906);
    let trawler = inject_creature_on_battlefield(&mut engine, 0, "scrap_trawler");
    let event_artifact = inject_permanent_on_battlefield(&mut engine, 0, "trading_post");
    let target = inject_graveyard_card(&mut engine, 0, "sculpting_steel");
    let original_generation = engine
        .state
        .zone_change_generation
        .get(&target)
        .copied()
        .unwrap_or(0);

    dev_move_card(&mut engine, 0, "Trading Post", DevZone::Graveyard);
    assert_eq!(engine.state.pending_triggers.len(), 1);
    let key = (u64::from(trawler) << 32) | 1;
    assert!(graveyard_candidates(&mut engine, 0, key).contains(&target));
    engine
        .apply_command(0, &choose_graveyard_target(target))
        .expect("choose the legal graveyard card");

    dev_move_card(&mut engine, 0, "Sculpting Steel", DevZone::Hand);
    dev_move_card(&mut engine, 0, "Sculpting Steel", DevZone::Graveyard);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(
        engine
            .state
            .zone_change_generation
            .get(&target)
            .copied()
            .unwrap_or(0),
        original_generation + 2
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&event_artifact].zone, Zone::Graveyard);
}

#[test]
fn scrap_trawler_removes_the_trigger_without_a_legal_target_and_ignores_exile() {
    let mut no_target = game(2_026_100_907);
    let trawler = inject_creature_on_battlefield(&mut no_target, 0, "scrap_trawler");
    let opponent_artifact = inject_graveyard_card(&mut no_target, 1, "chromatic_star");
    cast_wrath_of_god(&mut no_target);
    assert_eq!(no_target.state.objects[&trawler].zone, Zone::Graveyard);
    assert!(no_target.state.players[1]
        .graveyard
        .contains(&opponent_artifact));
    assert!(no_target.state.pending_triggers.is_empty());
    assert!(no_target.state.stack.is_empty());

    let mut exile = game(2_026_100_908);
    let t = inject_creature_on_battlefield(&mut exile, 0, "scrap_trawler");
    let event_artifact = inject_permanent_on_battlefield(&mut exile, 0, "trading_post");
    inject_graveyard_card(&mut exile, 0, "myr_retriever");
    dev_move_card(&mut exile, 0, "Trading Post", DevZone::Exile);
    assert_eq!(exile.state.objects[&event_artifact].zone, Zone::Exile);
    assert!(exile.state.pending_triggers.is_empty());
    assert_eq!(exile.state.objects[&t].zone, Zone::Battlefield);
}
