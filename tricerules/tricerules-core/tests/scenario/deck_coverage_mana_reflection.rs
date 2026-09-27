//! Actual-card evidence for Mana Reflection from the pinned Bello deck.
//!
//! Scryfall Oracle ID: a3a8a044-283d-443e-bc40-c2f826d70c22; 2XM printing UUID:
//! 81e0d739-990f-4ba5-b456-165c033014cf. The pinned map lists it under Bello. Its only card type
//! is Enchantment, so it is mainboard by exclusion under CR 903.3; the deck API did not expose a
//! directly verified subsection. CR 106.12/106.12b govern the tap-for-mana boundary, and 106.6a
//! keeps an ability's restrictions on increased mana.
//!
//! Triggered-mana evidence is N/A because the current registry has no actual-card triggered
//! mana-producing fixture. Generator Servant's tap-and-sacrifice mana ability qualifies under
//! CR 106.12 because its cost includes {T}; its increased mana must retain its spending rules
//! and Haste rider. Chandra's Embercat separately checks a tap-only mana ability's restriction.

use super::helpers::*;
use tricerules_cards::primitives::StaticAbilityDef;
use tricerules_cards::{AbilityPresentation, CardRegistry, Color, Keyword, Layout};
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::ruled_command::Cmd;

type ManaPool = (u32, u32, u32, u32, u32, u32);

fn mana_pool(engine: &GameEngine, player: usize) -> ManaPool {
    let pool = &engine.state.players[player].mana_pool;
    (
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    )
}

fn cast_mana_reflection(engine: &mut GameEngine) -> u32 {
    let source = inject_card_into_hand(engine, 0, "mana_reflection");
    let generation = semantic::generation(engine, source);
    let slot = hand_index_for_card(engine, 0, "mana_reflection");
    semantic::accepted(engine, 0, &cast_spell(slot, vec![]));
    semantic::assert_object(
        engine,
        source,
        "mana_reflection",
        0,
        0,
        Zone::Stack,
        generation + 1,
    );
    assert_eq!(
        engine.state.stack.last().unwrap().card_id,
        "mana_reflection"
    );
    semantic::complete(engine, 8, |_| None).require_exercised();
    semantic::assert_object(
        engine,
        source,
        "mana_reflection",
        0,
        0,
        Zone::Battlefield,
        generation + 2,
    );
    assert!(engine.state.stack.is_empty());
    source
}

fn main_with_mana_reflections(seed: u64, copies: u32) -> (GameEngine, Vec<u32>) {
    let mut engine = semantic::main_phase(seed);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4 * copies,
            g: 2 * copies,
            ..Default::default()
        },
    );
    let sources = (0..copies)
        .map(|_| cast_mana_reflection(&mut engine))
        .collect();
    (engine, sources)
}

#[test]
fn mana_reflection_has_the_reviewed_identity_characteristics_and_static_multiplier() {
    let card = CardRegistry::global()
        .get("mana_reflection")
        .expect("Mana Reflection is registered");
    assert_eq!(card.id, "mana_reflection");
    assert_eq!(card.name, "Mana Reflection");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "mana_reflection");
    assert_eq!(face.name, "Mana Reflection");
    assert_eq!(face.mana_cost.to_string(), "{4}{G}{G}");
    assert_eq!(face.types, vec!["Enchantment".to_string()]);
    assert_eq!(face.colors(), vec![Color::Green]);
    assert!(face.power.is_none());
    assert!(face.toughness.is_none());
    assert!(
        face.activated_abilities.is_empty(),
        "the enchantment has no mana ability of its own"
    );
    assert_eq!(face.static_abilities.len(), 1);
    let ability = &face.static_abilities[0];
    assert_eq!(ability.ability_id.as_str(), "static_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    match &ability.definition {
        StaticAbilityDef::MultiplyManaFromTappedPermanents { multiplier } => {
            assert_eq!(*multiplier, 2);
        }
        other => panic!("unexpected static ability: {other:?}"),
    }
}

#[test]
fn mana_reflection_doubles_forest_mana_without_changing_its_color_or_using_the_stack() {
    let (mut engine, reflections) = main_with_mana_reflections(342_101, 1);
    let forest = inject_permanent_on_battlefield(&mut engine, 0, "forest");

    let command = activate_ability_for(&engine, forest, 0, vec![]);
    semantic::accepted(&mut engine, 0, &command);

    assert_eq!(mana_pool(&engine, 0), (0, 0, 0, 0, 2, 0));
    assert!(engine.state.objects[&forest].tapped);
    assert!(!engine.state.objects[&reflections[0]].tapped);
    assert!(
        engine.state.stack.is_empty(),
        "mana abilities do not use the stack"
    );
    assert!(engine.state.pending_triggers.is_empty());
    assert!(engine.state.pending_resolution.is_none());
}

#[test]
fn two_mana_reflections_compound_to_four_green_mana_from_one_forest() {
    let (mut engine, reflections) = main_with_mana_reflections(342_102, 2);
    let forest = inject_permanent_on_battlefield(&mut engine, 0, "forest");

    let command = activate_ability_for(&engine, forest, 0, vec![]);
    semantic::accepted(&mut engine, 0, &command);

    assert_eq!(mana_pool(&engine, 0), (0, 0, 0, 0, 4, 0));
    assert!(engine.state.objects[&forest].tapped);
    assert!(reflections
        .iter()
        .all(|source| !engine.state.objects[source].tapped));
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_triggers.is_empty());
}

#[test]
fn an_opponents_mana_reflection_does_not_change_the_active_players_forest_mana() {
    let decks = Some(vec![
        deck_with("forest", &[]),
        deck_with("forest", &["mana_reflection"]),
    ]);
    let mut engine = GameEngine::new(342_103, &[0, 1], 20, decks, true).expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    let opponents_reflection = move_ready_to_battlefield(&mut engine, 1, "mana_reflection");
    let forest = inject_permanent_on_battlefield(&mut engine, 0, "forest");

    let command = activate_ability_for(&engine, forest, 0, vec![]);
    semantic::accepted(&mut engine, 0, &command);

    assert_eq!(mana_pool(&engine, 0), (0, 0, 0, 0, 1, 0));
    assert_eq!(mana_pool(&engine, 1), (0, 0, 0, 0, 0, 0));
    assert!(engine.state.objects[&forest].tapped);
    assert!(!engine.state.objects[&opponents_reflection].tapped);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn mana_reflection_does_not_multiply_dreadship_reefs_non_tap_storage_mana() {
    let (mut engine, reflections) = main_with_mana_reflections(342_104, 1);
    let reef = inject_permanent_on_battlefield(&mut engine, 0, "dreadship_reef");
    engine
        .state
        .objects
        .get_mut(&reef)
        .unwrap()
        .set_counter(tricerules_cards::CounterKind::Storage, 2);
    engine.state.players[0].mana_pool.colorless = 1;

    let mut command = activate_ability_for(&engine, reef, 2, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.x_value = 2;
    activation.mana_split_first_color_count = 1;
    semantic::accepted(&mut engine, 0, &command);

    assert_eq!(mana_pool(&engine, 0), (0, 1, 1, 0, 0, 0));
    assert_eq!(
        engine.state.objects[&reef].counter_count(tricerules_cards::CounterKind::Storage),
        0
    );
    assert!(
        !engine.state.objects[&reef].tapped,
        "this ability has no tap cost"
    );
    assert!(reflections
        .iter()
        .all(|source| !engine.state.objects[source].tapped));
    assert!(engine.state.stack.is_empty());
    assert!(engine.state.pending_triggers.is_empty());
}

#[test]
fn mana_reflection_preserves_the_restriction_on_both_chandras_embercat_mana_units() {
    let (mut engine, reflections) = main_with_mana_reflections(342_105, 1);
    let embercat = inject_permanent_on_battlefield(&mut engine, 0, "chandras_embercat");
    let activation = activate_ability_for(&engine, embercat, 0, vec![]);
    semantic::accepted(&mut engine, 0, &activation);

    assert_eq!(mana_pool(&engine, 0), (0, 0, 0, 0, 0, 0));
    assert_eq!(engine.state.players[0].restricted_mana.len(), 1);
    let contribution = &engine.state.players[0].restricted_mana[0];
    assert_eq!(contribution.amount.r, 2);
    let restriction_group_id = contribution.restriction_group_id;
    assert!(engine.state.objects[&embercat].tapped);
    assert!(engine.state.stack.is_empty());

    inject_card_into_hand(&mut engine, 0, "bonesplitter");
    let artifact_slot = hand_index_for_card(&engine, 0, "bonesplitter");
    let mut disallowed = cast_spell(artifact_slot, vec![]);
    let Some(Cmd::CastSpell(cast)) = disallowed.cmd.as_mut() else {
        unreachable!()
    };
    cast.restricted_mana.push(ManaSpendSelection {
        restriction_group_id,
        r: 1,
        ..Default::default()
    });
    let command_index = engine.state.command_index;
    assert!(engine.apply_command(0, &disallowed).is_err());
    assert_eq!(engine.state.command_index, command_index);
    assert_eq!(engine.state.players[0].restricted_mana[0].amount.r, 2);
    assert!(engine.state.stack.is_empty());

    inject_card_into_hand(&mut engine, 0, "generator_servant");
    let elemental_slot = hand_index_for_card(&engine, 0, "generator_servant");
    let elemental = engine.state.players[0].hand[elemental_slot];
    let mut allowed = cast_spell(elemental_slot, vec![]);
    let Some(Cmd::CastSpell(cast)) = allowed.cmd.as_mut() else {
        unreachable!()
    };
    cast.restricted_mana.push(ManaSpendSelection {
        restriction_group_id,
        r: 2,
        ..Default::default()
    });
    semantic::accepted(&mut engine, 0, &allowed);

    assert_eq!(engine.state.objects[&elemental].zone, Zone::Stack);
    assert_eq!(
        engine.state.stack.last().unwrap().card_id,
        "generator_servant"
    );
    assert!(engine.state.players[0].restricted_mana.is_empty());
    assert_eq!(engine.state.stack.len(), 1);
    semantic::complete(&mut engine, 8, |_| None).require_exercised();
    assert_eq!(engine.state.objects[&elemental].zone, Zone::Battlefield);
    assert_eq!(mana_pool(&engine, 0), (0, 0, 0, 0, 0, 0));
    assert!(!engine.state.objects[&reflections[0]].tapped);
    assert!(engine.state.stack.is_empty());
}

#[test]
fn mana_reflection_doubles_generator_servant_mana_and_preserves_its_rider() {
    let (mut engine, reflections) = main_with_mana_reflections(342_106, 1);
    let servant = inject_creature_on_battlefield(&mut engine, 0, "generator_servant");

    let activation = activate_ability_for(&engine, servant, 0, vec![]);
    semantic::accepted(&mut engine, 0, &activation);

    assert_eq!(engine.state.objects[&servant].zone, Zone::Graveyard);
    assert_eq!(mana_pool(&engine, 0), (0, 0, 0, 0, 0, 0));
    assert_eq!(engine.state.players[0].restricted_mana.len(), 1);
    let group_id = engine.state.players[0].restricted_mana[0].restriction_group_id;
    assert_eq!(
        engine.state.players[0].restricted_mana[0].amount,
        tricerules_cards::ManaAmount {
            c: 4,
            ..Default::default()
        }
    );
    assert_eq!(
        engine.state.objects[&reflections[0]].zone,
        Zone::Battlefield
    );
    assert!(
        engine.state.stack.is_empty(),
        "mana ability does not use the stack"
    );

    let myrs = [
        inject_card_into_hand(&mut engine, 0, "iron_myr"),
        inject_card_into_hand(&mut engine, 0, "iron_myr"),
    ];
    for (spent, myr) in myrs.into_iter().enumerate() {
        let slot = hand_index_for_card(&engine, 0, "iron_myr");
        let mut cast = cast_spell(slot, vec![]);
        let Some(Cmd::CastSpell(announcement)) = cast.cmd.as_mut() else {
            unreachable!()
        };
        announcement.restricted_mana.push(ManaSpendSelection {
            restriction_group_id: group_id,
            c: 2,
            ..Default::default()
        });
        semantic::accepted(&mut engine, 0, &cast);
        assert_eq!(engine.state.objects[&myr].zone, Zone::Stack);
        assert_eq!(engine.state.stack.last().unwrap().card_id, "iron_myr");

        semantic::complete(&mut engine, 8, |_| None).require_exercised();
        assert_eq!(engine.state.objects[&myr].zone, Zone::Battlefield);
        assert!(engine
            .characteristics(myr)
            .is_some_and(|characteristics| characteristics.has_keyword(Keyword::Haste)));
        assert_eq!(engine.state.stack.len(), 0);
        assert_eq!(
            engine.state.players[0]
                .restricted_mana
                .first()
                .map(|contribution| contribution.amount.c)
                .unwrap_or(0),
            2 - (spent as u32 * 2),
            "each Iron Myr uses two mana from the original restricted group"
        );
    }

    assert!(engine.state.players[0].restricted_mana.is_empty());
    assert_eq!(mana_pool(&engine, 0), (0, 0, 0, 0, 0, 0));
    assert!(engine.state.stack.is_empty());
}
