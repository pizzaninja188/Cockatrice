//! Issue #327 — the eight reviewed counter, pump, and trigger cards.
//!
//! These scenarios drive the generated definitions through the authoritative command path.
//! CR 122 governs +1/+1 counter placement; CR 602/601.2h and 611.2a govern the activated
//! single-{R} self-pump and its cleanup expiry; CR 611.2a/514.2 govern the -4/-4 pump; CR 611.3
//! and 613.4c govern the live attacking-creature anthem (attacking per CR 508.1k); CR 603.2b/513.2
//! and 701.21 govern the each-end-step sacrifice; CR 107.5 and 701.9 govern the tap-plus-discard
//! activation cost; CR 608.2h governs the ETB mass counter snapshot excluding the source; and
//! CR 603.6c/700.4 govern the another-creature-dies counter trigger and its controller scope.

use super::helpers::*;
use tricerules_cards::CounterKind;
use tricerules_core::{TurnStep, Zone};

fn main1_engine(seed: u64) -> GameEngine {
    let mut engine = GameEngine::new(seed, &[0, 1], 20, None, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn spell_engine(seed: u64, own: &[&str], opposing: &[&str]) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", own),
        deck_with("island", opposing),
    ]);
    let mut engine = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut engine);
    engine
}

fn enter_declare_attackers(engine: &mut GameEngine) {
    engine
        .apply_command(0, &primitive_yield())
        .expect("main1 to beginning of combat");
    pass_both_players(engine);
    assert_eq!(engine.state.turn_step, TurnStep::DeclareAttackers);
}

fn advance_to_main2(engine: &mut GameEngine) {
    for _ in 0..20 {
        let actor = engine.state.priority_player_id();
        engine
            .apply_command(actor, &pass())
            .expect("pass through combat");
        if engine.state.turn_step == TurnStep::Main2 {
            return;
        }
    }
    panic!("combat did not reach main2");
}

fn advance_to_end_step(engine: &mut GameEngine, active: i32) {
    engine
        .apply_command(active, &primitive_yield())
        .expect("main 1 to beginning of combat");
    engine
        .apply_command(active, &primitive_yield())
        .expect("beginning of combat advance");
    if engine.state.turn_step == TurnStep::DeclareAttackers {
        engine
            .apply_command(active, &primitive_yield())
            .expect("declare no attackers");
    }
    engine
        .apply_command(active, &primitive_yield())
        .expect("end combat to main 2");
    engine
        .apply_command(active, &primitive_yield())
        .expect("main 2 to end step");
    assert_eq!(engine.state.turn_step, TurnStep::EndStep);
}

fn plus_one_counters(engine: &GameEngine, object_id: u32) -> u32 {
    engine.state.objects[&object_id].counter_count(CounterKind::PlusOnePlusOne)
}

#[test]
fn issue_327_honor_places_one_counter_and_draws() {
    let mut engine = spell_engine(327_001, &["honor"], &[]);
    let bear = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 2, 2);
    ensure_in_hand(&mut engine, 0, "honor");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let library_before = engine.state.players[0].library.len();

    let honor = hand_index_for_card(&engine, 0, "honor");
    engine
        .apply_command(0, &cast_spell(honor, target_object(bear)))
        .expect("cast Honor");
    resolve_entire_stack_two_player(&mut engine);

    assert_eq!(
        plus_one_counters(&engine, bear),
        1,
        "CR 122: Honor puts one +1/+1 counter on the chosen target"
    );
    assert_eq!(engine.effective_power(bear), Some(3));
    assert_eq!(engine.effective_toughness(bear), Some(3));
    assert_eq!(
        engine.state.players[0].library.len(),
        library_before - 1,
        "Honor still draws a card after the counter is placed"
    );
    assert!(engine.state.stack.is_empty());
}

#[test]
fn issue_327_honor_fizzles_without_a_legal_target() {
    let mut engine = spell_engine(327_002, &["honor"], &[]);
    ensure_in_hand(&mut engine, 0, "honor");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            w: 1,
            ..Default::default()
        },
    );
    let library_before = engine.state.players[0].library.len();
    // No creatures are on the battlefield, so the mandatory target cannot be chosen.
    let honor = hand_index_for_card(&engine, 0, "honor");
    assert!(
        engine.apply_command(0, &cast_spell(honor, vec![])).is_err(),
        "CR 601.2c: Honor cannot be cast without its mandatory target"
    );
    assert_eq!(engine.state.players[0].library.len(), library_before);
}

#[test]
fn issue_327_shivan_dragon_self_pump_applies_and_expires() {
    let mut engine = main1_engine(327_003);
    let dragon = inject_creature_with_stats(&mut engine, 0, "shivan_dragon", 5, 5);
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );

    engine
        .apply_command(0, &activate_ability(dragon, 0, vec![]))
        .expect("activate the {R} self-pump");
    assert_eq!(
        engine.state.players[0].mana_pool.red, 0,
        "the {{R}} cost is paid on activation"
    );
    pass_both_players(&mut engine);
    assert_eq!(engine.effective_power(dragon), Some(6), "+1/+0 applies");
    assert_eq!(
        engine.effective_toughness(dragon),
        Some(5),
        "toughness is unchanged"
    );

    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.effective_power(dragon),
        Some(5),
        "CR 514.2: the pump expires at cleanup"
    );
}

#[test]
fn issue_327_dark_deed_kills_a_creature() {
    let mut engine = spell_engine(327_004, &["dark_deed"], &[]);
    let victim = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 2);
    ensure_in_hand(&mut engine, 0, "dark_deed");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );

    let dark_deed = hand_index_for_card(&engine, 0, "dark_deed");
    engine
        .apply_command(0, &cast_spell(dark_deed, target_object(victim)))
        .expect("cast Dark Deed at the 2/2");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&victim].zone,
        Zone::Graveyard,
        "CR 704.5f: -4/-4 sends a 2/2 to the graveyard as a state-based action"
    );
}

#[test]
fn issue_327_dark_deed_pump_expires() {
    let mut engine = spell_engine(327_005, &["dark_deed"], &[]);
    let survivor = inject_creature_with_stats(&mut engine, 0, "colossal_dreadmaw", 6, 6);
    ensure_in_hand(&mut engine, 0, "dark_deed");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );

    let dark_deed = hand_index_for_card(&engine, 0, "dark_deed");
    engine
        .apply_command(0, &cast_spell(dark_deed, target_object(survivor)))
        .expect("cast Dark Deed at the 6/6");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(engine.effective_power(survivor), Some(2));
    assert_eq!(engine.effective_toughness(survivor), Some(2));

    end_active_turn(&mut engine, 0);
    assert_eq!(
        engine.effective_power(survivor),
        Some(6),
        "the -4/-4 pump expires at cleanup"
    );
    assert_eq!(engine.effective_toughness(survivor), Some(6));
}

#[test]
fn issue_327_goblin_oriflamme_anthems_only_current_attackers() {
    let mut engine = spell_engine(327_006, &["goblin_oriflamme"], &[]);
    ensure_in_hand(&mut engine, 0, "goblin_oriflamme");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            r: 2,
            ..Default::default()
        },
    );
    let oriflamme = hand_index_for_card(&engine, 0, "goblin_oriflamme");
    engine
        .apply_command(0, &cast_spell(oriflamme, vec![]))
        .expect("cast Goblin Oriflamme");
    resolve_entire_stack_two_player(&mut engine);

    let attacker = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 2, 2);
    let bystander = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 2, 2);
    let opponent = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 2);
    enter_declare_attackers(&mut engine);
    engine
        .apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare one attacker");

    assert_eq!(
        engine.effective_power(attacker),
        Some(3),
        "CR 508.1k: an attacking creature you control gets +1/+0"
    );
    assert_eq!(
        engine.effective_power(bystander),
        Some(2),
        "a nonattacking friendly creature is outside the scope"
    );
    assert_eq!(
        engine.effective_power(opponent),
        Some(2),
        "an opponent's attacking-status creature is outside the scope"
    );

    advance_to_main2(&mut engine);
    assert_eq!(
        engine.effective_power(attacker),
        Some(2),
        "CR 613.4: the anthem re-evaluates continuously and stops applying after combat"
    );
}

#[test]
fn issue_327_ball_lightning_sacrifices_itself_at_the_end_step() {
    let mut engine = main1_engine(327_007);
    let ball = inject_creature_with_stats(&mut engine, 0, "ball_lightning", 6, 1);
    assert!(engine.effective_has_keyword(ball, tricerules_cards::Keyword::Trample));
    assert!(engine.effective_has_keyword(ball, tricerules_cards::Keyword::Haste));

    advance_to_end_step(&mut engine, 0);
    assert_eq!(
        engine.state.stack.len(),
        1,
        "CR 513.1a: the end-step trigger fires when the end step begins"
    );
    assert_eq!(
        engine.state.objects[&ball].zone,
        Zone::Battlefield,
        "the source has not been sacrificed before the trigger resolves"
    );

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.objects[&ball].zone,
        Zone::Graveyard,
        "CR 701.21: the trigger sacrifices the source"
    );
}

#[test]
fn issue_327_charging_strifeknight_requires_tap_and_discard() {
    let mut engine = spell_engine(327_008, &["charging_strifeknight"], &[]);
    let knight = relocate_to_battlefield(&mut engine, 0, "charging_strifeknight", false);
    let discard_slot = hand_index_for_card(&engine, 0, "forest");
    let discarded = engine.state.players[0].hand[discard_slot];
    let hand_before = engine.state.players[0].hand.len();
    let library_before = engine.state.players[0].library.len();

    // Both costs are mandatory: omitting the discard selection is illegal and rolls back the tap.
    assert!(
        engine
            .apply_command(0, &activate_ability(knight, 0, vec![]))
            .is_err(),
        "the discard cost cannot be skipped"
    );
    assert!(
        !engine.state.objects[&knight].tapped,
        "a rejected activation must not leave the source tapped"
    );

    engine
        .apply_command(
            0,
            &activate_ability_with_costs(
                knight,
                0,
                vec![],
                vec![hand_cost_selection(1, discard_slot as u32)],
            ),
        )
        .expect("pay {T} and discard a card");
    assert!(
        engine.state.objects[&knight].tapped,
        "the tap symbol is paid as a cost"
    );
    assert_eq!(
        engine.state.objects[&discarded].zone,
        Zone::Graveyard,
        "the chosen card is discarded as a cost"
    );

    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        engine.state.players[0].hand.len(),
        hand_before,
        "discarding one and drawing one leaves the hand size unchanged"
    );
    assert_eq!(
        engine.state.players[0].library.len(),
        library_before - 1,
        "the activation draws one card"
    );
}

#[test]
fn issue_327_web_warriors_counters_each_other_controlled_creature() {
    let mut engine = spell_engine(327_009, &["web-warriors"], &[]);
    let friendly = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 2, 2);
    let other_friendly = inject_creature_with_stats(&mut engine, 0, "savannah_lions", 2, 1);
    let opponent = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 2);

    ensure_in_hand(&mut engine, 0, "web-warriors");
    give_mana(
        &mut engine,
        0,
        ManaGift {
            g: 1,
            c: 4,
            ..Default::default()
        },
    );
    let web_warriors = hand_index_for_card(&engine, 0, "web-warriors");
    engine
        .apply_command(0, &cast_spell(web_warriors, vec![]))
        .expect("cast Web-Warriors");
    resolve_entire_stack_two_player(&mut engine);

    let source = battlefield_object_for_card(&engine, 0, "web-warriors");
    assert_eq!(
        plus_one_counters(&engine, friendly),
        1,
        "each other controlled creature gets one +1/+1 counter"
    );
    assert_eq!(plus_one_counters(&engine, other_friendly), 1);
    assert_eq!(
        plus_one_counters(&engine, source),
        0,
        "CR 122 / 608.2h: the entering source excludes itself"
    );
    assert_eq!(
        plus_one_counters(&engine, opponent),
        0,
        "the scope is the controller's creatures only"
    );
    assert_eq!(engine.effective_power(friendly), Some(3));
    assert_eq!(engine.effective_toughness(friendly), Some(3));
}

#[test]
fn issue_327_voracious_vermin_counts_only_another_controlled_creature() {
    let mut engine = main1_engine(327_010);
    let vermin = inject_creature_with_stats(&mut engine, 0, "voracious_vermin", 2, 1);
    let friendly = inject_creature_with_stats(&mut engine, 0, "grizzly_bears", 2, 2);

    // A friendly creature dies: the observer trigger fires and counters the source.
    engine
        .state
        .objects
        .get_mut(&friendly)
        .expect("bears")
        .damage = 2;
    let priority = engine.state.priority_player_id();
    engine
        .apply_command(priority, &pass())
        .expect("lethal-damage SBA");
    assert_eq!(engine.state.objects[&friendly].zone, Zone::Graveyard);
    assert_eq!(engine.state.stack.len(), 1, "one dies trigger is pending");
    resolve_entire_stack_two_player(&mut engine);
    assert_eq!(
        plus_one_counters(&engine, vermin),
        1,
        "the source receives one +1/+1 counter"
    );

    // An opponent's creature dies: controller scope suppresses the trigger.
    let opposing = inject_creature_with_stats(&mut engine, 1, "grizzly_bears", 2, 2);
    engine
        .state
        .objects
        .get_mut(&opposing)
        .expect("bears")
        .damage = 2;
    let priority = engine.state.priority_player_id();
    engine
        .apply_command(priority, &pass())
        .expect("opposing death SBA");
    assert_eq!(engine.state.objects[&opposing].zone, Zone::Graveyard);
    assert!(
        engine.state.stack.is_empty(),
        "CR 700.4 / 603.6: another player's creature does not satisfy the controller scope"
    );
    assert_eq!(plus_one_counters(&engine, vermin), 1);

    // The source's own death is excluded by "another".
    engine
        .state
        .objects
        .get_mut(&vermin)
        .expect("vermin")
        .damage = 2;
    let priority = engine.state.priority_player_id();
    engine
        .apply_command(priority, &pass())
        .expect("self death SBA");
    assert_eq!(engine.state.objects[&vermin].zone, Zone::Graveyard);
    assert!(
        engine.state.stack.is_empty(),
        "\"another creature\" excludes the source's own death"
    );
}
