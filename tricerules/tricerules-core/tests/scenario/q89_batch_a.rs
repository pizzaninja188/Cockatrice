//! Actual-card scenarios for the queue-v89 batch A card data.
//!
//! Module 1 covers the three Turbulent dual lands (conditional entry against the live opponent
//! land count, then two printed basic-land-type mana options), module 2 the four once-each-turn
//! self-pumps, and module 3 the three sacrifice-another-creature counter creatures. Expectations
//! are printed Oracle values: CR 614.1d/614.12 for the entry replacement, CR 305.6/305.8 for the
//! intrinsic land mana ability, CR 602.2/602.5b for the once-each-turn limit, CR 118.3a for mana
//! payment, CR 701.21a for the sacrifice cost, and CR 122.1/122.6 for the resolved +1/+1 counter.
use super::helpers::*;
use tricerules_cards::primitives::{CounterKind, Keyword};
use tricerules_core::Zone;

// ── 01 Turbulent dual lands ───────────────────────────────────────────────

struct LandCase {
    id: &'static str,
    first: char,
    second: char,
}

const LANDS: &[LandCase] = &[
    LandCase {
        id: "turbulent_crater",
        first: 'b',
        second: 'r',
    },
    LandCase {
        id: "turbulent_shore",
        first: 'w',
        second: 'u',
    },
    LandCase {
        id: "turbulent_wetlands",
        first: 'u',
        second: 'b',
    },
];

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

fn pool(e: &GameEngine, player: usize) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[player].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
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

fn land_engine(seed: u64, card: &str) -> GameEngine {
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

/// The controller-scope tests must hand priority to the foreign player first, otherwise the
/// activation is rejected on priority before the controller check ever runs and the test would
/// pass even if any player were allowed to activate a foreign permanent.
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

/// Some extra lands for one player, deployed straight to the battlefield so the opponent's land
/// count is a live board value rather than a deck size. The basic is the one that player's deck
/// already fills with.
fn deploy_lands(e: &mut GameEngine, player: usize, basic: &str, count: usize) {
    for _ in 0..count {
        deploy_to_battlefield(e, player, basic, false);
    }
}

/// Canary: the simplest card — with fewer than eight opponent lands it enters tapped, and it
/// enters untapped once the opponent controls eight.
#[test]
fn q89_turbulent_land_canary_simplest_checks_the_live_opponent_land_count() {
    let case = LANDS
        .iter()
        .find(|case| case.id == "turbulent_shore")
        .expect("Turbulent Shore case");

    let mut tapped = land_engine(89_010_001, case.id);
    deploy_lands(&mut tapped, 1, "island", 7);
    let card = inject_card_into_hand(&mut tapped, 0, case.id);
    let slot = tapped.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == card)
        .expect("the injected card is in hand");
    tapped
        .apply_scenario_command(0, &play_land(slot))
        .expect("play the land");
    let land = battlefield_object_for_card(&tapped, 0, case.id);
    assert!(
        tapped.state.objects[&land].tapped,
        "seven opponent lands is fewer than eight, so it enters tapped"
    );

    let mut untapped = land_engine(89_010_002, case.id);
    deploy_lands(&mut untapped, 1, "island", 8);
    let card = inject_card_into_hand(&mut untapped, 0, case.id);
    let slot = untapped.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == card)
        .expect("the injected card is in hand");
    untapped
        .apply_scenario_command(0, &play_land(slot))
        .expect("play the land");
    let land = battlefield_object_for_card(&untapped, 0, case.id);
    assert!(
        !untapped.state.objects[&land].tapped,
        "eight opponent lands satisfies the condition, so it enters untapped"
    );
}

/// Second canary: the {U}{B} pair. Turbulent Moor is a different pair, so a copied mana option
/// list is visible here rather than hidden behind an identical neighbour.
#[test]
fn q89_turbulent_land_canary_ub_offers_both_printed_colors_once() {
    let case = LANDS
        .iter()
        .find(|case| case.id == "turbulent_wetlands")
        .expect("Turbulent Wetlands case");
    let mut e = land_engine(89_010_003, case.id);
    let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
    e.state.objects.get_mut(&land).expect("land").tapped = false;
    e.apply_scenario_command(0, &mana_command(&e, land, 0))
        .expect("first printed color");
    assert_eq!(pool(&e, 0), mask(case.first), "the first printed color");
    assert!(e.state.objects[&land].tapped, "the ability taps the land");
    assert!(e.state.stack.is_empty(), "the mana ability uses no stack");
    assert!(
        e.apply_scenario_command(0, &mana_command(&e, land, 1))
            .is_err(),
        "a tapped land cannot produce mana again"
    );
    assert_eq!(pool(&e, 0), mask(case.first), "no second mana");

    e.state.objects.get_mut(&land).expect("land").tapped = false;
    e.apply_scenario_command(0, &mana_command(&e, land, 1))
        .expect("second printed color");
    assert_eq!(
        (
            e.state.players[0].mana_pool.blue,
            e.state.players[0].mana_pool.black
        ),
        (1, 1),
        "one blue and one black in total"
    );
}

#[test]
fn q89_turbulent_lands_apply_their_printed_colors_and_entry_replacement() {
    for (index, case) in LANDS.iter().enumerate() {
        for (offset, (option, color)) in [(0, case.first), (1, case.second)].into_iter().enumerate()
        {
            let mut e = land_engine(89_011_000 + index as u64 * 10 + offset as u64, case.id);
            deploy_lands(&mut e, 1, "island", 8);
            let card = inject_card_into_hand(&mut e, 0, case.id);
            let slot = e.state.players[0]
                .hand
                .iter()
                .position(|oid| *oid == card)
                .expect("the injected card is in hand");
            e.apply_scenario_command(0, &play_land(slot))
                .unwrap_or_else(|error| panic!("play {}: {error}", case.id));
            let land = battlefield_object_for_card(&e, 0, case.id);
            assert!(
                !e.state.objects[&land].tapped,
                "{}: eight opponent lands means untapped",
                case.id
            );
            assert!(e.state.stack.is_empty(), "{}", case.id);
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
            assert_eq!(e.state.players[0].life, 20, "{}: no life change", case.id);
        }
    }
}

/// The condition counts the entering player's opponents, not that player: eight lands the
/// controller already owns leave the entry replacement on, and only the opponents' count matters.
#[test]
fn q89_turbulent_lands_count_only_the_opponents_lands() {
    for (index, case) in LANDS.iter().enumerate() {
        let mut own = land_engine(89_012_000 + index as u64, case.id);
        deploy_lands(&mut own, 0, "forest", 8);
        let card = inject_card_into_hand(&mut own, 0, case.id);
        let slot = own.state.players[0]
            .hand
            .iter()
            .position(|oid| *oid == card)
            .expect("the injected card is in hand");
        own.apply_scenario_command(0, &play_land(slot))
            .unwrap_or_else(|error| panic!("{}: {error}", case.id));
        let land = battlefield_object_for_card(&own, 0, case.id);
        assert!(
            own.state.objects[&land].tapped,
            "{}: eight lands the controller controls do not satisfy the opponents' count",
            case.id
        );

        let mut split = land_engine(89_013_000 + index as u64, case.id);
        deploy_lands(&mut split, 0, "forest", 6);
        deploy_lands(&mut split, 1, "island", 7);
        let card = inject_card_into_hand(&mut split, 0, case.id);
        let slot = split.state.players[0]
            .hand
            .iter()
            .position(|oid| *oid == card)
            .expect("the injected card is in hand");
        split
            .apply_scenario_command(0, &play_land(slot))
            .unwrap_or_else(|error| panic!("{}: {error}", case.id));
        let land = battlefield_object_for_card(&split, 0, case.id);
        assert!(
            split.state.objects[&land].tapped,
            "{}: only the opponents' seven lands are counted, so the conditional stays on",
            case.id
        );
    }
}

// ── 02 once-each-turn self-pumps ──────────────────────────────────────────

struct PumpCase {
    id: &'static str,
    keyword: Option<Keyword>,
    activation: &'static str,
    delta: (u32, u32),
    body: (u32, u32),
}

const PUMPS: &[PumpCase] = &[
    PumpCase {
        id: "basking_rootwalla",
        keyword: None,
        activation: "{1}{G}",
        delta: (2, 2),
        body: (1, 1),
    },
    PumpCase {
        // Card id keeps the printed hyphen (engine slugify); the face id does not.
        id: "ghor-clan_bloodscale",
        keyword: Some(Keyword::FirstStrike),
        activation: "{3}{G}",
        delta: (2, 2),
        body: (2, 1),
    },
    PumpCase {
        id: "twinblade_slasher",
        keyword: Some(Keyword::Wither),
        activation: "{1}{G}",
        delta: (2, 2),
        body: (1, 1),
    },
    PumpCase {
        id: "viashino_slaughtermaster",
        keyword: Some(Keyword::DoubleStrike),
        activation: "{B}{G}",
        delta: (1, 1),
        body: (1, 1),
    },
];

fn pump_engine(seed: u64, card: &str) -> GameEngine {
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

fn give_activation(e: &mut GameEngine, player: i32, activation: &str) {
    let cost = tricerules_cards::ManaCost::parse(activation).expect("printed activation cost");
    let (mut w, mut u, mut b, mut r, mut g, mut c) = (0, 0, 0, 0, 0, 0);
    for pip in &cost.pips {
        match pip {
            tricerules_cards::ManaSymbol::W => w += 1,
            tricerules_cards::ManaSymbol::U => u += 1,
            tricerules_cards::ManaSymbol::B => b += 1,
            tricerules_cards::ManaSymbol::R => r += 1,
            tricerules_cards::ManaSymbol::G => g += 1,
            tricerules_cards::ManaSymbol::Generic(n) => c += n,
            other => panic!("unexpected printed activation symbol {other:?}"),
        }
    }
    give_mana(e, player, ManaGift { w, u, b, r, g, c });
}

fn pool_empty(e: &GameEngine, player: usize) -> bool {
    pool(e, player) == (0, 0, 0, 0, 0, 0)
}

/// Canary: the simplest card — one activation pumps and the second activation in the same turn
/// is refused by the printed once-each-turn limit.
#[test]
fn q89_once_pump_canary_simplest_pumps_once_per_turn() {
    let case = PUMPS
        .iter()
        .find(|case| case.id == "basking_rootwalla")
        .expect("Basking Rootwalla case");
    let mut e = pump_engine(89_020_001, case.id);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(case.body.0), Some(case.body.1)),
        "printed body"
    );
    give_activation(&mut e, 0, case.activation);
    e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
        .expect("first activation");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (
            Some(case.body.0 + case.delta.0),
            Some(case.body.1 + case.delta.1)
        ),
        "the printed +2/+2 applies"
    );
    assert!(pool_empty(&e, 0), "the printed mana was spent");
    assert!(
        !e.state.objects[&source].tapped,
        "the printed ability has no {{T}} cost"
    );

    give_activation(&mut e, 0, case.activation);
    let rejected = e
        .apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
        .expect_err("the printed once-each-turn limit refuses a second activation");
    assert!(
        format!("{rejected:?}").contains("limit"),
        "expected the activation-limit rejection, got {rejected:?}"
    );
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (
            Some(case.body.0 + case.delta.0),
            Some(case.body.1 + case.delta.1)
        ),
        "the refused activation adds nothing"
    );
}

/// Hardest canary: the two-color {B}{G} activation and a double-strike body. A copy-pasted
/// monocolor cost fails here, and the limit still applies with the keyword present.
#[test]
fn q89_once_pump_canary_hardest_charges_two_colors_and_keeps_its_keyword() {
    let case = PUMPS
        .iter()
        .find(|case| case.id == "viashino_slaughtermaster")
        .expect("Viashino Slaughtermaster case");
    let mut e = pump_engine(89_020_002, case.id);
    let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
    assert!(
        e.effective_has_keyword(source, case.keyword.expect("printed double strike")),
        "the printed double strike is unaffected by the activation"
    );
    // Only green mana available: the printed {B}{G} cannot be paid.
    give_mana(
        &mut e,
        0,
        ManaGift {
            g: 1,
            ..Default::default()
        },
    );
    assert!(
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
            .is_err(),
        "the printed {{B}}{{G}} requires black and green"
    );
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(case.body.0), Some(case.body.1)),
        "a failed activation adds nothing"
    );

    // One generic plus one green pays {1}{G} but not {B}{G}: this is the pool that separates the
    // printed two-color activation from a generic-plus-green copy of it.
    give_mana(
        &mut e,
        0,
        ManaGift {
            c: 1,
            g: 1,
            ..Default::default()
        },
    );
    assert!(
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
            .is_err(),
        "the black pip is not interchangeable with generic mana"
    );
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(case.body.0), Some(case.body.1)),
        "the refused activation adds nothing"
    );
    e.state.players[0].mana_pool.clear();

    give_mana(
        &mut e,
        0,
        ManaGift {
            b: 1,
            g: 1,
            ..Default::default()
        },
    );
    e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
        .expect("the printed two-color activation");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(2), Some(2)),
        "the printed +1/+1 applies to the 1/1"
    );
    assert!(
        e.effective_has_keyword(source, Keyword::DoubleStrike),
        "double strike survives the pump"
    );
}

#[test]
fn q89_once_pump_cards_pump_once_each_turn_and_require_their_controller() {
    for (index, case) in PUMPS.iter().enumerate() {
        let mut e = pump_engine(89_021_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        give_activation(&mut e, 0, case.activation);
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
            .unwrap_or_else(|error| panic!("{}: {error}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (
                Some(case.body.0 + case.delta.0),
                Some(case.body.1 + case.delta.1)
            ),
            "{}: one activation applies the printed delta once",
            case.id
        );
        assert!(pool_empty(&e, 0), "{}: the printed mana was spent", case.id);
        assert!(
            e.state.stack.is_empty(),
            "{}: nothing is left on the stack",
            case.id
        );
        assert!(
            !e.state.objects[&source].tapped,
            "{}: the printed ability has no {{T}} cost",
            case.id
        );

        // The printed limit, not an empty pool, is what refuses the second activation.
        give_activation(&mut e, 0, case.activation);
        let rejected = e
            .apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
            .expect_err("the printed once-each-turn limit refuses a second activation");
        assert!(
            format!("{rejected:?}").contains("limit"),
            "{}: expected the activation-limit rejection, got {rejected:?}",
            case.id
        );
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (
                Some(case.body.0 + case.delta.0),
                Some(case.body.1 + case.delta.1)
            ),
            "{}: the refused activation adds nothing",
            case.id
        );
        assert!(
            !pool_empty(&e, 0),
            "{}: the refused activation does not spend the mana",
            case.id
        );

        let mut foreign = pump_engine(89_022_000 + index as u64, case.id);
        let foreign_source = inject_permanent_on_battlefield(&mut foreign, 0, case.id);
        give_priority_to(&mut foreign, 1);
        give_activation(&mut foreign, 1, case.activation);
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
            (
                foreign.effective_power(foreign_source),
                foreign.effective_toughness(foreign_source)
            ),
            (Some(case.body.0), Some(case.body.1)),
            "{}: the refused activation adds nothing",
            case.id
        );
    }
}

/// Advance to the same player's next main phase, respecting cleanup discards. Modelled on the
/// shipped `attack_pump_batch` turn-advance loop.
fn advance_to_next_turn_main1(e: &mut GameEngine, player: i32) {
    let starting_turn = e.state.turn_instance;
    let starting_player = e.state.active_player_id();
    for _ in 0..240 {
        if e.state.turn_instance > starting_turn
            && e.state.active_player_id() == player
            && e.state.turn_step == tricerules_core::TurnStep::Main1
        {
            return;
        }
        resolve_cleanup_discards_if_any(e);
        let priority = e.state.priority_player_id();
        e.apply_scenario_command(priority, &pass())
            .expect("advance toward the next turn");
    }
    panic!(
        "turn advancement stalled before player {player}'s next main phase \
         (started on turn {starting_turn} with player {starting_player})"
    );
}

/// A synthetic control change keeps the exact physical incarnation (no zone change), so the
/// per-turn activation counter must stay spent. The pinned 2006-02-01 ruling on Basking Rootwalla
/// and Ghor-Clan Bloodscale states exactly this: the new controller cannot activate it that turn.
fn set_fixture_controller(e: &mut GameEngine, object: u32, from: usize, to: usize) {
    e.state.players[from].battlefield.retain(|id| *id != object);
    e.state.players[to].battlefield.push(object);
    let player = e.state.players[to].id;
    let object = e.state.objects.get_mut(&object).expect("object");
    object.controller = player;
    object.base_controller = player;
}

/// The printed limit follows the *object*, not the player: after another player gains control the
/// ability is still spent for this turn.
#[test]
fn q89_once_pump_cards_keep_the_limit_after_a_control_change() {
    for (index, case) in PUMPS.iter().enumerate() {
        let mut e = pump_engine(89_024_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        give_activation(&mut e, 0, case.activation);
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
            .unwrap_or_else(|error| panic!("{}: first activation: {error}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (
                Some(case.body.0 + case.delta.0),
                Some(case.body.1 + case.delta.1)
            ),
            "{}: one activation applied",
            case.id
        );

        // Hand the permanent to the opponent without a zone change, then give that player
        // priority and the printed mana.
        set_fixture_controller(&mut e, source, 0, 1);
        give_priority_to(&mut e, 1);
        give_activation(&mut e, 1, case.activation);
        let rejected = e
            .apply_scenario_command(1, &activate_ability_for(&e, source, 0, vec![]))
            .expect_err("the new controller cannot activate it again this turn");
        assert!(
            format!("{rejected:?}").contains("activation limit reached"),
            "{}: expected the activation-limit rejection, got {rejected:?}",
            case.id
        );
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (
                Some(case.body.0 + case.delta.0),
                Some(case.body.1 + case.delta.1)
            ),
            "{}: the refused activation adds nothing",
            case.id
        );
    }
}

/// The limit is per turn, not per object: a fresh turn grants exactly one more activation.
#[test]
fn q89_once_pump_cards_reset_the_limit_on_a_new_turn() {
    for (index, case) in PUMPS.iter().enumerate() {
        let mut e = pump_engine(89_023_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        give_activation(&mut e, 0, case.activation);
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
            .unwrap_or_else(|error| panic!("{}: {error}", case.id));
        resolve_entire_stack_two_player(&mut e);
        give_activation(&mut e, 0, case.activation);
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
                .is_err(),
            "{}: no second activation in the same turn",
            case.id
        );

        advance_to_next_turn_main1(&mut e, 0);
        e.state.objects.get_mut(&source).expect("source").tapped = false;
        give_activation(&mut e, 0, case.activation);
        e.apply_scenario_command(0, &activate_ability_for(&e, source, 0, vec![]))
            .unwrap_or_else(|error| panic!("{}: new-turn activation: {error}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (
                Some(case.body.0 + case.delta.0),
                Some(case.body.1 + case.delta.1)
            ),
            "{}: the new turn grants a fresh activation, and last turn's pump has expired",
            case.id
        );
    }
}

// ── 03 sacrifice another creature for a counter ───────────────────────────

struct CounterCase {
    id: &'static str,
    keyword: Option<Keyword>,
    activation: &'static str,
    body: (u32, u32),
}

const COUNTERS: &[CounterCase] = &[
    CounterCase {
        id: "gobbling_ooze",
        keyword: None,
        activation: "{G}",
        body: (3, 3),
    },
    CounterCase {
        id: "hobblefiend",
        keyword: Some(Keyword::Trample),
        activation: "{1}",
        body: (2, 1),
    },
    CounterCase {
        id: "ravenous_harpy",
        keyword: Some(Keyword::Flying),
        activation: "{1}",
        body: (1, 2),
    },
];

fn counter_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[card, "grizzly_bears"]),
        deck_with("island", &[]),
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

fn give_cost(e: &mut GameEngine, player: i32, case: &CounterCase) {
    let cost = tricerules_cards::ManaCost::parse(case.activation).expect("printed cost");
    let (mut w, mut u, mut b, mut r, mut g, mut c) = (0, 0, 0, 0, 0, 0);
    for pip in &cost.pips {
        match pip {
            tricerules_cards::ManaSymbol::W => w += 1,
            tricerules_cards::ManaSymbol::U => u += 1,
            tricerules_cards::ManaSymbol::B => b += 1,
            tricerules_cards::ManaSymbol::R => r += 1,
            tricerules_cards::ManaSymbol::G => g += 1,
            tricerules_cards::ManaSymbol::Generic(n) => c += n,
            other => panic!("unexpected printed activation symbol {other:?}"),
        }
    }
    give_mana(e, player, ManaGift { w, u, b, r, g, c });
}

fn sacrifice_selection(
    cost_index: u32,
    permanent_id: u32,
) -> tricerules_proto::ruled::v1::CostSelection {
    permanent_cost_selection(cost_index, permanent_id)
}

/// Canary: the simplest card — the printed {G} is spent, another creature is sacrificed, and the
/// source gets exactly one +1/+1 counter.
#[test]
fn q89_sac_counter_canary_simplest_sacrifices_another_creature_for_one_counter() {
    let case = COUNTERS
        .iter()
        .find(|case| case.id == "gobbling_ooze")
        .expect("Gobbling Ooze case");
    let mut e = counter_engine(89_030_001, case.id);
    let source = relocate_to_battlefield(&mut e, 0, case.id, false);
    let victim = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);
    let other = inject_permanent_on_battlefield(&mut e, 1, "grizzly_bears");
    give_cost(&mut e, 0, case);
    let command =
        activate_ability_with_costs(source, 0, vec![], vec![sacrifice_selection(1, victim)]);
    e.apply_scenario_command(0, &command)
        .expect("activate the sacrifice counter");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&victim].zone,
        Zone::Graveyard,
        "the chosen other creature is the sacrifice"
    );
    assert_eq!(
        e.state.objects[&source].zone,
        Zone::Battlefield,
        "the source stays"
    );
    assert_eq!(
        e.state.objects[&other].zone,
        Zone::Battlefield,
        "the opponent's creature is untouched"
    );
    assert_eq!(
        e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        1,
        "one +1/+1 counter"
    );
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(4), Some(4)),
        "the 3/3 becomes 4/4"
    );
    assert!(pool_empty(&e, 0), "the printed {{G}} was spent");
}

/// Hardest canary: the printed {1} cost plus flying. The source cannot sacrifice itself, and the
/// printed keyword survives.
#[test]
fn q89_sac_counter_canary_hardest_keeps_its_keyword_and_cannot_sacrifice_itself() {
    let case = COUNTERS
        .iter()
        .find(|case| case.id == "ravenous_harpy")
        .expect("Ravenous Harpy case");
    let mut e = counter_engine(89_030_002, case.id);
    let source = relocate_to_battlefield(&mut e, 0, case.id, false);
    assert!(
        e.effective_has_keyword(source, case.keyword.expect("printed flying")),
        "the printed flying is unaffected by the activation"
    );
    give_cost(&mut e, 0, case);
    e.apply_scenario_command(
        0,
        &activate_ability_with_costs(source, 0, vec![], vec![sacrifice_selection(1, source)]),
    )
    .expect_err("the source is excluded from its own sacrifice cost");
    assert_eq!(
        e.state.objects[&source].zone,
        Zone::Battlefield,
        "the source is still on the battlefield"
    );
    assert_eq!(
        e.state.players[0].mana_pool.colorless, 1,
        "the failed activation refunds the printed {{1}}"
    );

    let victim = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);
    e.apply_scenario_command(
        0,
        &activate_ability_with_costs(source, 0, vec![], vec![sacrifice_selection(1, victim)]),
    )
    .expect("activate with another creature");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
        1
    );
    assert!(e.effective_has_keyword(source, Keyword::Flying));
    assert_eq!(
        (e.effective_power(source), e.effective_toughness(source)),
        (Some(2), Some(3)),
        "the 1/2 becomes 2/3"
    );
}

#[test]
fn q89_sac_counter_cards_require_mana_another_creature_and_their_controller() {
    for (index, case) in COUNTERS.iter().enumerate() {
        let mut e = counter_engine(89_031_000 + index as u64, case.id);
        let source = relocate_to_battlefield(&mut e, 0, case.id, false);
        let victim = relocate_to_battlefield(&mut e, 0, "grizzly_bears", false);

        // No mana: the activation must fail before any sacrifice happens.
        e.apply_scenario_command(
            0,
            &activate_ability_with_costs(source, 0, vec![], vec![sacrifice_selection(1, victim)]),
        )
        .expect_err("the printed mana is required");
        assert_eq!(
            e.state.objects[&victim].zone,
            Zone::Battlefield,
            "{}: a failed activation does not sacrifice",
            case.id
        );
        assert_eq!(
            e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
            0,
            "{}: a failed activation adds no counter",
            case.id
        );

        // No other creature: the sacrifice is a real cost, so the activation fails.
        give_cost(&mut e, 0, case);
        e.apply_scenario_command(
            0,
            &activate_ability_with_costs(source, 0, vec![], vec![sacrifice_selection(1, source)]),
        )
        .expect_err("the source cannot pay for itself");
        assert_eq!(
            e.state.objects[&source].zone,
            Zone::Battlefield,
            "{}: the source survives",
            case.id
        );

        let mut foreign = counter_engine(89_033_000 + index as u64, case.id);
        let foreign_source = relocate_to_battlefield(&mut foreign, 0, case.id, false);
        let foreign_victim = relocate_to_battlefield(&mut foreign, 0, "grizzly_bears", false);
        give_priority_to(&mut foreign, 1);
        give_cost(&mut foreign, 1, case);
        let rejected = foreign
            .apply_scenario_command(
                1,
                &activate_ability_with_costs(
                    foreign_source,
                    0,
                    vec![],
                    vec![sacrifice_selection(1, foreign_victim)],
                ),
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
            foreign.state.objects[&foreign_victim].zone,
            Zone::Battlefield,
            "{}: the foreign activation sacrifices nothing",
            case.id
        );
    }
}

/// Two activations stack two counters, and the sacrificed creature is the one chosen each time.
#[test]
fn q89_sac_counter_cards_stack_one_counter_per_sacrifice() {
    for (index, case) in COUNTERS.iter().enumerate() {
        let mut e = counter_engine(89_034_000 + index as u64, case.id);
        let source = relocate_to_battlefield(&mut e, 0, case.id, false);
        for activation in 0..2 {
            let victim = inject_permanent_on_battlefield(&mut e, 0, "grizzly_bears");
            give_cost(&mut e, 0, case);
            e.apply_scenario_command(
                0,
                &activate_ability_with_costs(
                    source,
                    0,
                    vec![],
                    vec![sacrifice_selection(1, victim)],
                ),
            )
            .unwrap_or_else(|error| panic!("{} activation {activation}: {error}", case.id));
            resolve_entire_stack_two_player(&mut e);
            assert_eq!(
                e.state.objects[&victim].zone,
                Zone::Graveyard,
                "{}: activation {activation} sacrificed the chosen creature",
                case.id
            );
            assert_eq!(
                e.state.objects[&source].counter_count(CounterKind::PlusOnePlusOne),
                activation as u32 + 1,
                "{}: activation {activation} added one counter",
                case.id
            );
        }
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0 + 2), Some(case.body.1 + 2)),
            "{}: two counters on the printed body",
            case.id
        );
    }
}
