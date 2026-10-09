//! The Mycosynth Gardens: engine-published X/target pairs and exact permanent copy behavior.
use super::helpers::*;
use tricerules_cards::primitives::{
    ContinuousEffectKind, ControllerReference, EffectDuration, PermanentTypeFilter,
    TypeLineReplacement,
};
use tricerules_cards::CounterKind;
use tricerules_core::state::{AffectedScope, ContinuousEffect};
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{
    DevCommand, DevMoveCard, DevZone, RuledCommand, TargetRef, TargetRefKind,
};

fn game(seed: u64) -> GameEngine {
    game_with_specials(seed, &[])
}

fn game_with_specials(seed: u64, specials: &[&str]) -> GameEngine {
    let deck = deck_with("mountain", specials);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck; 2]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
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
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn dev_move_card(
    engine: &mut GameEngine,
    target_player: i32,
    card_name: &str,
    zone: DevZone,
    ready: bool,
) {
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
                            ready,
                        },
                    )),
                })),
            },
        )
        .expect("move the existing card through the logged dev command");
}

fn copy_command(engine: &GameEngine, source: u32, target: u32, x_value: u32) -> RuledCommand {
    let target_generation = engine
        .state
        .zone_change_generation
        .get(&target)
        .copied()
        .unwrap_or(0);
    let mut command = activate_ability_for(
        engine,
        source,
        2,
        vec![TargetRef {
            object_id: target,
            group_index: 0,
            kind: TargetRefKind::Permanent as i32,
            expected_zone_change_generation: Some(target_generation),
            ..Default::default()
        }],
    );
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!()
    };
    ability.x_value = x_value;
    command
}

fn resolve_one(engine: &mut GameEngine) {
    for _ in 0..engine
        .state
        .players
        .iter()
        .filter(|player| !player.has_lost)
        .count()
    {
        let actor = engine.state.priority_player_id();
        engine
            .apply_command(actor, &pass())
            .expect("pass priority to resolve the ability");
    }
}

fn resolve_until_copy_choice(engine: &mut GameEngine) {
    pass_both_players(engine);
    assert!(
        engine.state.pending_resolution.is_some(),
        "Cursed Mirror creates a copy-source choice after both players pass"
    );
}

fn advance_to_next_turn_instance(engine: &mut GameEngine) {
    let starting_turn = engine.state.turn_instance;
    for _ in 0..40 {
        if engine.state.turn_instance > starting_turn {
            return;
        }
        let actor = engine.state.priority_player_id();
        let command = if engine.state.cleanup_discard_player == Some(actor) {
            let index = engine.state.player_idx(actor).unwrap();
            discard_cleanup((engine.state.players[index].hand.len() - 1) as u32)
        } else {
            pass()
        };
        engine
            .apply_command(actor, &command)
            .expect("pass through the end of the turn");
    }
    panic!("game did not advance to the next turn instance");
}

fn generation(engine: &GameEngine, object_id: u32) -> u64 {
    engine
        .state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0)
}

#[test]
fn gardens_first_mana_ability_adds_colorless_immediately() {
    let mut engine = game(2026100910);
    let gardens = inject_permanent_on_battlefield(&mut engine, 0, "the_mycosynth_gardens");
    let priority = engine.state.priority_player_id();

    engine
        .apply_command(0, &activate_ability_for(&engine, gardens, 0, vec![]))
        .expect("tap for colorless mana");

    let pool = &engine.state.players[0].mana_pool;
    assert_eq!(
        (
            pool.white,
            pool.blue,
            pool.black,
            pool.red,
            pool.green,
            pool.colorless
        ),
        (0, 0, 0, 0, 0, 1)
    );
    assert!(engine.state.objects[&gardens].tapped);
    assert!(
        engine.state.stack.is_empty(),
        "mana abilities do not use the stack"
    );
    assert_eq!(engine.state.priority_player_id(), priority);
}

#[test]
fn gardens_second_mana_ability_pays_one_and_adds_each_color_immediately() {
    let colors = [
        (1, 0, 0, 0, 0, 0),
        (0, 1, 0, 0, 0, 0),
        (0, 0, 1, 0, 0, 0),
        (0, 0, 0, 1, 0, 0),
        (0, 0, 0, 0, 1, 0),
    ];

    for (option, expected) in colors.into_iter().enumerate() {
        let mut engine = game(2026100917 + option as u64);
        let gardens = inject_permanent_on_battlefield(&mut engine, 0, "the_mycosynth_gardens");
        engine.state.players[0].mana_pool.colorless = 1;
        let priority = engine.state.priority_player_id();
        let mut command = activate_ability_for(&engine, gardens, 1, vec![]);
        let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
            unreachable!()
        };
        ability.mana_option_index = option as u32;

        engine
            .apply_command(0, &command)
            .expect("pay one generic mana and choose one color");

        let pool = &engine.state.players[0].mana_pool;
        assert_eq!(
            (
                pool.white,
                pool.blue,
                pool.black,
                pool.red,
                pool.green,
                pool.colorless
            ),
            expected
        );
        assert!(engine.state.objects[&gardens].tapped);
        assert!(
            engine.state.stack.is_empty(),
            "mana abilities do not use the stack"
        );
        assert_eq!(engine.state.priority_player_id(), priority);
    }
}

#[test]
fn gardens_publishes_exact_x_target_pairs_with_current_generations() {
    let mut engine = game(2026100911);
    let gardens = inject_permanent_on_battlefield(&mut engine, 0, "the_mycosynth_gardens");
    let one = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    let two = inject_permanent_on_battlefield(&mut engine, 0, "arcane_signet");
    let opponent_artifact = inject_permanent_on_battlefield(&mut engine, 1, "sol_ring");
    let nonartifact = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let token_artifact = inject_permanent_on_battlefield(&mut engine, 0, "treasure");

    let key = (u64::from(gardens) << 32) | 2;
    let legal = engine.initial_response_batch();
    let group = &legal.legal_by_player[&0].valid_targets_by_ability[&key].groups[0];
    let published = group
        .x_target_choices
        .as_ref()
        .expect("the engine marks this group's X-bound target contract");
    assert_eq!(published.choices.len(), 2);
    let one_choice = published
        .choices
        .iter()
        .find(|choice| choice.x_value == 1)
        .expect("Sol Ring has mana value one");
    assert_eq!(
        one_choice
            .candidates
            .iter()
            .map(|candidate| candidate.object_id)
            .collect::<Vec<_>>(),
        vec![one]
    );
    assert_eq!(
        one_choice.candidates[0].zone_change_generation,
        generation(&engine, one)
    );
    let two_choice = published
        .choices
        .iter()
        .find(|choice| choice.x_value == 2)
        .expect("Arcane Signet has mana value two");
    assert_eq!(
        two_choice
            .candidates
            .iter()
            .map(|candidate| candidate.object_id)
            .collect::<Vec<_>>(),
        vec![two]
    );
    assert!(!group.valid_permanent_ids.contains(&opponent_artifact));
    assert!(!group.valid_permanent_ids.contains(&nonartifact));
    assert!(!group.valid_permanent_ids.contains(&token_artifact));
}

#[test]
fn gardens_rejects_a_target_that_left_and_returned_before_submission() {
    let mut engine = game_with_specials(2026100920, &["the_mycosynth_gardens", "sol_ring"]);
    let gardens = move_ready_to_battlefield(&mut engine, 0, "the_mycosynth_gardens");
    let ring = move_ready_to_battlefield(&mut engine, 0, "sol_ring");
    engine.state.players[0].mana_pool.colorless = 1;
    let selected_generation = generation(&engine, ring);
    let stale = copy_command(&engine, gardens, ring, 1);

    dev_move_card(&mut engine, 0, "Sol Ring", DevZone::Graveyard, false);
    dev_move_card(&mut engine, 0, "Sol Ring", DevZone::Battlefield, true);
    assert_eq!(generation(&engine, ring), selected_generation + 2);
    assert_eq!(engine.state.objects[&ring].zone, Zone::Battlefield);

    let before = format!("{:?}", engine.state);
    assert!(engine.apply_command(0, &stale).is_err());
    assert_eq!(format!("{:?}", engine.state), before);
    assert!(!engine.state.objects[&gardens].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
}

#[test]
fn gardens_copies_target_characteristics_but_keeps_its_physical_state() {
    let mut engine = game(2026100912);
    let gardens = inject_permanent_on_battlefield(&mut engine, 0, "the_mycosynth_gardens");
    let ring = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    engine
        .state
        .objects
        .get_mut(&gardens)
        .unwrap()
        .counters
        .insert(CounterKind::PlusOnePlusOne, 2);
    let gardens_generation = generation(&engine, gardens);
    let ring_generation = generation(&engine, ring);
    engine.state.players[0].mana_pool.colorless = 1;

    engine
        .apply_command(0, &copy_command(&engine, gardens, ring, 1))
        .expect("X equals Sol Ring's mana value");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert!(engine.state.objects[&gardens].tapped);
    assert_eq!(engine.state.objects[&gardens].zone, Zone::Battlefield);
    assert_eq!(generation(&engine, gardens), gardens_generation);

    resolve_one(&mut engine);

    assert_eq!(engine.state.objects[&gardens].copy_revision, 1);
    assert_eq!(
        engine.state.objects[&gardens].active_copy_occurrence,
        Some(1)
    );
    let copied = engine.characteristics(gardens).expect("copied source");
    assert!(copied.is_artifact());
    assert!(!copied.has_type("Land"));
    assert_eq!(copied.primary_name(), Some("Sol Ring"));
    assert_eq!(copied.mana_value, 1);
    assert_eq!(engine.state.objects[&gardens].id, gardens);
    assert!(engine.state.objects[&gardens].tapped);
    assert_eq!(
        engine.state.objects[&gardens].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    assert_eq!(generation(&engine, gardens), gardens_generation);
    assert_eq!(generation(&engine, ring), ring_generation);
}

#[test]
fn gardens_copy_carries_the_target_combat_requirements() {
    let mut engine = game(2026100915);
    let gardens = inject_permanent_on_battlefield(&mut engine, 0, "the_mycosynth_gardens");
    let juggernaut = inject_permanent_on_battlefield(&mut engine, 0, "juggernaut");
    engine.state.players[0].mana_pool.colorless = 4;

    engine
        .apply_command(0, &copy_command(&engine, gardens, juggernaut, 4))
        .expect("X equals Juggernaut's mana value");
    resolve_one(&mut engine);

    assert!(engine.state.objects[&gardens].must_attack_if_able);
    assert!(engine.characteristics(gardens).unwrap().is_creature());
}

#[test]
fn gardens_can_legally_target_itself_when_it_is_an_artifact() {
    let mut engine = game(2026100921);
    let gardens = inject_permanent_on_battlefield(&mut engine, 0, "the_mycosynth_gardens");
    let coating = inject_permanent_on_battlefield(&mut engine, 0, "liquimetal_coating");
    let target = TargetRef {
        object_id: gardens,
        group_index: 0,
        kind: TargetRefKind::Permanent as i32,
        ..Default::default()
    };
    engine
        .apply_command(0, &activate_ability_for(&engine, coating, 0, vec![target]))
        .expect("Liquimetal Coating can make Gardens an artifact");
    resolve_one(&mut engine);
    assert!(engine.characteristics(gardens).unwrap().is_artifact());

    let key = (u64::from(gardens) << 32) | 2;
    let legal = engine.initial_response_batch();
    let choices = legal.legal_by_player[&0].valid_targets_by_ability[&key].groups[0]
        .x_target_choices
        .as_ref()
        .expect("Garden's X target choices are engine-authored");
    assert!(choices
        .choices
        .iter()
        .find(|choice| choice.x_value == 0)
        .is_some_and(|choice| choice
            .candidates
            .iter()
            .any(|candidate| candidate.object_id == gardens)));

    engine
        .apply_command(0, &copy_command(&engine, gardens, gardens, 0))
        .expect("the source is a legal target when it is also an artifact");
    resolve_one(&mut engine);

    assert_eq!(engine.state.objects[&gardens].copy_revision, 1);
    assert!(engine.characteristics(gardens).unwrap().is_artifact());
    assert!(engine.characteristics(gardens).unwrap().has_type("Land"));
}

#[test]
fn gardens_does_not_copy_a_new_source_incarnation_after_it_leaves_and_returns() {
    let mut engine = game_with_specials(2026100922, &["the_mycosynth_gardens", "sol_ring"]);
    let gardens = move_ready_to_battlefield(&mut engine, 0, "the_mycosynth_gardens");
    let ring = move_ready_to_battlefield(&mut engine, 0, "sol_ring");
    engine.state.players[0].mana_pool.colorless = 1;
    let source_generation = generation(&engine, gardens);

    engine
        .apply_command(0, &copy_command(&engine, gardens, ring, 1))
        .expect("activate Gardens before it changes zones");
    dev_move_card(
        &mut engine,
        0,
        "The Mycosynth Gardens",
        DevZone::Graveyard,
        false,
    );
    dev_move_card(
        &mut engine,
        0,
        "The Mycosynth Gardens",
        DevZone::Battlefield,
        true,
    );
    assert_eq!(generation(&engine, gardens), source_generation + 2);

    resolve_one(&mut engine);

    assert_eq!(engine.state.objects[&gardens].copy_revision, 0);
    assert_eq!(engine.state.objects[&gardens].zone, Zone::Battlefield);
    assert!(engine.characteristics(gardens).unwrap().has_type("Land"));
    assert!(!engine.state.objects[&gardens].tapped);
}

#[test]
fn gardens_rechecks_target_controller_when_the_ability_resolves() {
    let mut engine = game(2026100923);
    let gardens = inject_permanent_on_battlefield(&mut engine, 0, "the_mycosynth_gardens");
    let signet = inject_permanent_on_battlefield(&mut engine, 0, "arcane_signet");
    engine.state.players[0].mana_pool.colorless = 2;

    engine
        .apply_command(0, &copy_command(&engine, gardens, signet, 2))
        .expect("activate while Arcane Signet is controlled by its targeter");
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(signet),
        kind: ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(1),
        },
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index + 1,
    });

    resolve_one(&mut engine);

    assert_eq!(engine.state.objects[&signet].controller, 1);
    assert_eq!(engine.state.objects[&gardens].copy_revision, 0);
    assert!(engine.characteristics(gardens).unwrap().has_type("Land"));
}

#[test]
fn gardens_rechecks_artifact_type_when_the_ability_resolves() {
    let mut engine = game(2026100924);
    let gardens = inject_permanent_on_battlefield(&mut engine, 0, "the_mycosynth_gardens");
    let signet = inject_permanent_on_battlefield(&mut engine, 0, "arcane_signet");
    engine.state.players[0].mana_pool.colorless = 2;

    engine
        .apply_command(0, &copy_command(&engine, gardens, signet, 2))
        .expect("activate while Arcane Signet is an artifact");
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(signet),
        kind: ContinuousEffectKind::Layer4SetTypeLine(TypeLineReplacement {
            land_types: Vec::new(),
            card_types: vec![PermanentTypeFilter::Land],
            creature_types: Vec::new(),
        }),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index + 1,
    });

    let characteristics = engine.characteristics(signet).unwrap();
    assert_eq!(
        characteristics.mana_value, 2,
        "the target keeps its mana value"
    );
    assert!(characteristics.has_type("Land"));
    assert!(!characteristics.is_artifact());
    resolve_one(&mut engine);

    assert_eq!(engine.state.objects[&gardens].copy_revision, 0);
    assert!(engine.characteristics(gardens).unwrap().has_type("Land"));
}

#[test]
fn gardens_keeps_a_snapshot_of_cursed_mirrors_temporary_copy() {
    let mut engine = game_with_cursed_mirror(2026100916);
    let mirror = relocate_to_hand(&mut engine, 0, "cursed_mirror");
    let clone = relocate_to_hand(&mut engine, 0, "clone");
    let gardens = inject_permanent_on_battlefield(&mut engine, 0, "the_mycosynth_gardens");
    let ornithopter = inject_permanent_on_battlefield(&mut engine, 0, "ornithopter");
    engine.state.players[0].mana_pool.colorless = 4;
    engine.state.players[0].mana_pool.blue = 2;
    engine.state.players[0].mana_pool.red = 1;

    engine
        .apply_command(
            0,
            &cast_spell(hand_index_for_card(&engine, 0, "clone"), Vec::new()),
        )
        .expect("cast Clone to create the first copy layer");
    resolve_until_copy_choice(&mut engine);
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
        .expect("Clone copies Ornithopter");
    assert_eq!(engine.state.objects[&clone].card_id, "clone");
    assert_eq!(
        engine.characteristics(clone).unwrap().primary_name(),
        Some("Ornithopter")
    );
    let hand_slot = hand_index_for_card(&engine, 0, "cursed_mirror");

    engine
        .apply_command(0, &cast_spell(hand_slot, Vec::new()))
        .expect("cast Cursed Mirror");
    resolve_until_copy_choice(&mut engine);
    let deciding_player = engine
        .state
        .pending_resolution
        .as_ref()
        .unwrap()
        .deciding_player;
    engine
        .apply_command(deciding_player, &submit_resolution_choice(vec![clone]))
        .expect("Cursed Mirror copies an artifact that is already copying Ornithopter");
    assert_eq!(
        engine.characteristics(mirror).unwrap().primary_name(),
        Some("Ornithopter")
    );
    assert!(engine.effective_has_keyword(mirror, tricerules_cards::Keyword::Haste));

    engine
        .apply_command(0, &copy_command(&engine, gardens, mirror, 0))
        .expect("copy the current Cursed Mirror characteristics");
    resolve_one(&mut engine);
    let gardens_revision = engine.state.objects[&gardens].copy_revision;
    assert_eq!(
        engine.characteristics(gardens).unwrap().primary_name(),
        Some("Ornithopter")
    );
    assert!(engine.effective_has_keyword(gardens, tricerules_cards::Keyword::Haste));

    advance_to_next_turn_instance(&mut engine);

    assert_eq!(
        engine.characteristics(mirror).unwrap().primary_name(),
        Some("Cursed Mirror")
    );
    assert!(!engine.effective_has_keyword(mirror, tricerules_cards::Keyword::Haste));
    assert_eq!(
        engine.characteristics(gardens).unwrap().primary_name(),
        Some("Ornithopter")
    );
    assert!(engine.effective_has_keyword(gardens, tricerules_cards::Keyword::Haste));
    assert_eq!(
        engine.state.objects[&gardens].copy_revision,
        gardens_revision
    );
}

#[test]
fn gardens_rejects_mismatched_or_stale_x_target_choices_without_paying() {
    let mut engine = game(2026100913);
    let gardens = inject_permanent_on_battlefield(&mut engine, 0, "the_mycosynth_gardens");
    let ring = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    engine.state.players[0].mana_pool.colorless = 3;
    let before = format!("{:?}", engine.state);

    assert!(engine
        .apply_command(0, &copy_command(&engine, gardens, ring, 2))
        .is_err());
    assert_eq!(format!("{:?}", engine.state), before);

    let mut stale = copy_command(&engine, gardens, ring, 1);
    let Some(Cmd::ActivateAbility(ability)) = stale.cmd.as_mut() else {
        unreachable!()
    };
    ability.targets[0].expected_zone_change_generation = Some(generation(&engine, ring) + 1);
    assert!(engine.apply_command(0, &stale).is_err());
    assert_eq!(format!("{:?}", engine.state), before);

    let mut unbound = copy_command(&engine, gardens, ring, 1);
    let Some(Cmd::ActivateAbility(ability)) = unbound.cmd.as_mut() else {
        unreachable!()
    };
    ability.targets[0].expected_zone_change_generation = None;
    assert!(engine.apply_command(0, &unbound).is_err());
    assert_eq!(format!("{:?}", engine.state), before);
}

#[test]
fn gardens_does_not_copy_when_target_mana_value_changes_before_resolution() {
    let mut engine = game(2026100914);
    let gardens = inject_permanent_on_battlefield(&mut engine, 0, "the_mycosynth_gardens");
    let ring = inject_permanent_on_battlefield(&mut engine, 0, "sol_ring");
    engine.state.players[0].mana_pool.colorless = 1;
    let source_generation = generation(&engine, gardens);
    engine
        .apply_command(0, &copy_command(&engine, gardens, ring, 1))
        .expect("the target is legal when activated");

    engine.state.objects.get_mut(&ring).unwrap().card_id = "arcane_signet".to_string();
    resolve_one(&mut engine);

    assert!(engine.state.objects[&gardens].copiable_values.is_none());
    assert_eq!(generation(&engine, gardens), source_generation);
    assert!(engine.characteristics(gardens).unwrap().has_type("Land"));
    assert!(engine.state.objects[&gardens].tapped);
}
