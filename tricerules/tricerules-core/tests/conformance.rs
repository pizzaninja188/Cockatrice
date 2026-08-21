//! Registry conformance smoke test (Phase 4.4).
//!
//! For **every** card in `CardRegistry::global()` this builds a minimal 2-player game, puts the
//! card under P0's control with ample mana and a vanilla creature on each battlefield to serve
//! as a target, then performs the card's primary action (play a land / cast the spell with the
//! first legal target / activate each ability) and resolves the stack.
//!
//! The contract is deliberately weak so it scales automatically with the corpus: an action that
//! the engine judges `Illegal` is fine (a card may need a board this harness doesn't set up), but
//! the engine must never **panic** and must always leave a *sane* zone layout — every object in
//! exactly one place, and no *non-token* objects conjured or lost. Tokens (CR 111) legitimately
//! appear when a maker resolves and may cease to exist, so they are counted separately from the
//! fixed deck-card population. This is the safety net that makes Phase 6's bulk
//! vanilla/french-vanilla import trustworthy without a hand-written scenario per generated card.

use tricerules_cards::CardRegistry;
use tricerules_core::{GameEngine, TurnStep, Zone};
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{
    ActivateAbility, CastSource, CastSpell, PassPriority, PlayLand, ResolutionChoiceDecision,
    RuledCommand, SubmitResolutionChoice, TargetRef,
};

fn pass() -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::PassPriority(PassPriority {})),
    }
}

fn play_land(hand_card_index: u32) -> RuledCommand {
    play_land_face(hand_card_index, 0)
}

fn play_land_face(hand_card_index: u32, face_index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::PlayLand(PlayLand {
            hand_card_index,
            face_index,
        })),
    }
}

fn cast_spell(hand_card_index: u32, targets: Vec<TargetRef>) -> RuledCommand {
    cast_spell_face(hand_card_index, targets, 0)
}

fn cast_spell_face(hand_card_index: u32, targets: Vec<TargetRef>, face_index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::CastSpell(CastSpell {
            source: Some(CastSource {
                location: Some(
                    tricerules_proto::ruled::v1::cast_source::Location::HandIndex(hand_card_index),
                ),
            }),
            targets,
            x_value: 0,
            face_index,
            ..Default::default()
        })),
    }
}

fn activate_ability(
    permanent_id: u32,
    ability_index: u32,
    targets: Vec<TargetRef>,
) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: permanent_id,
            ability_index,
            targets,
            ..Default::default()
        })),
    }
}

fn tref(object_id: u32) -> TargetRef {
    TargetRef {
        object_id,
        damage_amount: 0,
        group_index: 0,
        kind: 0,
    }
}

/// Drive a freshly created game to P0's first main phase, where both sorcery- and instant-speed
/// actions are legal with an empty stack.
fn advance_to_main1(e: &mut GameEngine) {
    // upkeep -> draw -> main1 (active player + opponent both pass each step).
    for _ in 0..2 {
        let ap = e.state.priority_player_id();
        let nap = other_player(e, ap);
        let _ = e.apply_command(ap, &pass());
        let _ = e.apply_command(nap, &pass());
    }
    assert_eq!(
        e.state.turn_step,
        TurnStep::Main1,
        "expected to reach main1"
    );
}

fn other_player(e: &GameEngine, p: i32) -> i32 {
    if p == e.state.players[0].id {
        e.state.players[1].id
    } else {
        e.state.players[0].id
    }
}

/// Pull a card from P0's library into hand (direct state edit), returning the new hand slot.
fn put_card_in_hand(e: &mut GameEngine, card_id: &str) -> u32 {
    let pos = e.state.players[0]
        .library
        .iter()
        .position(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some(card_id))
        .unwrap_or_else(|| panic!("card {card_id} not in P0 library (deck too small?)"));
    let oid = e.state.players[0].library.remove(pos).expect("oid");
    e.state.objects.get_mut(&oid).expect("obj").zone = Zone::Hand;
    e.state.players[0].hand.push(oid);
    (e.state.players[0].hand.len() - 1) as u32
}

/// Put a card from `player`'s library onto the battlefield, untapped and not summoning-sick.
fn deploy(e: &mut GameEngine, player: usize, card_id: &str) -> u32 {
    let pos = e.state.players[player]
        .library
        .iter()
        .position(|oid| e.state.objects.get(oid).map(|o| o.card_id.as_str()) == Some(card_id))
        .unwrap_or_else(|| panic!("card {card_id} not in P{player} library"));
    let oid = e.state.players[player].library.remove(pos).expect("oid");
    e.state.players[player].battlefield.push(oid);
    let obj = e.state.objects.get_mut(&oid).expect("obj");
    obj.zone = Zone::Battlefield;
    obj.tapped = false;
    obj.summoning_sick = false;
    oid
}

/// Refill P0's pool so a failed cast attempt never starves the next attempt.
fn grant_ample_mana(e: &mut GameEngine) {
    let pool = &mut e.state.players[0].mana_pool;
    pool.white = 99;
    pool.blue = 99;
    pool.black = 99;
    pool.red = 99;
    pool.green = 99;
    pool.colorless = 99;
}

/// Resolve whatever sits on the stack, bounded so a target-prompt stall can't hang the test.
fn try_drain_stack(e: &mut GameEngine) {
    for _ in 0..40 {
        if let Some(pending) = e.state.pending_resolution.as_ref() {
            let deciding_player = pending.deciding_player;
            let pick_count = (pending
                .presentation
                .min
                .max(u32::from(!pending.presentation.candidates.is_empty())))
            .min(pending.presentation.max)
            .min(pending.presentation.candidates.len() as u32)
                as usize;
            let chosen_object_ids = pending
                .presentation
                .candidates
                .iter()
                .copied()
                .take(pick_count)
                .collect();
            let answer = RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    chosen_object_ids,
                    decision: if matches!(
                        pending.continuation,
                        tricerules_core::state::ResolutionContinuation::AuthoredBranch { .. }
                    ) {
                        ResolutionChoiceDecision::SelectBranch as i32
                    } else {
                        ResolutionChoiceDecision::Unspecified as i32
                    },
                    selected_branch_index: 0,
                })),
            };
            if e.apply_command(deciding_player, &answer).is_err() {
                break;
            }
            continue;
        }
        if e.state.stack.is_empty() {
            break;
        }
        let p = e.state.priority_player_id();
        if e.apply_command(p, &pass()).is_err() {
            break;
        }
    }
}

/// Sanity invariant: every object lives in exactly one place, and the fixed deck-card population
/// is unchanged. `expected_objects` is the non-token baseline; tokens (CR 111) created by a
/// resolving maker are counted on top of it, since they legitimately appear and vanish.
fn assert_zone_integrity(e: &GameEngine, expected_objects: usize, ctx: &str) {
    let registry = CardRegistry::global();
    let token_count = e
        .state
        .objects
        .values()
        .filter(|o| registry.is_token(&o.card_id))
        .count();
    assert_eq!(
        e.state.objects.len(),
        expected_objects + token_count,
        "{ctx}: non-token object count changed (deck cards conjured or lost)"
    );
    let mut seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
    let mut count_in_zones = 0usize;
    for p in &e.state.players {
        for oid in p
            .library
            .iter()
            .chain(p.hand.iter())
            .chain(p.battlefield.iter())
            .chain(p.graveyard.iter())
            .chain(p.exile.iter())
        {
            assert!(seen.insert(*oid), "{ctx}: object {oid} is in two zones");
            count_in_zones += 1;
        }
    }
    for item in &e.state.stack {
        assert!(
            seen.insert(item.id) || item.ability_text.is_some(),
            "{ctx}: stack object {} also in a zone",
            item.id
        );
        if item.ability_text.is_none() {
            count_in_zones += 1; // a spell on the stack is a real object
        }
    }
    // Every real object (deck cards + live tokens) is in some zone or is a spell on the stack.
    assert_eq!(
        count_in_zones,
        expected_objects + token_count,
        "{ctx}: some objects are not in any zone"
    );
}

/// Play or cast one face of `card_id` (CR 709/712/715) in a fresh game and assert zone integrity.
/// Used to exercise the non-front faces of a multi-face card, which the front-face sweep skips.
fn exercise_face_in_fresh_game(card_id: &str, face_index: u32) {
    let mut p0_deck: Vec<String> = std::iter::repeat_n(card_id.to_string(), 20).collect();
    p0_deck.extend(std::iter::repeat_n("grizzly_bears".to_string(), 10));
    p0_deck.extend(std::iter::repeat_n("forest".to_string(), 10));
    let p1_deck: Vec<String> = std::iter::repeat_n("grizzly_bears".to_string(), 30).collect();
    let decks = Some(vec![p0_deck, p1_deck]);
    let mut e = GameEngine::new(901, &[0, 1], 20, decks, true)
        .unwrap_or_else(|err| panic!("{card_id}: engine init failed: {err:?}"));
    advance_to_main1(&mut e);
    let my_creature = deploy(&mut e, 0, "grizzly_bears");
    let their_creature = deploy(&mut e, 1, "grizzly_bears");
    let opp = e.state.players[1].id as u32;
    let me = e.state.players[0].id as u32;
    let baseline = e.state.objects.len();
    let slot = put_card_in_hand(&mut e, card_id);
    let is_land = CardRegistry::global()
        .get(card_id)
        .and_then(|def| def.face(face_index as usize))
        .is_some_and(|face| face.is_land);
    if is_land {
        // A land face is played, not cast; a per-turn-limit rejection is acceptable.
        let _ = e.apply_command(0, &play_land_face(slot, face_index));
        assert_zone_integrity(&e, baseline, &format!("{card_id} face {face_index} (land)"));
        return;
    }
    let candidates: Vec<Vec<TargetRef>> = vec![
        vec![],
        vec![tref(opp)],
        vec![tref(me)],
        vec![tref(their_creature)],
        vec![tref(my_creature)],
    ];
    for targets in candidates {
        grant_ample_mana(&mut e);
        if e.apply_command(0, &cast_spell_face(slot, targets, face_index))
            .is_ok()
        {
            try_drain_stack(&mut e);
            break;
        }
    }
    assert_zone_integrity(
        &e,
        baseline,
        &format!("{card_id} face {face_index} (spell)"),
    );
}

#[test]
fn every_registered_card_resolves_without_panic() {
    let registry = CardRegistry::global();
    // Deterministic order so a failure is reproducible.
    let mut card_ids: Vec<&str> = registry.definitions().map(|d| d.id.as_str()).collect();
    card_ids.sort_unstable();

    for card_id in card_ids {
        let def = registry.get(card_id).expect("definition");

        // P0 deck: 20 copies of the card under test padded with basics for castability targets;
        // P1 deck: grizzly_bears so we can deploy an enemy creature to target.
        let mut p0_deck: Vec<String> = std::iter::repeat_n(card_id.to_string(), 20).collect();
        p0_deck.extend(std::iter::repeat_n("grizzly_bears".to_string(), 10));
        p0_deck.extend(std::iter::repeat_n("forest".to_string(), 10));
        let p1_deck: Vec<String> = std::iter::repeat_n("grizzly_bears".to_string(), 30).collect();
        let decks = Some(vec![p0_deck, p1_deck]);

        let mut e = GameEngine::new(900, &[0, 1], 20, decks, true)
            .unwrap_or_else(|err| panic!("{card_id}: engine init failed: {err:?}"));
        advance_to_main1(&mut e);

        // Vanilla creatures on each side serve as creature/permanent targets.
        let my_creature = deploy(&mut e, 0, "grizzly_bears");
        let their_creature = deploy(&mut e, 1, "grizzly_bears");
        let opp = e.state.players[1].id as u32;
        let me = e.state.players[0].id as u32;

        let baseline = e.state.objects.len();

        // CR 712.4a: a card in hand shows its front face, so front-face characteristics decide
        // how this sweep exercises it. Each additional face gets its own fresh game below.
        let front = def.primary_face();

        // CR 709/712/715: the sweep here plays/casts the front face (index 0); exercise every
        // other face of a multi-face card (split half / MDFC face) separately.
        for face_index in 1..def.face_count() as u32 {
            exercise_face_in_fresh_game(card_id, face_index);
        }

        if front.is_land {
            let slot = put_card_in_hand(&mut e, card_id);
            // play_land never needs a target; a per-turn-limit rejection is acceptable.
            let _ = e.apply_command(0, &play_land(slot));
            assert_zone_integrity(&e, baseline, &format!("{card_id} (land)"));
            continue;
        }

        // Spell: try the empty target set, then each candidate, taking the first the engine
        // accepts. Re-grant mana before each attempt (a rejected cast may have paid nothing,
        // but X-costs and partial failures shouldn't starve later tries).
        let slot = put_card_in_hand(&mut e, card_id);
        let candidates: Vec<Vec<TargetRef>> = vec![
            vec![],
            vec![tref(opp)],
            vec![tref(me)],
            vec![tref(their_creature)],
            vec![tref(my_creature)],
        ];
        let mut cast_ok = false;
        for targets in candidates {
            grant_ample_mana(&mut e);
            if e.apply_command(0, &cast_spell(slot, targets)).is_ok() {
                cast_ok = true;
                break;
            }
        }
        if cast_ok {
            try_drain_stack(&mut e);
        }
        assert_zone_integrity(&e, baseline, &format!("{card_id} (spell)"));

        // Activated abilities: exercise each on a battlefield copy of the card (permanents only).
        if front.is_permanent() && !front.activated_abilities.is_empty() {
            let deck = Some(vec![
                std::iter::repeat_n(card_id.to_string(), 20)
                    .chain(std::iter::repeat_n("grizzly_bears".to_string(), 10))
                    .collect::<Vec<_>>(),
                std::iter::repeat_n("grizzly_bears".to_string(), 30).collect(),
            ]);
            let mut ae = GameEngine::new(902, &[0, 1], 20, deck, true)
                .unwrap_or_else(|err| panic!("{card_id}: engine init failed: {err:?}"));
            advance_to_main1(&mut ae);
            let src = deploy(&mut ae, 0, card_id);
            let my_c = deploy(&mut ae, 0, "grizzly_bears");
            let their_c = deploy(&mut ae, 1, "grizzly_bears");
            let opp_a = ae.state.players[1].id as u32;
            let base = ae.state.objects.len();
            for ai in 0..front.activated_abilities.len() as u32 {
                let candidates: Vec<Vec<TargetRef>> = vec![
                    vec![],
                    vec![tref(their_c)],
                    vec![tref(my_c)],
                    vec![tref(opp_a)],
                ];
                for targets in candidates {
                    grant_ample_mana(&mut ae);
                    if ae
                        .apply_command(0, &activate_ability(src, ai, targets))
                        .is_ok()
                    {
                        try_drain_stack(&mut ae);
                        break;
                    }
                }
            }
            assert_zone_integrity(&ae, base, &format!("{card_id} (ability)"));
        }
    }
}
