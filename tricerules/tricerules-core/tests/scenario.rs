//! Scripted command sequences (M2).

use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::ruled_event::Ev;
use tricerules_proto::ruled::v1::{
    CastSpell, DeclareAttackers, PassPriority, PlayLand, PrimitiveYieldStructured, RuledCommand, TargetRef,
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
        .expect("skip attackers to main2");
    e.apply_command(player, &primitive_yield())
        .expect("main2 to next turn");
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

#[test]
fn primitive_yield_active_skips_double_pass_main1() {
    let mut e = GameEngine::new(99, &[0, 1], 20, None).expect("new");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Main1);
    e.apply_command(0, &primitive_yield()).expect("active primitive");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::BeginCombat);
}

#[test]
fn two_player_passes_empty_stack_advances_toward_combat() {
    let mut e = GameEngine::new(99, &[0, 1], 20, None).expect("new");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Main1);
    e.apply_command(0, &pass()).expect("p0");
    e.apply_command(1, &pass()).expect("p1");
    // After two passes, should leave main1 to begin combat
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::BeginCombat);
}

#[test]
fn empty_stack_double_pass_emits_ap_priority_in_new_phase() {
    let mut e = GameEngine::new(99, &[0, 1], 20, None).expect("new");
    e.apply_command(0, &pass()).expect("p0 pass");
    let b = e.apply_command(1, &pass()).expect("p1 pass");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::BeginCombat);
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

    e.apply_command(0, &primitive_yield()).expect("active primitive");

    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::BeginCombat);
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
    let decks = Some(vec![
        vec!["mountain".into(); 7],
        vec!["forest".into(); 7],
    ]);
    let mut e = GameEngine::new(7, &[0, 1], 20, decks).expect("new");
    let hand_before = e.state.players[0].hand.len();
    let battlefield_before = e.state.players[0].battlefield.len();

    e.apply_command(0, &play_land(0)).expect("play land");

    assert_eq!(e.state.players[0].hand.len(), hand_before - 1);
    assert_eq!(e.state.players[0].battlefield.len(), battlefield_before + 1);
    let mountain = battlefield_object_for_card(&e, 0, "mountain");
    assert_eq!(e.state.objects.get(&mountain).expect("mountain").card_id, "mountain");
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

    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx)).expect("play mountain");

    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    let pushed = e
        .apply_command(0, &cast_spell(bolt_idx, vec![]))
        .expect("cast bolt");
    let bolt_oid = e.state.stack.last().expect("spell on stack").id;
    assert!(pushed
        .events
        .iter()
        .any(|ev| matches!(ev.ev, Some(Ev::StackPushed(_)))));

    e.apply_command(1, &pass()).expect("opponent pass");
    let resolved = e.apply_command(0, &pass()).expect("active pass");
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
fn casting_spell_emits_priority_handoff_to_opponent() {
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
    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx)).expect("play mountain");
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    let pushed = e
        .apply_command(0, &cast_spell(bolt_idx, vec![]))
        .expect("cast bolt");
    assert!(
        priority_changes_in(&pushed).contains(&1),
        "caster should hand priority to opponent after casting"
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
    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx)).expect("play mountain");
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, vec![]))
        .expect("cast bolt");
    e.apply_command(1, &pass()).expect("opponent pass");
    let resolved = e.apply_command(0, &pass()).expect("active pass");
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
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::DeclareAttackers);

    let bears_oid = battlefield_object_for_card(&e, 0, "grizzly_bears");
    let b = e
        .apply_command(0, &declare_attackers(vec![bears_oid]))
        .expect("declare attackers");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::DeclareBlockers);
    assert!(
        priority_changes_in(&b).contains(&1),
        "defending player should get priority in declare blockers"
    );
}

#[test]
fn no_attackers_skip_to_main2_emits_active_priority() {
    let mut e = GameEngine::new(67, &[0, 1], 20, None).expect("new");
    e.apply_command(0, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_command(0, &pass()).expect("ap pass begin combat");
    e.apply_command(1, &pass()).expect("nap pass begin combat");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::DeclareAttackers);

    let b = e
        .apply_command(0, &declare_attackers(vec![]))
        .expect("no attackers");
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Main2);
    assert!(
        priority_changes_in(&b).contains(&0),
        "active player should keep/regain priority in main2 after no attackers"
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

    e.apply_command(1, &pass()).expect("p1 pass");
    let resolved = e.apply_command(0, &pass()).expect("p0 pass");

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

    let hand_before_turn = e.state.players[0].hand.len();
    let mountain_idx = hand_index_for_card(&e, 0, "mountain");
    e.apply_command(0, &play_land(mountain_idx)).expect("play mountain");

    let mountain_oid = battlefield_object_for_card(&e, 0, "mountain");
    let bolt_idx = hand_index_for_card(&e, 0, "lightning_bolt");
    e.apply_command(0, &cast_spell(bolt_idx, vec![]))
        .expect("cast lightning bolt");
    e.apply_command(1, &pass()).expect("opponent pass");
    e.apply_command(0, &pass()).expect("active pass to resolve");

    assert!(
        e.state.objects.get(&mountain_oid).expect("mountain object").tapped,
        "mountain is tapped after paying for bolt"
    );

    end_active_turn(&mut e, 0);
    end_active_turn(&mut e, 1);

    assert_eq!(e.state.active_player_id(), 0);
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Main1);
    assert!(
        !e.state.objects.get(&mountain_oid).expect("mountain object").tapped,
        "mountain untaps during the active player's untap phase"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before_turn - 1,
        "player drew one card during draw phase after spending two cards"
    );
}
