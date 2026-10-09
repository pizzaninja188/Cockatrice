use super::helpers::*;
use tricerules_cards::{Color, CounterKind};
use tricerules_core::{GameEngine, TurnStep, Zone};

const FANATIC: &str = "fanatic_of_rhonas";
const TOKEN: &str = "fanatic_of_rhonas_b_4_4_zombie_eternalized";

fn eternalize_command(e: &GameEngine, source: u32) -> RuledCommand {
    let mut command = activate_ability_for(e, source, 2, vec![]);
    if let Some(tricerules_proto::ruled::v1::ruled_command::Cmd::ActivateAbility(ability)) =
        command.cmd.as_mut()
    {
        ability.source_zone = tricerules_proto::ruled::v1::AbilitySourceZone::Graveyard as i32;
    }
    command
}

fn game(seed: u64) -> GameEngine {
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        Some(vec![deck_with("forest", &[]), deck_with("island", &[])]),
        true,
    )
    .unwrap();
    advance_to_main1_from_game_start(&mut e);
    e
}

fn cast(e: &mut GameEngine, name: &str, targets: Vec<TargetRef>) -> u32 {
    let id = inject_card_into_hand(e, 0, name);
    grant_pool(e, 0);
    let slot = hand_index_for_card(e, 0, name);
    e.apply_command(0, &cast_spell(slot, targets))
        .expect("cast exact registered card");
    resolve_entire_stack_two_player(e);
    e.state.players[0].mana_pool = Default::default();
    id
}

fn reject(e: &mut GameEngine, actor: i32, cmd: &RuledCommand) {
    let before = format!("{:?}", e.state);
    e.apply_command(actor, cmd)
        .expect_err("illegal activation rejects");
    assert_eq!(format!("{:?}", e.state), before);
}

fn pay_eternalize(e: &mut GameEngine) {
    e.state.players[0].mana_pool.green = 2;
    e.state.players[0].mana_pool.colorless = 2;
}

fn dead(e: &mut GameEngine) -> (u32, RuledCommand) {
    let source = cast(e, FANATIC, vec![]);
    let stale = eternalize_command(e, source);
    e.state
        .objects
        .get_mut(&source)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 2);
    e.state.objects.get_mut(&source).unwrap().tapped = true;
    cast(e, "giant_growth", target_object(source));
    assert_eq!(e.effective_power(source), Some(6));
    cast(e, "murder", target_object(source));
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(e.state.priority_player_id(), 0);
    (source, stale)
}

fn eternalize(e: &mut GameEngine, source: u32) -> u32 {
    pay_eternalize(e);
    e.apply_command(0, &eternalize_command(e, source)).unwrap();
    assert_eq!(e.state.objects[&source].zone, Zone::Exile);
    assert_eq!(e.state.players[0].mana_pool, Default::default());
    assert!(battlefield_token_oids(e, 0, TOKEN).is_empty());
    assert_eq!(e.state.stack.len(), 1);
    resolve_entire_stack_two_player(e);
    let tokens = battlefield_token_oids(e, 0, TOKEN);
    assert_eq!(tokens.len(), 1);
    tokens[0]
}

fn assert_token_values(e: &GameEngine, id: u32) {
    let c = e.characteristics(id).unwrap();
    assert_eq!(c.names, ["Fanatic of Rhonas"]);
    assert_eq!(c.colors, [Color::Black]);
    let mut types = c.types;
    types.sort();
    assert_eq!(types, ["Creature", "Druid", "Snake", "Zombie"]);
    assert_eq!(c.mana_value, 0);
    assert_eq!(c.power, Some(4));
    assert_eq!(c.toughness, Some(4));
    assert!(e.state.objects[&id].counters.is_empty());
    assert!(!e.state.objects[&id].tapped);
}

#[test]
fn exact_source_and_token_preserve_all_three_copiable_abilities() {
    let registry = tricerules_cards::registry::global();
    let card = registry.get(FANATIC).expect("exact Fanatic registered");
    assert_eq!(card.name, "Fanatic of Rhonas");
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{1}{G}");
    assert_eq!(face.types, ["Creature", "Snake", "Druid"]);
    assert_eq!(face.power, Some(1));
    assert_eq!(face.toughness, Some(4));
    assert_eq!(face.activated_abilities.len(), 3);
    let token = registry
        .get(TOKEN)
        .expect("complete Eternalize token registered");
    let face = token.primary_face();
    assert_eq!(face.name, "Fanatic of Rhonas");
    assert_eq!(face.mana_cost.to_string(), "");
    assert_eq!(face.colors_override, Some(vec![Color::Black]));
    assert_eq!(face.activated_abilities.len(), 3);
    assert_eq!(
        face.activated_abilities[2].source_zone,
        tricerules_cards::primitives::AbilitySourceZone::Graveyard
    );
}

#[test]
fn both_mana_abilities_respect_summoning_sickness_and_tap_costs() {
    let mut e = game(26_130_100);
    let source = cast(&mut e, FANATIC, vec![]);
    inject_permanent_on_battlefield(&mut e, 0, "air_elemental");
    assert!(e.state.objects[&source].summoning_sick);
    for ability in 0..2 {
        let cmd = activate_ability_for(&e, source, ability, vec![]);
        reject(&mut e, 0, &cmd);
    }
    end_active_turn(&mut e, 0);
    advance_to_main1_from_game_start(&mut e);
    end_active_turn(&mut e, 1);
    advance_to_main1_from_game_start(&mut e);
    assert!(!e.state.objects[&source].summoning_sick);
    for (ability, amount) in [(0, 1), (1, 4)] {
        e.state.objects.get_mut(&source).unwrap().tapped = false;
        e.state.players[0].mana_pool = Default::default();
        let cmd = activate_ability_for(&e, source, ability, vec![]);
        e.apply_command(0, &cmd).unwrap();
        assert_eq!(e.state.players[0].mana_pool.green, amount);
        assert!(e.state.objects[&source].tapped);
        assert!(e.state.stack.is_empty());
        reject(&mut e, 0, &cmd);
    }
}

#[test]
fn ferocious_requires_current_derived_power_of_a_controlled_creature() {
    let mut e = game(26_130_200);
    let source = cast(&mut e, FANATIC, vec![]);
    e.state.objects.get_mut(&source).unwrap().summoning_sick = false;
    inject_permanent_on_battlefield(&mut e, 1, "air_elemental");
    let own = inject_permanent_on_battlefield(&mut e, 0, "hill_giant");
    let cmd = activate_ability_for(&e, source, 1, vec![]);
    reject(&mut e, 0, &cmd);
    e.state
        .objects
        .get_mut(&own)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    assert_eq!(e.effective_power(own), Some(4));
    e.apply_command(0, &cmd).unwrap();
    assert_eq!(e.state.players[0].mana_pool.green, 4);
    e.state.objects.get_mut(&source).unwrap().tapped = false;
    e.state
        .objects
        .get_mut(&own)
        .unwrap()
        .set_counter(CounterKind::PlusOnePlusOne, 0);
    reject(&mut e, 0, &cmd);
}

#[test]
fn eternalize_exiles_immediately_and_creates_exact_printed_value_token_on_resolution() {
    let mut e = game(26_130_300);
    let (source, _) = dead(&mut e);
    let token = eternalize(&mut e, source);
    assert!(e.state.objects[&token].is_token());
    assert_token_values(&e, token);
    assert!(e.state.objects[&token].summoning_sick);
    let cmd = activate_ability_for(&e, token, 1, vec![]);
    reject(&mut e, 0, &cmd);
    end_active_turn(&mut e, 0);
    advance_to_main1_from_game_start(&mut e);
    end_active_turn(&mut e, 1);
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &cmd).unwrap();
    assert_eq!(
        e.state.players[0].mana_pool.green, 4,
        "token alone qualifies itself for ferocious"
    );
    assert!(e.state.stack.is_empty());
}

#[test]
fn eternalize_rejects_wrong_actor_stale_generation_and_insufficient_colored_payment() {
    let mut e = game(26_130_400);
    let (source, stale) = dead(&mut e);
    let cmd = eternalize_command(&e, source);
    reject(&mut e, 0, &cmd);
    e.state.players[0].mana_pool.red = 4;
    reject(&mut e, 0, &cmd);
    e.state.players[0].mana_pool = Default::default();
    pay_eternalize(&mut e);
    reject(&mut e, 1, &cmd);
    reject(&mut e, 0, &stale);
    e.apply_command(0, &cmd).unwrap();
    assert_eq!(e.state.objects[&source].zone, Zone::Exile);
    reject(&mut e, 0, &cmd);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(battlefield_token_oids(&e, 0, TOKEN).len(), 1);
}

#[test]
fn eternalize_requires_own_main_priority_and_empty_stack() {
    let mut e = game(26_130_500);
    let (source, _) = dead(&mut e);
    inject_card_into_hand(&mut e, 0, "sol_ring");
    e.state.players[0].mana_pool.colorless = 1;
    let slot = hand_index_for_card(&e, 0, "sol_ring");
    e.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pay_eternalize(&mut e);
    let cmd = eternalize_command(&e, source);
    reject(&mut e, 0, &cmd);
    resolve_entire_stack_two_player(&mut e);
    pass_both_players(&mut e);
    assert_eq!(e.state.turn_step, TurnStep::BeginCombat);
    pay_eternalize(&mut e);
    reject(&mut e, 0, &cmd);
    for _ in 0..8 {
        if e.state.active_player_id() != 0 {
            break;
        }
        e.apply_command(0, &primitive_yield()).unwrap();
    }
    advance_to_main1_from_game_start(&mut e);
    pay_eternalize(&mut e);
    reject(&mut e, 0, &cmd);
}

#[test]
fn clone_copies_all_eternalized_values_and_abilities_but_remains_a_physical_card() {
    let mut e = game(26_130_600);
    let (source, _) = dead(&mut e);
    let token = eternalize(&mut e, source);
    let clone = inject_card_into_hand(&mut e, 0, "clone");
    grant_pool(&mut e, 0);
    let slot = hand_index_for_card(&e, 0, "clone");
    e.apply_command(0, &cast_spell(slot, vec![])).unwrap();
    pass_both_players(&mut e);
    e.apply_command(0, &submit_resolution_choice(vec![token]))
        .unwrap();
    assert_eq!(e.state.objects[&clone].zone, Zone::Battlefield);
    assert!(!e.state.objects[&clone].is_token());
    assert_token_values(&e, clone);
    let copied = e.state.objects[&clone].copiable_values.as_ref().unwrap();
    assert_eq!(copied.source_card_id, TOKEN);
    assert_eq!(copied.face.mana_cost.to_string(), "");
    assert_eq!(copied.face.activated_abilities.len(), 3);
    assert_eq!(
        copied.face.activated_abilities[2].source_zone,
        tricerules_cards::primitives::AbilitySourceZone::Graveyard
    );
}
