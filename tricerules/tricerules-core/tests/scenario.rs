//! Scripted command sequences (M2).

use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::ruled_event::Ev;
use tricerules_proto::ruled::v1::{
    ActivateAbility, AddManaToPool, AssignCombatDamage, BlockPair, CastSpell, ChooseTriggerTarget,
    DamagePair, DeclareAttackers, DeclareBlockers, DiscardToHandSize, PassPriority, PlayLand,
    PreviewDeclareAttackers, PreviewDeclareBlockers, PrimitiveYieldStructured, RuledCommand,
    TargetRef,
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

fn activate_ability(
    permanent_id: u32,
    ability_index: u32,
    targets: Vec<TargetRef>,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            permanent_id,
            ability_index,
            targets,
        })),
    }
}

/// Place a card from `player`'s library directly onto the battlefield (untapped, not
/// summoning-sick unless `tapped`), returning its object id. Used to set up board states that
/// would otherwise take many turns of legal play to reach.
fn deploy_to_battlefield(e: &mut GameEngine, player: usize, card_id: &str, tapped: bool) -> u32 {
    let pos = e.state.players[player]
        .library
        .iter()
        .position(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some(card_id))
        .unwrap_or_else(|| panic!("missing card {card_id} in P{player} library"));
    let oid = e.state.players[player]
        .library
        .remove(pos)
        .expect("index from position()");
    e.state.players[player].battlefield.push(oid);
    let obj = e.state.objects.get_mut(&oid).expect("object");
    obj.zone = tricerules_core::Zone::Battlefield;
    obj.tapped = tapped;
    obj.summoning_sick = false;
    oid
}

/// Player targets for `DamageTarget` spells use `TargetRef.object_id == player_id` (see engine).
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
        .filter(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some(card_id))
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

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana");
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
    // Spells carry their engine card id so the relay can bind the physical stack card
    // through the CardCatalog instead of guessing from the display description.
    assert_eq!(stack_push.card_id, "lightning_bolt");

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
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana");
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
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana");
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
    {
        let idx = hand_index_for_card(&e, 1, "grizzly_bears");
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
        e.state.combat.as_ref().is_some_and(|c| c.blockers_declared),
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

/// Regression test: combat damage triggers must land on the stack and require both players to
/// pass priority before resolving.  The fix is that PhaseChanged is emitted before StackPushed
/// so the C++ client doesn't clear ruledStackObjectIds and mistakenly auto-passes.
#[test]
fn combat_damage_trigger_lands_on_stack_and_requires_priority() {
    // 10-card decks: skip_opening draws 7 as opening hand, leaving 3 in library
    // so the Scroll Thief trigger can draw without hitting an empty library.
    let p0_deck: Vec<String> = vec![
        "island".into(),
        "island".into(),
        "scroll_thief".into(),
        "island".into(),
        "island".into(),
        "island".into(),
        "island".into(),
        "island".into(),
        "island".into(),
        "island".into(),
    ];
    let p1_deck: Vec<String> = std::iter::repeat_n("mountain".into(), 10).collect();
    let decks = Some(vec![p0_deck, p1_deck]);
    let mut e = GameEngine::new(77, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Cheat Scroll Thief onto P0's battlefield without summoning sickness.
    let thief_hand_idx = hand_index_for_card(&e, 0, "scroll_thief");
    let thief_oid = e.state.players[0].hand.remove(thief_hand_idx);
    e.state.players[0].battlefield.push(thief_oid);
    if let Some(obj) = e.state.objects.get_mut(&thief_oid) {
        obj.zone = tricerules_core::Zone::Battlefield;
        obj.summoning_sick = false;
    }

    let p0_hand_before = e.state.players[0].hand.len();

    // Enter combat and attack with Scroll Thief.
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap begin combat pass");
    e.apply_command(1, &pass()).expect("nap begin combat pass");
    e.apply_command(0, &declare_attackers(vec![thief_oid]))
        .expect("declare attackers");
    e.apply_command(0, &pass())
        .expect("ap pass declare attackers");
    e.apply_command(1, &pass())
        .expect("nap pass declare attackers");
    // P1 has no blockers; engine auto-declares empty blockers.
    e.apply_command(0, &pass())
        .expect("ap pass declare blockers");
    let b = e
        .apply_command(1, &pass())
        .expect("nap pass declare blockers -> combat damage");

    // After both players pass in DeclareBlockers, combat damage resolves and Scroll Thief's
    // trigger fires.  The trigger must be on the stack awaiting priority.
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::CombatDamage,
        "should be in CombatDamage step"
    );
    assert_eq!(
        e.state.stack.len(),
        1,
        "combat damage trigger should be on the stack"
    );

    // Verify that in the returned batch PhaseChanged arrives before StackPushed.
    // This is the ordering fix: C++ clears its ruledStackObjectIds on PhaseChanged; the
    // trigger must follow so it is visible when the auto-pass timer fires.
    let event_order: Vec<&str> = b
        .events
        .iter()
        .filter_map(|ev| match &ev.ev {
            Some(Ev::PhaseChanged(_)) => Some("phase"),
            Some(Ev::StackPushed(_)) => Some("stack_pushed"),
            _ => None,
        })
        .collect();
    let phase_pos = event_order.iter().position(|&e| e == "phase");
    let pushed_pos = event_order.iter().position(|&e| e == "stack_pushed");
    assert!(
        phase_pos.is_some() && pushed_pos.is_some(),
        "batch must contain both PhaseChanged and StackPushed"
    );
    assert!(
        phase_pos.unwrap() < pushed_pos.unwrap(),
        "PhaseChanged must precede StackPushed in the combat damage batch"
    );

    // Abilities have no physical card on the stack: StackPushed.card_id stays empty
    // (spells carry their engine card id for catalog-based relay binding).
    let trigger_push = b
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::StackPushed(s)) => Some(s),
            _ => None,
        })
        .expect("trigger StackPushed");
    assert!(trigger_push.card_id.is_empty());

    // Both players passing resolves the trigger; Scroll Thief draws a card for P0.
    pass_both_players(&mut e);
    assert!(
        e.state.stack.is_empty(),
        "stack should be empty after trigger resolves"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        p0_hand_before + 1,
        "Scroll Thief trigger should have drawn a card for P0"
    );
}

#[test]
fn cleanup_batch_discard_three_at_once() {
    let mut e = GameEngine::new(1002, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let ap_idx = e.state.player_idx(0).unwrap();
    for _ in 0..3 {
        let oid = e.state.players[ap_idx]
            .library
            .pop_front()
            .expect("library");
        e.state.players[ap_idx].hand.push(oid);
        e.state.objects.get_mut(&oid).expect("obj").zone = tricerules_core::Zone::Hand;
    }
    assert_eq!(e.state.players[ap_idx].hand.len(), 10);

    e.apply_command(0, &primitive_yield())
        .expect("main1->begin combat");
    // No eligible attackers: BeginCombat auto-skips to EndCombat.
    e.apply_command(0, &primitive_yield())
        .expect("begin combat->end combat");
    e.apply_command(0, &primitive_yield())
        .expect("end combat->main2");
    e.apply_command(0, &primitive_yield())
        .expect("main2->end step");
    e.apply_command(0, &primitive_yield())
        .expect("end step->cleanup");
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
    let oid = e.state.players[ap_idx]
        .library
        .pop_front()
        .expect("library");
    e.state.players[ap_idx].hand.push(oid);
    e.state.objects.get_mut(&oid).expect("obj").zone = tricerules_core::Zone::Hand;
    assert!(e.state.players[ap_idx].hand.len() > 7);

    e.apply_command(0, &primitive_yield())
        .expect("main1->begin combat");
    // No eligible attackers: BeginCombat auto-skips to EndCombat.
    e.apply_command(0, &primitive_yield())
        .expect("begin combat->end combat");
    e.apply_command(0, &primitive_yield())
        .expect("end combat->main2");
    e.apply_command(0, &primitive_yield())
        .expect("main2->end step");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::EndStep);

    e.apply_command(0, &primitive_yield())
        .expect("end step->cleanup");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Cleanup);
    assert_eq!(e.state.cleanup_discard_player, Some(0));

    e.apply_command(0, &discard_cleanup(0))
        .expect("discard one");
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
    e.apply_command(0, &cast_spell(merfolk_idx, vec![]))
        .expect("cast");
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

    // Tap both forests to pay for grizzly bears (simulating player tapping lands for mana).
    for &oid in &e.state.players[0].battlefield.clone() {
        if e.state.objects.get(&oid).map(|o| o.card_id.as_str()) == Some("forest") {
            e.state.objects.get_mut(&oid).expect("forest").tapped = true;
        }
    }
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            g: 1,
            c: 1,
            ..Default::default()
        }),
    )
    .expect("add mana for 1G");
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

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for first bolt");
    let bolt_one = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_one, target_player(1)))
        .expect("cast first bolt");
    assert_eq!(
        e.state.priority_player_id(),
        0,
        "caster should keep priority after casting first spell"
    );

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for second bolt");
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

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for bolt");
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, target_player(1)))
        .expect("p0 cast bolt");
    let bolt_oid = e.state.stack.last().expect("bolt on stack").id;
    e.apply_command(0, &pass())
        .expect("p0 pass to give p1 priority");

    // Manually tap an island (simulates client-side land tap for mana).
    e.state
        .objects
        .get_mut(&p1_island_a)
        .expect("p1 island")
        .tapped = true;
    e.apply_command(
        1,
        &add_mana_to_pool(AddManaToPool {
            u: 2,
            ..Default::default()
        }),
    )
    .expect("add UU for counterspell");
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
    .expect("NAP with priority should cast counterspell");
    assert!(
        e.state.objects.get(&p1_island_a).expect("p1 island").tapped,
        "an island should tap to help pay UU"
    );
    assert_eq!(e.state.stack.len(), 2, "bolt and counterspell on stack");

    e.apply_command(1, &pass())
        .expect("p1 pass after casting counter");
    e.apply_command(0, &pass())
        .expect("p0 pass resolves counterspell");
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

// Regression: a spell countered by the *opponent* must go to its OWNER's graveyard (CR 701.6a),
// not the counterer's. The engine emits a PermanentMoved stamped with the countered spell's owner
// so the relay can route the physical card off the shared stack to the right player — without any
// per-card name special-case. Here P0 owns the bolt and P1 counters it.
#[test]
fn countered_spell_moves_to_its_owners_graveyard() {
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

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for bolt");
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, target_player(1)))
        .expect("p0 cast bolt");
    let bolt_oid = e.state.stack.last().expect("bolt on stack").id;
    e.apply_command(0, &pass())
        .expect("p0 pass to give p1 priority");

    e.apply_command(
        1,
        &add_mana_to_pool(AddManaToPool {
            u: 2,
            ..Default::default()
        }),
    )
    .expect("add UU for counterspell");
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
    .expect("p1 cast counterspell at bolt");
    let counterspell_oid = e.state.stack.last().expect("counterspell on stack").id;

    e.apply_command(1, &pass()).expect("p1 pass");
    let resolve_batch = e
        .apply_command(0, &pass())
        .expect("p0 pass resolves counter");

    // The decisive assertion: the engine routes the countered bolt to its OWNER (P0).
    let bolt_move = permanents_moved_in(&resolve_batch)
        .into_iter()
        .find(|pm| pm.object_id == bolt_oid)
        .expect("counter must emit a PermanentMoved for the bolt");
    assert_eq!(
        bolt_move.owner_player_id, 0,
        "countered bolt must route to its owner P0, not the counterer P1"
    );
    assert_eq!(
        bolt_move.destination,
        tricerules_proto::ruled::v1::permanent_moved::Destination::Graveyard as i32
    );

    assert!(e.state.stack.is_empty(), "counter clears the stack");
    assert!(
        e.state.players[0].graveyard.contains(&bolt_oid),
        "bolt in its owner P0's graveyard"
    );
    assert!(
        !e.state.players[1].graveyard.contains(&bolt_oid),
        "bolt must NOT be in counterer P1's graveyard"
    );
    assert!(
        e.state.players[1].graveyard.contains(&counterspell_oid),
        "counterspell in its owner P1's graveyard"
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
    // Tap the mountain to produce mana (simulating the client tapping land before casting).
    e.state
        .objects
        .get_mut(&mountain_oid)
        .expect("mountain object")
        .tapped = true;
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana");
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
            deathtouch_damage: false,
            counters: std::collections::BTreeMap::new(),
        },
    );
    e.state.players[player].battlefield.push(id);
    id
}

/// Inject a card directly onto the bottom of a player's library (so e.g. a draw effect has
/// something to draw when the opening hand consumed the whole deck). Returns its object id.
fn inject_library_card(e: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let id = e.state.next_object_id;
    e.state.next_object_id += 1;
    let player_id = e.state.players[player].id;
    e.state.objects.insert(
        id,
        tricerules_core::state::GameObject {
            id,
            owner: player_id,
            card_id: card_id.to_string(),
            zone: tricerules_core::Zone::Library,
            tapped: false,
            summoning_sick: false,
            power: None,
            toughness: None,
            damage: 0,
            deathtouch_damage: false,
            counters: std::collections::BTreeMap::new(),
        },
    );
    e.state.players[player].library.push_back(id);
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
    let err = e
        .apply_command(0, &cmd)
        .expect_err("preview must not apply");
    assert!(err.to_string().contains("preview"), "unexpected err: {err}");
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
    let err = e
        .apply_command(0, &cmd)
        .expect_err("preview must not apply");
    assert!(err.to_string().contains("preview"), "unexpected err: {err}");
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
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            u: 1,
            c: 2,
            ..Default::default()
        }),
    )
    .expect("add mana for 2U");
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
    while e.state.players[0]
        .hand
        .iter()
        .filter(|oid| e.state.objects.get(*oid).map(|o| o.card_id.as_str()) == Some("divination"))
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
    let decks = Some(vec![vec!["mountain".into(); 10], vec!["forest".into(); 10]]);
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

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            g: 1,
            ..Default::default()
        }),
    )
    .expect("add green mana for giant growth");
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
    e.state.objects.get_mut(&forest_oid).expect("forest").zone = tricerules_core::Zone::Battlefield;

    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    let mountain_oid = e.state.players[0].hand.remove(mountain_idx);
    e.state.players[0].battlefield.push(mountain_oid);
    e.state
        .objects
        .get_mut(&mountain_oid)
        .expect("mountain")
        .zone = tricerules_core::Zone::Battlefield;

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            g: 1,
            ..Default::default()
        }),
    )
    .expect("add green mana for giant growth");
    let growth_idx = hand_index_for_card(&e, 0, "giant_growth");
    e.apply_command(
        0,
        &cast_spell(growth_idx, vec![TargetRef { object_id: bear }]),
    )
    .expect("cast growth");

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for lightning bolt");
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
    assert!(!saw_pump_log, "fizzled pump spell must not log +3/+3 line");
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
        e.state.objects.get_mut(&oid).expect("mountain").zone = tricerules_core::Zone::Battlefield;
    }

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for first bolt");
    let bolt_a = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_a, vec![TargetRef { object_id: bear }]))
        .expect("first bolt");
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for second bolt");
    let bolt_b = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_b, vec![TargetRef { object_id: bear }]))
        .expect("second bolt on top");

    resolve_entire_stack_two_player(&mut e);

    let dead = e.state.objects.get(&bear).expect("bear");
    assert_eq!(dead.zone, tricerules_core::Zone::Graveyard);
    assert_eq!(
        dead.damage, 3,
        "only the first resolving bolt should deal damage"
    );
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

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            b: 1,
            c: 1,
            ..Default::default()
        }),
    )
    .expect("add mana for 1B");
    let gfth_idx = hand_index_for_card(&e, 0, "go_for_the_throat");
    e.apply_command(
        0,
        &cast_spell(gfth_idx, vec![TargetRef { object_id: bear }]),
    )
    .expect("go for the throat");

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for bolt");
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

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for bolt");
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, target_player(1)))
        .expect("bolt");
    e.apply_command(0, &pass())
        .expect("AP pass so NAP can respond");

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
        e.state.objects.get_mut(&oid).expect("island").zone = tricerules_core::Zone::Battlefield;
    }

    e.apply_command(
        1,
        &add_mana_to_pool(AddManaToPool {
            u: 2,
            ..Default::default()
        }),
    )
    .expect("add UU for first counterspell");
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

    e.apply_command(
        1,
        &add_mana_to_pool(AddManaToPool {
            u: 2,
            ..Default::default()
        }),
    )
    .expect("add UU for second counterspell");
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
        fizzle_logs += batch
            .events
            .iter()
            .filter(|ev| matches!(&ev.ev, Some(Ev::Log(l)) if l.text.contains("fizzles")))
            .count();
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

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            g: 1,
            ..Default::default()
        }),
    )
    .expect("add green mana for giant growth");
    let growth_idx = hand_index_for_card(&e, 0, "giant_growth");
    e.apply_command(
        0,
        &cast_spell(growth_idx, vec![TargetRef { object_id: bear }]),
    )
    .expect("cast growth");
    pass_both_players(&mut e);

    assert_eq!(
        e.effective_power(bear),
        Some(5),
        "pumped bear should have 5 effective power"
    );
    assert_eq!(
        e.effective_toughness(bear),
        Some(5),
        "pumped bear should have 5 effective toughness"
    );

    end_active_turn(&mut e, 0);

    assert_eq!(
        e.effective_power(bear),
        Some(2),
        "Giant Growth should expire at end of turn"
    );
    assert_eq!(e.effective_toughness(bear), Some(2));
}

/// Two Giant Growths on the same creature stack: effective P/T = base + both deltas.
#[test]
fn two_giant_growths_stack_correctly() {
    let decks = Some(vec![
        vec![
            "forest".into(),
            "forest".into(),
            "giant_growth".into(),
            "giant_growth".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(9050, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    // Tap two forests and cast both Giant Growths.
    for _ in 0..2 {
        let forest_idx = hand_index_for_card(&e, 0, "forest");
        let foid = e.state.players[0].hand.remove(forest_idx);
        e.state.players[0].battlefield.push(foid);
        e.state.objects.get_mut(&foid).expect("forest").zone = tricerules_core::Zone::Battlefield;
        e.apply_command(
            0,
            &add_mana_to_pool(AddManaToPool {
                g: 1,
                ..Default::default()
            }),
        )
        .expect("add green mana");
        let growth_idx = hand_index_for_card(&e, 0, "giant_growth");
        e.apply_command(
            0,
            &cast_spell(growth_idx, vec![TargetRef { object_id: bear }]),
        )
        .expect("cast growth");
        pass_both_players(&mut e);
    }

    assert_eq!(
        e.effective_power(bear),
        Some(8),
        "two Giant Growths should give +6/+6 total"
    );
    assert_eq!(e.effective_toughness(bear), Some(8));
    assert_eq!(
        e.state.continuous_effects.len(),
        2,
        "two active ContinuousEffects expected"
    );

    end_active_turn(&mut e, 0);

    assert_eq!(e.effective_power(bear), Some(2), "pump expires at cleanup");
    assert_eq!(e.effective_toughness(bear), Some(2));
    assert!(
        e.state.continuous_effects.is_empty(),
        "continuous_effects must be empty after cleanup"
    );
}

/// CR 122 + CR 613.4 layer 7d: a +1/+1 counter from Battlegrowth raises a creature's P/T, and
/// unlike a Giant Growth pump it persists past the end of the turn (counters are not
/// until-end-of-turn continuous effects).
#[test]
fn battlegrowth_counter_raises_pt_and_persists() {
    use tricerules_cards::CounterKind;
    let decks = Some(vec![
        vec![
            "battlegrowth".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(1221, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            g: 1,
            ..Default::default()
        }),
    )
    .expect("add green mana");
    let idx = hand_index_for_card(&e, 0, "battlegrowth");
    e.apply_command(0, &cast_spell(idx, vec![TargetRef { object_id: bear }]))
        .expect("cast battlegrowth");
    pass_both_players(&mut e);

    assert_eq!(e.effective_power(bear), Some(3), "2/2 + one +1/+1 counter");
    assert_eq!(e.effective_toughness(bear), Some(3));
    assert_eq!(
        e.state
            .objects
            .get(&bear)
            .unwrap()
            .counter_count(CounterKind::PlusOnePlusOne),
        1
    );

    end_active_turn(&mut e, 0);

    assert_eq!(
        e.effective_power(bear),
        Some(3),
        "counter persists past end of turn (not a continuous effect)"
    );
    assert_eq!(e.effective_toughness(bear), Some(3));
    assert_eq!(
        e.state
            .objects
            .get(&bear)
            .unwrap()
            .counter_count(CounterKind::PlusOnePlusOne),
        1,
        "counter survives cleanup"
    );
}

/// CR 122.3: when a creature has both +1/+1 and -1/-1 counters, equal numbers annihilate as a
/// state-based action. Battlegrowth (+1/+1) then Instill Infection (-1/-1) net back to base P/T.
#[test]
fn plus_and_minus_counters_annihilate() {
    let decks = Some(vec![
        vec![
            "battlegrowth".into(),
            "instill_infection".into(),
            "grizzly_bears".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(1222, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bear = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");

    // Battlegrowth: +1/+1 counter -> 3/3.
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            g: 1,
            ..Default::default()
        }),
    )
    .expect("green mana");
    let bg = hand_index_for_card(&e, 0, "battlegrowth");
    e.apply_command(0, &cast_spell(bg, vec![TargetRef { object_id: bear }]))
        .expect("cast battlegrowth");
    pass_both_players(&mut e);
    assert_eq!(e.effective_toughness(bear), Some(3));

    // Instill Infection also draws a card; give the (opening-hand-emptied) library something.
    inject_library_card(&mut e, 0, "forest");
    // Instill Infection: -1/-1 counter; the SBA annihilates the +1/+1/-1/-1 pair -> back to 2/2.
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            b: 1,
            c: 3,
            ..Default::default()
        }),
    )
    .expect("black + generic mana");
    let ii = hand_index_for_card(&e, 0, "instill_infection");
    e.apply_command(0, &cast_spell(ii, vec![TargetRef { object_id: bear }]))
        .expect("cast instill infection");
    pass_both_players(&mut e);

    assert_eq!(
        e.effective_power(bear),
        Some(2),
        "counters annihilated to net 0"
    );
    assert_eq!(e.effective_toughness(bear), Some(2));
    assert!(
        e.state.objects.get(&bear).unwrap().counters.is_empty(),
        "no counters remain after annihilation"
    );
}

/// CR 704.5f via CR 122: a -1/-1 counter dropping a 1/1's toughness to 0 kills it as an SBA.
#[test]
fn minus_counter_to_zero_toughness_kills_via_sba() {
    let decks = Some(vec![
        vec![
            "prodigal_sorcerer".into(),
            "instill_infection".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(1223, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Prodigal Sorcerer is a 1/1.
    let sorc = put_creature_on_battlefield(&mut e, 0, "prodigal_sorcerer");
    assert_eq!(e.effective_toughness(sorc), Some(1));

    // Instill Infection also draws a card; give the (opening-hand-emptied) library something.
    inject_library_card(&mut e, 0, "swamp");

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            b: 1,
            c: 3,
            ..Default::default()
        }),
    )
    .expect("black + generic mana");
    let ii = hand_index_for_card(&e, 0, "instill_infection");
    e.apply_command(0, &cast_spell(ii, vec![TargetRef { object_id: sorc }]))
        .expect("cast instill infection");
    pass_both_players(&mut e);

    assert!(
        !e.state.players[0].battlefield.contains(&sorc),
        "0-toughness creature left the battlefield"
    );
    assert!(
        e.state.players[0].graveyard.contains(&sorc),
        "dead creature is in its owner's graveyard"
    );
}

#[test]
fn marked_damage_clears_at_cleanup() {
    let decks = Some(vec![
        {
            let mut d = vec![
                "forest".into(),
                "giant_growth".into(),
                "grizzly_bears".into(),
            ];
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

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            g: 1,
            ..Default::default()
        }),
    )
    .expect("add green mana for giant growth");
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
        e.state
            .objects
            .get(&bear)
            .expect("bear after cleanup")
            .damage,
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

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for bolt");
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, target_player(1)))
        .expect("cast bolt");
    let bolt_oid = e.state.stack.last().expect("bolt on stack").id;

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            u: 2,
            ..Default::default()
        }),
    )
    .expect("add UU for counterspell");
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

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            b: 1,
            c: 1,
            ..Default::default()
        }),
    )
    .expect("add mana for 1B");
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
fn go_for_the_throat_rejects_artifact_creature_target() {
    // Go for the Throat can't target artifact creatures (not_artifact: true filter).
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
            "plains".into(),
            "ornithopter".into(), // artifact creature
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
    ]);
    let mut e = GameEngine::new(3001, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Seed Ornithopter directly onto P1's battlefield (bypasses priority).
    let ornithopter_oid = put_creature_on_battlefield(&mut e, 1, "ornithopter");

    // Seed a swamp for P0 and play a land for the second mana.
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
        .expect("play swamp");

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            b: 1,
            c: 1,
            ..Default::default()
        }),
    )
    .expect("add mana for 1B");
    let gftt_idx = hand_index_for_card(&e, 0, "go_for_the_throat");
    let err = e
        .apply_command(
            0,
            &cast_spell(
                gftt_idx,
                vec![TargetRef {
                    object_id: ornithopter_oid,
                }],
            ),
        )
        .expect_err("go for the throat cannot target artifact creature");
    assert!(
        err.to_string().contains("creature") || err.to_string().contains("illegal"),
        "unexpected: {err}"
    );
    // Ornithopter must still be on the battlefield.
    assert!(e.state.players[1].battlefield.contains(&ornithopter_oid));
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

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            b: 1,
            c: 1,
            ..Default::default()
        }),
    )
    .expect("add mana for 1B");
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

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for bolt");
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
    e.apply_command(
        1,
        &add_mana_to_pool(AddManaToPool {
            g: 1,
            ..Default::default()
        }),
    )
    .expect("add green mana for giant growth");
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

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for p0 first bolt");
    let bolt_p0_first = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_p0_first, target_player(1)))
        .expect("p0 first bolt");
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for p0 second bolt");
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
    e.apply_command(
        1,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for p1 bolt");
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
    assert_eq!(
        e.state.players[0].life, 17,
        "NAP bolt resolves first (3 to P0)"
    );
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

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for p0 first bolt");
    let b0 = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(b0, target_player(1)))
        .expect("p0 first bolt");
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for p0 second bolt");
    let b1 = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(b1, target_player(1)))
        .expect("p0 second bolt");
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for p0 third bolt");
    let b2 = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(b2, target_player(1)))
        .expect("p0 third bolt");
    assert_eq!(
        e.state.stack.len(),
        3,
        "AP should stack three bolts before passing"
    );
    assert_eq!(e.state.priority_player_id(), 0);

    e.apply_command(0, &pass())
        .expect("AP pass — priority to NAP");

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
    e.apply_command(
        1,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for p1 first bolt");
    let b3 = hand_index_for_card(&e, 1, "lightning_bolt");
    e.apply_command(1, &cast_spell(b3, target_player(0)))
        .expect("p1 first bolt");
    e.apply_command(
        1,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for p1 second bolt");
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
    assert_eq!(
        e.state.players[0].life, 14,
        "NAP's two bolts resolve first (6 to P0)"
    );
    assert_eq!(
        e.state.players[1].life, 11,
        "then AP's three bolts (9 to P1)"
    );
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
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for AP bolt");
    let bolt_ap = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_ap, target_player(1)))
        .expect("AP bolt targeting P1");
    e.apply_command(0, &pass())
        .expect("AP pass — priority to P1");

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

    e.apply_command(
        1,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for NAP first bolt");
    let b1 = hand_index_for_card(&e, 1, "lightning_bolt");
    e.apply_command(1, &cast_spell(b1, target_player(0)))
        .expect("NAP first bolt");
    assert_eq!(e.state.priority_player_id(), 1);
    e.apply_command(
        1,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for NAP second bolt");
    let b2 = hand_index_for_card(&e, 1, "lightning_bolt");
    e.apply_command(1, &cast_spell(b2, target_player(0)))
        .expect("NAP second bolt while holding priority");
    assert_eq!(e.state.stack.len(), 3);

    resolve_entire_stack_two_player(&mut e);

    assert!(e.state.stack.is_empty());
    assert_eq!(
        e.state.players[0].life, 14,
        "two NAP bolts resolve before AP's"
    );
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

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for first bolt");
    let bolt_bottom = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_bottom, target_player(1)))
        .expect("first bolt (stack bottom)");
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add red mana for second bolt");
    let bolt_top = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_top, target_player(1)))
        .expect("second bolt while holding priority (stack top before counter)");
    let top_bolt_oid = e.state.stack.last().expect("top bolt").id;
    e.apply_command(0, &pass()).expect("AP pass");

    for _ in 0..2 {
        let ii = hand_index_for_card(&e, 1, "island");
        let oid = e.state.players[1].hand.remove(ii);
        e.state.players[1].battlefield.push(oid);
        e.state.objects.get_mut(&oid).expect("p1 island").zone = tricerules_core::Zone::Battlefield;
    }
    e.apply_command(
        1,
        &add_mana_to_pool(AddManaToPool {
            u: 2,
            ..Default::default()
        }),
    )
    .expect("add UU for counterspell");
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

    assert_eq!(
        e.state.stack.len(),
        3,
        "bottom bolt, top bolt, counterspell"
    );

    resolve_entire_stack_two_player(&mut e);

    assert!(e.state.stack.is_empty());
    assert_eq!(
        e.state.players[1].life, 17,
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
    let op = e
        .state
        .opening
        .as_ref()
        .expect("opening still active for bottom");
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
    assert!(
        err.is_err(),
        "must reject further mulligan when bottom phase is active"
    );
}

fn assign_combat_damage_cmd(attacker_id: u32, pairs: Vec<(u32, u32)>) -> RuledCommand {
    assign_combat_damage_cmd_with_player(attacker_id, pairs, 0)
}

fn assign_combat_damage_cmd_with_player(
    attacker_id: u32,
    pairs: Vec<(u32, u32)>,
    defending_player_damage: u32,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::AssignCombatDamage(AssignCombatDamage {
            attacker_id,
            assignments: pairs
                .into_iter()
                .map(|(blocker_id, damage)| DamagePair { blocker_id, damage })
                .collect(),
            defending_player_damage,
        })),
    }
}

/// Ensure `card_id` is in the player's hand, pulling from library if needed.
fn ensure_in_hand(e: &mut GameEngine, player: usize, card_id: &str) {
    let in_hand = e.state.players[player].hand.iter().any(|oid| {
        e.state
            .objects
            .get(oid)
            .map(|o| o.card_id == card_id)
            .unwrap_or(false)
    });
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
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");

    // Defender sends both blockers to the same attacker.
    let b = e
        .apply_command(
            1,
            &declare_blockers(vec![
                BlockPair {
                    attacker_id: attacker,
                    blocker_id: blocker_lions,
                },
                BlockPair {
                    attacker_id: attacker,
                    blocker_id: blocker_bears,
                },
            ]),
        )
        .expect("declare two blockers");

    assert!(
        e.state.combat.as_ref().unwrap().damage_assignment_needed,
        "damage_assignment_needed must be true after multi-block"
    );
    assert!(
        !e.state.combat.as_ref().unwrap().assign_combat_damage_phase,
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

    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    e.apply_command(1, &pass())
        .expect("defender pass → assign combat damage step");
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
    assert!(
        !dead_ids.contains(&blocker_bears),
        "bears survive: {dead_ids:?}"
    );
    let bears_obj = e.state.objects.get(&blocker_bears).expect("bears object");
    assert_eq!(bears_obj.damage, 1, "bears has 1 marked damage");
    assert_eq!(bears_obj.zone, tricerules_core::Zone::Battlefield);
    assert!(
        life_changes_in(&b3).is_empty(),
        "no life change on fully-blocked combat"
    );
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
            BlockPair {
                attacker_id: attacker,
                blocker_id: blocker_merfolk,
            },
            BlockPair {
                attacker_id: attacker,
                blocker_id: blocker_bears,
            },
        ]),
    )
    .expect("two blockers");
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    e.apply_command(1, &pass())
        .expect("defender pass → assign combat damage");
    let b = e
        .apply_command(
            0,
            &assign_combat_damage_cmd(attacker, vec![(blocker_merfolk, 1), (blocker_bears, 1)]),
        )
        .expect("assign combat damage");

    let dead = permanents_moved_in(&b);
    let dead_ids: Vec<u32> = dead.iter().map(|p| p.object_id).collect();

    // Attacker (2/1) gets 2+2=4 damage → dies.
    assert!(
        dead_ids.contains(&attacker),
        "lions attacker dies: {dead_ids:?}"
    );
    // Merfolk (2/1) gets 1 lethal → dies.
    assert!(
        dead_ids.contains(&blocker_merfolk),
        "merfolk die: {dead_ids:?}"
    );
    // Bears (2/2) gets remaining 1 damage (< toughness 2) → survives.
    assert!(
        !dead_ids.contains(&blocker_bears),
        "bears survive: {dead_ids:?}"
    );
    assert!(
        life_changes_in(&b).is_empty(),
        "no life change (fully blocked)"
    );
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
        &declare_blockers(vec![BlockPair {
            attacker_id: attacker,
            blocker_id: blocker,
        }]),
    )
    .expect("declare single blocker");

    assert!(
        !e.state.combat.as_ref().unwrap().damage_assignment_needed,
        "damage_assignment_needed must be false for single-blocker combat"
    );

    // Combat resolves normally without any AssignCombatDamage step: both 2/2s die.
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    let b = e.apply_command(1, &pass()).expect("combat damage");
    let dead = permanents_moved_in(&b);
    let dead_ids: Vec<u32> = dead.iter().map(|p| p.object_id).collect();
    assert!(
        dead_ids.contains(&attacker),
        "attacker dies in mutual block"
    );
    assert!(dead_ids.contains(&blocker), "blocker dies in mutual block");
    assert!(
        life_changes_in(&b).is_empty(),
        "no life loss on fully blocked combat"
    );
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
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    e.apply_command(1, &pass()).expect("defender pass");
    assert!(e.state.combat.as_ref().unwrap().assign_combat_damage_phase);
    (e, attacker, blocker_a, blocker_b)
}

#[test]
fn assign_combat_damage_rejects_sum_mismatch() {
    let (mut e, attacker, a, b) = setup_two_blockers_assign_phase(910);
    assert!(e
        .apply_command(0, &assign_combat_damage_cmd(attacker, vec![(a, 1), (b, 0)]),)
        .is_err());
    assert!(!e
        .state
        .combat
        .as_ref()
        .unwrap()
        .damage_assignments
        .contains_key(&attacker));
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
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    e.apply_command(1, &pass()).expect("defender pass");
    let b = e
        .apply_command(
            0,
            &assign_combat_damage_cmd(attacker, vec![(b1, 1), (b2, 1)]),
        )
        .expect("assign 1+1");
    let dead = permanents_moved_in(&b);
    let dead_ids: Vec<u32> = dead.iter().map(|p| p.object_id).collect();
    assert!(
        dead_ids.contains(&attacker),
        "attacker dies from 2+2 blocker damage"
    );
    assert!(
        !dead_ids.contains(&b1) && !dead_ids.contains(&b2),
        "both blockers survive with 1 dmg"
    );
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
        .apply_command(1, &assign_combat_damage_cmd(attacker, vec![(a, 1), (b, 1)]),)
        .is_err());
}

#[test]
fn assign_combat_damage_rejects_sum_exceeds_power() {
    // 2-power attacker, two blockers: 1+2 sums to 3 > power. Must reject.
    let (mut e, attacker, a, b) = setup_two_blockers_assign_phase(914);
    assert!(e
        .apply_command(0, &assign_combat_damage_cmd(attacker, vec![(a, 1), (b, 2)]))
        .is_err());
    assert!(!e
        .state
        .combat
        .as_ref()
        .unwrap()
        .damage_assignments
        .contains_key(&attacker));
    // State stays in assign-damage phase so the AP can retry with a legal split.
    assert!(e.state.combat.as_ref().unwrap().assign_combat_damage_phase);
}

#[test]
fn assign_combat_damage_three_blockers_split_one_each() {
    // 3-power attacker (Balduvian Barbarians, 3/2) blocked by three 2/2 grizzly bears.
    // Split 1+1+1: every blocker takes 1 (survives); attacker takes 2+2+2=6 → dies.
    let decks = Some(vec![
        std::iter::repeat_n("balduvian_barbarians".to_string(), 10).collect::<Vec<_>>(),
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
    ]);
    let mut e = GameEngine::new(915, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);
    ensure_in_hand(&mut e, 0, "balduvian_barbarians");
    ensure_in_hand(&mut e, 1, "grizzly_bears");
    let attacker = put_creature_on_battlefield(&mut e, 0, "balduvian_barbarians");
    let b1 = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let b2 = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let b3 = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");
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
            BlockPair {
                attacker_id: attacker,
                blocker_id: b3,
            },
        ]),
    )
    .expect("declare three blockers");
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    e.apply_command(1, &pass()).expect("defender pass");

    // Sum != power must still be rejected with N=3.
    assert!(e
        .apply_command(
            0,
            &assign_combat_damage_cmd(attacker, vec![(b1, 1), (b2, 1), (b3, 0)]),
        )
        .is_err());
    // Wrong blocker set (missing one) must be rejected.
    assert!(e
        .apply_command(
            0,
            &assign_combat_damage_cmd(attacker, vec![(b1, 2), (b2, 1)])
        )
        .is_err());

    let b = e
        .apply_command(
            0,
            &assign_combat_damage_cmd(attacker, vec![(b1, 1), (b2, 1), (b3, 1)]),
        )
        .expect("assign 1+1+1");
    let dead: Vec<u32> = permanents_moved_in(&b)
        .iter()
        .map(|p| p.object_id)
        .collect();
    assert!(
        dead.contains(&attacker),
        "attacker dies from 2+2+2 blocker damage: {dead:?}"
    );
    assert!(
        !dead.contains(&b1) && !dead.contains(&b2) && !dead.contains(&b3),
        "all three blockers survive at 1 marked damage: {dead:?}"
    );
    for bid in [b1, b2, b3] {
        let obj = e.state.objects.get(&bid).expect("blocker present");
        assert_eq!(obj.damage, 1);
        assert_eq!(obj.zone, tricerules_core::Zone::Battlefield);
    }
    // After resolution combat is cleared.
    assert!(e.state.combat.is_none());
}

#[test]
fn assign_combat_damage_two_multi_blocked_attackers_requires_both() {
    // Two grizzly_bears (2/2) attackers, each blocked by two coral_merfolk (2/1).
    // Engine must hold damage resolution until BOTH attackers receive assignments,
    // and resolution should only fire on the second assign call.
    let decks = Some(vec![
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
        std::iter::repeat_n("coral_merfolk".to_string(), 10).collect::<Vec<_>>(),
    ]);
    let mut e = GameEngine::new(916, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);
    ensure_in_hand(&mut e, 0, "grizzly_bears");
    ensure_in_hand(&mut e, 1, "coral_merfolk");
    let atk1 = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let atk2 = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let b1a = put_creature_on_battlefield(&mut e, 1, "coral_merfolk");
    let b1b = put_creature_on_battlefield(&mut e, 1, "coral_merfolk");
    let b2a = put_creature_on_battlefield(&mut e, 1, "coral_merfolk");
    let b2b = put_creature_on_battlefield(&mut e, 1, "coral_merfolk");
    e.apply_command(0, &declare_attackers(vec![atk1, atk2]))
        .expect("declare two attackers");
    e.apply_command(0, &pass()).expect("active pass");
    e.apply_command(1, &pass()).expect("defender pass");
    e.apply_command(
        1,
        &declare_blockers(vec![
            BlockPair {
                attacker_id: atk1,
                blocker_id: b1a,
            },
            BlockPair {
                attacker_id: atk1,
                blocker_id: b1b,
            },
            BlockPair {
                attacker_id: atk2,
                blocker_id: b2a,
            },
            BlockPair {
                attacker_id: atk2,
                blocker_id: b2b,
            },
        ]),
    )
    .expect("declare blockers");
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    e.apply_command(1, &pass()).expect("defender pass");
    assert!(e.state.combat.as_ref().unwrap().assign_combat_damage_phase);

    // First assignment: combat must NOT yet resolve.
    let b_first = e
        .apply_command(0, &assign_combat_damage_cmd(atk1, vec![(b1a, 1), (b1b, 1)]))
        .expect("assign for atk1");
    assert!(
        permanents_moved_in(&b_first).is_empty(),
        "no permanents moved yet; second attacker still needs assignment"
    );
    assert!(
        e.state
            .combat
            .as_ref()
            .expect("combat still active")
            .damage_assignment_needed,
        "still waiting on atk2 assignment"
    );

    // Second assignment: combat resolves now.
    let b_second = e
        .apply_command(0, &assign_combat_damage_cmd(atk2, vec![(b2a, 1), (b2b, 1)]))
        .expect("assign for atk2");
    let dead: Vec<u32> = permanents_moved_in(&b_second)
        .iter()
        .map(|p| p.object_id)
        .collect();
    // Each 2/2 attacker takes 1+1=2 damage from its two 2/1 blockers → both attackers die.
    assert!(dead.contains(&atk1), "atk1 dies: {dead:?}");
    assert!(dead.contains(&atk2), "atk2 dies: {dead:?}");
    // Each 2/1 blocker takes 1 lethal damage → all blockers die.
    for bid in [b1a, b1b, b2a, b2b] {
        assert!(dead.contains(&bid), "blocker {bid} dies: {dead:?}");
    }
    assert!(e.state.combat.is_none(), "combat cleared after resolution");
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
    let b = e.apply_command(1, &pass()).expect("nap pass begin_combat");
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
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );

    e.apply_command(0, &declare_attackers(vec![bears]))
        .expect("declare attacker");
    // Both pass in DeclareAttackers.
    e.apply_command(0, &pass())
        .expect("ap pass declare_attackers");
    let b = e
        .apply_command(1, &pass())
        .expect("nap pass declare_attackers");
    // Engine lands in DeclareBlockers with blockers_declared = true and active player holding priority.
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers
    );
    assert!(
        priority_changes_in(&b).contains(&0),
        "active player must hold priority when blockers auto-declared"
    );
    assert!(
        e.state.combat.as_ref().is_some_and(|c| c.blockers_declared),
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
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );

    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("ap pass declare_attackers");
    let b = e
        .apply_command(1, &pass())
        .expect("nap pass declare_attackers");
    // Defender has an eligible (summoning-sick) blocker → must get priority in DeclareBlockers.
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers
    );
    assert!(
        priority_changes_in(&b).contains(&1),
        "defender must hold priority in declare_blockers when they have a summoning-sick blocker"
    );
}

#[test]
fn cannot_add_mana_while_declaring_attackers() {
    let mut e = GameEngine::new(4010, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    // Priority is locked until the active player declares attackers.
    let err = e
        .apply_command(
            0,
            &add_mana_to_pool(AddManaToPool {
                r: 1,
                ..Default::default()
            }),
        )
        .expect_err("mana ability must be illegal during declare attackers");
    assert!(
        format!("{err:?}").contains("declaring attackers or blockers"),
        "unexpected error: {err:?}"
    );
}

#[test]
fn cannot_add_mana_while_declaring_blockers() {
    let mut e = GameEngine::new(4011, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let attacker = put_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attackers");
    e.apply_command(0, &pass())
        .expect("ap pass declare attackers");
    let b = e
        .apply_command(1, &pass())
        .expect("defender pass declare attackers");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers,
        "should be in declare blockers"
    );
    assert!(
        priority_changes_in(&b).contains(&1),
        "defender must hold priority in declare blockers"
    );
    // Defender holds priority but priority is locked for blocker declaration.
    let err = e
        .apply_command(
            1,
            &add_mana_to_pool(AddManaToPool {
                g: 1,
                ..Default::default()
            }),
        )
        .expect_err("mana ability must be illegal during declare blockers");
    assert!(
        format!("{err:?}").contains("declaring attackers or blockers"),
        "unexpected error: {err:?}"
    );
}

// ----------------------------------------------------------------------------
// New M2 primitives: GainLife, LoseLife, ExileTarget, ReturnToHand, Mill.
// ----------------------------------------------------------------------------

fn forest_only_deck() -> Vec<String> {
    vec!["forest".into(); 30]
}

fn island_only_deck() -> Vec<String> {
    vec!["island".into(); 30]
}

#[test]
fn healing_salve_gains_three_life_for_target_player() {
    let decks = Some(vec![
        vec![
            "plains".into(),
            "healing_salve".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        forest_only_deck(),
    ]);
    let mut e = GameEngine::new(2601, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            w: 1,
            ..Default::default()
        }),
    )
    .expect("mana for W");

    let salve_idx = hand_index_for_card(&e, 0, "healing_salve");
    let p1_life_before = e.state.players[1].life;
    e.apply_command(0, &cast_spell(salve_idx, target_player(1)))
        .expect("cast salve targeting opponent");
    let batch = {
        e.apply_command(0, &pass()).expect("p0 pass");
        e.apply_command(1, &pass()).expect("p1 pass")
    };

    assert_eq!(
        e.state.players[1].life,
        p1_life_before + 3,
        "target player (P1) gains 3"
    );
    let life = life_changes_in(&batch);
    assert!(
        life.iter()
            .any(|lc| lc.player_id == 1 && lc.delta == 3 && lc.new_total == p1_life_before + 3),
        "LifeChanged event expected, got {life:?}"
    );
}

#[test]
fn healing_salve_can_target_controller() {
    let decks = Some(vec![
        vec![
            "plains".into(),
            "healing_salve".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        forest_only_deck(),
    ]);
    let mut e = GameEngine::new(2602, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            w: 1,
            ..Default::default()
        }),
    )
    .expect("mana");
    let salve_idx = hand_index_for_card(&e, 0, "healing_salve");
    let p0_life_before = e.state.players[0].life;
    e.apply_command(0, &cast_spell(salve_idx, target_player(0)))
        .expect("salve may target controller");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");
    assert_eq!(e.state.players[0].life, p0_life_before + 3);
}

#[test]
fn angels_mercy_gains_seven_life_for_controller() {
    let mut p0_deck = vec!["angels_mercy".into()];
    for _ in 0..6 {
        p0_deck.push("plains".into());
    }
    let decks = Some(vec![p0_deck, forest_only_deck()]);
    let mut e = GameEngine::new(2603, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            w: 2,
            c: 3,
            ..Default::default()
        }),
    )
    .expect("mana for 3WW");
    let mercy_idx = hand_index_for_card(&e, 0, "angels_mercy");
    let life_before = e.state.players[0].life;
    e.apply_command(0, &cast_spell(mercy_idx, vec![]))
        .expect("cast mercy");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");
    assert_eq!(e.state.players[0].life, life_before + 7, "mercy gains 7");
}

#[test]
fn bump_in_the_night_drains_three_from_target_player() {
    let decks = Some(vec![
        vec![
            "swamp".into(),
            "bump_in_the_night".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
        ],
        forest_only_deck(),
    ]);
    let mut e = GameEngine::new(2604, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            b: 1,
            ..Default::default()
        }),
    )
    .expect("mana for B");
    let bump_idx = hand_index_for_card(&e, 0, "bump_in_the_night");
    let p1_life_before = e.state.players[1].life;
    let p0_life_before = e.state.players[0].life;
    e.apply_command(0, &cast_spell(bump_idx, target_player(1)))
        .expect("cast bump");
    let batch = {
        e.apply_command(0, &pass()).expect("p0 pass");
        e.apply_command(1, &pass()).expect("p1 pass")
    };
    assert_eq!(e.state.players[1].life, p1_life_before - 3);
    assert_eq!(
        e.state.players[0].life, p0_life_before,
        "controller unaffected"
    );
    let life = life_changes_in(&batch);
    assert!(
        life.iter().any(|lc| lc.player_id == 1 && lc.delta == -3),
        "LifeChanged(-3) on P1 expected, got {life:?}"
    );
}

#[test]
fn bump_in_the_night_rejects_creature_target() {
    let decks = Some(vec![
        vec![
            "swamp".into(),
            "bump_in_the_night".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
        ],
        forest_only_deck(),
    ]);
    let mut e = GameEngine::new(2605, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            b: 1,
            ..Default::default()
        }),
    )
    .expect("mana");
    let bump_idx = hand_index_for_card(&e, 0, "bump_in_the_night");
    let err = e
        .apply_command(
            0,
            &cast_spell(bump_idx, vec![TargetRef { object_id: bear }]),
        )
        .expect_err("bump cannot target creature");
    assert!(format!("{err:?}").contains("player"), "unexpected: {err:?}");
}

#[test]
fn bump_in_the_night_rejects_self_target() {
    let decks = Some(vec![
        vec![
            "swamp".into(),
            "bump_in_the_night".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
        ],
        forest_only_deck(),
    ]);
    let mut e = GameEngine::new(2615, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            b: 1,
            ..Default::default()
        }),
    )
    .expect("mana");
    let bump_idx = hand_index_for_card(&e, 0, "bump_in_the_night");
    let err = e
        .apply_command(0, &cast_spell(bump_idx, target_player(0)))
        .expect_err("bump cannot target self (target opponent)");
    assert!(
        format!("{err:?}").contains("opponent"),
        "unexpected: {err:?}"
    );
}

#[test]
fn blood_tithe_drains_each_opponent_and_gains_controller_equal_life() {
    let mut p0_deck = vec!["blood_tithe".into()];
    for _ in 0..6 {
        p0_deck.push("swamp".into());
    }
    let decks = Some(vec![p0_deck, forest_only_deck()]);
    let mut e = GameEngine::new(2606, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            b: 1,
            c: 2,
            ..Default::default()
        }),
    )
    .expect("mana for 2B");
    let tithe_idx = hand_index_for_card(&e, 0, "blood_tithe");
    let p1_life_before = e.state.players[1].life;
    let p0_life_before = e.state.players[0].life;
    e.apply_command(0, &cast_spell(tithe_idx, vec![]))
        .expect("cast tithe");
    let batch = {
        e.apply_command(0, &pass()).expect("p0 pass");
        e.apply_command(1, &pass()).expect("p1 pass")
    };
    assert_eq!(e.state.players[1].life, p1_life_before - 3);
    assert_eq!(
        e.state.players[0].life,
        p0_life_before + 3,
        "controller gains 3 (life lost from one opponent)"
    );
    let life = life_changes_in(&batch);
    assert!(
        life.iter().any(|lc| lc.player_id == 0 && lc.delta == 3),
        "expected +3 LifeChanged on controller, got {life:?}"
    );
}

#[test]
fn eyeblights_ending_destroys_target_creature() {
    let decks = Some(vec![
        vec![
            "swamp".into(),
            "eyeblights_ending".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
        ],
        forest_only_deck(),
    ]);
    let mut e = GameEngine::new(2607, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            b: 1,
            c: 1,
            ..Default::default()
        }),
    )
    .expect("mana for 1B");
    let idx = hand_index_for_card(&e, 0, "eyeblights_ending");
    e.apply_command(0, &cast_spell(idx, vec![TargetRef { object_id: bear }]))
        .expect("cast eyeblight");
    let batch = {
        e.apply_command(0, &pass()).expect("p0 pass");
        e.apply_command(1, &pass()).expect("p1 pass")
    };
    assert_eq!(
        e.state.objects.get(&bear).expect("bear").zone,
        tricerules_core::Zone::Graveyard
    );
    assert!(e.state.players[1].graveyard.contains(&bear));
    assert!(!e.state.players[1].battlefield.contains(&bear));
    let moves = permanents_moved_in(&batch);
    assert!(
        moves.iter().any(|m| m.object_id == bear
            && m.destination
                == tricerules_proto::ruled::v1::permanent_moved::Destination::Graveyard as i32),
        "expected PermanentMoved(Graveyard) for bear, got {moves:?}"
    );
}

#[test]
fn swords_to_plowshares_exiles_and_gains_life_equal_to_power() {
    let decks = Some(vec![
        vec![
            "plains".into(),
            "swords_to_plowshares".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        forest_only_deck(),
    ]);
    let mut e = GameEngine::new(2608, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let bear_power = e.state.objects.get(&bear).unwrap().power.unwrap();
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            w: 1,
            ..Default::default()
        }),
    )
    .expect("mana for W");
    let idx = hand_index_for_card(&e, 0, "swords_to_plowshares");
    let p1_life_before = e.state.players[1].life;
    e.apply_command(0, &cast_spell(idx, vec![TargetRef { object_id: bear }]))
        .expect("cast swords");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");

    assert_eq!(
        e.state.objects.get(&bear).expect("bear").zone,
        tricerules_core::Zone::Exile
    );
    // Lifegain accrues to the creature's controller (P1), per Swords' Oracle text.
    assert_eq!(
        e.state.players[1].life,
        p1_life_before + bear_power as i32,
        "controller of exiled creature gains life equal to its power"
    );
}

#[test]
fn swords_to_plowshares_fizzles_if_target_dies_before_resolution() {
    let decks = Some(vec![
        vec![
            "plains".into(),
            "swords_to_plowshares".into(),
            "mountain".into(),
            "lightning_bolt".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        forest_only_deck(),
    ]);
    let mut e = GameEngine::new(2609, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            w: 1,
            r: 1,
            ..Default::default()
        }),
    )
    .expect("mana for W+R");

    let swords_idx = hand_index_for_card(&e, 0, "swords_to_plowshares");
    e.apply_command(
        0,
        &cast_spell(swords_idx, vec![TargetRef { object_id: bear }]),
    )
    .expect("cast swords");
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(
        0,
        &cast_spell(bolt_idx, vec![TargetRef { object_id: bear }]),
    )
    .expect("cast bolt on top");
    assert_eq!(e.state.stack.len(), 2);

    let p1_life_before = e.state.players[1].life;
    resolve_entire_stack_two_player(&mut e);

    // Bolt killed the bear; Swords had no legal target → fizzles, no life change.
    assert_eq!(
        e.state.objects.get(&bear).expect("bear").zone,
        tricerules_core::Zone::Graveyard,
        "bear died to bolt"
    );
    assert_eq!(
        e.state.players[1].life, p1_life_before,
        "swords fizzled, no life gain"
    );
}

#[test]
fn unsummon_returns_target_creature_to_owner_hand() {
    let decks = Some(vec![
        vec![
            "island".into(),
            "unsummon".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
        forest_only_deck(),
    ]);
    let mut e = GameEngine::new(2610, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let bear = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            u: 1,
            ..Default::default()
        }),
    )
    .expect("mana for U");
    let idx = hand_index_for_card(&e, 0, "unsummon");
    let p1_hand_before = e.state.players[1].hand.len();
    e.apply_command(0, &cast_spell(idx, vec![TargetRef { object_id: bear }]))
        .expect("cast unsummon");
    let batch = {
        e.apply_command(0, &pass()).expect("p0 pass");
        e.apply_command(1, &pass()).expect("p1 pass")
    };
    assert_eq!(
        e.state.objects.get(&bear).expect("bear").zone,
        tricerules_core::Zone::Hand
    );
    assert_eq!(e.state.players[1].hand.len(), p1_hand_before + 1);
    assert!(!e.state.players[1].battlefield.contains(&bear));
    let moves = permanents_moved_in(&batch);
    assert!(
        moves.iter().any(|m| m.object_id == bear
            && m.destination
                == tricerules_proto::ruled::v1::permanent_moved::Destination::Hand as i32),
        "expected PermanentMoved(Hand) for bear, got {moves:?}"
    );
}

#[test]
fn unsummon_rejects_land_target() {
    let decks = Some(vec![
        vec![
            "island".into(),
            "unsummon".into(),
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
        ],
    ]);
    let mut e = GameEngine::new(2611, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let island_idx = hand_index_for_card(&e, 0, "island");
    e.apply_command(0, &play_land(island_idx))
        .expect("play island");
    let island_oid = battlefield_object_for_card(&e, 0, "island");
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            u: 1,
            ..Default::default()
        }),
    )
    .expect("mana");
    let idx = hand_index_for_card(&e, 0, "unsummon");
    let err = e
        .apply_command(
            0,
            &cast_spell(
                idx,
                vec![TargetRef {
                    object_id: island_oid,
                }],
            ),
        )
        .expect_err("unsummon cannot target land");
    assert!(
        format!("{err:?}").contains("creature"),
        "unexpected: {err:?}"
    );
}

#[test]
fn boomerang_returns_target_land_to_owner_hand() {
    let decks = Some(vec![
        vec![
            "island".into(),
            "boomerang".into(),
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
        ],
    ]);
    let mut e = GameEngine::new(2612, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    let island_idx = hand_index_for_card(&e, 0, "island");
    e.apply_command(0, &play_land(island_idx))
        .expect("play island");
    let island_oid = battlefield_object_for_card(&e, 0, "island");
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            u: 2,
            ..Default::default()
        }),
    )
    .expect("mana for UU");
    let idx = hand_index_for_card(&e, 0, "boomerang");
    e.apply_command(
        0,
        &cast_spell(
            idx,
            vec![TargetRef {
                object_id: island_oid,
            }],
        ),
    )
    .expect("cast boomerang on own island");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");
    assert_eq!(
        e.state.objects.get(&island_oid).expect("island").zone,
        tricerules_core::Zone::Hand
    );
    assert!(!e.state.players[0].battlefield.contains(&island_oid));
}

#[test]
fn tome_scour_mills_five_cards_from_target_player() {
    let mut p1_deck = vec!["forest".into(); 30];
    // Sentinel cards at the top so we can assert ordering.
    p1_deck[0] = "grizzly_bears".into();
    p1_deck[1] = "savannah_lions".into();
    p1_deck[2] = "coral_merfolk".into();
    p1_deck[3] = "walking_corpse".into();
    p1_deck[4] = "balduvian_barbarians".into();
    let decks = Some(vec![island_only_deck(), p1_deck]);
    let mut e = GameEngine::new(2613, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    // Place tome_scour in P0 hand directly to avoid deck ordering churn.
    take_card_from_library_to_hand(&mut e, 0, "island");
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            u: 1,
            ..Default::default()
        }),
    )
    .expect("mana");
    // Inject the spell into hand from the registry.
    let scour_id = {
        let id = e.state.next_object_id;
        e.state.next_object_id += 1;
        e.state.objects.insert(
            id,
            tricerules_core::state::GameObject {
                id,
                owner: 0,
                card_id: "tome_scour".into(),
                zone: tricerules_core::Zone::Hand,
                tapped: false,
                summoning_sick: false,
                power: None,
                toughness: None,
                damage: 0,
                deathtouch_damage: false,
                counters: std::collections::BTreeMap::new(),
            },
        );
        e.state.players[0].hand.push(id);
        id
    };
    let scour_idx = e.state.players[0]
        .hand
        .iter()
        .position(|&oid| oid == scour_id)
        .expect("scour in hand");
    let lib_before = e.state.players[1].library.len();
    let grave_before = e.state.players[1].graveyard.len();
    e.apply_command(0, &cast_spell(scour_idx, target_player(1)))
        .expect("cast tome scour");
    let batch = {
        e.apply_command(0, &pass()).expect("p0 pass");
        e.apply_command(1, &pass()).expect("p1 pass")
    };
    assert_eq!(e.state.players[1].library.len(), lib_before - 5);
    assert_eq!(e.state.players[1].graveyard.len(), grave_before + 5);
    let moves = permanents_moved_in(&batch);
    let to_grave: Vec<_> = moves
        .iter()
        .filter(|m| {
            m.owner_player_id == 1
                && m.destination
                    == tricerules_proto::ruled::v1::permanent_moved::Destination::Graveyard as i32
        })
        .collect();
    assert_eq!(to_grave.len(), 5, "five PermanentMoved->Graveyard events");
    assert!(
        to_grave.iter().all(|m| !m.card_id.is_empty()),
        "milled PermanentMoved events must carry card_id so servers can resolve library cards"
    );
}

#[test]
fn tome_scour_caps_at_library_size() {
    let mut p1_deck = vec!["forest".into(); 8];
    let decks = Some(vec![island_only_deck(), p1_deck.split_off(0)]);
    let mut e = GameEngine::new(2614, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    // Manually drain P1 library to 2 cards.
    while e.state.players[1].library.len() > 2 {
        let oid = e.state.players[1].library.pop_back().unwrap();
        e.state.players[1].graveyard.push(oid);
        if let Some(o) = e.state.objects.get_mut(&oid) {
            o.zone = tricerules_core::Zone::Graveyard;
        }
    }
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            u: 1,
            ..Default::default()
        }),
    )
    .expect("mana");
    let scour_id = {
        let id = e.state.next_object_id;
        e.state.next_object_id += 1;
        e.state.objects.insert(
            id,
            tricerules_core::state::GameObject {
                id,
                owner: 0,
                card_id: "tome_scour".into(),
                zone: tricerules_core::Zone::Hand,
                tapped: false,
                summoning_sick: false,
                power: None,
                toughness: None,
                damage: 0,
                deathtouch_damage: false,
                counters: std::collections::BTreeMap::new(),
            },
        );
        e.state.players[0].hand.push(id);
        id
    };
    let scour_idx = e.state.players[0]
        .hand
        .iter()
        .position(|&oid| oid == scour_id)
        .expect("scour in hand");
    e.apply_command(0, &cast_spell(scour_idx, target_player(1)))
        .expect("cast scour");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");
    // Library should be empty (only had 2 to mill), graveyard should hold both — engine must not panic.
    assert_eq!(e.state.players[1].library.len(), 0);
}

#[test]
fn tome_scour_can_target_controller() {
    // Tome Scour is Oracle "target player": milling yourself is legal.
    let decks = Some(vec![island_only_deck(), forest_only_deck()]);
    let mut e = GameEngine::new(2615, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            u: 1,
            ..Default::default()
        }),
    )
    .expect("mana");
    let scour_id = {
        let id = e.state.next_object_id;
        e.state.next_object_id += 1;
        e.state.objects.insert(
            id,
            tricerules_core::state::GameObject {
                id,
                owner: 0,
                card_id: "tome_scour".into(),
                zone: tricerules_core::Zone::Hand,
                tapped: false,
                summoning_sick: false,
                power: None,
                toughness: None,
                damage: 0,
                deathtouch_damage: false,
                counters: std::collections::BTreeMap::new(),
            },
        );
        e.state.players[0].hand.push(id);
        id
    };
    let scour_idx = e.state.players[0]
        .hand
        .iter()
        .position(|&oid| oid == scour_id)
        .expect("scour in hand");
    let lib_before = e.state.players[0].library.len();
    e.apply_command(0, &cast_spell(scour_idx, target_player(0)))
        .expect("tome scour targeting its controller is legal");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");
    // Five cards milled from the controller's own library (the resolved sorcery also lands
    // in the controller's graveyard, so assert the library side for an unambiguous count).
    assert_eq!(e.state.players[0].library.len(), lib_before - 5);
}

#[test]
fn mind_sculpt_rejects_self_target() {
    // Mind Sculpt is opponent-only in this build: casting at yourself is illegal at cast time.
    let decks = Some(vec![island_only_deck(), forest_only_deck()]);
    let mut e = GameEngine::new(2616, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            u: 1,
            c: 1,
            ..Default::default()
        }),
    )
    .expect("mana");
    let sculpt_id = {
        let id = e.state.next_object_id;
        e.state.next_object_id += 1;
        e.state.objects.insert(
            id,
            tricerules_core::state::GameObject {
                id,
                owner: 0,
                card_id: "mind_sculpt".into(),
                zone: tricerules_core::Zone::Hand,
                tapped: false,
                summoning_sick: false,
                power: None,
                toughness: None,
                damage: 0,
                deathtouch_damage: false,
                counters: std::collections::BTreeMap::new(),
            },
        );
        e.state.players[0].hand.push(id);
        id
    };
    let sculpt_idx = e.state.players[0]
        .hand
        .iter()
        .position(|&oid| oid == sculpt_id)
        .expect("sculpt in hand");
    let lib_before = e.state.players[0].library.len();
    let err = e.apply_command(0, &cast_spell(sculpt_idx, target_player(0)));
    assert!(
        err.is_err(),
        "mind sculpt targeting its controller must be rejected"
    );
    // No cards milled from the caster.
    assert_eq!(e.state.players[0].library.len(), lib_before);
}

// ── Flying & Reach Keyword Tests ─────────────────────────────────────────────
//
// Tests for CR 702.9b (flying) and CR 702.17b (reach) blocking restrictions.

/// A ground creature (no flying, no reach) attempting to block a flying attacker
/// must be rejected by set_blockers with an Illegal error mentioning "flying".
///
/// Setup: P1 has *both* coral_merfolk and a storm_crow so `defending_player_has_eligible_blockers`
/// returns true (storm_crow can block the flyer) and P1 actually reaches manual declaration.
/// We then try to block with the ground-only merfolk — that must fail.
#[test]
fn flying_creature_blocked_by_ground_creature_is_illegal() {
    let mut e = GameEngine::new(9001, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let crow_atk = inject_creature_on_battlefield(&mut e, 0, "storm_crow");
    // P1 has a ground creature and a flying creature; engine won't auto-skip.
    let merfolk = inject_creature_on_battlefield(&mut e, 1, "coral_merfolk");
    let _crow_blk = inject_creature_on_battlefield(&mut e, 1, "storm_crow");
    e.apply_command(0, &declare_attackers(vec![crow_atk]))
        .expect("declare storm crow attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    // P1 tries to block with coral_merfolk (no flying/reach) — must be rejected.
    let err = e
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: crow_atk,
                blocker_id: merfolk,
            }]),
        )
        .expect_err("ground creature blocking a flyer should be illegal");
    assert!(
        err.to_string().contains("evasion"),
        "error should mention evasion, got: {err}"
    );
}

/// A creature with flying can block another creature with flying (CR 702.9b).
#[test]
fn flying_creature_can_be_blocked_by_flying_creature() {
    let mut e = GameEngine::new(9002, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let crow_atk = inject_creature_on_battlefield(&mut e, 0, "storm_crow");
    let crow_blk = inject_creature_on_battlefield(&mut e, 1, "storm_crow");
    e.apply_command(0, &declare_attackers(vec![crow_atk]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: crow_atk,
            blocker_id: crow_blk,
        }]),
    )
    .expect("flying creature must be able to block another flying creature");
}

/// A creature with reach can block a creature with flying (CR 702.17b).
#[test]
fn flying_creature_can_be_blocked_by_reach_creature() {
    let mut e = GameEngine::new(9003, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let crow = inject_creature_on_battlefield(&mut e, 0, "storm_crow");
    let spider = inject_creature_on_battlefield(&mut e, 1, "giant_spider");
    e.apply_command(0, &declare_attackers(vec![crow]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: crow,
            blocker_id: spider,
        }]),
    )
    .expect("reach creature must be able to block a flying creature");
}

/// When the only attacker has flying and the defender has only ground creatures,
/// `defending_player_has_eligible_blockers` returns false and the engine emits
/// BlockersDeclared (empty) automatically — no manual declaration needed.
#[test]
fn flying_auto_skips_blockers_when_no_reach_or_flyers() {
    let mut e = GameEngine::new(9004, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let crow = inject_creature_on_battlefield(&mut e, 0, "storm_crow");
    // P1 has only a ground creature — cannot legally block the flying crow.
    let _bears = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![crow]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    // After P1's pass the engine should auto-skip blockers (grizzly_bears can't block flyer).
    let b = e
        .apply_command(1, &pass())
        .expect("defender pass declare attackers -> auto-skip");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers,
        "should enter DeclareBlockers after auto-skip"
    );
    // Auto-skip emits BlockersDeclared with empty pairs.
    let bd = blockers_declared_in(&b);
    assert_eq!(
        bd.len(),
        1,
        "exactly one BlockersDeclared event from auto-skip"
    );
    assert!(bd[0].block_pairs.is_empty(), "no block pairs in auto-skip");
    // The blockers_declared flag is set so combat can proceed without manual declaration.
    assert!(
        e.state.combat.as_ref().unwrap().blockers_declared,
        "blockers_declared flag must be set after auto-skip"
    );
}

/// Regression: flying/reach changes must not affect normal ground-vs-ground blocking.
#[test]
fn ground_creature_still_blockable_by_ground_blocker_regression() {
    let mut e = GameEngine::new(9005, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let merfolk = inject_creature_on_battlefield(&mut e, 0, "coral_merfolk");
    let bears = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![merfolk]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: merfolk,
            blocker_id: bears,
        }]),
    )
    .expect("ground creature must still be able to block a ground attacker");
}

// ── Intimidate Keyword Tests ──────────────────────────────────────────────────
//
// Tests for CR 702.13b: a creature with intimidate can only be blocked by
// artifact creatures and/or creatures that share a color with it.

/// A non-artifact creature of a different color cannot block an intimidate creature.
/// Accursed Spirit (Black) vs Grizzly Bears (Green) — no shared color, not artifact.
#[test]
fn intimidate_blocked_by_different_color_non_artifact_is_illegal() {
    let mut e = GameEngine::new(9010, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let spirit = inject_creature_on_battlefield(&mut e, 0, "accursed_spirit");
    // P1 has Grizzly Bears (Green) AND a black creature so engine won't auto-skip.
    let bears = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let _corpse = inject_creature_on_battlefield(&mut e, 1, "walking_corpse"); // black — keeps blockers open
    e.apply_command(0, &declare_attackers(vec![spirit]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    // Grizzly Bears is green, non-artifact — cannot block a black intimidate creature.
    let err = e
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: spirit,
                blocker_id: bears,
            }]),
        )
        .expect_err("different-color non-artifact blocker must be rejected");
    assert!(
        err.to_string().contains("evasion"),
        "error should mention evasion, got: {err}"
    );
}

/// A creature that shares a color with the intimidate creature can block it.
/// Accursed Spirit (Black) vs Walking Corpse (Black) — same color.
#[test]
fn intimidate_blocked_by_same_color_creature_is_legal() {
    let mut e = GameEngine::new(9011, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let spirit = inject_creature_on_battlefield(&mut e, 0, "accursed_spirit");
    // Walking Corpse costs 1B — it is a Black creature, shares color with the Spirit.
    let corpse = inject_creature_on_battlefield(&mut e, 1, "walking_corpse");
    e.apply_command(0, &declare_attackers(vec![spirit]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: spirit,
            blocker_id: corpse,
        }]),
    )
    .expect("same-color creature must be able to block an intimidate creature");
}

/// An artifact creature can always block an intimidate creature regardless of color.
/// Accursed Spirit (Black) vs Ornithopter (Colorless artifact) — no shared color, but artifact.
#[test]
fn intimidate_blocked_by_artifact_creature_is_legal() {
    let mut e = GameEngine::new(9012, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let spirit = inject_creature_on_battlefield(&mut e, 0, "accursed_spirit");
    // Ornithopter is a colorless artifact creature — qualifies despite no shared color.
    let thopter = inject_creature_on_battlefield(&mut e, 1, "ornithopter");
    e.apply_command(0, &declare_attackers(vec![spirit]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: spirit,
            blocker_id: thopter,
        }]),
    )
    .expect("artifact creature must be able to block an intimidate creature");
}

// ── Vigilance Keyword Tests ───────────────────────────────────────────────────
//
// Tests for CR 702.20b: a creature with vigilance doesn't tap when it attacks.

/// A creature with Vigilance remains untapped after being declared as an attacker.
/// Alpine Watchdog attacks — it should still be untapped after declaration.
#[test]
fn vigilance_attacker_does_not_tap() {
    let mut e = GameEngine::new(9020, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let watchdog = inject_creature_on_battlefield(&mut e, 0, "alpine_watchdog");
    e.apply_command(0, &declare_attackers(vec![watchdog]))
        .expect("declare vigilance attacker");
    let obj = e.state.objects.get(&watchdog).expect("watchdog object");
    assert!(
        !obj.tapped,
        "CR 702.20b: Alpine Watchdog (Vigilance) must NOT be tapped after attacking"
    );
}

/// Regression: a creature without Vigilance still taps when attacking.
/// Grizzly Bears attacks — it should be tapped after declaration.
#[test]
fn non_vigilance_attacker_still_taps() {
    let mut e = GameEngine::new(9021, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let bears = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    // Need a blocker on P1's side so the engine doesn't auto-skip to end combat.
    let _blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![bears]))
        .expect("declare normal attacker");
    let obj = e.state.objects.get(&bears).expect("bears object");
    assert!(
        obj.tapped,
        "a creature without Vigilance must be tapped after attacking"
    );
}

// ── Lifelink Keyword Tests ────────────────────────────────────────────────────
//
// Tests for CR 702.15b: damage dealt by a lifelink permanent also causes its
// controller to gain that much life.

/// An unblocked lifelink attacker deals damage to the defending player AND its
/// controller gains that much life simultaneously (CR 702.15b).
/// Child of Night (2/1 Lifelink) attacks unblocked — P1 loses 2 life, P0 gains 2.
#[test]
fn lifelink_unblocked_attacker_gains_life() {
    let mut e = GameEngine::new(9030, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let con = inject_creature_on_battlefield(&mut e, 0, "child_of_night");
    // No blockers for P1 → auto-skip.
    e.apply_command(0, &declare_attackers(vec![con]))
        .expect("declare lifelink attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    // Auto-empty blockers declared; active player has priority in DeclareBlockers.
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    let b = e
        .apply_command(1, &pass())
        .expect("defender pass declare blockers -> combat damage");
    let life = life_changes_in(&b);
    // Expect two events: defender loses 2, attacker controller gains 2.
    let defender_ev = life
        .iter()
        .find(|lc| lc.player_id == 1)
        .expect("defender LifeChanged");
    let attacker_ev = life
        .iter()
        .find(|lc| lc.player_id == 0)
        .expect("attacker controller LifeChanged (lifelink)");
    assert_eq!(defender_ev.delta, -2, "CR 702.15b: defender takes 2 damage");
    assert_eq!(defender_ev.new_total, 18);
    assert_eq!(attacker_ev.delta, 2, "CR 702.15b: lifelink gains 2 life");
    assert_eq!(attacker_ev.new_total, 22);
    assert_eq!(e.state.players[0].life, 22);
    assert_eq!(e.state.players[1].life, 18);
}

/// A lifelink creature gains its controller life when blocked — it deals damage to
/// the blocker (not the player), but lifelink still triggers (CR 702.15b).
#[test]
fn lifelink_blocked_attacker_still_gains_life() {
    let mut e = GameEngine::new(9031, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let con = inject_creature_on_battlefield(&mut e, 0, "child_of_night");
    // P1 has a blocker to intercept.
    let blocker = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![con]))
        .expect("declare lifelink attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: con,
            blocker_id: blocker,
        }]),
    )
    .expect("declare blocker");
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    let b = e
        .apply_command(1, &pass())
        .expect("defender pass declare blockers -> combat damage");
    let life = life_changes_in(&b);
    // Defender player takes no direct damage (blocked).
    assert!(
        !life.iter().any(|lc| lc.player_id == 1 && lc.delta < 0),
        "defending player should not lose life when attack is blocked"
    );
    // Attacker controller should gain life equal to damage dealt to the blocker.
    let attacker_ev = life
        .iter()
        .find(|lc| lc.player_id == 0)
        .expect("attacker controller LifeChanged (lifelink)");
    assert_eq!(
        attacker_ev.delta, 2,
        "CR 702.15b: lifelink gains life even when blocked"
    );
    assert_eq!(e.state.players[0].life, 22);
}

/// A lifelink blocker gains its controller life for the damage it deals to the
/// attacker (CR 702.15b).
#[test]
fn lifelink_blocker_gains_life() {
    let mut e = GameEngine::new(9032, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    // P0 attacks with a plain Grizzly Bears (no lifelink).
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    // P1 blocks with Child of Night (Lifelink, 2 power in injected state).
    let con = inject_creature_on_battlefield(&mut e, 1, "child_of_night");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: attacker,
            blocker_id: con,
        }]),
    )
    .expect("declare lifelink blocker");
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    let b = e
        .apply_command(1, &pass())
        .expect("defender pass declare blockers -> combat damage");
    let life = life_changes_in(&b);
    // Defending player takes no direct damage (blocked).
    assert!(
        !life.iter().any(|lc| lc.player_id == 1 && lc.delta < 0),
        "defending player should not lose life (attack was blocked)"
    );
    // Blocker's controller (P1) gains life equal to damage the lifelink blocker dealt.
    let blocker_ev = life
        .iter()
        .find(|lc| lc.player_id == 1 && lc.delta > 0)
        .expect("blocker controller LifeChanged (lifelink)");
    assert_eq!(
        blocker_ev.delta, 2,
        "CR 702.15b: lifelink blocker gains life equal to damage it dealt"
    );
    assert_eq!(e.state.players[1].life, 22);
}

/// When all of the defender's creatures are ineligible to block an intimidate creature,
/// the engine auto-skips blocker declaration.
/// Accursed Spirit (Black) vs Grizzly Bears only (Green, non-artifact) → auto-skip.
#[test]
fn intimidate_auto_skips_blockers_when_no_eligible_creatures() {
    let mut e = GameEngine::new(9013, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let spirit = inject_creature_on_battlefield(&mut e, 0, "accursed_spirit");
    // P1 has only Grizzly Bears — Green, non-artifact, cannot block Black intimidate.
    let _bears = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![spirit]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    // After P1's pass the engine detects no eligible blockers and auto-skips.
    let b = e
        .apply_command(1, &pass())
        .expect("defender pass declare attackers -> auto-skip");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers,
        "should enter DeclareBlockers after auto-skip"
    );
    let bd = blockers_declared_in(&b);
    assert_eq!(
        bd.len(),
        1,
        "exactly one BlockersDeclared event from auto-skip"
    );
    assert!(bd[0].block_pairs.is_empty(), "no block pairs in auto-skip");
    assert!(
        e.state.combat.as_ref().unwrap().blockers_declared,
        "blockers_declared flag must be set after auto-skip"
    );
}

// ---------------------------------------------------------------------------
// Haste (CR 702.10)
// ---------------------------------------------------------------------------

/// CR 702.10b: A creature with Haste can attack the same turn it enters the battlefield,
/// ignoring summoning sickness. Happy path: Raging Goblin is injected with summoning_sick=true
/// but is allowed to be declared as an attacker.
#[test]
fn haste_creature_can_attack_same_turn_it_enters() {
    let mut e = GameEngine::new(9020, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    // Inject Raging Goblin directly onto the battlefield *with* summoning sickness still set,
    // simulating the turn it just entered.
    let goblin = e.state.next_object_id;
    e.state.next_object_id += 1;
    let pid = e.state.players[0].id;
    e.state.objects.insert(
        goblin,
        tricerules_core::state::GameObject {
            id: goblin,
            owner: pid,
            card_id: "raging_goblin".to_string(),
            zone: tricerules_core::Zone::Battlefield,
            tapped: false,
            summoning_sick: true, // still sick — haste should bypass this
            power: Some(1),
            toughness: Some(1),
            damage: 0,
            deathtouch_damage: false,
            counters: std::collections::BTreeMap::new(),
        },
    );
    e.state.players[0].battlefield.push(goblin);

    // Declare the Goblin as an attacker — must succeed despite summoning_sick = true.
    e.apply_command(0, &declare_attackers(vec![goblin]))
        .expect("haste creature should be allowed to attack same turn it entered");
    assert!(
        e.state.combat.as_ref().unwrap().attacking.contains(&goblin),
        "raging goblin must appear in the attacking list"
    );
}

/// CR 302.6 / 702.10: Without Haste, a summoning-sick creature may NOT be declared as an
/// attacker. Illegal path: Grizzly Bears with summoning_sick=true are rejected.
#[test]
fn non_haste_summoning_sick_creature_cannot_attack() {
    let mut e = GameEngine::new(9021, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    // Inject Bears directly with summoning sickness and no haste.
    let bears = e.state.next_object_id;
    e.state.next_object_id += 1;
    let pid = e.state.players[0].id;
    e.state.objects.insert(
        bears,
        tricerules_core::state::GameObject {
            id: bears,
            owner: pid,
            card_id: "grizzly_bears".to_string(),
            zone: tricerules_core::Zone::Battlefield,
            tapped: false,
            summoning_sick: true,
            power: Some(2),
            toughness: Some(2),
            damage: 0,
            deathtouch_damage: false,
            counters: std::collections::BTreeMap::new(),
        },
    );
    e.state.players[0].battlefield.push(bears);

    assert!(
        e.apply_command(0, &declare_attackers(vec![bears])).is_err(),
        "summoning-sick creature without haste must not be allowed to attack"
    );
}

// ── Deathtouch Keyword Tests ──────────────────────────────────────────────────
//
// Tests for CR 702.2b / CR 704.5h: any amount of damage dealt by a deathtouch
// source to a creature is enough to destroy it via SBA.

/// Helper: inject a creature with explicit power and toughness (unlike the
/// default 2/2 of `inject_creature_on_battlefield`).
fn inject_creature_with_stats(
    e: &mut GameEngine,
    player: usize,
    card_id: &str,
    power: u32,
    toughness: u32,
) -> u32 {
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
            power: Some(power),
            toughness: Some(toughness),
            damage: 0,
            deathtouch_damage: false,
            counters: std::collections::BTreeMap::new(),
        },
    );
    e.state.players[player].battlefield.push(id);
    id
}

/// CR 702.2b / CR 704.5h: A deathtouch attacker destroys a blocker even when
/// the attacker's power is less than the blocker's toughness.
/// Pharika's Chosen (1/1 Deathtouch) attacks → Walking Corpse (2/2) blocks.
/// Chosen deals 1 damage: insufficient to kill normally (toughness 2), but
/// lethal via deathtouch. Walking Corpse dies; Chosen dies to the 2 damage back.
#[test]
fn deathtouch_attacker_kills_blocker_with_higher_toughness() {
    let mut e = GameEngine::new(9040, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    // Pharika's Chosen: 1/1 deathtouch — inject at actual power/toughness.
    let chosen = inject_creature_with_stats(&mut e, 0, "pharikas_chosen", 1, 1);
    // Walking Corpse: 2/2 — would survive 1 damage without deathtouch.
    let corpse = inject_creature_with_stats(&mut e, 1, "walking_corpse", 2, 2);

    e.apply_command(0, &declare_attackers(vec![chosen]))
        .expect("declare deathtouch attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: chosen,
            blocker_id: corpse,
        }]),
    )
    .expect("declare blocker");
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    let _ = e
        .apply_command(1, &pass())
        .expect("defender pass -> combat damage");

    // Both should be gone: Chosen takes 2 damage ≥ toughness 1; Corpse takes 1
    // deathtouch damage (lethal regardless of toughness).
    assert!(
        !e.state
            .objects
            .get(&chosen)
            .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield),
        "Pharika's Chosen should have died to 2 back-damage"
    );
    assert!(
        !e.state
            .objects
            .get(&corpse)
            .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield),
        "CR 702.2b: Walking Corpse must die to deathtouch even with 2 toughness"
    );
}

/// CR 702.2b: A deathtouch blocker destroys an attacker with higher toughness.
/// Walking Corpse (2/2) attacks → Pharika's Chosen (1/1 Deathtouch) blocks.
/// Chosen deals 1 deathtouch damage to the Corpse (lethal). Corpse deals 2
/// damage to Chosen (also lethal at 1 toughness). Both die.
#[test]
fn deathtouch_blocker_kills_attacker_with_higher_toughness() {
    let mut e = GameEngine::new(9041, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    let corpse = inject_creature_with_stats(&mut e, 0, "walking_corpse", 2, 2);
    let chosen = inject_creature_with_stats(&mut e, 1, "pharikas_chosen", 1, 1);

    e.apply_command(0, &declare_attackers(vec![corpse]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: corpse,
            blocker_id: chosen,
        }]),
    )
    .expect("declare deathtouch blocker");
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    let _ = e
        .apply_command(1, &pass())
        .expect("defender pass -> combat damage");

    assert!(
        !e.state
            .objects
            .get(&chosen)
            .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield),
        "Pharika's Chosen should die to 2 damage (toughness 1)"
    );
    assert!(
        !e.state
            .objects
            .get(&corpse)
            .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield),
        "CR 702.2b: Walking Corpse must die to deathtouch blocker's 1 damage"
    );
}

/// CR 702.2b (non-deathtouch control): a non-deathtouch 1-power attacker deals
/// 1 damage to a 2-toughness blocker — the blocker does NOT die.
/// Without deathtouch, 1 damage is not enough to kill a 2/2.
#[test]
fn non_deathtouch_one_power_does_not_kill_two_toughness_blocker() {
    let mut e = GameEngine::new(9042, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    // Raging Goblin: 1/1, Haste — no deathtouch.
    let goblin = inject_creature_with_stats(&mut e, 0, "raging_goblin", 1, 1);
    let corpse = inject_creature_with_stats(&mut e, 1, "walking_corpse", 2, 2);

    e.apply_command(0, &declare_attackers(vec![goblin]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: goblin,
            blocker_id: corpse,
        }]),
    )
    .expect("declare blocker");
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    let _ = e
        .apply_command(1, &pass())
        .expect("defender pass -> combat damage");

    // Goblin (1/1) dies to 2 back-damage. Corpse (2/2) takes only 1 non-deathtouch
    // damage — survives with 1 marked damage remaining.
    assert!(
        !e.state
            .objects
            .get(&goblin)
            .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield),
        "Raging Goblin should die to 2 back-damage"
    );
    assert!(
        e.state
            .objects
            .get(&corpse)
            .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield),
        "Walking Corpse should survive 1 non-deathtouch damage (toughness 2)"
    );
    assert_eq!(
        e.state.objects[&corpse].damage, 1,
        "Corpse should have 1 marked damage from the non-deathtouch hit"
    );
}

// ── Menace Keyword Tests ──────────────────────────────────────────────────────
//
// Tests for CR 702.111: a creature with menace can't be blocked except by two
// or more creatures. A single blocker is illegal; zero blockers (unblocked) is
// always fine; two or more blockers is legal.

/// CR 702.111 illegal path: attempting to block a menace creature with exactly
/// one blocker must be rejected with "Illegal blocks."
/// Goblin Trailblazer (2/1 Menace) is attacked; P1 tries to block with a single
/// Grizzly Bears — the engine must reject the declaration, leaving game state in
/// DeclareBlockers so the defender can correct their blocks.
#[test]
fn menace_single_blocker_is_illegal() {
    let mut e = GameEngine::new(9050, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    let trailblazer = inject_creature_on_battlefield(&mut e, 0, "goblin_trailblazer");
    // Give P1 two blockers so the engine won't auto-skip — the defender will
    // manually submit an illegal single-blocker declaration.
    let bears1 = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let _bears2 = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    e.apply_command(0, &declare_attackers(vec![trailblazer]))
        .expect("declare menace attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");

    // One blocker on a menace attacker — must be rejected.
    let err = e
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: trailblazer,
                blocker_id: bears1,
            }]),
        )
        .expect_err("single blocker on menace must be rejected");
    assert_eq!(
        err.to_string(),
        "illegal command: Illegal blocks.",
        "error must say 'Illegal blocks.', got: {err}"
    );
    // Game must NOT have advanced — still in DeclareBlockers, blockers_declared still false.
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers,
        "game must stay in DeclareBlockers after illegal menace block"
    );
    assert!(
        !e.state.combat.as_ref().unwrap().blockers_declared,
        "blockers_declared must remain false after illegal menace block"
    );
}

/// CR 702.111 happy path: a menace creature may be blocked by two or more creatures.
/// Goblin Trailblazer blocked by two Grizzly Bears — must succeed.
#[test]
fn menace_two_blockers_is_legal() {
    let mut e = GameEngine::new(9051, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    let trailblazer = inject_creature_on_battlefield(&mut e, 0, "goblin_trailblazer");
    let bears1 = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let bears2 = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    e.apply_command(0, &declare_attackers(vec![trailblazer]))
        .expect("declare menace attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");

    // Two blockers on a menace attacker — must be accepted.
    e.apply_command(
        1,
        &declare_blockers(vec![
            BlockPair {
                attacker_id: trailblazer,
                blocker_id: bears1,
            },
            BlockPair {
                attacker_id: trailblazer,
                blocker_id: bears2,
            },
        ]),
    )
    .expect("two blockers on a menace creature must be legal");
    assert!(
        e.state.combat.as_ref().unwrap().blockers_declared,
        "blockers_declared must be true after legal two-blocker menace block"
    );
}

/// CR 702.111 unblocked case: a menace creature that is not blocked at all is
/// perfectly legal — menace only restricts how it *can* be blocked, not whether
/// the defending player is forced to block it.
/// Two defender creatures are needed here so that `defending_player_has_eligible_blockers`
/// returns true (both can legally co-block the menace attacker), which keeps the engine
/// in the manual declare-blockers step where we can submit an empty declaration.
#[test]
fn menace_unblocked_is_legal() {
    let mut e = GameEngine::new(9052, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    let trailblazer = inject_creature_on_battlefield(&mut e, 0, "goblin_trailblazer");
    // Two defender creatures: together they could legally co-block the menace attacker, so the
    // engine keeps the manual step open. The defender then voluntarily declares no blocks.
    let _bears1 = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let _bears2 = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    e.apply_command(0, &declare_attackers(vec![trailblazer]))
        .expect("declare menace attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");

    // Empty blocker declaration — menace creature goes unblocked, which is always fine.
    e.apply_command(1, &declare_blockers(vec![]))
        .expect("empty blockers (menace unblocked) must be legal");
    assert!(
        e.state.combat.as_ref().unwrap().blockers_declared,
        "blockers_declared must be true after legal empty block"
    );
}

/// CR 702.111 auto-skip: when every attacker has menace and the defender has only one
/// creature, no legal non-empty blocking assignment exists (a single creature can't
/// satisfy the 2-blocker minimum alone). The engine must auto-declare empty blockers
/// and skip the manual declare-blockers step, exactly as it does for flying evasion.
#[test]
fn menace_single_creature_auto_skips_blockers() {
    let mut e = GameEngine::new(9053, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    let trailblazer = inject_creature_on_battlefield(&mut e, 0, "goblin_trailblazer");
    // Only one defender creature — cannot satisfy menace's 2-blocker requirement alone.
    let _bears = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    e.apply_command(0, &declare_attackers(vec![trailblazer]))
        .expect("declare menace attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");

    // P1 passes — engine detects no eligible blockers (menace prevents single-blocker block)
    // and must auto-declare empty blockers, just like flying when no reach/flyers exist.
    let b = e
        .apply_command(1, &pass())
        .expect("defender pass → auto-skip blockers");

    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareBlockers,
        "should enter DeclareBlockers after menace auto-skip"
    );
    let bd = blockers_declared_in(&b);
    assert_eq!(
        bd.len(),
        1,
        "exactly one BlockersDeclared event from auto-skip"
    );
    assert!(
        bd[0].block_pairs.is_empty(),
        "auto-skipped block must be empty (menace creature goes unblocked)"
    );
    assert!(
        e.state.combat.as_ref().unwrap().blockers_declared,
        "blockers_declared must be set after menace auto-skip"
    );
}

// ── Trample scenarios ─────────────────────────────────────────────────────────

/// Helper: advance a Colossal Dreadmaw (6/6 Trample) to the assign-combat-damage phase
/// with one blocker (grizzly_bears 2/2). Returns (engine, attacker_oid, blocker_oid).
fn setup_trample_single_blocker_assign_phase() -> (GameEngine, u32, u32) {
    let decks = Some(vec![
        std::iter::repeat_n("colossal_dreadmaw".to_string(), 10).collect::<Vec<_>>(),
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
    ]);
    let mut e = GameEngine::new(5001, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);
    ensure_in_hand(&mut e, 0, "colossal_dreadmaw");
    ensure_in_hand(&mut e, 1, "grizzly_bears");
    let attacker = put_creature_on_battlefield(&mut e, 0, "colossal_dreadmaw");
    let blocker = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare dreadmaw attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");

    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: attacker,
            blocker_id: blocker,
        }]),
    )
    .expect("declare single blocker");

    // Trample + 1 blocker must require explicit assignment.
    assert!(
        e.state.combat.as_ref().unwrap().damage_assignment_needed,
        "damage_assignment_needed must be true for trample+single-blocker"
    );

    // Pass priority in declare-blockers to open assign-combat-damage phase.
    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    e.apply_command(1, &pass())
        .expect("defender pass → assign phase");
    assert!(
        e.state.combat.as_ref().unwrap().assign_combat_damage_phase,
        "assign_combat_damage_phase must be open"
    );
    (e, attacker, blocker)
}

#[test]
fn trample_single_blocker_damage_assignment_needed() {
    // Verify that a trample attacker with a single blocker sets damage_assignment_needed
    // (unlike a non-trample single-blocker which proceeds automatically).
    setup_trample_single_blocker_assign_phase();
}

#[test]
fn trample_single_blocker_lethal_plus_excess_to_player() {
    // Colossal Dreadmaw (6/6 Trample) vs Grizzly Bears (2/2).
    // Legal assignment: 2 to blocker (lethal), 4 to player.
    // Blocker dies (2 damage = toughness). Player takes 4 trample damage.
    let (mut e, attacker, blocker) = setup_trample_single_blocker_assign_phase();
    let p1_life_before = e.state.players[1].life;

    let b = e
        .apply_command(
            0,
            &assign_combat_damage_cmd_with_player(attacker, vec![(blocker, 2)], 4),
        )
        .expect("assign 2 to blocker + 4 to player");

    let dead: Vec<u32> = permanents_moved_in(&b)
        .iter()
        .map(|p| p.object_id)
        .collect();

    // Blocker (2/2) receives 2 lethal → dies.
    assert!(
        dead.contains(&blocker),
        "blocker dies from lethal damage: {dead:?}"
    );
    // Attacker (6/6) receives 2 damage from blocker but survives (toughness 6).
    let att_obj = e.state.objects.get(&attacker);
    // Attacker may still be alive (6 toughness vs 2 damage); just confirm it's not in dead list.
    assert!(
        !dead.contains(&attacker),
        "dreadmaw survives (toughness 6 vs blocker power 2): {dead:?}"
    );

    // Defending player takes 4 trample damage.
    let life_evs = life_changes_in(&b);
    assert!(
        life_evs
            .iter()
            .any(|lc| lc.player_id == 1 && lc.delta == -4),
        "player must take 4 trample damage: {life_evs:?}"
    );
    assert_eq!(
        e.state.players[1].life,
        p1_life_before - 4,
        "player life after trample"
    );
    // Attacker stat: blocker dealt 2 power back
    if let Some(att_o) = att_obj {
        assert_eq!(
            att_o.damage, 2,
            "dreadmaw has 2 marked damage from the blocker"
        );
    }
    assert!(e.state.combat.is_none(), "combat cleared after resolution");
}

#[test]
fn trample_rejects_less_than_lethal_to_blocker() {
    // CR 702.19b: must assign >= lethal damage to each blocker before trample to player.
    // Colossal Dreadmaw (6/6) vs Grizzly Bears (2/2): assigning only 1 to blocker (< lethal 2) is illegal.
    let (mut e, attacker, blocker) = setup_trample_single_blocker_assign_phase();
    let err = e.apply_command(
        0,
        &assign_combat_damage_cmd_with_player(attacker, vec![(blocker, 1)], 5),
    );
    assert!(
        err.is_err(),
        "assigning less than lethal to blocker must be rejected"
    );
    // State remains in assign phase for retry.
    assert!(e.state.combat.as_ref().unwrap().assign_combat_damage_phase);
    assert!(e.state.combat.as_ref().unwrap().damage_assignment_needed);
}

#[test]
fn trample_rejects_sum_mismatch() {
    // Colossal Dreadmaw (6/6) vs Grizzly Bears (2/2).
    // 2 to blocker + 3 to player = 5 ≠ 6 (power): must be rejected.
    let (mut e, attacker, blocker) = setup_trample_single_blocker_assign_phase();
    assert!(
        e.apply_command(
            0,
            &assign_combat_damage_cmd_with_player(attacker, vec![(blocker, 2)], 3),
        )
        .is_err(),
        "blocker + player damage not equal to attacker power must be rejected"
    );
}

#[test]
fn trample_rejects_player_damage_without_trample() {
    // Non-trample multi-blocked attacker (Grizzly Bears 2/2) must not accept defending_player_damage > 0.
    let (mut e, attacker, a, b) = setup_two_blockers_assign_phase(5010);
    assert!(
        e.apply_command(
            0,
            &assign_combat_damage_cmd_with_player(attacker, vec![(a, 1), (b, 0)], 1),
        )
        .is_err(),
        "player damage on non-trample attacker must be rejected"
    );
}

#[test]
fn trample_all_damage_to_blocker_zero_to_player() {
    // Colossal Dreadmaw (6/6 Trample) vs Grizzly Bears (2/2).
    // It is legal to assign all 6 damage to the single blocker (0 to player), provided
    // lethal is still met (6 >= 2). Blocker dies, player takes 0.
    let (mut e, attacker, blocker) = setup_trample_single_blocker_assign_phase();
    let p1_life_before = e.state.players[1].life;

    let b = e
        .apply_command(
            0,
            &assign_combat_damage_cmd_with_player(attacker, vec![(blocker, 6)], 0),
        )
        .expect("assign all 6 to blocker, 0 to player");

    let dead: Vec<u32> = permanents_moved_in(&b)
        .iter()
        .map(|p| p.object_id)
        .collect();
    assert!(dead.contains(&blocker), "blocker dies: {dead:?}");
    assert_eq!(
        e.state.players[1].life, p1_life_before,
        "player takes 0 trample damage when all assigned to blocker"
    );
    assert!(
        life_changes_in(&b)
            .iter()
            .all(|lc| lc.delta >= 0 || lc.player_id != 1),
        "no negative life event for defending player"
    );
}

#[test]
fn trample_multi_blocked_excess_to_player() {
    // Colossal Dreadmaw (6/6 Trample) vs two Grizzly Bears (2/2 each).
    // Legal: assign 2 to first blocker (lethal), 2 to second (lethal), 2 to player.
    // Both blockers die; player takes 2.
    let decks = Some(vec![
        std::iter::repeat_n("colossal_dreadmaw".to_string(), 10).collect::<Vec<_>>(),
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
    ]);
    let mut e = GameEngine::new(5020, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);
    ensure_in_hand(&mut e, 0, "colossal_dreadmaw");
    ensure_in_hand(&mut e, 1, "grizzly_bears");
    let attacker = put_creature_on_battlefield(&mut e, 0, "colossal_dreadmaw");
    let b1 = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let b2 = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("active pass declare attackers");
    e.apply_command(1, &pass())
        .expect("defender pass declare attackers");

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

    e.apply_command(0, &pass())
        .expect("active pass declare blockers");
    e.apply_command(1, &pass())
        .expect("defender pass → assign phase");

    let p1_life_before = e.state.players[1].life;
    let batch = e
        .apply_command(
            0,
            &assign_combat_damage_cmd_with_player(attacker, vec![(b1, 2), (b2, 2)], 2),
        )
        .expect("assign 2+2+2 trample");

    let dead: Vec<u32> = permanents_moved_in(&batch)
        .iter()
        .map(|p| p.object_id)
        .collect();
    assert!(dead.contains(&b1), "b1 dies: {dead:?}");
    assert!(dead.contains(&b2), "b2 dies: {dead:?}");
    assert!(
        !dead.contains(&attacker),
        "dreadmaw survives (6 toughness vs 2+2 blocker power): {dead:?}"
    );

    let life_evs = life_changes_in(&batch);
    assert!(
        life_evs
            .iter()
            .any(|lc| lc.player_id == 1 && lc.delta == -2),
        "defending player takes 2 trample damage: {life_evs:?}"
    );
    assert_eq!(e.state.players[1].life, p1_life_before - 2);
    assert!(e.state.combat.is_none(), "combat cleared");
}

#[test]
fn trample_multi_blocked_rejects_less_than_lethal_to_first_blocker() {
    // Colossal Dreadmaw (6/6 Trample) vs two Grizzly Bears (2/2 each).
    // Assign 1 to first blocker (< lethal 2): must be rejected.
    let decks = Some(vec![
        std::iter::repeat_n("colossal_dreadmaw".to_string(), 10).collect::<Vec<_>>(),
        std::iter::repeat_n("grizzly_bears".to_string(), 10).collect::<Vec<_>>(),
    ]);
    let mut e = GameEngine::new(5021, &[0, 1], 20, decks, true).expect("new");
    advance_to_declare_attackers(&mut e);
    ensure_in_hand(&mut e, 0, "colossal_dreadmaw");
    ensure_in_hand(&mut e, 1, "grizzly_bears");
    let attacker = put_creature_on_battlefield(&mut e, 0, "colossal_dreadmaw");
    let b1 = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    let b2 = put_creature_on_battlefield(&mut e, 1, "grizzly_bears");

    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare");
    e.apply_command(0, &pass()).expect("pass");
    e.apply_command(1, &pass()).expect("pass");
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
    e.apply_command(0, &pass()).expect("pass");
    e.apply_command(1, &pass()).expect("pass");

    // 1 to first blocker < lethal (2): illegal.
    assert!(
        e.apply_command(
            0,
            &assign_combat_damage_cmd_with_player(attacker, vec![(b1, 1), (b2, 2)], 3),
        )
        .is_err(),
        "must reject when first blocker receives less than lethal"
    );
}

// CR 510.4 + 702.7 + 702.4: first strike / double strike — these tests verify the engine
// splits combat damage into a first-strike substep when applicable, and that participation
// follows the CR 510.4 rule (FirstStrike/DoubleStrike in first step; remainder + DoubleStrike
// in regular step). The pass-priority button label change is verified separately in the C++
// prompt-widget code path (the engine emits `first_strike_step_pending` via zone view).

/// CR 702.7 (first strike) + CR 510.4: a first-strike attacker kills a vanilla blocker in
/// the first-strike step (CR 510.2 SBAs run between steps) and takes no return damage in the
/// regular step. Goblin Striker (1/1 FS+Haste) attacks; defender blocks with Walking Corpse
/// (2/2). Walking Corpse has only 2 toughness — but the attacker only has power 1, so the
/// corpse survives if there's no first strike. With first strike: Goblin Striker deals 1 in
/// first-strike step (corpse marked but not lethal — toughness 2), then in the regular step
/// the corpse deals 2 back. The corpse survives at 1 toughness, goblin dies. This documents
/// the *order* and shows the first-strike step ran (corpse remained alive without taking
/// return damage from goblin a second time).
#[test]
fn first_strike_attacker_against_vanilla_blocker_survives_or_dies_per_pt() {
    let mut e = GameEngine::new(11_001, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    let goblin = inject_creature_with_stats(&mut e, 0, "goblin_striker", 1, 1);
    let corpse = inject_creature_with_stats(&mut e, 1, "walking_corpse", 2, 2);

    e.apply_command(0, &declare_attackers(vec![goblin]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("ap pass dec atk");
    e.apply_command(1, &pass()).expect("def pass dec atk");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: goblin,
            blocker_id: corpse,
        }]),
    )
    .expect("declare blocker");

    // Both players pass priority in declare blockers → engine enters first-strike substep.
    e.apply_command(0, &pass()).expect("ap pass dec blk");
    e.apply_command(1, &pass()).expect("def pass dec blk");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::FirstStrikeDamage,
        "engine must enter FirstStrikeDamage substep when an attacker has First Strike"
    );

    // Goblin is alive (corpse hasn't dealt damage yet); corpse has 1 marked damage.
    assert!(
        e.state
            .objects
            .get(&goblin)
            .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield),
        "goblin must be alive entering the regular damage step"
    );
    assert_eq!(
        e.state.objects.get(&corpse).map(|o| o.damage),
        Some(1),
        "corpse must have 1 marked damage from first-strike"
    );

    // Pass priority in first-strike step → engine runs regular combat damage step.
    e.apply_command(0, &pass()).expect("ap pass fs damage");
    e.apply_command(1, &pass()).expect("def pass fs damage");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::CombatDamage,
        "engine must reach CombatDamage after the regular damage step resolves"
    );
    // Corpse deals 2 to goblin in regular step → goblin dies. Corpse keeps its 1 damage.
    assert!(
        !e.state
            .objects
            .get(&goblin)
            .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield),
        "goblin must die to corpse's 2 return damage in the regular step"
    );
    assert!(
        e.state
            .objects
            .get(&corpse)
            .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield),
        "corpse must survive with 1 marked damage (toughness 2)"
    );
}

/// CR 702.4 (double strike): a double-strike attacker deals damage in both steps. Fencing Ace
/// (1/1 double strike) attacks → Grizzly Bears (2/2 vanilla) blocks. First-strike step: Ace
/// deals 1, Bears marked. Bears die not yet (toughness 2). Regular step: both deal damage —
/// Ace deals another 1 (Bears now at 2 → dies), Bears deal 2 (Ace dies). Both die.
#[test]
fn double_strike_attacker_against_vanilla_blocker_both_die() {
    let mut e = GameEngine::new(11_002, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    let ace = inject_creature_with_stats(&mut e, 0, "fencing_ace", 1, 1);
    let bears = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);

    e.apply_command(0, &declare_attackers(vec![ace]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("ap pass dec atk");
    e.apply_command(1, &pass()).expect("def pass dec atk");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: ace,
            blocker_id: bears,
        }]),
    )
    .expect("declare blocker");
    e.apply_command(0, &pass()).expect("ap pass dec blk");
    e.apply_command(1, &pass()).expect("def pass dec blk");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::FirstStrikeDamage,
        "double strike triggers the first-strike substep"
    );
    // After first strike: bears at 1 damage, ace alive (bears haven't swung yet).
    assert_eq!(e.state.objects.get(&bears).map(|o| o.damage), Some(1));
    assert!(e
        .state
        .objects
        .get(&ace)
        .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield));

    e.apply_command(0, &pass()).expect("ap pass fs damage");
    e.apply_command(1, &pass()).expect("def pass fs damage");
    // Regular step: ace deals another 1 (bears die at 2 damage = toughness), bears deal 2 (ace dies).
    assert!(
        !e.state
            .objects
            .get(&ace)
            .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield),
        "ace must die to bears' return damage"
    );
    assert!(
        !e.state
            .objects
            .get(&bears)
            .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield),
        "bears must die to ace's two damage instances"
    );
}

/// CR 510.4: when no attacker or blocker has FirstStrike/DoubleStrike, the engine skips the
/// first-strike substep — DeclareBlockers transitions directly to CombatDamage, just like
/// vanilla combat before this change.
#[test]
fn vanilla_combat_skips_first_strike_step() {
    let mut e = GameEngine::new(11_003, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let bears = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let corpse = inject_creature_with_stats(&mut e, 1, "walking_corpse", 2, 2);
    e.apply_command(0, &declare_attackers(vec![bears]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("ap pass dec atk");
    e.apply_command(1, &pass()).expect("def pass dec atk");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: bears,
            blocker_id: corpse,
        }]),
    )
    .expect("declare blocker");
    e.apply_command(0, &pass()).expect("ap pass dec blk");
    e.apply_command(1, &pass()).expect("def pass dec blk");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::CombatDamage,
        "vanilla combat must skip FirstStrikeDamage and go straight to CombatDamage"
    );
    // Both 2/2s trade as before.
    assert!(!e
        .state
        .objects
        .get(&bears)
        .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield));
    assert!(!e
        .state
        .objects
        .get(&corpse)
        .is_some_and(|o| o.zone == tricerules_core::Zone::Battlefield));
}

/// CR 510.4: when a first-strike attacker is unblocked, the defending player takes damage in
/// the first-strike step. Verified via the LifeChanged event and player life total.
#[test]
fn first_strike_unblocked_deals_damage_in_first_strike_step() {
    let mut e = GameEngine::new(11_004, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let goblin = inject_creature_with_stats(&mut e, 0, "goblin_striker", 1, 1);
    e.apply_command(0, &declare_attackers(vec![goblin]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("ap pass dec atk");
    e.apply_command(1, &pass()).expect("def pass dec atk");
    // No blockers: engine auto-declares empty blockers then awaits passes.
    e.apply_command(0, &pass()).expect("ap pass dec blk");
    e.apply_command(1, &pass()).expect("def pass dec blk");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::FirstStrikeDamage,
        "first-strike substep must run for an unblocked FS attacker"
    );
    let life_after_fs = e.state.players[1].life;
    assert_eq!(
        life_after_fs, 19,
        "defender should take 1 damage in first-strike step"
    );
    e.apply_command(0, &pass()).expect("ap pass fs damage");
    e.apply_command(1, &pass()).expect("def pass fs damage");
    assert_eq!(
        e.state.players[1].life, 19,
        "regular step deals no extra damage for first strike"
    );
}

/// CR 702.4 double strike unblocked: deals damage in BOTH steps. Fencing Ace unblocked hits
/// the defender for 1 in first-strike step, then 1 more in the regular step → 2 total.
#[test]
fn double_strike_unblocked_deals_damage_in_both_steps() {
    let mut e = GameEngine::new(11_005, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let ace = inject_creature_with_stats(&mut e, 0, "fencing_ace", 1, 1);
    e.apply_command(0, &declare_attackers(vec![ace]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("ap pass dec atk");
    e.apply_command(1, &pass()).expect("def pass dec atk");
    e.apply_command(0, &pass()).expect("ap pass dec blk");
    e.apply_command(1, &pass()).expect("def pass dec blk");
    assert_eq!(e.state.players[1].life, 19, "first-strike step deals 1");
    e.apply_command(0, &pass()).expect("ap pass fs damage");
    e.apply_command(1, &pass()).expect("def pass fs damage");
    assert_eq!(
        e.state.players[1].life, 18,
        "double strike deals damage again in the regular step (total 2)"
    );
}

/// CR 510.4: the per-player zone view exposes `first_strike_step_pending=true` between
/// declare-attackers and the end of the first-strike step, so the client can show the
/// "First Strike Damage" pass-priority button label.
#[test]
fn zone_view_signals_first_strike_step_pending() {
    let mut e = GameEngine::new(11_006, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let goblin = inject_creature_with_stats(&mut e, 0, "goblin_striker", 1, 1);

    let b = e
        .apply_command(0, &declare_attackers(vec![goblin]))
        .expect("declare attacker");
    let zv = b
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::ZoneView(zv)) => Some(zv.clone()),
            _ => None,
        })
        .expect("zone view present");
    assert!(
        zv.per_player.iter().all(|p| p.first_strike_step_pending),
        "first_strike_step_pending must be true while a FS attacker is in combat"
    );
}

/// CR 510.4: `first_strike_step_pending` must remain true after blockers are declared (still
/// pre-resolution), so the declare-blockers pass-priority button stays labeled
/// "First Strike Damage" up until the substep actually resolves.
#[test]
fn zone_view_signals_pending_after_blockers_declared() {
    let mut e = GameEngine::new(11_007, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let goblin = inject_creature_with_stats(&mut e, 0, "goblin_striker", 1, 1);
    let corpse = inject_creature_with_stats(&mut e, 1, "walking_corpse", 2, 2);

    e.apply_command(0, &declare_attackers(vec![goblin]))
        .expect("declare attacker");
    e.apply_command(0, &pass()).expect("ap pass dec atk");
    e.apply_command(1, &pass()).expect("def pass dec atk");
    let b = e
        .apply_command(
            1,
            &declare_blockers(vec![BlockPair {
                attacker_id: goblin,
                blocker_id: corpse,
            }]),
        )
        .expect("declare blockers");
    let zv = b
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::ZoneView(zv)) => Some(zv.clone()),
            _ => None,
        })
        .expect("zone view present");
    assert!(
        zv.per_player.iter().all(|p| p.first_strike_step_pending),
        "pending must stay true after blockers declared (mixed FS attacker + vanilla blocker)"
    );

    // And it must flip to false once the FS substep resolves.
    e.apply_command(0, &pass()).expect("ap pass dec blk");
    let b2 = e.apply_command(1, &pass()).expect("def pass dec blk");
    let zv2 = b2
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::ZoneView(zv)) => Some(zv.clone()),
            _ => None,
        })
        .expect("zone view present");
    assert!(
        zv2.per_player.iter().all(|p| !p.first_strike_step_pending),
        "pending must flip false once the first-strike substep has resolved"
    );
}

/// CR 510.4: when no FS/DS creature is in combat, `first_strike_step_pending` is never true.
#[test]
fn zone_view_does_not_signal_pending_for_vanilla_combat() {
    let mut e = GameEngine::new(11_008, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);
    let bears = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let b = e
        .apply_command(0, &declare_attackers(vec![bears]))
        .expect("declare attacker");
    let zv = b
        .events
        .iter()
        .find_map(|ev| match &ev.ev {
            Some(Ev::ZoneView(zv)) => Some(zv.clone()),
            _ => None,
        })
        .expect("zone view present");
    assert!(
        zv.per_player.iter().all(|p| !p.first_strike_step_pending),
        "pending must stay false in vanilla combat (no FS/DS combatants)"
    );
}

// ---------------------------------------------------------------------------
// CR 702.12 — Indestructible
// ---------------------------------------------------------------------------

/// CR 702.12b: A "destroy" spell has no effect on an indestructible permanent.
/// Darksteel Myr survives Murder (used here because Murder has no targeting restrictions
/// that will need future enforcement; Go for the Throat can't target artifacts).
#[test]
fn indestructible_survives_destroy_spell() {
    let decks = Some(vec![
        vec![
            "swamp".into(),
            "murder".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
            "swamp".into(),
        ],
        vec![
            "mountain".into(),
            "darksteel_myr".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
    ]);
    let mut e = GameEngine::new(7001, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let myr = put_creature_on_battlefield(&mut e, 1, "darksteel_myr");

    // Give P0 three black mana for Murder (1BB): seed two swamps and play a third.
    for _ in 0..2 {
        let idx = hand_index_for_card(&e, 0, "swamp");
        let oid = e.state.players[0].hand.remove(idx);
        e.state.players[0].battlefield.push(oid);
        e.state.objects.get_mut(&oid).unwrap().zone = tricerules_core::Zone::Battlefield;
    }
    let swamp_idx = hand_index_for_card(&e, 0, "swamp");
    e.apply_command(0, &play_land(swamp_idx))
        .expect("play swamp");
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            b: 2,
            c: 1,
            ..Default::default()
        }),
    )
    .expect("add mana");

    let murder_idx = hand_index_for_card(&e, 0, "murder");
    e.apply_command(
        0,
        &cast_spell(murder_idx, vec![TargetRef { object_id: myr }]),
    )
    .expect("cast murder");
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");

    // Myr must still be on the battlefield.
    assert_eq!(
        e.state.objects.get(&myr).expect("myr object").zone,
        tricerules_core::Zone::Battlefield,
        "CR 702.12b: indestructible Myr must not be destroyed by Go for the Throat"
    );
    assert!(
        !e.state.players[1].graveyard.contains(&myr),
        "Myr must not be in graveyard"
    );
}

/// CR 702.12b: An indestructible creature is not destroyed by lethal combat damage.
/// Darksteel Myr (0/1 indestructible) blocks a 5/5; it takes lethal damage but stays.
#[test]
fn indestructible_survives_lethal_combat_damage() {
    let mut e = GameEngine::new(7002, &[0, 1], 20, None, true).expect("new");
    advance_to_declare_attackers(&mut e);

    let attacker = inject_creature_with_stats(&mut e, 0, "colossal_dreadmaw", 6, 6);
    let myr = inject_creature_with_stats(&mut e, 1, "darksteel_myr", 0, 1);

    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_command(0, &pass())
        .expect("p0 pass after attackers");
    e.apply_command(1, &pass())
        .expect("p1 pass after attackers");
    e.apply_command(
        1,
        &declare_blockers(vec![BlockPair {
            attacker_id: attacker,
            blocker_id: myr,
        }]),
    )
    .expect("declare blocker");
    e.apply_command(0, &pass()).expect("p0 pass after blockers");
    e.apply_command(1, &pass())
        .expect("p1 pass after blockers -> combat damage resolves");

    assert_eq!(
        e.state.objects.get(&myr).expect("myr object").zone,
        tricerules_core::Zone::Battlefield,
        "CR 702.12b: indestructible Myr must survive lethal combat damage"
    );
}

/// CR 704.5f + CR 702.12b: indestructible does NOT protect against toughness dropping to 0.
/// A -0/-1 pump on a 0/1 Darksteel Myr brings toughness to 0; SBA kills it.
#[test]
fn indestructible_dies_when_toughness_reaches_zero() {
    let mut e = GameEngine::new(7003, &[0, 1], 20, None, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Inject Darksteel Myr (0/1) directly onto P1's battlefield.
    let myr = inject_creature_with_stats(&mut e, 1, "darksteel_myr", 0, 1);

    // Manually set toughness to 0 to simulate a -0/-1 effect, then trigger SBAs via
    // a pass-priority sequence (SBAs run at the start of each priority check).
    e.state.objects.get_mut(&myr).unwrap().toughness = Some(0);

    // Pass priority — SBAs fire on the next priority check.
    e.apply_command(0, &pass()).expect("p0 pass");
    e.apply_command(1, &pass()).expect("p1 pass");

    assert_eq!(
        e.state.objects.get(&myr).expect("myr object").zone,
        tricerules_core::Zone::Graveyard,
        "CR 704.5f: indestructible Myr with toughness 0 must still die"
    );
}

/// CR 603.3b: when multiple triggered abilities fire from the same event (simultaneous triggers),
/// all of them must go on the stack — not just the first one.  Regression test for the old
/// `Option<PendingTrigger>` design that silently dropped any trigger after the first.
///
/// Scenario: two Scroll Thieves (WheneverSelfDealsCombatDamageToPlayer → DrawCards(1)) both
/// attack an unblocked player. Both triggers must fire, resulting in two cards drawn.
#[test]
fn simultaneous_combat_damage_triggers_both_fire() {
    // 12-card library so drawing 7 opening hand + 2 triggers still has cards remaining.
    let p0_deck: Vec<String> = vec![
        "scroll_thief".into(),
        "scroll_thief".into(),
        "island".into(),
        "island".into(),
        "island".into(),
        "island".into(),
        "island".into(),
        "island".into(),
        "island".into(),
        "island".into(),
        "island".into(),
        "island".into(),
    ];
    let p1_deck: Vec<String> = std::iter::repeat_n("mountain".into(), 12).collect();
    let decks = Some(vec![p0_deck, p1_deck]);
    let mut e = GameEngine::new(8001, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Cheat both Scroll Thieves onto P0's battlefield without summoning sickness.
    let thief1_hand_idx = hand_index_for_card(&e, 0, "scroll_thief");
    let thief1_oid = e.state.players[0].hand.remove(thief1_hand_idx);
    e.state.players[0].battlefield.push(thief1_oid);
    if let Some(obj) = e.state.objects.get_mut(&thief1_oid) {
        obj.zone = tricerules_core::Zone::Battlefield;
        obj.summoning_sick = false;
    }

    let thief2_hand_idx = hand_index_for_card(&e, 0, "scroll_thief");
    let thief2_oid = e.state.players[0].hand.remove(thief2_hand_idx);
    e.state.players[0].battlefield.push(thief2_oid);
    if let Some(obj) = e.state.objects.get_mut(&thief2_oid) {
        obj.zone = tricerules_core::Zone::Battlefield;
        obj.summoning_sick = false;
    }

    let p0_hand_before = e.state.players[0].hand.len();

    // Enter combat and attack with both Scroll Thieves.
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap begin combat pass");
    e.apply_command(1, &pass()).expect("nap begin combat pass");
    e.apply_command(0, &declare_attackers(vec![thief1_oid, thief2_oid]))
        .expect("declare attackers");
    e.apply_command(0, &pass())
        .expect("ap pass after attackers");
    e.apply_command(1, &pass())
        .expect("nap pass after attackers");
    // P1 has no creatures; engine auto-declares empty blockers.
    e.apply_command(0, &pass())
        .expect("ap pass declare blockers");
    e.apply_command(1, &pass())
        .expect("nap pass declare blockers -> combat damage");

    // Both Scroll Thieves dealt combat damage to P1, so both triggers must be on the stack.
    assert_eq!(
        e.state.stack.len(),
        2,
        "both Scroll Thief triggers must be on the stack simultaneously"
    );

    // Resolving both triggers draws 2 cards for P0.
    resolve_entire_stack_two_player(&mut e);
    assert!(
        e.state.stack.is_empty(),
        "stack must be empty after both triggers resolve"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        p0_hand_before + 2,
        "two Scroll Thief triggers must draw two cards total"
    );
}

/// Regression: trigger queue serializes correctly when a single targeted trigger needs target
/// selection. After `choose_trigger_target` the queue should be empty and priority returned.
/// This guards against regressions in the queue-based `choose_trigger_target` path.
#[test]
fn targeted_trigger_resolves_after_target_chosen() {
    // Thieving Magpie: WheneverSelfDealsCombatDamageToPlayer → DrawCards(1) (no target needed).
    // Use a card whose trigger DOES need a target so choose_trigger_target is exercised.
    // Flametongue Kavu: WhenSelfEntersBattlefield → DamageTarget(4) — targeted ETB trigger.
    let p0_deck: Vec<String> = vec![
        "flametongue_kavu".into(),
        "mountain".into(),
        "mountain".into(),
        "mountain".into(),
        "mountain".into(),
        "mountain".into(),
        "mountain".into(),
        "mountain".into(),
        "mountain".into(),
        "mountain".into(),
        "mountain".into(),
    ];
    let p1_deck: Vec<String> = vec![
        "grizzly_bears".into(),
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
    ];
    let decks = Some(vec![p0_deck, p1_deck]);
    let mut e = GameEngine::new(8003, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Get Grizzly Bears onto P1's battlefield (cheat it in).
    let bears_oid = {
        let pos = e.state.players[1]
            .hand
            .iter()
            .position(|oid| {
                e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some("grizzly_bears")
            })
            .expect("grizzly_bears in P1 hand");
        let oid = e.state.players[1].hand.remove(pos);
        e.state.players[1].battlefield.push(oid);
        e.state.objects.get_mut(&oid).expect("obj").zone = tricerules_core::Zone::Battlefield;
        oid
    };

    // Give P0 enough mana to cast Flametongue Kavu (3R = 3 colorless + 1 red).
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            c: 3,
            ..Default::default()
        }),
    )
    .ok();

    // Cast Flametongue Kavu (no target at cast time for its ETB trigger — target chosen later).
    let ftk_idx = hand_index_for_card(&e, 0, "flametongue_kavu");
    e.apply_command(0, &cast_spell(ftk_idx, vec![]))
        .expect("cast FTK");

    // Let it resolve (both players pass).
    pass_both_players(&mut e);

    // ETB trigger fires: DamageTarget(4) needs a target → pending_triggers should have 1 entry.
    assert_eq!(
        e.state.pending_triggers.len(),
        1,
        "FTK ETB trigger must be queued for target selection"
    );

    // P0 chooses Grizzly Bears as the target.
    e.apply_command(
        0,
        &RuledCommand {
            cmd: Some(Cmd::ChooseTriggerTarget(ChooseTriggerTarget {
                target_object_id: bears_oid,
            })),
        },
    )
    .expect("choose trigger target");

    // Queue must be empty and trigger on the stack.
    assert!(
        e.state.pending_triggers.is_empty(),
        "pending_triggers queue must be empty after target chosen"
    );
    assert_eq!(e.state.stack.len(), 1, "trigger must be on the stack");

    // Resolve the trigger: Grizzly Bears (2/2) takes 4 damage → dies.
    pass_both_players(&mut e);
    assert_eq!(
        e.state.objects.get(&bears_oid).expect("bears").zone,
        tricerules_core::Zone::Graveyard,
        "Grizzly Bears must die to Flametongue Kavu's ETB trigger"
    );
}

// ---------------------------------------------------------------------------
// Hexproof / Shroud (CR 702.18 / CR 702.16)
// ---------------------------------------------------------------------------

/// CR 702.18: Gladecover Scout has hexproof — an opponent cannot target it with
/// Lightning Bolt. The cast attempt must be rejected as illegal.
#[test]
fn hexproof_opponent_cannot_target_with_spell() {
    // 14-card decks so library is never empty after the opening hand + draw step.
    let p0_deck: Vec<String> = std::iter::once("lightning_bolt".into())
        .chain(std::iter::repeat_n("mountain".into(), 13))
        .collect();
    let p1_deck: Vec<String> = std::iter::repeat_n("mountain".into(), 14).collect();
    let decks = Some(vec![p0_deck, p1_deck]);
    let mut e = GameEngine::new(9001, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Inject Gladecover Scout (1/1 hexproof) directly onto P1's battlefield.
    let scout = inject_creature_with_stats(&mut e, 1, "gladecover_scout", 1, 1);

    // Give P0 one red mana (Lightning Bolt costs R) by tapping a Mountain.
    let mtn_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mtn_idx))
        .expect("play mountain");
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 1,
            ..Default::default()
        }),
    )
    .expect("add mana");

    // Ensure Lightning Bolt is in P0's hand (may be in library depending on seed).
    if !e.state.players[0]
        .hand
        .iter()
        .any(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some("lightning_bolt"))
    {
        take_card_from_library_to_hand(&mut e, 0, "lightning_bolt");
    }

    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    let result = e.apply_command(
        0,
        &cast_spell(bolt_idx, vec![TargetRef { object_id: scout }]),
    );
    assert!(
        result.is_err(),
        "CR 702.18: opponent must not be able to target a hexproof permanent with a spell"
    );
}

/// CR 702.18: a player CAN target their own hexproof permanent (hexproof only
/// protects against opponents). Giant Growth on your own Gladecover Scout is legal.
#[test]
fn hexproof_controller_can_target_own_permanent() {
    let p0_deck: Vec<String> = std::iter::once("giant_growth".into())
        .chain(std::iter::repeat_n("forest".into(), 13))
        .collect();
    let p1_deck: Vec<String> = std::iter::repeat_n("mountain".into(), 14).collect();
    let decks = Some(vec![p0_deck, p1_deck]);
    let mut e = GameEngine::new(9002, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Inject Gladecover Scout (1/1 hexproof) directly onto P0's battlefield.
    let scout = inject_creature_with_stats(&mut e, 0, "gladecover_scout", 1, 1);

    let forest_idx = hand_index_for_card(&e, 0, "forest");
    e.apply_command(0, &play_land(forest_idx))
        .expect("play forest");
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            g: 1,
            ..Default::default()
        }),
    )
    .expect("add mana");

    // Ensure Giant Growth is in P0's hand.
    if !e.state.players[0]
        .hand
        .iter()
        .any(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some("giant_growth"))
    {
        take_card_from_library_to_hand(&mut e, 0, "giant_growth");
    }

    let gg_idx = hand_index_for_card(&e, 0, "giant_growth");
    e.apply_command(0, &cast_spell(gg_idx, vec![TargetRef { object_id: scout }]))
        .expect("CR 702.18: controller can target own hexproof creature");

    // Resolve the pump.
    pass_both_players(&mut e);

    assert_eq!(
        e.effective_power(scout),
        Some(4),
        "Giant Growth (+3/+3) must pump Gladecover Scout from 1 to 4 effective power"
    );
}

/// CR 702.16: Argothian Enchantress has shroud — even its controller cannot
/// target it with a spell. Giant Growth targeting the Enchantress must be rejected.
#[test]
fn shroud_controller_cannot_target_own_permanent() {
    let p0_deck: Vec<String> = std::iter::once("giant_growth".into())
        .chain(std::iter::repeat_n("forest".into(), 13))
        .collect();
    let p1_deck: Vec<String> = std::iter::repeat_n("mountain".into(), 14).collect();
    let decks = Some(vec![p0_deck, p1_deck]);
    let mut e = GameEngine::new(9003, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Inject Argothian Enchantress (0/1 shroud) directly onto P0's battlefield.
    let enchantress = inject_creature_with_stats(&mut e, 0, "argothian_enchantress", 0, 1);

    let forest_idx = hand_index_for_card(&e, 0, "forest");
    e.apply_command(0, &play_land(forest_idx))
        .expect("play forest");
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            g: 1,
            ..Default::default()
        }),
    )
    .expect("add mana");

    // Ensure Giant Growth is in P0's hand.
    if !e.state.players[0]
        .hand
        .iter()
        .any(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some("giant_growth"))
    {
        take_card_from_library_to_hand(&mut e, 0, "giant_growth");
    }

    let gg_idx = hand_index_for_card(&e, 0, "giant_growth");
    let result = e.apply_command(
        0,
        &cast_spell(
            gg_idx,
            vec![TargetRef {
                object_id: enchantress,
            }],
        ),
    );
    assert!(
        result.is_err(),
        "CR 702.16: controller must not be able to target a shroud permanent with a spell"
    );
}

/// CR 603.2 + Argothian Enchantress: casting an enchantment spell triggers a draw.
/// Scenario: P0 controls an Argothian Enchantress; P0 casts Exploration (a green
/// enchantment). The trigger fires and puts a Draw(1) on the stack, which resolves
/// and increases P0's hand size by one.
#[test]
fn argothian_enchantress_triggers_on_enchantment_cast() {
    // 14-card decks so library is never empty after the opening hand + draw step.
    let p0_deck: Vec<String> = std::iter::once("exploration".into())
        .chain(std::iter::repeat_n("forest".into(), 13))
        .collect();
    let p1_deck: Vec<String> = std::iter::repeat_n("mountain".into(), 14).collect();
    let decks = Some(vec![p0_deck, p1_deck]);
    let mut e = GameEngine::new(9004, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    // Inject Argothian Enchantress onto P0's battlefield (no summoning sickness needed).
    inject_creature_with_stats(&mut e, 0, "argothian_enchantress", 0, 1);

    // Play a Forest for mana.
    let forest_idx = hand_index_for_card(&e, 0, "forest");
    e.apply_command(0, &play_land(forest_idx))
        .expect("play forest");
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            g: 1,
            ..Default::default()
        }),
    )
    .expect("add mana");

    // Ensure Exploration is in P0's hand.
    if !e.state.players[0]
        .hand
        .iter()
        .any(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some("exploration"))
    {
        take_card_from_library_to_hand(&mut e, 0, "exploration");
    }

    let hand_before = e.state.players[0].hand.len();

    let expl_idx = hand_index_for_card(&e, 0, "exploration");
    e.apply_command(0, &cast_spell(expl_idx, vec![]))
        .expect("cast Exploration");

    // Both Exploration and the enchantress draw trigger must be on the stack.
    assert!(
        e.state.stack.len() >= 2,
        "Exploration and its triggered Draw must both be on the stack"
    );

    // Resolve the draw trigger (top of stack) — P0 draws one card.
    // Net hand change: -1 for casting Exploration, +1 for trigger draw = 0 vs hand_before.
    pass_both_players(&mut e);
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before,
        "Argothian Enchantress trigger must draw exactly one card on enchantment cast (net hand = hand_before)"
    );

    // Resolve Exploration itself.
    pass_both_players(&mut e);
}

/// Royal Assassin's `{T}: Destroy target tapped creature.` — now a `DestroyTarget` with a
/// `tapped: true` filter (the old single-card `DestroyTargetTapped` primitive was removed).
/// Happy path: a tapped enemy creature is a legal target and is destroyed on resolution.
#[test]
fn royal_assassin_destroys_tapped_creature() {
    let decks = Some(vec![
        vec!["royal_assassin".into(); 20],
        vec!["grizzly_bears".into(); 20],
    ]);
    let mut e = GameEngine::new(4201, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let assassin = deploy_to_battlefield(&mut e, 0, "royal_assassin", false);
    let bears = deploy_to_battlefield(&mut e, 1, "grizzly_bears", /* tapped */ true);

    e.apply_command(
        0,
        &activate_ability(assassin, 0, vec![TargetRef { object_id: bears }]),
    )
    .expect("activate Royal Assassin on tapped creature");

    // Source taps to pay the cost; ability is on the stack.
    assert!(e.state.objects.get(&assassin).expect("assassin").tapped);
    assert_eq!(e.state.stack.len(), 1);

    // Both players pass → ability resolves, destroying the tapped creature.
    pass_both_players(&mut e);
    assert!(e.state.stack.is_empty());
    assert!(
        e.state.players[1].graveyard.contains(&bears),
        "tapped creature should be destroyed to its owner's graveyard"
    );
}

/// Illegal path: an untapped creature fails the `tapped: true` filter, so activation is
/// rejected at target validation (CR 602.2) before any cost is paid.
#[test]
fn royal_assassin_cannot_target_untapped_creature() {
    let decks = Some(vec![
        vec!["royal_assassin".into(); 20],
        vec!["grizzly_bears".into(); 20],
    ]);
    let mut e = GameEngine::new(4202, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let assassin = deploy_to_battlefield(&mut e, 0, "royal_assassin", false);
    let bears = deploy_to_battlefield(&mut e, 1, "grizzly_bears", /* tapped */ false);

    let err = e.apply_command(
        0,
        &activate_ability(assassin, 0, vec![TargetRef { object_id: bears }]),
    );
    assert!(err.is_err(), "untapped creature is not a legal target");
    // Cost untouched: source stays untapped, nothing on the stack.
    assert!(!e.state.objects.get(&assassin).expect("assassin").tapped);
    assert!(e.state.stack.is_empty());
}

/// Build a 20-card deck: `specials` (once each) plus enough `basic` lands to fill out.
fn deck_with(basic: &str, specials: &[&str]) -> Vec<String> {
    let mut d: Vec<String> = specials.iter().map(|s| s.to_string()).collect();
    while d.len() < 20 {
        d.push(basic.to_string());
    }
    d
}

/// Remove the first object of `card_id` from `player`'s library or hand (wherever it landed
/// after the random opening draw) and return its id. Lets a test place specific cards without
/// depending on shuffle order.
fn take_oid_from_library_or_hand(e: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    if let Some(pos) = e.state.players[player]
        .library
        .iter()
        .position(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some(card_id))
    {
        return e.state.players[player]
            .library
            .remove(pos)
            .expect("library idx");
    }
    if let Some(pos) = e.state.players[player]
        .hand
        .iter()
        .position(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some(card_id))
    {
        return e.state.players[player].hand.remove(pos);
    }
    panic!("missing card {card_id} for P{player}");
}

fn relocate_to_battlefield(e: &mut GameEngine, player: usize, card_id: &str, tapped: bool) -> u32 {
    let oid = take_oid_from_library_or_hand(e, player, card_id);
    e.state.players[player].battlefield.push(oid);
    let o = e.state.objects.get_mut(&oid).expect("object");
    o.zone = tricerules_core::Zone::Battlefield;
    o.tapped = tapped;
    o.summoning_sick = false;
    oid
}

fn relocate_to_hand(e: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let oid = take_oid_from_library_or_hand(e, player, card_id);
    e.state.players[player].hand.push(oid);
    e.state.objects.get_mut(&oid).expect("object").zone = tricerules_core::Zone::Hand;
    oid
}

#[test]
fn defender_creature_cannot_be_declared_as_attacker() {
    let decks = Some(vec![
        deck_with("mountain", &["wall_of_stone", "grizzly_bears"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(7001, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let wall = relocate_to_battlefield(&mut e, 0, "wall_of_stone", false);
    let bears = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);

    // Main1 -> BeginCombat -> DeclareAttackers (an eligible non-defender attacker exists).
    e.apply_command(0, &primitive_yield())
        .expect("main1 -> begin combat");
    e.apply_command(0, &primitive_yield())
        .expect("begin combat -> declare attackers");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );

    // CR 702.3b: a creature with defender can't attack.
    let err = e.apply_command(0, &declare_attackers(vec![wall]));
    assert!(
        err.is_err(),
        "defender creature must be rejected as an attacker"
    );
    // The rejected declaration mutates nothing: the bears can still be declared.
    e.apply_command(0, &declare_attackers(vec![bears]))
        .expect("non-defender attacks");
    assert_eq!(
        e.state.combat.as_ref().expect("combat").attacking,
        vec![bears]
    );
}

#[test]
fn lone_defender_provides_no_eligible_attackers() {
    let decks = Some(vec![
        deck_with("mountain", &["wall_of_stone"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(7002, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    relocate_to_battlefield(&mut e, 0, "wall_of_stone", false);

    e.apply_command(0, &primitive_yield())
        .expect("main1 -> begin combat");
    e.apply_command(0, &primitive_yield())
        .expect("begin combat advance");
    // With only a defender out, combat never enters the declare-attackers step.
    assert_ne!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers,
        "a lone defender is not an eligible attacker"
    );
}

#[test]
fn flash_creature_castable_at_instant_speed_unlike_flashless() {
    // 7-card opening hand holds both creatures; we cast during P0's own upkeep, which is
    // instant timing (CR 503) but NOT sorcery speed.
    let decks = Some(vec![
        deck_with("forest", &["ambush_viper", "grizzly_bears"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(7100, &[0, 1], 20, decks, true).expect("new");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Upkeep);
    relocate_to_hand(&mut e, 0, "ambush_viper");
    relocate_to_hand(&mut e, 0, "grizzly_bears");
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            g: 4,
            ..Default::default()
        }),
    )
    .expect("add green mana");

    // A flashless creature can't be cast at instant speed (CR 601.3a).
    let bears_idx = hand_index_for_card(&e, 0, "grizzly_bears");
    let err = e.apply_command(0, &cast_spell(bears_idx, vec![]));
    assert!(
        err.is_err(),
        "flashless creature must be sorcery-speed only"
    );

    // The flash creature casts in the same window (CR 702.8b).
    let viper_idx = hand_index_for_card(&e, 0, "ambush_viper");
    e.apply_command(0, &cast_spell(viper_idx, vec![]))
        .expect("flash creature casts at instant speed");
    assert_eq!(
        e.state.stack.last().expect("viper on stack").card_id,
        "ambush_viper"
    );
}

#[test]
fn wrath_of_god_destroys_all_creatures_except_indestructible() {
    let decks = Some(vec![
        deck_with(
            "plains",
            &["grizzly_bears", "darksteel_myr", "wrath_of_god"],
        ),
        deck_with("plains", &["savannah_lions"]),
    ]);
    let mut e = GameEngine::new(7200, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bears = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);
    let myr = relocate_to_battlefield(&mut e, 0, "darksteel_myr", false);
    let lions = relocate_to_battlefield(&mut e, 1, "savannah_lions", false);
    relocate_to_hand(&mut e, 0, "wrath_of_god");

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            w: 4,
            ..Default::default()
        }),
    )
    .expect("add white mana");
    let wrath_idx = hand_index_for_card(&e, 0, "wrath_of_god");
    e.apply_command(0, &cast_spell(wrath_idx, vec![]))
        .expect("cast wrath of god");
    resolve_entire_stack_two_player(&mut e);

    assert!(
        e.state.players[0].graveyard.contains(&bears),
        "grizzly bears destroyed"
    );
    assert!(
        e.state.players[1].graveyard.contains(&lions),
        "savannah lions destroyed"
    );
    // CR 702.12b: an indestructible creature survives "destroy all creatures".
    assert!(
        e.state.players[0].battlefield.contains(&myr),
        "indestructible Darksteel Myr survives"
    );
}

#[test]
fn pyroclasm_deals_two_damage_to_each_creature() {
    let decks = Some(vec![
        deck_with("mountain", &["grizzly_bears", "giant_spider", "pyroclasm"]),
        deck_with("mountain", &["savannah_lions"]),
    ]);
    let mut e = GameEngine::new(7300, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    let bears = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false); // 2/2 -> dies
    let spider = relocate_to_battlefield(&mut e, 0, "giant_spider", false); // 2/4 -> survives
    let lions = relocate_to_battlefield(&mut e, 1, "savannah_lions", false); // 2/1 -> dies
    relocate_to_hand(&mut e, 0, "pyroclasm");

    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            r: 2,
            ..Default::default()
        }),
    )
    .expect("add red mana");
    let idx = hand_index_for_card(&e, 0, "pyroclasm");
    e.apply_command(0, &cast_spell(idx, vec![]))
        .expect("cast pyroclasm");
    resolve_entire_stack_two_player(&mut e);

    // State-based actions destroy creatures with lethal damage (CR 704.5g).
    assert!(
        e.state.players[0].graveyard.contains(&bears),
        "2-toughness creature dies"
    );
    assert!(
        e.state.players[1].graveyard.contains(&lions),
        "1-toughness creature dies"
    );
    // Giant Spider (toughness 4) survives, marked with 2 damage until cleanup.
    assert!(
        e.state.players[0].battlefield.contains(&spider),
        "4-toughness creature survives"
    );
    assert_eq!(e.state.objects.get(&spider).expect("spider").damage, 2);
}

#[test]
fn soul_warden_gains_life_when_another_creature_enters() {
    let decks = Some(vec![
        deck_with("forest", &["soul_warden", "grizzly_bears"]),
        vec!["forest".into(); 20],
    ]);
    let mut e = GameEngine::new(7400, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    relocate_to_hand(&mut e, 0, "soul_warden");
    relocate_to_hand(&mut e, 0, "grizzly_bears");
    assert_eq!(e.state.players[0].life, 20);

    // Casting Soul Warden itself does NOT gain life (the "another creature" / exclude_self clause).
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            w: 1,
            ..Default::default()
        }),
    )
    .expect("add white mana");
    let warden_idx = hand_index_for_card(&e, 0, "soul_warden");
    e.apply_command(0, &cast_spell(warden_idx, vec![]))
        .expect("cast soul warden");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0].life, 20,
        "Soul Warden's own ETB must not trigger itself"
    );

    // Another creature entering the battlefield triggers Soul Warden: +1 life.
    e.apply_command(
        0,
        &add_mana_to_pool(AddManaToPool {
            g: 2,
            ..Default::default()
        }),
    )
    .expect("add green mana");
    let bears_idx = hand_index_for_card(&e, 0, "grizzly_bears");
    e.apply_command(0, &cast_spell(bears_idx, vec![]))
        .expect("cast grizzly bears");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[0].life, 21,
        "another creature entering gains Soul Warden's controller 1 life"
    );
}

// ---------------------------------------------------------------------------
// Tokens (CR 111)
// ---------------------------------------------------------------------------

/// Refill P0's pool so a cast in the middle of a scenario never starves for mana.
fn grant_pool(e: &mut GameEngine, player: usize) {
    let pool = &mut e.state.players[player].mana_pool;
    pool.white = 9;
    pool.blue = 9;
    pool.black = 9;
    pool.red = 9;
    pool.green = 9;
    pool.colorless = 9;
}

fn battlefield_token_oids(e: &GameEngine, player: usize, token_id: &str) -> Vec<u32> {
    e.state.players[player]
        .battlefield
        .iter()
        .copied()
        .filter(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some(token_id))
        .collect()
}

fn token_created_events(
    batch: &tricerules_proto::ruled::v1::RuledEventBatch,
) -> Vec<&tricerules_proto::ruled::v1::TokenCreated> {
    batch
        .events
        .iter()
        .filter_map(|ev| match &ev.ev {
            Some(Ev::TokenCreated(t)) => Some(t),
            _ => None,
        })
        .collect()
}

/// Raise the Alarm makes exactly two 1/1 white Soldier tokens under the caster, summoning-sick,
/// on the battlefield, and emits a self-describing TokenCreated for each (CR 111.1/111.4).
#[test]
fn raise_the_alarm_creates_two_soldier_tokens() {
    let decks = Some(vec![
        vec![
            "raise_the_alarm".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(21, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    grant_pool(&mut e, 0);
    let idx = hand_index_for_card(&e, 0, "raise_the_alarm");
    e.apply_command(0, &cast_spell(idx, vec![]))
        .expect("cast raise the alarm");
    e.apply_command(0, &pass()).expect("caster pass");
    let resolved = e.apply_command(1, &pass()).expect("opponent pass");

    let soldiers = battlefield_token_oids(&e, 0, "soldier");
    assert_eq!(soldiers.len(), 2, "two soldier tokens created");
    for oid in &soldiers {
        let o = e.state.objects.get(oid).expect("token object");
        assert_eq!(o.owner, 0, "token controlled by caster");
        assert_eq!(o.zone, tricerules_core::Zone::Battlefield);
        assert_eq!((o.power, o.toughness), (Some(1), Some(1)), "1/1");
        assert!(o.summoning_sick, "entering token is summoning sick");
    }
    // P1 received no tokens (Controller, not EachPlayer).
    assert!(battlefield_token_oids(&e, 1, "soldier").is_empty());

    let created = token_created_events(&resolved);
    assert_eq!(created.len(), 2, "one TokenCreated per token");
    for tc in &created {
        assert_eq!(tc.controller_player_id, 0);
        assert_eq!(tc.card_id, "soldier");
        let id = tc.identity.as_ref().expect("identity");
        assert_eq!(id.name, "Soldier");
        assert_eq!(id.pt, "1/1");
        assert_eq!(id.color, "w");
        assert!(id.is_creature);
    }
}

/// One spell with several CreateTokens effects (Bestial Menace) mints each distinct token type
/// from its own registry definition — proving the ordered `spell_effect` Vec resolves tokens of
/// different characteristics in a single resolution.
#[test]
fn bestial_menace_creates_three_distinct_tokens() {
    let decks = Some(vec![
        vec![
            "bestial_menace".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
            "forest".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(25, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    grant_pool(&mut e, 0);
    let idx = hand_index_for_card(&e, 0, "bestial_menace");
    e.apply_command(0, &cast_spell(idx, vec![])).expect("cast");
    e.apply_command(0, &pass()).expect("pass");
    e.apply_command(1, &pass()).expect("pass");

    let snake = battlefield_token_oids(&e, 0, "snake");
    let wolf = battlefield_token_oids(&e, 0, "wolf");
    let elephant = battlefield_token_oids(&e, 0, "elephant");
    assert_eq!(snake.len(), 1);
    assert_eq!(wolf.len(), 1);
    assert_eq!(elephant.len(), 1);
    assert_eq!(
        e.state.objects[&snake[0]]
            .power
            .zip(e.state.objects[&snake[0]].toughness),
        Some((1, 1))
    );
    assert_eq!(
        e.state.objects[&wolf[0]]
            .power
            .zip(e.state.objects[&wolf[0]].toughness),
        Some((2, 2))
    );
    assert_eq!(
        e.state.objects[&elephant[0]]
            .power
            .zip(e.state.objects[&elephant[0]].toughness),
        Some((3, 3))
    );
}

/// CR 111.7/704.5: a token that dies leaves the battlefield, then ceases to exist as an SBA —
/// the object is gone entirely (not stranded in the graveyard). Other tokens are unaffected.
#[test]
fn token_dies_and_ceases_to_exist() {
    let decks = Some(vec![
        vec![
            "raise_the_alarm".into(),
            "lightning_bolt".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(22, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    grant_pool(&mut e, 0);
    let idx = hand_index_for_card(&e, 0, "raise_the_alarm");
    e.apply_command(0, &cast_spell(idx, vec![])).expect("cast");
    e.apply_command(0, &pass()).expect("pass");
    e.apply_command(1, &pass()).expect("pass");

    let soldiers = battlefield_token_oids(&e, 0, "soldier");
    assert_eq!(soldiers.len(), 2);
    let victim = soldiers[0];
    let survivor = soldiers[1];

    grant_pool(&mut e, 0);
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(
        0,
        &cast_spell(bolt_idx, vec![TargetRef { object_id: victim }]),
    )
    .expect("bolt the token");
    e.apply_command(0, &pass()).expect("pass");
    e.apply_command(1, &pass()).expect("pass");

    // CR 111.7: the dead token object no longer exists in any zone.
    assert!(
        !e.state.objects.contains_key(&victim),
        "dead token ceased to exist"
    );
    assert!(
        !e.state.players[0].graveyard.contains(&victim),
        "token must not linger in the graveyard"
    );
    // The other token is untouched.
    assert!(e.state.objects.contains_key(&survivor));
    assert_eq!(battlefield_token_oids(&e, 0, "soldier"), vec![survivor]);
}

/// CR 111.7: a token returned to its owner's hand ceases to exist rather than becoming a hand
/// card — it must not show up in the hand zone afterward.
#[test]
fn bounced_token_ceases_to_exist() {
    let decks = Some(vec![
        vec![
            "raise_the_alarm".into(),
            "unsummon".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(23, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    grant_pool(&mut e, 0);
    let idx = hand_index_for_card(&e, 0, "raise_the_alarm");
    e.apply_command(0, &cast_spell(idx, vec![])).expect("cast");
    e.apply_command(0, &pass()).expect("pass");
    e.apply_command(1, &pass()).expect("pass");

    let victim = battlefield_token_oids(&e, 0, "soldier")[0];
    let hand_before = e.state.players[0].hand.len();

    grant_pool(&mut e, 0);
    let uns_idx = hand_index_for_card(&e, 0, "unsummon");
    e.apply_command(
        0,
        &cast_spell(uns_idx, vec![TargetRef { object_id: victim }]),
    )
    .expect("unsummon the token");
    e.apply_command(0, &pass()).expect("pass");
    e.apply_command(1, &pass()).expect("pass");

    assert!(
        !e.state.objects.contains_key(&victim),
        "bounced token ceased to exist"
    );
    assert!(
        !e.state.players[0].hand.contains(&victim),
        "token must not enter the hand"
    );
    // Unsummon itself left hand (cast) and the token didn't join it.
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before - 1,
        "only the cast spell left the hand"
    );
}

/// An anthem ("creatures you control get +1/+1") modeled as an AllCreatures continuous effect
/// buffs a token through the same base-P/T + continuous-effects path used for cards — proving
/// the engine never special-cases token-ness for characteristic queries.
#[test]
fn anthem_buffs_token_via_shared_pt_path() {
    use tricerules_cards::primitives::{ContinuousEffectKind, EffectDuration};

    let decks = Some(vec![
        vec![
            "raise_the_alarm".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
            "plains".into(),
        ],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(24, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);

    grant_pool(&mut e, 0);
    let idx = hand_index_for_card(&e, 0, "raise_the_alarm");
    e.apply_command(0, &cast_spell(idx, vec![])).expect("cast");
    e.apply_command(0, &pass()).expect("pass");
    e.apply_command(1, &pass()).expect("pass");

    let token = battlefield_token_oids(&e, 0, "soldier")[0];
    assert_eq!(e.effective_power(token), Some(1));
    assert_eq!(e.effective_toughness(token), Some(1));

    e.state
        .continuous_effects
        .push(tricerules_core::ContinuousEffect {
            source_id: None,
            affected: tricerules_core::AffectedScope::AllCreatures,
            kind: ContinuousEffectKind::PtModify {
                delta_power: 1,
                delta_toughness: 1,
            },
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 0,
        });

    assert_eq!(
        e.effective_power(token),
        Some(2),
        "anthem buffs token power"
    );
    assert_eq!(
        e.effective_toughness(token),
        Some(2),
        "anthem buffs token toughness"
    );
}
