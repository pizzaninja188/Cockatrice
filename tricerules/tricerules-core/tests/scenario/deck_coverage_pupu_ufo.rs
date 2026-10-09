//! Exact PuPu UFO identity and its two activated abilities.

use super::helpers::*;
use tricerules_cards::{
    AbilityCost, AbilityPresentation, AbilitySourceZone, CounterKind, Keyword, Layout, ManaCost,
};
use tricerules_core::{GameEngine, Zone};

fn pupu_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("forest", &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn pupu_engine_with_basic(seed: u64, basic: &str) -> GameEngine {
    let decks = Some(vec![deck_with(basic, &[]), deck_with(basic, &[])]);
    let mut engine = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("new game");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

#[test]
fn pupu_ufo_registers_its_complete_printed_identity() {
    let card = tricerules_cards::registry::global()
        .get("pupu_ufo")
        .expect("complete PuPu UFO definition");
    assert_eq!(card.id, "pupu_ufo");
    assert_eq!(card.name, "PuPu UFO");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "pupu_ufo");
    assert_eq!(face.mana_cost.to_string(), "{2}");
    assert_eq!(
        face.types,
        vec![
            "Artifact".to_string(),
            "Creature".to_string(),
            "Construct".to_string(),
            "Alien".to_string()
        ]
    );
    assert_eq!(face.power, Some(0));
    assert_eq!(face.toughness, Some(4));
    assert_eq!(face.keywords, vec![Keyword::Flying]);

    assert_eq!(face.activated_abilities.len(), 2);
    let put_land = &face.activated_abilities[0];
    assert_eq!(put_land.ability_id.as_str(), "put_land_from_hand");
    assert_eq!(
        put_land.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(put_land.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(put_land.costs, vec![AbilityCost::Tap]);

    let town_power = &face.activated_abilities[1];
    assert_eq!(town_power.ability_id.as_str(), "set_power_to_towns");
    assert_eq!(
        town_power.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(town_power.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        town_power.costs,
        vec![AbilityCost::Mana(ManaCost::parse("{3}").unwrap())]
    );
}

#[test]
fn pupu_ufo_may_put_a_private_land_choice_onto_the_battlefield_without_playing_it() {
    let mut engine = pupu_engine(20_261_006);
    let pupu = inject_permanent_on_battlefield(&mut engine, 0, "pupu_ufo");

    let normal_land = inject_card_into_hand(&mut engine, 0, "forest");
    let normal_land_index = engine.state.players[0]
        .hand
        .iter()
        .position(|object_id| *object_id == normal_land)
        .expect("injected Forest in hand");
    semantic::accepted(&mut engine, 0, &play_land(normal_land_index));
    assert_eq!(engine.state.lands_played_this_turn, 1);
    assert_eq!(engine.state.objects[&normal_land].zone, Zone::Battlefield);

    let chosen_land = inject_card_into_hand(&mut engine, 0, "blossoming_sands");
    let nonland = inject_card_into_hand(&mut engine, 0, "ornithopter");
    let activation = activate_ability_for(&engine, pupu, 0, Vec::new());
    semantic::accepted(&mut engine, 0, &activation);
    semantic::accepted(&mut engine, 0, &pass());
    let parked = semantic::accepted(&mut engine, 1, &pass());
    let choice = find_resolution_choice(&parked).expect("private optional land choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.min, 0);
    assert_eq!(choice.max, 1);
    assert_eq!(
        choice.candidate_object_ids.len(),
        choice.candidate_selectable.len()
    );
    let chosen_land_index = choice
        .candidate_object_ids
        .iter()
        .position(|object_id| *object_id == chosen_land)
        .expect("chosen land candidate");
    let nonland_index = choice
        .candidate_object_ids
        .iter()
        .position(|object_id| *object_id == nonland)
        .expect("nonland candidate");
    assert!(choice.candidate_selectable[chosen_land_index]);
    assert!(!choice.candidate_selectable[nonland_index]);
    assert!(engine.state.objects[&pupu].tapped);
    assert_eq!(engine.state.objects[&chosen_land].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&nonland].zone, Zone::Hand);

    let revision = engine.state.command_index;
    assert!(engine
        .apply_command(0, &submit_resolution_choice(vec![nonland]))
        .is_err());
    assert_eq!(engine.state.command_index, revision);
    assert!(engine.state.pending_resolution.is_some());
    assert_eq!(engine.state.objects[&chosen_land].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&nonland].zone, Zone::Hand);

    let life_before = engine.state.players[0].life;
    semantic::accepted(&mut engine, 0, &submit_resolution_choice(vec![chosen_land]));
    assert_eq!(engine.state.objects[&chosen_land].zone, Zone::Battlefield);
    assert_eq!(engine.state.objects[&chosen_land].controller, 0);
    assert!(engine.state.objects[&chosen_land].tapped);
    assert_eq!(engine.state.objects[&nonland].zone, Zone::Hand);
    assert_eq!(engine.state.lands_played_this_turn, 1);
    assert!(engine
        .state
        .stack
        .iter()
        .any(|item| item.card_id == "blossoming_sands"));
    semantic::complete(&mut engine, 8, |_| None).require_exercised();
    assert_eq!(engine.state.players[0].life, life_before + 1);
}

#[test]
fn pupu_ufo_can_decline_its_land_choice_and_does_not_prompt_without_a_land() {
    let mut engine = pupu_engine(20_261_007);
    let pupu = inject_permanent_on_battlefield(&mut engine, 0, "pupu_ufo");
    let land = inject_card_into_hand(&mut engine, 0, "forest");
    let _nonland = inject_card_into_hand(&mut engine, 0, "ornithopter");

    let activation = activate_ability_for(&engine, pupu, 0, Vec::new());
    semantic::accepted(&mut engine, 0, &activation);
    semantic::accepted(&mut engine, 0, &pass());
    let parked = semantic::accepted(&mut engine, 1, &pass());
    let choice = find_resolution_choice(&parked).expect("optional land choice");
    assert_eq!(choice.min, 0);
    semantic::accepted(&mut engine, 0, &submit_resolution_choice(Vec::new()));
    assert_eq!(engine.state.objects[&land].zone, Zone::Hand);
    assert_eq!(engine.state.objects[&pupu].zone, Zone::Battlefield);
    assert!(engine.state.objects[&pupu].tapped);
    assert!(engine.state.stack.is_empty());

    let mut no_land = pupu_engine_with_basic(20_261_008, "ornithopter");
    let source = inject_permanent_on_battlefield(&mut no_land, 0, "pupu_ufo");
    inject_card_into_hand(&mut no_land, 0, "ornithopter");
    let activation = activate_ability_for(&no_land, source, 0, Vec::new());
    semantic::accepted(&mut no_land, 0, &activation);
    semantic::accepted(&mut no_land, 0, &pass());
    let resolved = semantic::accepted(&mut no_land, 1, &pass());
    assert!(find_resolution_choice(&resolved).is_none());
    assert!(no_land.state.pending_resolution.is_none());
    assert!(no_land.state.stack.is_empty());
    assert!(no_land.state.objects[&source].tapped);
}

#[test]
fn pupu_ufo_snapshots_its_controllers_towns_as_base_power_until_cleanup() {
    let mut engine = pupu_engine(20_261_009);
    let pupu = inject_permanent_on_battlefield(&mut engine, 0, "pupu_ufo");
    inject_permanent_on_battlefield(&mut engine, 0, "gongaga,_reactor_town");
    inject_permanent_on_battlefield(&mut engine, 1, "windurst,_federation_center");
    inject_permanent_on_battlefield(&mut engine, 1, "capital_city");
    engine
        .state
        .objects
        .get_mut(&pupu)
        .unwrap()
        .add_counters(CounterKind::PlusOnePlusOne, 1, 0);
    grant_pool(&mut engine, 0);

    let activation = activate_ability_for(&engine, pupu, 1, Vec::new());
    semantic::accepted(&mut engine, 0, &activation);
    // A land entering after activation but before resolution counts; opponents' Towns do not.
    inject_permanent_on_battlefield(&mut engine, 0, "rabanastre,_royal_city");
    semantic::accepted(&mut engine, 0, &pass());
    semantic::accepted(&mut engine, 1, &pass());
    let characteristics = engine.characteristics(pupu).expect("PuPu characteristics");
    assert_eq!(characteristics.power, Some(3));
    assert_eq!(characteristics.toughness, Some(5));

    // The resolution-time count is a snapshot, not a continuously re-evaluated count.
    inject_permanent_on_battlefield(&mut engine, 0, "adventurers_inn");
    assert_eq!(engine.characteristics(pupu).unwrap().power, Some(3));
    assert_eq!(engine.characteristics(pupu).unwrap().toughness, Some(5));

    end_active_turn(&mut engine, 0);
    assert_eq!(engine.characteristics(pupu).unwrap().power, Some(1));
    assert_eq!(engine.characteristics(pupu).unwrap().toughness, Some(5));
}
