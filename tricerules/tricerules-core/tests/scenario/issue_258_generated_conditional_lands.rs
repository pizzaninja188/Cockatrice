//! Issue #258 — generated conditional and Surveil lands reuse established activation,
//! derived-characteristic entry, multiplayer life-snapshot, and replacement-order paths.
//!
//! Oracle and rulings checked 2026-09-11. Current CR 113.6h, 602.1-.2, 603.6d,
//! 614.1d, 614.12, 616.1, 701.25, and 701.26 govern these abilities.

use super::helpers::*;
use tricerules_cards::primitives::{
    ContinuousEffectKind, EffectDuration, PermanentTypeFilter, TypeLineAddition,
};
use tricerules_core::{AffectedScope, ContinuousEffect, Zone};
use tricerules_proto::ruled::v1::{ruled_command::Cmd, ChoiceKind};

fn seat_on_top(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let object_id = inject_library_card(engine, player, card_id);
    engine.state.players[player]
        .library
        .retain(|candidate| *candidate != object_id);
    engine.state.players[player].library.push_front(object_id);
    object_id
}

fn play_from_hand(engine: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let object_id = relocate_to_hand(engine, player, card_id);
    let hand_index = hand_index_for_card(engine, player, card_id);
    engine
        .apply_command(engine.state.players[player].id, &play_land(hand_index))
        .unwrap_or_else(|error| panic!("play {card_id}: {error}"));
    assert_eq!(engine.state.objects[&object_id].zone, Zone::Battlefield);
    object_id
}

fn add_land_type(engine: &mut GameEngine, object_id: u32) {
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(object_id),
        kind: ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Land],
            creature_types: Vec::new(),
        }),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });
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
fn generated_surveil_activation_is_priority_generation_and_payment_safe_and_private() {
    let decks = Some(vec![
        deck_with("forest", &["savage_mansion"]),
        deck_with("island", &[]),
    ]);
    let mut engine = GameEngine::new(258_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let source = relocate_to_battlefield(&mut engine, 0, "savage_mansion", false);
    let surveilled = seat_on_top(&mut engine, 0, "storm_crow");

    engine.state.priority_idx = 1;
    engine
        .apply_command(0, &activate_ability_for(&engine, source, 1, vec![]))
        .expect_err("controller cannot activate without priority");
    engine.state.priority_idx = 0;

    let unfunded = activate_ability_for(&engine, source, 1, vec![]);
    engine
        .apply_command(0, &unfunded)
        .expect_err("insufficient mana rejects the whole activation");
    assert!(!engine.state.objects[&source].tapped);
    assert!(engine.state.stack.is_empty());

    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 4,
            ..Default::default()
        },
    );
    let mut stale = activate_ability_for(&engine, source, 1, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = stale.cmd.as_mut() else {
        unreachable!()
    };
    ability.expected_zone_change_generation += 1;
    engine
        .apply_command(0, &stale)
        .expect_err("stale source generation rejects before payment");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 4);
    assert!(!engine.state.objects[&source].tapped);

    apply_ability(&mut engine, 0, source, 1, vec![]).expect("activate Surveil ability");
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
    assert!(engine.state.objects[&source].tapped);
    assert_eq!(engine.state.stack.len(), 1);

    engine.apply_command(0, &pass()).expect("first pass");
    let resolving = engine.apply_command(1, &pass()).expect("resolve ability");
    let choice = find_resolution_choice(&resolving).expect("private Surveil choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.candidate_object_ids, [surveilled]);
    assert!(choice.public_reveal.is_none());
    engine
        .apply_command(1, &submit_resolution_choice(vec![surveilled]))
        .expect_err("opponent cannot submit the private choice");
    engine
        .apply_command(0, &submit_resolution_choice(vec![surveilled]))
        .expect("move exact Surveil candidate");
    assert_eq!(engine.state.objects[&surveilled].zone, Zone::Graveyard);
}

#[test]
fn generated_fast_and_slow_lands_use_other_current_derived_lands_at_boundaries() {
    for (card_id, count, expected_tapped) in [
        ("concealed_courtyard", 0, false),
        ("concealed_courtyard", 2, false),
        ("concealed_courtyard", 3, true),
        ("sundown_pass", 0, true),
        ("sundown_pass", 1, true),
        ("sundown_pass", 2, false),
    ] {
        let decks = Some(vec![
            deck_with("island", &[card_id]),
            deck_with("forest", &[]),
        ]);
        let mut engine =
            GameEngine::new(258_100 + count, &[0, 1], 20, decks, true).expect("engine");
        advance_to_main1_from_game_start(&mut engine);
        for _ in 0..count {
            inject_permanent_on_battlefield(&mut engine, 0, "island");
        }
        let land = play_from_hand(&mut engine, 0, card_id);
        assert_eq!(
            engine.state.objects[&land].tapped, expected_tapped,
            "{card_id} with {count} other lands"
        );
    }

    let decks = Some(vec![
        deck_with("island", &["botanical_sanctum"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(258_200, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "island");
    inject_permanent_on_battlefield(&mut engine, 0, "island");
    let creature = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    add_land_type(&mut engine, creature);
    let land = play_from_hand(&mut engine, 0, "botanical_sanctum");
    assert!(
        engine.state.objects[&land].tapped,
        "the type-changed creature is a current derived Land"
    );
}

#[test]
fn a_copy_of_a_generated_fast_land_rechecks_its_intrinsic_entry_condition() {
    let decks = Some(vec![
        deck_with("island", &["concealed_courtyard", "clone"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(258_300, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    inject_permanent_on_battlefield(&mut engine, 0, "island");
    inject_permanent_on_battlefield(&mut engine, 0, "island");
    let source = relocate_to_battlefield(&mut engine, 0, "concealed_courtyard", false);
    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(source),
        kind: ContinuousEffectKind::Layer4AddTypes(TypeLineAddition {
            card_types: vec![PermanentTypeFilter::Creature],
            creature_types: vec!["Shapeshifter".into()],
        }),
        condition: None,
        duration: EffectDuration::Indefinite,
        timestamp: engine.state.command_index,
    });

    grant_pool(&mut engine, 0);
    relocate_to_hand(&mut engine, 0, "clone");
    let clone_index = hand_index_for_card(&engine, 0, "clone");
    engine
        .apply_command(0, &cast_spell(clone_index, Vec::new()))
        .expect("cast Clone");
    pass_both_players(&mut engine);
    let choice = engine
        .state
        .pending_resolution
        .as_ref()
        .expect("copy source choice");
    assert_eq!(choice.presentation.choice_kind, ChoiceKind::CopySource);
    assert!(choice.presentation.candidates.contains(&source));
    engine
        .apply_command(0, &submit_resolution_choice(vec![source]))
        .expect("copy generated fast land");

    let clone = battlefield_object_for_card(&engine, 0, "clone");
    assert_eq!(engine.state.objects[&clone].copy_revision, 1);
    assert!(
        engine.state.objects[&clone].tapped,
        "the copied fast-land ability sees three other derived Lands"
    );
}

#[test]
fn generated_life_threshold_land_checks_every_player_at_thirteen_and_fourteen() {
    for (third_life, expected_tapped) in [(13, false), (14, true)] {
        let decks = Some(vec![
            deck_with("plains", &["raucous_carnival"]),
            deck_with("island", &[]),
        ]);
        let mut engine =
            GameEngine::new(258_400 + third_life as u64, &[0, 1], 20, decks, true).expect("engine");
        advance_to_main1_from_game_start(&mut engine);
        add_third_player(&mut engine);
        engine.state.players[1].life = 14;
        engine.state.players[2].life = third_life;
        let land = play_from_hand(&mut engine, 0, "raucous_carnival");
        assert_eq!(
            engine.state.objects[&land].tapped, expected_tapped,
            "minimum life {third_life}"
        );
    }
}

#[test]
fn generated_entry_condition_composes_with_another_replacement_in_either_order() {
    for choose_orb_first in [false, true] {
        let decks = Some(vec![
            deck_with("plains", &["etched_cornfield"]),
            deck_with("forest", &[]),
        ]);
        let mut engine = GameEngine::new(
            258_500 + u64::from(choose_orb_first),
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("engine");
        advance_to_main1_from_game_start(&mut engine);
        inject_permanent_on_battlefield(&mut engine, 0, "orb_of_dreams");
        let land = relocate_to_hand(&mut engine, 0, "etched_cornfield");
        let hand_index = hand_index_for_card(&engine, 0, "etched_cornfield");
        let played = engine
            .apply_command(0, &play_land(hand_index))
            .expect("start conditional land entry");
        let ordering = find_resolution_choice(&played).expect("CR 616 ordering choice");
        assert_eq!(ordering.choice_kind(), ChoiceKind::ReplacementEffect);
        assert_eq!(ordering.candidate_names.len(), 2);
        let wanted = if choose_orb_first {
            "Orb of Dreams"
        } else {
            "Etched Cornfield"
        };
        let index = ordering
            .candidate_names
            .iter()
            .position(|name| name.starts_with(wanted))
            .expect("replacement label");
        engine
            .apply_command(
                0,
                &submit_resolution_choice(vec![ordering.candidate_object_ids[index]]),
            )
            .expect("choose replacement order");
        assert_eq!(engine.state.objects[&land].zone, Zone::Battlefield);
        assert!(
            engine.state.objects[&land].tapped,
            "neither replacement order may clear an established tapped result"
        );
    }
}
