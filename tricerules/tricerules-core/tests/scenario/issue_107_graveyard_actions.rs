use super::helpers::*;
use crate::helpers::ScenarioCommandExt;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, RuledCommand, TargetRef,
};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        std::iter::repeat_n("forest".to_string(), 20).collect(),
        std::iter::repeat_n("island".to_string(), 20).collect(),
    ]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn three_player_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![vec!["forest".to_string(); 20]; 3]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1, 2],
        20,
        decks,
        true,
    )
    .expect("three-player engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn grouped(ids: &[u32]) -> Vec<TargetRef> {
    ids.iter()
        .copied()
        .map(|object_id| TargetRef {
            object_id,
            group_index: 0,
            ..Default::default()
        })
        .collect()
}

fn choose_trigger_targets(ids: &[u32]) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: grouped(ids),
        })),
    }
}

fn cast_creature(engine: &mut GameEngine, card_id: &str) -> u32 {
    let object_id = inject_card_into_hand(engine, 0, card_id);
    grant_pool(engine, 0);
    let index = hand_index_for_card(engine, 0, card_id);
    engine
        .apply_scenario_command(0, &cast_spell(index, Vec::new()))
        .unwrap_or_else(|error| panic!("cast {card_id}: {error}"));
    pass_both_players(engine);
    object_id
}

fn soul_zombie_fixture(seed: u64, cards: &[&str]) -> (GameEngine, u32, Vec<u32>) {
    let mut engine = engine(seed);
    let targets = cards
        .iter()
        .map(|card| inject_graveyard_card(&mut engine, 1, card))
        .collect();
    let zombie = cast_creature(&mut engine, "soul-shackled_zombie");
    (engine, zombie, targets)
}

fn move_graveyard_card_to_hand(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .graveyard
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].hand.push(object_id);
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("graveyard fixture card")
        .zone = Zone::Hand;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn move_hand_card_to_graveyard(engine: &mut GameEngine, player: usize, object_id: u32) {
    engine.state.players[player]
        .hand
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].graveyard.push(object_id);
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("hand fixture card")
        .zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(object_id)
        .or_default() += 1;
}

fn assert_atomic_rejection(engine: &mut GameEngine, command: &RuledCommand) {
    let before = engine.diagnostic_snapshot().expect("before rejection");
    assert!(engine.apply_scenario_command(0, command).is_err());
    assert_eq!(
        engine.diagnostic_snapshot().expect("after rejection"),
        before,
        "an invalid target set leaves all engine state unchanged"
    );
}

#[test]
fn arashin_sunshield_requires_one_graveyard_and_revalidates_each_target() {
    let mut engine = engine(107_001);
    let own_first = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let own_second = inject_graveyard_card(&mut engine, 0, "storm_crow");
    let opposing = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    cast_creature(&mut engine, "arashin_sunshield");

    let published = engine.initial_response_batch();
    let targets = published.legal_by_player[&0]
        .valid_targets_by_ability
        .values()
        .next()
        .expect("pending trigger targets");
    assert!(targets.groups[0].same_graveyard);
    assert_eq!(targets.groups[0].min, 0);
    assert_eq!(targets.groups[0].max, 2);
    assert!(engine
        .apply_scenario_command(0, &choose_trigger_targets(&[own_first, opposing]))
        .is_err());
    engine
        .apply_scenario_command(0, &choose_trigger_targets(&[own_first, own_second]))
        .expect("choose two cards from one graveyard");

    engine.state.players[0]
        .graveyard
        .retain(|object_id| *object_id != own_second);
    engine.state.players[0].hand.push(own_second);
    engine
        .state
        .objects
        .get_mut(&own_second)
        .expect("card")
        .zone = Zone::Hand;
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&own_first].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&own_second].zone, Zone::Hand);
}

#[test]
fn graveyard_actions_place_cards_at_the_requested_library_end() {
    let mut top = engine(107_010);
    let top_card = inject_graveyard_card(&mut top, 0, "lightning_bolt");
    cast_creature(&mut top, "monastery_messenger");
    top.apply_scenario_command(0, &choose_trigger_targets(&[top_card]))
        .expect("choose top card");
    pass_both_players(&mut top);
    assert_eq!(top.state.players[0].library.front(), Some(&top_card));

    let mut bottom = engine(107_011);
    let bottom_card = inject_graveyard_card(&mut bottom, 1, "grizzly_bears");
    let chandelier = inject_creature_on_battlefield(&mut bottom, 0, "malevolent_chandelier");
    grant_pool(&mut bottom, 0);
    bottom
        .apply_scenario_command(0, &activate_ability(chandelier, 0, grouped(&[bottom_card])))
        .expect("activate Chandelier");
    pass_both_players(&mut bottom);
    assert_eq!(bottom.state.players[1].library.back(), Some(&bottom_card));
}

#[test]
fn soul_shackled_zombie_drains_only_when_a_creature_was_actually_exiled() {
    let mut drain = engine(107_020);
    let creature = inject_graveyard_card(&mut drain, 1, "grizzly_bears");
    let noncreature = inject_graveyard_card(&mut drain, 1, "forest");
    drain.state.add_damage_prevention_shield(1, 2);
    cast_creature(&mut drain, "soul-shackled_zombie");
    drain
        .apply_scenario_command(0, &choose_trigger_targets(&[creature, noncreature]))
        .expect("choose graveyard cohort");
    pass_both_players(&mut drain);
    assert_eq!(drain.state.players[0].life, 22);
    assert_eq!(drain.state.players[1].life, 18);
    assert_eq!(
        drain.state.remaining_damage_prevention(1),
        2,
        "losing life does not use or consume a damage prevention shield"
    );

    let mut no_drain = engine(107_021);
    let stale_creature = inject_graveyard_card(&mut no_drain, 1, "grizzly_bears");
    let surviving_noncreature = inject_graveyard_card(&mut no_drain, 1, "forest");
    cast_creature(&mut no_drain, "soul-shackled_zombie");
    no_drain
        .apply_scenario_command(
            0,
            &choose_trigger_targets(&[stale_creature, surviving_noncreature]),
        )
        .expect("choose graveyard cohort");
    no_drain.state.players[1]
        .graveyard
        .retain(|object_id| *object_id != stale_creature);
    no_drain.state.players[1].hand.push(stale_creature);
    no_drain
        .state
        .objects
        .get_mut(&stale_creature)
        .expect("card")
        .zone = Zone::Hand;
    pass_both_players(&mut no_drain);
    assert_eq!(
        no_drain.state.objects[&surviving_noncreature].zone,
        Zone::Exile
    );
    assert_eq!(no_drain.state.players[0].life, 20);
    assert_eq!(no_drain.state.players[1].life, 20);
}

#[test]
fn soul_shackled_zombie_allows_zero_targets_and_does_not_drain_for_two_noncreatures() {
    let (mut zero, _, zero_targets) = soul_zombie_fixture(107_022, &["grizzly_bears"]);
    let unchosen = zero_targets[0];
    zero.apply_scenario_command(0, &choose_trigger_targets(&[]))
        .expect("zero targets is legal");
    pass_both_players(&mut zero);
    assert_eq!(zero.state.objects[&unchosen].zone, Zone::Graveyard);
    assert_eq!(zero.state.players[0].life, 20);
    assert_eq!(zero.state.players[1].life, 20);

    let (mut noncreatures, _, noncreature_targets) =
        soul_zombie_fixture(107_023, &["forest", "island"]);
    let (forest, island) = (noncreature_targets[0], noncreature_targets[1]);
    noncreatures
        .apply_scenario_command(0, &choose_trigger_targets(&[forest, island]))
        .expect("choose two noncreature cards");
    pass_both_players(&mut noncreatures);
    assert_eq!(noncreatures.state.objects[&forest].zone, Zone::Exile);
    assert_eq!(noncreatures.state.objects[&island].zone, Zone::Exile);
    assert_eq!(noncreatures.state.players[0].life, 20);
    assert_eq!(noncreatures.state.players[1].life, 20);
}

#[test]
fn soul_shackled_zombie_drains_when_only_the_creature_target_survives() {
    let (mut engine, _, targets) = soul_zombie_fixture(107_024, &["grizzly_bears", "forest"]);
    let (creature, noncreature) = (targets[0], targets[1]);
    engine
        .apply_scenario_command(0, &choose_trigger_targets(&[creature, noncreature]))
        .expect("choose a creature and noncreature");
    move_graveyard_card_to_hand(&mut engine, 1, noncreature);
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&creature].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&noncreature].zone, Zone::Hand);
    assert_eq!(engine.state.players[0].life, 22);
    assert_eq!(engine.state.players[1].life, 18);
}

#[test]
fn soul_shackled_zombie_all_illegal_targets_do_not_resolve_and_rejections_are_atomic() {
    let (mut all_illegal, zombie, targets) =
        soul_zombie_fixture(107_025, &["grizzly_bears", "forest"]);
    let (creature, noncreature) = (targets[0], targets[1]);
    all_illegal
        .apply_scenario_command(0, &choose_trigger_targets(&[creature, noncreature]))
        .expect("choose two targets");
    move_graveyard_card_to_hand(&mut all_illegal, 1, creature);
    move_graveyard_card_to_hand(&mut all_illegal, 1, noncreature);
    pass_both_players(&mut all_illegal);
    assert!(all_illegal.state.stack.is_empty());
    assert_eq!(all_illegal.state.objects[&zombie].zone, Zone::Battlefield);
    assert_eq!(all_illegal.state.players[0].life, 20);
    assert_eq!(all_illegal.state.players[1].life, 20);

    let (mut cross_graveyard, _, targets) = soul_zombie_fixture(107_026, &["grizzly_bears"]);
    let first = targets[0];
    let second = inject_graveyard_card(&mut cross_graveyard, 0, "forest");
    assert_atomic_rejection(
        &mut cross_graveyard,
        &choose_trigger_targets(&[first, second]),
    );

    let (mut duplicate, _, targets) = soul_zombie_fixture(107_027, &["grizzly_bears"]);
    let first = targets[0];
    assert_atomic_rejection(&mut duplicate, &choose_trigger_targets(&[first, first]));

    let (mut over_count, _, targets) =
        soul_zombie_fixture(107_028, &["grizzly_bears", "storm_crow"]);
    let (first, second) = (targets[0], targets[1]);
    let third = inject_graveyard_card(&mut over_count, 1, "forest");
    assert_atomic_rejection(
        &mut over_count,
        &choose_trigger_targets(&[first, second, third]),
    );
}

#[test]
fn soul_shackled_zombie_rejects_the_same_object_after_a_graveyard_round_trip() {
    let (mut engine, _, targets) = soul_zombie_fixture(107_029, &["grizzly_bears", "forest"]);
    let (creature, noncreature) = (targets[0], targets[1]);
    let selected_generation = engine
        .state
        .zone_change_generation
        .get(&creature)
        .copied()
        .unwrap_or_default();
    engine
        .apply_scenario_command(0, &choose_trigger_targets(&[creature, noncreature]))
        .expect("choose a creature and noncreature");
    move_graveyard_card_to_hand(&mut engine, 1, creature);
    move_hand_card_to_graveyard(&mut engine, 1, creature);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Graveyard);
    assert!(
        engine.state.zone_change_generation[&creature] > selected_generation,
        "the same OID has a new zone-change generation after leaving and returning"
    );
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&creature].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&noncreature].zone, Zone::Exile);
    assert_eq!(engine.state.players[0].life, 20);
    assert_eq!(engine.state.players[1].life, 20);
}

#[test]
fn soul_shackled_zombie_etb_resolves_after_its_source_leaves() {
    let (mut engine, zombie, targets) = soul_zombie_fixture(107_032, &["grizzly_bears", "forest"]);
    let (creature, noncreature) = (targets[0], targets[1]);
    engine
        .apply_scenario_command(0, &choose_trigger_targets(&[creature, noncreature]))
        .expect("choose a creature and noncreature");
    engine.state.players[0]
        .battlefield
        .retain(|candidate| *candidate != zombie);
    engine.state.players[0].graveyard.push(zombie);
    engine
        .state
        .objects
        .get_mut(&zombie)
        .expect("source Zombie")
        .zone = Zone::Graveyard;
    *engine
        .state
        .zone_change_generation
        .entry(zombie)
        .or_default() += 1;
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&zombie].zone, Zone::Graveyard);
    assert_eq!(engine.state.objects[&creature].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&noncreature].zone, Zone::Exile);
    assert_eq!(engine.state.players[0].life, 22);
    assert_eq!(engine.state.players[1].life, 18);
}

#[test]
fn soul_shackled_zombie_three_player_cast_replays_the_same_events_and_life_changes() {
    fn setup(seed: u64) -> (GameEngine, u32, u32, u32) {
        let mut engine = three_player_engine(seed);
        let first_creature = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
        let second_creature = inject_graveyard_card(&mut engine, 1, "storm_crow");
        let zombie = inject_card_into_hand(&mut engine, 0, "soul-shackled_zombie");
        grant_pool(&mut engine, 0);
        (engine, zombie, first_creature, second_creature)
    }

    let seed = 107_033;
    let (mut first, zombie, first_creature, second_creature) = setup(seed);
    let (mut replay, replay_zombie, replay_first, replay_second) = setup(seed);
    assert_eq!(
        (zombie, first_creature, second_creature),
        (replay_zombie, replay_first, replay_second),
        "the same deterministic fixture starts with the same engine object IDs"
    );

    let mut accepted_commands = Vec::new();
    let mut first_batches = Vec::new();
    let cast = cast_spell(
        hand_index_for_card(&first, 0, "soul-shackled_zombie"),
        Vec::new(),
    );
    first_batches.push(
        first
            .apply_scenario_command(0, &cast)
            .expect("cast Soul-Shackled Zombie"),
    );
    accepted_commands.push((0, cast));
    for _ in 0..3 {
        let actor = first.state.priority_player_id();
        let command = pass();
        first_batches.push(
            first
                .apply_scenario_command(actor, &command)
                .expect("pass priority around the creature spell"),
        );
        accepted_commands.push((actor, command));
    }
    let first_offer = first.initial_response_batch();
    assert!(
        first_offer.legal_by_player[&0]
            .valid_targets_by_ability
            .values()
            .any(|targets| targets.groups[0].same_graveyard),
        "the ETB offers its same-graveyard target group"
    );
    let selection_boundary = accepted_commands.len();
    let choose = choose_trigger_targets(&[first_creature, second_creature]);
    first_batches.push(
        first
            .apply_scenario_command(0, &choose)
            .expect("choose both creature cards"),
    );
    accepted_commands.push((0, choose));
    for _ in 0..3 {
        let actor = first.state.priority_player_id();
        let command = pass();
        first_batches.push(
            first
                .apply_scenario_command(actor, &command)
                .expect("pass priority around the ETB"),
        );
        accepted_commands.push((actor, command));
    }

    let mut replay_batches = Vec::new();
    for (index, (actor, command)) in accepted_commands.iter().enumerate() {
        if index == selection_boundary {
            assert_eq!(
                replay.initial_response_batch(),
                first_offer,
                "replaying accepted commands reproduces the same public target offer"
            );
        }
        replay_batches.push(
            replay
                .apply_scenario_command(*actor, command)
                .expect("replay every accepted cast, target, and pass command"),
        );
    }
    assert_eq!(replay_batches, first_batches);
    assert_eq!(
        replay.diagnostic_snapshot().unwrap(),
        first.diagnostic_snapshot().unwrap()
    );

    assert_eq!(first.state.objects[&zombie].zone, Zone::Battlefield);
    assert_eq!(first.characteristics(zombie).unwrap().power, Some(4));
    assert_eq!(first.characteristics(zombie).unwrap().toughness, Some(2));
    assert_eq!(first.state.objects[&first_creature].zone, Zone::Exile);
    assert_eq!(first.state.objects[&second_creature].zone, Zone::Exile);
    assert_eq!(first.state.players[0].life, 22);
    assert_eq!(first.state.players[1].life, 18);
    assert_eq!(first.state.players[2].life, 18);

    let final_batch = first_batches
        .last()
        .expect("the last pass resolves the ETB");
    let mut changes = life_changes_in(final_batch)
        .into_iter()
        .map(|change| (change.player_id, change.delta))
        .collect::<Vec<_>>();
    changes.sort_unstable();
    assert_eq!(changes, vec![(0, 2), (1, -2), (2, -2)]);
}

#[test]
fn rooftop_percher_accepts_targets_from_different_graveyards() {
    let mut engine = engine(107_030);
    let own = inject_graveyard_card(&mut engine, 0, "grizzly_bears");
    let opposing = inject_graveyard_card(&mut engine, 1, "storm_crow");
    cast_creature(&mut engine, "rooftop_percher");

    let published = engine.initial_response_batch();
    let targets = published.legal_by_player[&0]
        .valid_targets_by_ability
        .values()
        .next()
        .expect("pending Rooftop Percher targets");
    assert_eq!((targets.groups[0].min, targets.groups[0].max), (0, 2));
    assert!(!targets.groups[0].same_graveyard);
    engine
        .apply_scenario_command(0, &choose_trigger_targets(&[own, opposing]))
        .expect("choose one target from each graveyard");
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&own].zone, Zone::Exile);
    assert_eq!(engine.state.objects[&opposing].zone, Zone::Exile);
    assert_eq!(engine.state.players[0].life, 23);
}

#[test]
fn rooftop_percher_can_choose_no_targets_and_still_gain_life() {
    let mut engine = engine(107_031);
    let unchosen = inject_graveyard_card(&mut engine, 1, "grizzly_bears");
    cast_creature(&mut engine, "rooftop_percher");
    engine
        .apply_scenario_command(0, &choose_trigger_targets(&[]))
        .expect("decline all optional targets");
    pass_both_players(&mut engine);

    assert_eq!(engine.state.objects[&unchosen].zone, Zone::Graveyard);
    assert_eq!(engine.state.players[0].life, 23);
}
