//! Actual-card semantics for five pinned Standard artifact-token cards.
//! Scryfall Oracle and rulings checked 2026-09-22. CR 111.10, 602, 603.6a, 608.2b/c, 702.3b.

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{ruled_command::Cmd, CostSelection, RuledCommand, TargetRef};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    e
}

fn generation(e: &GameEngine, id: u32) -> u64 {
    e.state
        .zone_change_generation
        .get(&id)
        .copied()
        .unwrap_or(0)
}

fn cast(e: &mut GameEngine, card_id: &str) -> u32 {
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

fn activate(
    e: &GameEngine,
    id: u32,
    targets: Vec<TargetRef>,
    costs: Vec<CostSelection>,
) -> RuledCommand {
    let mut command = activate_ability_with_costs(id, 0, targets, costs);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation = generation(e, id);
    command
}

#[test]
fn issue_misc29_biomechan_tokens_and_draw() {
    let mut e = engine(829_001);
    let source = cast(&mut e, "biomechan_engineer");
    assert_eq!(battlefield_token_oids(&e, 0, "lander").len(), 1);
    assert!(battlefield_token_oids(&e, 1, "lander").is_empty());
    let hand_before = e.state.players[0].hand.len();
    let command = activate(&e, source, vec![], vec![]);
    semantic::accepted(&mut e, 0, &command);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].hand.len(), hand_before + 2);
    assert_eq!(battlefield_token_oids(&e, 0, "robot_c_2_2").len(), 1);
}

#[test]
fn issue_misc29_witch_food_sacrifice_and_target() {
    let mut e = engine(829_002);
    let source = cast(&mut e, "sweettooth_witch");
    assert_eq!(battlefield_token_oids(&e, 0, "food").len(), 1);
    let nontoken_food = inject_permanent_on_battlefield(&mut e, 0, "tough_cookie");
    let nonfood = inject_permanent_on_battlefield(&mut e, 0, "forest");
    assert!(
        e.apply_command(
            0,
            &activate(
                &e,
                source,
                target_player(1),
                vec![permanent_cost_selection(1, nonfood)],
            ),
        )
        .is_err(),
        "a land cannot pay the Food sacrifice cost"
    );
    assert_eq!(e.state.objects[&nonfood].zone, Zone::Battlefield);
    let life_before = e.state.players[1].life;
    let payment = permanent_cost_selection(1, nontoken_food);
    let command = activate(&e, source, target_player(1), vec![payment]);
    semantic::accepted(&mut e, 0, &command);
    assert_eq!(
        e.state.objects[&nontoken_food].zone,
        Zone::Graveyard,
        "Food sacrifice pays the cost"
    );
    assert_eq!(
        e.state.players[1].life, life_before,
        "life loss resolves later"
    );
    assert!(
        e.apply_command(
            0,
            &activate(
                &e,
                source,
                target_player(1),
                vec![permanent_cost_selection(1, nontoken_food)]
            )
        )
        .is_err(),
        "one Food cannot pay twice"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, life_before - 2);
    assert_eq!(battlefield_token_oids(&e, 0, "food").len(), 1);
}

#[test]
fn issue_misc29_sentinel_land_cost() {
    let mut e = engine(829_003);
    let source = cast(&mut e, "redrock_sentinel");
    assert!(e.effective_has_keyword(source, Keyword::Defender));
    e.state.objects.get_mut(&source).unwrap().summoning_sick = false;
    let land = inject_permanent_on_battlefield(&mut e, 0, "forest");
    let hand_before = e.state.players[0].hand.len();
    let payment = permanent_cost_selection(2, land);
    let command = activate(&e, source, vec![], vec![payment]);
    semantic::accepted(&mut e, 0, &command);
    assert_eq!(e.state.objects[&land].zone, Zone::Graveyard);
    assert!(e.state.objects[&source].tapped);
    assert_eq!(e.state.players[0].hand.len(), hand_before);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].hand.len(), hand_before + 1);
    assert_eq!(battlefield_token_oids(&e, 0, "treasure").len(), 1);
    assert!(e
        .apply_command(
            0,
            &activate(&e, source, vec![], vec![permanent_cost_selection(2, land)])
        )
        .is_err());
}

#[test]
fn issue_misc29_flick_resolution_and_illegal_target() {
    let mut e = engine(829_004);
    inject_card_into_hand(&mut e, 0, "flick_a_coin");
    let hand_before = e.state.players[0].hand.len();
    let life_before = e.state.players[1].life;
    let slot = hand_index_for_card(&e, 0, "flick_a_coin");
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_player(1)));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[1].life, life_before - 1);
    assert_eq!(battlefield_token_oids(&e, 0, "treasure").len(), 1);
    assert_eq!(e.state.players[0].hand.len(), hand_before);

    let mut fizzle = engine(829_005);
    let target = inject_creature_on_battlefield(&mut fizzle, 1, "grizzly_bears");
    inject_card_into_hand(&mut fizzle, 0, "flick_a_coin");
    let hand_before = fizzle.state.players[0].hand.len();
    let slot = hand_index_for_card(&fizzle, 0, "flick_a_coin");
    semantic::accepted(&mut fizzle, 0, &cast_spell(slot, target_object(target)));
    fizzle.state.players[1]
        .battlefield
        .retain(|id| *id != target);
    fizzle.state.players[1].graveyard.push(target);
    fizzle.state.objects.get_mut(&target).expect("target").zone = Zone::Graveyard;
    *fizzle
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut fizzle);
    assert!(battlefield_token_oids(&fizzle, 0, "treasure").is_empty());
    assert_eq!(fizzle.state.players[0].hand.len(), hand_before - 1);
}

#[test]
fn issue_misc29_bounty_resolution_land_count() {
    let mut e = engine(829_006);
    inject_permanent_on_battlefield(&mut e, 0, "forest");
    inject_permanent_on_battlefield(&mut e, 1, "island");
    inject_card_into_hand(&mut e, 0, "brasss_bounty");
    let slot = hand_index_for_card(&e, 0, "brasss_bounty");
    semantic::accepted(&mut e, 0, &cast_spell(slot, vec![]));
    inject_permanent_on_battlefield(&mut e, 0, "island");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        battlefield_token_oids(&e, 0, "treasure").len(),
        2,
        "count current own lands"
    );
    assert!(battlefield_token_oids(&e, 1, "treasure").is_empty());
}
