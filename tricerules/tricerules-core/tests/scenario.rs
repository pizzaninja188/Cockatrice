//! Scripted command sequences (M2).

use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::ruled_event::Ev;
use tricerules_proto::ruled::v1::{
    AddManaToPool, AssignCombatDamage, BlockPair, CastSpell, DeclareAttackers, DeclareBlockers,
    DamagePair, DiscardToHandSize, PassPriority, PlayLand, PreviewDeclareAttackers,
    PreviewDeclareBlockers, PrimitiveYieldStructured, RuledCommand, TargetRef,
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

fn discard_cleanup(hand_card_index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DiscardToHandSize(DiscardToHandSize {
            hand_card_index,
            hand_card_indices: vec![],
        })),
    }
}

fn discard_cleanup_batch(indices: Vec<u32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DiscardToHandSize(DiscardToHandSize {
            hand_card_index: 0,
            hand_card_indices: indices,
        })),
    }
}

/// After leaving the end step, the engine may stop in cleanup for 514.1 discards.
fn resolve_cleanup_discards_if_any(e: &mut GameEngine) {
    while e.state.turn_step == tricerules_core::TurnStep::Cleanup {
        let Some(cp) = e.state.cleanup_discard_player else {
            break;
        };
        let idx = e.state.player_idx(cp).expect("cleanup discard player");
        assert!(
            e.state.players[idx].hand.len() > 7,
            "cleanup without over-max hand"
        );
        e.apply_command(cp, &discard_cleanup(0))
            .expect("discard during cleanup");
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

fn add_mana_to_pool(m: AddManaToPool) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::AddManaToPool(m)),
    }
}

/// Player targets for `DealDamage` spells use `TargetRef.object_id == player_id` (see engine).
fn target_player(pid: i32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id: pid as u32,
    }]
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

fn count_card_id_in_graveyard(e: &GameEngine, player: usize, card_id: &str) -> usize {
    e.state.players[player]
        .graveyard
        .iter()
        .filter(|oid| {
            e.state
                .objects
                .get(oid)
                .map(|o| o.card_id.as_str()) == Some(card_id)
        })
        .count()
}

fn take_card_from_library_to_hand(e: &mut GameEngine, player: usize, card_id: &str) {
    let pos = e.state.players[player]
        .library
        .iter()
        .position(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some(card_id))
        .unwrap_or_else(|| panic!("missing card {card_id} in P{player} library"));
    let oid = e.state.players[player]
        .library
        .remove(pos)
        .expect("index from position()");
    e.state.players[player].hand.push(oid);
    e.state.objects.get_mut(&oid).expect("object").zone = tricerules_core::Zone::Hand;
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
        .expect("begin combat advance");
    // If eligible attackers exist, BeginCombat enters DeclareAttackers; skip them.
    if e.state.turn_step == tricerules_core::TurnStep::DeclareAttackers {
        e.apply_command(player, &primitive_yield())
            .expect("skip attackers to end combat");
    }
    e.apply_command(player, &primitive_yield())
        .expect("end combat to main2");
    e.apply_command(player, &primitive_yield())
        .expect("main2 to end step");
    e.apply_command(player, &primitive_yield())
        .expect("end step to cleanup or next upkeep");
    resolve_cleanup_discards_if_any(e);
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

/// After each stack resolution the active player receives priority (CR-style);
/// repeat a full two-player pass cycle until the stack is empty.
fn resolve_entire_stack_two_player(e: &mut GameEngine) {
    while !e.state.stack.is_empty() {
        pass_both_players(e);
    }
}

fn advance_to_main1_from_game_start(e: &mut GameEngine) {
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Upkeep);
    pass_both_players(e); // upkeep -> draw
    pass_both_players(e); // draw -> main1
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Main1);
}

#[test]
fn primitive_yield_active_skips_double_pass_main1() {
    let mut e = GameEngine::new(99, &[0, 1], 20, None, true).expect("new");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Upkeep);
    e.apply_command(0, &primitive_yield())
        .expect("active primitive");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Draw);
}

#[test]
fn two_player_passes_empty_stack_advances_toward_combat() {
    let mut e = GameEngine::new(99, &[0, 1], 20, None, true).expect("new");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Upkeep);
    e.apply_command(0, &pass()).expect("p0");
    e.apply_command(1, &pass()).expect("p1");
    // After two passes, should leave upkeep to draw.
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Draw);
}

#[test]
fn empty_stack_double_pass_emits_ap_priority_in_new_phase() {
    let mut e = GameEngine::new(99, &[0, 1], 20, None, true).expect("new");
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
    let mut e = GameEngine::new(99, &[0, 1], 20, None, true).expect("new");
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
    let e = GameEngine::new(1, &[0, 1], 20, decks, true).expect("new");
    assert_eq!(
        e.state.players[0].library.len() + e.state.players[0].hand.len(),
        30
    );
}

#[test]
fn play_land_moves_card_from_hand_to_battlefield() {
    let decks = Some(vec![vec!["mountain".into(); 7], vec!["forest".into(); 7]]);
    let mut e = GameEngine::new(7, &[0, 1], 20, decks, true).expect("new");
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
    let mut e = GameEngine::new(13, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");

    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    let pushed = e
        .apply_command(0, &cast_spell(bolt_idx, target_player(1)))
        .expect("cast bolt");
    let bolt_oid = e.state.stack.last().expect("spell on stack").id;
    let stack_push = pushed
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::StackPushed(s)) => Some(s),
            _ => None,
        })
        .expect("stack pushed");
    assert_eq!(stack_push.targets.len(), 1);
    assert_eq!(stack_push.targets[0].object_id, 1);

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
fn lightning_bolt_rejects_basic_land_target() {
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
    let mut e = GameEngine::new(1401, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");
    let land_oid = battlefield_object_for_card(&e, 0, "mountain");
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    let err = e
        .apply_command(
            0,
            &cast_spell(
                bolt_idx,
                vec![TargetRef {
                    object_id: land_oid,
                }],
            ),
        )
        .expect_err("bolt cannot target land");
    assert!(err.to_string().contains("creature"), "unexpected: {err}");
}

#[test]
fn lightning_bolt_rejects_missing_target() {
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
    let mut e = GameEngine::new(1402, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    let err = e
        .apply_command(0, &cast_spell(bolt_idx, vec![]))
        .expect_err("bolt needs a target");
    assert!(
        err.to_string().contains("exactly one target"),
        "unexpected: {err}"
    );
}

#[test]
fn giant_growth_rejects_land_target() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "giant_growth".into(),
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
    let mut e = GameEngine::new(1403, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let forest_idx = hand_index_for_card(&e, 0, "forest");
    e.apply_command(0, &play_land(forest_idx))
        .expect("play forest");
    let land_oid = battlefield_object_for_card(&e, 0, "forest");
    let growth_idx = hand_index_for_card(&e, 0, "giant_growth");
    let err = e
        .apply_command(
            0,
            &cast_spell(
                growth_idx,
                vec![TargetRef {
                    object_id: land_oid,
                }],
            ),
        )
        .expect_err("growth cannot target land");
    assert!(err.to_string().contains("creature"), "unexpected: {err}");
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
    let mut e = GameEngine::new(13, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    let pushed = e
        .apply_command(0, &cast_spell(bolt_idx, target_player(1)))
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
    let mut e = GameEngine::new(13, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, target_player(1)))
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
    // Defender needs an eligible blocker so the engine enters DeclareBlockers with
    // the defender holding priority (rather than auto-declaring empty blockers).
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
            "forest".into(),
            "grizzly_bears".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
    ]);
    let mut e = GameEngine::new(66, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    // Put one creature and two forests on battlefield for attacker.
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
    // Give defender an eligible blocker (untapped, not summoning-sick).
    for card in ["grizzly_bears"] {
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
    // No creatures on battlefield → BeginCombat auto-skips to EndCombat.
    let mut e = GameEngine::new(67, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap pass begin combat");
    let b = e.apply_command(1, &pass()).expect("nap pass begin combat");
    // Engine must skip directly to EndCombat (no DeclareAttackers needed).
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::EndCombat);
    assert!(
        priority_changes_in(&b).contains(&0),
        "active player should hold priority in end_combat after auto-skip"
    );

    // EndCombat still has a full priority pass cycle before postcombat main.
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
    let mut e = GameEngine::new(68, &[0, 1], 20, decks, true).expect("new");
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
    // No eligible blockers for defender: engine auto-declares empty blockers,
    // active player gets priority in DeclareBlockers.
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers,
        "engine should auto-declare empty blockers and stay in DeclareBlockers"
    );
    assert!(
        e.state.combat.as_ref().map_or(false, |c| c.blockers_declared),
        "blockers_declared must be true after auto-skip"
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
fn cleanup_batch_discard_three_at_once() {
    let mut e = GameEngine::new(1002, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let ap_idx = e.state.player_idx(0).unwrap();
    for _ in 0..3 {
        let oid = e.state.players[ap_idx].library.pop_front().expect("library");
        e.state.players[ap_idx].hand.push(oid);
        e.state
            .objects
            .get_mut(&oid)
            .expect("obj")
            .zone = tricerules_core::Zone::Hand;
    }
    assert_eq!(e.state.players[ap_idx].hand.len(), 10);

    e.apply_command(0, &primitive_yield()).expect("main1->begin combat");
    // No eligible attackers: BeginCombat auto-skips to EndCombat.
    e.apply_command(0, &primitive_yield()).expect("begin combat->end combat");
    e.apply_command(0, &primitive_yield()).expect("end combat->main2");
    e.apply_command(0, &primitive_yield()).expect("main2->end step");
    e.apply_command(0, &primitive_yield()).expect("end step->cleanup");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Cleanup);

    e.apply_command(0, &discard_cleanup_batch(vec![9, 8, 7]))
        .expect("batch discard top three");
    assert_eq!(e.state.players[ap_idx].hand.len(), 7);
    assert_eq!(e.state.active_player_id(), 1);
}

#[test]
fn cleanup_step_opens_when_hand_exceeds_max_and_discard_finishes_turn() {
    let mut e = GameEngine::new(1001, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let ap_idx = e.state.player_idx(0).unwrap();
    let oid = e.state.players[ap_idx].library.pop_front().expect("library");
    e.state.players[ap_idx].hand.push(oid);
    e.state
        .objects
        .get_mut(&oid)
        .expect("obj")
        .zone = tricerules_core::Zone::Hand;
    assert!(e.state.players[ap_idx].hand.len() > 7);

    e.apply_command(0, &primitive_yield()).expect("main1->begin combat");
    // No eligible attackers: BeginCombat auto-skips to EndCombat.
    e.apply_command(0, &primitive_yield()).expect("begin combat->end combat");
    e.apply_command(0, &primitive_yield()).expect("end combat->main2");
    e.apply_command(0, &primitive_yield()).expect("main2->end step");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::EndStep);

    e.apply_command(0, &primitive_yield()).expect("end step->cleanup");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Cleanup);
    assert_eq!(e.state.cleanup_discard_player, Some(0));

    e.apply_command(0, &discard_cleanup(0)).expect("discard one");
    assert_eq!(e.state.players[ap_idx].hand.len(), 7);
    assert_eq!(e.state.active_player_id(), 1);
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Upkeep);
}

#[test]
fn main2_double_pass_advances_to_end_step_stop() {
    let mut e = GameEngine::new(69, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    // No eligible attackers: BeginCombat auto-skips to EndCombat in one yield.
    e.apply_command(0, &primitive_yield())
        .expect("begin combat to end combat");
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
    let mut e = GameEngine::new(70, &[0, 1], 20, None, true).expect("new");
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

/// CR 103.8: only the starting player skips their first draw. The duel `turn` counter can remain 1
/// for the second seat's first turn (it bumps when active wraps to seat 0), so skip logic must
/// key off who started, not `turn == 1` alone.
#[test]
fn second_seat_first_draw_draws_when_seat_zero_started() {
    let mut e = GameEngine::new(71, &[0, 1], 20, None, true).expect("new");
    assert_eq!(e.state.starting_player_idx, 0);
    advance_to_main1_from_game_start(&mut e);
    assert_eq!(
        e.state.players[0].hand.len(),
        7,
        "starting seat skipped first draw"
    );
    end_active_turn(&mut e, 0);
    assert_eq!(e.state.active_player_id(), 1);
    assert_eq!(e.state.turn, 1);
    e.apply_command(1, &pass()).expect("ap pass upkeep");
    e.apply_command(0, &pass()).expect("nap pass upkeep");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Draw);
    assert_eq!(
        e.state.players[1].hand.len(),
        8,
        "second seat must draw on their first draw step"
    );
}

#[test]
fn cast_1u_creature_pays_from_mana_pool_without_tapping_extra_island() {
    let decks = Some(vec![
        vec![
            "island".into(),
            "island".into(),
            "mountain".into(),
            "coral_merfolk".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
        vec!["mountain".into(); 7],
    ]);
    let mut e = GameEngine::new(202, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Two islands + mountain on the battlefield (no land drop this turn).
    for _ in 0..2 {
        let idx = hand_index_for_card(&e, 0, "island");
        let oid = e.state.players[0].hand.remove(idx);
        e.state.players[0].battlefield.push(oid);
        e.state.objects.get_mut(&oid).expect("obj").zone = tricerules_core::Zone::Battlefield;
    }
    {
        let idx = hand_index_for_card(&e, 0, "mountain");
        let oid = e.state.players[0].hand.remove(idx);
        e.state.players[0].battlefield.push(oid);
        e.state.objects.get_mut(&oid).expect("obj").zone = tricerules_core::Zone::Battlefield;
    }
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            u: 1,
            r: 1,
            ..Default::default()
        }),
    )
    .expect("pool like two land taps");
    let merfolk_idx = hand_index_for_card(&e, 0, "coral_merfolk");
    e.apply_command(0, &cast_spell(merfolk_idx, vec![])).expect("cast");
    let tapped_islands = e.state.players[0]
        .battlefield
        .iter()
        .filter(|oid| {
            e.state
                .objects
                .get(*oid)
                .map(|o| o.card_id == "island" && o.tapped)
                .unwrap_or(false)
        })
        .count();
    assert_eq!(
        tapped_islands, 0,
        "1U paid from pool; no extra island should auto-tap"
    );
    let mountain_oid = battlefield_object_for_card(&e, 0, "mountain");
    assert!(
        !e.state.objects.get(&mountain_oid).expect("mountain").tapped,
        "mountain should not be tapped by engine payment"
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
    let mut e = GameEngine::new(22, &[0, 1], 20, decks, true).expect("new");
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
    let mut e = GameEngine::new(333, &[0, 1], 20, decks, true).expect("new");
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
    e.apply_command(0, &cast_spell(bolt_one, target_player(1)))
        .expect("cast first bolt");
    assert_eq!(
        e.state.priority_player_id(),
        0,
        "caster should keep priority after casting first spell"
    );

    let bolt_two = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_two, target_player(1)))
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
    let mut e = GameEngine::new(144, &[0, 1], 20, decks, true).expect("new");
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
    assert!(!e.state.objects.get(&p1_island_a).expect("p1 island").tapped);

    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, target_player(1)))
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
        e.state.objects.get(&p1_island_a).expect("p1 island").tapped,
        "an island should tap to help pay UU"
    );
    assert_eq!(e.state.stack.len(), 2, "bolt and counterspell on stack");

    e.apply_command(1, &pass()).expect("p1 pass after casting counter");
    e.apply_command(0, &pass()).expect("p0 pass resolves counterspell");
    assert!(e.state.stack.is_empty(), "stack empty after counter");
    assert_eq!(e.state.active_player_id(), 0, "AP is P0 in this test");
    assert_eq!(
        e.state.priority_player_id(),
        0,
        "with empty stack, priority should return to active player (CR 117.3c)"
    );
    assert_eq!(
        e.state.passes_since_stack_change, 0,
        "pass counter should reset after stack closed"
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
    let mut e = GameEngine::new(88, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let hand_before_turn = e.state.players[0].hand.len();
    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx))
        .expect("play mountain");

    let mountain_oid = battlefield_object_for_card(&e, 0, "mountain");
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, target_player(1)))
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
    // No eligible attackers: BeginCombat auto-skips to EndCombat on both-player pass.
    pass_both_players(&mut e); // begin combat -> end combat
    pass_both_players(&mut e); // end combat -> main2
    pass_both_players(&mut e); // main2 -> end step
    pass_both_players(&mut e); // end step -> cleanup or p0 upkeep
    resolve_cleanup_discards_if_any(&mut e);
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
    let mut e = GameEngine::new(101, &[0, 1], 20, decks, true).expect("new");
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
    let mut e = GameEngine::new(202, &[0, 1], 20, decks, true).expect("new");
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

/// Inject a creature directly onto the battlefield without consuming a card from hand or library.
/// Use this when you need an eligible attacker/blocker but the deck budget is already spent.
fn inject_creature_on_battlefield(e: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let id = e.state.next_object_id;
    e.state.next_object_id += 1;
    let player_id = e.state.players[player].id;
    e.state.objects.insert(
        id,
        tricerules_core::state::GameObject {
            id,
            owner: player_id,
            card_id: card_id.to_string(),
            zone: tricerules_core::Zone::Battlefield,
            tapped: false,
            summoning_sick: false,
            power: Some(2),
            toughness: Some(2),
            damage: 0,
            plus_one_plus_one: 0,
            minus_one_minus_one: 0,
        },
    );
    e.state.players[player].battlefield.push(id);
    id
}

fn advance_to_declare_attackers(e: &mut GameEngine) {
    advance_to_main1_from_game_start(e);
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    // Inject an eligible attacker (no hand/library consumed) so BeginCombat enters DeclareAttackers.
    inject_creature_on_battlefield(e, 0, "grizzly_bears");
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
            Some(Ev::LifeChanged(lc)) => Some(*lc),
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
            Some(Ev::PermanentMoved(pm)) => Some(*pm),
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

fn blockers_declared_in(
    batch: &tricerules_proto::ruled::v1::RuledEventBatch,
) -> Vec<tricerules_proto::ruled::v1::BlockersDeclared> {
    batch
        .events
        .iter()
        .filter_map(|ev| match &ev.ev {
            Some(Ev::BlockersDeclared(bd)) => Some(bd.clone()),
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
    let mut e = GameEngine::new(404, &[0, 1], 20, decks, true).expect("new");
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
    assert_eq!(p0.hand_object_id.len(), p0.hand.len());
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
    let mut e = GameEngine::new(505, &[0, 1], 20, None, true).expect("new");
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
fn preview_declare_attackers_is_rejected_by_engine() {
    let mut e = GameEngine::new(508, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let idx_before = e.state.command_index;
    let cmd = RuledCommand {
        cmd: Some(Cmd::PreviewDeclareAttackers(PreviewDeclareAttackers {
            creature_ids: vec![],
        })),
    };
    let err = e.apply_command(0, &cmd).expect_err("preview must not apply");
    assert!(
        err.to_string().contains("preview"),
        "unexpected err: {err}"
    );
    assert_eq!(e.state.command_index, idx_before);
}

#[test]
fn preview_declare_blockers_is_rejected_by_engine() {
    let mut e = GameEngine::new(507, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let idx_before = e.state.command_index;
    let cmd = RuledCommand {
        cmd: Some(Cmd::PreviewDeclareBlockers(PreviewDeclareBlockers {
            block_pairs: vec![],
        })),
    };
    let err = e.apply_command(0, &cmd).expect_err("preview must not apply");
    assert!(
        err.to_string().contains("preview"),
        "unexpected err: {err}"
    );
    assert_eq!(
        e.state.command_index, idx_before,
        "preview must not advance command_index"
    );
}

#[test]
fn declare_blockers_emits_blockers_declared_event() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "grizzly_bears".into(),
        ],
        vec![
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "grizzly_bears".into(),
        ],
    ]);
    let mut e = GameEngine::new(506, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let atk = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let blk = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![atk]))
        .expect("declare attackers");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    let b = e
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: atk,
                blocker_id: blk,
            }]),
        )
        .expect("declare blockers");
    let evs = blockers_declared_in(&b);
    assert_eq!(evs.len(), 1, "exactly one BlockersDeclared event");
    assert_eq!(evs[0].block_pairs.len(), 1);
    assert_eq!(evs[0].block_pairs[0].attacker_id, atk);
    assert_eq!(evs[0].block_pairs[0].blocker_id, blk);
}

#[test]
fn unblocked_combat_damage_emits_life_changed() {
    let mut e = GameEngine::new(606, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let bears_a = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let bears_b = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![bears_a, bears_b]))
        .expect("two attackers");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    // No eligible blockers: engine auto-declares empty blockers, active player has priority.
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
    let mut e = GameEngine::new(707, &[0, 1], 20, decks, true).expect("new");
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
    let mut e = GameEngine::new(808, &[0, 1], 20, decks, true).expect("new");
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
    let mut e = GameEngine::new(901, &[0, 1], 20, decks, true).expect("new");
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
fn second_sorcery_rejected_while_spell_on_stack_even_with_priority() {
    let p0_deck: Vec<String> = std::iter::repeat_n("island".into(), 25)
        .chain(std::iter::repeat_n("divination".into(), 5))
        .collect();
    let decks = Some(vec![p0_deck, vec!["forest".into(); 15]]);
    let mut e = GameEngine::new(904, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    while e
        .state
        .players[0]
        .hand
        .iter()
        .filter(|oid| {
            e.state
                .objects
                .get(*oid)
                .map(|o| o.card_id.as_str())
                == Some("divination")
        })
        .count()
        < 2
    {
        take_card_from_library_to_hand(&mut e, 0, "divination");
    }
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            u: 1,
            c: 2,
            ..Default::default()
        }),
    )
    .expect("mana for 2U");
    let div0 = hand_index_for_card(&e, 0, "divination");
    e.apply_command(0, &cast_spell(div0, vec![]))
        .expect("first divination");
    assert_eq!(
        e.state.stack.len(),
        1,
        "first sorcery should sit on the stack while AP still has priority"
    );

    let div1 = hand_index_for_card(&e, 0, "divination");
    let err = e
        .apply_command(0, &cast_spell(div1, vec![]))
        .expect_err("second sorcery with stack nonempty");
    assert!(
        err.to_string().contains("sorcery speed"),
        "unexpected: {err}"
    );
}

#[test]
fn nonactive_player_cannot_play_land_in_opponents_main() {
    let decks = Some(vec![
        vec!["mountain".into(); 10],
        vec!["forest".into(); 10],
    ]);
    let mut e = GameEngine::new(905, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &pass()).expect("active passes");
    assert_eq!(e.state.priority_player_id(), 1);
    let forest_idx = hand_index_for_card(&e, 1, "forest");
    let err = e
        .apply_command(1, &play_land(forest_idx))
        .expect_err("NAP cannot play land during AP main");
    assert!(
        err.to_string().contains("sorcery speed"),
        "unexpected: {err}"
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
    let mut e = GameEngine::new(902, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let p0_bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let p1_bear = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    let forest_idx = hand_index_for_card(&e, 0, "forest");
    let forest_oid = e.state.players[0].hand.remove(forest_idx);
    e.state.players[0].battlefield.push(forest_oid);
    e.state.objects.get_mut(&forest_oid).expect("forest").zone = tricerules_core::Zone::Battlefield;

    let growth_idx = hand_index_for_card(&e, 0, "giant_growth");
    let growth_batch = e
        .apply_command(
            0,
            &cast_spell(growth_idx, vec![TargetRef { object_id: p0_bear }]),
        )
        .expect("cast growth");
    let growth_push = growth_batch
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::StackPushed(s)) => Some(s),
            _ => None,
        })
        .expect("growth stack pushed");
    assert_eq!(growth_push.targets.len(), 1);
    assert_eq!(growth_push.targets[0].object_id, p0_bear);
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

/// Stack LIFO: `Lightning Bolt` on top kills the creature; `Giant Growth` underneath fizzles (CR 608.2b).
#[test]
fn giant_growth_fizzles_if_creature_target_dies_before_resolution() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "grizzly_bears".into(),
            "giant_growth".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(91021, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    let forest_idx = hand_index_for_card(&e, 0, "forest");
    let forest_oid = e.state.players[0].hand.remove(forest_idx);
    e.state.players[0].battlefield.push(forest_oid);
    e.state
        .objects
        .get_mut(&forest_oid)
        .expect("forest")
        .zone = tricerules_core::Zone::Battlefield;

    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    let mountain_oid = e.state.players[0].hand.remove(mountain_idx);
    e.state.players[0].battlefield.push(mountain_oid);
    e.state
        .objects
        .get_mut(&mountain_oid)
        .expect("mountain")
        .zone = tricerules_core::Zone::Battlefield;

    let growth_idx = hand_index_for_card(&e, 0, "giant_growth");
    e.apply_command(
        0,
        &cast_spell(growth_idx, vec![TargetRef { object_id: bear }]),
    )
    .expect("cast growth");

    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(
        0,
        &cast_spell(bolt_idx, vec![TargetRef { object_id: bear }]),
    )
    .expect("cast bolt on top of growth");

    assert_eq!(e.state.stack.len(), 2);

    let mut growth_fizzled = false;
    let mut saw_pump_log = false;
    while !e.state.stack.is_empty() {
        let first = e.state.priority_player_id();
        let second = if first == e.state.players[0].id {
            e.state.players[1].id
        } else {
            e.state.players[0].id
        };
        e.apply_command(first, &pass()).expect("pass");
        let batch = e.apply_command(second, &pass()).expect("pass resolves");
        for ev in &batch.events {
            if let Some(Ev::Log(lm)) = &ev.ev {
                if lm.text.contains("Giant Growth") && lm.text.contains("fizzles") {
                    growth_fizzled = true;
                }
                if lm.text.contains("+3/+3") {
                    saw_pump_log = true;
                }
            }
        }
    }

    assert!(growth_fizzled, "expected Giant Growth to fizzle");
    assert!(
        !saw_pump_log,
        "fizzled pump spell must not log +3/+3 line"
    );
    let dead = e.state.objects.get(&bear).expect("bear object");
    assert_eq!(dead.zone, tricerules_core::Zone::Graveyard);
    assert_eq!(dead.power, Some(2));
    assert_eq!(dead.toughness, Some(2));
}

/// Second bolt should not add damage to a creature already in the graveyard (608.2b).
#[test]
fn lightning_bolt_fizzles_when_creature_target_left_battlefield() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "mountain".into(),
            "grizzly_bears".into(),
            "lightning_bolt".into(),
            "lightning_bolt".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(91022, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    for _ in 0..2 {
        let mi = hand_index_for_card(&e, 0, "mountain");
        let oid = e.state.players[0].hand.remove(mi);
        e.state.players[0].battlefield.push(oid);
        e.state.objects.get_mut(&oid).expect("mountain").zone =
            tricerules_core::Zone::Battlefield;
    }

    let bolt_a = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(
        0,
        &cast_spell(bolt_a, vec![TargetRef { object_id: bear }]),
    )
    .expect("first bolt");
    let bolt_b = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(
        0,
        &cast_spell(bolt_b, vec![TargetRef { object_id: bear }]),
    )
    .expect("second bolt on top");

    resolve_entire_stack_two_player(&mut e);

    let dead = e.state.objects.get(&bear).expect("bear");
    assert_eq!(dead.zone, tricerules_core::Zone::Graveyard);
    assert_eq!(dead.damage, 3, "only the first resolving bolt should deal damage");
}

/// `Go for the Throat` under a bolt that kills the same creature fizzles on resolution.
#[test]
fn go_for_the_throat_fizzles_when_creature_target_left_battlefield() {
    let decks = Some(vec![
        vec![
            "swamp".into(),
            "swamp".into(),
            "grizzly_bears".into(),
            "go_for_the_throat".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "forest".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(91023, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    let mountain_oid = e.state.players[0].hand.remove(mountain_idx);
    e.state.players[0].battlefield.push(mountain_oid);
    e.state
        .objects
        .get_mut(&mountain_oid)
        .expect("mountain")
        .zone = tricerules_core::Zone::Battlefield;

    for _ in 0..2 {
        let si = hand_index_for_card(&e, 0, "swamp");
        let oid = e.state.players[0].hand.remove(si);
        e.state.players[0].battlefield.push(oid);
        e.state.objects.get_mut(&oid).expect("swamp").zone = tricerules_core::Zone::Battlefield;
    }

    let gfth_idx = hand_index_for_card(&e, 0, "go_for_the_throat");
    e.apply_command(
        0,
        &cast_spell(gfth_idx, vec![TargetRef { object_id: bear }]),
    )
    .expect("go for the throat");

    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(
        0,
        &cast_spell(bolt_idx, vec![TargetRef { object_id: bear }]),
    )
    .expect("bolt on top");

    let mut saw_destroy = false;
    let mut saw_fizzle = false;
    while !e.state.stack.is_empty() {
        let first = e.state.priority_player_id();
        let second = if first == e.state.players[0].id {
            e.state.players[1].id
        } else {
            e.state.players[0].id
        };
        e.apply_command(first, &pass()).expect("pass");
        let batch = e.apply_command(second, &pass()).expect("resolve");
        for ev in &batch.events {
            if let Some(Ev::Log(lm)) = &ev.ev {
                if lm.text.contains("destroys") && lm.text.contains("Grizzly Bears") {
                    saw_destroy = true;
                }
                if lm.text.contains("Go for the Throat") && lm.text.contains("fizzles") {
                    saw_fizzle = true;
                }
            }
        }
    }

    assert!(
        !saw_destroy,
        "destroy effect should not run when the creature is already gone"
    );
    assert!(saw_fizzle);
}

/// Top counterspell counters the bolt; the second counterspell's target is gone — it fizzles.
#[test]
fn counterspell_fizzles_when_original_target_already_left_stack() {
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
            "island".into(),
            "counterspell".into(),
            "island".into(),
            "island".into(),
            "counterspell".into(),
            "island".into(),
        ],
    ]);
    let mut e = GameEngine::new(91024, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let m0 = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(m0)).expect("mountain");

    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, target_player(1)))
        .expect("bolt");
    e.apply_command(0, &pass()).expect("AP pass so NAP can respond");

    let bolt_oid = e
        .state
        .stack
        .iter()
        .find(|s| s.card_id == "lightning_bolt")
        .expect("bolt on stack")
        .id;

    for _ in 0..4 {
        let ii = hand_index_for_card(&e, 1, "island");
        let oid = e.state.players[1].hand.remove(ii);
        e.state.players[1].battlefield.push(oid);
        e.state.objects.get_mut(&oid).expect("island").zone =
            tricerules_core::Zone::Battlefield;
    }

    let cs1 = hand_index_for_card(&e, 1, "counterspell");
    e.apply_command(
        1,
        &cast_spell(
            cs1,
            vec![TargetRef {
                object_id: bolt_oid,
            }],
        ),
    )
    .expect("counter 1");

    let cs2 = hand_index_for_card(&e, 1, "counterspell");
    e.apply_command(
        1,
        &cast_spell(
            cs2,
            vec![TargetRef {
                object_id: bolt_oid,
            }],
        ),
    )
    .expect("counter 2 on top");

    assert_eq!(e.state.stack.len(), 3);

    let mut fizzle_logs = 0usize;
    while !e.state.stack.is_empty() {
        let first = e.state.priority_player_id();
        let second = if first == e.state.players[0].id {
            e.state.players[1].id
        } else {
            e.state.players[0].id
        };
        e.apply_command(first, &pass()).expect("pass");
        let batch = e.apply_command(second, &pass()).expect("resolve");
        fizzle_logs += batch.events.iter().filter(|ev| {
            matches!(&ev.ev, Some(Ev::Log(l)) if l.text.contains("fizzles"))
        }).count();
    }

    assert_eq!(fizzle_logs, 1, "only the second counterspell should fizzle");
    assert_eq!(e.state.players[1].life, 20, "bolt never dealt damage");
}

#[test]
fn giant_growth_pump_expires_after_active_turn_ends() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "giant_growth".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(904, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let forest_idx = hand_index_for_card(&e, 0, "forest");
    let forest_oid = e.state.players[0].hand.remove(forest_idx);
    e.state.players[0].battlefield.push(forest_oid);
    e.state.objects.get_mut(&forest_oid).expect("forest").zone = tricerules_core::Zone::Battlefield;

    let growth_idx = hand_index_for_card(&e, 0, "giant_growth");
    e.apply_command(
        0,
        &cast_spell(growth_idx, vec![TargetRef { object_id: bear }]),
    )
    .expect("cast growth");
    pass_both_players(&mut e);

    let o = e.state.objects.get(&bear).expect("bear");
    assert_eq!(o.power, Some(5));
    assert_eq!(o.toughness, Some(5));

    end_active_turn(&mut e, 0);

    let o2 = e.state.objects.get(&bear).expect("bear after turn");
    assert_eq!(o2.power, Some(2), "Giant Growth should expire at end of turn");
    assert_eq!(o2.toughness, Some(2));
}

#[test]
fn marked_damage_clears_at_cleanup() {
    let decks = Some(vec![
        {
            let mut d = vec!["forest".into(), "giant_growth".into(), "grizzly_bears".into()];
            d.extend(std::iter::repeat_n("forest".into(), 17));
            d
        },
        vec!["mountain".into(); 20],
    ]);
    let mut e = GameEngine::new(906, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let forest_idx = hand_index_for_card(&e, 0, "forest");
    let forest_oid = e.state.players[0].hand.remove(forest_idx);
    e.state.players[0].battlefield.push(forest_oid);
    e.state.objects.get_mut(&forest_oid).expect("forest").zone = tricerules_core::Zone::Battlefield;

    let growth_idx = hand_index_for_card(&e, 0, "giant_growth");
    e.apply_command(
        0,
        &cast_spell(growth_idx, vec![TargetRef { object_id: bear }]),
    )
    .expect("cast growth");
    pass_both_players(&mut e);

    assert_eq!(e.state.objects.get(&bear).expect("bear").damage, 0);

    if let Some(o) = e.state.objects.get_mut(&bear) {
        o.damage = 1;
    }
    assert_eq!(e.state.objects.get(&bear).expect("bear").damage, 1);

    end_active_turn(&mut e, 0);

    assert_eq!(
        e.state.objects.get(&bear).expect("bear after cleanup").damage,
        0,
        "marked damage should clear during cleanup"
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
    let mut e = GameEngine::new(903, &[0, 1], 20, decks, true).expect("new");
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
    e.apply_command(0, &cast_spell(bolt_idx, target_player(1)))
        .expect("cast bolt");
    let bolt_oid = e.state.stack.last().expect("bolt on stack").id;

    let cs_idx = hand_index_for_card(&e, 0, "counterspell");
    let cs_batch = e
        .apply_command(
            0,
            &cast_spell(
                cs_idx,
                vec![TargetRef {
                    object_id: bolt_oid,
                }],
            ),
        )
        .expect("cast counterspell");
    let cs_push = cs_batch
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::StackPushed(s)) => Some(s),
            _ => None,
        })
        .expect("counterspell stack pushed");
    assert_eq!(cs_push.targets.len(), 1);
    assert_eq!(cs_push.targets[0].object_id, bolt_oid);
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
    let mut e = GameEngine::new(904, &[0, 1], 20, decks, true).expect("new");
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
    let mut e = GameEngine::new(905, &[0, 1], 20, decks, true).expect("new");
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
    let mut e = GameEngine::new(9200, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let _bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    while !e.state.players[0]
        .hand
        .iter()
        .any(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some("lightning_bolt"))
    {
        take_card_from_library_to_hand(&mut e, 0, "lightning_bolt");
    }
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    let err = e
        .apply_command(0, &cast_spell(bolt_idx, target_player(1)))
        .expect_err("cast before attackers illegal");
    assert!(
        err.to_string()
            .contains("cannot cast until attack or block declaration is complete"),
        "unexpected: {err}"
    );

    let bear_oid = battlefield_object_for_card(&e, 0, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![bear_oid]))
        .expect("declare attackers");

    while !e.state.players[0]
        .hand
        .iter()
        .any(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some("mountain"))
    {
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
    e.apply_command(0, &cast_spell(bolt_idx2, target_player(1)))
        .expect("instant legal after attackers committed");
    assert_eq!(e.state.stack.len(), 1);
}

#[test]
fn cannot_cast_spell_until_blockers_declared() {
    let mut e = GameEngine::new(9300, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    // Inject an eligible blocker for the defender so the engine prompts them in DeclareBlockers.
    inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare");
    e.apply_command(0, &pass())
        .expect("ap pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers -> declare blockers");

    while !e.state.players[1]
        .hand
        .iter()
        .any(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some("giant_growth"))
    {
        take_card_from_library_to_hand(&mut e, 1, "giant_growth");
    }
    while !e.state.players[1]
        .hand
        .iter()
        .any(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some("forest"))
    {
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
        err.to_string()
            .contains("cannot cast until attack or block declaration is complete"),
        "unexpected: {err}"
    );

    e.apply_command(1, &declare_blockers(vec![]))
        .expect("declare no blockers");
    e.apply_command(0, &pass())
        .expect("ap pass declare blockers");
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

/// Active casts two `Lightning Bolt` while holding priority, then non-active responds
/// with a third bolt. Stack resolves LIFO: NAP's bolt, then AP's second, then AP's first.
#[test]
fn three_bolts_stack_lifo_active_sequential_then_non_active_response() {
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
            "mountain".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
    ]);
    let mut e = GameEngine::new(4401, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let m0a = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(m0a))
        .expect("p0 play mountain");
    let m0b = hand_index_for_card(&e, 0, "mountain");
    let m0b_oid = e.state.players[0].hand.remove(m0b);
    e.state.players[0].battlefield.push(m0b_oid);
    e.state
        .objects
        .get_mut(&m0b_oid)
        .expect("p0 second mountain")
        .zone = tricerules_core::Zone::Battlefield;

    let bolt_p0_first = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_p0_first, target_player(1)))
        .expect("p0 first bolt");
    let bolt_p0_second = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_p0_second, target_player(1)))
        .expect("p0 second bolt while holding priority");
    assert_eq!(
        e.state.stack.len(),
        2,
        "p0 should have stacked two bolts before passing"
    );
    assert_eq!(
        e.state.priority_player_id(),
        0,
        "active player keeps priority after sequential casts"
    );

    for _ in 0..2 {
        let mi = hand_index_for_card(&e, 1, "mountain");
        let oid = e.state.players[1].hand.remove(mi);
        e.state.players[1].battlefield.push(oid);
        e.state
            .objects
            .get_mut(&oid)
            .expect("p1 seeded mountain")
            .zone = tricerules_core::Zone::Battlefield;
    }
    let bolt_p1 = hand_index_for_card(&e, 1, "lightning_bolt");
    e.apply_command(0, &pass()).expect("p0 pass to NAP");
    e.apply_command(1, &cast_spell(bolt_p1, target_player(0)))
        .expect("p1 bolt on top of stack");

    assert_eq!(
        e.state
            .stack
            .iter()
            .map(|s| s.card_id.as_str())
            .collect::<Vec<_>>(),
        vec!["lightning_bolt", "lightning_bolt", "lightning_bolt"],
        "bottom-to-top: AP bolt, AP bolt, NAP bolt"
    );
    assert_eq!(e.state.priority_player_id(), 1);

    // Do not pass here alone: with `passes_since_stack_change == 0`, a lone NAP pass would
    // leave `passes_since == 1` and the next AP pass would resolve the top spell mid–`pass_both_players`.
    resolve_entire_stack_two_player(&mut e);

    assert!(e.state.stack.is_empty());
    assert_eq!(e.state.players[0].life, 17, "NAP bolt resolves first (3 to P0)");
    assert_eq!(
        e.state.players[1].life, 14,
        "then both AP bolts (6 total to P1)"
    );
}

/// Five `Lightning Bolt`s on one stack (AP stacks three, passes; NAP stacks two). Covers the
/// Cockatrice/Servatrice case where resolved NAP spells must move from the canonical stack zone
/// (lowest player id) into the caster's graveyard — engine-only regression for LIFO + zone state.
#[test]
fn five_lightning_bolts_combined_stack_resolves_lifo_two_players() {
    let decks = Some(vec![
        vec![
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "lightning_bolt".into(),
            "lightning_bolt".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "mountain".into(),
            "mountain".into(),
            "lightning_bolt".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
    ]);
    let mut e = GameEngine::new(4405, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let m0a = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(m0a))
        .expect("p0 play first mountain");
    for _ in 0..2 {
        let mi = hand_index_for_card(&e, 0, "mountain");
        let oid = e.state.players[0].hand.remove(mi);
        e.state.players[0].battlefield.push(oid);
        e.state
            .objects
            .get_mut(&oid)
            .expect("p0 seeded mountain")
            .zone = tricerules_core::Zone::Battlefield;
    }

    let b0 = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(b0, target_player(1)))
        .expect("p0 first bolt");
    let b1 = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(b1, target_player(1)))
        .expect("p0 second bolt");
    let b2 = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(b2, target_player(1)))
        .expect("p0 third bolt");
    assert_eq!(
        e.state.stack.len(),
        3,
        "AP should stack three bolts before passing"
    );
    assert_eq!(e.state.priority_player_id(), 0);

    e.apply_command(0, &pass()).expect("AP pass — priority to NAP");

    for _ in 0..2 {
        let mi = hand_index_for_card(&e, 1, "mountain");
        let oid = e.state.players[1].hand.remove(mi);
        e.state.players[1].battlefield.push(oid);
        e.state
            .objects
            .get_mut(&oid)
            .expect("p1 seeded mountain")
            .zone = tricerules_core::Zone::Battlefield;
    }
    let b3 = hand_index_for_card(&e, 1, "lightning_bolt");
    e.apply_command(1, &cast_spell(b3, target_player(0)))
        .expect("p1 first bolt");
    let b4 = hand_index_for_card(&e, 1, "lightning_bolt");
    e.apply_command(1, &cast_spell(b4, target_player(0)))
        .expect("p1 second bolt while holding priority");

    assert_eq!(
        e.state.stack.len(),
        5,
        "combined stack: three from AP (bottom) then two from NAP (top)"
    );
    assert_eq!(
        e.state
            .stack
            .iter()
            .map(|s| s.card_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "lightning_bolt",
            "lightning_bolt",
            "lightning_bolt",
            "lightning_bolt",
            "lightning_bolt"
        ]
    );
    assert_eq!(e.state.priority_player_id(), 1);

    resolve_entire_stack_two_player(&mut e);

    assert!(e.state.stack.is_empty());
    assert_eq!(e.state.players[0].life, 14, "NAP's two bolts resolve first (6 to P0)");
    assert_eq!(e.state.players[1].life, 11, "then AP's three bolts (9 to P1)");
    assert_eq!(
        count_card_id_in_graveyard(&e, 0, "lightning_bolt"),
        3,
        "AP's three bolts in AP graveyard"
    );
    assert_eq!(
        count_card_id_in_graveyard(&e, 1, "lightning_bolt"),
        2,
        "NAP's two bolts in NAP graveyard"
    );
}

/// NAP casts two bolts in a row while holding priority in response to AP's bolt.
#[test]
fn non_active_holds_priority_two_bolts_on_stack_above_active_bolt() {
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
            "mountain".into(),
            "mountain".into(),
            "lightning_bolt".into(),
            "lightning_bolt".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
    ]);
    let mut e = GameEngine::new(4402, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let m0 = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(m0))
        .expect("p0 play mountain");
    let bolt_ap = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_ap, target_player(1)))
        .expect("AP bolt targeting P1");
    e.apply_command(0, &pass()).expect("AP pass — priority to P1");

    for _ in 0..2 {
        let mi = hand_index_for_card(&e, 1, "mountain");
        let oid = e.state.players[1].hand.remove(mi);
        e.state.players[1].battlefield.push(oid);
        e.state
            .objects
            .get_mut(&oid)
            .expect("p1 seeded mountain")
            .zone = tricerules_core::Zone::Battlefield;
    }

    let b1 = hand_index_for_card(&e, 1, "lightning_bolt");
    e.apply_command(1, &cast_spell(b1, target_player(0)))
        .expect("NAP first bolt");
    assert_eq!(e.state.priority_player_id(), 1);
    let b2 = hand_index_for_card(&e, 1, "lightning_bolt");
    e.apply_command(1, &cast_spell(b2, target_player(0)))
        .expect("NAP second bolt while holding priority");
    assert_eq!(e.state.stack.len(), 3);

    resolve_entire_stack_two_player(&mut e);

    assert!(e.state.stack.is_empty());
    assert_eq!(e.state.players[0].life, 14, "two NAP bolts resolve before AP's");
    assert_eq!(e.state.players[1].life, 17, "AP bolt still resolves last");
}

/// AP stacks two bolts, passes; NAP counters the top (second) bolt so only the first resolves.
#[test]
fn counterspell_on_top_bolt_fizzles_second_leaves_bottom_bolt() {
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
            "island".into(),
            "island".into(),
            "counterspell".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
    ]);
    let mut e = GameEngine::new(4403, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let m0a = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(m0a))
        .expect("p0 play mountain");
    let m0b = hand_index_for_card(&e, 0, "mountain");
    let m0b_oid = e.state.players[0].hand.remove(m0b);
    e.state.players[0].battlefield.push(m0b_oid);
    e.state
        .objects
        .get_mut(&m0b_oid)
        .expect("p0 second mountain")
        .zone = tricerules_core::Zone::Battlefield;

    let bolt_bottom = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_bottom, target_player(1)))
        .expect("first bolt (stack bottom)");
    let bolt_top = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_top, target_player(1)))
        .expect("second bolt while holding priority (stack top before counter)");
    let top_bolt_oid = e.state.stack.last().expect("top bolt").id;
    e.apply_command(0, &pass()).expect("AP pass");

    for _ in 0..2 {
        let ii = hand_index_for_card(&e, 1, "island");
        let oid = e.state.players[1].hand.remove(ii);
        e.state.players[1].battlefield.push(oid);
        e.state
            .objects
            .get_mut(&oid)
            .expect("p1 island")
            .zone = tricerules_core::Zone::Battlefield;
    }
    let cs_idx = hand_index_for_card(&e, 1, "counterspell");
    e.apply_command(
        1,
        &cast_spell(
            cs_idx,
            vec![TargetRef {
                object_id: top_bolt_oid,
            }],
        ),
    )
    .expect("counterspell targets AP's second bolt");

    assert_eq!(e.state.stack.len(), 3, "bottom bolt, top bolt, counterspell");

    resolve_entire_stack_two_player(&mut e);

    assert!(e.state.stack.is_empty());
    assert_eq!(
        e.state.players[1].life,
        17,
        "only the uncountered first bolt deals 3 damage"
    );
    assert_eq!(e.state.players[0].life, 20);
}

#[test]
fn opening_choose_first_london_mulligan_then_start() {
    use tricerules_proto::ruled::v1::ruled_command::Cmd;
    use tricerules_proto::ruled::v1::{
        ChooseStartingPlayer, MulliganDecision, PutOpeningHandOnBottom, RuledCommand,
    };
    // seed 100 → chooser is player_ids[0] == 5
    let mut e = GameEngine::new(100, &[5, 6], 20, None, false).expect("new");
    let chooser = e.state.opening.as_ref().expect("opening").chooser;
    assert_eq!(chooser, 5);
    e.apply_command(
        chooser,
        &RuledCommand {
            cmd: Some(Cmd::ChooseStartingPlayer(ChooseStartingPlayer {
                starting_player_id: 5,
            })),
        },
    )
    .expect("choose first");
    assert_eq!(e.state.players[0].hand.len(), 7);
    assert_eq!(e.state.players[1].hand.len(), 7);
    e.apply_command(
        5,
        &RuledCommand {
            cmd: Some(Cmd::Mulligan(MulliganDecision { keep: false })),
        },
    )
    .expect("mulligan");
    assert_eq!(e.state.opening.as_ref().unwrap().mulligans_taken[0], 1);
    assert_eq!(
        e.state.opening.as_ref().unwrap().mulligan_actor,
        Some(6),
        "after a mulligan, the other player is offered a decision while they have not kept"
    );
    e.apply_command(
        6,
        &RuledCommand {
            cmd: Some(Cmd::Mulligan(MulliganDecision { keep: true })),
        },
    )
    .expect("p6 keep (opponent locked in first)");
    assert!(e.state.opening.as_ref().unwrap().resolved[1]);
    assert_eq!(
        e.state.opening.as_ref().unwrap().mulligan_actor,
        Some(5),
        "once the opponent has kept, the mulliganing player acts again"
    );
    e.apply_command(
        5,
        &RuledCommand {
            cmd: Some(Cmd::Mulligan(MulliganDecision { keep: true })),
        },
    )
    .expect("keep to bottom");
    let hi = 0u32;
    e.apply_command(
        5,
        &RuledCommand {
            cmd: Some(Cmd::PutOpeningHandOnBottom(PutOpeningHandOnBottom {
                hand_card_index: hi,
            })),
        },
    )
    .expect("bottom one");
    assert!(e.state.opening.is_none());
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Upkeep);
}

#[test]
fn opening_mulligan_to_zero_auto_keeps_and_enters_bottom_phase() {
    use tricerules_proto::ruled::v1::ruled_command::Cmd;
    use tricerules_proto::ruled::v1::{
        ChooseStartingPlayer, MulliganDecision, PutOpeningHandOnBottom, RuledCommand,
    };
    let mut e = GameEngine::new(100, &[5, 6], 20, None, false).expect("new");
    let chooser = e.state.opening.as_ref().unwrap().chooser;
    e.apply_command(
        chooser,
        &RuledCommand {
            cmd: Some(Cmd::ChooseStartingPlayer(ChooseStartingPlayer {
                starting_player_id: 5,
            })),
        },
    )
    .expect("choose first");

    // P5 (starting player) mulligans first; P6 keeps on their turn; then P5 mulligans 6 more times.
    e.apply_command(
        5,
        &RuledCommand {
            cmd: Some(Cmd::Mulligan(MulliganDecision { keep: false })),
        },
    )
    .expect("p5 first mulligan");
    e.apply_command(
        6,
        &RuledCommand {
            cmd: Some(Cmd::Mulligan(MulliganDecision { keep: true })),
        },
    )
    .expect("p6 keep");
    // P5 mulligans 6 more times (7 total → auto-keep at 0).
    for _ in 0..6 {
        e.apply_command(
            5,
            &RuledCommand {
                cmd: Some(Cmd::Mulligan(MulliganDecision { keep: false })),
            },
        )
        .expect("mulligan");
    }

    // After the 7th mulligan the engine must auto-keep: bottom phase active, no more keep/mulligan.
    let op = e.state.opening.as_ref().expect("opening still active for bottom");
    assert_eq!(op.mulligans_taken[0], 7, "7 mulligans taken");
    assert!(op.bottom.is_some(), "bottom phase must be active");
    assert_eq!(op.bottom.unwrap().1, 7, "must place 7 cards on bottom");
    // mulligan_actor still points to P5 (they are bottoming).
    assert_eq!(op.mulligan_actor, Some(5));

    // P5 places all 7 cards on the bottom one by one.
    for _ in 0..7 {
        e.apply_command(
            5,
            &RuledCommand {
                cmd: Some(Cmd::PutOpeningHandOnBottom(PutOpeningHandOnBottom {
                    hand_card_index: 0,
                })),
            },
        )
        .expect("place on bottom");
    }

    // Opening complete; P5 has 0 cards in hand.
    assert!(e.state.opening.is_none(), "opening should be finished");
    assert_eq!(e.state.players[0].hand.len(), 0);
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Upkeep);
}

#[test]
fn opening_mulligan_to_zero_cannot_mulligan_further() {
    use tricerules_proto::ruled::v1::ruled_command::Cmd;
    use tricerules_proto::ruled::v1::{ChooseStartingPlayer, MulliganDecision, RuledCommand};
    let mut e = GameEngine::new(100, &[5, 6], 20, None, false).expect("new");
    let chooser = e.state.opening.as_ref().unwrap().chooser;
    e.apply_command(
        chooser,
        &RuledCommand {
            cmd: Some(Cmd::ChooseStartingPlayer(ChooseStartingPlayer {
                starting_player_id: 5,
            })),
        },
    )
    .expect("choose first");

    // P5 mulligans first, then P6 keeps, then P5 mulligans 6 more (7 total → auto-keep).
    e.apply_command(
        5,
        &RuledCommand {
            cmd: Some(Cmd::Mulligan(MulliganDecision { keep: false })),
        },
    )
    .expect("p5 first mulligan");
    e.apply_command(
        6,
        &RuledCommand {
            cmd: Some(Cmd::Mulligan(MulliganDecision { keep: true })),
        },
    )
    .expect("p6 keep");
    for _ in 0..6 {
        e.apply_command(
            5,
            &RuledCommand {
                cmd: Some(Cmd::Mulligan(MulliganDecision { keep: false })),
            },
        )
        .expect("mulligan");
    }

    // An 8th Mulligan { keep: false } must be rejected (bottom phase is active, not mulligan phase).
    let err = e.apply_command(
        5,
        &RuledCommand {
            cmd: Some(Cmd::Mulligan(MulliganDecision { keep: false })),
        },
    );
    assert!(err.is_err(), "must reject further mulligan when bottom phase is active");
}

fn assign_combat_damage_cmd(attacker_id: u32, pairs: Vec<(u32, u32)>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::AssignCombatDamage(AssignCombatDamage {
            attacker_id,
            assignments: pairs
                .into_iter()
                .map(|(blocker_id, damage)| DamagePair {
                    blocker_id,
                    damage,
                })
                .collect(),
        })),
    }
}

/// Ensure `card_id` is in the player's hand, pulling from library if needed.
fn ensure_in_hand(e: &mut GameEngine, player: usize, card_id: &str) {
    let in_hand = e.state.players[player]
        .hand
        .iter()
        .any(|oid| e.state.objects.get(oid).map(|o| o.card_id == card_id).unwrap_or(false));
    if !in_hand {
        take_card_from_library_to_hand(e, player, card_id);
    }
}

#[test]
fn two_blockers_damage_order_required_and_resolves() {
    // Attacker: grizzly_bears (2/2) = 2 power.
    // Blockers: savannah_lions (2/1) + grizzly_bears (2/2).
    // Assignment: lions 1, bears 1 (sum = attacker power).
    // Attacker receives 2+2=4 damage (toughness 2) → dies. No life loss.
    let decks = Some(vec![
        // P0: enough grizzly_bears to guarantee one in hand after draw step
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
        // P1: equal mix so both are available in library after opening draw
        {
            let mut d: Vec<String> = std::iter::repeat_n("savannah_lions".to_string(), 5).collect();
            d.extend(std::iter::repeat_n("grizzly_bears".to_string(), 5));
            d
        },
    ]);
    let mut e = GameEngine::new(901, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);
    ensure_in_hand(&mut e, 0, "grizzly_bears");
    ensure_in_hand(&mut e, 1, "savannah_lions");
    ensure_in_hand(&mut e, 1, "grizzly_bears");
    let attacker = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let blocker_lions = put_creature_on_battlefield(&mut e, 1, "savannah_lions");
    let blocker_bears = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("active pass declare attackers");
    e.apply_command(1, &pass()).expect("defender pass declare attackers");

    // Defender sends both blockers to the same attacker.
    let b = e
        .apply_command(
            1,
            &declare_blockers(vec![
                BlockPair { attacker_id: attacker, blocker_id: blocker_lions },
                BlockPair { attacker_id: attacker, blocker_id: blocker_bears },
            ]),
        )
        .expect("declare two blockers");

    assert!(
        e.state.combat.as_ref().unwrap().damage_assignment_needed,
        "damage_assignment_needed must be true after multi-block"
    );
    assert!(
        !e.state
            .combat
            .as_ref()
            .unwrap()
            .assign_combat_damage_phase,
        "still in declare blockers priority before passes"
    );
    assert!(life_changes_in(&b).is_empty(), "no damage dealt yet");

    assert!(
        e.apply_command(
            0,
            &assign_combat_damage_cmd(attacker, vec![(blocker_lions, 1), (blocker_bears, 1)]),
        )
        .is_err(),
        "cannot assign combat damage before declare-blockers priority round"
    );

    e.apply_command(0, &pass()).expect("active pass declare blockers");
    e.apply_command(1, &pass()).expect("defender pass → assign combat damage step");
    assert!(
        e.state.combat.as_ref().unwrap().assign_combat_damage_phase,
        "assign_combat_damage_phase after both pass"
    );

    let b3 = e
        .apply_command(
            0,
            &assign_combat_damage_cmd(attacker, vec![(blocker_lions, 1), (blocker_bears, 1)]),
        )
        .expect("assign combat damage");

    let dead = permanents_moved_in(&b3);
    let dead_ids: Vec<u32> = dead.iter().map(|p| p.object_id).collect();

    // Attacker (2/2) gets 2+2=4 total blocker damage → dies.
    assert!(dead_ids.contains(&attacker), "attacker dies: {dead_ids:?}");
    // Lions (2/1) gets 1 lethal damage first in order → dies.
    assert!(dead_ids.contains(&blocker_lions), "lions die: {dead_ids:?}");
    // Bears (2/2) gets remaining 1 damage (< toughness 2) → survives.
    assert!(!dead_ids.contains(&blocker_bears), "bears survive: {dead_ids:?}");
    let bears_obj = e.state.objects.get(&blocker_bears).expect("bears object");
    assert_eq!(bears_obj.damage, 1, "bears has 1 marked damage");
    assert_eq!(bears_obj.zone, tricerules_core::Zone::Battlefield);
    assert!(life_changes_in(&b3).is_empty(), "no life change on fully-blocked combat");
}

#[test]
fn two_blockers_insufficient_power_kills_only_first_in_order() {
    // Attacker: savannah_lions (2/1) = 2 power.
    // Blockers: coral_merfolk (2/1) + grizzly_bears (2/2).
    // merfolk 1 lethal, bears 1 partial.
    // Attacker receives 2+2=4 damage → dies. No life loss.
    let decks = Some(vec![
        {
            let mut d: Vec<String> = std::iter::repeat_n("savannah_lions".to_string(), 5).collect();
            d.extend(std::iter::repeat_n("grizzly_bears".to_string(), 5));
            d
        },
        {
            let mut d: Vec<String> = std::iter::repeat_n("coral_merfolk".to_string(), 5).collect();
            d.extend(std::iter::repeat_n("grizzly_bears".to_string(), 5));
            d
        },
    ]);
    let mut e = GameEngine::new(902, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);
    ensure_in_hand(&mut e, 0, "savannah_lions");
    ensure_in_hand(&mut e, 1, "coral_merfolk");
    ensure_in_hand(&mut e, 1, "grizzly_bears");
    let attacker = put_creature_on_battlefield(&mut e, 0, "savannah_lions");
    let blocker_merfolk = put_creature_on_battlefield(&mut e, 1, "coral_merfolk");
    let blocker_bears = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("active pass");
    e.apply_command(1, &pass()).expect("defender pass");

    e.apply_command(
        1,
        &declare_blockers(vec![
            BlockPair { attacker_id: attacker, blocker_id: blocker_merfolk },
            BlockPair { attacker_id: attacker, blocker_id: blocker_bears },
        ]),
    )
    .expect("two blockers");
    e.apply_command(0, &pass()).expect("active pass declare blockers");
    e.apply_command(1, &pass()).expect("defender pass → assign combat damage");
    let b = e
        .apply_command(
            0,
            &assign_combat_damage_cmd(attacker, vec![(blocker_merfolk, 1), (blocker_bears, 1)]),
        )
        .expect("assign combat damage");

    let dead = permanents_moved_in(&b);
    let dead_ids: Vec<u32> = dead.iter().map(|p| p.object_id).collect();

    // Attacker (2/1) gets 2+2=4 damage → dies.
    assert!(dead_ids.contains(&attacker), "lions attacker dies: {dead_ids:?}");
    // Merfolk (2/1) gets 1 lethal → dies.
    assert!(dead_ids.contains(&blocker_merfolk), "merfolk die: {dead_ids:?}");
    // Bears (2/2) gets remaining 1 damage (< toughness 2) → survives.
    assert!(!dead_ids.contains(&blocker_bears), "bears survive: {dead_ids:?}");
    assert!(life_changes_in(&b).is_empty(), "no life change (fully blocked)");
}

#[test]
fn single_blocker_no_damage_order_needed() {
    // Regression: single blocker must not trigger damage_assignment_needed; combat proceeds normally.
    let decks = Some(vec![
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
    ]);
    let mut e = GameEngine::new(903, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let blocker = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("active pass");
    e.apply_command(1, &pass()).expect("defender pass");

    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair { attacker_id: attacker, blocker_id: blocker }]),
    )
    .expect("declare single blocker");

    assert!(
        !e.state.combat.as_ref().unwrap().damage_assignment_needed,
        "damage_assignment_needed must be false for single-blocker combat"
    );

    // Combat resolves normally without any AssignCombatDamage step: both 2/2s die.
    e.apply_command(0, &pass()).expect("active pass declare blockers");
    let b = e.apply_command(1, &pass()).expect("combat damage");
    let dead = permanents_moved_in(&b);
    let dead_ids: Vec<u32> = dead.iter().map(|p| p.object_id).collect();
    assert!(dead_ids.contains(&attacker), "attacker dies in mutual block");
    assert!(dead_ids.contains(&blocker), "blocker dies in mutual block");
    assert!(life_changes_in(&b).is_empty(), "no life loss on fully blocked combat");
}

/// Two blockers on one 2-power attacker, both passes done → assign_combat_damage_phase.
fn setup_two_blockers_assign_phase(
    seed: u64,
) -> (
    GameEngine,
    u32, // attacker
    u32, // blocker_a (savannah_lions)
    u32, // blocker_b (grizzly_bears)
) {
    let decks = Some(vec![
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
        {
            let mut d: Vec<String> = std::iter::repeat_n("savannah_lions".to_string(), 5).collect();
            d.extend(std::iter::repeat_n("grizzly_bears".to_string(), 5));
            d
        },
    ]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);
    ensure_in_hand(&mut e, 0, "grizzly_bears");
    ensure_in_hand(&mut e, 1, "savannah_lions");
    ensure_in_hand(&mut e, 1, "grizzly_bears");
    let attacker = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let blocker_a = put_creature_on_battlefield(&mut e, 1, "savannah_lions");
    let blocker_b = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("active pass");
    e.apply_command(1, &pass()).expect("defender pass");
    e.apply_command(
        1,
        &declare_blockers(vec![
            BlockPair {
                attacker_id: attacker,
                blocker_id: blocker_a,
            },
            BlockPair {
                attacker_id: attacker,
                blocker_id: blocker_b,
            },
        ]),
    )
    .expect("declare two blockers");
    e.apply_command(0, &pass()).expect("active pass declare blockers");
    e.apply_command(1, &pass()).expect("defender pass");
    assert!(e.state.combat.as_ref().unwrap().assign_combat_damage_phase);
    (e, attacker, blocker_a, blocker_b)
}

#[test]
fn assign_combat_damage_rejects_sum_mismatch() {
    let (mut e, attacker, a, b) = setup_two_blockers_assign_phase(910);
    assert!(e
        .apply_command(
            0,
            &assign_combat_damage_cmd(attacker, vec![(a, 1), (b, 0)]),
        )
        .is_err());
    assert!(!e.state.combat.as_ref().unwrap().damage_assignments.contains_key(&attacker));
}

#[test]
fn assign_combat_damage_accepts_split_with_two_nonlethal_hits() {
    // Two 2/2 blockers vs 2-power attacker: 1+1 is allowed (no lethal-first requirement).
    let decks = Some(vec![
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
    ]);
    let mut e = GameEngine::new(911, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);
    ensure_in_hand(&mut e, 0, "grizzly_bears");
    ensure_in_hand(&mut e, 1, "grizzly_bears");
    let attacker = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let b1 = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let b2 = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("active pass");
    e.apply_command(1, &pass()).expect("defender pass");
    e.apply_command(
        1,
        &declare_blockers(vec![
            BlockPair {
                attacker_id: attacker,
                blocker_id: b1,
            },
            BlockPair {
                attacker_id: attacker,
                blocker_id: b2,
            },
        ]),
    )
    .expect("declare two blockers");
    e.apply_command(0, &pass()).expect("active pass declare blockers");
    e.apply_command(1, &pass()).expect("defender pass");
    let b = e
        .apply_command(0, &assign_combat_damage_cmd(attacker, vec![(b1, 1), (b2, 1)]))
        .expect("assign 1+1");
    let dead = permanents_moved_in(&b);
    let dead_ids: Vec<u32> = dead.iter().map(|p| p.object_id).collect();
    assert!(dead_ids.contains(&attacker), "attacker dies from 2+2 blocker damage");
    assert!(!dead_ids.contains(&b1) && !dead_ids.contains(&b2), "both blockers survive with 1 dmg");
    assert_eq!(e.state.objects.get(&b1).unwrap().damage, 1);
    assert_eq!(e.state.objects.get(&b2).unwrap().damage, 1);
}

#[test]
fn assign_combat_damage_rejects_wrong_blocker_set() {
    let (mut e, attacker, a, _b) = setup_two_blockers_assign_phase(912);
    let other = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    assert!(e
        .apply_command(
            0,
            &assign_combat_damage_cmd(attacker, vec![(a, 1), (other, 1)]),
        )
        .is_err());
}

#[test]
fn assign_combat_damage_rejects_defender_player() {
    let (mut e, attacker, a, b) = setup_two_blockers_assign_phase(913);
    assert!(e
        .apply_command(
            1,
            &assign_combat_damage_cmd(attacker, vec![(a, 1), (b, 1)]),
        )
        .is_err());
}

// ── Combat eligibility skip tests ────────────────────────────────────────────

#[test]
fn begin_combat_skips_when_no_eligible_attackers() {
    // Default deck has no creatures on the battlefield.
    // BeginCombat must auto-skip directly to EndCombat.
    let mut e = GameEngine::new(4001, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin_combat");
    e.apply_command(0, &pass()).expect("ap pass begin_combat");
    let b = e
        .apply_command(1, &pass())
        .expect("nap pass begin_combat");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::EndCombat,
        "no eligible attackers must skip to end_combat"
    );
    assert!(
        priority_changes_in(&b).contains(&0),
        "active player must hold priority in end_combat after auto-skip"
    );
}

#[test]
fn begin_combat_skips_when_all_creatures_summoning_sick() {
    let mut e = GameEngine::new(4002, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin_combat");
    // Inject a summoning-sick creature (cannot attack).
    let oid = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    if let Some(obj) = e.state.objects.get_mut(&oid) {
        obj.summoning_sick = true;
    }
    e.apply_command(0, &pass()).expect("ap pass begin_combat");
    e.apply_command(1, &pass()).expect("nap pass begin_combat");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::EndCombat,
        "summoning-sick creature must not prevent skip to end_combat"
    );
}

#[test]
fn begin_combat_skips_when_all_creatures_tapped() {
    let mut e = GameEngine::new(4003, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin_combat");
    // Inject a tapped creature (cannot attack).
    let oid = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    if let Some(obj) = e.state.objects.get_mut(&oid) {
        obj.tapped = true;
    }
    e.apply_command(0, &pass()).expect("ap pass begin_combat");
    e.apply_command(1, &pass()).expect("nap pass begin_combat");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::EndCombat,
        "tapped creature must not prevent skip to end_combat"
    );
}

#[test]
fn begin_combat_enters_declare_attackers_when_eligible_attacker_exists() {
    let mut e = GameEngine::new(4004, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin_combat");
    inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.apply_command(0, &pass()).expect("ap pass begin_combat");
    e.apply_command(1, &pass()).expect("nap pass begin_combat");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers,
        "eligible attacker must cause engine to enter declare_attackers"
    );
}

#[test]
fn declare_attackers_skips_blockers_when_no_eligible_blockers() {
    // Active player has an attacker; defending player has no creatures.
    // After both pass priority in DeclareAttackers, engine auto-declares empty blockers.
    let mut e = GameEngine::new(4005, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin_combat");
    let bears = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.apply_command(0, &pass()).expect("ap pass begin_combat");
    e.apply_command(1, &pass()).expect("nap pass begin_combat");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::DeclareAttackers);

    e.apply_command(0, &declare_attackers(vec![bears]))
        .expect("declare attacker");
    // Both pass in DeclareAttackers.
    e.apply_command(0, &pass()).expect("ap pass declare_attackers");
    let b = e
        .apply_command(1, &pass())
        .expect("nap pass declare_attackers");
    // Engine lands in DeclareBlockers with blockers_declared = true and active player holding priority.
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::DeclareBlockers);
    assert!(
        priority_changes_in(&b).contains(&0),
        "active player must hold priority when blockers auto-declared"
    );
    assert!(
        e.state.combat.as_ref().map_or(false, |c| c.blockers_declared),
        "blockers_declared must be true after auto-skip"
    );
}

#[test]
fn summoning_sick_creature_can_block() {
    // CR 302.6: summoning sickness does NOT prevent blocking.
    // Defender has a summoning-sick but untapped creature → engine must enter DeclareBlockers
    // with the defender holding priority.
    let mut e = GameEngine::new(4006, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin_combat");
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    // Defender's creature is summoning-sick but untapped → eligible blocker.
    let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    if let Some(obj) = e.state.objects.get_mut(&blocker) {
        obj.summoning_sick = true;
        obj.tapped = false;
    }
    e.apply_command(0, &pass()).expect("ap pass begin_combat");
    e.apply_command(1, &pass()).expect("nap pass begin_combat");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::DeclareAttackers);

    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("ap pass declare_attackers");
    let b = e
        .apply_command(1, &pass())
        .expect("nap pass declare_attackers");
    // Defender has an eligible (summoning-sick) blocker → must get priority in DeclareBlockers.
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::DeclareBlockers);
    assert!(
        priority_changes_in(&b).contains(&1),
        "defender must hold priority in declare_blockers when they have a summoning-sick blocker"
    );
}
