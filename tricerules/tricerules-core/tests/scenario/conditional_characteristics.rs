//! Issue #78: dynamic self-static conditions across characteristic and attack-legality layers.

use crate::helpers::*;
use tricerules_cards::primitives::{
    CombatRestriction, ContinuousEffectKind, ControllerReference, EffectDuration,
};
use tricerules_cards::Keyword;
use tricerules_core::{AffectedScope, ContinuousEffect};
use tricerules_proto::ruled::v1::dev_command::Dev;
use tricerules_proto::ruled::v1::{
    BattlefieldObject, DevCommand, DevMoveCard, DevPutCardInZone, DevZone, RuledCommand,
};

fn dev(target: i32, payload: Dev) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DevCommand(DevCommand {
            target_player_id: target,
            dev: Some(payload),
        })),
    }
}

fn put_ready(target: i32, card_name: &str) -> RuledCommand {
    dev(
        target,
        Dev::PutCardInZone(DevPutCardInZone {
            card_name: card_name.to_string(),
            zone: DevZone::Battlefield as i32,
            ready: true,
        }),
    )
}

fn move_to_hand(target: i32, card_name: &str) -> RuledCommand {
    dev(
        target,
        Dev::MoveCard(DevMoveCard {
            card_name: card_name.to_string(),
            zone: DevZone::Hand as i32,
            ready: false,
        }),
    )
}

fn move_to_battlefield(target: i32, card_name: &str) -> RuledCommand {
    dev(
        target,
        Dev::MoveCard(DevMoveCard {
            card_name: card_name.to_string(),
            zone: DevZone::Battlefield as i32,
            ready: true,
        }),
    )
}

fn issue_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![vec!["mountain".into(); 12], vec!["forest".into(); 12]]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new engine");
    engine.enable_dev_commands();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn put(engine: &mut GameEngine, player: i32, card_name: &str, card_id: &str) -> u32 {
    engine
        .apply_command(player, &put_ready(player, card_name))
        .unwrap_or_else(|error| panic!("put {card_name}: {error:?}"));
    battlefield_object_for_card(engine, player as usize, card_id)
}

fn power(engine: &GameEngine, oid: u32) -> u32 {
    engine
        .characteristics(oid)
        .and_then(|characteristics| characteristics.power)
        .expect("creature power")
}

fn battlefield_snapshot(engine: &mut GameEngine, player: i32, oid: u32) -> BattlefieldObject {
    engine
        .initial_response_batch()
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => view
                .per_player
                .iter()
                .find(|entry| entry.player_id == player)
                .and_then(|entry| {
                    entry
                        .battlefield_objects
                        .iter()
                        .find(|object| object.object_id == oid)
                })
                .cloned(),
            _ => None,
        })
        .expect("battlefield object in zone view")
}

fn selectable_attackers(engine: &mut GameEngine, player: i32) -> Vec<u32> {
    engine
        .initial_response_batch()
        .legal_by_player
        .get(&player)
        .expect("legal actions")
        .selectable_attacker_ids
        .clone()
}

fn enter_declare_attackers(engine: &mut GameEngine) {
    engine
        .apply_command(0, &primitive_yield())
        .expect("main1 to beginning of combat");
    pass_both_players(engine);
    assert_eq!(
        engine.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
}

#[test]
fn daggersail_aeronaut_has_flying_only_during_its_controllers_turn() {
    let mut engine = issue_engine(78_001);
    let aeronaut = put(&mut engine, 0, "Daggersail Aeronaut", "daggersail_aeronaut");

    assert!(engine.effective_has_keyword(aeronaut, Keyword::Flying));
    assert!(
        battlefield_snapshot(&mut engine, 0, aeronaut)
            .keywords
            .contains(&"Flying".to_string()),
        "the existing public battlefield snapshot carries the derived keyword"
    );
    engine.state.active_player_idx = 1;
    assert!(!engine.effective_has_keyword(aeronaut, Keyword::Flying));
    assert!(!battlefield_snapshot(&mut engine, 0, aeronaut)
        .keywords
        .contains(&"Flying".to_string()));
}

#[test]
fn gearsmith_modifiers_track_current_qualifying_permanents() {
    let mut engine = issue_engine(78_002);
    let guardian = put(&mut engine, 0, "Gearsmith Guardian", "gearsmith_guardian");
    let prodigy = put(&mut engine, 0, "Gearsmith Prodigy", "gearsmith_prodigy");

    assert_eq!(power(&engine, guardian), 5, "Prodigy is a blue creature");
    assert_eq!(battlefield_snapshot(&mut engine, 0, guardian).power, 5);
    assert_eq!(power(&engine, prodigy), 2, "Guardian is an artifact");

    engine
        .apply_command(0, &move_to_hand(0, "Gearsmith Prodigy"))
        .expect("remove blue creature");
    assert_eq!(power(&engine, guardian), 3);

    put(&mut engine, 0, "Air Elemental", "air_elemental");
    assert_eq!(power(&engine, guardian), 5);

    engine
        .apply_command(0, &move_to_hand(0, "Air Elemental"))
        .expect("remove blue creature");
    assert_eq!(power(&engine, guardian), 3);

    let prodigy = put(&mut engine, 0, "Gearsmith Prodigy", "gearsmith_prodigy");
    assert_eq!(power(&engine, prodigy), 2, "Guardian is an artifact");

    engine
        .apply_command(0, &move_to_hand(0, "Gearsmith Guardian"))
        .expect("remove artifact");
    assert_eq!(power(&engine, prodigy), 1);

    engine
        .apply_command(0, &move_to_battlefield(0, "Gearsmith Guardian"))
        .expect("return artifact to battlefield");
    let returned_guardian = battlefield_object_for_card(&engine, 0, "gearsmith_guardian");
    assert_eq!(returned_guardian, guardian, "the physical object is reused");
    assert_eq!(
        power(&engine, returned_guardian),
        5,
        "leaving drains the old conditional effects before re-entry emits them again"
    );
}

#[test]
fn conditional_controller_reference_tracks_layer_2_control() {
    let mut engine = issue_engine(78_005);
    let guardian = put(&mut engine, 0, "Gearsmith Guardian", "gearsmith_guardian");
    put(&mut engine, 0, "Air Elemental", "air_elemental");
    assert_eq!(power(&engine, guardian), 5);

    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(guardian),
        kind: ContinuousEffectKind::Layer2Control {
            controller: ControllerReference::Fixed(1),
        },
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    assert_eq!(
        power(&engine, guardian),
        3,
        "P0's blue creature no longer counts"
    );

    put(&mut engine, 1, "Air Elemental", "air_elemental");
    assert_eq!(
        power(&engine, guardian),
        5,
        "the condition rebases to the source's current controller"
    );
}

#[test]
fn drowsing_tyrannodon_uses_current_power_only_when_declaring_attackers() {
    let mut engine = issue_engine(78_003);
    let tyrannodon = put(&mut engine, 0, "Drowsing Tyrannodon", "drowsing_tyrannodon");
    let air_elemental = put(&mut engine, 0, "Air Elemental", "air_elemental");
    enter_declare_attackers(&mut engine);
    assert!(selectable_attackers(&mut engine, 0).contains(&tyrannodon));

    engine
        .apply_command(0, &declare_attackers(vec![tyrannodon]))
        .expect("power-four creature lets Tyrannodon attack");
    engine
        .apply_command(0, &move_to_hand(0, "Air Elemental"))
        .expect("remove qualifier after declaration");

    assert_eq!(
        engine.state.combat.as_ref().expect("combat").attacking,
        vec![tyrannodon],
        "losing the condition does not remove an already-declared attacker"
    );
    assert_eq!(
        engine
            .state
            .objects
            .get(&air_elemental)
            .expect("object")
            .zone,
        tricerules_core::Zone::Hand
    );
}

#[test]
fn drowsing_tyrannodon_bypasses_only_defender_when_the_condition_holds() {
    let mut without_power = issue_engine(78_006);
    let sleeping = put(
        &mut without_power,
        0,
        "Drowsing Tyrannodon",
        "drowsing_tyrannodon",
    );
    put(&mut without_power, 0, "Grizzly Bears", "grizzly_bears");
    enter_declare_attackers(&mut without_power);
    assert!(!selectable_attackers(&mut without_power, 0).contains(&sleeping));
    assert!(without_power
        .apply_command(0, &declare_attackers(vec![sleeping]))
        .is_err());

    let mut restricted = issue_engine(78_007);
    let restricted_tyrannodon = put(
        &mut restricted,
        0,
        "Drowsing Tyrannodon",
        "drowsing_tyrannodon",
    );
    restricted
        .state
        .objects
        .get_mut(&restricted_tyrannodon)
        .expect("tyrannodon")
        .power = Some(4);
    restricted.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(restricted_tyrannodon),
        kind: ContinuousEffectKind::CombatRestriction(CombatRestriction {
            cant_attack: true,
            cant_block: false,
            cant_be_blocked: false,
            ..Default::default()
        }),
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: restricted.state.command_index,
    });
    put(&mut restricted, 0, "Grizzly Bears", "grizzly_bears");
    enter_declare_attackers(&mut restricted);
    assert!(!selectable_attackers(&mut restricted, 0).contains(&restricted_tyrannodon));
    assert!(
        restricted
            .apply_command(0, &declare_attackers(vec![restricted_tyrannodon]))
            .is_err(),
        "the conditional permission does not override unrelated attack restrictions"
    );
}

#[test]
fn drowsing_tyrannodon_can_qualify_itself_but_still_has_defender() {
    let mut engine = issue_engine(78_004);
    let tyrannodon = put(&mut engine, 0, "Drowsing Tyrannodon", "drowsing_tyrannodon");
    engine
        .state
        .objects
        .get_mut(&tyrannodon)
        .expect("tyrannodon")
        .power = Some(4);
    assert!(engine.effective_has_keyword(tyrannodon, Keyword::Defender));

    enter_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![tyrannodon]))
        .expect("Tyrannodon's own derived power qualifies it");
}
