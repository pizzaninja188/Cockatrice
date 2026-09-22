//! Actual-card semantics for five pinned Standard token cards.
//! Exact Scryfall Oracle and rulings checked 2026-09-22.

use super::helpers::*;
use tricerules_cards::{Color, Keyword};
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChooseTriggerTarget, RuledCommand, TargetRef,
};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    e
}

fn cast_permanent(e: &mut GameEngine, card_id: &str) -> u32 {
    inject_card_into_hand(e, 0, card_id);
    grant_pool(e, 0);
    let slot = hand_index_for_card(e, 0, card_id);
    semantic::accepted(e, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(e);
    *e.state.players[0]
        .battlefield
        .iter()
        .find(|&&id| e.state.objects[&id].card_id == card_id)
        .unwrap_or_else(|| panic!("{card_id} on battlefield"))
}

fn choose_trigger(targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            targets,
            ..Default::default()
        })),
    }
}

#[test]
fn issue_misc31_mine_raider_another_outlaw() {
    let mut self_only = engine(831_001);
    cast_permanent(&mut self_only, "mine_raider");
    assert!(battlefield_token_oids(&self_only, 0, "treasure").is_empty());

    let mut opponent_only = engine(831_002);
    inject_creature_on_battlefield(&mut opponent_only, 1, "corsair_captain");
    cast_permanent(&mut opponent_only, "mine_raider");
    assert!(battlefield_token_oids(&opponent_only, 0, "treasure").is_empty());

    let mut own_outlaw = engine(831_003);
    inject_creature_on_battlefield(&mut own_outlaw, 0, "corsair_captain");
    cast_permanent(&mut own_outlaw, "mine_raider");
    assert_eq!(battlefield_token_oids(&own_outlaw, 0, "treasure").len(), 1);

    let mut warlock = engine(831_005);
    inject_creature_on_battlefield(&mut warlock, 0, "eternal_student");
    cast_permanent(&mut warlock, "mine_raider");
    assert_eq!(battlefield_token_oids(&warlock, 0, "treasure").len(), 1);

    let mut lost_before_resolution = engine(831_004);
    let pirate = inject_creature_on_battlefield(&mut lost_before_resolution, 0, "corsair_captain");
    inject_card_into_hand(&mut lost_before_resolution, 0, "mine_raider");
    let slot = hand_index_for_card(&lost_before_resolution, 0, "mine_raider");
    semantic::accepted(&mut lost_before_resolution, 0, &cast_spell(slot, vec![]));
    pass_both_players(&mut lost_before_resolution);
    assert_eq!(
        lost_before_resolution.state.stack.len(),
        1,
        "ETB trigger was created"
    );
    lost_before_resolution.state.players[0]
        .battlefield
        .retain(|id| *id != pirate);
    lost_before_resolution.state.players[0]
        .graveyard
        .push(pirate);
    lost_before_resolution
        .state
        .objects
        .get_mut(&pirate)
        .unwrap()
        .zone = Zone::Graveyard;
    *lost_before_resolution
        .state
        .zone_change_generation
        .entry(pirate)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut lost_before_resolution);
    assert!(battlefield_token_oids(&lost_before_resolution, 0, "treasure").is_empty());
}

#[test]
fn issue_misc31_prompto_actual_mana_spent() {
    let mut e = engine(831_010);
    let prompto = cast_permanent(&mut e, "prompto_argentum");
    assert!(e.effective_has_keyword(prompto, Keyword::Haste));
    inject_card_into_hand(&mut e, 0, "divination");
    grant_pool(&mut e, 0);
    let low = hand_index_for_card(&e, 0, "divination");
    semantic::accepted(&mut e, 0, &cast_spell(low, vec![]));
    assert_eq!(
        e.state.stack.len(),
        1,
        "three mana does not trigger Prompto"
    );
    resolve_entire_stack_two_player(&mut e);
    assert!(battlefield_token_oids(&e, 0, "treasure").is_empty());

    inject_card_into_hand(&mut e, 0, "brasss_bounty");
    grant_pool(&mut e, 0);
    let high = hand_index_for_card(&e, 0, "brasss_bounty");
    semantic::accepted(&mut e, 0, &cast_spell(high, vec![]));
    assert_eq!(
        e.state.stack.len(),
        2,
        "seven mana cast creates a trigger above its spell"
    );
    pass_both_players(&mut e);
    assert_eq!(battlefield_token_oids(&e, 0, "treasure").len(), 1);
    assert_eq!(e.state.stack.len(), 1, "trigger resolved before the spell");
    resolve_entire_stack_two_player(&mut e);

    inject_card_into_hand(&mut e, 0, "grizzly_bears");
    grant_pool(&mut e, 0);
    let creature = hand_index_for_card(&e, 0, "grizzly_bears");
    semantic::accepted(&mut e, 0, &cast_spell(creature, vec![]));
    assert_eq!(
        e.state.stack.len(),
        1,
        "creature cast does not trigger Prompto"
    );
}

#[test]
fn issue_misc31_smaug_treasure_count() {
    let mut e = engine(831_020);
    let smaug = inject_creature_on_battlefield(&mut e, 0, "smaug_the_magnificent");
    assert!(e.effective_has_keyword(smaug, Keyword::Flying));
    assert!(e.effective_has_keyword(smaug, Keyword::Haste));
    inject_permanent_on_battlefield(&mut e, 0, "treasure");
    inject_permanent_on_battlefield(&mut e, 0, "treasure");
    inject_permanent_on_battlefield(&mut e, 1, "treasure");
    semantic::accepted(&mut e, 0, &primitive_yield());
    pass_both_players(&mut e);
    assert_eq!(e.state.turn_step, TurnStep::DeclareAttackers);
    semantic::accepted(&mut e, 0, &declare_attackers(vec![smaug]));
    semantic::accepted(&mut e, 0, &choose_trigger(target_player(1)));
    inject_permanent_on_battlefield(&mut e, 0, "treasure");
    pass_both_players(&mut e);
    assert_eq!(
        e.state.players[1].life, 17,
        "three Treasures at resolution, not two at declaration"
    );

    let mut upkeep = engine(831_021);
    inject_creature_on_battlefield(&mut upkeep, 0, "smaug_the_magnificent");
    end_active_turn(&mut upkeep, 0);
    assert!(
        upkeep.state.stack.is_empty(),
        "opponent upkeep does not trigger Smaug"
    );
    for _ in 0..6 {
        if upkeep.state.turn_step == TurnStep::Main1 {
            break;
        }
        pass_both_players(&mut upkeep);
    }
    assert_eq!(upkeep.state.turn_step, TurnStep::Main1);
    end_active_turn(&mut upkeep, 1);
    resolve_entire_stack_two_player(&mut upkeep);
    assert_eq!(battlefield_token_oids(&upkeep, 0, "treasure").len(), 1);
    assert!(battlefield_token_oids(&upkeep, 1, "treasure").is_empty());
}

#[test]
fn issue_misc31_fountainport_four_abilities() {
    let mut mana = engine(831_030);
    let land = inject_permanent_on_battlefield(&mut mana, 0, "fountainport");
    let before = mana.state.players[0].mana_pool.colorless;
    let command = activate_ability_for(&mana, land, 0, vec![]);
    semantic::accepted(&mut mana, 0, &command);
    assert_eq!(mana.state.players[0].mana_pool.colorless, before + 1);
    assert!(mana.state.objects[&land].tapped);

    let mut draw = engine(831_031);
    let land = inject_permanent_on_battlefield(&mut draw, 0, "fountainport");
    let nontoken = inject_permanent_on_battlefield(&mut draw, 0, "forest");
    let bad =
        activate_ability_with_costs(land, 1, vec![], vec![permanent_cost_selection(2, nontoken)]);
    assert!(
        draw.apply_command(0, &bad).is_err(),
        "a card cannot pay the token sacrifice"
    );
    assert!(!draw.state.objects[&land].tapped);
    let treasure = inject_permanent_on_battlefield(&mut draw, 0, "treasure");
    let before = draw.state.players[0].hand.len();
    semantic::accepted(
        &mut draw,
        0,
        &activate_ability_with_costs(land, 1, vec![], vec![permanent_cost_selection(2, treasure)]),
    );
    assert!(draw.state.objects[&land].tapped);
    resolve_entire_stack_two_player(&mut draw);
    assert_eq!(draw.state.players[0].hand.len(), before + 1);
    assert!(!draw.state.players[0].battlefield.contains(&treasure));

    let mut fish = engine(831_032);
    let land = inject_permanent_on_battlefield(&mut fish, 0, "fountainport");
    let life = fish.state.players[0].life;
    let command = activate_ability_for(&fish, land, 2, vec![]);
    semantic::accepted(&mut fish, 0, &command);
    resolve_entire_stack_two_player(&mut fish);
    assert_eq!(fish.state.players[0].life, life - 1);
    let token = battlefield_token_oids(&fish, 0, "fish_u_1_1");
    assert_eq!(token.len(), 1);
    assert_eq!(
        fish.characteristics(token[0]).unwrap().colors,
        vec![Color::Blue]
    );

    let mut gold = engine(831_033);
    let land = inject_permanent_on_battlefield(&mut gold, 0, "fountainport");
    let command = activate_ability_for(&gold, land, 3, vec![]);
    semantic::accepted(&mut gold, 0, &command);
    resolve_entire_stack_two_player(&mut gold);
    assert_eq!(battlefield_token_oids(&gold, 0, "treasure").len(), 1);
}

#[test]
fn issue_misc31_baxter_combat_target() {
    let mut e = engine(831_040);
    cast_permanent(&mut e, "baxter_stockman");
    let robot = battlefield_token_oids(&e, 0, "robot_c_1_1")[0];
    assert_eq!(
        (e.effective_power(robot), e.effective_toughness(robot)),
        (Some(1), Some(1))
    );
    let ordinary = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    semantic::accepted(&mut e, 0, &primitive_yield());
    assert!(
        e.apply_command(0, &choose_trigger(target_object(ordinary)))
            .is_err(),
        "a nonartifact creature is illegal"
    );
    semantic::accepted(&mut e, 0, &choose_trigger(target_object(robot)));
    pass_both_players(&mut e);
    assert_eq!(e.effective_power(robot), Some(4));
    assert_eq!(e.effective_toughness(robot), Some(1));
    assert!(e.effective_has_keyword(robot, Keyword::FirstStrike));
    assert!(e.effective_has_keyword(robot, Keyword::Vigilance));
    assert_eq!(e.effective_power(ordinary), Some(2));
    end_active_turn(&mut e, 0);
    assert_eq!(e.effective_power(robot), Some(1));
    assert!(!e.effective_has_keyword(robot, Keyword::FirstStrike));
}
