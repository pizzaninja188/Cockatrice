//! Shared helpers for scenario tests.
#![allow(dead_code)]

pub(crate) use tricerules_core::{AttachmentRecipient, GameEngine};
use tricerules_proto::ruled::v1 as rv1;
pub(crate) use tricerules_proto::ruled::v1::ruled_command::Cmd;
pub(crate) use tricerules_proto::ruled::v1::ruled_event::Ev;
pub(crate) use tricerules_proto::ruled::v1::{
    ActivateAbility, AssignCombatDamage, BlockPair, CastSource, CastSpell, ChoiceKind,
    ChooseTriggerTarget, CostSelection, DamagePair, DeclareAttackers, DeclareBlockers,
    DiscardToHandSize, FlexPipPayment, ManaSpendSelection, PassPriority, PlayLand,
    PreviewDeclareAttackers, PreviewDeclareBlockers, PrimitiveYieldStructured,
    ResolutionChoiceRequired, RuledCommand, RuledEventBatch, SelectedSpellMode,
    SubmitResolutionChoice, SubmitTriggerOrder, TargetRef, TargetRefKind, UndoManaAbility,
};

pub(crate) fn pass() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::PassPriority(PassPriority {})),
    }
}

pub(crate) fn cast_modal_spell(
    hand_card_index: usize,
    modes: Vec<(u32, Vec<TargetRef>)>,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            source: Some(hand_cast_source(hand_card_index)),
            selected_modes: modes
                .into_iter()
                .map(|(mode_index, targets)| SelectedSpellMode {
                    mode_index,
                    targets,
                })
                .collect(),
            ..Default::default()
        })),
    }
}

pub(crate) fn primitive_yield() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::PrimitiveYieldStructured(PrimitiveYieldStructured {})),
    }
}

pub(crate) fn concede() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::Concede(tricerules_proto::ruled::v1::Concede {})),
    }
}

pub(crate) fn discard_cleanup(hand_card_index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DiscardToHandSize(DiscardToHandSize {
            hand_card_indices: vec![hand_card_index],
        })),
    }
}

pub(crate) fn discard_cleanup_batch(indices: Vec<u32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DiscardToHandSize(DiscardToHandSize {
            hand_card_indices: indices,
        })),
    }
}

/// After leaving the end step, the engine may stop in cleanup for 514.1 discards.
pub(crate) fn resolve_cleanup_discards_if_any(e: &mut GameEngine) {
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

pub(crate) fn play_land(hand_card_index: usize) -> RuledCommand {
    play_land_face(hand_card_index, 0)
}

pub(crate) fn play_land_face(hand_card_index: usize, face_index: usize) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::PlayLand(PlayLand {
            hand_card_index: hand_card_index as u32,
            face_index: face_index as u32,
        })),
    }
}

pub(crate) fn cast_spell(hand_card_index: usize, targets: Vec<TargetRef>) -> RuledCommand {
    cast_spell_x(hand_card_index, targets, 0)
}

pub(crate) fn cast_spell_with_costs(
    hand_card_index: usize,
    targets: Vec<TargetRef>,
    cost_selections: Vec<CostSelection>,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            source: Some(hand_cast_source(hand_card_index)),
            targets,
            cost_selections,
            ..Default::default()
        })),
    }
}

pub(crate) fn cast_spell_x(
    hand_card_index: usize,
    targets: Vec<TargetRef>,
    x_value: u32,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            source: Some(hand_cast_source(hand_card_index)),
            targets,
            x_value,
            ..Default::default()
        })),
    }
}

pub(crate) fn cast_spell_flex(
    hand_card_index: usize,
    targets: Vec<TargetRef>,
    flex_payments: Vec<FlexPipPayment>,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            source: Some(hand_cast_source(hand_card_index)),
            targets,
            x_value: 0,
            flex_payments,
            ..Default::default()
        })),
    }
}

/// Cast a specific face of a multi-face card (CR 709/712/715): split half / MDFC face index.
pub(crate) fn cast_spell_face(
    hand_card_index: usize,
    targets: Vec<TargetRef>,
    face_index: u32,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            source: Some(hand_cast_source(hand_card_index)),
            targets,
            face_index,
            ..Default::default()
        })),
    }
}

/// A bag of mana to drop straight into a player's pool for test setup. Mana production is now
/// engine-owned (CR 605 mana abilities); there is no client command to mint mana, so tests fund
/// the pool directly (mirroring `conformance.rs::grant_ample_mana`) rather than tapping lands.
#[derive(Default)]
pub(crate) struct ManaGift {
    pub(crate) w: u32,
    pub(crate) u: u32,
    pub(crate) b: u32,
    pub(crate) r: u32,
    pub(crate) g: u32,
    pub(crate) c: u32,
}

pub(crate) fn give_mana(e: &mut GameEngine, player: i32, m: ManaGift) {
    let idx = e.state.player_idx(player).expect("known player");
    let pool = &mut e.state.players[idx].mana_pool;
    pool.white += m.w;
    pool.blue += m.u;
    pool.black += m.b;
    pool.red += m.r;
    pool.green += m.g;
    pool.colorless += m.c;
}

pub(crate) fn activate_ability(
    permanent_id: u32,
    ability_index: u32,
    targets: Vec<TargetRef>,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            permanent_id,
            ability_index,
            targets,
            ..Default::default()
        })),
    }
}

pub(crate) fn activate_ability_with_costs(
    permanent_id: u32,
    ability_index: u32,
    targets: Vec<TargetRef>,
    cost_selections: Vec<CostSelection>,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            permanent_id,
            ability_index,
            targets,
            cost_selections,
            ..Default::default()
        })),
    }
}

pub(crate) fn hand_cost_selection(cost_index: u32, hand_index: u32) -> CostSelection {
    CostSelection {
        cost_index,
        selection: Some(rv1::cost_selection::Selection::HandIndex(hand_index)),
    }
}

pub(crate) fn permanent_cost_selection(cost_index: u32, permanent_id: u32) -> CostSelection {
    CostSelection {
        cost_index,
        selection: Some(rv1::cost_selection::Selection::PermanentId(permanent_id)),
    }
}

pub(crate) fn undo_mana_ability() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::UndoManaAbility(UndoManaAbility {})),
    }
}

/// Place a card from `player`'s library directly onto the battlefield (untapped, not
/// summoning-sick unless `tapped`), returning its object id. Used to set up board states that
/// would otherwise take many turns of legal play to reach.
pub(crate) fn deploy_to_battlefield(
    e: &mut GameEngine,
    player: usize,
    card_id: &str,
    tapped: bool,
) -> u32 {
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
pub(crate) fn target_player(pid: i32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id: pid as u32,
        damage_amount: 0,
        group_index: 0,
        kind: 0,
    }]
}

pub(crate) fn target_object(oid: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id: oid,
        ..Default::default()
    }]
}

/// Player target with explicit damage allocation, for `DamageTargets` spells (Fireball, Fire).
pub(crate) fn target_player_damage(pid: i32, damage: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id: pid as u32,
        damage_amount: damage,
        group_index: 0,
        kind: 0,
    }]
}

/// Multiple targets with explicit damage allocations for `DamageTargets`.
pub(crate) fn targets_with_damage(pairs: Vec<(u32, u32)>) -> Vec<TargetRef> {
    pairs
        .into_iter()
        .map(|(oid, dmg)| TargetRef {
            object_id: oid,
            damage_amount: dmg,
            group_index: 0,
            kind: 0,
        })
        .collect()
}

pub(crate) fn declare_attackers(creature_ids: Vec<u32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DeclareAttackers(DeclareAttackers { creature_ids })),
    }
}

pub(crate) fn declare_blockers(block_pairs: Vec<BlockPair>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::DeclareBlockers(DeclareBlockers { block_pairs })),
    }
}

pub(crate) fn hand_index_for_card(e: &GameEngine, player: usize, card_id: &str) -> usize {
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

pub(crate) fn count_card_id_in_graveyard(e: &GameEngine, player: usize, card_id: &str) -> usize {
    e.state.players[player]
        .graveyard
        .iter()
        .filter(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some(card_id))
        .count()
}

/// Ensures `card_id` is in the player's hand — no-op if already there, otherwise moves it from library.
pub(crate) fn ensure_card_in_hand(e: &mut GameEngine, player: usize, card_id: &str) {
    let already_in_hand = e.state.players[player]
        .hand
        .iter()
        .any(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some(card_id));
    if !already_in_hand {
        take_card_from_library_to_hand(e, player, card_id);
    }
}

pub(crate) fn take_card_from_library_to_hand(e: &mut GameEngine, player: usize, card_id: &str) {
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

pub(crate) fn battlefield_object_for_card(e: &GameEngine, player: usize, card_id: &str) -> u32 {
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

pub(crate) fn end_active_turn(e: &mut GameEngine, player: i32) {
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

pub(crate) fn priority_changes_in(
    batch: &tricerules_proto::ruled::v1::RuledEventBatch,
) -> Vec<i32> {
    batch
        .events
        .iter()
        .filter_map(|ev| match &ev.ev {
            Some(Ev::PriorityChanged(pc)) => Some(pc.player_id),
            _ => None,
        })
        .collect()
}

/// Answer an outstanding CR 603.3b ordering prompt by accepting the engine's own APNAP order.
///
/// Most scenarios don't care about trigger order — they just need the game to keep moving — so the
/// pass helpers call this first. Tests that *are* about ordering pick their own trigger instead and
/// never reach here. Returns whether any prompt was answered.
///
/// Loops because the prompt is per-pick, not per-block: answering one re-raises it for the rest
/// (until one is left, which the engine places itself). Stops at a parked target choice, since that
/// blocks the next pick and only the caller knows what to target.
pub(crate) fn answer_trigger_order_in_engine_order(e: &mut GameEngine) -> bool {
    let mut answered = false;
    while let Some(pending) = e.state.pending_trigger_order.as_ref() {
        if !e.state.pending_triggers.is_empty() {
            break;
        }
        let player = pending.deciding_player;
        let first = pending.candidates[0].object_id;
        e.apply_command(player, &submit_trigger_order(first))
            .expect("submit trigger order");
        answered = true;
    }
    answered
}

pub(crate) fn submit_trigger_order(trigger_object_id: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitTriggerOrder(SubmitTriggerOrder {
            trigger_object_id,
        })),
    }
}

pub(crate) fn pass_both_players(e: &mut GameEngine) {
    // A pending ordering prompt blocks every command, `pass` included — clear it the boring way so
    // scenarios about something else don't have to know this mechanic exists.
    answer_trigger_order_in_engine_order(e);
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
pub(crate) fn resolve_entire_stack_two_player(e: &mut GameEngine) {
    loop {
        answer_trigger_order_in_engine_order(e);
        if e.state.stack.is_empty() {
            break;
        }
        pass_both_players(e);
    }
}

pub(crate) fn hand_cast_source(hand_card_index: usize) -> CastSource {
    CastSource {
        location: Some(rv1::cast_source::Location::HandIndex(
            hand_card_index as u32,
        )),
    }
}

pub(crate) fn graveyard_cast_source(object_id: u32) -> CastSource {
    CastSource {
        location: Some(rv1::cast_source::Location::GraveyardObjectId(object_id)),
    }
}

pub(crate) fn exile_cast_source(object_id: u32) -> CastSource {
    CastSource {
        location: Some(rv1::cast_source::Location::ExileObjectId(object_id)),
    }
}

pub(crate) fn advance_to_main1_from_game_start(e: &mut GameEngine) {
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Upkeep);
    pass_both_players(e); // upkeep -> draw
    pass_both_players(e); // draw -> main1
    assert_eq!(e.state.turn_step, tricerules_core::TurnStep::Main1);
}

pub(crate) fn put_creature_on_battlefield(e: &mut GameEngine, player: usize, card_id: &str) -> u32 {
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
pub(crate) fn inject_creature_on_battlefield(
    e: &mut GameEngine,
    player: usize,
    card_id: &str,
) -> u32 {
    let id = e.state.next_object_id;
    e.state.next_object_id += 1;
    let player_id = e.state.players[player].id;
    e.state.objects.insert(
        id,
        tricerules_core::state::GameObject {
            id,
            owner: player_id,
            base_controller: player_id,
            controller: player_id,
            card_id: card_id.to_string(),
            copiable_values: None,
            copy_revision: 0,
            zone: tricerules_core::Zone::Battlefield,
            tapped: false,
            summoning_sick: false,
            power: Some(2),
            toughness: Some(2),
            damage: 0,
            deathtouch_damage: false,
            counters: std::collections::BTreeMap::new(),
            attached_to: None,
            regeneration_shields: 0,
            must_attack_if_able: false,
            must_block_if_able: false,
            face_up_index: 0,
            adventure_cast_permission: None,
        },
    );
    e.state.players[player].battlefield.push(id);
    id
}

/// Inject a non-creature permanent (e.g. a land) onto the battlefield, untapped and not
/// summoning-sick, without consuming a hand/library card. Returns its object id.
pub(crate) fn inject_permanent_on_battlefield(
    e: &mut GameEngine,
    player: usize,
    card_id: &str,
) -> u32 {
    let id = e.state.next_object_id;
    e.state.next_object_id += 1;
    let player_id = e.state.players[player].id;
    e.state.objects.insert(
        id,
        tricerules_core::state::GameObject {
            id,
            owner: player_id,
            base_controller: player_id,
            controller: player_id,
            card_id: card_id.to_string(),
            copiable_values: None,
            copy_revision: 0,
            zone: tricerules_core::Zone::Battlefield,
            tapped: false,
            summoning_sick: false,
            power: None,
            toughness: None,
            damage: 0,
            deathtouch_damage: false,
            counters: std::collections::BTreeMap::new(),
            attached_to: None,
            regeneration_shields: 0,
            must_attack_if_able: false,
            must_block_if_able: false,
            face_up_index: 0,
            adventure_cast_permission: None,
        },
    );
    e.state.players[player].battlefield.push(id);
    id
}

/// Inject a known registry card into a player's hand for scenarios that need exact cast identity
/// without depending on opening-hand shuffle order.
pub(crate) fn inject_card_into_hand(e: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let id = e.state.next_object_id;
    e.state.next_object_id += 1;
    let player_id = e.state.players[player].id;
    e.state.objects.insert(
        id,
        tricerules_core::state::GameObject {
            id,
            owner: player_id,
            base_controller: player_id,
            controller: player_id,
            card_id: card_id.to_string(),
            copiable_values: None,
            copy_revision: 0,
            zone: tricerules_core::Zone::Hand,
            tapped: false,
            summoning_sick: false,
            power: None,
            toughness: None,
            damage: 0,
            deathtouch_damage: false,
            counters: std::collections::BTreeMap::new(),
            attached_to: None,
            regeneration_shields: 0,
            must_attack_if_able: false,
            must_block_if_able: false,
            face_up_index: 0,
            adventure_cast_permission: None,
        },
    );
    e.state.players[player].hand.push(id);
    id
}

/// Inject a card directly onto the bottom of a player's library (so e.g. a draw effect has
/// something to draw when the opening hand consumed the whole deck). Returns its object id.
pub(crate) fn inject_library_card(e: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let id = e.state.next_object_id;
    e.state.next_object_id += 1;
    let player_id = e.state.players[player].id;
    e.state.objects.insert(
        id,
        tricerules_core::state::GameObject {
            id,
            owner: player_id,
            base_controller: player_id,
            controller: player_id,
            card_id: card_id.to_string(),
            copiable_values: None,
            copy_revision: 0,
            zone: tricerules_core::Zone::Library,
            tapped: false,
            summoning_sick: false,
            power: None,
            toughness: None,
            damage: 0,
            deathtouch_damage: false,
            counters: std::collections::BTreeMap::new(),
            attached_to: None,
            regeneration_shields: 0,
            must_attack_if_able: false,
            must_block_if_able: false,
            face_up_index: 0,
            adventure_cast_permission: None,
        },
    );
    e.state.players[player].library.push_back(id);
    id
}

/// Inject a creature onto `controller`'s battlefield that is **owned by someone else** — the
/// shape a reanimated permanent has (CR 108.3 owner vs CR 110.2 controller). Untapped and not
/// summoning sick, so combat tests can use it immediately.
pub(crate) fn inject_creature_under_foreign_control(
    e: &mut GameEngine,
    owner: usize,
    controller: usize,
    card_id: &str,
) -> u32 {
    let id = e.state.next_object_id;
    e.state.next_object_id += 1;
    let owner_id = e.state.players[owner].id;
    let controller_id = e.state.players[controller].id;
    e.state.objects.insert(
        id,
        tricerules_core::state::GameObject {
            id,
            owner: owner_id,
            base_controller: controller_id,
            controller: controller_id,
            card_id: card_id.to_string(),
            copiable_values: None,
            copy_revision: 0,
            zone: tricerules_core::Zone::Battlefield,
            tapped: false,
            summoning_sick: false,
            power: None,
            toughness: None,
            damage: 0,
            deathtouch_damage: false,
            counters: std::collections::BTreeMap::new(),
            attached_to: None,
            regeneration_shields: 0,
            must_attack_if_able: false,
            must_block_if_able: false,
            face_up_index: 0,
            adventure_cast_permission: None,
        },
    );
    // The battlefield list is the control index, so the permanent goes on the *controller's*.
    e.state.players[controller].battlefield.push(id);
    id
}

/// Put a fresh card object straight into `player`'s graveyard, bypassing the game (no dies
/// trigger, no zone-change events) — the graveyard equivalent of [`inject_library_card`], for
/// setting up reanimation and graveyard-targeting scenarios.
pub(crate) fn inject_graveyard_card(e: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let id = e.state.next_object_id;
    e.state.next_object_id += 1;
    let player_id = e.state.players[player].id;
    e.state.objects.insert(
        id,
        tricerules_core::state::GameObject {
            id,
            owner: player_id,
            base_controller: player_id,
            controller: player_id,
            card_id: card_id.to_string(),
            copiable_values: None,
            copy_revision: 0,
            zone: tricerules_core::Zone::Graveyard,
            tapped: false,
            summoning_sick: false,
            power: None,
            toughness: None,
            damage: 0,
            deathtouch_damage: false,
            counters: std::collections::BTreeMap::new(),
            attached_to: None,
            regeneration_shields: 0,
            must_attack_if_able: false,
            must_block_if_able: false,
            face_up_index: 0,
            adventure_cast_permission: None,
        },
    );
    e.state.players[player].graveyard.push(id);
    id
}

pub(crate) fn advance_to_declare_attackers(e: &mut GameEngine) {
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

pub(crate) fn life_changes_in(
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

pub(crate) fn permanents_moved_in(
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

pub(crate) fn attackers_declared_in(
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

pub(crate) fn blockers_declared_in(
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

pub(crate) fn assign_combat_damage_cmd(attacker_id: u32, pairs: Vec<(u32, u32)>) -> RuledCommand {
    assign_combat_damage_cmd_with_player(attacker_id, pairs, 0)
}

pub(crate) fn assign_combat_damage_cmd_with_player(
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
pub(crate) fn ensure_in_hand(e: &mut GameEngine, player: usize, card_id: &str) {
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

/// Two blockers on one 2-power attacker, both passes done → assign_combat_damage_phase.
pub(crate) fn setup_two_blockers_assign_phase(
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

// ----------------------------------------------------------------------------
// New M2 primitives: GainLife, LoseLife, ExileTarget, ReturnToHand, Mill.
// ----------------------------------------------------------------------------

pub(crate) fn forest_only_deck() -> Vec<String> {
    vec!["forest".into(); 30]
}

pub(crate) fn island_only_deck() -> Vec<String> {
    vec!["island".into(); 30]
}

// ── Deathtouch Keyword Tests ──────────────────────────────────────────────────
//
// Tests for CR 702.2b / CR 704.5h: any amount of damage dealt by a deathtouch
// source to a creature is enough to destroy it via SBA.

/// Helper: inject a creature with explicit power and toughness (unlike the
/// default 2/2 of `inject_creature_on_battlefield`).
pub(crate) fn inject_creature_with_stats(
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
            base_controller: player_id,
            controller: player_id,
            card_id: card_id.to_string(),
            copiable_values: None,
            copy_revision: 0,
            zone: tricerules_core::Zone::Battlefield,
            tapped: false,
            summoning_sick: false,
            power: Some(power),
            toughness: Some(toughness),
            damage: 0,
            deathtouch_damage: false,
            counters: std::collections::BTreeMap::new(),
            attached_to: None,
            regeneration_shields: 0,
            must_attack_if_able: false,
            must_block_if_able: false,
            face_up_index: 0,
            adventure_cast_permission: None,
        },
    );
    e.state.players[player].battlefield.push(id);
    id
}

// ── Trample scenarios ─────────────────────────────────────────────────────────

/// Helper: advance a Colossal Dreadmaw (6/6 Trample) to the assign-combat-damage phase
/// with one blocker (grizzly_bears 2/2). Returns (engine, attacker_oid, blocker_oid).
pub(crate) fn setup_trample_single_blocker_assign_phase() -> (GameEngine, u32, u32) {
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
    let batch = e
        .apply_command(1, &pass())
        .expect("defender pass → assign phase");
    assert!(
        e.state.combat.as_ref().unwrap().assign_combat_damage_phase,
        "assign_combat_damage_phase must be open"
    );
    let active_legal = batch
        .legal_by_player
        .get(&0)
        .expect("legal actions for active player");
    assert_eq!(
        active_legal.labels,
        vec!["Assign combat damage for Colossal Dreadmaw".to_string()],
        "single-blocked trample attacker must have an assignment prompt"
    );
    let defending_legal = batch
        .legal_by_player
        .get(&1)
        .expect("legal actions for defending player");
    assert_eq!(
        defending_legal.labels,
        vec!["Waiting: opponent assigning combat damage".to_string()],
        "defending player must retain the waiting prompt"
    );
    (e, attacker, blocker)
}

/// Build a 20-card deck: `specials` (once each) plus enough `basic` lands to fill out.
pub(crate) fn deck_with(basic: &str, specials: &[&str]) -> Vec<String> {
    let mut d: Vec<String> = specials.iter().map(|s| s.to_string()).collect();
    while d.len() < 20 {
        d.push(basic.to_string());
    }
    d
}

/// Remove the first object of `card_id` from `player`'s library or hand (wherever it landed
/// after the random opening draw) and return its id. Lets a test place specific cards without
/// depending on shuffle order.
pub(crate) fn take_oid_from_library_or_hand(
    e: &mut GameEngine,
    player: usize,
    card_id: &str,
) -> u32 {
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

pub(crate) fn relocate_to_battlefield(
    e: &mut GameEngine,
    player: usize,
    card_id: &str,
    tapped: bool,
) -> u32 {
    let oid = take_oid_from_library_or_hand(e, player, card_id);
    e.state.players[player].battlefield.push(oid);
    let o = e.state.objects.get_mut(&oid).expect("object");
    o.zone = tricerules_core::Zone::Battlefield;
    o.tapped = tapped;
    o.summoning_sick = false;
    oid
}

pub(crate) fn relocate_to_hand(e: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let oid = take_oid_from_library_or_hand(e, player, card_id);
    e.state.players[player].hand.push(oid);
    e.state.objects.get_mut(&oid).expect("object").zone = tricerules_core::Zone::Hand;
    oid
}

// ---------------------------------------------------------------------------
// Tokens (CR 111)
// ---------------------------------------------------------------------------

/// Refill P0's pool so a cast in the middle of a scenario never starves for mana.
pub(crate) fn grant_pool(e: &mut GameEngine, player: usize) {
    let pool = &mut e.state.players[player].mana_pool;
    pool.white = 9;
    pool.blue = 9;
    pool.black = 9;
    pool.red = 9;
    pool.green = 9;
    pool.colorless = 9;
}

pub(crate) fn battlefield_token_oids(e: &GameEngine, player: usize, token_id: &str) -> Vec<u32> {
    e.state.players[player]
        .battlefield
        .iter()
        .copied()
        .filter(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some(token_id))
        .collect()
}

pub(crate) fn token_created_events(
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

// ---------------------------------------------------------------------------
// Tier-3 custom resolution (resumable resolution): Brainstorm + Gifts Ungiven.
// ---------------------------------------------------------------------------

pub(crate) fn submit_resolution_choice(chosen: Vec<u32>) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_object_ids: chosen,
            decision: 0,
        })),
    }
}

pub(crate) fn submit_resolution_decision(decision: rv1::ResolutionChoiceDecision) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            chosen_object_ids: Vec::new(),
            decision: decision as i32,
        })),
    }
}

pub(crate) fn find_resolution_choice(batch: &RuledEventBatch) -> Option<ResolutionChoiceRequired> {
    batch.events.iter().find_map(|e| match &e.ev {
        Some(Ev::ResolutionChoiceRequired(r)) => Some(r.clone()),
        _ => None,
    })
}

/// Cast the instant `card_id` for `mana` and pass both players so it resolves; returns the
/// resolving batch (which parks a custom resolution for tier-3 cards).
pub(crate) fn cast_instant_and_resolve(
    e: &mut GameEngine,
    caster: i32,
    card_id: &str,
    mana: ManaGift,
) -> RuledEventBatch {
    ensure_in_hand(e, caster as usize, card_id);
    give_mana(e, caster, mana);
    let idx = hand_index_for_card(e, caster as usize, card_id);
    e.apply_command(caster, &cast_spell(idx, vec![]))
        .expect("cast");
    e.apply_command(caster, &pass()).expect("caster pass");
    e.apply_command(1 - caster, &pass()).expect("other pass")
}

// ===========================================================================
// Card-coverage expansion — P1 (static anthems / lords, one-shot mass pump)
// and P2 (target-filter widening). See docs/plan-card-coverage-expansion.md.
// ===========================================================================

/// Build a two-player engine at Main 1 with P0 holding `p0_card` (deck padded with mountains)
/// and P1 a vanilla island deck. The named card is in P0's opening hand.
pub(crate) fn anthem_engine(seed: u64, p0_card: &str) -> GameEngine {
    let decks = Some(vec![
        vec![
            p0_card.into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
            "mountain".into(),
        ],
        vec![
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
            "island".into(),
        ],
    ]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("new");
    advance_to_main1_from_game_start(&mut e);
    e
}

/// `AbilityInfo.activatable` for every activated ability of `oid`, read out of the zone-view
/// snapshot the engine broadcasts. This is exactly what the client greys its activation menu from,
/// so asserting on it is asserting on what the UI will actually offer.
pub(crate) fn zone_view_ability_flags(e: &mut GameEngine, player: usize, oid: u32) -> Vec<bool> {
    e.initial_response_batch()
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .and_then(|view| view.per_player.get(player))
        .into_iter()
        .flat_map(|p| p.battlefield_objects.iter())
        .find(|object| object.object_id == oid)
        .map(|object| {
            object
                .activated_abilities
                .iter()
                .map(|ability| ability.activatable)
                .collect()
        })
        .unwrap_or_default()
}

/// Engine-authored display labels for nonintrinsic rules state on `oid`.
pub(crate) fn zone_view_rules_annotation_labels(
    e: &mut GameEngine,
    player: usize,
    oid: u32,
) -> Vec<String> {
    e.initial_response_batch()
        .events
        .iter()
        .find_map(|event| match &event.ev {
            Some(Ev::ZoneView(view)) => Some(view),
            _ => None,
        })
        .and_then(|view| view.per_player.get(player))
        .into_iter()
        .flat_map(|p| p.battlefield_objects.iter())
        .find(|object| object.object_id == oid)
        .map(|object| object.rules_annotation_labels.clone())
        .unwrap_or_default()
}
