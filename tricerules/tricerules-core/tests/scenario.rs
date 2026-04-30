//! Scripted command sequences (M2).

use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::ruled_event::Ev;
use tricerules_proto::ruled::v1::{
    BlockPair, CastSpell, DeclareAttackers, DeclareBlockers, PassPriority, PlayLand,
    PrimitiveYieldStructured, RuledCommand, TargetRef,
};

use tricerules_core::GameEngine;

fn pass() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::PassPriority(PassPriority {})),
    }
}

fn primitive_yield() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::PrimitiveYieldStructured(PrimitiveYieldStructured {})),
    }
}

fn play_land(hand_card_index: usize) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::PlayLand(PlayLand {
            hand_card_index: hand_card_index as u32,
        })),
    }
}

fn cast_spell(hand_card_index: usize, targets: Vec<TargetRef>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            hand_card_index: hand_card_index as u32,
            targets,
        })),
    }
}

fn declare_attackers(creature_ids: Vec<u32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DeclareAttackers(DeclareAttackers { creature_ids })),
    }
}

fn declare_blockers(block_pairs: Vec<BlockPair>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DeclareBlockers(DeclareBlockers { block_pairs })),
    }
}

fn hand_index_for_card(e: &GameEngine, player: usize, card_id: &str) -> usize {
    e.state.players[player]
        .hand
        .iter()
        .enumerate()
        .find_map(|(i, oid)| {
            e.state
                .objects
                .get(oid)
                .filter(|o| o.card_id == card_id)
                .map(|_| i)
        })
        .unwrap_or_else(|| panic!("missing card {card_id} in hand"))
}

fn take_card_from_library_to_hand(e: &mut GameEngine, player: usize, card_id: &str) {
    let pos = e.state.players[player]
        .library
        .iter()
        .position(|oid| {
            e.state
                .objects
                .get(oid)
                .map(|o| o.card_id.as_str())
                == Some(card_id)
        })
        .unwrap_or_else(|| panic!("missing card {card_id} in P{player} library"));
    let oid = e.state.players[player]
        .library
        .remove(pos)
        .expect("index from position()");
    e.state.players[player].hand.push(oid);
    e.state
        .objects
        .get_mut(&oid)
        .expect("object")
        .zone = tricerules_core::Zone::Hand;
}

fn battlefield_object_for_card(e: &GameEngine, player: usize, card_id: &str) -> u32 {
    e.state.players[player]
        .battlefield
        .iter()
        .copied()
        .find(|oid| {
            e.state
                .objects
                .get(oid)
                .map(|o| o.card_id == card_id)
                .unwrap_or(false)
        })
        .unwrap_or_else(|| panic!("missing card {card_id} on battlefield"))
}

fn end_active_turn(e: &mut GameEngine, player: i32) {
    e.apply_command(player, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(player, &primitive_yield())
        .expect("begin combat to declare attackers");
    e.apply_command(player, &primitive_yield())
        .expect("skip attackers to end combat");
    e.apply_command(player, &primitive_yield())
        .expect("end combat to main2");
    e.apply_command(player, &primitive_yield())
        .expect("main2 to end step");
    e.apply_command(player, &primitive_yield())
        .expect("end step to next turn upkeep");
}

fn priority_changes_in(batch: &tricerules_proto::ruled::v1::RuledEventBatch) -> Vec<i32> {
    batch
        .events
        .iter()
        .filter_map(|ev| match &ev.ev {
            Some(Ev::PriorityChanged(pc)) => Some(pc.player_id),
            _ => None,
        })
        .collect()
}

fn pass_both_players(e: &mut GameEngine) {
    let first = e.state.priority_player_id();
    let second = if first == e.state.players[0].id {
        e.state.players[1].id
    } else {
        e.state.players[0].id
    };
    e.apply_command(first, &pass()).expect("first player pass");
    e.apply_command(second, &pass())
        .expect("second player pass");
}

fn advance_to_main1_from_game_start(e: &mut GameEngine) {
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Upkeep);
    pass_both_players(e); // upkeep -> draw
    pass_both_players(e); // draw -> main1
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Main1);
}

#[test]
fn primitive_yield_active_skips_double_pass_main1() {
    let mut e = GameEngine::new(99, &[0, 1], 20, None).expect("new");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Upkeep);
    e.apply_command(0, &primitive_yield())
        .expect("active primitive");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Draw);
}

#[test]
fn two_player_passes_empty_stack_advances_toward_combat() {
    let mut e = GameEngine::new(99, &[0, 1], 20, None).expect("new");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Upkeep);
    e.apply_command(0, &pass()).expect("p0");
    e.apply_command(1, &pass()).expect("p1");
    // After two passes, should leave upkeep to draw.
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Draw);
}

#[test]
fn empty_stack_double_pass_emits_ap_priority_in_new_phase() {
    let mut e = GameEngine::new(99, &[0, 1], 20, None).expect("new");
    e.apply_command(0, &pass()).expect("p0 pass");
    let b = e.apply_command(1, &pass()).expect("p1 pass");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Draw);
    assert!(
        priority_changes_in(&b).contains(&0),
        "after phase advance, active player should explicitly regain priority"
    );
}

#[test]
fn mana_pools_empty_on_step_change() {
    let mut e = GameEngine::new(99, &[0, 1], 20, None).expect("new");
    e.state.players[0].mana_pool.red = 2;
    e.state.players[1].mana_pool.green = 1;

    e.apply_command(0, &primitive_yield())
        .expect("active primitive");

    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Draw);
    assert_eq!(e.state.players[0].mana_pool.red, 0);
    assert_eq!(e.state.players[0].mana_pool.green, 0);
    assert_eq!(e.state.players[0].mana_pool.blue, 0);
    assert_eq!(e.state.players[0].mana_pool.colorless, 0);
    assert_eq!(e.state.players[1].mana_pool.red, 0);
    assert_eq!(e.state.players[1].mana_pool.green, 0);
    assert_eq!(e.state.players[1].mana_pool.blue, 0);
    assert_eq!(e.state.players[1].mana_pool.colorless, 0);
}

#[test]
fn new_with_custom_deck_length() {
    let decks = Some(vec![vec!["mountain".into(); 30], vec!["forest".into(); 30]]);
    let e = GameEngine::new(1, &[0, 1], 20, decks).expect("new");
    assert_eq!(
        e.state.players[0].library.len() + e.state.players[0].hand.len(),
        30
    );
}

#[test]
fn play_land_moves_card_from_hand_to_battlefield() {
    let decks = Some(vec![vec!["mountain".into(); 7], vec!["forest".into(); 7]]);
    let mut e = GameEngine::new(7, &[0, 1], 20, decks).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let hand_before = e.state.players[0].hand.len();
    let battlefield_before = e.state.players[0].battlefield.len();

    e.apply_command(0, &play_land(0)).expect("play land");

    assert_eq!(e.state.players[0].hand.len(), hand_before - 1);
    assert_eq!(e.state.players[0].battlefield.len(), battlefield_before + 1);
    let mountain = battlefield_object_for_card(&e, 0, "mountain");
    assert_eq!(
        e.state.objects.get(&mountain).expect("mountain").card_id,
        "mountain"
    );
}

#[test]
fn cast_lightning_bolt_resolves_to_graveyard_after_double_pass() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(13, &[0, 1], 20, decks).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");

    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    let pushed = e
        .apply_command(0, &cast_spell(bolt_idx, vec![]))
        .expect("cast bolt");
    let bolt_oid = e.state.stack.last().expect("spell on stack").id;
    assert!(pushed
        .events
        .iter()
        .any(|ev| matches!(ev.ev, Some(Ev::StackPushed(_)))));

    e.apply_command(0, &pass()).expect("caster pass");
    let resolved = e.apply_command(1, &pass()).expect("opponent pass");
    assert!(e.state.stack.is_empty());
    assert!(e.state.players[0].graveyard.contains(&bolt_oid));
    assert!(resolved.events.iter().any(|ev| {
        matches!(
            ev.ev,
            Some(Ev::StackResolved(ref r))
                if r.object_id == bolt_oid
                    && r.destination
                        == tricerules_proto::ruled::v1::StackResolveDestination::Graveyard as i32
        )
    }));
}

#[test]
fn casting_spell_keeps_priority_with_caster() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(13, &[0, 1], 20, decks).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    let pushed = e
        .apply_command(0, &cast_spell(bolt_idx, vec![]))
        .expect("cast bolt");
    assert!(
        priority_changes_in(&pushed).contains(&0),
        "caster should keep priority after casting"
    );
}

#[test]
fn stack_resolution_emits_priority_to_active_player() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(13, &[0, 1], 20, decks).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, vec![]))
        .expect("cast bolt");
    e.apply_command(0, &pass()).expect("caster pass");
    let resolved = e.apply_command(1, &pass()).expect("opponent pass");
    assert!(
        priority_changes_in(&resolved).contains(&0),
        "active player should regain priority after stack resolves"
    );
}

#[test]
fn declare_attackers_handoff_emits_defender_priority() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "forest".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec![
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
    ]);
    let mut e = GameEngine::new(66, &[0, 1], 20, decks).expect("new");
    advance_to_main1_from_game_start(&mut e);
    // Put one creature and two forests on battlefield to mimic later turn.
    for card in ["forest", "forest", "grizzly_bears"] {
        let idx = hand_index_for_card(&e, 0, card);
        let oid = e.state.players[0].hand.remove(idx);
        e.state.players[0].battlefield.push(oid);
        if let Some(obj) = e.state.objects.get_mut(&oid) {
            obj.zone = tricerules_core::Zone::Battlefield;
            obj.summoning_sick = false;
            obj.tapped = false;
        }
    }

    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap pass begin combat");
    e.apply_command(1, &pass()).expect("nap pass begin combat");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );

    let bears_oid = battlefield_object_for_card(&e, 0, "grizzly_bears");
    let b = e
        .apply_command(0, &declare_attackers(vec![bears_oid]))
        .expect("declare attackers");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
    assert!(
        priority_changes_in(&b).contains(&0),
        "after declaring attackers, active player keeps priority in declare attackers"
    );
    let to_defender = e
        .apply_command(0, &pass())
        .expect("active pass declare attackers");
    assert!(
        priority_changes_in(&to_defender).contains(&1),
        "defender should receive priority in declare attackers"
    );
    let to_blockers = e
        .apply_command(1, &pass())
        .expect("defender pass declare attackers");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers
    );
    assert!(
        priority_changes_in(&to_blockers).contains(&1),
        "on entering declare blockers, defender has priority"
    );
}

#[test]
fn no_attackers_skip_to_end_combat_emits_active_priority() {
    let mut e = GameEngine::new(67, &[0, 1], 20, None).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap pass begin combat");
    e.apply_command(1, &pass()).expect("nap pass begin combat");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );

    let b = e
        .apply_command(0, &declare_attackers(vec![]))
        .expect("no attackers");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::EndCombat);
    assert!(
        priority_changes_in(&b).contains(&0),
        "active player should keep/regain priority in end combat after no attackers"
    );

    // End combat still has a full priority pass cycle before postcombat main.
    let to_nap = e.apply_command(0, &pass()).expect("ap pass end combat");
    assert!(
        priority_changes_in(&to_nap).contains(&1),
        "non-active player should receive priority in end combat"
    );
    e.apply_command(1, &pass()).expect("nap pass end combat");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Main2);
}

#[test]
fn blockers_to_combat_damage_emits_priority_stop() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "forest".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec![
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
    ]);
    let mut e = GameEngine::new(68, &[0, 1], 20, decks).expect("new");
    advance_to_main1_from_game_start(&mut e);
    for card in ["forest", "grizzly_bears"] {
        let idx = hand_index_for_card(&e, 0, card);
        let oid = e.state.players[0].hand.remove(idx);
        e.state.players[0].battlefield.push(oid);
        if let Some(obj) = e.state.objects.get_mut(&oid) {
            obj.zone = tricerules_core::Zone::Battlefield;
            obj.summoning_sick = false;
        }
    }
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap pass begin combat");
    e.apply_command(1, &pass()).expect("nap pass begin combat");
    let bears_oid = battlefield_object_for_card(&e, 0, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![bears_oid]))
        .expect("declare attackers");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers
    );
    let declare_blockers_batch = e.apply_command(1, &pass()).expect("defender no blocks");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers,
        "declaring no blockers should not immediately deal combat damage"
    );
    assert!(
        priority_changes_in(&declare_blockers_batch).contains(&0),
        "after blockers are declared, active player gets priority in declare blockers"
    );
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    let b = e
        .apply_command(1, &pass())
        .expect("defender pass declare blockers -> combat damage");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::CombatDamage);
    assert!(
        priority_changes_in(&b).contains(&0),
        "combat damage should open a priority window for active player"
    );
}

#[test]
fn main2_double_pass_advances_to_end_step_stop() {
    let mut e = GameEngine::new(69, &[0, 1], 20, None).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &primitive_yield())
        .expect("begin combat to declare attackers");
    e.apply_command(0, &primitive_yield())
        .expect("skip attackers to end combat");
    e.apply_command(0, &primitive_yield())
        .expect("end combat to main2");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Main2);
    e.apply_command(0, &pass()).expect("ap pass main2");
    let b = e.apply_command(1, &pass()).expect("nap pass main2");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::EndStep);
    assert!(
        priority_changes_in(&b).contains(&0),
        "end step should open a priority window for active player"
    );
}

#[test]
fn new_turn_stops_at_upkeep_then_draw_then_main1() {
    let mut e = GameEngine::new(70, &[0, 1], 20, None).expect("new");
    advance_to_main1_from_game_start(&mut e);
    end_active_turn(&mut e, 0);
    assert_eq!(e.state.active_player_id(), 1);
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Upkeep);
    e.apply_command(1, &pass()).expect("ap pass upkeep");
    let to_draw = e.apply_command(0, &pass()).expect("nap pass upkeep");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Draw);
    assert!(
        priority_changes_in(&to_draw).contains(&1),
        "draw step should open priority for the active player"
    );
    e.apply_command(1, &pass()).expect("ap pass draw");
    let to_main = e.apply_command(0, &pass()).expect("nap pass draw");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Main1);
    assert!(
        priority_changes_in(&to_main).contains(&1),
        "main1 should open priority for the active player"
    );
}

#[test]
fn cast_grizzly_bears_resolves_to_battlefield_and_taps_two_forests() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "forest".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec![
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
    ]);
    let mut e = GameEngine::new(22, &[0, 1], 20, decks).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Simulate one untapped Forest that was played on a previous turn.
    let seeded_forest_idx = hand_index_for_card(&e, 0, "forest");
    let seeded_forest_oid = e.state.players[0].hand.remove(seeded_forest_idx);
    e.state.players[0].battlefield.push(seeded_forest_oid);
    e.state
        .objects
        .get_mut(&seeded_forest_oid)
        .expect("seeded forest")
        .zone = tricerules_core::Zone::Battlefield;

    // Play the second Forest this turn.
    let forest_to_play_idx = hand_index_for_card(&e, 0, "forest");
    e.apply_command(0, &play_land(forest_to_play_idx))
        .expect("play second forest");

    let bears_idx = hand_index_for_card(&e, 0, "grizzly_bears");
    e.apply_command(0, &cast_spell(bears_idx, vec![]))
        .expect("cast bears");
    let bears_oid = e.state.stack.first().expect("bears stack item").id;

    let untapped_before_resolve = e.state.players[0]
        .battlefield
        .iter()
        .filter(|oid| e.state.objects.get(oid).map(|o| !o.tapped).unwrap_or(false))
        .count();
    assert_eq!(untapped_before_resolve, 0, "both forests are tapped for 1G");

    e.apply_command(0, &pass()).expect("p0 pass");
    let resolved = e.apply_command(1, &pass()).expect("p1 pass");

    assert!(e.state.players[0].battlefield.contains(&bears_oid));
    assert!(resolved.events.iter().any(|ev| {
        matches!(
            ev.ev,
            Some(Ev::StackResolved(ref r))
                if r.object_id == bears_oid
                    && r.destination
                        == tricerules_proto::ruled::v1::StackResolveDestination::Battlefield as i32
        )
    }));
}

#[test]
fn caster_can_cast_second_spell_before_passing_priority() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "mountain".into(),
            "lightning_bolt".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(333, &[0, 1], 20, decks).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let mountain_a = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_a))
        .expect("play first mountain");
    // Seed a second untapped mountain to allow casting another bolt while holding priority.
    let mountain_b = hand_index_for_card(&e, 0, "mountain");
    let mountain_b_oid = e.state.players[0].hand.remove(mountain_b);
    e.state.players[0].battlefield.push(mountain_b_oid);
    e.state
        .objects
        .get_mut(&mountain_b_oid)
        .expect("second mountain")
        .zone = tricerules_core::Zone::Battlefield;

    let bolt_one = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_one, vec![]))
        .expect("cast first bolt");
    assert_eq!(
        e.state.priority_player_id(),
        0,
        "caster should keep priority after casting first spell"
    );

    let bolt_two = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_two, vec![]))
        .expect("cast second bolt while holding priority");
    assert_eq!(
        e.state.stack.len(),
        2,
        "both spells should be on the stack before any opponent pass"
    );
}

#[test]
fn non_active_player_with_priority_pays_mana_for_counterspell() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "island".into(),
            "counterspell".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
    ]);
    let mut e = GameEngine::new(144, &[0, 1], 20, decks).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("p0 play mountain");

    for _ in 0..2 {
        let island_idx = hand_index_for_card(&e, 1, "island");
        let island_oid = e.state.players[1].hand.remove(island_idx);
        e.state.players[1].battlefield.push(island_oid);
        e.state
            .objects
            .get_mut(&island_oid)
            .expect("seeded island")
            .zone = tricerules_core::Zone::Battlefield;
    }

    let p1_island_a = battlefield_object_for_card(&e, 1, "island");
    assert!(
        !e.state
            .objects
            .get(&p1_island_a)
            .expect("p1 island")
            .tapped
    );

    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, vec![]))
        .expect("p0 cast bolt");
    let bolt_oid = e.state.stack.last().expect("bolt on stack").id;
    e.apply_command(0, &pass())
        .expect("p0 pass to give p1 priority");

    let counter_idx = hand_index_for_card(&e, 1, "counterspell");
    e.apply_command(
        1,
        &cast_spell(
            counter_idx,
            vec![TargetRef {
                object_id: bolt_oid,
            }],
        ),
    )
    .expect("NAP with priority should tap lands and cast counterspell");
    assert!(
        e.state
            .objects
            .get(&p1_island_a)
            .expect("p1 island")
            .tapped,
        "an island should tap to help pay UU"
    );
    assert_eq!(
        e.state.stack.len(),
        2,
        "bolt and counterspell on stack"
    );
}

#[test]
fn untap_and_draw_happen_in_new_turn_sequence() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(88, &[0, 1], 20, decks).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let hand_before_turn = e.state.players[0].hand.len();
    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");

    let mountain_oid = battlefield_object_for_card(&e, 0, "mountain");
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, vec![]))
        .expect("cast lightning bolt");
    e.apply_command(0, &pass()).expect("caster pass");
    e.apply_command(1, &pass())
        .expect("opponent pass to resolve");

    assert!(
        e.state
            .objects
            .get(&mountain_oid)
            .expect("mountain object")
            .tapped,
        "mountain is tapped after paying for bolt"
    );

    end_active_turn(&mut e, 0); // now active player 1, upkeep
    pass_both_players(&mut e); // upkeep -> draw
    pass_both_players(&mut e); // draw -> main1
    e.apply_command(1, &primitive_yield())
        .expect("p1 main1 to begin combat");
    pass_both_players(&mut e); // begin combat -> declare attackers
    e.apply_command(1, &declare_attackers(vec![]))
        .expect("p1 no attackers to end combat");
    pass_both_players(&mut e); // end combat -> main2
    pass_both_players(&mut e); // main2 -> end step
    pass_both_players(&mut e); // end step -> p0 upkeep
    pass_both_players(&mut e); // upkeep -> draw
    pass_both_players(&mut e); // draw -> main1

    assert_eq!(e.state.active_player_id(), 0);
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Main1);
    assert!(
        !e.state
            .objects
            .get(&mountain_oid)
            .expect("mountain object")
            .tapped,
        "mountain untaps during the active player's untap phase"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before_turn - 1,
        "player drew one card during draw phase after spending two cards"
    );
}

#[test]
fn duplicate_attacker_ids_are_rejected() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "forest".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec![
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
    ]);
    let mut e = GameEngine::new(101, &[0, 1], 20, decks).expect("new");
    advance_to_main1_from_game_start(&mut e);
    for card in ["forest", "grizzly_bears"] {
        let idx = hand_index_for_card(&e, 0, card);
        let oid = e.state.players[0].hand.remove(idx);
        e.state.players[0].battlefield.push(oid);
        if let Some(obj) = e.state.objects.get_mut(&oid) {
            obj.zone = tricerules_core::Zone::Battlefield;
            obj.summoning_sick = false;
            obj.tapped = false;
        }
    }
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap pass begin combat");
    e.apply_command(1, &pass()).expect("nap pass begin combat");
    let bears_oid = battlefield_object_for_card(&e, 0, "grizzly_bears");

    let err = e
        .apply_command(0, &declare_attackers(vec![bears_oid, bears_oid]))
        .expect_err("duplicate attackers should fail");
    assert_eq!(err.to_string(), "illegal command: duplicate attacker");
}

#[test]
fn same_blocker_cannot_block_two_attackers() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "forest".into(),
            "grizzly_bears".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec![
            "forest".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(202, &[0, 1], 20, decks).expect("new");
    advance_to_main1_from_game_start(&mut e);

    for card in ["forest", "forest", "grizzly_bears", "grizzly_bears"] {
        let idx = hand_index_for_card(&e, 0, card);
        let oid = e.state.players[0].hand.remove(idx);
        e.state.players[0].battlefield.push(oid);
        if let Some(obj) = e.state.objects.get_mut(&oid) {
            obj.zone = tricerules_core::Zone::Battlefield;
            obj.summoning_sick = false;
            obj.tapped = false;
        }
    }
    for card in ["forest", "grizzly_bears"] {
        let idx = hand_index_for_card(&e, 1, card);
        let oid = e.state.players[1].hand.remove(idx);
        e.state.players[1].battlefield.push(oid);
        if let Some(obj) = e.state.objects.get_mut(&oid) {
            obj.zone = tricerules_core::Zone::Battlefield;
            obj.summoning_sick = false;
            obj.tapped = false;
        }
    }
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap pass begin combat");
    e.apply_command(1, &pass()).expect("nap pass begin combat");

    let attacker_a = battlefield_object_for_card(&e, 0, "grizzly_bears");
    let attacker_b = e.state.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|oid| {
            *oid != attacker_a
                && e.state
                    .objects
                    .get(oid)
                    .map(|o| o.card_id == "grizzly_bears")
                    .unwrap_or(false)
        })
        .expect("second attacker");
    e.apply_command(0, &declare_attackers(vec![attacker_a, attacker_b]))
        .expect("declare two attackers");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    let blocker = battlefield_object_for_card(&e, 1, "grizzly_bears");

    let err = e
        .apply_command(
            1,
            &declare_blockers(vec![
                BlockPair {
                    attacker_id: attacker_a,
                    blocker_id: blocker,
                },
                BlockPair {
                    attacker_id: attacker_b,
                    blocker_id: blocker,
                },
            ]),
        )
        .expect_err("same blocker twice should fail");
    assert_eq!(
        err.to_string(),
        "illegal command: blocker assigned more than once"
    );
}

fn put_creature_on_battlefield(e: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let idx = hand_index_for_card(e, player, card_id);
    let oid = e.state.players[player].hand.remove(idx);
    e.state.players[player].battlefield.push(oid);
    if let Some(obj) = e.state.objects.get_mut(&oid) {
        obj.zone = tricerules_core::Zone::Battlefield;
        obj.summoning_sick = false;
        obj.tapped = false;
    }
    oid
}

fn advance_to_declare_attackers(e: &mut GameEngine) {
    advance_to_main1_from_game_start(e);
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap pass begin combat");
    e.apply_command(1, &pass()).expect("nap pass begin combat");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
}

fn life_changes_in(
    batch: &tricerules_proto::ruled::v1::RuledEventBatch,
) -> Vec<tricerules_proto::ruled::v1::LifeChanged> {
    batch
        .events
        .iter()
        .filter_map(|ev| match &ev.ev {
            Some(Ev::LifeChanged(lc)) => Some(lc.clone()),
            _ => None,
        })
        .collect()
}

fn permanents_moved_in(
    batch: &tricerules_proto::ruled::v1::RuledEventBatch,
) -> Vec<tricerules_proto::ruled::v1::PermanentMoved> {
    batch
        .events
        .iter()
        .filter_map(|ev| match &ev.ev {
            Some(Ev::PermanentMoved(pm)) => Some(pm.clone()),
            _ => None,
        })
        .collect()
}

fn attackers_declared_in(
    batch: &tricerules_proto::ruled::v1::RuledEventBatch,
) -> Vec<tricerules_proto::ruled::v1::AttackersDeclared> {
    batch
        .events
        .iter()
        .filter_map(|ev| match &ev.ev {
            Some(Ev::AttackersDeclared(ad)) => Some(ad.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn zone_view_includes_battlefield_object_ids() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec![
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
    ]);
    let mut e = GameEngine::new(404, &[0, 1], 20, decks).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bears = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    // ZoneViewSync is emitted as part of every batch via apply_command's tail.
    let b = e.apply_command(0, &pass()).expect("ap pass main1");
    let zone_view = b
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::ZoneView(zv)) => Some(zv.clone()),
            _ => None,
        })
        .expect("zone view in batch");
    let p0 = zone_view
        .per_player
        .iter()
        .find(|p| p.player_id == 0)
        .expect("p0 view");
    assert_eq!(p0.battlefield_object_id.len(), p0.battlefield.len());
    assert_eq!(p0.battlefield_power.len(), p0.battlefield.len());
    assert_eq!(p0.battlefield_toughness.len(), p0.battlefield.len());
    assert_eq!(p0.battlefield_damage.len(), p0.battlefield.len());
    assert_eq!(p0.battlefield_is_creature.len(), p0.battlefield.len());
    let pos = p0
        .battlefield
        .iter()
        .position(|c| c == "grizzly_bears")
        .expect("bears in view");
    assert_eq!(p0.battlefield_object_id[pos], bears);
    assert!(p0.battlefield_is_creature[pos]);
    assert_eq!(p0.battlefield_power[pos], 2);
    assert_eq!(p0.battlefield_toughness[pos], 2);
    assert_eq!(p0.battlefield_damage[pos], 0);
}

#[test]
fn declare_attackers_emits_attackers_declared_event() {
    let mut e = GameEngine::new(505, &[0, 1], 20, None).expect("new");
    advance_to_declare_attackers(&mut e);
    let bears = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let b = e
        .apply_command(0, &declare_attackers(vec![bears]))
        .expect("declare attackers");
    let evs = attackers_declared_in(&b);
    assert_eq!(evs.len(), 1, "exactly one AttackersDeclared event");
    assert_eq!(evs[0].attacking_player_id, 0);
    assert_eq!(evs[0].attacker_object_ids, vec![bears]);
}

#[test]
fn unblocked_combat_damage_emits_life_changed() {
    let mut e = GameEngine::new(606, &[0, 1], 20, None).expect("new");
    advance_to_declare_attackers(&mut e);
    let bears_a = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let bears_b = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![bears_a, bears_b]))
        .expect("two attackers");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    let declared = e.apply_command(1, &pass()).expect("defender no blocks");
    assert!(
        life_changes_in(&declared).is_empty(),
        "no damage during declare blockers immediately after no-block declaration"
    );
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    let b = e
        .apply_command(1, &pass())
        .expect("defender pass declare blockers -> combat damage");
    let life = life_changes_in(&b);
    assert_eq!(life.len(), 1, "single LifeChanged event for defender");
    assert_eq!(life[0].player_id, 1);
    assert_eq!(life[0].delta, -4, "two 2/2s deal 4 damage");
    assert_eq!(life[0].new_total, 16);
    assert_eq!(e.state.players[1].life, 16);
}

#[test]
fn blocked_combat_kills_blocker_and_emits_permanent_moved() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec![
            "forest".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(707, &[0, 1], 20, decks).expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    // Defender needs a creature on the battlefield to block. Put a 2/2 too -> mutual destruction.
    let blocker = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    let declared = e
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker,
                blocker_id: blocker,
            }]),
        )
        .expect("declare blocker");
    assert!(
        permanents_moved_in(&declared).is_empty(),
        "creatures should not die until combat damage step"
    );
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    let b = e
        .apply_command(1, &pass())
        .expect("defender pass declare blockers -> combat damage");
    let dead = permanents_moved_in(&b);
    let dead_ids: Vec<u32> = dead.iter().map(|p| p.object_id).collect();
    assert!(
        dead_ids.contains(&attacker) && dead_ids.contains(&blocker),
        "both 2/2s die in mutual block, got {dead_ids:?}"
    );
    for pm in &dead {
        assert_eq!(
            pm.destination,
            tricerules_proto::ruled::v1::permanent_moved::Destination::Graveyard as i32
        );
    }
    // No life loss on a mutual block.
    let life = life_changes_in(&b);
    assert!(life.is_empty(), "no life change on a fully blocked combat");
}

#[test]
fn full_combat_2v1_trade_and_life_loss() {
    // Active player has two 2/2 attackers; defender has one 2/2 blocker.
    // Active player attacks with both. Defender blocks attacker_a only.
    // Outcome: attacker_a + blocker trade (both move to graveyard); attacker_b
    // hits the defender for 2 unblocked damage.
    let decks = Some(vec![
        vec![
            "forest".into(),
            "grizzly_bears".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec![
            "forest".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(808, &[0, 1], 20, decks).expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker_a = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let attacker_b = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let blocker = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    // Snapshot pre-combat state we care about.
    let attacker_a_pre_tapped = e
        .state
        .objects
        .get(&attacker_a)
        .map(|o| o.tapped)
        .unwrap_or(true);
    let attacker_b_pre_tapped = e
        .state
        .objects
        .get(&attacker_b)
        .map(|o| o.tapped)
        .unwrap_or(true);
    assert!(
        !attacker_a_pre_tapped,
        "attacker_a should be untapped pre-combat"
    );
    assert!(
        !attacker_b_pre_tapped,
        "attacker_b should be untapped pre-combat"
    );

    // Declare attackers.
    let attack_batch = e
        .apply_command(0, &declare_attackers(vec![attacker_a, attacker_b]))
        .expect("declare two attackers");
    let ad = attackers_declared_in(&attack_batch);
    assert_eq!(ad.len(), 1);
    assert_eq!(ad[0].attacking_player_id, 0);
    let mut declared_ids = ad[0].attacker_object_ids.clone();
    declared_ids.sort();
    let mut expected = vec![attacker_a, attacker_b];
    expected.sort();
    assert_eq!(declared_ids, expected, "both attackers reported");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers,
        "after attackers are declared, still in declare attackers until priority passes"
    );

    // Engine should auto-tap attackers.
    assert!(
        e.state
            .objects
            .get(&attacker_a)
            .map(|o| o.tapped)
            .unwrap_or(false),
        "attacker_a tapped on attack"
    );
    assert!(
        e.state
            .objects
            .get(&attacker_b)
            .map(|o| o.tapped)
            .unwrap_or(false),
        "attacker_b tapped on attack"
    );
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");

    // Declare blockers: only attacker_a is blocked.
    let declared_blockers_batch = e
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: attacker_a,
                blocker_id: blocker,
            }]),
        )
        .expect("declare blocker");
    assert!(
        permanents_moved_in(&declared_blockers_batch).is_empty(),
        "no deaths during blocker declaration itself"
    );
    assert!(
        life_changes_in(&declared_blockers_batch).is_empty(),
        "no life loss during blocker declaration itself"
    );
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    let block_batch = e
        .apply_command(1, &pass())
        .expect("defender pass declare blockers -> combat damage");

    // Mutual destruction on the blocked pair -> both go to graveyard.
    let dead = permanents_moved_in(&block_batch);
    let dead_ids: Vec<u32> = dead.iter().map(|p| p.object_id).collect();
    assert!(
        dead_ids.contains(&attacker_a),
        "attacker_a dies in trade, got {dead_ids:?}"
    );
    assert!(
        dead_ids.contains(&blocker),
        "blocker dies in trade, got {dead_ids:?}"
    );
    assert!(
        !dead_ids.contains(&attacker_b),
        "attacker_b survives, got {dead_ids:?}"
    );
    for pm in &dead {
        assert_eq!(
            pm.destination,
            tricerules_proto::ruled::v1::permanent_moved::Destination::Graveyard as i32,
            "trade victims go to graveyard"
        );
    }

    // Defender takes 2 from attacker_b's unblocked damage.
    let life = life_changes_in(&block_batch);
    assert_eq!(life.len(), 1, "exactly one life change event");
    assert_eq!(life[0].player_id, 1);
    assert_eq!(life[0].delta, -2, "attacker_b deals 2 unblocked");
    assert_eq!(life[0].new_total, 18);
    assert_eq!(e.state.players[1].life, 18);
}

#[test]
fn cast_divination_draws_two_cards() {
    let decks = Some(vec![
        vec![
            "island".into(),
            "divination".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
        vec![
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(901, &[0, 1], 20, decks).expect("new");
    advance_to_main1_from_game_start(&mut e);

    for _ in 0..2 {
        let seeded_island_idx = hand_index_for_card(&e, 0, "island");
        let seeded_island = e.state.players[0].hand.remove(seeded_island_idx);
        e.state.players[0].battlefield.push(seeded_island);
        e.state
            .objects
            .get_mut(&seeded_island)
            .expect("seeded island")
            .zone = tricerules_core::Zone::Battlefield;
    }

    let island_to_play_idx = hand_index_for_card(&e, 0, "island");
    e.apply_command(0, &play_land(island_to_play_idx))
        .expect("play third island");

    let hand_before_cast = e.state.players[0].hand.len();
    let div_idx = hand_index_for_card(&e, 0, "divination");
    e.apply_command(0, &cast_spell(div_idx, vec![]))
        .expect("cast divination");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");

    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before_cast + 1,
        "cast consumes one card and draws two"
    );
}

#[test]
fn giant_growth_changes_combat_outcome() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "forest".into(),
            "giant_growth".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec![
            "forest".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(902, &[0, 1], 20, decks).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let p0_bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let p1_bear = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    let forest_idx = hand_index_for_card(&e, 0, "forest");
    let forest_oid = e.state.players[0].hand.remove(forest_idx);
    e.state.players[0].battlefield.push(forest_oid);
    e.state.objects.get_mut(&forest_oid).expect("forest").zone = tricerules_core::Zone::Battlefield;

    let growth_idx = hand_index_for_card(&e, 0, "giant_growth");
    e.apply_command(
        0,
        &cast_spell(growth_idx, vec![TargetRef { object_id: p0_bear }]),
    )
    .expect("cast growth");
    e.apply_command(0, &pass()).expect("p0 pass growth");
    e.apply_command(1, &pass()).expect("p1 pass growth");

    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap pass begin combat");
    e.apply_command(1, &pass()).expect("nap pass begin combat");
    e.apply_command(0, &declare_attackers(vec![p0_bear]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("ap pass declare attackers");
    e.apply_command(1, &pass())
        .expect("nap pass declare attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: p0_bear,
            blocker_id: p1_bear,
        }]),
    )
    .expect("declare blocker");
    e.apply_command(0, &pass())
        .expect("ap pass declare blockers");
    let damage_batch = e
        .apply_command(1, &pass())
        .expect("nap pass declare blockers");

    let moved_ids: Vec<u32> = permanents_moved_in(&damage_batch)
        .iter()
        .map(|p| p.object_id)
        .collect();
    assert!(moved_ids.contains(&p1_bear), "blocked bear should die");
    assert!(
        !moved_ids.contains(&p0_bear),
        "grown attacker should survive combat"
    );
}

#[test]
fn counterspell_counters_a_spell_on_stack() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "island".into(),
            "island".into(),
            "lightning_bolt".into(),
            "counterspell".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(903, &[0, 1], 20, decks).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");

    for _ in 0..2 {
        let island_idx = hand_index_for_card(&e, 0, "island");
        let island_oid = e.state.players[0].hand.remove(island_idx);
        e.state.players[0].battlefield.push(island_oid);
        e.state
            .objects
            .get_mut(&island_oid)
            .expect("seed island")
            .zone = tricerules_core::Zone::Battlefield;
    }

    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, vec![]))
        .expect("cast bolt");
    let bolt_oid = e.state.stack.last().expect("bolt on stack").id;

    let cs_idx = hand_index_for_card(&e, 0, "counterspell");
    e.apply_command(
        0,
        &cast_spell(
            cs_idx,
            vec![TargetRef {
                object_id: bolt_oid,
            }],
        ),
    )
    .expect("cast counterspell");
    let counterspell_oid = e.state.stack.last().expect("counterspell on stack").id;

    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");

    assert!(e.state.stack.is_empty(), "counterspell should clear stack");
    assert!(e.state.players[0].graveyard.contains(&counterspell_oid));
    assert!(e.state.players[0].graveyard.contains(&bolt_oid));
}

#[test]
fn go_for_the_throat_destroys_target_creature() {
    let decks = Some(vec![
        vec![
            "swamp".into(),
            "go_for_the_throat".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
        ],
        vec![
            "forest".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(904, &[0, 1], 20, decks).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let p1_bear = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    let seeded_swamp_idx = hand_index_for_card(&e, 0, "swamp");
    let seeded_swamp = e.state.players[0].hand.remove(seeded_swamp_idx);
    e.state.players[0].battlefield.push(seeded_swamp);
    e.state
        .objects
        .get_mut(&seeded_swamp)
        .expect("seeded swamp")
        .zone = tricerules_core::Zone::Battlefield;

    let swamp_to_play_idx = hand_index_for_card(&e, 0, "swamp");
    e.apply_command(0, &play_land(swamp_to_play_idx))
        .expect("play second swamp");

    let gftt_idx = hand_index_for_card(&e, 0, "go_for_the_throat");
    e.apply_command(
        0,
        &cast_spell(gftt_idx, vec![TargetRef { object_id: p1_bear }]),
    )
    .expect("cast go for the throat");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");
    assert!(e.state.players[1].graveyard.contains(&p1_bear));
    assert_eq!(
        e.state
            .objects
            .get(&p1_bear)
            .expect("target creature object")
            .zone,
        tricerules_core::Zone::Graveyard
    );
}

#[test]
fn can_cast_new_vanilla_creature_with_swamp() {
    let decks = Some(vec![
        vec![
            "swamp".into(),
            "walking_corpse".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
        ],
        vec![
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
    ]);
    let mut e = GameEngine::new(905, &[0, 1], 20, decks).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let seeded_swamp_idx = hand_index_for_card(&e, 0, "swamp");
    let seeded_swamp = e.state.players[0].hand.remove(seeded_swamp_idx);
    e.state.players[0].battlefield.push(seeded_swamp);
    e.state
        .objects
        .get_mut(&seeded_swamp)
        .expect("seeded swamp")
        .zone = tricerules_core::Zone::Battlefield;

    let swamp_to_play_idx = hand_index_for_card(&e, 0, "swamp");
    e.apply_command(0, &play_land(swamp_to_play_idx))
        .expect("play second swamp");

    let corpse_idx = hand_index_for_card(&e, 0, "walking_corpse");
    e.apply_command(0, &cast_spell(corpse_idx, vec![]))
        .expect("cast walking corpse");
    let corpse_oid = e.state.stack.first().expect("corpse on stack").id;
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");

    assert!(e.state.players[0].battlefield.contains(&corpse_oid));
}

#[test]
fn cannot_cast_spell_until_attackers_declared() {
    let mut e = GameEngine::new(9200, &[0, 1], 20, None).expect("new");
    advance_to_declare_attackers(&mut e);
    let _bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    while !e.state.players[0].hand.iter().any(|oid| {
        e.state
            .objects
            .get(oid)
            .map(|o| o.card_id.as_str())
            == Some("lightning_bolt")
    }) {
        take_card_from_library_to_hand(&mut e, 0, "lightning_bolt");
    }
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    let err = e
        .apply_command(0, &cast_spell(bolt_idx, vec![]))
        .expect_err("cast before attackers illegal");
    assert!(
        err.to_string().contains("cannot cast until attack or block declaration is complete"),
        "unexpected: {err}"
    );

    let bear_oid = battlefield_object_for_card(&e, 0, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![bear_oid]))
        .expect("declare attackers");

    while !e.state.players[0].hand.iter().any(|oid| {
        e.state
            .objects
            .get(oid)
            .map(|o| o.card_id.as_str())
            == Some("mountain")
    }) {
        take_card_from_library_to_hand(&mut e, 0, "mountain");
    }
    let m_idx = hand_index_for_card(&e, 0, "mountain");
    let m_oid = e.state.players[0].hand.remove(m_idx);
    e.state.players[0].battlefield.push(m_oid);
    let o = e.state.objects.get_mut(&m_oid).expect("mountain");
    o.zone = tricerules_core::Zone::Battlefield;
    o.summoning_sick = false;
    o.tapped = false;

    let bolt_idx2 = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx2, vec![]))
        .expect("instant legal after attackers committed");
    assert_eq!(e.state.stack.len(), 1);
}

#[test]
fn cannot_cast_spell_until_blockers_declared() {
    let mut e = GameEngine::new(9300, &[0, 1], 20, None).expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare");
    e.apply_command(0, &pass()).expect("ap pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers -> declare blockers");

    while !e.state.players[1].hand.iter().any(|oid| {
        e.state
            .objects
            .get(oid)
            .map(|o| o.card_id.as_str())
            == Some("giant_growth")
    }) {
        take_card_from_library_to_hand(&mut e, 1, "giant_growth");
    }
    while !e.state.players[1].hand.iter().any(|oid| {
        e.state
            .objects
            .get(oid)
            .map(|o| o.card_id.as_str())
            == Some("forest")
    }) {
        take_card_from_library_to_hand(&mut e, 1, "forest");
    }
    let f_idx = hand_index_for_card(&e, 1, "forest");
    let f_oid = e.state.players[1].hand.remove(f_idx);
    e.state.players[1].battlefield.push(f_oid);
    let fo = e.state.objects.get_mut(&f_oid).expect("forest");
    fo.zone = tricerules_core::Zone::Battlefield;
    fo.summoning_sick = false;
    fo.tapped = false;

    let growth_idx = hand_index_for_card(&e, 1, "giant_growth");
    let err = e
        .apply_command(
            1,
            &cast_spell(
                growth_idx,
                vec![TargetRef {
                    object_id: attacker,
                }],
            ),
        )
        .expect_err("cast before blockers illegal");
    assert!(
        err.to_string().contains("cannot cast until attack or block declaration is complete"),
        "unexpected: {err}"
    );

    e.apply_command(1, &declare_blockers(vec![]))
        .expect("declare no blockers");
    e.apply_command(0, &pass()).expect("ap pass declare blockers");
    let growth_idx2 = hand_index_for_card(&e, 1, "giant_growth");
    e.apply_command(
        1,
        &cast_spell(
            growth_idx2,
            vec![TargetRef {
                object_id: attacker,
            }],
        ),
    )
    .expect("instant legal after blockers committed");
    assert_eq!(e.state.stack.len(), 1);
}
