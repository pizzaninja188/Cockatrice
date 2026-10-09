//! Doubling Season's token and counter replacement behavior.
use super::helpers::*;
use tricerules_cards::{AbilityPresentation, CounterKind, Layout};
use tricerules_core::{GameEngine, Zone};
use tricerules_proto::ruled::v1::{ruled_command::Cmd, PaymentMana, PreviewPayment};

const SEASON: &str = "doubling_season";

fn setup(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("forest", &[]), deck_with("island", &[])]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn add_seasons(engine: &mut GameEngine, player: usize, count: usize) {
    for _ in 0..count {
        inject_permanent_on_battlefield(engine, player, SEASON);
    }
}

fn battlefield_card(engine: &GameEngine, player: usize, card_id: &str) -> u32 {
    engine.state.players[player]
        .battlefield
        .iter()
        .copied()
        .find(|object_id| engine.state.objects[object_id].card_id == card_id)
        .expect("card on battlefield")
}

fn seasons_registry_definition() -> &'static tricerules_cards::CardDefinition {
    tricerules_cards::registry::global()
        .get(SEASON)
        .expect("complete Doubling Season definition")
}

fn cast_with_payment(engine: &mut GameEngine, card_id: &str, mana: PaymentMana) {
    inject_card_into_hand(engine, 0, card_id);
    let mut command = cast_spell(hand_index_for_card(engine, 0, card_id), vec![]);
    let Some(Cmd::CastSpell(cast)) = command.cmd.as_mut() else {
        unreachable!()
    };
    let preview = engine.preview_payment(
        0,
        &PreviewPayment {
            cast_spell: Some(cast.clone()),
            ..Default::default()
        },
    );
    assert!(preview.valid, "{preview:?}");
    let mut selection = preview.selection.expect("payment selection");
    selection.mana = Some(mana);
    cast.payment = Some(selection);
    engine
        .apply_command(0, &command)
        .expect("cast with selected payment");
    resolve_entire_stack_two_player(engine);
}

#[test]
fn doubling_season_registers_exact_identity_and_both_replacement_abilities() {
    let card = seasons_registry_definition();
    assert_eq!(card.id, SEASON);
    assert_eq!(card.name, "Doubling Season");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), SEASON);
    assert_eq!(face.mana_cost.to_string(), "{4}{G}");
    assert_eq!(face.types, ["Enchantment"]);
    assert!(!face.colors().is_empty());
    assert_eq!(face.static_abilities.len(), 2);
    assert_eq!(
        face.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        face.static_abilities[1].presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    let definitions = face
        .static_abilities
        .iter()
        .map(|ability| format!("{:?}", ability.definition))
        .collect::<Vec<_>>();
    assert!(definitions
        .iter()
        .any(|definition| definition.contains("DoubleTokensCreatedUnderYourControl")));
    assert!(definitions.iter().any(|definition| {
        definition.contains("DoubleEffectCountersPlacedOnPermanentsYouControl")
    }));
}

#[test]
fn effect_created_tokens_compound_and_keep_their_physical_identity() {
    for (seasons, expected) in [(1, 4), (2, 8)] {
        let mut engine = setup(20_261_008 + seasons as u64);
        add_seasons(&mut engine, 0, seasons);
        inject_card_into_hand(&mut engine, 0, "call_the_cavalry");
        grant_pool(&mut engine, 0);
        let slot = hand_index_for_card(&engine, 0, "call_the_cavalry");
        engine
            .apply_command(0, &cast_spell(slot, vec![]))
            .expect("cast Call the Cavalry");
        resolve_entire_stack_two_player(&mut engine);

        let knights = battlefield_token_oids(&engine, 0, "knight_w_2_2_vigilance");
        assert_eq!(knights.len(), expected);
        assert_eq!(
            knights
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            expected
        );
        for knight in knights {
            assert_eq!(engine.state.objects[&knight].zone, Zone::Battlefield);
            assert_eq!(engine.state.objects[&knight].controller, 0);
            assert!(engine.state.objects[&knight].token_origin.is_some());
            assert_eq!(engine.characteristics(knight).unwrap().power, Some(2));
            assert_eq!(engine.characteristics(knight).unwrap().toughness, Some(2));
        }
    }
}

#[test]
fn token_replacement_uses_each_recipient_controller() {
    for (opponent_seasons, expected) in [(0, 1), (1, 2)] {
        let mut engine = setup(20_261_020 + opponent_seasons as u64);
        add_seasons(&mut engine, 0, 1);
        add_seasons(&mut engine, 1, opponent_seasons);
        let target = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
        inject_card_into_hand(&mut engine, 0, "beast_within");
        grant_pool(&mut engine, 0);
        let slot = hand_index_for_card(&engine, 0, "beast_within");
        engine
            .apply_command(0, &cast_spell(slot, target_object(target)))
            .expect("cast Beast Within");
        resolve_entire_stack_two_player(&mut engine);

        assert_eq!(
            battlefield_token_oids(&engine, 1, "beast_g_3_3").len(),
            expected
        );
        assert!(battlefield_token_oids(&engine, 0, "beast_g_3_3").is_empty());
    }
}

#[test]
fn doublers_preserve_tapped_token_properties() {
    let mut engine = setup(20_261_030);
    add_seasons(&mut engine, 0, 1);
    let moxite = inject_permanent_on_battlefield(&mut engine, 0, "melded_moxite");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 3,
            ..Default::default()
        },
    );
    engine
        .apply_command(0, &activate_ability(moxite, 0, vec![]))
        .expect("activate Melded Moxite");
    resolve_entire_stack_two_player(&mut engine);

    let robots = battlefield_token_oids(&engine, 0, "robot_c_2_2");
    assert_eq!(robots.len(), 2);
    assert!(robots
        .iter()
        .all(|robot| engine.state.objects[robot].tapped));
}

#[test]
fn effect_created_copy_tokens_are_doubled_without_losing_copiable_values() {
    let mut engine = setup(20_261_031);
    add_seasons(&mut engine, 0, 1);
    let source = inject_creature_on_battlefield(&mut engine, 0, "storm_crow");
    inject_card_into_hand(&mut engine, 0, "cackling_counterpart");
    grant_pool(&mut engine, 0);
    let slot = hand_index_for_card(&engine, 0, "cackling_counterpart");
    engine
        .apply_command(0, &cast_spell(slot, target_object(source)))
        .expect("cast Cackling Counterpart");
    resolve_entire_stack_two_player(&mut engine);

    let mut copies = engine
        .state
        .objects
        .values()
        .filter(|object| {
            object.zone == Zone::Battlefield
                && object.card_id == "storm_crow"
                && object.token_origin.is_some()
        })
        .map(|object| object.id)
        .collect::<Vec<_>>();
    copies.sort_unstable();
    assert_eq!(copies.len(), 2);
    assert_ne!(copies[0], copies[1]);
    for copy in copies {
        let characteristics = engine.characteristics(copy).expect("copiable values");
        assert_eq!(
            (characteristics.power, characteristics.toughness),
            (Some(1), Some(2))
        );
        assert_eq!(
            engine.state.objects[&copy].controller, 0,
            "copy token remains under the effect controller"
        );
    }
}

#[test]
fn effect_and_entry_counters_are_doubled() {
    let mut effect = setup(20_261_040);
    add_seasons(&mut effect, 0, 1);
    let bear = inject_creature_on_battlefield(&mut effect, 0, "grizzly_bears");
    inject_card_into_hand(&mut effect, 0, "battlegrowth");
    give_mana(
        &mut effect,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&effect, 0, "battlegrowth");
    effect
        .apply_command(0, &cast_spell(slot, target_object(bear)))
        .expect("cast Battlegrowth");
    resolve_entire_stack_two_player(&mut effect);
    assert_eq!(
        effect.state.objects[&bear].counter_count(CounterKind::PlusOnePlusOne),
        2
    );

    let mut entry = setup(20_261_041);
    add_seasons(&mut entry, 0, 1);
    give_mana(
        &mut entry,
        0,
        ManaGift {
            r: 1,
            g: 1,
            ..Default::default()
        },
    );
    cast_with_payment(
        &mut entry,
        "pentad_prism",
        PaymentMana {
            r: 1,
            g: 1,
            ..Default::default()
        },
    );
    let prism = battlefield_card(&entry, 0, "pentad_prism");
    assert_eq!(
        entry.state.objects[&prism].counter_count(CounterKind::Charge),
        4
    );
}
