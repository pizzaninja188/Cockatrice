use super::helpers::*;
use tricerules_cards::{ContinuousEffectKind, EffectDuration};
use tricerules_core::{AffectedScope, ContinuousEffect, GameEngine, TurnStep};

#[track_caller]
fn hand_action_reduction(
    engine: &mut GameEngine,
    player: usize,
    card_id: &str,
    stage: &str,
) -> u32 {
    let hand_index = hand_index_for_card(engine, player, card_id) as u32;
    let batch = engine.initial_response_batch();
    let actions = &batch.legal_by_player[&(player as i32)].hand_actions;
    actions
        .iter()
        .find(|action| action.hand_index == hand_index)
        .unwrap_or_else(|| {
            panic!(
                "missing cast action for {card_id} at {stage}; published hand indices: {:?}",
                actions
                    .iter()
                    .map(|action| (action.hand_index, action.card_name.as_str()))
                    .collect::<Vec<_>>()
            )
        })
        .generic_cost_reduction
}

#[test]
fn issue_341_ballyrush_matches_subtype_or_once_per_source_and_casts_at_zero_generic() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &[
                "ballyrush_banneret",
                "ballyrush_banneret",
                "ballyrush_banneret",
                "crossroads_watcher",
                "boros_recruit",
                "zealous_guardian",
                "divination",
            ],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(341_001, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    for card in [
        "crossroads_watcher",
        "boros_recruit",
        "zealous_guardian",
        "divination",
    ] {
        ensure_in_hand(&mut engine, 0, card);
    }
    for _ in 0..3 {
        relocate_to_battlefield(&mut engine, 0, "ballyrush_banneret", false);
    }

    assert_eq!(
        hand_action_reduction(&mut engine, 0, "crossroads_watcher", "Kithkin"),
        3
    );
    assert_eq!(
        hand_action_reduction(&mut engine, 0, "boros_recruit", "Soldier"),
        3
    );
    assert_eq!(
        hand_action_reduction(&mut engine, 0, "zealous_guardian", "Kithkin and Soldier"),
        3,
        "each source's OR filter matches a dual-type spell only once"
    );
    assert_eq!(
        hand_action_reduction(&mut engine, 0, "divination", "unmatched"),
        0
    );

    engine.state.players[0].mana_pool.green = 1;
    let watcher = hand_index_for_card(&engine, 0, "crossroads_watcher");
    engine
        .apply_command(0, &cast_spell(watcher, vec![]))
        .expect("three reductions floor the generic component of Crossroads Watcher to zero");
    assert_eq!(engine.state.players[0].mana_pool.green, 0);
}

#[test]
fn issue_341_dragonlords_servant_matches_dragon_types_and_changeling_in_hand() {
    let decks = Some(vec![
        deck_with(
            "mountain",
            &[
                "dragonlords_servant",
                "adult_gold_dragon",
                "firdoch_core",
                "airbending_lesson",
            ],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(341_002, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    for card in ["adult_gold_dragon", "firdoch_core", "airbending_lesson"] {
        ensure_in_hand(&mut engine, 0, card);
    }
    relocate_to_battlefield(&mut engine, 0, "dragonlords_servant", false);

    assert_eq!(
        hand_action_reduction(&mut engine, 0, "adult_gold_dragon", "printed Dragon"),
        1
    );
    assert_eq!(
        hand_action_reduction(&mut engine, 0, "firdoch_core", "Changeling in hand"),
        1,
        "Changeling supplies every creature subtype while the card is a spell"
    );
    assert_eq!(
        hand_action_reduction(&mut engine, 0, "airbending_lesson", "non-Dragon"),
        0
    );

    engine.state.players[0].mana_pool.red = 1;
    engine.state.players[0].mana_pool.white = 1;
    engine.state.players[0].mana_pool.colorless = 2;
    let dragon = hand_index_for_card(&engine, 0, "adult_gold_dragon");
    engine
        .apply_command(0, &cast_spell(dragon, vec![]))
        .expect("Dragonlord's Servant removes one generic mana and preserves {R}{W}");
    assert_eq!(engine.state.players[0].mana_pool.red, 0);
    assert_eq!(engine.state.players[0].mana_pool.white, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
}

#[test]
fn issue_341_geyser_drake_uses_the_active_players_opponent_set() {
    let decks = Some(vec![
        deck_with("island", &["geyser_drake", "airbending_lesson"]),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(341_003, &[0, 1], 20, decks, true).expect("engine");
    engine.state.turn_step = TurnStep::Main1;
    ensure_in_hand(&mut engine, 0, "airbending_lesson");
    relocate_to_battlefield(&mut engine, 0, "geyser_drake", false);
    inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    engine.state.active_player_idx = 1;
    engine.state.priority_idx = 0;
    assert_eq!(
        hand_action_reduction(&mut engine, 0, "airbending_lesson", "opponent's turn"),
        1,
        "Geyser Drake reduces the controller's instant during the opponent's turn"
    );
    engine.state.active_player_idx = 0;
    engine.state.priority_idx = 0;
    assert_eq!(
        hand_action_reduction(&mut engine, 0, "airbending_lesson", "controller's turn"),
        0
    );
}

#[test]
fn issue_341_voyager_quickwelder_reduces_artifact_spells_only() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &["voyager_quickwelder", "firdoch_core", "crossroads_watcher"],
        ),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(341_004, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "firdoch_core");
    ensure_in_hand(&mut engine, 0, "crossroads_watcher");
    relocate_to_battlefield(&mut engine, 0, "voyager_quickwelder", false);

    assert_eq!(
        hand_action_reduction(&mut engine, 0, "firdoch_core", "Artifact"),
        1
    );
    assert_eq!(
        hand_action_reduction(&mut engine, 0, "crossroads_watcher", "nonartifact"),
        0
    );
    engine.state.players[0].mana_pool.colorless = 2;
    let core = hand_index_for_card(&engine, 0, "firdoch_core");
    engine
        .apply_command(0, &cast_spell(core, vec![]))
        .expect("Voyager Quickwelder reduces Firdoch Core's artifact spell cost to {2}");
}

#[test]
fn issue_341_static_reduction_applies_to_a_flashback_alternative_cost() {
    let decks = Some(vec![
        deck_with("island", &["mocking_sprite", "grizzly_bears"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(341_006, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    relocate_to_battlefield(&mut engine, 0, "mocking_sprite", false);
    let target = relocate_to_battlefield(&mut engine, 0, "grizzly_bears", false);
    let spell = inject_graveyard_card(&mut engine, 0, "cackling_counterpart");
    let generation = engine
        .state
        .zone_change_generation
        .get(&spell)
        .copied()
        .unwrap_or(0);
    let batch = engine.initial_response_batch();
    let action = batch.legal_by_player[&0]
        .zone_cast_actions
        .iter()
        .find(|action| action.object_id == spell)
        .expect("Flashback action for Cackling Counterpart");
    assert_eq!(action.generic_cost_reduction, 1);

    engine.state.players[0].mana_pool.blue = 2;
    engine.state.players[0].mana_pool.colorless = 4;
    engine
        .apply_command(
            0,
            &RuledCommand {
                cmd: Some(Cmd::CastSpell(CastSpell {
                    cast_method: CastMethod::Flashback as i32,
                    source: Some(graveyard_cast_source(spell, generation)),
                    targets: target_object(target),
                    ..Default::default()
                })),
            },
        )
        .expect("one generic reduction applies to Flashback's alternative {5}{U}{U} cost");
    assert_eq!(engine.state.players[0].mana_pool.blue, 0);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 0);
}

#[test]
fn issue_341_uncle_iroh_reduces_lessons_and_firebending_adds_combat_mana() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &["uncle_iroh", "airbending_lesson", "crossroads_watcher"],
        ),
        deck_with("forest", &["grizzly_bears"]),
    ]);
    let mut engine = GameEngine::new(341_005, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "airbending_lesson");
    ensure_in_hand(&mut engine, 0, "crossroads_watcher");
    let iroh = relocate_to_battlefield(&mut engine, 0, "uncle_iroh", false);
    inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");

    assert_eq!(
        hand_action_reduction(&mut engine, 0, "airbending_lesson", "Lesson"),
        1
    );
    assert_eq!(
        hand_action_reduction(&mut engine, 0, "crossroads_watcher", "non-Lesson"),
        0
    );

    engine
        .apply_command(0, &primitive_yield())
        .expect("active player advances from main one to beginning of combat");
    engine
        .apply_command(0, &pass())
        .expect("active player passes combat");
    engine
        .apply_command(1, &pass())
        .expect("nonactive player passes combat");
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
    engine
        .apply_command(0, &declare_attackers(vec![iroh]))
        .expect("Uncle Iroh can attack after entering the battlefield before the combat turn");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "Firebending uses the trigger stack"
    );
    pass_both_players(&mut engine);
    assert_eq!(engine.state.players[0].mana_pool.red, 1);
    assert_eq!(engine.state.players[0].retained_combat_mana.red, 1);
}

#[test]
fn issue_341_reducers_stop_working_when_their_source_loses_abilities() {
    let decks = Some(vec![
        deck_with("plains", &["ballyrush_banneret", "crossroads_watcher"]),
        deck_with("forest", &[]),
    ]);
    let mut engine = GameEngine::new(341_007, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    ensure_in_hand(&mut engine, 0, "crossroads_watcher");
    let reducer = relocate_to_battlefield(&mut engine, 0, "ballyrush_banneret", false);

    assert_eq!(
        hand_action_reduction(&mut engine, 0, "crossroads_watcher", "active Banneret"),
        1
    );

    engine.state.continuous_effects.push(ContinuousEffect {
        trigger_grant_origin: None,
        source_id: None,
        affected: AffectedScope::Single(reducer),
        kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
        condition: None,
        duration: EffectDuration::UntilEndOfTurn,
        timestamp: engine.state.command_index,
    });
    assert_eq!(
        hand_action_reduction(&mut engine, 0, "crossroads_watcher", "abilities removed"),
        0,
        "an ability-removed Banneret cannot reduce Kithkin or Soldier spell costs"
    );

    engine.state.continuous_effects.clear();
    engine.state.objects.get_mut(&reducer).unwrap().face_down = true;
    assert_eq!(
        hand_action_reduction(&mut engine, 0, "crossroads_watcher", "face down"),
        0,
        "a face-down Banneret cannot apply its printed static reduction"
    );
}
