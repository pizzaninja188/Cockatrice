//! Issue #416 — the Roads cycle's "This land enters tapped unless you control a Mount or
//! Vehicle." replacement condition (CR 614.1d).
//!
//! The five Roads identities stay unretained until blocker #319 can express the Pilot token's
//! saddle/crew contribution static, so this scenario carries the exact `EntersTapped` definition
//! the new recipe emits as an inline fixture face. The fixture face is attached through the
//! engine's ordinary copiable-value channel, and the land is played through the real land-play
//! command, exercising the shipped entry-replacement path rather than a card-specific dispatch.

use super::helpers::*;
use tricerules_cards::primitives::{
    BattlefieldAggregate, BattlefieldPermanentFilter, ContinuousEffectKind, EffectDuration,
    EntersTappedAffected, GameCondition, RelativePlayerSet, StaticAbilityDef, TypeLineAddition,
};
use tricerules_cards::{AbilityPresentation, CardFace, CardRegistry, IdentifiedAbility};
use tricerules_core::state::CopiableValues;
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::RuledEventBatch;

const FIXTURE_ID: &str = "issue_416_roads_fixture";

/// The exact static ability emitted by
/// `static.enters_tapped.unless_control_mount_or_vehicle`: tap unless the controller controls at
/// least one Mount or Vehicle, so the condition holds only at a count of zero.
fn roads_fixture_face() -> CardFace {
    let fixture = r#"(id: "issue_416_roads_fixture", name: "Issue 416 Roads Fixture",
        face_id: "issue_416_roads_fixture", types: ["Land"],
        static_abilities: [(
            ability_id: "static_01",
            presentation: Fallback,
            definition: EntersTapped(
                affected: Self_,
                condition: Some(BattlefieldAggregate(
                    filter: (
                        controllers: Controller,
                        any_of: Some([
                            (controllers: Controller, required_subtypes: ["Mount"]),
                            (controllers: Controller, required_subtypes: ["Vehicle"]),
                        ]),
                    ),
                    aggregate: Count,
                    max: Some(0),
                )),
            ),
        )])"#;
    CardRegistry::from_chunks_and_tokens(&[fixture], &[])
        .expect("issue #416 entry fixture is valid card data")
        .get(FIXTURE_ID)
        .expect("fixture card is loaded")
        .primary_face()
        .clone()
}

fn attach_roads_face(engine: &mut GameEngine, object_id: u32) {
    engine
        .state
        .objects
        .get_mut(&object_id)
        .expect("fixture object")
        .copiable_values = Some(CopiableValues {
        source_card_id: FIXTURE_ID.into(),
        source_face_index: 0,
        face: roads_fixture_face(),
        room_faces: None,
        display_name: "Issue 416 Roads Fixture".into(),
    });
}

fn roads_engine(seed: u64) -> (GameEngine, u32) {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut engine =
        GameEngine::new(seed, &[0, 1], 20, decks, true).expect("issue #416 scenario engine");
    advance_to_main1_from_game_start(&mut engine);
    let land = inject_card_into_hand(&mut engine, 0, "island");
    attach_roads_face(&mut engine, land);
    (engine, land)
}

fn inject_mount(engine: &mut GameEngine, player: usize) -> u32 {
    let mount = inject_creature_on_battlefield(engine, player, "grizzly_bears");
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(mount),
        kind: ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: Vec::new(),
            creature_types: vec!["Mount".into()],
        }),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
    mount
}

fn inject_vehicle(engine: &mut GameEngine, player: usize) -> u32 {
    inject_permanent_on_battlefield(engine, player, "skybox_ferry")
}

fn play_fixture_land(engine: &mut GameEngine, land: u32) -> Vec<RuledEventBatch> {
    let index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == land)
        .expect("fixture land is in hand");
    let batch = engine
        .apply_command(engine.state.players[0].id, &play_land(index))
        .expect("fixture land play");
    assert_eq!(engine.state.objects[&land].zone, Zone::Battlefield);
    vec![batch]
}

fn add_third_player(engine: &mut GameEngine) {
    let mut third = engine.state.players[1].clone();
    third.id = 2;
    third.hand.clear();
    third.library.clear();
    third.battlefield.clear();
    third.graveyard.clear();
    engine.state.players.push(third);
}

#[test]
fn issue_416_roads_entry_condition_covers_every_mount_vehicle_combination() {
    for (mount, vehicle, expected_tapped) in [
        (false, false, true),
        (true, false, false),
        (false, true, false),
        (true, true, false),
    ] {
        let (mut engine, land) = roads_engine(416_000 + u64::from(mount) * 2 + u64::from(vehicle));
        if mount {
            inject_mount(&mut engine, 0);
        }
        if vehicle {
            inject_vehicle(&mut engine, 0);
        }
        play_fixture_land(&mut engine, land);
        assert_eq!(
            engine.state.objects[&land].tapped, expected_tapped,
            "Mount={mount} Vehicle={vehicle}"
        );
    }
}

#[test]
fn issue_416_roads_entry_condition_ignores_other_players_permanents() {
    let (mut engine, land) = roads_engine(416_100);
    inject_mount(&mut engine, 1);
    inject_vehicle(&mut engine, 1);
    play_fixture_land(&mut engine, land);
    assert!(
        engine.state.objects[&land].tapped,
        "an opponent's Mount and Vehicle do not satisfy the controller condition"
    );
}

#[test]
fn issue_416_roads_entry_condition_is_player_set_generic_in_multiplayer() {
    let (mut engine, land) = roads_engine(416_200);
    add_third_player(&mut engine);
    inject_mount(&mut engine, 2);
    inject_vehicle(&mut engine, 2);
    play_fixture_land(&mut engine, land);
    assert!(
        engine.state.objects[&land].tapped,
        "a third player's Mount and Vehicle do not satisfy the controller condition"
    );

    let (mut engine, land) = roads_engine(416_201);
    add_third_player(&mut engine);
    inject_mount(&mut engine, 0);
    inject_vehicle(&mut engine, 2);
    play_fixture_land(&mut engine, land);
    assert!(
        !engine.state.objects[&land].tapped,
        "the controller's own Mount plus a third player's Vehicle satisfy the condition"
    );
}

#[test]
fn issue_416_roads_entry_commands_replay_identically() {
    fn run() -> (Vec<RuledEventBatch>, bool) {
        let (mut engine, land) = roads_engine(416_300);
        inject_mount(&mut engine, 0);
        let batches = play_fixture_land(&mut engine, land);
        (batches, engine.state.objects[&land].tapped)
    }

    let (first, tapped) = run();
    let (second, replay_tapped) = run();
    assert_eq!(first, second);
    assert_eq!(tapped, replay_tapped);
    assert!(!tapped, "the controller's Mount keeps the land untapped");
}

/// The emitted static ability is exactly the fixture definition; the card identities themselves
/// are checked by the recipes catalog tests. This guards the fixture against accidental drift.
#[test]
fn issue_416_roads_fixture_matches_the_emitted_definition() {
    let face = roads_fixture_face();
    let [ability] = face.static_abilities.as_slice() else {
        panic!("fixture must carry exactly one static ability");
    };
    assert_eq!(
        ability,
        &IdentifiedAbility {
            ability_id: tricerules_cards::AbilityId::new("static_01").expect("stable id"),
            presentation: AbilityPresentation::Fallback,
            definition: StaticAbilityDef::EntersTapped {
                affected: EntersTappedAffected::Self_,
                condition: Some(GameCondition::BattlefieldAggregate {
                    filter: BattlefieldPermanentFilter {
                        token: None,
                        any_of: Some(vec![
                            BattlefieldPermanentFilter {
                                token: None,
                                any_of: None,
                                controllers: RelativePlayerSet::Controller,
                                card_type: None,
                                color: None,
                                name: None,
                                required_subtypes: vec!["Mount".into()],
                                exclude_source: false,
                            },
                            BattlefieldPermanentFilter {
                                token: None,
                                any_of: None,
                                controllers: RelativePlayerSet::Controller,
                                card_type: None,
                                color: None,
                                name: None,
                                required_subtypes: vec!["Vehicle".into()],
                                exclude_source: false,
                            },
                        ]),
                        controllers: RelativePlayerSet::Controller,
                        card_type: None,
                        color: None,
                        name: None,
                        required_subtypes: Vec::new(),
                        exclude_source: false,
                    },
                    aggregate: BattlefieldAggregate::Count,
                    min: None,
                    max: Some(0),
                }),
                unless_cost: None,
            },
        }
    );
}
