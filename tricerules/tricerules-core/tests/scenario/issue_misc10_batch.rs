//! Reviewed direct-RON activated-ability and token-entry scenarios: Umbral Collar Zealot,
//! Bold Biochemist, Hardened Tactician, Wildheart Invoker, Sting-Slinger, Tunnel Surveyor,
//! Fire Nation Raider and Sami's Curiosity.
//!
//! Exact Scryfall records and `rulings_uri` were fetched 2026-09-21 against the pinned snapshot.
//! Every expectation is the reviewed printed Oracle behavior. Governance: CR 115 (targets),
//! CR 118.12 (additional costs), CR 119 (life), CR 120.3 (damage), CR 301.5 (Clue/Lander),
//! CR 602 (activated abilities), CR 603.4 (intervening-if), CR 603.6 (entry triggers),
//! CR 611.2c (until end of turn), CR 701.25 (surveil), CR 701.68 (blight), and CR 702.177
//! (power-up/exhaust activation limit).

use super::helpers::*;
use tricerules_cards::primitives::CounterKind;
use tricerules_cards::Keyword;
use tricerules_core::{TurnStep, Zone};
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ChoiceKind, CostObjectRef, CostObjectRefs, CostSelection,
};

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("swamp", &[]), deck_with("island", &[])]);
    let mut e = GameEngine::new(seed, &[0, 1], 20, decks, true).expect("engine");
    advance_to_main1_from_game_start(&mut e);
    grant_pool(&mut e, 0);
    grant_pool(&mut e, 1);
    e
}

fn cast(
    e: &mut GameEngine,
    player: i32,
    card_id: &str,
    targets: Vec<tricerules_proto::ruled::v1::TargetRef>,
) {
    inject_card_into_hand(e, player as usize, card_id);
    grant_pool(e, player as usize);
    let slot = hand_index_for_card(e, player as usize, card_id);
    semantic::accepted(e, player, &cast_spell(slot, targets));
    resolve_entire_stack_two_player(e);
}

fn blight_selection(cost_index: u32, object_id: u32, zone_change_generation: u64) -> CostSelection {
    CostSelection {
        cost_index,
        selection: Some(Selection::BattlefieldObjects(CostObjectRefs {
            objects: vec![CostObjectRef {
                object_id,
                zone_change_generation,
            }],
        })),
    }
}

fn generation(e: &GameEngine, oid: u32) -> u64 {
    e.state
        .zone_change_generation
        .get(&oid)
        .copied()
        .unwrap_or_default()
}

fn minus_one(e: &GameEngine, oid: u32) -> u32 {
    e.state.objects[&oid].counter_count(CounterKind::MinusOneMinusOne)
}

fn advance_to_main2(engine: &mut GameEngine, active: i32) {
    for _ in 0..40 {
        if engine.state.turn_step == TurnStep::Main2 && engine.state.priority_player_id() == active
        {
            return;
        }
        let priority = engine.state.priority_player_id();
        engine
            .apply_command(priority, &pass())
            .expect("advance to main 2");
    }
    panic!("stalled before main 2");
}

#[test]
fn issue_misc10_umbral_collar_zealot_sacrifices_for_surveil() {
    let mut e = engine(734_001);
    let zealot = inject_creature_on_battlefield(&mut e, 0, "umbral_collar_zealot");
    let fodder = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    semantic::accepted(
        &mut e,
        0,
        &activate_ability_with_costs(zealot, 0, vec![], vec![permanent_cost_selection(0, fodder)]),
    );
    e.apply_command(0, &pass()).expect("caster passes");
    let batch = e.apply_command(1, &pass()).expect("the ability resolves");
    assert_eq!(e.state.objects[&fodder].zone, Zone::Graveyard);
    let choice = find_resolution_choice(&batch).expect("surveil choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibraryLook);
    semantic::accepted(&mut e, 0, &submit_resolution_choice(vec![]));
    assert!(e.state.blocking_choice().is_none());

    // The source alone cannot pay the "another creature or artifact" cost.
    let mut alone = engine(734_011);
    let zealot = inject_creature_on_battlefield(&mut alone, 0, "umbral_collar_zealot");
    assert!(
        alone
            .apply_command(0, &activate_ability(zealot, 0, vec![]))
            .is_err(),
        "the source is excluded from its own sacrifice cost"
    );
}

#[test]
fn issue_misc10_bold_biochemist_power_up_is_once_per_object() {
    let mut e = engine(734_002);
    let biochemist = inject_creature_on_battlefield(&mut e, 0, "bold_biochemist");
    let hand_before = e.state.players[0].hand.len();
    semantic::accepted(&mut e, 0, &activate_ability(biochemist, 0, vec![]));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&biochemist].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(e.state.players[0].hand.len(), hand_before + 2, "draw two");

    // Power-up: activate each power-up ability only once.
    assert!(
        e.apply_command(0, &activate_ability(biochemist, 0, vec![]))
            .is_err(),
        "the power-up ability cannot be activated a second time"
    );
}

#[test]
fn issue_misc10_hardened_tactician_sacrifices_a_token() {
    let mut e = engine(734_003);
    let tactician = inject_creature_on_battlefield(&mut e, 0, "hardened_tactician");
    let token = inject_permanent_on_battlefield(&mut e, 0, "treasure");
    let hand_before = e.state.players[0].hand.len();
    semantic::accepted(
        &mut e,
        0,
        &activate_ability_with_costs(
            tactician,
            0,
            vec![],
            vec![permanent_cost_selection(1, token)],
        ),
    );
    resolve_entire_stack_two_player(&mut e);
    assert!(
        !e.state.players[0].battlefield.contains(&token),
        "the sacrificed token left the battlefield (and ceased to exist)"
    );
    assert_eq!(e.state.players[0].hand.len(), hand_before + 1);

    // A nontoken permanent is not a legal sacrifice for "Sacrifice a token".
    let mut bad = engine(734_013);
    let tactician = inject_creature_on_battlefield(&mut bad, 0, "hardened_tactician");
    let bear = inject_creature_on_battlefield(&mut bad, 0, "grizzly_bears");
    assert!(
        bad.apply_command(
            0,
            &activate_ability_with_costs(
                tactician,
                0,
                vec![],
                vec![permanent_cost_selection(1, bear)],
            ),
        )
        .is_err(),
        "a nontoken creature is not a token"
    );
}

#[test]
fn issue_misc10_wildheart_invoker_pumps_and_grants_trample() {
    let mut e = engine(734_004);
    let invoker = inject_creature_on_battlefield(&mut e, 0, "wildheart_invoker");
    let target = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let base_power = e.effective_power(target).expect("power");
    let base_toughness = e.effective_toughness(target).expect("toughness");
    semantic::accepted(
        &mut e,
        0,
        &activate_ability(invoker, 0, target_object(target)),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(target), Some(base_power + 5));
    assert_eq!(e.effective_toughness(target), Some(base_toughness + 5));
    assert!(e.effective_has_keyword(target, Keyword::Trample));
}

#[test]
fn issue_misc10_sting_slinger_blights_for_damage() {
    let mut e = engine(734_005);
    let slinger = inject_creature_on_battlefield(&mut e, 0, "sting-slinger");
    let fodder = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let p1_before = e.state.players[1].life;
    let fodder_generation = generation(&e, fodder);
    semantic::accepted(
        &mut e,
        0,
        &activate_ability_with_costs(
            slinger,
            0,
            vec![],
            vec![blight_selection(2, fodder, fodder_generation)],
        ),
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(minus_one(&e, fodder), 1, "blight 1 placed a -1/-1 counter");
    assert_eq!(
        e.state.players[1].life,
        p1_before - 2,
        "2 damage to the opponent"
    );
}

#[test]
fn issue_misc10_tunnel_surveyor_creates_a_glimmer() {
    let mut e = engine(734_006);
    cast(&mut e, 0, "tunnel_surveyor", vec![]);
    assert_eq!(battlefield_token_oids(&e, 0, "glimmer_w_1_1").len(), 1);
}

#[test]
fn issue_misc10_fire_nation_raider_raid_creates_a_clue_only_after_attacking() {
    // Without attacking this turn, the raid intervening-if fails.
    let mut no_attack = engine(734_007);
    cast(&mut no_attack, 0, "fire_nation_raider", vec![]);
    assert_eq!(battlefield_token_oids(&no_attack, 0, "clue").len(), 0);

    // After attacking this turn, entering creates a Clue.
    let mut e = GameEngine::new(734_017, &[0, 1], 20, None, true).expect("engine");
    advance_to_declare_attackers(&mut e);
    let attacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    e.apply_command(0, &declare_attackers(vec![attacker]))
        .expect("declare an attacker");
    advance_to_main2(&mut e, 0);
    grant_pool(&mut e, 0);
    inject_card_into_hand(&mut e, 0, "fire_nation_raider");
    let slot = hand_index_for_card(&e, 0, "fire_nation_raider");
    semantic::accepted(&mut e, 0, &cast_spell(slot, vec![]));
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        battlefield_token_oids(&e, 0, "clue").len(),
        1,
        "the raid trigger creates a Clue after attacking"
    );
}

#[test]
fn issue_misc10_samis_curiosity_gains_life_and_makes_a_lander() {
    let mut e = engine(734_008);
    let life_before = e.state.players[0].life;
    cast(&mut e, 0, "samis_curiosity", vec![]);
    assert_eq!(e.state.players[0].life, life_before + 2);
    assert_eq!(battlefield_token_oids(&e, 0, "lander").len(), 1);
}
