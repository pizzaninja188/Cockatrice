//! Issue #429 — command-boundary scenarios for the four newly eligible Aura identities.
//!
//! Every expectation is the reviewed printed Oracle behavior. Exact Scryfall records and
//! `rulings_uri` responses were fetched 2026-09-20 against pinned snapshot
//! `27bf3214-1271-490b-bdfe-c0be6c23d02e`. Exercised rulings: New Horizons (a cast with no
//! controlled creature is legal and its entry trigger simply has no legal target; a fizzled Aura
//! never triggers), Friendly Neighborhood (the creature count is determined as the granted
//! ability resolves), and Stop Cold (an ability granted after attachment is kept). Governing CR
//! concepts: 608.2b (fizzled Aura), 603.6a (entry triggers), 701.26 (tap), 122.1 (counters),
//! 111.1 (tokens), 605.1a (granted mana ability), 613.1f/613.7 (layer-6 removal and timestamps),
//! and 502.3 (untap-step restriction).

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_proto::ruled::v1::{ruled_command::Cmd, ChooseTriggerTarget, RuledCommand};

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: Vec::new(),
            targets: target_object(object_id),
        })),
    }
}

fn activate_mana_option(
    engine: &GameEngine,
    permanent_id: u32,
    ability_index: u32,
    option: u32,
) -> RuledCommand {
    let mut command = activate_ability_for(engine, permanent_id, ability_index, vec![]);
    let Some(Cmd::ActivateAbility(ability)) = command.cmd.as_mut() else {
        unreachable!("activate_ability_for returns an activation")
    };
    ability.mana_option_index = option;
    command
}

#[test]
fn stop_cold_taps_removes_abilities_and_keeps_a_later_grant() {
    let decks = Some(vec![
        deck_with("island", &["stop_cold", "flight"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(429_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let farrier = inject_creature_on_battlefield(&mut engine, 1, "surly_farrier");
    assert_eq!(
        zone_view_ability_flags(&mut engine, 1, farrier).len(),
        1,
        "Surly Farrier publishes its printed activated ability before the Aura attaches"
    );

    ensure_card_in_hand(&mut engine, 0, "stop_cold");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "stop_cold");
    engine
        .apply_command(0, &cast_spell(slot, target_object(farrier)))
        .expect("cast Stop Cold on a creature");
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        engine.state.objects[&farrier].tapped,
        "the entry trigger taps the enchanted creature"
    );
    assert!(
        zone_view_ability_flags(&mut engine, 1, farrier).is_empty(),
        "the enchanted permanent loses every activated ability"
    );

    // Stop Cold's 2024-04-12 ruling: an ability gained after attachment is kept. Flight is
    // attached later, so its layer-6 grant has a later timestamp than the removal effect.
    ensure_card_in_hand(&mut engine, 0, "flight");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "flight");
    engine
        .apply_command(0, &cast_spell(slot, target_object(farrier)))
        .expect("cast Flight on the same creature");
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        engine.effective_has_keyword(farrier, Keyword::Flying),
        "a later ability grant is kept"
    );

    end_active_turn(&mut engine, 0);
    assert!(
        engine.state.objects[&farrier].tapped,
        "the enchanted creature does not untap during its controller's untap step"
    );
}

#[test]
fn flood_the_engine_requires_a_creature_or_vehicle_and_taps_it() {
    let decks = Some(vec![
        deck_with("island", &["flood_the_engine"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(429_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    ensure_card_in_hand(&mut engine, 0, "flood_the_engine");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "flood_the_engine");
    assert!(
        engine
            .apply_command(0, &cast_spell(slot, target_object(land)))
            .is_err(),
        "a land is not a legal Enchant creature or Vehicle target"
    );
    let slot = hand_index_for_card(&engine, 0, "flood_the_engine");
    engine
        .apply_command(0, &cast_spell(slot, target_object(bear)))
        .expect("cast Flood the Engine on a creature");
    resolve_entire_stack_two_player(&mut engine);
    assert!(
        engine.state.objects[&bear].tapped,
        "the entry trigger taps the enchanted permanent"
    );
}

#[test]
fn new_horizons_counters_and_grants_two_mana_of_one_color() {
    let decks = Some(vec![
        deck_with("forest", &["new_horizons"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(429_003, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    ensure_card_in_hand(&mut engine, 0, "new_horizons");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "new_horizons");
    engine
        .apply_command(0, &cast_spell(slot, target_object(land)))
        .expect("cast New Horizons on a land");
    engine.apply_command(0, &pass()).expect("caster passes");
    engine
        .apply_command(1, &pass())
        .expect("New Horizons resolves");
    assert!(
        engine
            .apply_command(0, &choose_trigger_target(opposing))
            .is_err(),
        "the counter trigger targets only a creature you control"
    );
    engine
        .apply_command(0, &choose_trigger_target(bear))
        .expect("choose the controlled creature");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(bear), Some(3));
    assert_eq!(engine.effective_toughness(bear), Some(3));

    // The printed Forest mana ability is index 0; the granted ability is index 1.
    let command = activate_mana_option(&engine, land, 1, 0);
    engine
        .apply_command(0, &command)
        .expect("activate the granted two-mana ability");
    assert_eq!(engine.state.players[0].mana_pool.white, 2);
    assert!(engine.state.objects[&land].tapped);
    assert!(
        engine.state.stack.is_empty(),
        "a mana ability does not stack"
    );
}

#[test]
fn new_horizons_cast_with_no_creature_is_legal_and_has_no_counter_target() {
    let decks = Some(vec![
        deck_with("forest", &["new_horizons"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(429_005, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");

    ensure_card_in_hand(&mut engine, 0, "new_horizons");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "new_horizons");
    engine
        .apply_command(0, &cast_spell(slot, target_object(land)))
        .expect("New Horizons is castable with no creatures you control");
    resolve_entire_stack_two_player(&mut engine);
    let aura = battlefield_object_for_card(&engine, 0, "new_horizons");
    assert_eq!(
        engine.state.objects[&aura].zone,
        tricerules_core::Zone::Battlefield
    );
    assert!(
        engine.state.pending_trigger_order.is_none()
            && engine.state.pending_triggers.is_empty()
            && engine.state.stack.is_empty(),
        "the entry trigger has no legal target and leaves no pending choice"
    );
}

#[test]
fn new_horizons_fizzles_and_never_triggers_when_its_land_leaves() {
    let decks = Some(vec![
        deck_with("forest", &["new_horizons"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(429_006, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");

    ensure_card_in_hand(&mut engine, 0, "new_horizons");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 2,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "new_horizons");
    engine
        .apply_command(0, &cast_spell(slot, target_object(land)))
        .expect("cast New Horizons");
    let aura_stack_id = engine.state.stack.last().expect("Aura on the stack").id;
    {
        let object = engine.state.objects.get_mut(&land).expect("land object");
        object.zone = tricerules_core::Zone::Graveyard;
    }
    engine.state.players[0].battlefield.retain(|&id| id != land);
    engine.state.players[0].graveyard.push(land);

    engine.apply_command(0, &pass()).expect("caster passes");
    engine
        .apply_command(1, &pass())
        .expect("fizzled Aura resolves");
    assert_eq!(
        engine.state.objects.get(&aura_stack_id).map(|o| o.zone),
        Some(tricerules_core::Zone::Graveyard),
        "the Aura goes to the graveyard when its target left (CR 608.2b)"
    );
    assert!(
        engine
            .state
            .continuous_effects
            .iter()
            .all(|effect| effect.source_id != Some(aura_stack_id)),
        "a fizzled Aura creates no attached modifier"
    );
    assert!(
        engine.state.pending_trigger_order.is_none()
            && engine.state.pending_triggers.is_empty()
            && engine.state.stack.is_empty(),
        "the fizzled Aura never triggers its entry ability"
    );
}

#[test]
fn friendly_neighborhood_creates_three_citizens_and_pumps_per_creature() {
    let decks = Some(vec![
        deck_with("forest", &["friendly_neighborhood"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(429_004, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    let land = inject_permanent_on_battlefield(&mut engine, 0, "forest");
    let bear = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");

    ensure_card_in_hand(&mut engine, 0, "friendly_neighborhood");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 3,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&engine, 0, "friendly_neighborhood");
    engine
        .apply_command(0, &cast_spell(slot, target_object(land)))
        .expect("cast Friendly Neighborhood on a land");
    resolve_entire_stack_two_player(&mut engine);
    let tokens = battlefield_token_oids(&engine, 0, "human_citizen_gw_1_1");
    assert_eq!(tokens.len(), 3, "the entry trigger creates three tokens");
    for token in &tokens {
        assert_eq!(engine.effective_power(*token), Some(1));
        assert_eq!(engine.effective_toughness(*token), Some(1));
    }

    // The printed Forest mana ability is index 0; the granted pump is index 1. The count is
    // determined as the ability resolves (2025-09-19 ruling): at activation the Bear plus three
    // Citizens is four, and a second Bear joins before the ability resolves so the bonus is +5/+5.
    give_mana(
        &mut engine,
        0,
        ManaGift {
            c: 1,
            ..Default::default()
        },
    );
    apply_ability(&mut engine, 0, land, 1, target_object(bear))
        .expect("activate the granted pump ability");
    let reinforcement = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(bear), Some(7));
    assert_eq!(engine.effective_toughness(bear), Some(7));
    assert_eq!(engine.effective_power(reinforcement), Some(2));
    assert!(engine.state.objects[&land].tapped);
}
