//! Actual-card scenarios for the queue-v45 reuse-only cohorts.
//!
//! Expectations are printed Oracle values. CR 601.2a and 603.1/603.2 govern the cast triggers;
//! CR 503.1 and 603.4 govern the upkeep triggers and their intervening "if"; CR 118.12 governs
//! optional mana payments; CR 701.21a governs sacrifice as a cost; CR 701.26 governs tapping;
//! CR 611.2a/514.2 govern until-end-of-turn pumps; CR 400.7 and 608.2b govern hand returns and
//! stale targets; CR 111 governs the created tokens.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_core::GameEngine;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_command::Cmd;
use tricerules_proto::ruled::v1::{
    ChoiceKind, ResolutionChoiceDecision, RuledCommand, SubmitResolutionChoice, TargetRef,
};

type ManaPool = (u32, u32, u32, u32, u32, u32);

fn pool(e: &GameEngine) -> ManaPool {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn base_engine(seed: u64) -> GameEngine {
    let decks = Some(vec![
        deck_with(
            "forest",
            &[
                "grizzly_bears",
                "white_knight",
                "air_elemental",
                "gray_ogre",
                "scathe_zombies",
            ],
        ),
        deck_with(
            "island",
            &[
                "grizzly_bears",
                "white_knight",
                "air_elemental",
                "gray_ogre",
                "scathe_zombies",
            ],
        ),
    ]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    e
}

fn select_branch(index: u32) -> RuledCommand {
    RuledCommand {
        cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
            decision: ResolutionChoiceDecision::SelectBranch as i32,
            selected_branch_index: index,
            ..Default::default()
        })),
    }
}

fn give_pool(e: &mut GameEngine, player: i32, gift: ManaGift) {
    give_mana(e, player, gift);
}

fn gift(m: &ManaGift) -> ManaGift {
    ManaGift {
        w: m.w,
        u: m.u,
        b: m.b,
        r: m.r,
        g: m.g,
        c: m.c,
    }
}

fn cast_named(
    e: &mut GameEngine,
    player: i32,
    card: &str,
    mana: ManaGift,
    targets: Vec<TargetRef>,
) {
    inject_card_into_hand(e, player as usize, card);
    give_pool(e, player, mana);
    let slot = hand_index_for_card(e, player as usize, card);
    e.apply_scenario_command(player, &cast_spell(slot, targets))
        .unwrap_or_else(|error| panic!("cast {card}: {error:?}"));
}

fn pass_to_trigger_window(e: &mut GameEngine) -> tricerules_proto::ruled::v1::RuledEventBatch {
    // The cast trigger is on top of the spell; both players passing begins its resolution.
    pass_both_players_with_batch(e)
}

fn pay_one(e: &mut GameEngine, branch_index: u32) {
    e.apply_scenario_command(0, &select_branch(branch_index))
        .expect("select the pay branch");
    assert_eq!(
        e.state
            .pending_resolution
            .as_ref()
            .expect("mana payment offer")
            .presentation
            .choice_kind,
        ChoiceKind::ManaPayment
    );
    submit_mana_resolution_decision(e, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay the branch cost and resume the branch");
}

fn advance_to_active_player_main1(e: &mut GameEngine, player: i32) {
    for _ in 0..80 {
        answer_trigger_order_in_engine_order(e);
        if e.state.active_player_id() == player
            && e.state.turn_step == tricerules_core::TurnStep::Main1
        {
            return;
        }
        let actor = e.state.priority_player_id();
        e.apply_scenario_command(actor, &pass())
            .expect("pass toward the requested main phase");
    }
    panic!("game did not reach player {player}'s first main phase");
}

fn advance_to_active_player_upkeep(e: &mut GameEngine, player: i32) {
    for _ in 0..60 {
        let (actor, command) = match e.state.cleanup_discard_player {
            Some(cleanup_player) => {
                let player_index = e.state.player_idx(cleanup_player).expect("cleanup player");
                let excess = e.state.players[player_index].hand.len().saturating_sub(7);
                (
                    cleanup_player,
                    discard_cleanup_batch((0..excess as u32).collect()),
                )
            }
            None => (e.state.priority_player_id(), pass()),
        };
        e.apply_scenario_command(actor, &command)
            .expect("pass through turn");
        if e.state.active_player_id() == player
            && e.state.turn_step == tricerules_core::TurnStep::Upkeep
        {
            answer_trigger_order_in_engine_order(e);
            return;
        }
    }
    panic!("game did not reach player {player}'s upkeep");
}

// ── 01 spell-life artifacts with an optional {1} ─────────────────────────────

struct LifeCase {
    id: &'static str,
    color: &'static str,
    spell: &'static str,
    mana: ManaGift,
}

fn life_pay_cases() -> [LifeCase; 6] {
    [
        LifeCase {
            id: "wooden_sphere",
            color: "green",
            spell: "grizzly_bears",
            mana: ManaGift {
                g: 2,
                c: 1,
                ..Default::default()
            },
        },
        LifeCase {
            id: "ivory_cup",
            color: "white",
            spell: "white_knight",
            mana: ManaGift {
                w: 2,
                c: 2,
                ..Default::default()
            },
        },
        LifeCase {
            id: "crystal_rod",
            color: "blue",
            spell: "air_elemental",
            mana: ManaGift {
                u: 4,
                c: 2,
                ..Default::default()
            },
        },
        LifeCase {
            id: "iron_star",
            color: "red",
            spell: "gray_ogre",
            mana: ManaGift {
                r: 2,
                c: 2,
                ..Default::default()
            },
        },
        LifeCase {
            id: "throne_of_bone",
            color: "black",
            spell: "scathe_zombies",
            mana: ManaGift {
                b: 2,
                c: 2,
                ..Default::default()
            },
        },
        LifeCase {
            id: "apothecary_initiate",
            color: "white",
            spell: "white_knight",
            mana: ManaGift {
                w: 2,
                c: 2,
                ..Default::default()
            },
        },
    ]
}

#[test]
fn q45_spell_life_pay_canary_simplest_pays_and_gains() {
    let case = &life_pay_cases()[0];
    let mut e = base_engine(45_010_001);
    inject_permanent_on_battlefield(&mut e, 0, case.id);
    let before = e.state.players[0].life;
    cast_named(&mut e, 0, case.spell, gift(&case.mana), vec![]);
    let batch = pass_to_trigger_window(&mut e);
    let choice = find_resolution_choice(&batch).expect("pay-or-pass branch");
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    assert_eq!(choice.deciding_player_id, 0);
    pay_one(&mut e, 0);
    assert_eq!(
        e.state.players[0].life,
        before + 1,
        "paying one mana gains exactly one life"
    );
    assert_eq!(
        pool(&e),
        (0, 0, 0, 0, 0, 0),
        "the one-mana payment and the green spell cost consume the pool"
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&battlefield_object_for_card(&e, 0, case.spell)].zone,
        Zone::Battlefield
    );
}

#[test]
fn q45_spell_life_pay_canary_hardest_creature_has_the_same_trigger() {
    let case = &life_pay_cases()[5];
    let mut e = base_engine(45_010_002);
    inject_permanent_on_battlefield(&mut e, 0, case.id);
    let before = e.state.players[0].life;
    cast_named(&mut e, 0, case.spell, gift(&case.mana), vec![]);
    let batch = pass_to_trigger_window(&mut e);
    assert!(find_resolution_choice(&batch).is_some());
    pay_one(&mut e, 0);
    assert_eq!(e.state.players[0].life, before + 1);
}

#[test]
fn q45_spell_life_pay_cards_trigger_for_every_color_and_any_caster() {
    for (index, case) in life_pay_cases().iter().enumerate() {
        let mut e = base_engine(45_011_000 + index as u64);
        inject_permanent_on_battlefield(&mut e, 0, case.id);
        let before = e.state.players[0].life;
        advance_to_active_player_main1(&mut e, 1);
        cast_named(&mut e, 1, case.spell, gift(&case.mana), vec![]);
        give_pool(
            &mut e,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        let batch = pass_to_trigger_window(&mut e);
        let choice = find_resolution_choice(&batch)
            .unwrap_or_else(|| panic!("{}: trigger did not offer payment", case.id));
        assert_eq!(choice.deciding_player_id, 0, "{}: controller pays", case.id);
        pay_one(&mut e, 0);
        assert_eq!(
            e.state.players[0].life,
            before + 1,
            "{}: an opponent's {} spell still triggers the artifact",
            case.id,
            case.color
        );
    }
}

#[test]
fn q45_spell_life_pay_cards_may_decline_without_life_or_payment() {
    let case = &life_pay_cases()[0];
    let mut e = base_engine(45_012_001);
    inject_permanent_on_battlefield(&mut e, 0, case.id);
    let before = e.state.players[0].life;
    cast_named(&mut e, 0, case.spell, gift(&case.mana), vec![]);
    let pool_after_cast = pool(&e);
    pass_to_trigger_window(&mut e);
    e.apply_scenario_command(
        0,
        &submit_resolution_decision(ResolutionChoiceDecision::Decline),
    )
    .expect("decline the optional payment");
    assert_eq!(e.state.players[0].life, before, "declining gains no life");
    assert_eq!(pool(&e), pool_after_cast, "declining pays no extra mana");
}

// ── 02 spell-life artifacts without a payment ────────────────────────────────

fn life_free_cases() -> [(&'static str, &'static str, ManaGift); 5] {
    [
        (
            "krakens_eye",
            "air_elemental",
            ManaGift {
                u: 5,
                ..Default::default()
            },
        ),
        (
            "angels_feather",
            "white_knight",
            ManaGift {
                w: 2,
                ..Default::default()
            },
        ),
        (
            "wurms_tooth",
            "grizzly_bears",
            ManaGift {
                g: 2,
                ..Default::default()
            },
        ),
        (
            "dragons_claw",
            "gray_ogre",
            ManaGift {
                r: 3,
                ..Default::default()
            },
        ),
        (
            "demons_horn",
            "scathe_zombies",
            ManaGift {
                b: 3,
                ..Default::default()
            },
        ),
    ]
}

#[test]
fn q45_spell_life_free_canary_simplest_may_gain_one() {
    let cases = life_free_cases();
    let (id, spell, mana) = &cases[0];
    let mut e = base_engine(45_020_001);
    inject_permanent_on_battlefield(&mut e, 0, id);
    let before = e.state.players[0].life;
    cast_named(&mut e, 0, spell, gift(mana), vec![]);
    let batch = pass_to_trigger_window(&mut e);
    let choice = find_resolution_choice(&batch).expect("optional gain choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    e.apply_scenario_command(0, &select_branch(0))
        .expect("choose to gain one life");
    assert_eq!(e.state.players[0].life, before + 1);
}

#[test]
fn q45_spell_life_free_canary_hardest_may_be_declined() {
    let cases = life_free_cases();
    let (id, spell, mana) = &cases[4];
    let mut e = base_engine(45_020_002);
    inject_permanent_on_battlefield(&mut e, 0, id);
    let before = e.state.players[0].life;
    cast_named(&mut e, 0, spell, gift(mana), vec![]);
    pass_to_trigger_window(&mut e);
    e.apply_scenario_command(
        0,
        &submit_resolution_decision(ResolutionChoiceDecision::Decline),
    )
    .expect("decline the optional life gain");
    assert_eq!(e.state.players[0].life, before);
}

#[test]
fn q45_spell_life_free_cards_trigger_for_any_caster_and_each_color() {
    for (index, (id, spell, mana)) in life_free_cases().iter().enumerate() {
        let mut e = base_engine(45_021_000 + index as u64);
        inject_permanent_on_battlefield(&mut e, 0, id);
        let before = e.state.players[0].life;
        advance_to_active_player_main1(&mut e, 1);
        cast_named(&mut e, 1, spell, gift(mana), vec![]);
        pass_to_trigger_window(&mut e);
        e.apply_scenario_command(0, &select_branch(0))
            .unwrap_or_else(|error| panic!("{id}: {error:?}"));
        assert_eq!(e.state.players[0].life, before + 1, "{id}");
    }
}

// ── 03 upkeep sacrifice unless the printed mana is paid ──────────────────────

struct UpkeepCase {
    id: &'static str,
    cost: ManaGift,
    body: (u32, u32),
    flying: bool,
}

fn upkeep_cases() -> [UpkeepCase; 6] {
    [
        UpkeepCase {
            id: "darba",
            cost: ManaGift {
                g: 2,
                ..Default::default()
            },
            body: (5, 4),
            flying: false,
        },
        UpkeepCase {
            id: "hungry_mist",
            cost: ManaGift {
                g: 2,
                ..Default::default()
            },
            body: (6, 2),
            flying: false,
        },
        UpkeepCase {
            id: "school_of_piranha",
            cost: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
            body: (3, 3),
            flying: false,
        },
        UpkeepCase {
            id: "phantasmal_forces",
            cost: ManaGift {
                u: 1,
                ..Default::default()
            },
            body: (4, 1),
            flying: true,
        },
        UpkeepCase {
            id: "spindrift_drake",
            cost: ManaGift {
                u: 1,
                ..Default::default()
            },
            body: (2, 1),
            flying: true,
        },
        UpkeepCase {
            id: "molting_harpy",
            cost: ManaGift {
                c: 2,
                ..Default::default()
            },
            body: (2, 1),
            flying: true,
        },
    ]
}

fn upkeep_choice(
    e: &mut GameEngine,
    case: &UpkeepCase,
) -> (u32, tricerules_proto::ruled::v1::RuledEventBatch) {
    let source = inject_permanent_on_battlefield(e, 0, case.id);
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(case.body.0), Some(case.body.1)),
        "{}: printed body",
        case.id
    );
    assert_eq!(
        e.effective_has_keyword(source, tricerules_cards::primitives::Keyword::Flying),
        case.flying,
        "{}: printed flying",
        case.id
    );
    advance_to_active_player_upkeep(e, 0);
    give_pool(e, 0, gift(&case.cost));
    (source, pass_both_players_with_batch(e))
}

#[test]
fn q45_upkeep_sacrifice_canary_simplest_pays_and_keeps_the_creature() {
    let case = &upkeep_cases()[0];
    let mut e = base_engine(45_030_001);
    let (_source, batch) = upkeep_choice(&mut e, case);
    let choice = find_resolution_choice(&batch).expect("pay-or-sacrifice branch");
    assert_eq!(choice.choice_kind(), ChoiceKind::ResolutionBranch);
    e.apply_scenario_command(0, &select_branch(0))
        .expect("choose the printed payment");
    assert_eq!(
        e.state
            .pending_resolution
            .as_ref()
            .expect("mana payment offer")
            .presentation
            .choice_kind,
        ChoiceKind::ManaPayment
    );
    submit_mana_resolution_decision(&mut e, 0, ResolutionChoiceDecision::PayMana)
        .expect("pay the upkeep cost and finish the trigger");
    let source = battlefield_object_for_card(&e, 0, case.id);
    assert_eq!(e.state.objects[&source].zone, Zone::Battlefield);
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "the exact cost was paid");
}

#[test]
fn q45_upkeep_sacrifice_canary_hardest_declines_and_sacrifices() {
    let case = &upkeep_cases()[5];
    let mut e = base_engine(45_030_002);
    let (source, batch) = upkeep_choice(&mut e, case);
    assert!(find_resolution_choice(&batch).is_some());
    e.apply_scenario_command(
        0,
        &submit_resolution_decision(ResolutionChoiceDecision::Decline),
    )
    .expect("decline the payment");
    assert_eq!(
        e.state.objects[&source].zone,
        Zone::Graveyard,
        "declining sacrifices the source"
    );
}

#[test]
fn q45_upkeep_sacrifice_cards_pay_their_printed_cost_or_die() {
    for (index, case) in upkeep_cases().iter().enumerate() {
        let mut e = base_engine(45_031_000 + index as u64);
        let (source, batch) = upkeep_choice(&mut e, case);
        assert!(
            find_resolution_choice(&batch).is_some(),
            "{}: upkeep trigger",
            case.id
        );
        e.apply_scenario_command(0, &select_branch(0))
            .unwrap_or_else(|error| panic!("{}: select pay: {error:?}", case.id));
        submit_mana_resolution_decision(&mut e, 0, ResolutionChoiceDecision::PayMana)
            .unwrap_or_else(|error| panic!("{}: pay: {error:?}", case.id));
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}: paid and survived",
            case.id
        );
    }
}

#[test]
fn q45_upkeep_sacrifice_does_not_trigger_on_the_opponents_upkeep() {
    let case = &upkeep_cases()[0];
    let mut e = base_engine(45_032_001);
    inject_permanent_on_battlefield(&mut e, 0, case.id);
    advance_to_active_player_upkeep(&mut e, 1);
    let batch = pass_both_players_with_batch(&mut e);
    assert!(
        find_resolution_choice(&batch).is_none(),
        "the opponent's upkeep offers no payment choice"
    );
    assert!(
        e.state.pending_resolution.is_none(),
        "no postponed trigger is waiting on the opponent's upkeep"
    );
    let source = battlefield_object_for_card(&e, 0, case.id);
    assert_eq!(
        e.state.objects[&source].zone,
        Zone::Battlefield,
        "the printed trigger is on your upkeep only"
    );
}

// ── 04 opponent-upkeep hand-size damage ─────────────────────────────────────

struct HandCase {
    id: &'static str,
    damage: u32,
}

fn hand_cases() -> [HandCase; 3] {
    [
        HandCase {
            id: "lavaborn_muse",
            damage: 3,
        },
        HandCase {
            id: "paupers_cage",
            damage: 2,
        },
        HandCase {
            id: "hellfire_mongrel",
            damage: 2,
        },
    ]
}

fn set_hand(e: &mut GameEngine, player: usize, count: usize) {
    e.state.players[player].hand.clear();
    for _ in 0..count {
        inject_card_into_hand(e, player, "grizzly_bears");
    }
}

#[test]
fn q45_upkeep_hand_damage_canary_simplest_hits_a_two_card_hand() {
    let case = &hand_cases()[0];
    let mut e = base_engine(45_040_001);
    inject_permanent_on_battlefield(&mut e, 0, case.id);
    set_hand(&mut e, 1, 2);
    advance_to_active_player_upkeep(&mut e, 1);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[1].life,
        20 - case.damage as i32,
        "{}: printed damage",
        case.id
    );
}

#[test]
fn q45_upkeep_hand_damage_canary_hardest_respects_the_intervening_if() {
    let case = &hand_cases()[2];
    let mut e = base_engine(45_040_002);
    inject_permanent_on_battlefield(&mut e, 0, case.id);
    set_hand(&mut e, 1, 3);
    advance_to_active_player_upkeep(&mut e, 1);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[1].life, 20,
        "three cards suppress the trigger"
    );
}

#[test]
fn q45_upkeep_hand_damage_cards_hit_each_opponent_upkeep() {
    for (index, case) in hand_cases().iter().enumerate() {
        let mut e = base_engine(45_041_000 + index as u64);
        inject_permanent_on_battlefield(&mut e, 0, case.id);
        set_hand(&mut e, 1, 2);
        advance_to_active_player_upkeep(&mut e, 1);
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.players[1].life,
            20 - case.damage as i32,
            "{}: damage",
            case.id
        );
    }
}

#[test]
fn q45_upkeep_hand_damage_rechecks_hand_size_on_resolution() {
    let case = &hand_cases()[1];
    let mut e = base_engine(45_042_001);
    inject_permanent_on_battlefield(&mut e, 0, case.id);
    set_hand(&mut e, 1, 2);
    advance_to_active_player_upkeep(&mut e, 1);
    // The trigger is on the stack; the intervening "if" is rechecked at resolution.
    inject_card_into_hand(&mut e, 1, "grizzly_bears");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.players[1].life, 20,
        "the hand grew before resolution"
    );
}

// ── 05 dies into Insect tokens ──────────────────────────────────────────────

fn kill_with_murder(e: &mut GameEngine, source: u32) {
    inject_card_into_hand(e, 0, "murder");
    give_pool(
        e,
        0,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(e, 0, "murder");
    e.apply_scenario_command(0, &cast_spell(slot, target_object(source)))
        .expect("cast Murder");
    resolve_entire_stack_two_player(e);
}

#[test]
fn q45_dies_insect_tokens_canary_simplest_creates_four() {
    let mut e = base_engine(45_050_001);
    let source = inject_permanent_on_battlefield(&mut e, 0, "symbiotic_beast");
    kill_with_murder(&mut e, source);
    let tokens = battlefield_token_oids(&e, 0, "insect_g_1_1");
    assert_eq!(tokens.len(), 4);
    for token in tokens {
        assert_eq!(e.effective_power(token), Some(1));
        assert_eq!(e.effective_toughness(token), Some(1));
    }
}

#[test]
fn q45_dies_insect_tokens_canary_hardest_creates_seven() {
    let mut e = base_engine(45_050_002);
    let source = inject_permanent_on_battlefield(&mut e, 0, "symbiotic_wurm");
    kill_with_murder(&mut e, source);
    assert_eq!(battlefield_token_oids(&e, 0, "insect_g_1_1").len(), 7);
}

#[test]
fn q45_dies_insect_tokens_cards_create_their_printed_counts() {
    for (index, (id, count)) in [
        ("symbiotic_beast", 4),
        ("symbiotic_wurm", 7),
        ("symbiotic_elf", 2),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = base_engine(45_051_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, id);
        kill_with_murder(&mut e, source);
        assert_eq!(
            battlefield_token_oids(&e, 0, "insect_g_1_1").len(),
            count,
            "{id}"
        );
    }
}

// ── 06 enters with Eldrazi Spawn ────────────────────────────────────────────

#[test]
fn q45_etb_eldrazi_spawn_canary_simplest_creates_three() {
    let mut e = base_engine(45_060_001);
    cast_named(
        &mut e,
        0,
        "emrakuls_hatcher",
        ManaGift {
            r: 1,
            c: 4,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    let tokens = battlefield_token_oids(&e, 0, "eldrazi_spawn_c_0_1");
    assert_eq!(tokens.len(), 3);
    for token in tokens {
        assert_eq!(e.effective_power(token), Some(0));
        assert_eq!(e.effective_toughness(token), Some(1));
    }
}

#[test]
fn q45_etb_eldrazi_spawn_canary_hardest_creates_two() {
    let mut e = base_engine(45_060_002);
    cast_named(
        &mut e,
        0,
        "dread_drone",
        ManaGift {
            b: 1,
            c: 4,
            ..Default::default()
        },
        vec![],
    );
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        battlefield_token_oids(&e, 0, "eldrazi_spawn_c_0_1").len(),
        2
    );
}

#[test]
fn q45_etb_eldrazi_spawn_cards_create_their_printed_counts_and_mana_tokens() {
    for (index, (id, cost, count)) in [
        (
            "emrakuls_hatcher",
            ManaGift {
                r: 1,
                c: 4,
                ..Default::default()
            },
            3,
        ),
        (
            "kozileks_predator",
            ManaGift {
                g: 1,
                c: 3,
                ..Default::default()
            },
            2,
        ),
        (
            "dread_drone",
            ManaGift {
                b: 1,
                c: 4,
                ..Default::default()
            },
            2,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let mut e = base_engine(45_061_000 + index as u64);
        cast_named(&mut e, 0, id, cost, vec![]);
        resolve_entire_stack_two_player(&mut e);
        let tokens = battlefield_token_oids(&e, 0, "eldrazi_spawn_c_0_1");
        assert_eq!(tokens.len(), count, "{id}");
        let token = tokens[0];
        let before = pool(&e);
        e.apply_scenario_command(0, &activate_ability_for(&e, token, 0, vec![]))
            .unwrap_or_else(|error| panic!("{id}: spawn mana: {error:?}"));
        assert!(
            !e.state.players[0].battlefield.contains(&token),
            "{id}: the sacrificed Spawn left the battlefield"
        );
        assert_eq!(pool(&e).5, before.5 + 1, "{id}: adds {{C}}");
    }
}

// ── 07 sacrifice for a pump ─────────────────────────────────────────────────

struct PumpCase {
    id: &'static str,
    pump: (i32, i32),
}

fn pump_cases() -> [PumpCase; 3] {
    [
        PumpCase {
            id: "shield_mate",
            pump: (0, 4),
        },
        PumpCase {
            id: "seal_of_strength",
            pump: (3, 3),
        },
        PumpCase {
            id: "child_of_thorns",
            pump: (1, 1),
        },
    ]
}

#[test]
fn q45_sacrifice_pump_canary_simplest_pumps_toughness() {
    let case = &pump_cases()[0];
    let mut e = base_engine(45_070_001);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    e.apply_scenario_command(
        0,
        &activate_ability_for(&e, source, 0, target_object(target)),
    )
    .expect("activate sacrifice pump");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(2), Some(6)),
        "Shield Mate gives +0/+4"
    );
}

#[test]
fn q45_sacrifice_pump_canary_hardest_is_an_enchantment() {
    let case = &pump_cases()[1];
    let mut e = base_engine(45_070_002);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    e.apply_scenario_command(
        0,
        &activate_ability_for(&e, source, 0, target_object(target)),
    )
    .expect("activate the enchantment sacrifice");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    assert_eq!(
        (e.effective_power(target), e.effective_toughness(target)),
        (Some(5), Some(5))
    );
}

#[test]
fn q45_sacrifice_pump_cards_apply_their_printed_pump() {
    for (index, case) in pump_cases().iter().enumerate() {
        let mut e = base_engine(45_071_000 + index as u64);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let target = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        e.apply_scenario_command(
            0,
            &activate_ability_for(&e, source, 0, target_object(target)),
        )
        .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (
                e.effective_power(target).expect("power"),
                e.effective_toughness(target).expect("toughness")
            ),
            ((2 + case.pump.0) as u32, (2 + case.pump.1) as u32),
            "{}",
            case.id
        );
        assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    }
}

// ── 08 return up to N target creatures ──────────────────────────────────────

struct BounceCase {
    id: &'static str,
    max: usize,
    mana: ManaGift,
}

fn bounce_cases() -> [BounceCase; 3] {
    [
        BounceCase {
            id: "into_the_void",
            max: 2,
            mana: ManaGift {
                u: 1,
                c: 3,
                ..Default::default()
            },
        },
        BounceCase {
            id: "waterwhirl",
            max: 2,
            mana: ManaGift {
                u: 2,
                c: 4,
                ..Default::default()
            },
        },
        BounceCase {
            id: "captivating_gyre",
            max: 3,
            mana: ManaGift {
                u: 2,
                c: 4,
                ..Default::default()
            },
        },
    ]
}

fn cast_bounce(e: &mut GameEngine, case: &BounceCase, targets: Vec<TargetRef>) {
    cast_named(e, 0, case.id, gift(&case.mana), targets);
    resolve_entire_stack_two_player(e);
}

#[test]
fn q45_bounce_canary_simplest_returns_two() {
    let case = &bounce_cases()[0];
    let mut e = base_engine(45_080_001);
    let a = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let b = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    cast_bounce(&mut e, case, [target_object(a), target_object(b)].concat());
    assert_eq!(e.state.objects[&a].zone, Zone::Hand);
    assert_eq!(e.state.objects[&b].zone, Zone::Hand);
}

#[test]
fn q45_bounce_canary_hardest_returns_three() {
    let case = &bounce_cases()[2];
    let mut e = base_engine(45_080_002);
    let a = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let b = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let c = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    cast_bounce(
        &mut e,
        case,
        [target_object(a), target_object(b), target_object(c)].concat(),
    );
    for oid in [a, b, c] {
        assert_eq!(e.state.objects[&oid].zone, Zone::Hand);
    }
}

#[test]
fn q45_bounce_cards_allow_zero_targets_and_honor_their_maximum() {
    for (index, case) in bounce_cases().iter().enumerate() {
        let mut e = base_engine(45_081_000 + index as u64);
        let a = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        cast_bounce(&mut e, case, vec![]);
        assert_eq!(
            e.state.objects[&a].zone,
            Zone::Battlefield,
            "{}: zero targets leave the board alone",
            case.id
        );
    }
}

#[test]
fn q45_bounce_cards_return_every_legal_target() {
    for (index, case) in bounce_cases().iter().enumerate() {
        let mut e = base_engine(45_082_000 + index as u64);
        let targets: Vec<u32> = (0..case.max)
            .map(|_| inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2))
            .collect();
        let refs = targets
            .iter()
            .copied()
            .map(target_object)
            .collect::<Vec<_>>();
        cast_bounce(&mut e, case, refs.concat());
        for oid in targets {
            assert_eq!(e.state.objects[&oid].zone, Zone::Hand, "{}", case.id);
        }
    }
}

// ── 09 tap up to N target creatures ─────────────────────────────────────────

struct TapCase {
    id: &'static str,
    max: usize,
    mana: ManaGift,
}

fn tap_cases() -> [TapCase; 3] {
    [
        TapCase {
            id: "lead_astray",
            max: 2,
            mana: ManaGift {
                w: 1,
                c: 1,
                ..Default::default()
            },
        },
        TapCase {
            id: "downpour",
            max: 3,
            mana: ManaGift {
                u: 1,
                c: 1,
                ..Default::default()
            },
        },
        TapCase {
            id: "terashis_cry",
            max: 3,
            mana: ManaGift {
                w: 1,
                c: 3,
                ..Default::default()
            },
        },
    ]
}

fn cast_tap(e: &mut GameEngine, case: &TapCase, targets: Vec<TargetRef>) {
    cast_named(e, 0, case.id, gift(&case.mana), targets);
    resolve_entire_stack_two_player(e);
}

#[test]
fn q45_tap_up_to_canary_simplest_taps_two() {
    let case = &tap_cases()[0];
    let mut e = base_engine(45_090_001);
    let a = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let b = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    cast_tap(&mut e, case, [target_object(a), target_object(b)].concat());
    assert!(e.state.objects[&a].tapped);
    assert!(e.state.objects[&b].tapped);
}

#[test]
fn q45_tap_up_to_canary_hardest_taps_three() {
    let case = &tap_cases()[2];
    let mut e = base_engine(45_090_002);
    let a = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let b = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let c = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    cast_tap(
        &mut e,
        case,
        [target_object(a), target_object(b), target_object(c)].concat(),
    );
    for oid in [a, b, c] {
        assert!(e.state.objects[&oid].tapped);
    }
}

#[test]
fn q45_tap_up_to_cards_allow_zero_targets_and_honor_their_maximum() {
    for (index, case) in tap_cases().iter().enumerate() {
        let mut e = base_engine(45_091_000 + index as u64);
        let a = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        cast_tap(&mut e, case, vec![]);
        assert!(
            !e.state.objects[&a].tapped,
            "{}: zero targets tap nothing",
            case.id
        );
        let targets: Vec<u32> = (0..case.max)
            .map(|_| inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2))
            .collect();
        let refs = targets
            .iter()
            .copied()
            .map(target_object)
            .collect::<Vec<_>>();
        cast_tap(&mut e, case, refs.concat());
        for oid in targets {
            assert!(e.state.objects[&oid].tapped, "{}", case.id);
        }
    }
}

#[test]
fn q45_tap_up_to_cards_tap_the_remaining_legal_targets() {
    let case = &tap_cases()[1];
    let mut e = base_engine(45_092_001);
    let a = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let b = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    cast_named(
        &mut e,
        0,
        case.id,
        gift(&case.mana),
        [target_object(a), target_object(b)].concat(),
    );
    // Murder one target before Downpour resolves; the other target must still be tapped.
    inject_card_into_hand(&mut e, 1, "murder");
    give_pool(
        &mut e,
        1,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 1, "murder");
    e.apply_scenario_command(0, &pass()).expect("p0 passes");
    e.apply_scenario_command(1, &cast_spell(slot, target_object(a)))
        .expect("respond with Murder");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&a].zone, Zone::Graveyard);
    assert!(
        e.state.objects[&b].tapped,
        "the remaining legal target is tapped"
    );
}

#[test]
fn q45_sacrifice_pump_stale_target_does_not_error() {
    let case = &pump_cases()[0];
    let mut e = base_engine(45_073_001);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let target = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    e.apply_scenario_command(
        0,
        &activate_ability_for(&e, source, 0, target_object(target)),
    )
    .expect("activate sacrifice pump");
    e.apply_scenario_command(0, &pass()).expect("p0 passes");
    inject_card_into_hand(&mut e, 1, "murder");
    give_pool(
        &mut e,
        1,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 1, "murder");
    e.apply_scenario_command(1, &cast_spell(slot, target_object(target)))
        .expect("respond with Murder");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&target].zone, Zone::Graveyard);
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
    assert!(
        e.state.player_idx(1).is_some(),
        "the stale-target path resolved without error"
    );
}

#[test]
fn q45_bounce_stale_target_returns_the_remaining_creature() {
    let case = &bounce_cases()[0];
    let mut e = base_engine(45_083_001);
    let a = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    let b = inject_creature_with_stats(&mut e, 1, "grizzly_bears", 2, 2);
    cast_named(
        &mut e,
        0,
        case.id,
        gift(&case.mana),
        [target_object(a), target_object(b)].concat(),
    );
    e.apply_scenario_command(0, &pass()).expect("p0 passes");
    inject_card_into_hand(&mut e, 1, "murder");
    give_pool(
        &mut e,
        1,
        ManaGift {
            b: 2,
            c: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 1, "murder");
    e.apply_scenario_command(1, &cast_spell(slot, target_object(a)))
        .expect("respond with Murder");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&a].zone, Zone::Graveyard);
    assert_eq!(
        e.state.objects[&b].zone,
        Zone::Hand,
        "the remaining legal target still returns"
    );
}
