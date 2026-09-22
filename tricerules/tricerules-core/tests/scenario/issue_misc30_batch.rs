//! Actual-card semantics for five pinned Standard token cards.
//! Scryfall Oracle and rulings checked 2026-09-22. CR 111.10, 308, 603.6a, 608.2b/c, 700.14.

use super::helpers::*;
use tricerules_cards::{Color, Keyword};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChoiceKind, ResolutionChoiceDecision, RuledCommand, RuledEventBatch,
    SubmitResolutionChoice,
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

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn resolve_top_stack(e: &mut GameEngine) -> RuledEventBatch {
    let first = e.state.priority_player_id();
    let second = if first == e.state.players[0].id {
        e.state.players[1].id
    } else {
        e.state.players[0].id
    };
    e.apply_command(first, &pass()).expect("first pass");
    e.apply_command(second, &pass())
        .expect("second pass resolves top stack")
}

#[test]
fn issue_misc30_involuntary_control_then_revert_and_fizzle() {
    let mut e = engine(830_001);
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.state.objects.get_mut(&target).unwrap().tapped = true;
    inject_card_into_hand(&mut e, 0, "involuntary_employment");
    let slot = hand_index_for_card(&e, 0, "involuntary_employment");
    semantic::accepted(&mut e, 0, &cast_spell(slot, target_object(target)));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].controller, 0);
    assert!(!e.state.objects[&target].tapped);
    assert!(e.effective_has_keyword(target, Keyword::Haste));
    assert_eq!(battlefield_token_oids(&e, 0, "treasure").len(), 1);
    assert!(battlefield_token_oids(&e, 1, "treasure").is_empty());
    end_active_turn(&mut e, 0);
    assert_eq!(e.state.objects[&target].controller, 1);
    assert!(!e.effective_has_keyword(target, Keyword::Haste));

    let mut fizzle = engine(830_002);
    let target = inject_creature_on_battlefield(&mut fizzle, 1, "grizzly_bears");
    inject_card_into_hand(&mut fizzle, 0, "involuntary_employment");
    let slot = hand_index_for_card(&fizzle, 0, "involuntary_employment");
    semantic::accepted(&mut fizzle, 0, &cast_spell(slot, target_object(target)));
    fizzle.state.players[1]
        .battlefield
        .retain(|id| *id != target);
    fizzle.state.players[1].graveyard.push(target);
    fizzle.state.objects.get_mut(&target).unwrap().zone = Zone::Graveyard;
    *fizzle
        .state
        .zone_change_generation
        .entry(target)
        .or_default() += 1;
    resolve_entire_stack_two_player(&mut fizzle);
    assert!(battlefield_token_oids(&fizzle, 0, "treasure").is_empty());
    assert_eq!(fizzle.state.objects[&target].controller, 1);
}

#[test]
fn issue_misc30_ant_mans_choice_is_exactly_one_token() {
    for (choice, expected_token, other_token) in [(0, "food", "treasure"), (1, "treasure", "food")]
    {
        let mut e = engine(830_010 + choice as u64);
        inject_card_into_hand(&mut e, 0, "ant-mans_army");
        let slot = hand_index_for_card(&e, 0, "ant-mans_army");
        semantic::accepted(&mut e, 0, &cast_spell(slot, vec![]));
        pass_both_players(&mut e); // creature enters, then ETB trigger reaches the stack
        let batch = resolve_top_stack(&mut e); // trigger presents the mandatory token choice
        let choice_event = find_resolution_choice(&batch).expect("published token choice");
        let pending = e.state.pending_resolution.as_ref().expect("entry choice");
        assert_eq!(
            pending.presentation.choice_kind,
            ChoiceKind::ResolutionBranch
        );
        assert_eq!((pending.presentation.min, pending.presentation.max), (1, 1));
        assert_eq!(choice_event.resolution_branches.len(), 2);
        assert_ne!(
            choice_event.resolution_branches[0]
                .presentation
                .as_ref()
                .unwrap()
                .fallback_text,
            choice_event.resolution_branches[1]
                .presentation
                .as_ref()
                .unwrap()
                .fallback_text,
            "Food and Treasure must have distinguishable choice labels"
        );
        for (option, id) in choice_event
            .resolution_branches
            .iter()
            .zip(["create_food", "create_treasure"])
        {
            let presentation = option.presentation.as_ref().expect("stable token choice");
            assert_eq!(presentation.path.last().unwrap().id, id);
            assert_ne!(presentation.fallback_text, "Choice");
        }
        assert!(e.apply_command(0, &select_branch(2)).is_err());
        assert!(e.apply_command(1, &select_branch(choice)).is_err());
        semantic::accepted(&mut e, 0, &select_branch(choice));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(battlefield_token_oids(&e, 0, expected_token).len(), 1);
        assert!(battlefield_token_oids(&e, 0, other_token).is_empty());
        assert!(battlefield_token_oids(&e, 1, expected_token).is_empty());
    }
}

#[test]
fn issue_misc30_confectioner_observes_nontoken_food_sacrifice() {
    let mut e = engine(830_020);
    cast_permanent(&mut e, "experimental_confectioner");
    assert_eq!(battlefield_token_oids(&e, 0, "food").len(), 1);
    let food_card = inject_creature_on_battlefield(&mut e, 0, "tough_cookie");
    inject_card_into_hand(&mut e, 0, "village_rites");
    grant_pool(&mut e, 0);
    let slot = hand_index_for_card(&e, 0, "village_rites");
    semantic::accepted(
        &mut e,
        0,
        &cast_spell_with_costs(slot, vec![], vec![permanent_cost_selection(0, food_card)]),
    );
    assert_eq!(e.state.objects[&food_card].zone, Zone::Graveyard);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        battlefield_token_oids(&e, 0, "rat_b_1_1_cant_block").len(),
        1
    );

    let food_token = battlefield_token_oids(&e, 0, "food")[0];
    let life_before = e.state.players[0].life;
    grant_pool(&mut e, 0);
    let mut use_food = activate_ability(food_token, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = use_food.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation = e.state.zone_change_generation[&food_token];
    semantic::accepted(&mut e, 0, &use_food);
    assert!(
        !e.state.players[0].battlefield.contains(&food_token),
        "Food pays its own sacrifice cost"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.players[0].life, life_before + 3);
    assert_eq!(
        battlefield_token_oids(&e, 0, "rat_b_1_1_cant_block").len(),
        2
    );

    let nonfood = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    inject_card_into_hand(&mut e, 0, "village_rites");
    grant_pool(&mut e, 0);
    let slot = hand_index_for_card(&e, 0, "village_rites");
    semantic::accepted(
        &mut e,
        0,
        &cast_spell_with_costs(slot, vec![], vec![permanent_cost_selection(0, nonfood)]),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        battlefield_token_oids(&e, 0, "rat_b_1_1_cant_block").len(),
        2
    );
}

#[test]
fn issue_misc30_bakersbane_expend_once_on_crossing_four() {
    let mut e = engine(830_030);
    let duo = cast_permanent(&mut e, "bakersbane_duo");
    assert_eq!(battlefield_token_oids(&e, 0, "food").len(), 1);
    assert_eq!(
        (e.effective_power(duo), e.effective_toughness(duo)),
        (Some(2), Some(2))
    );
    for expected in [3, 3] {
        inject_card_into_hand(&mut e, 0, "grizzly_bears");
        grant_pool(&mut e, 0);
        let slot = hand_index_for_card(&e, 0, "grizzly_bears");
        semantic::accepted(&mut e, 0, &cast_spell(slot, vec![]));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.effective_power(duo),
            Some(expected),
            "expend 4 crosses only once"
        );
    }
    end_active_turn(&mut e, 0);
    assert_eq!(
        (e.effective_power(duo), e.effective_toughness(duo)),
        (Some(2), Some(2))
    );

    // The expenditure belongs to the turn, not to the source: an earlier crossing
    // cannot be observed retroactively by a Duo that enters afterward.
    let mut late = engine(830_031);
    for _ in 0..2 {
        inject_card_into_hand(&mut late, 0, "grizzly_bears");
        grant_pool(&mut late, 0);
        let slot = hand_index_for_card(&late, 0, "grizzly_bears");
        semantic::accepted(&mut late, 0, &cast_spell(slot, vec![]));
        resolve_entire_stack_two_player(&mut late);
    }
    let late_duo = cast_permanent(&mut late, "bakersbane_duo");
    inject_card_into_hand(&mut late, 0, "grizzly_bears");
    grant_pool(&mut late, 0);
    let slot = hand_index_for_card(&late, 0, "grizzly_bears");
    semantic::accepted(&mut late, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut late);
    assert_eq!(late.effective_power(late_duo), Some(2));
}

#[test]
fn issue_misc30_clachan_kindred_and_token_counts() {
    let mut e = engine(830_040);
    let source = cast_permanent(&mut e, "clachan_festival");
    let tokens = battlefield_token_oids(&e, 0, "kithkin_gw_1_1");
    assert_eq!(tokens.len(), 2);
    let ch = e.characteristics(tokens[0]).expect("Kithkin token");
    assert_eq!((ch.power, ch.toughness), (Some(1), Some(1)));
    assert_eq!(ch.colors, vec![Color::White, Color::Green]);
    assert!(battlefield_token_oids(&e, 1, "kithkin_gw_1_1").is_empty());
    grant_pool(&mut e, 0);
    let mut command = activate_ability(source, 0, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation = e.state.zone_change_generation[&source];
    semantic::accepted(&mut e, 0, &command);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(battlefield_token_oids(&e, 0, "kithkin_gw_1_1").len(), 3);
}
