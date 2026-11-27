//! Actual-card scenarios for the queue-v88 batch A card data.
//!
//! Module 1 covers the ten Planet refuge lands (enters tapped, gains one life, taps for one of
//! two printed colors), module 2 the three sacrifice-for-a-card creatures, and module 3 the
//! three activated +1/+1 counter creatures. Expectations are printed Oracle values: CR 603.6d
//! and 614.1d/614.12 for the entry replacement, CR 603.6a/119.3 for the entry trigger,
//! CR 605.1a/605.3b for the mana ability, CR 602.2/118.3a/701.21a for the sacrifice cost, and
//! CR 122.1/122.6 for the resolved +1/+1 counter.
use super::helpers::*;
use tricerules_cards::primitives::{CounterKind, Keyword};
use tricerules_core::Zone;

// ── 01 Planet refuges ─────────────────────────────────────────────────────

struct RefugeCase {
    id: &'static str,
    first: char,
    second: char,
}

const REFUGES: &[RefugeCase] = &[
    RefugeCase {
        id: "andoria",
        first: 'u',
        second: 'r',
    },
    RefugeCase {
        id: "bajor",
        first: 'r',
        second: 'w',
    },
    RefugeCase {
        id: "cardassia_prime",
        first: 'w',
        second: 'b',
    },
    RefugeCase {
        id: "earth,_sector_001",
        first: 'g',
        second: 'u',
    },
    RefugeCase {
        id: "ferenginar",
        first: 'b',
        second: 'g',
    },
    RefugeCase {
        id: "orion",
        first: 'r',
        second: 'g',
    },
    RefugeCase {
        id: "qonos",
        first: 'b',
        second: 'r',
    },
    RefugeCase {
        id: "risa",
        first: 'g',
        second: 'w',
    },
    RefugeCase {
        id: "romulus",
        first: 'u',
        second: 'b',
    },
    RefugeCase {
        id: "vulcan",
        first: 'w',
        second: 'u',
    },
];

fn refuge_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("island", &[])]);
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

fn mana_command(
    e: &GameEngine,
    source: u32,
    option: u32,
) -> tricerules_proto::ruled::v1::RuledCommand {
    let mut command = activate_ability_for(e, source, 0, vec![]);
    let Some(tricerules_proto::ruled::v1::ruled_command::Cmd::ActivateAbility(activation)) =
        command.cmd.as_mut()
    else {
        unreachable!("activate_ability_for builds an ActivateAbility command")
    };
    activation.mana_option_index = option;
    command
}

fn pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

/// The controller-scope tests must hand priority to the foreign player first, otherwise the
/// activation is rejected on priority before the controller check ever runs and the test would
/// pass even if any player were allowed to activate.
fn give_priority_to(e: &mut GameEngine, target: i32) {
    for _ in 0..6 {
        if e.state.priority_player_id() == target {
            return;
        }
        let actor = e.state.priority_player_id();
        e.apply_scenario_command(actor, &pass())
            .expect("pass priority toward the foreign player");
    }
    panic!("could not give priority to player {target}");
}

fn mask(color: char) -> (u32, u32, u32, u32, u32, u32) {
    match color {
        'w' => (1, 0, 0, 0, 0, 0),
        'u' => (0, 1, 0, 0, 0, 0),
        'b' => (0, 0, 1, 0, 0, 0),
        'r' => (0, 0, 0, 1, 0, 0),
        'g' => (0, 0, 0, 0, 1, 0),
        _ => unreachable!("printed color"),
    }
}

/// Canary: the simplest card — a single-color-pair refuge proves entry, trigger and mana.
#[test]
fn q88_planet_refuge_canary_simplest_enters_tapped_gains_life_and_taps() {
    let case = REFUGES
        .iter()
        .find(|case| case.id == "andoria")
        .expect("Andoria case");
    let mut e = refuge_engine(88_010_001, case.id);
    let card = inject_card_into_hand(&mut e, 0, case.id);
    let slot = e.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == card)
        .expect("the injected card is in hand");
    e.apply_scenario_command(0, &play_land(slot))
        .expect("play the refuge");
    let land = battlefield_object_for_card(&e, 0, case.id);
    assert_eq!(land, card, "the played object");
    assert!(e.state.objects[&land].tapped, "it enters tapped");
    assert_eq!(
        e.state.players[0].life, 20,
        "no life before the entry trigger resolves"
    );
    pass_both_players(&mut e);
    assert_eq!(
        e.state.players[0].life, 21,
        "the entry trigger gains exactly one life"
    );
    assert_eq!(e.state.players[1].life, 20, "the opponent does not gain");

    e.state.objects.get_mut(&land).expect("land").tapped = false;
    e.apply_scenario_command(0, &mana_command(&e, land, 0))
        .expect("tap for the first printed color");
    assert_eq!(
        pool(&e, 0),
        mask(case.first),
        "the first printed color is produced"
    );
    assert!(e.state.objects[&land].tapped, "the ability taps the land");
    assert!(e.state.stack.is_empty(), "the mana ability uses no stack");
}

/// Second canary: the white/blue pair reuses the exact Sejiri Refuge color pair, so a stored
/// Planet subtype must not perturb mana production, the tap restriction or controller scope.
#[test]
fn q88_planet_refuge_canary_wu_pair_offers_both_printed_colors_and_rejects_a_second_activation() {
    let case = REFUGES
        .iter()
        .find(|case| case.id == "vulcan")
        .expect("Vulcan case");
    let mut e = refuge_engine(88_010_002, case.id);
    let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
    e.apply_scenario_command(0, &mana_command(&e, land, 0))
        .expect("first printed color");
    assert_eq!(pool(&e, 0), mask(case.first), "the first printed color");
    assert!(
        e.apply_scenario_command(0, &mana_command(&e, land, 1))
            .is_err(),
        "a tapped land cannot produce mana again"
    );
    assert_eq!(pool(&e, 0), mask(case.first), "no second mana");

    let mut foreign = refuge_engine(88_010_003, case.id);
    let foreign_land = inject_permanent_on_battlefield(&mut foreign, 0, case.id);
    give_priority_to(&mut foreign, 1);
    let rejected = foreign
        .apply_scenario_command(1, &mana_command(&foreign, foreign_land, 0))
        .expect_err("another player cannot activate it");
    assert!(
        matches!(
            rejected,
            tricerules_core::EngineError::Illegal(message)
                if message == "ability source is not in its authored zone"
        ),
        "expected the controller/zone rejection, got {rejected:?}"
    );
    // The priority handoff must have happened, otherwise this asserts an earlier refusal and
    // would pass even if any player were allowed to activate a foreign permanent.
    assert_eq!(
        foreign.state.priority_player_id(),
        1,
        "the foreign player must hold priority when the activation is attempted"
    );
    assert_eq!(pool(&foreign, 1), (0, 0, 0, 0, 0, 0));
    assert!(!foreign.state.objects[&foreign_land].tapped);
}

#[test]
fn q88_planet_refuges_apply_their_printed_colors_and_entry_replacement() {
    for (index, case) in REFUGES.iter().enumerate() {
        for (offset, (option, color)) in [(0, case.first), (1, case.second)].into_iter().enumerate()
        {
            let mut e = refuge_engine(88_011_000 + index as u64 * 10 + offset as u64, case.id);
            let card = inject_card_into_hand(&mut e, 0, case.id);
            let slot = e.state.players[0]
                .hand
                .iter()
                .position(|oid| *oid == card)
                .expect("the injected card is in hand");
            e.apply_scenario_command(0, &play_land(slot))
                .unwrap_or_else(|error| panic!("play {}: {error}", case.id));
            let land = battlefield_object_for_card(&e, 0, case.id);
            assert!(e.state.objects[&land].tapped, "{}: enters tapped", case.id);
            assert_eq!(e.state.players[0].life, 20, "{}", case.id);
            pass_both_players(&mut e);
            assert_eq!(
                e.state.players[0].life, 21,
                "{}: entry gains exactly one life",
                case.id
            );
            assert_eq!(e.state.players[1].life, 20, "{}", case.id);

            e.state.objects.get_mut(&land).expect("land").tapped = false;
            e.apply_scenario_command(0, &mana_command(&e, land, option))
                .unwrap_or_else(|error| panic!("{} option {option}: {error}", case.id));
            assert_eq!(
                pool(&e, 0),
                mask(color),
                "{} option {option}: exact printed color",
                case.id
            );
            assert!(
                e.state.objects[&land].tapped,
                "{}: the ability taps the land",
                case.id
            );
            assert!(
                e.state.stack.is_empty(),
                "{}: the mana ability uses no stack",
                case.id
            );
            assert_eq!(e.state.players[0].life, 21, "{}", case.id);
        }
    }
}

#[test]
fn q88_planet_refuges_do_not_gain_life_when_they_do_not_enter() {
    for (index, case) in REFUGES.iter().enumerate() {
        let mut e = refuge_engine(88_012_000 + index as u64, case.id);
        inject_graveyard_card(&mut e, 0, case.id);
        pass_both_players(&mut e);
        assert_eq!(
            e.state.players[0].life, 20,
            "{}: no entry, no life",
            case.id
        );
        assert_eq!(
            e.state.players[1].life, 20,
            "{}: opponent unchanged",
            case.id
        );
    }
}

// ── 02 sacrifice for a card ───────────────────────────────────────────────

struct SacCase {
    id: &'static str,
    keyword: Option<Keyword>,
}

const SAC_CARDS: &[SacCase] = &[
    SacCase {
        id: "brass_secretary",
        keyword: None,
    },
    SacCase {
        id: "parcel_myr",
        keyword: None,
    },
    SacCase {
        id: "slinking_skirge",
        keyword: Some(Keyword::Flying),
    },
];

fn sac_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("island", &[card]), deck_with("forest", &[])]);
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

fn give_two_colorless(e: &mut GameEngine, player: i32) {
    give_mana(
        e,
        player,
        ManaGift {
            c: 2,
            ..Default::default()
        },
    );
}

/// Canary: the simplest card — sacrifice the source, draw exactly one card, pay exactly {2}.
#[test]
fn q88_sac_draw_canary_simplest_sacrifices_the_source_and_draws_one() {
    let case = SAC_CARDS
        .iter()
        .find(|case| case.id == "brass_secretary")
        .expect("Brass Secretary case");
    let mut e = sac_engine(88_020_001, case.id);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    let library_before = e.state.players[0].library.len();
    let hand_before = e.state.players[0].hand.len();
    give_two_colorless(&mut e, 0);
    e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
        .expect("activate the sacrifice draw");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&source].zone,
        Zone::Graveyard,
        "the source is the sacrificed permanent"
    );
    assert_eq!(
        e.state.players[0].hand.len(),
        hand_before + 1,
        "exactly one card is drawn"
    );
    assert_eq!(
        e.state.players[0].library.len(),
        library_before - 1,
        "the draw comes off the library"
    );
    let mana = &e.state.players[0].mana_pool;
    assert_eq!(
        (
            mana.white,
            mana.blue,
            mana.black,
            mana.red,
            mana.green,
            mana.colorless
        ),
        (0, 0, 0, 0, 0, 0),
        "the printed {{2}} was spent"
    );
}

/// Canary: the hardest card — the printed keyword survives and the sacrifice is still mandatory.
#[test]
fn q88_sac_draw_canary_hardest_keeps_its_keyword_and_sacrifices_itself() {
    let case = SAC_CARDS
        .iter()
        .find(|case| case.id == "slinking_skirge")
        .expect("Slinking Skirge case");
    let mut e = sac_engine(88_020_002, case.id);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    assert!(
        e.effective_has_keyword(source, case.keyword.expect("printed flying")),
        "the printed flying is unaffected by the activation"
    );
    give_two_colorless(&mut e, 0);
    e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
        .expect("activate the sacrifice draw");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.state.objects[&source].zone, Zone::Graveyard);
}

#[test]
fn q88_sac_draw_cards_require_the_printed_mana_and_their_controller() {
    for (index, case) in SAC_CARDS.iter().enumerate() {
        let mut e = sac_engine(88_021_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let hand_before = e.state.players[0].hand.len();
        give_mana(
            &mut e,
            0,
            ManaGift {
                c: 1,
                ..Default::default()
            },
        );
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                .is_err(),
            "{}: the printed {{2}} is required",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}: a failed activation does not sacrifice",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before,
            "{}: a failed activation does not draw",
            case.id
        );

        let mut foreign = sac_engine(88_022_000 + index as u64, case.id);
        let foreign_source = inject_permanent_on_battlefield(&mut foreign, 0, case.id);
        give_priority_to(&mut foreign, 1);
        give_two_colorless(&mut foreign, 1);
        let rejected = foreign
            .apply_scenario_command(
                1,
                &activate_ability_for(&foreign, foreign_source, 0, vec![]),
            )
            .expect_err("only the controller may activate it");
        assert!(
            matches!(
                rejected,
                tricerules_core::EngineError::Illegal(message)
                    if message == "ability source is not in its authored zone"
            ),
            "{}: expected the controller/zone rejection, got {rejected:?}",
            case.id
        );
        assert_eq!(
            foreign.state.priority_player_id(),
            1,
            "{}: the foreign player must hold priority when the activation is attempted",
            case.id
        );
        assert_eq!(
            foreign.state.objects[&foreign_source].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
    }
}

#[test]
fn q88_sac_draw_cards_draw_one_and_leave_the_graveyard_alone() {
    for (index, case) in SAC_CARDS.iter().enumerate() {
        let mut e = sac_engine(88_023_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let other = inject_graveyard_card(&mut e, 0, "grizzly_bears");
        let hand_before = e.state.players[0].hand.len();
        let library_before = e.state.players[0].library.len();
        give_two_colorless(&mut e, 0);
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
            .unwrap_or_else(|error| panic!("{}: {error}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Graveyard,
            "{}: the sacrificed source",
            case.id
        );
        assert_eq!(
            e.state.objects[&other].zone,
            Zone::Graveyard,
            "{}: the unrelated graveyard card is untouched",
            case.id
        );
        assert_eq!(
            e.state.players[0].graveyard.len(),
            2,
            "{}: exactly the source was added",
            case.id
        );
        assert_eq!(
            e.state.players[0].hand.len(),
            hand_before + 1,
            "{}: exactly one card is drawn",
            case.id
        );
        assert_eq!(
            e.state.players[0].library.len(),
            library_before - 1,
            "{}: the draw comes off the library",
            case.id
        );
    }
}

// ── 03 activated +1/+1 counters ───────────────────────────────────────────

struct CounterCase {
    id: &'static str,
    keyword: Option<Keyword>,
    /// Generic half of the printed activation cost.
    generic: u32,
    /// Colored half: exactly one of these is one, matching the printed pip.
    white: u32,
    green: u32,
}

const COUNTER_CARDS: &[CounterCase] = &[
    CounterCase {
        id: "jungle_delver",
        keyword: None,
        generic: 3,
        white: 0,
        green: 1,
    },
    CounterCase {
        id: "unholy_officiant",
        keyword: Some(Keyword::Vigilance),
        generic: 4,
        white: 1,
        green: 0,
    },
    CounterCase {
        id: "verdant_automaton",
        keyword: None,
        generic: 3,
        white: 0,
        green: 1,
    },
];

fn counter_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("island", &[])]);
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

fn give_cost(e: &mut GameEngine, player: i32, case: &CounterCase) {
    give_mana(
        e,
        player,
        ManaGift {
            w: case.white,
            g: case.green,
            c: case.generic,
            ..Default::default()
        },
    );
}

/// Canary: the simplest card — one activation, one counter, printed body plus one.
#[test]
fn q88_counter_canary_simplest_adds_one_counter() {
    let case = COUNTER_CARDS
        .iter()
        .find(|case| case.id == "jungle_delver")
        .expect("Jungle Delver case");
    let mut e = counter_engine(88_030_001, case.id);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    assert_eq!(
        e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        0,
        "no counter before activation"
    );
    give_cost(&mut e, 0, case);
    e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
        .expect("activate the counter ability");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "one +1/+1 counter"
    );
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(2), Some(2)),
        "the 1/1 becomes 2/2"
    );
    assert!(
        !e.state.objects[&source].tapped,
        "the printed ability has no {{T}} cost"
    );
    let mana = &e.state.players[0].mana_pool;
    assert_eq!(
        (
            mana.white,
            mana.blue,
            mana.black,
            mana.red,
            mana.green,
            mana.colorless
        ),
        (0, 0, 0, 0, 0, 0),
        "the printed {{3}}{{G}} was spent"
    );
}

/// Canary: the hardest card — the largest printed cost plus a printed keyword and no green mana
/// in the cost, so a wrong cost mapping would be rejected rather than silently accepted.
#[test]
fn q88_counter_canary_hardest_charges_its_exact_cost_and_keeps_its_keyword() {
    let case = COUNTER_CARDS
        .iter()
        .find(|case| case.id == "unholy_officiant")
        .expect("Unholy Officiant case");
    let mut e = counter_engine(88_030_002, case.id);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    assert!(
        e.effective_has_keyword(source, case.keyword.expect("printed vigilance")),
        "the printed vigilance is unaffected"
    );
    give_cost(&mut e, 0, case);
    e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
        .expect("activate the counter ability");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(2), Some(3)),
        "the 1/2 becomes 2/3"
    );
    assert!(
        !e.state.objects[&source].tapped,
        "the printed ability has no {{T}} cost"
    );
    let mana = &e.state.players[0].mana_pool;
    assert_eq!(
        (
            mana.white,
            mana.blue,
            mana.black,
            mana.red,
            mana.green,
            mana.colorless
        ),
        (0, 0, 0, 0, 0, 0),
        "the printed {{4}}{{W}} was spent"
    );
}

#[test]
fn q88_counter_cards_add_one_counter_each_activation_and_survive_a_failed_one() {
    for (index, case) in COUNTER_CARDS.iter().enumerate() {
        let mut e = counter_engine(88_031_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        give_mana(
            &mut e,
            0,
            ManaGift {
                w: case.white,
                g: case.green,
                c: case.generic.saturating_sub(1),
                ..Default::default()
            },
        );
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                .is_err(),
            "{}: the printed cost is required",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            0,
            "{}: a failed activation places no counter",
            case.id
        );

        give_cost(&mut e, 0, case);
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
            .unwrap_or_else(|error| panic!("{}: {error}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            1,
            "{}: the first activation",
            case.id
        );

        give_cost(&mut e, 0, case);
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
            .unwrap_or_else(|error| panic!("{}: {error}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            2,
            "{}: the ability is repeatable",
            case.id
        );
    }
}

#[test]
fn q88_counter_cards_require_their_controller() {
    for (index, case) in COUNTER_CARDS.iter().enumerate() {
        let mut e = counter_engine(88_032_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        give_priority_to(&mut e, 1);
        give_cost(&mut e, 1, case);
        let rejected = e
            .apply_scenario_command(1, &activate_ability_for(&e, source, 0, vec![]))
            .expect_err("an opponent cannot activate it");
        assert!(
            matches!(
                rejected,
                tricerules_core::EngineError::Illegal(message)
                    if message == "ability source is not in its authored zone"
            ),
            "{}: expected the controller/zone rejection, got {rejected:?}",
            case.id
        );
        assert_eq!(
            e.state.priority_player_id(),
            1,
            "{}: the foreign player must hold priority when the activation is attempted",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            0,
            "{}",
            case.id
        );
    }
}
