//! Issue #335 — the eight reviewed attack, dies, anthem, and loot Standard cards.
//!
//! These scenarios drive the generated definitions through the authoritative command path.
//! CR 508.1/119.3 govern the each-opponent attack life loss and its player scope; CR 603.6c/119.3
//! the dies gain-two-life trigger; CR 603.6a the self-or-other creature entry trigger with the
//! source included; CR 701.13/115 the mandatory creature-or-enchantment exile and its target
//! legality/fizzle; CR 603.6a the land's own entry gain-two-life trigger; CR 604.1/611.3/613.1f
//! layer 6 the live counter-filtered trample anthem; CR 603.6a/611.2a/514.2 the Landfall self pump
//! and its cleanup expiry; and CR 602.2/701.9 the {1}{U}, {T} draw-then-discard loot activation
//! with a private discard.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::{TurnStep, Zone};

fn main1_engine(seed: u64, own: &[&str], opposing: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", own),
        deck_with("forest", opposing),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn relocate_spell_to_hand(engine: &mut GameEngine, card: &str) -> usize {
    relocate_to_hand(engine, 0, card);
    hand_index_for_card(engine, 0, card)
}

fn resolve_entire_stack_three_player(engine: &mut GameEngine) {
    loop {
        answer_trigger_order_in_engine_order(engine);
        if engine.state.stack.is_empty() {
            break;
        }
        for _ in 0..engine.state.players.len() {
            if engine.state.stack.is_empty() {
                break;
            }
            let player = engine.state.priority_player_id();
            engine
                .apply_command(player, &pass())
                .expect("three-player priority pass");
        }
    }
}

#[test]
fn issue_335_pulse_tracker_drains_every_opponent_not_the_controller() {
    let mut engine = GameEngine::new(335_001, &[0, 1], 20, None, true).expect("engine");
    advance_to_declare_attackers(&mut engine);
    engine
        .state
        .players
        .push(tricerules_core::state::PlayerState::new(2, 20));

    let tracker = inject_creature_on_battlefield(&mut engine, 0, "pulse_tracker");
    let generation = engine
        .state
        .zone_change_generation
        .get(&tracker)
        .copied()
        .unwrap_or(0);
    let mut attack = declare_attackers(vec![tracker]);
    let Some(Cmd::DeclareAttackers(declare)) = attack.cmd.as_mut() else {
        unreachable!("declare_attackers builds a DeclareAttackers command");
    };
    let assignment = declare.assignments.first_mut().expect("one attacker");
    assignment.attacker_zone_change_generation = generation;
    assignment.defending_player_id = 1;
    assignment.defender = Some(TargetRef {
        object_id: 1,
        kind: TargetRefKind::Player as i32,
        ..Default::default()
    });
    engine
        .apply_command(0, &attack)
        .expect("attack player 1 with Pulse Tracker");
    assert_eq!(
        engine.state.stack.len(),
        1,
        "the attack trigger is on the stack"
    );
    resolve_entire_stack_three_player(&mut engine);

    assert_eq!(
        engine.state.players[0].life, 20,
        "CR 119.3: the attacking controller is not an opponent of itself"
    );
    assert_eq!(engine.state.players[1].life, 19);
    assert_eq!(engine.state.players[2].life, 19);
}

#[test]
fn issue_335_grasping_longneck_gains_two_life_when_it_dies() {
    let mut engine = main1_engine(335_002, &[], &[]);
    let neck = inject_creature_on_battlefield(&mut engine, 0, "grasping_longneck");
    assert_eq!(engine.state.players[0].life, 20);

    engine.state.objects.get_mut(&neck).expect("neck").damage = 2;
    engine
        .apply_command(0, &pass())
        .expect("lethal state-based action");
    assert_eq!(engine.state.objects[&neck].zone, Zone::Graveyard);
    assert_eq!(engine.state.stack.len(), 1, "one dies trigger");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(engine.state.players[0].life, 22);
    assert_eq!(engine.state.players[1].life, 20);
}

#[test]
fn issue_335_bogwater_lumaret_includes_its_own_creature_entry() {
    let mut engine = main1_engine(
        335_003,
        &["bogwater_lumaret", "grizzly_bears"],
        &["grizzly_bears"],
    );
    move_ready_to_battlefield(&mut engine, 0, "bogwater_lumaret");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].life, 21,
        "CR 603.6a: \"this creature or another\" includes the source's own entry"
    );

    move_ready_to_battlefield(&mut engine, 0, "grizzly_bears");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, 22);

    move_ready_to_battlefield(&mut engine, 1, "grizzly_bears");
    assert!(
        engine.state.stack.is_empty(),
        "an opponent's creature does not satisfy \"you control\""
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, 22);
    assert_eq!(engine.state.players[1].life, 20);
}

#[test]
fn issue_335_angelic_edict_exiles_a_creature_or_enchantment_but_not_a_land() {
    let mut engine = main1_engine(335_004, &["angelic_edict"], &["grizzly_bears"]);
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 4,
            ..Default::default()
        },
    );
    let spell = relocate_spell_to_hand(&mut engine, "angelic_edict");
    engine
        .apply_command(0, &cast_spell(spell, target_object(bear)))
        .expect("cast Angelic Edict at a creature");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&bear].zone, Zone::Exile);

    let mut engine = main1_engine(335_005, &["angelic_edict"], &[]);
    let enchantment = inject_permanent_on_battlefield(&mut engine, 1, "exploration");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 4,
            ..Default::default()
        },
    );
    let spell = relocate_spell_to_hand(&mut engine, "angelic_edict");
    engine
        .apply_command(0, &cast_spell(spell, target_object(enchantment)))
        .expect("cast Angelic Edict at an enchantment");
    pass_both_players(&mut engine);
    assert_eq!(engine.state.objects[&enchantment].zone, Zone::Exile);

    let mut engine = main1_engine(335_006, &["angelic_edict"], &[]);
    let land = inject_permanent_on_battlefield(&mut engine, 1, "forest");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 4,
            ..Default::default()
        },
    );
    let spell = relocate_spell_to_hand(&mut engine, "angelic_edict");
    assert!(
        engine
            .apply_command(0, &cast_spell(spell, target_object(land)))
            .is_err(),
        "CR 601.2c: a land is not a legal creature-or-enchantment target"
    );
    assert_eq!(engine.state.objects[&land].zone, Zone::Battlefield);
}

#[test]
fn issue_335_angelic_edict_fizzles_when_its_only_target_leaves() {
    let mut engine = main1_engine(335_007, &["angelic_edict"], &["grizzly_bears"]);
    let bear = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            c: 4,
            ..Default::default()
        },
    );
    let spell = relocate_spell_to_hand(&mut engine, "angelic_edict");
    engine
        .apply_command(0, &cast_spell(spell, target_object(bear)))
        .expect("cast Angelic Edict");

    engine.state.players[1].battlefield.retain(|id| *id != bear);
    engine.state.players[1].graveyard.push(bear);
    engine.state.objects.get_mut(&bear).expect("bear").zone = Zone::Graveyard;
    *engine.state.zone_change_generation.entry(bear).or_default() += 1;

    pass_both_players(&mut engine);
    assert_eq!(
        engine.state.objects[&bear].zone,
        Zone::Graveyard,
        "CR 608.2b: the illegal target fizzles without being exiled"
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_335_adventurers_inn_gains_two_life_only_for_its_own_entry() {
    let mut engine = main1_engine(335_008, &["adventurers_inn"], &["adventurers_inn"]);
    move_ready_to_battlefield(&mut engine, 0, "adventurers_inn");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, 22);

    move_ready_to_battlefield(&mut engine, 0, "forest");
    assert!(
        engine.state.stack.is_empty(),
        "the land entry trigger is not a Landfall observer"
    );
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.state.players[0].life, 22);

    move_ready_to_battlefield(&mut engine, 1, "adventurers_inn");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].life, 22,
        "an opponent's land entering does not trigger this land's own entry ability"
    );

    let inn = battlefield_object_for_card(&engine, 0, "adventurers_inn");
    engine
        .apply_command(0, &activate_ability_for(&engine, inn, 0, vec![]))
        .expect("tap Adventurer's Inn for colorless mana");
    assert!(engine.state.objects[&inn].tapped);
    assert_eq!(engine.state.players[0].mana_pool.colorless, 1);
}

#[test]
fn issue_335_drix_fatemaker_anthem_reevaluates_live_counters() {
    let mut engine = main1_engine(335_009, &["drix_fatemaker"], &[]);
    // Enter Drix through the real entry path with no other creatures so its mandatory ETB has no
    // legal target; the counter-filtered anthem continuous effect is still established on entry.
    let drix = move_ready_to_battlefield(&mut engine, 0, "drix_fatemaker");
    resolve_entire_stack_two_player(&mut engine);
    let own = inject_creature_on_battlefield(&mut engine, 0, "grizzly_bears");
    let opposing = inject_creature_on_battlefield(&mut engine, 1, "grizzly_bears");
    assert!(
        !engine.effective_has_keyword(drix, tricerules_cards::Keyword::Trample),
        "the anthem excludes the counterless source itself"
    );

    assert!(
        !engine.effective_has_keyword(own, tricerules_cards::Keyword::Trample),
        "a controlled creature without a +1/+1 counter has no trample"
    );

    let timestamp = engine.state.command_index;
    engine
        .state
        .objects
        .get_mut(&own)
        .expect("own creature")
        .add_counters(CounterKind::PlusOnePlusOne, 1, timestamp);
    assert!(
        engine.effective_has_keyword(own, tricerules_cards::Keyword::Trample),
        "CR 611.3: the anthem grants trample while the counter is present"
    );

    let timestamp = engine.state.command_index;
    engine
        .state
        .objects
        .get_mut(&opposing)
        .expect("opposing creature")
        .add_counters(CounterKind::PlusOnePlusOne, 1, timestamp);
    assert!(
        !engine.effective_has_keyword(opposing, tricerules_cards::Keyword::Trample),
        "an opponent's countered creature is outside the YouControl scope"
    );

    engine
        .state
        .objects
        .get_mut(&own)
        .expect("own creature")
        .counters
        .remove(&CounterKind::PlusOnePlusOne);
    assert!(
        !engine.effective_has_keyword(own, tricerules_cards::Keyword::Trample),
        "the anthem re-evaluates live as the counter leaves"
    );
}

#[test]
fn issue_335_attercop_landfall_pump_expires_at_cleanup() {
    let mut engine = main1_engine(335_010, &["attercop"], &[]);
    let attercop = move_ready_to_battlefield(&mut engine, 0, "attercop");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(attercop), Some(2));
    assert_eq!(engine.effective_toughness(attercop), Some(1));

    move_ready_to_battlefield(&mut engine, 0, "forest");
    assert_eq!(engine.state.stack.len(), 1, "landfall trigger");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(attercop), Some(3));
    assert_eq!(engine.effective_toughness(attercop), Some(2));

    move_ready_to_battlefield(&mut engine, 1, "forest");
    assert!(
        engine.state.stack.is_empty(),
        "CR 603.6a: landfall is controller-scoped"
    );
    assert_eq!(engine.effective_power(attercop), Some(3));

    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.effective_power(attercop),
        Some(2),
        "CR 514.2: the pump expires at cleanup"
    );
    assert_eq!(engine.effective_toughness(attercop), Some(1));
}

#[test]
fn issue_335_strix_lookout_loots_with_a_private_discard() {
    let mut engine = main1_engine(335_011, &[], &[]);
    let source = inject_creature_on_battlefield(&mut engine, 0, "strix_lookout");
    let known = inject_card_into_hand(&mut engine, 0, "grizzly_bears");
    let hand_before = engine.state.players[0].hand.clone();
    let library_before = engine.state.players[0].library.len();
    give_mana(
        &mut engine,
        0,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );

    engine
        .apply_command(0, &activate_ability(source, 0, vec![]))
        .expect("activate {1}{U}, {T}: loot");
    assert!(engine.state.objects[&source].tapped);
    assert_eq!(engine.state.stack.len(), 1);
    engine
        .apply_command(0, &pass())
        .expect("active player passes");
    let parked = engine
        .apply_command(1, &pass())
        .expect("ability resolves to the discard choice");

    let choice = find_resolution_choice(&parked).expect("mandatory discard choice");
    assert_eq!(choice.deciding_player_id, 0);
    assert_eq!(choice.choice_kind(), ChoiceKind::HandCards);
    assert_eq!((choice.min, choice.max), (1, 1));
    assert!(
        choice.public_reveal.is_none(),
        "CR 701.9: the controller's discard choice stays private"
    );
    assert_eq!(
        engine.state.players[0].library.len(),
        library_before - 1,
        "the draw happens before the discard choice"
    );
    assert_eq!(engine.state.players[0].hand.len(), hand_before.len() + 1);
    let drawn = engine.state.players[0]
        .hand
        .iter()
        .find(|object_id| !hand_before.contains(object_id))
        .copied()
        .expect("drawn card");
    assert!(choice.candidate_object_ids.contains(&drawn));

    engine
        .apply_command(0, &submit_resolution_choice(vec![known]))
        .expect("discard the known card");
    assert!(engine.state.players[0].graveyard.contains(&known));
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_335_scenarios_reach_main1() {
    let engine = main1_engine(335_012, &[], &[]);
    assert_eq!(engine.state.turn_step, TurnStep::Main1);
}
