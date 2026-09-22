//! Actual-card scenarios for five pinned Standard entry and token identities.
//! Exact Scryfall Oracle and rulings checked 2026-09-22 (no card-specific rulings).
//! CR 111.10, 603.6a/603.6c, 701.22, 702.9, 702.20, and 702.122 govern these effects.

use super::helpers::*;
use tricerules_cards::{CounterKind, Keyword};
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ruled_command::Cmd, ChooseTriggerTarget, CostObjectRef,
    CostObjectRefs, CostSelection, RuledCommand, TargetRef, TargetRefKind,
};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn cast(e: &mut GameEngine, card_id: &str) {
    inject_card_into_hand(e, 0, card_id);
    grant_pool(e, 0);
    let slot = hand_index_for_card(e, 0, card_id);
    semantic::accepted(e, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(e);
}

fn battlefield_object(e: &GameEngine, card_id: &str) -> u32 {
    *e.state.players[0]
        .battlefield
        .iter()
        .find(|id| e.state.objects[id].card_id == card_id)
        .unwrap_or_else(|| panic!("{card_id} on battlefield"))
}

fn generation(e: &GameEngine, object_id: u32) -> u64 {
    e.state
        .zone_change_generation
        .get(&object_id)
        .copied()
        .unwrap_or(0)
}

fn cost_selection(e: &GameEngine, objects: &[u32]) -> CostSelection {
    CostSelection {
        cost_index: 0,
        selection: Some(Selection::BattlefieldObjects(CostObjectRefs {
            objects: objects
                .iter()
                .map(|id| CostObjectRef {
                    object_id: *id,
                    zone_change_generation: generation(e, *id),
                })
                .collect(),
        })),
    }
}

fn activate_on(
    e: &GameEngine,
    object_id: u32,
    ability_index: u32,
    costs: Vec<CostSelection>,
) -> RuledCommand {
    let mut cmd = activate_ability_with_costs(object_id, ability_index, vec![], costs);
    let Some(Cmd::ActivateAbility(activation)) = cmd.cmd.as_mut() else {
        unreachable!()
    };
    activation.expected_zone_change_generation = generation(e, object_id);
    cmd
}

fn choose_trigger_target(object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
            decline: false,
            selected_modes: vec![],
            targets: vec![TargetRef {
                object_id,
                group_index: 0,
                kind: TargetRefKind::Permanent as i32,
                ..Default::default()
            }],
        })),
    }
}

#[test]
fn issue_misc25_news_helicopter_creates_one_exact_human_citizen() {
    let mut e = engine(825_001);
    cast(&mut e, "news_helicopter");
    let helicopter = battlefield_object(&e, "news_helicopter");
    assert!(e.effective_has_keyword(helicopter, Keyword::Flying));
    let tokens = battlefield_token_oids(&e, 0, "human_citizen_gw_1_1");
    assert_eq!(tokens.len(), 1);
    assert_eq!(e.effective_power(tokens[0]), Some(1));
    assert_eq!(e.effective_toughness(tokens[0]), Some(1));
}

#[test]
fn issue_misc25_turtle_blimp_creates_mutant_and_crews_with_two_power() {
    let mut e = engine(825_002);
    cast(&mut e, "turtle_blimp");
    let blimp = battlefield_object(&e, "turtle_blimp");
    let tokens = battlefield_token_oids(&e, 0, "mutant_r_2_2");
    assert_eq!(tokens.len(), 1);
    assert_eq!(e.effective_power(tokens[0]), Some(2));
    assert!(!e.characteristics(blimp).unwrap().is_creature());
    let weak = inject_creature_with_stats(&mut e, 0, "news_helicopter", 1, 1);
    let too_weak = activate_on(&e, blimp, 0, vec![cost_selection(&e, &[weak])]);
    assert!(
        e.apply_command(0, &too_weak).is_err(),
        "one power cannot crew 2"
    );
    let crew = activate_on(&e, blimp, 0, vec![cost_selection(&e, &[tokens[0]])]);
    semantic::accepted(&mut e, 0, &crew);
    resolve_entire_stack_two_player(&mut e);
    assert!(e.state.objects[&tokens[0]].tapped);
    assert!(e.characteristics(blimp).unwrap().is_creature());
    assert!(e.effective_has_keyword(blimp, Keyword::Flying));
}

#[test]
fn issue_misc25_hopeful_vigil_sacrifice_scries_but_bounce_does_not() {
    let mut e = engine(825_003);
    cast(&mut e, "hopeful_vigil");
    let vigil = battlefield_object(&e, "hopeful_vigil");
    let knights = battlefield_token_oids(&e, 0, "knight_w_2_2_vigilance");
    assert_eq!(knights.len(), 1);
    assert!(e.effective_has_keyword(knights[0], Keyword::Vigilance));
    let sacrifice = activate_on(&e, vigil, 0, vec![]);
    semantic::accepted(&mut e, 0, &sacrifice);
    assert_eq!(e.state.objects[&vigil].zone, Zone::Graveyard);
    for _ in 0..20 {
        if e.state.pending_resolution.is_some() {
            break;
        }
        answer_trigger_order_in_engine_order(&mut e);
        let priority = e.state.priority_player_id();
        semantic::accepted(&mut e, priority, &pass());
    }
    let pending = e.state.pending_resolution.as_ref().expect("scry 2 choice");
    assert_eq!(
        pending.presentation.choice_kind,
        tricerules_proto::ruled::v1::ChoiceKind::LibraryTop
    );
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![]));

    let mut bounced = engine(825_013);
    cast(&mut bounced, "hopeful_vigil");
    let vigil = battlefield_object(&bounced, "hopeful_vigil");
    inject_card_into_hand(&mut bounced, 0, "boomerang");
    let slot = hand_index_for_card(&bounced, 0, "boomerang");
    semantic::accepted(&mut bounced, 0, &cast_spell(slot, target_object(vigil)));
    resolve_entire_stack_two_player(&mut bounced);
    assert_eq!(bounced.state.objects[&vigil].zone, Zone::Hand);
    assert!(
        bounced.state.pending_resolution.is_none(),
        "bounce cannot trigger scry"
    );
}

#[test]
fn issue_misc25_mechan_assembler_excludes_itself_and_caps_artifact_entries() {
    let mut e = engine(825_004);
    cast(&mut e, "mechan_assembler");
    assert!(
        battlefield_token_oids(&e, 0, "robot_c_2_2").is_empty(),
        "own entry excluded"
    );
    cast(&mut e, "bonesplitter");
    assert_eq!(battlefield_token_oids(&e, 0, "robot_c_2_2").len(), 1);
    cast(&mut e, "potioners_trove");
    assert_eq!(
        battlefield_token_oids(&e, 0, "robot_c_2_2").len(),
        1,
        "one trigger per turn, including its own Robot entry"
    );
}

#[test]
fn issue_misc25_mighty_mutanimals_own_token_triggers_alliance_on_chosen_creature() {
    let mut e = engine(825_005);
    let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    cast(&mut e, "mighty_mutanimals");
    let mighty = battlefield_object(&e, "mighty_mutanimals");
    assert_eq!(
        e.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
    assert_eq!(battlefield_token_oids(&e, 0, "mutant_r_2_2").len(), 1);
    assert!(
        e.apply_command(0, &choose_trigger_target(opponent))
            .is_err(),
        "opponent creature is illegal"
    );
    semantic::accepted(&mut e, 0, &choose_trigger_target(target));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&target].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        e.state.objects[&mighty].counter_count(CounterKind::PlusOnePlusOne),
        0
    );
}
