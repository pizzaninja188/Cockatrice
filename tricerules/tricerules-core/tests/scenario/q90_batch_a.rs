//! Actual-card scenarios for the queue-v90 batch A card data.
//!
//! Module 1 covers the ten "enters tapped unless you control a planeswalker" dual lands, module 2
//! the three "{T}, Tap an untapped creature you control: Add one mana of any color" creatures and
//! module 3 the four keyword-plus-"{cost}, {T}: Tap target creature" creatures. Expectations are
//! printed Oracle values: CR 614.1d/614.12 for the entry replacement, CR 305.6 for the land mana
//! ability, CR 602.1a/602.2 and 701.26a for the activation and tap payments, CR 605.1a for the mana
//! ability, CR 118.3 for mana payment, CR 608.2b for a stale target and CR 701.26 for the tap.
use super::helpers::*;
use tricerules_cards::primitives::Keyword;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::{
    cost_selection::Selection, ruled_command::Cmd, CostObjectRef, CostObjectRefs, CostSelection,
    RuledCommand,
};

// ── 01 Planeswalker-conditional dual lands ───────────────────────────────

struct LandCase {
    id: &'static str,
    first: char,
    second: char,
}

const LANDS: &[LandCase] = &[
    LandCase {
        id: "theorix_annex",
        first: 'u',
        second: 'b',
    },
    LandCase {
        id: "stingerquill_annex",
        first: 'b',
        second: 'r',
    },
    LandCase {
        id: "konstrari_annex",
        first: 'r',
        second: 'g',
    },
    LandCase {
        id: "formidable_commons",
        first: 'b',
        second: 'g',
    },
    LandCase {
        id: "innovative_commons",
        first: 'u',
        second: 'r',
    },
    LandCase {
        id: "meticulous_commons",
        first: 'w',
        second: 'b',
    },
    LandCase {
        id: "transformative_commons",
        first: 'g',
        second: 'u',
    },
    LandCase {
        id: "fatehold_annex",
        first: 'w',
        second: 'u',
    },
    LandCase {
        id: "dedicated_commons",
        first: 'r',
        second: 'w',
    },
    LandCase {
        id: "vigorbloom_annex",
        first: 'g',
        second: 'w',
    },
];

/// A planeswalker to satisfy (or not satisfy) the entry condition. Ajani Steadfast is a
/// normal-layout, fully implemented planeswalker, so deploying it needs no special fixture.
const PLANESWALKER: &str = "ajani_steadfast";

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

/// The `TapPermanents` cost is a selection, not a target: it needs one explicit battlefield
/// object reference so the payment planner can debit it.
fn tap_selection(engine: &GameEngine, cost_index: u32, objects: &[u32]) -> CostSelection {
    CostSelection {
        cost_index,
        selection: Some(Selection::BattlefieldObjects(CostObjectRefs {
            objects: objects
                .iter()
                .map(|&object_id| CostObjectRef {
                    object_id,
                    zone_change_generation: engine
                        .state
                        .zone_change_generation
                        .get(&object_id)
                        .copied()
                        .unwrap_or(0),
                })
                .collect(),
        })),
    }
}

/// Activate a `TapPermanents` cost and choose the printed colour in one command.
fn activate_with_tap(
    engine: &GameEngine,
    source: u32,
    ability_index: u32,
    tap_payer: u32,
    option: u32,
) -> RuledCommand {
    let mut command = activate_ability_with_costs(
        source,
        ability_index,
        vec![],
        vec![tap_selection(engine, 1, &[tap_payer])],
    );
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!("activate_ability_with_costs builds an ActivateAbility command")
    };
    activation.ability_index = ability_index;
    activation.expected_zone_change_generation = engine
        .state
        .zone_change_generation
        .get(&source)
        .copied()
        .unwrap_or(0);
    activation.mana_option_index = option;
    command
}

fn land_engine(seed: u64) -> GameEngine {
    // The tested land is injected into hand by `play_this_land`, so the deck must not also
    // pre-place a copy: `inject_card_into_hand` is the only instance under test.
    let decks = Some(vec![
        deck_with("forest", &[]),
        deck_with("island", &[PLANESWALKER]),
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

/// Play this land from hand and return its battlefield object. The hand slot is the raw hand
/// index the `PlayLand` command expects, exactly as the shipped q89 land scenarios do it.
fn play_this_land(e: &mut GameEngine, card: &str) -> u32 {
    let card_object = inject_card_into_hand(e, 0, card);
    let slot = e.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == card_object)
        .expect("the injected card is in hand");
    e.apply_scenario_command(0, &play_land(slot))
        .unwrap_or_else(|error| panic!("play {card}: {error}"));
    card_object
}

/// Canary: the simplest card — with no planeswalker it enters tapped, and with one it enters
/// untapped. The condition reads the controller's own planeswalkers, not the opponent's.
#[test]
fn q90_planeswalker_land_canary_checks_the_controllers_planeswalker() {
    let mut tapped = land_engine(90_010_001);
    let land = play_this_land(&mut tapped, "theorix_annex");
    assert!(
        tapped.state.objects[&land].tapped,
        "no planeswalker: the land enters tapped"
    );

    let mut untapped = land_engine(90_010_002);
    inject_permanent_on_battlefield(&mut untapped, 0, PLANESWALKER);
    let land = play_this_land(&mut untapped, "theorix_annex");
    assert!(
        !untapped.state.objects[&land].tapped,
        "a planeswalker you control: the land enters untapped"
    );
}

/// The condition is controller-scoped: an opponent's planeswalker does not exempt the land.
#[test]
fn q90_planeswalker_land_ignores_an_opponents_planeswalker() {
    let mut e = land_engine(90_010_003);
    inject_permanent_on_battlefield(&mut e, 1, PLANESWALKER);
    let land = play_this_land(&mut e, "theorix_annex");
    assert!(
        e.state.objects[&land].tapped,
        "the opponent's planeswalker does not satisfy 'you control'"
    );
}

/// The replacement runs once, at entry (CR 614.12): a planeswalker that arrives afterwards does not
/// retroactively untap the land. That live-at-entry half is asserted by the canary and by this test;
/// the `q90_planeswalker_lands_*` tests exercise the mana ability only and say nothing about the
/// condition. The same-batch half is crate-private to the entry-replacement layer and is covered by
/// the shipped entry-layer scenarios named below.
///
/// The literal "enters at the same time" case is asserted by the shipped entry-replacement scenarios
/// `conditional_slow_land_ignores_lands_entering_in_the_same_batch` and
/// `conditional_fast_land_ignores_lands_entering_in_the_same_batch`
/// (`tricerules-core/src/engine/replacement/entry_layers_tests.rs`). Their filters and bounds differ
/// (`[Land]` with `min: Some(3)` / `max: Some(1)`), but they drive the same
/// `BattlefieldAggregate` leaf and the same exclusion mechanism through the internal
/// simultaneous-entry seam, which is not reachable from a scenario test.
#[test]
fn q90_planeswalker_land_checks_its_condition_only_at_entry() {
    let mut e = land_engine(90_010_004);
    let first = play_this_land(&mut e, "theorix_annex");
    assert!(
        e.state.objects[&first].tapped,
        "no planeswalker yet: the first land enters tapped"
    );

    inject_permanent_on_battlefield(&mut e, 0, PLANESWALKER);
    assert!(
        e.state.objects[&first].tapped,
        "the later planeswalker does not retroactively untap the land"
    );

    // A later *PlayLand* in the same turn is refused by the engine's one-land-per-turn rule, so
    // the "reevaluated per entry" half is not observable from this seam. It is asserted by the
    // shipped entry-replacement scenarios named above, which drive the same BattlefieldAggregate
    // leaf (with a different filter and bounds) through the internal simultaneous-entry batch.
}

#[test]
fn q90_planeswalker_lands_apply_their_printed_colors() {
    for (index, case) in LANDS.iter().enumerate() {
        for (offset, (option, color)) in [(0, case.first), (1, case.second)].into_iter().enumerate()
        {
            let mut e = land_engine(90_011_000 + index as u64 * 10 + offset as u64);
            let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
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
            assert_eq!(e.state.players[1].life, 20, "{}: no life change", case.id);
        }
    }
}

/// A tapped land cannot produce again, and the land is not a mana ability of the wrong colour.
#[test]
fn q90_planeswalker_lands_refuse_a_second_activation_while_tapped() {
    for (index, case) in LANDS.iter().enumerate() {
        let mut e = land_engine(90_013_000 + index as u64);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &mana_command(&e, land, 0))
            .unwrap_or_else(|error| panic!("{}: {error}", case.id));
        assert!(e.state.objects[&land].tapped, "{}", case.id);
        assert!(
            e.apply_scenario_command(0, &mana_command(&e, land, 1))
                .is_err(),
            "{}: a tapped land cannot produce mana again",
            case.id
        );
        assert_eq!(
            pool(&e, 0),
            mask(case.first),
            "{}: the refused activation adds no second mana",
            case.id
        );
    }
}

// ── 02 Any-color mana from tapping a second creature ─────────────────────

struct ManaCase {
    id: &'static str,
    keyword: Option<Keyword>,
    body: (u32, u32),
}

const MANA_CREATURES: &[ManaCase] = &[
    ManaCase {
        id: "loam_dryad",
        keyword: None,
        body: (1, 2),
    },
    ManaCase {
        id: "saruli_caretaker",
        keyword: Some(Keyword::Defender),
        body: (0, 3),
    },
    ManaCase {
        id: "jaspera_sentinel",
        keyword: Some(Keyword::Reach),
        body: (1, 2),
    },
];

fn mana_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("forest", &[card]), deck_with("forest", &[])]);
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

/// Canary: the simplest card — a second untapped creature pays the tap half and the source then
/// adds one mana of any chosen colour.
#[test]
fn q90_any_color_canary_taps_a_second_creature_for_one_chosen_color() {
    let mut e = mana_engine(90_020_001, "loam_dryad");
    let source = inject_permanent_on_battlefield(&mut e, 0, "loam_dryad");
    e.state
        .objects
        .get_mut(&source)
        .expect("source")
        .summoning_sick = false;
    let helper = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    assert!(
        !e.state.objects[&helper].tapped,
        "the helper starts untapped"
    );

    e.apply_scenario_command(0, &activate_with_tap(&e, source, 0, helper, 3))
        .expect("the printed ability");
    assert!(e.state.objects[&source].tapped, "the source taps itself");
    assert!(
        e.state.objects[&helper].tapped,
        "the second creature is tapped as the printed cost"
    );
    assert_eq!(pool(&e, 0), mask('r'), "the fourth printed option is red");
    assert!(e.state.stack.is_empty(), "a mana ability uses no stack");
}

/// The cost needs an untapped creature other than the source: with none available the activation
/// is refused and nothing is tapped or produced.
#[test]
fn q90_any_color_requires_a_second_untapped_creature() {
    for (index, case) in MANA_CREATURES.iter().enumerate() {
        let mut e = mana_engine(90_021_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        e.state
            .objects
            .get_mut(&source)
            .expect("source")
            .summoning_sick = false;
        // Only the source itself is on the battlefield, so there is no second creature to select.
        // The activation is refused; `AbilityCost::Tap` debiting the source is what makes the
        // self-payment illegal, not this authored flag alone.
        assert!(
            e.apply_scenario_command(0, &activate_with_tap(&e, source, 0, source, 0))
                .is_err(),
            "{}: the source cannot pay its own tap cost",
            case.id
        );
        assert!(
            !e.state.objects[&source].tapped,
            "{}: nothing tapped",
            case.id
        );
        assert_eq!(
            pool(&e, 0),
            (0, 0, 0, 0, 0, 0),
            "{}: nothing produced",
            case.id
        );
    }
}

/// The cost is exactly one creature *you* control: an opponent's creature cannot pay it, and the
/// printed `ExactCount(1)` is one, not "up to one" and not two.
#[test]
fn q90_any_color_requires_exactly_one_own_creature() {
    for (index, case) in MANA_CREATURES.iter().enumerate() {
        // An opponent's creature does not match "you control".
        let mut foreign = mana_engine(90_024_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut foreign, 0, case.id);
        foreign
            .state
            .objects
            .get_mut(&source)
            .expect("source")
            .summoning_sick = false;
        let theirs = inject_creature_on_battlefield(&mut foreign, 1, "grizzly_bears");
        assert!(
            foreign
                .apply_scenario_command(0, &activate_with_tap(&foreign, source, 0, theirs, 0))
                .is_err(),
            "{}: an opponent's creature cannot pay 'you control'",
            case.id
        );
        assert!(!foreign.state.objects[&source].tapped, "{}", case.id);
        assert_eq!(
            pool(&foreign, 0),
            (0, 0, 0, 0, 0, 0),
            "{}: nothing produced",
            case.id
        );

        // Two creatures offered to an ExactCount(1) cost are refused: the printed cost is one.
        let mut two = mana_engine(90_025_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut two, 0, case.id);
        two.state
            .objects
            .get_mut(&source)
            .expect("source")
            .summoning_sick = false;
        let first = inject_creature_on_battlefield(&mut two, 0, "grizzly_bears");
        let second = inject_creature_on_battlefield(&mut two, 0, "grizzly_bears");
        let selection = tap_selection(&two, 1, &[first, second]);
        let mut command = activate_ability_with_costs(source, 0, vec![], vec![selection]);
        if let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() {
            activation.expected_zone_change_generation = two
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0);
            activation.mana_option_index = 0;
        }
        assert!(
            two.apply_scenario_command(0, &command).is_err(),
            "{}: the cost is exactly one creature, not two",
            case.id
        );
        assert!(!two.state.objects[&source].tapped, "{}", case.id);
        assert!(!two.state.objects[&first].tapped, "{}", case.id);
        assert!(!two.state.objects[&second].tapped, "{}", case.id);
    }
}

/// A tapped helper is not a legal payment, and the source itself must be untapped to tap.
#[test]
fn q90_any_color_refuses_a_tapped_helper_or_a_tapped_source() {
    for (index, case) in MANA_CREATURES.iter().enumerate() {
        let mut e = mana_engine(90_022_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        e.state
            .objects
            .get_mut(&source)
            .expect("source")
            .summoning_sick = false;
        let helper = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        e.state.objects.get_mut(&helper).expect("helper").tapped = true;
        assert!(
            e.apply_scenario_command(0, &activate_with_tap(&e, source, 0, helper, 0))
                .is_err(),
            "{}: a tapped creature cannot pay the tap cost",
            case.id
        );
        assert!(
            !e.state.objects[&source].tapped,
            "{}: nothing tapped",
            case.id
        );

        e.state.objects.get_mut(&helper).expect("helper").tapped = false;
        e.state.objects.get_mut(&source).expect("source").tapped = true;
        assert!(
            e.apply_scenario_command(0, &activate_with_tap(&e, source, 0, helper, 0))
                .is_err(),
            "{}: a tapped source cannot be activated",
            case.id
        );
        assert!(
            !e.state.objects[&helper].tapped,
            "{}: the helper is not tapped",
            case.id
        );
    }
}

/// The pinned ruling's asymmetry: the *payer* may be summoning-sick (it is a cost selection, not a
/// `{T}` activation), while the *source* may not be. Both halves are asserted here.
#[test]
fn q90_any_color_checks_source_and_payer_summoning_sickness() {
    for (index, case) in MANA_CREATURES.iter().enumerate() {
        // A summoning-sick source is refused: its ability carries a printed `{T}` cost.
        let mut sick = mana_engine(90_026_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut sick, 0, case.id);
        sick.state
            .objects
            .get_mut(&source)
            .expect("source")
            .summoning_sick = true;
        let payer = inject_creature_on_battlefield(&mut sick, 0, "grizzly_bears");
        assert!(
            sick.apply_scenario_command(0, &activate_with_tap(&sick, source, 0, payer, 0))
                .is_err(),
            "{}: a summoning-sick source cannot pay its own {{T}} cost",
            case.id
        );
        assert!(!sick.state.objects[&source].tapped, "{}", case.id);
        assert!(!sick.state.objects[&payer].tapped, "{}", case.id);
        assert_eq!(pool(&sick, 0), (0, 0, 0, 0, 0, 0), "{}", case.id);

        // A summoning-sick payer is fine: the printed cost is "tap an untapped creature you
        // control", which this pinned ruling explicitly confirms is legal.
        let mut fresh = mana_engine(90_027_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut fresh, 0, case.id);
        fresh
            .state
            .objects
            .get_mut(&source)
            .expect("source")
            .summoning_sick = false;
        let payer = inject_creature_on_battlefield(&mut fresh, 0, "grizzly_bears");
        fresh
            .state
            .objects
            .get_mut(&payer)
            .expect("payer")
            .summoning_sick = true;
        fresh
            .apply_scenario_command(0, &activate_with_tap(&fresh, source, 0, payer, 0))
            .unwrap_or_else(|error| {
                panic!("{}: a summoning-sick payer is legal: {error}", case.id)
            });
        assert!(
            fresh.state.objects[&payer].tapped,
            "{}: the summoning-sick payer was tapped",
            case.id
        );
        assert_eq!(
            pool(&fresh, 0),
            mask('w'),
            "{}: the first printed option is white",
            case.id
        );
    }
}

#[test]
fn q90_any_color_creatures_offer_all_five_colors_and_keep_their_keywords() {
    for (index, case) in MANA_CREATURES.iter().enumerate() {
        for (option, color) in ['w', 'u', 'b', 'r', 'g'].into_iter().enumerate() {
            let mut e = mana_engine(90_023_000 + index as u64 * 10 + option as u64, case.id);
            let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
            e.state
                .objects
                .get_mut(&source)
                .expect("source")
                .summoning_sick = false;
            assert_eq!(
                (e.effective_power(source), e.effective_toughness(source)),
                (Some(case.body.0), Some(case.body.1)),
                "{}: printed body",
                case.id
            );
            if let Some(keyword) = case.keyword {
                assert!(
                    e.effective_has_keyword(source, keyword),
                    "{}: printed keyword survives the activation",
                    case.id
                );
            }
            let helper = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
            e.apply_scenario_command(0, &activate_with_tap(&e, source, 0, helper, option as u32))
                .unwrap_or_else(|error| panic!("{} option {option}: {error}", case.id));
            assert_eq!(
                pool(&e, 0),
                mask(color),
                "{} option {option}: exact printed color",
                case.id
            );
            if let Some(keyword) = case.keyword {
                assert!(
                    e.effective_has_keyword(source, keyword),
                    "{}: the keyword is still there after the activation",
                    case.id
                );
            }
        }
    }
}

// ── 03 Keyword plus "{cost}, {T}: Tap target creature." ─────────────────

struct TapCase {
    id: &'static str,
    keyword: Keyword,
    activation: &'static str,
    body: (u32, u32),
}

const TAP_CREATURES: &[TapCase] = &[
    TapCase {
        id: "mothrider_patrol",
        keyword: Keyword::Flying,
        activation: "{3}{W}",
        body: (1, 1),
    },
    TapCase {
        id: "dazzling_ramparts",
        keyword: Keyword::Defender,
        activation: "{1}{W}",
        body: (0, 7),
    },
    TapCase {
        id: "squall_drifter",
        keyword: Keyword::Flying,
        activation: "{W}",
        body: (1, 1),
    },
    TapCase {
        id: "clockwork_drawbridge",
        keyword: Keyword::Defender,
        activation: "{2}{W}",
        body: (0, 3),
    },
];

fn tap_engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![deck_with("plains", &[card]), deck_with("forest", &[])]);
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

/// Canary: the simplest card — pay the printed cost, tap the source, tap the chosen creature.
#[test]
fn q90_keyword_tap_canary_taps_the_chosen_creature_and_itself() {
    let mut e = tap_engine(90_030_001, "squall_drifter");
    let source = move_ready_to_battlefield(&mut e, 0, "squall_drifter");
    let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    assert!(
        e.effective_has_keyword(source, Keyword::Flying),
        "the printed flying is unaffected"
    );
    give_activation(&mut e, 0, "{W}");
    e.apply_scenario_command(
        0,
        &activate_ability_for(&e, source, 0, target_object(target)),
    )
    .expect("the printed activation");
    resolve_entire_stack_two_player(&mut e);
    assert!(
        e.state.objects[&source].tapped,
        "the source taps itself as a cost"
    );
    assert!(
        e.state.objects[&target].tapped,
        "the chosen creature is tapped"
    );
    assert_eq!(
        e.state.players[0].mana_pool.white, 0,
        "the printed mana was spent"
    );
}

/// The printed cost is exact: one white plus N generic, never a different colour.
#[test]
fn q90_keyword_tap_requires_the_printed_colored_cost() {
    for (index, case) in TAP_CREATURES.iter().enumerate() {
        // Green mana alone cannot pay a white cost.
        let mut green = tap_engine(90_031_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut green, 0, case.id);
        let target = inject_creature_on_battlefield(&mut green, 1, "grizzly_bears");
        give_mana(
            &mut green,
            0,
            ManaGift {
                g: 6,
                ..Default::default()
            },
        );
        assert!(
            green
                .apply_scenario_command(
                    0,
                    &activate_ability_for(&green, source, 0, target_object(target))
                )
                .is_err(),
            "{}: green mana cannot pay the white pip",
            case.id
        );
        assert!(!green.state.objects[&source].tapped, "{}", case.id);
        assert!(!green.state.objects[&target].tapped, "{}", case.id);

        // The exact printed amount is required; one white short is refused.
        let mut short = tap_engine(90_032_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut short, 0, case.id);
        let target = inject_creature_on_battlefield(&mut short, 1, "grizzly_bears");
        let cost = tricerules_cards::ManaCost::parse(case.activation).expect("cost");
        let mut generic = 0;
        let mut whites = 0;
        for pip in &cost.pips {
            match pip {
                tricerules_cards::ManaSymbol::W => whites += 1,
                tricerules_cards::ManaSymbol::Generic(n) => generic += n,
                other => panic!("unexpected printed symbol {other:?}"),
            }
        }
        give_mana(
            &mut short,
            0,
            ManaGift {
                w: whites - 1,
                c: generic + 1,
                ..Default::default()
            },
        );
        assert!(
            short
                .apply_scenario_command(
                    0,
                    &activate_ability_for(&short, source, 0, target_object(target))
                )
                .is_err(),
            "{}: a generic mana cannot replace the printed white pip",
            case.id
        );
        assert!(!short.state.objects[&target].tapped, "{}", case.id);
    }
}

/// The printed amount is exact: paying one white too few is refused, and so is a pool that is one
/// short *after* substituting generic mana for the coloured pips. This is what separates a
/// `{3}{W}` activation from the `{W}` activation next door in the same family.
#[test]
fn q90_keyword_tap_requires_the_exact_printed_amount() {
    for (index, case) in TAP_CREATURES.iter().enumerate() {
        let cost = tricerules_cards::ManaCost::parse(case.activation).expect("printed cost");
        let (mut whites, mut generic) = (0u32, 0u32);
        for pip in &cost.pips {
            match pip {
                tricerules_cards::ManaSymbol::W => whites += 1,
                tricerules_cards::ManaSymbol::Generic(n) => generic += n,
                other => panic!("unexpected printed symbol {other:?}"),
            }
        }
        // One blue overflow instead of the last white: refused.
        let mut short = tap_engine(90_035_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut short, 0, case.id);
        let target = inject_creature_on_battlefield(&mut short, 1, "grizzly_bears");
        give_mana(
            &mut short,
            0,
            ManaGift {
                w: whites.saturating_sub(1),
                u: 1,
                c: generic,
                ..Default::default()
            },
        );
        assert!(
            short
                .apply_scenario_command(0, &activate_ability_for(&short, source, 0, target_object(target)))
                .is_err(),
            "{}: a blue mana cannot stand in for the printed white pip ({whites} white, {generic} generic)",
            case.id
        );
        assert!(!short.state.objects[&target].tapped, "{}", case.id);

        // Exactly the printed amount succeeds and taps the chosen creature.
        let mut exact = tap_engine(90_036_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut exact, 0, case.id);
        let target = inject_creature_on_battlefield(&mut exact, 1, "grizzly_bears");
        give_mana(
            &mut exact,
            0,
            ManaGift {
                w: whites,
                c: generic,
                ..Default::default()
            },
        );
        exact
            .apply_scenario_command(
                0,
                &activate_ability_for(&exact, source, 0, target_object(target)),
            )
            .unwrap_or_else(|error| panic!("{} exact {whites}W/{generic}C: {error}", case.id));
        // The whole printed amount is consumed: nothing is left over. This distinguishes the
        // `{3}{W}` activation from the `{W}` activation in the same family.
        assert_eq!(
            pool(&exact, 0),
            (0, 0, 0, 0, 0, 0),
            "{}: exactly {whites} white and {generic} generic are spent",
            case.id
        );
        resolve_entire_stack_two_player(&mut exact);
        assert!(
            exact.state.objects[&target].tapped,
            "{}: exactly the printed amount pays and taps",
            case.id
        );
    }
}

/// A stale target fizzles the tap: it is not tapped, and the costs stay paid. This mirrors the
/// shipped `tap_target_mana_batch_cards_handle_stale_or_expired_state` fixture, which bumps the
/// zone-change generation so the revalidation is really exercised (CR 608.2b).
#[test]
fn q90_keyword_tap_fizzles_on_a_stale_target() {
    for (index, case) in TAP_CREATURES.iter().enumerate() {
        let mut e = tap_engine(90_033_000 + index as u64, case.id);
        let source = move_ready_to_battlefield(&mut e, 0, case.id);
        let target = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        give_activation(&mut e, 0, case.activation);
        e.apply_scenario_command(
            0,
            &activate_ability_for(&e, source, 0, target_object(target)),
        )
        .unwrap_or_else(|error| panic!("{}: {error}", case.id));
        assert!(
            e.state.objects[&source].tapped,
            "{}: the source taps itself when the ability is activated",
            case.id
        );
        // The target leaves and its generation advances, so a stale identity is revalidated.
        e.state.players[1].battlefield.retain(|oid| *oid != target);
        e.state.objects.get_mut(&target).expect("target").zone = Zone::Exile;
        e.state.players[1].exile.push(target);
        *e.state.zone_change_generation.entry(target).or_insert(0) += 1;
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&target].zone,
            Zone::Exile,
            "{}: a stale target is not returned",
            case.id
        );
        assert!(
            !e.state.objects[&target].tapped,
            "{}: a stale target is not tapped",
            case.id
        );
        assert!(
            e.state.objects[&source].tapped,
            "{}: the tap cost remains paid",
            case.id
        );
        assert_eq!(
            pool(&e, 0),
            (0, 0, 0, 0, 0, 0),
            "{}: the mana cost remains paid",
            case.id
        );
        assert!(e.state.stack.is_empty(), "{}", case.id);
    }
}

#[test]
fn q90_keyword_tap_cards_keep_their_printed_keyword_and_body() {
    for (index, case) in TAP_CREATURES.iter().enumerate() {
        let mut e = tap_engine(90_034_000 + index as u64, case.id);
        let source = inject_permanent_on_battlefield(&mut e, 0, case.id);
        assert_eq!(
            (e.effective_power(source), e.effective_toughness(source)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        assert!(
            e.effective_has_keyword(source, case.keyword),
            "{}: printed keyword",
            case.id
        );
        // The other printed keyword must not appear: two cards fly and two defend, never both.
        let other = if case.keyword == Keyword::Flying {
            Keyword::Defender
        } else {
            Keyword::Flying
        };
        assert!(
            !e.effective_has_keyword(source, other),
            "{}: the card does not print {other:?}",
            case.id
        );
    }
}
