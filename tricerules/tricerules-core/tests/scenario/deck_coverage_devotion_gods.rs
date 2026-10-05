//! Actual God definitions through logged dev placement and paid authoritative activations.
use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    dev_command::Dev, DevCommand, DevMoveCard, DevPutCardInZone, DevZone,
};

fn setup() -> GameEngine {
    let mut engine = GameEngine::new(
        700_510,
        &[0, 1, 2],
        20,
        Some(vec![deck_with("forest", &[]); 3]),
        true,
    )
    .unwrap();
    engine.enable_dev_commands();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn put(engine: &mut GameEngine, player: i32, name: &str) -> (u32, RuledEventBatch) {
    let batch = engine
        .apply_command(
            engine.state.priority_player_id(),
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: player,
                    dev: Some(Dev::PutCardInZone(DevPutCardInZone {
                        card_name: name.into(),
                        zone: DevZone::Battlefield as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .unwrap();
    let id = tricerules_cards::slugify(name);
    let oid = *engine.state.players[engine.state.player_idx(player).unwrap()]
        .battlefield
        .iter()
        .filter(|oid| engine.state.objects[oid].card_id == id)
        .max()
        .unwrap();
    (oid, batch)
}

fn move_card(engine: &mut GameEngine, player: i32, name: &str, zone: DevZone) -> RuledEventBatch {
    engine
        .apply_command(
            engine.state.priority_player_id(),
            &RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: player,
                    dev: Some(Dev::MoveCard(DevMoveCard {
                        card_name: name.into(),
                        zone: zone as i32,
                        ready: true,
                    })),
                })),
            },
        )
        .unwrap()
}

fn settle_stack(engine: &mut GameEngine) {
    for _ in 0..30 {
        answer_trigger_order_in_engine_order(engine);
        answer_simultaneous_entry_order_in_engine_order(engine);
        if engine.state.stack.is_empty() {
            return;
        }
        pass_priority_round(engine);
    }
    panic!("bounded God fixture did not settle");
}

fn paid_activation(engine: &GameEngine, god: u32, targets: Vec<TargetRef>) -> RuledCommand {
    let mut command = activate_ability_for(engine, god, 0, targets);
    authoring_actions::pay(engine, 0, &mut command).unwrap();
    command
}

fn rejected_without_state_change(engine: &mut GameEngine, player: i32, command: &RuledCommand) {
    let before = format!("{:?}", engine.state);
    assert!(engine.apply_command(player, command).is_err());
    assert_eq!(format!("{:?}", engine.state), before);
}

#[test]
fn devotion_gods_cast_as_creature_spells_with_exact_paid_costs_then_enter_as_noncreatures() {
    for name in ["Nylea, God of the Hunt", "Purphoros, God of the Forge"] {
        let mut engine = setup();
        let (god, _) = put(&mut engine, 0, name);
        move_card(&mut engine, 0, name, DevZone::Hand);
        assert!(engine.characteristics(god).unwrap().is_creature());
        let generation = engine
            .state
            .zone_change_generation
            .get(&god)
            .copied()
            .unwrap_or(0);
        engine.state.players[0].mana_pool.colorless = 3;
        if name.starts_with("Nylea") {
            engine.state.players[0].mana_pool.green = 1;
        } else {
            engine.state.players[0].mana_pool.red = 1;
        }
        let slot = hand_index_for_card(&engine, 0, &tricerules_cards::slugify(name));
        let mut cast = cast_spell(slot, vec![]);
        authoring_actions::pay(&engine, 0, &mut cast).unwrap();
        rejected_without_state_change(&mut engine, 1, &cast);
        engine.apply_command(0, &cast).unwrap();
        assert_eq!(engine.state.objects[&god].zone, Zone::Stack);
        assert!(engine.characteristics(god).unwrap().is_creature());
        assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
        assert_eq!(
            engine.state.players[0].mana_pool.green + engine.state.players[0].mana_pool.red,
            0
        );
        settle_stack(&mut engine);
        assert_eq!(engine.state.objects[&god].zone, Zone::Battlefield);
        assert!(engine.state.zone_change_generation[&god] > generation);
        assert!(!engine.characteristics(god).unwrap().is_creature());
        assert!(engine.state.stack.is_empty());
    }
}

#[test]
fn nylea_paid_pump_does_not_rebind_a_target_that_leaves_and_returns() {
    let mut engine = setup();
    let (god, _) = put(&mut engine, 0, "Nylea, God of the Hunt");
    let (target, _) = put(&mut engine, 1, "Grizzly Bears");
    engine.state.players[0].mana_pool.colorless = 3;
    engine.state.players[0].mana_pool.green = 1;
    let command = paid_activation(&engine, god, target_object(target));
    engine.apply_command(0, &command).unwrap();
    move_card(&mut engine, 1, "Grizzly Bears", DevZone::Hand);
    move_card(&mut engine, 1, "Grizzly Bears", DevZone::Battlefield);
    settle_stack(&mut engine);
    assert_eq!(engine.characteristics(target).unwrap().power, Some(2));
    assert_eq!(engine.characteristics(target).unwrap().toughness, Some(2));
    assert!(engine.state.stack.is_empty());
}

#[test]
fn purphoros_five_devotion_team_pump_includes_source_and_excludes_later_entrants() {
    let mut engine = setup();
    let (god, _) = put(&mut engine, 0, "Purphoros, God of the Forge");
    let mut own = vec![];
    for _ in 0..4 {
        own.push(put(&mut engine, 0, "Hill Giant").0);
        settle_stack(&mut engine);
    }
    assert!(
        engine.characteristics(god).unwrap().is_creature(),
        "own R plus four Hill Giant pips is five"
    );
    engine.state.players[0].mana_pool.colorless = 2;
    engine.state.players[0].mana_pool.red = 1;
    let command = paid_activation(&engine, god, vec![]);
    engine.apply_command(0, &command).unwrap();
    settle_stack(&mut engine);
    assert_eq!(engine.characteristics(god).unwrap().power, Some(7));
    assert!(own
        .iter()
        .all(|id| engine.characteristics(*id).unwrap().power == Some(4)));
    let later = put(&mut engine, 0, "Hill Giant").0;
    settle_stack(&mut engine);
    assert_eq!(engine.characteristics(later).unwrap().power, Some(3));
}

#[test]
fn nylea_threshold_own_pip_other_creature_trample_and_counters_survive_type_changes() {
    let mut engine = setup();
    let (god, _) = put(&mut engine, 0, "Nylea, God of the Hunt");
    let (own, _) = put(&mut engine, 0, "Grizzly Bears");
    let (opponent, _) = put(&mut engine, 1, "Grizzly Bears");
    assert!(!engine.characteristics(god).unwrap().is_creature());
    assert!(engine
        .characteristics(god)
        .unwrap()
        .has_keyword(Keyword::Indestructible));
    assert!(!engine
        .characteristics(god)
        .unwrap()
        .has_keyword(Keyword::Trample));
    assert!(engine
        .characteristics(own)
        .unwrap()
        .has_keyword(Keyword::Trample));
    assert!(!engine
        .characteristics(opponent)
        .unwrap()
        .has_keyword(Keyword::Trample));
    // God G + Bears G + Mammoth GGG = five, with every repeated symbol counted.
    put(&mut engine, 0, "Aggressive Mammoth");
    assert!(engine.characteristics(god).unwrap().is_creature());
    // Mammoth independently gives Nylea trample while Nylea is a creature.
    assert!(engine
        .characteristics(god)
        .unwrap()
        .has_keyword(Keyword::Trample));
    engine
        .state
        .objects
        .get_mut(&god)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 2);
    move_card(&mut engine, 0, "Aggressive Mammoth", DevZone::Hand);
    assert!(!engine.characteristics(god).unwrap().is_creature());
    assert_eq!(
        engine.state.objects[&god].counter_count(CounterKind::PlusOnePlusOne),
        2
    );
    move_card(&mut engine, 0, "Aggressive Mammoth", DevZone::Battlefield);
    assert!(engine.characteristics(god).unwrap().is_creature());
    assert_eq!(engine.characteristics(god).unwrap().power, Some(8));
    move_card(&mut engine, 0, "Nylea, God of the Hunt", DevZone::Hand);
    assert!(
        engine.characteristics(god).unwrap().is_creature(),
        "the type-changing ability functions only on the battlefield"
    );
    assert_eq!(engine.state.objects[&god].zone, Zone::Hand);
}

#[test]
fn nylea_noncreature_paid_pump_accepts_opponent_and_rejects_illegal_stale_and_unpaid_commands() {
    let mut engine = setup();
    let (god, _) = put(&mut engine, 0, "Nylea, God of the Hunt");
    let (target, _) = put(&mut engine, 1, "Grizzly Bears");
    let (land, _) = put(&mut engine, 0, "Forest");
    assert!(!engine.characteristics(god).unwrap().is_creature());
    engine.state.players[0].mana_pool.colorless = 3;
    engine.state.players[0].mana_pool.green = 1;
    let command = paid_activation(&engine, god, target_object(target));
    rejected_without_state_change(&mut engine, 1, &command);
    for bad_target in [god, land] {
        let mut invalid = command.clone();
        let Some(Cmd::ActivateAbility(activation)) = invalid.cmd.as_mut() else {
            unreachable!()
        };
        activation.targets = target_object(bad_target);
        rejected_without_state_change(&mut engine, 0, &invalid);
    }
    let mut stale = command.clone();
    let Some(Cmd::ActivateAbility(activation)) = stale.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation += 1;
    rejected_without_state_change(&mut engine, 0, &stale);
    engine.state.players[0].mana_pool.green = 0;
    rejected_without_state_change(&mut engine, 0, &command);
    engine.state.players[0].mana_pool.green = 1;
    engine.apply_command(0, &command).unwrap();
    assert_eq!(engine.state.players[0].mana_pool.green, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    settle_stack(&mut engine);
    assert_eq!(engine.characteristics(target).unwrap().power, Some(4));
    assert_eq!(engine.characteristics(target).unwrap().toughness, Some(4));
}

#[test]
fn purphoros_noncreature_triggers_only_other_controlled_creatures_and_damages_each_opponent() {
    let mut engine = setup();
    let (god, _) = put(&mut engine, 0, "Purphoros, God of the Forge");
    assert!(!engine.characteristics(god).unwrap().is_creature());
    assert!(
        engine.state.stack.is_empty(),
        "source does not trigger itself"
    );
    put(&mut engine, 1, "Grizzly Bears");
    put(&mut engine, 0, "Mind Stone");
    assert!(
        engine.state.stack.is_empty(),
        "opponent creatures and own noncreatures are excluded"
    );
    put(&mut engine, 0, "Grizzly Bears");
    assert_eq!(engine.state.stack.len(), 1);
    settle_stack(&mut engine);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|p| p.life)
            .collect::<Vec<_>>(),
        vec![20, 18, 18]
    );
    put(&mut engine, 0, "Grizzly Bears");
    settle_stack(&mut engine);
    assert_eq!(
        engine
            .state
            .players
            .iter()
            .map(|p| p.life)
            .collect::<Vec<_>>(),
        vec![20, 16, 16]
    );
}

#[test]
fn purphoros_noncreature_paid_team_pump_snapshots_only_own_creatures() {
    let mut engine = setup();
    let (god, _) = put(&mut engine, 0, "Purphoros, God of the Forge");
    let (own, _) = put(&mut engine, 0, "Grizzly Bears");
    settle_stack(&mut engine);
    let (opponent, _) = put(&mut engine, 1, "Grizzly Bears");
    engine.state.players[0].mana_pool.colorless = 2;
    engine.state.players[0].mana_pool.red = 1;
    let command = paid_activation(&engine, god, vec![]);
    rejected_without_state_change(&mut engine, 1, &command);
    let mut invalid = command.clone();
    let Some(Cmd::ActivateAbility(activation)) = invalid.cmd.as_mut() else {
        unreachable!()
    };
    activation.targets = target_object(own);
    rejected_without_state_change(&mut engine, 0, &invalid);
    engine.state.players[0].mana_pool.red = 0;
    rejected_without_state_change(&mut engine, 0, &command);
    engine.state.players[0].mana_pool.red = 1;
    engine.apply_command(0, &command).unwrap();
    settle_stack(&mut engine);
    assert_eq!(engine.characteristics(own).unwrap().power, Some(3));
    assert_eq!(engine.characteristics(opponent).unwrap().power, Some(2));
    assert!(!engine.characteristics(god).unwrap().is_creature());
}
