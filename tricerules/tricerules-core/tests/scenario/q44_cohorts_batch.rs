//! Actual-card scenarios for the queue-v44 reuse-only cohorts.
//!
//! Expectations are printed Oracle values. CR 305.6/305.8 and 605.1a/605.3b govern the
//! conditional land mana abilities; CR 602.1/118.3 govern activated costs; CR 701.21a covers
//! sacrificing as a cost; CR 701.23b/e and 701.24a cover hidden-zone searches and shuffling;
//! CR 303.4a and 613.1f cover attached keyword grants; CR 513.1a/513.2 covers the end-step return.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::primitives::Keyword;
use tricerules_cards::CounterKind;
use tricerules_core::state::AttachmentRecipient;
use tricerules_core::GameEngine;
use tricerules_core::Zone;
use tricerules_proto::ruled::v1::ruled_event::Ev;
use tricerules_proto::ruled::v1::{
    ruled_command::Cmd, ChoiceKind, RuledCommand, TargetRef, TargetRefKind,
};

type ManaPool = (u32, u32, u32, u32, u32, u32);

fn pool(e: &GameEngine) -> ManaPool {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn clear_pool(e: &mut GameEngine, player: usize) {
    let p = &mut e.state.players[player].mana_pool;
    p.white = 0;
    p.blue = 0;
    p.black = 0;
    p.red = 0;
    p.green = 0;
    p.colorless = 0;
}

fn untap(e: &mut GameEngine, oid: u32) {
    e.state.objects.get_mut(&oid).expect("permanent").tapped = false;
    e.initial_response_batch();
}

fn activate_option(e: &GameEngine, source: u32, index: u32, option: u32) -> RuledCommand {
    let mut command = activate_ability_for(e, source, index, vec![]);
    let Some(Cmd::ActivateAbility(activation)) = command.cmd.as_mut() else {
        unreachable!()
    };
    activation.mana_option_index = option;
    command
}

fn stack_target(object_id: u32) -> Vec<TargetRef> {
    vec![TargetRef {
        object_id,
        kind: TargetRefKind::Stack as i32,
        ..Default::default()
    }]
}

fn base_engine(seed: u64, extra: &[&str]) -> GameEngine {
    let decks = Some(vec![deck_with("forest", extra), deck_with("island", &[])]);
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

fn put_card(e: &mut GameEngine, player: i32, card_id: &str) -> u32 {
    use tricerules_proto::ruled::v1::dev_command::Dev;
    use tricerules_proto::ruled::v1::{DevCommand, DevPutCardInZone, DevZone};
    let name = tricerules_cards::registry::global()
        .get(card_id)
        .unwrap_or_else(|| panic!("missing card definition {card_id}"))
        .name
        .clone();
    e.enable_dev_commands();
    e.apply_scenario_command(
        player,
        &RuledCommand {
            cmd: Some(Cmd::DevCommand(DevCommand {
                target_player_id: player,
                dev: Some(Dev::PutCardInZone(DevPutCardInZone {
                    card_name: name,
                    zone: DevZone::Battlefield as i32,
                    ready: true,
                })),
            })),
        },
    )
    .unwrap_or_else(|error| panic!("put {card_id}: {error:?}"));
    battlefield_object_for_card(e, player as usize, card_id)
}

fn end_turn_with_end_step_triggers(e: &mut GameEngine, player: i32) {
    e.apply_scenario_command(player, &primitive_yield())
        .expect("main1 to begin combat");
    e.apply_scenario_command(player, &primitive_yield())
        .expect("begin combat advance");
    if e.state.turn_step == tricerules_core::TurnStep::DeclareAttackers {
        e.apply_scenario_command(player, &primitive_yield())
            .expect("skip attackers to end combat");
    }
    e.apply_scenario_command(player, &primitive_yield())
        .expect("end combat to main2");
    e.apply_scenario_command(player, &primitive_yield())
        .expect("main2 to end step");
    resolve_entire_stack_two_player(e);
    e.apply_scenario_command(player, &primitive_yield())
        .expect("end step to cleanup or next upkeep");
    resolve_cleanup_discards_if_any(e);
}

fn cast_aura_on(e: &mut GameEngine, card: &str, target: u32, mana: ManaGift) {
    inject_card_into_hand(e, 0, card);
    give_mana(e, 0, mana);
    let slot = hand_index_for_card(e, 0, card);
    e.apply_scenario_command(0, &cast_spell(slot, target_object(target)))
        .unwrap_or_else(|error| panic!("cast {card}: {error:?}"));
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.state.objects.get(&target).map(|o| o.zone),
        Some(Zone::Battlefield),
        "{card}: recipient"
    );
}

// ── 01 Verge lands ───────────────────────────────────────────────────────────

struct Verge {
    id: &'static str,
    first: u8,
    second: u8,
    subtype_a: &'static str,
    subtype_b: &'static str,
}

const VERGES: [Verge; 6] = [
    Verge {
        id: "bleachbone_verge",
        first: 2,
        second: 0,
        subtype_a: "plains",
        subtype_b: "swamp",
    },
    Verge {
        id: "blazemire_verge",
        first: 2,
        second: 3,
        subtype_a: "swamp",
        subtype_b: "mountain",
    },
    Verge {
        id: "hushwood_verge",
        first: 4,
        second: 0,
        subtype_a: "forest",
        subtype_b: "plains",
    },
    Verge {
        id: "sunbillow_verge",
        first: 0,
        second: 3,
        subtype_a: "mountain",
        subtype_b: "plains",
    },
    Verge {
        id: "thornspire_verge",
        first: 3,
        second: 4,
        subtype_a: "mountain",
        subtype_b: "forest",
    },
    Verge {
        id: "wastewood_verge",
        first: 4,
        second: 2,
        subtype_a: "swamp",
        subtype_b: "forest",
    },
];

fn expected_one(slot: u8) -> ManaPool {
    let mut value = [0; 6];
    value[slot as usize] = 1;
    (value[0], value[1], value[2], value[3], value[4], value[5])
}

#[test]
fn q44_verge_canary_produces_the_printed_colors() {
    let mut e = base_engine(44_010_001, &[]);
    let land = inject_permanent_on_battlefield(&mut e, 0, "bleachbone_verge");
    e.apply_scenario_command(0, &activate_ability_for(&e, land, 0, vec![]))
        .expect("first mana ability");
    assert_eq!(
        pool(&e),
        expected_one(2),
        "Bleachbone's first ability adds black"
    );
    assert!(
        e.state.stack.is_empty(),
        "mana abilities do not use the stack"
    );
    untap(&mut e, land);
    clear_pool(&mut e, 0);
    inject_permanent_on_battlefield(&mut e, 0, "plains");
    e.apply_scenario_command(0, &activate_ability_for(&e, land, 1, vec![]))
        .expect("conditional mana ability");
    assert_eq!(
        pool(&e),
        expected_one(0),
        "with a Plains the second ability adds white"
    );
}

#[test]
fn q44_verge_cards_require_their_printed_condition() {
    for (index, case) in VERGES.iter().enumerate() {
        let mut e = base_engine(44_011_000 + index as u64, &[]);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, land, 1, vec![]))
                .is_err(),
            "{}: no listed basic land rejects activation",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{}: no mana", case.id);
        inject_permanent_on_battlefield(&mut e, 0, case.subtype_a);
        e.apply_scenario_command(0, &activate_ability_for(&e, land, 1, vec![]))
            .unwrap_or_else(|error| panic!("{} first subtype: {error:?}", case.id));
        assert_eq!(pool(&e), expected_one(case.second), "{}", case.id);

        let mut e = base_engine(44_012_000 + index as u64, &[]);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        inject_permanent_on_battlefield(&mut e, 0, case.subtype_b);
        e.apply_scenario_command(0, &activate_ability_for(&e, land, 1, vec![]))
            .unwrap_or_else(|error| panic!("{} second subtype: {error:?}", case.id));
        assert_eq!(pool(&e), expected_one(case.second), "{}", case.id);

        let mut e = base_engine(44_013_000 + index as u64, &[]);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        inject_permanent_on_battlefield(&mut e, 0, "island");
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, land, 1, vec![]))
                .is_err(),
            "{}: an unrelated basic land does not enable the second ability",
            case.id
        );
    }
}

#[test]
fn q44_verge_cards_keep_the_unconditional_first_ability() {
    for (index, case) in VERGES.iter().enumerate() {
        let mut e = base_engine(44_014_000 + index as u64, &[]);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &activate_ability_for(&e, land, 0, vec![]))
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
        assert_eq!(pool(&e), expected_one(case.first), "{}", case.id);
    }
}

#[test]
fn q44_verge_condition_ignores_opponent_lands() {
    for (index, case) in VERGES.iter().enumerate() {
        let mut e = base_engine(44_015_000 + index as u64, &[]);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        inject_permanent_on_battlefield(&mut e, 1, case.subtype_a);
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, land, 1, vec![]))
                .is_err(),
            "{}: an opponent's basic land does not enable the condition",
            case.id
        );
    }
}

// ── 02 Tainted lands ─────────────────────────────────────────────────────────

struct Tainted {
    id: &'static str,
    options: [(u8, u8); 2],
}

const TAINTED: [Tainted; 4] = [
    Tainted {
        id: "tainted_field",
        options: [(0, 0), (2, 0)],
    },
    Tainted {
        id: "tainted_isle",
        options: [(1, 0), (2, 0)],
    },
    Tainted {
        id: "tainted_peak",
        options: [(2, 0), (3, 0)],
    },
    Tainted {
        id: "tainted_wood",
        options: [(2, 0), (4, 0)],
    },
];

#[test]
fn q44_tainted_canary_adds_colorless_and_a_chosen_color_with_a_swamp() {
    let mut e = base_engine(44_020_001, &[]);
    let land = inject_permanent_on_battlefield(&mut e, 0, "tainted_field");
    e.apply_scenario_command(0, &activate_ability_for(&e, land, 0, vec![]))
        .expect("colorless ability");
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 1));
    untap(&mut e, land);
    clear_pool(&mut e, 0);
    inject_permanent_on_battlefield(&mut e, 0, "swamp");
    e.apply_scenario_command(0, &activate_option(&e, land, 1, 0))
        .expect("white option");
    assert_eq!(pool(&e), expected_one(0));
    untap(&mut e, land);
    clear_pool(&mut e, 0);
    e.apply_scenario_command(0, &activate_option(&e, land, 1, 1))
        .expect("black option");
    assert_eq!(pool(&e), expected_one(2));
}

#[test]
fn q44_tainted_cards_require_a_swamp() {
    for (index, case) in TAINTED.iter().enumerate() {
        let mut e = base_engine(44_021_000 + index as u64, &[]);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &activate_option(&e, land, 1, 0))
                .is_err(),
            "{}: no Swamp rejects the colored ability",
            case.id
        );
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0));
        inject_permanent_on_battlefield(&mut e, 1, "swamp");
        assert!(
            e.apply_scenario_command(0, &activate_option(&e, land, 1, 0))
                .is_err(),
            "{}: an opponent's Swamp does not enable the condition",
            case.id
        );
        inject_permanent_on_battlefield(&mut e, 0, "swamp");
        for (option, (slot, _)) in case.options.iter().enumerate() {
            untap(&mut e, land);
            clear_pool(&mut e, 0);
            e.apply_scenario_command(0, &activate_option(&e, land, 1, option as u32))
                .unwrap_or_else(|error| panic!("{} option {option}: {error:?}", case.id));
            assert_eq!(pool(&e), expected_one(*slot), "{} option {option}", case.id);
        }
    }
}

#[test]
fn q44_tainted_cards_keep_the_colorless_ability() {
    for (index, case) in TAINTED.iter().enumerate() {
        let mut e = base_engine(44_022_000 + index as u64, &[]);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &activate_ability_for(&e, land, 0, vec![]))
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 1), "{}", case.id);
    }
}

// ── 03 Monument animation ────────────────────────────────────────────────────

struct Monument {
    id: &'static str,
    first: u8,
    second: u8,
    colors: [tricerules_cards::Color; 2],
    cost: ManaGift,
}

const MONUMENTS: [Monument; 5] = [
    Monument {
        id: "atarka_monument",
        first: 3,
        second: 4,
        colors: [tricerules_cards::Color::Red, tricerules_cards::Color::Green],
        cost: ManaGift {
            w: 0,
            u: 0,
            b: 0,
            r: 1,
            g: 1,
            c: 4,
        },
    },
    Monument {
        id: "dromoka_monument",
        first: 4,
        second: 0,
        colors: [
            tricerules_cards::Color::Green,
            tricerules_cards::Color::White,
        ],
        cost: ManaGift {
            w: 1,
            u: 0,
            b: 0,
            r: 0,
            g: 1,
            c: 4,
        },
    },
    Monument {
        id: "kolaghan_monument",
        first: 2,
        second: 3,
        colors: [tricerules_cards::Color::Black, tricerules_cards::Color::Red],
        cost: ManaGift {
            w: 0,
            u: 0,
            b: 1,
            r: 1,
            g: 0,
            c: 4,
        },
    },
    Monument {
        id: "ojutai_monument",
        first: 0,
        second: 1,
        colors: [
            tricerules_cards::Color::White,
            tricerules_cards::Color::Blue,
        ],
        cost: ManaGift {
            w: 1,
            u: 1,
            b: 0,
            r: 0,
            g: 0,
            c: 4,
        },
    },
    Monument {
        id: "silumgar_monument",
        first: 1,
        second: 2,
        colors: [
            tricerules_cards::Color::Blue,
            tricerules_cards::Color::Black,
        ],
        cost: ManaGift {
            w: 0,
            u: 1,
            b: 1,
            r: 0,
            g: 0,
            c: 4,
        },
    },
];

fn gift(value: &ManaGift) -> ManaGift {
    ManaGift {
        w: value.w,
        u: value.u,
        b: value.b,
        r: value.r,
        g: value.g,
        c: value.c,
    }
}

#[test]
fn q44_monument_canary_animates_itself_until_cleanup() {
    let mut e = base_engine(44_030_001, &[]);
    let monument = inject_permanent_on_battlefield(&mut e, 0, "atarka_monument");
    e.state
        .objects
        .get_mut(&monument)
        .expect("Monument")
        .set_counter(CounterKind::PlusOnePlusOne, 1);
    e.apply_scenario_command(0, &activate_option(&e, monument, 0, 0))
        .expect("red mana option");
    assert_eq!(pool(&e), expected_one(3));
    untap(&mut e, monument);
    clear_pool(&mut e, 0);
    give_mana(&mut e, 0, gift(&MONUMENTS[0].cost));
    e.apply_scenario_command(0, &activate_ability_for(&e, monument, 1, vec![]))
        .expect("animate");
    resolve_entire_stack_two_player(&mut e);
    let animated = e.characteristics(monument).expect("animated Monument");
    assert!(animated.is_creature(), "the artifact becomes a creature");
    assert!(animated.has_type("Artifact"), "it remains an artifact");
    assert!(animated.has_type("Dragon"), "printed creature type");
    assert_eq!(animated.power, Some(5), "+1/+1 counter plus the 4/4 base");
    assert_eq!(animated.toughness, Some(5));
    assert_eq!(
        animated.colors,
        vec![tricerules_cards::Color::Red, tricerules_cards::Color::Green]
    );
    assert!(animated.keywords.contains(&Keyword::Flying));
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "exact animation cost");

    end_active_turn(&mut e, 0);
    advance_to_main1_from_game_start(&mut e);
    end_active_turn(&mut e, 1);
    advance_to_main1_from_game_start(&mut e);
    let reverted = e.characteristics(monument).expect("reverted Monument");
    assert!(!reverted.is_creature(), "cleanup ends the animation");
    assert!(
        !reverted.has_type("Dragon"),
        "the Dragon type ends with the animation"
    );
    assert!(
        !reverted.keywords.contains(&Keyword::Flying),
        "flying ends with the animation"
    );
    assert!(reverted.has_type("Artifact"));
    assert!(
        reverted.colors.is_empty(),
        "the artifact is colorless again"
    );
    assert_eq!(
        e.state.objects[&monument]
            .counters
            .get(&CounterKind::PlusOnePlusOne)
            .copied(),
        Some(1),
        "the +1/+1 counter remains after the animation ends"
    );

    untap(&mut e, monument);
    give_mana(&mut e, 0, gift(&MONUMENTS[0].cost));
    e.apply_scenario_command(0, &activate_ability_for(&e, monument, 1, vec![]))
        .expect("animate again");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(e.effective_power(monument), Some(5));
    assert_eq!(e.effective_toughness(monument), Some(5));
}

#[test]
fn q44_monument_cards_animate_with_printed_colors_and_costs() {
    for (index, case) in MONUMENTS.iter().enumerate() {
        let mut e = base_engine(44_031_000 + index as u64, &[]);
        let monument = inject_permanent_on_battlefield(&mut e, 0, case.id);
        e.apply_scenario_command(0, &activate_option(&e, monument, 0, 0))
            .unwrap_or_else(|error| panic!("{} mana option: {error:?}", case.id));
        assert_eq!(pool(&e), expected_one(case.first), "{}", case.id);
        untap(&mut e, monument);
        clear_pool(&mut e, 0);
        e.apply_scenario_command(0, &activate_option(&e, monument, 0, 1))
            .unwrap_or_else(|error| panic!("{} second mana option: {error:?}", case.id));
        assert_eq!(pool(&e), expected_one(case.second), "{}", case.id);
        untap(&mut e, monument);
        clear_pool(&mut e, 0);
        give_mana(&mut e, 0, gift(&case.cost));
        e.apply_scenario_command(0, &activate_ability_for(&e, monument, 1, vec![]))
            .unwrap_or_else(|error| panic!("{} animate: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        let value = e.characteristics(monument).expect("animated");
        assert!(value.is_creature(), "{}", case.id);
        assert!(value.has_type("Dragon"), "{}", case.id);
        assert_eq!(
            (value.power, value.toughness),
            (Some(4), Some(4)),
            "{}",
            case.id
        );
        assert_eq!(value.colors, case.colors.to_vec(), "{}", case.id);
        assert!(value.keywords.contains(&Keyword::Flying), "{}", case.id);
        assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "{} exact cost", case.id);
    }
}

#[test]
fn q44_monument_animation_requires_the_printed_mana() {
    for (index, case) in MONUMENTS.iter().enumerate() {
        let mut e = base_engine(44_032_000 + index as u64, &[]);
        let monument = inject_permanent_on_battlefield(&mut e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &activate_ability_for(&e, monument, 1, vec![]))
                .is_err(),
            "{}: missing mana rejects the animation",
            case.id
        );
        assert!(!e.characteristics(monument).expect("artifact").is_creature());
    }
}

// ── 04 Hybrid evasion ───────────────────────────────────────────────────────

struct HybridEvasion {
    id: &'static str,
    hated: tricerules_cards::Color,
    hated_blocker: &'static str,
    allowed_blocker: &'static str,
}

const HYBRID_EVASION: [HybridEvasion; 5] = [
    HybridEvasion {
        id: "sootwalkers",
        hated: tricerules_cards::Color::White,
        hated_blocker: "savannah_lions",
        allowed_blocker: "grizzly_bears",
    },
    HybridEvasion {
        id: "wanderbrine_rootcutters",
        hated: tricerules_cards::Color::Green,
        hated_blocker: "grizzly_bears",
        allowed_blocker: "savannah_lions",
    },
    HybridEvasion {
        id: "barrenton_cragtreads",
        hated: tricerules_cards::Color::Red,
        hated_blocker: "mogg_fanatic",
        allowed_blocker: "grizzly_bears",
    },
    HybridEvasion {
        id: "mudbrawler_raiders",
        hated: tricerules_cards::Color::Blue,
        hated_blocker: "cloud_spirit",
        allowed_blocker: "grizzly_bears",
    },
    HybridEvasion {
        id: "ravens_run_dragoon",
        hated: tricerules_cards::Color::Black,
        hated_blocker: "walking_dead",
        allowed_blocker: "grizzly_bears",
    },
];

fn advance_main1_to_declare_attackers(e: &mut GameEngine) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    e.apply_scenario_command(0, &pass())
        .expect("active player passes in beginning of combat");
    e.apply_scenario_command(1, &pass())
        .expect("defender passes in beginning of combat");
    assert_eq!(
        e.state.turn_step,
        tricerules_core::TurnStep::DeclareAttackers
    );
}

#[test]
fn q44_hybrid_evasion_cards_reject_only_the_printed_color() {
    for (index, case) in HYBRID_EVASION.iter().enumerate() {
        let mut e = base_engine(44_040_000 + index as u64, &[case.id]);
        let attacker = put_card(&mut e, 0, case.id);
        assert_eq!(
            (e.effective_power(attacker), e.effective_toughness(attacker)),
            (Some(3), Some(3)),
            "{}: printed body",
            case.id
        );
        let hated = put_card(&mut e, 1, case.hated_blocker);
        let allowed = put_card(&mut e, 1, case.allowed_blocker);
        assert!(
            e.characteristics(hated)
                .expect("hated blocker")
                .colors
                .contains(&case.hated),
            "{}: fixture color",
            case.id
        );
        advance_main1_to_declare_attackers(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare attacker");
        e.apply_scenario_command(0, &pass())
            .expect("active player passes");
        e.apply_scenario_command(1, &pass())
            .expect("defender passes");
        assert_eq!(
            e.state.turn_step,
            tricerules_core::TurnStep::DeclareBlockers
        );
        let pairs = &e.initial_response_batch().legal_by_player[&1].legal_block_pairs;
        assert!(
            !pairs
                .iter()
                .any(|pair| pair.attacker_id == attacker && pair.blocker_id == hated),
            "{}: the printed color cannot block",
            case.id
        );
        assert!(
            pairs
                .iter()
                .any(|pair| pair.attacker_id == attacker && pair.blocker_id == allowed),
            "{}: another color can block",
            case.id
        );
    }
}

#[test]
fn q44_hybrid_evasion_canary_checks_the_printed_color() {
    let mut e = base_engine(44_041_001, &["sootwalkers"]);
    let attacker = put_card(&mut e, 0, "sootwalkers");
    let white = put_card(&mut e, 1, "savannah_lions");
    let green = put_card(&mut e, 1, "grizzly_bears");
    advance_main1_to_declare_attackers(&mut e);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    e.apply_scenario_command(0, &pass())
        .expect("active player passes");
    e.apply_scenario_command(1, &pass())
        .expect("defender passes");
    let pairs = e.initial_response_batch().legal_by_player[&1]
        .legal_block_pairs
        .clone();
    assert!(!pairs
        .iter()
        .any(|pair| pair.attacker_id == attacker && pair.blocker_id == white));
    assert!(pairs
        .iter()
        .any(|pair| pair.attacker_id == attacker && pair.blocker_id == green));
}

// ── 05 Keyword Auras ─────────────────────────────────────────────────────────

struct KeywordAura {
    id: &'static str,
    keywords: &'static [Keyword],
    mana: ManaGift,
    flash: bool,
}

const KEYWORD_AURAS: [KeywordAura; 5] = [
    KeywordAura {
        id: "alexis_cloak",
        keywords: &[Keyword::Shroud],
        mana: ManaGift {
            w: 0,
            u: 1,
            b: 0,
            r: 0,
            g: 0,
            c: 1,
        },
        flash: true,
    },
    KeywordAura {
        id: "buoyancy",
        keywords: &[Keyword::Flying],
        mana: ManaGift {
            w: 0,
            u: 1,
            b: 0,
            r: 0,
            g: 0,
            c: 1,
        },
        flash: true,
    },
    KeywordAura {
        id: "eternal_warrior",
        keywords: &[Keyword::Vigilance],
        mana: ManaGift {
            w: 0,
            u: 0,
            b: 0,
            r: 1,
            g: 0,
            c: 0,
        },
        flash: false,
    },
    KeywordAura {
        id: "reflexes",
        keywords: &[Keyword::FirstStrike],
        mana: ManaGift {
            w: 0,
            u: 0,
            b: 0,
            r: 1,
            g: 0,
            c: 0,
        },
        flash: false,
    },
    KeywordAura {
        id: "zephyr_net",
        keywords: &[Keyword::Defender, Keyword::Flying],
        mana: ManaGift {
            w: 0,
            u: 1,
            b: 0,
            r: 0,
            g: 0,
            c: 1,
        },
        flash: false,
    },
];

#[test]
fn q44_keyword_auras_canary_grants_first_strike() {
    let mut e = base_engine(44_050_001, &[]);
    let creature = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    let bystander = inject_creature_with_stats(&mut e, 0, "savannah_lions", 2, 2);
    cast_aura_on(
        &mut e,
        "reflexes",
        creature,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    assert!(e
        .characteristics(creature)
        .expect("creature")
        .keywords
        .contains(&Keyword::FirstStrike));
    assert!(!e
        .characteristics(bystander)
        .expect("bystander")
        .keywords
        .contains(&Keyword::FirstStrike));
}

#[test]
fn q44_keyword_auras_grant_their_printed_keywords() {
    for (index, case) in KEYWORD_AURAS.iter().enumerate() {
        let mut e = base_engine(44_051_000 + index as u64, &[]);
        let creature = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        cast_aura_on(&mut e, case.id, creature, gift(&case.mana));
        let value = e.characteristics(creature).expect("enchanted creature");
        for keyword in case.keywords {
            assert!(value.keywords.contains(keyword), "{}: {keyword:?}", case.id);
        }
        let aura = battlefield_object_for_card(&e, 0, case.id);
        assert_eq!(
            e.state.objects[&aura].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert_eq!(
            e.state.objects[&aura].attached_to,
            Some(AttachmentRecipient::Object(creature)),
            "{}",
            case.id
        );
        let definition = tricerules_cards::registry::global()
            .get(case.id)
            .expect("definition");
        assert_eq!(
            definition.primary_face().keywords.contains(&Keyword::Flash),
            case.flash,
            "{}: printed Flash",
            case.id
        );
    }
}

#[test]
fn q44_keyword_auras_stop_granting_when_the_aura_leaves() {
    let mut e = base_engine(44_052_001, &[]);
    let creature = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    cast_aura_on(
        &mut e,
        "buoyancy",
        creature,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    assert!(e
        .characteristics(creature)
        .expect("creature")
        .keywords
        .contains(&Keyword::Flying));
    let aura = battlefield_object_for_card(&e, 0, "buoyancy");
    e.state.players[0].battlefield.retain(|oid| *oid != aura);
    e.state.players[0].graveyard.push(aura);
    e.state.objects.get_mut(&aura).expect("Aura").zone = Zone::Graveyard;
    e.initial_response_batch();
    assert!(!e
        .characteristics(creature)
        .expect("creature")
        .keywords
        .contains(&Keyword::Flying));
}

#[test]
fn q44_keyword_auras_shroud_blocks_targeting() {
    let mut e = base_engine(44_053_001, &["lightning_bolt"]);
    let creature = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    cast_aura_on(
        &mut e,
        "alexis_cloak",
        creature,
        ManaGift {
            u: 1,
            c: 1,
            ..Default::default()
        },
    );
    inject_card_into_hand(&mut e, 0, "lightning_bolt");
    give_mana(
        &mut e,
        0,
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    let slot = hand_index_for_card(&e, 0, "lightning_bolt");
    assert!(
        e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
            .is_err(),
        "shroud prevents the enchanted creature from being targeted"
    );
    let aura = battlefield_object_for_card(&e, 0, "alexis_cloak");
    e.state.players[0].battlefield.retain(|oid| *oid != aura);
    e.state.players[0].graveyard.push(aura);
    e.state.objects.get_mut(&aura).expect("Aura").zone = Zone::Graveyard;
    e.initial_response_batch();
    e.apply_scenario_command(0, &cast_spell(slot, target_object(creature)))
        .expect("without shroud the creature can be targeted");
}

#[test]
fn q44_keyword_auras_require_a_creature_target() {
    for case in KEYWORD_AURAS {
        let mut e = base_engine(44_054_000, &[]);
        inject_card_into_hand(&mut e, 0, case.id);
        give_mana(&mut e, 0, gift(&case.mana));
        let slot = hand_index_for_card(&e, 0, case.id);
        assert!(
            e.apply_scenario_command(0, &cast_spell(slot, target_player(1)))
                .is_err(),
            "{}: an Aura cannot enchant a player",
            case.id
        );
        assert!(!e.state.players[0].hand.is_empty(), "{}", case.id);
    }
}

// ── 06 Color-hate counters ───────────────────────────────────────────────────

struct ColorHate {
    id: &'static str,
    mana: ManaGift,
    matching: &'static str,
    matching_mana: ManaGift,
    matching_target: fn(u32) -> Vec<TargetRef>,
    wrong: &'static str,
    wrong_mana: ManaGift,
    wrong_target: fn(u32) -> Vec<TargetRef>,
}

fn creature_target(oid: u32) -> Vec<TargetRef> {
    target_object(oid)
}

fn player_target(_oid: u32) -> Vec<TargetRef> {
    target_player(1)
}

const COLOR_HATE: [ColorHate; 3] = [
    ColorHate {
        id: "deathgrip",
        mana: ManaGift {
            w: 0,
            u: 0,
            b: 2,
            r: 0,
            g: 0,
            c: 0,
        },
        matching: "giant_growth",
        matching_mana: ManaGift {
            w: 0,
            u: 0,
            b: 0,
            r: 0,
            g: 1,
            c: 0,
        },
        matching_target: creature_target,
        wrong: "lightning_bolt",
        wrong_mana: ManaGift {
            w: 0,
            u: 0,
            b: 0,
            r: 1,
            g: 0,
            c: 0,
        },
        wrong_target: player_target,
    },
    ColorHate {
        id: "douse",
        mana: ManaGift {
            w: 0,
            u: 1,
            b: 0,
            r: 0,
            g: 0,
            c: 1,
        },
        matching: "lightning_bolt",
        matching_mana: ManaGift {
            w: 0,
            u: 0,
            b: 0,
            r: 1,
            g: 0,
            c: 0,
        },
        matching_target: player_target,
        wrong: "giant_growth",
        wrong_mana: ManaGift {
            w: 0,
            u: 0,
            b: 0,
            r: 0,
            g: 1,
            c: 0,
        },
        wrong_target: creature_target,
    },
    ColorHate {
        id: "lifeforce",
        mana: ManaGift {
            w: 0,
            u: 0,
            b: 0,
            r: 0,
            g: 2,
            c: 0,
        },
        matching: "fugue",
        matching_mana: ManaGift {
            w: 0,
            u: 0,
            b: 2,
            r: 0,
            g: 0,
            c: 5,
        },
        matching_target: player_target,
        wrong: "giant_growth",
        wrong_mana: ManaGift {
            w: 0,
            u: 0,
            b: 0,
            r: 0,
            g: 1,
            c: 0,
        },
        wrong_target: creature_target,
    },
];

fn cast_colored_spell(
    e: &mut GameEngine,
    card: &str,
    mana: ManaGift,
    target: fn(u32) -> Vec<TargetRef>,
    host: u32,
) -> u32 {
    inject_card_into_hand(e, 0, card);
    give_mana(e, 0, mana);
    let slot = hand_index_for_card(e, 0, card);
    e.apply_scenario_command(0, &cast_spell(slot, target(host)))
        .unwrap_or_else(|error| panic!("cast {card}: {error:?}"));
    e.state.stack.last().expect("spell on stack").id
}

#[test]
fn q44_color_hate_canary_counters_its_color() {
    let mut e = base_engine(44_060_001, &[]);
    let host = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    inject_permanent_on_battlefield(&mut e, 1, "deathgrip");
    let spell = cast_colored_spell(
        &mut e,
        "giant_growth",
        ManaGift {
            g: 1,
            ..Default::default()
        },
        creature_target,
        host,
    );
    e.apply_scenario_command(0, &pass()).expect("pass priority");
    give_mana(
        &mut e,
        1,
        ManaGift {
            b: 2,
            ..Default::default()
        },
    );
    let grip = battlefield_object_for_card(&e, 1, "deathgrip");
    e.apply_scenario_command(1, &activate_ability_for(&e, grip, 0, stack_target(spell)))
        .expect("counter the green spell");
    assert_eq!(e.state.stack.len(), 2);
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&spell].zone,
        Zone::Graveyard,
        "the spell is countered"
    );
    assert_eq!(
        (e.effective_power(host), e.effective_toughness(host)),
        (Some(2), Some(2)),
        "Giant Growth never resolved"
    );
}

#[test]
fn q44_color_hate_cards_counter_only_their_printed_color() {
    for (index, case) in COLOR_HATE.iter().enumerate() {
        let mut e = base_engine(44_061_000 + index as u64, &[]);
        let host = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        inject_permanent_on_battlefield(&mut e, 1, case.id);
        let spell = cast_colored_spell(
            &mut e,
            case.matching,
            gift(&case.matching_mana),
            case.matching_target,
            host,
        );
        e.apply_scenario_command(0, &pass()).expect("pass priority");
        give_mana(&mut e, 1, gift(&case.mana));
        let source = battlefield_object_for_card(&e, 1, case.id);
        e.apply_scenario_command(1, &activate_ability_for(&e, source, 0, stack_target(spell)))
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
        resolve_entire_stack_two_player(&mut e);
        assert_eq!(
            e.state.objects[&spell].zone,
            Zone::Graveyard,
            "{}: countered",
            case.id
        );
        assert!(e.state.players[0].graveyard.contains(&spell), "{}", case.id);

        let mut e = base_engine(44_062_000 + index as u64, &[]);
        let host = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
        inject_permanent_on_battlefield(&mut e, 1, case.id);
        let wrong = cast_colored_spell(
            &mut e,
            case.wrong,
            gift(&case.wrong_mana),
            case.wrong_target,
            host,
        );
        e.apply_scenario_command(0, &pass()).expect("pass priority");
        give_mana(&mut e, 1, gift(&case.mana));
        let source = battlefield_object_for_card(&e, 1, case.id);
        assert!(
            e.apply_scenario_command(1, &activate_ability_for(&e, source, 0, stack_target(wrong)))
                .is_err(),
            "{}: the other color is not a legal target",
            case.id
        );
        assert_eq!(e.state.stack.len(), 1, "{}", case.id);
    }
}

#[test]
fn q44_color_hate_cards_fizzle_when_the_spell_leaves_the_stack() {
    let mut e = base_engine(44_063_001, &[]);
    let host = inject_creature_with_stats(&mut e, 0, "grizzly_bears", 2, 2);
    inject_permanent_on_battlefield(&mut e, 1, "deathgrip");
    let spell = cast_colored_spell(
        &mut e,
        "giant_growth",
        ManaGift {
            g: 1,
            ..Default::default()
        },
        creature_target,
        host,
    );
    e.apply_scenario_command(0, &pass()).expect("pass priority");
    give_mana(
        &mut e,
        1,
        ManaGift {
            b: 2,
            ..Default::default()
        },
    );
    let grip = battlefield_object_for_card(&e, 1, "deathgrip");
    e.apply_scenario_command(1, &activate_ability_for(&e, grip, 0, stack_target(spell)))
        .expect("counter the green spell");
    e.state.stack.retain(|entry| entry.id != spell);
    e.state.objects.get_mut(&spell).expect("spell").zone = Zone::Graveyard;
    e.state.players[0].graveyard.push(spell);
    *e.state.zone_change_generation.entry(spell).or_insert(0) += 1;
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        e.state.objects[&spell].zone,
        Zone::Graveyard,
        "stale target stays moved"
    );
    assert_eq!(
        e.state.objects[&grip].zone,
        Zone::Battlefield,
        "the counter enchantment is a permanent, not a card that moves"
    );
    assert!(e.state.stack.is_empty());
}

// ── 07 Basic-land pump ──────────────────────────────────────────────────────

struct LandPump {
    id: &'static str,
    body: (u32, u32),
    bonus: (u32, u32),
    subtype: &'static str,
}

const LAND_PUMPS: [LandPump; 3] = [
    LandPump {
        id: "kird_ape",
        body: (1, 1),
        bonus: (1, 2),
        subtype: "forest",
    },
    LandPump {
        id: "loam_lion",
        body: (1, 1),
        bonus: (1, 2),
        subtype: "forest",
    },
    LandPump {
        id: "mire_kavu",
        body: (3, 2),
        bonus: (1, 1),
        subtype: "swamp",
    },
];

#[test]
fn q44_basic_land_pump_canary_counts_a_forest() {
    let mut e = base_engine(44_070_001, &["kird_ape"]);
    inject_card_into_hand(&mut e, 0, "forest");
    let slot = hand_index_for_card(&e, 0, "forest");
    e.apply_scenario_command(0, &play_land(slot))
        .expect("play Forest");
    let ape = put_card(&mut e, 0, "kird_ape");
    assert_eq!(
        (e.effective_power(ape), e.effective_toughness(ape)),
        (Some(2), Some(3))
    );
    inject_permanent_on_battlefield(&mut e, 0, "forest");
    assert_eq!(
        (e.effective_power(ape), e.effective_toughness(ape)),
        (Some(2), Some(3)),
        "binary, not per land"
    );
}

#[test]
fn q44_basic_land_pump_cards_apply_printed_bonus_once() {
    for (index, case) in LAND_PUMPS.iter().enumerate() {
        let mut e = base_engine(44_071_000 + index as u64, &[case.id]);
        let creature = put_card(&mut e, 0, case.id);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: base body",
            case.id
        );
        let land = inject_permanent_on_battlefield(&mut e, 0, case.subtype);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (
                Some(case.body.0 + case.bonus.0),
                Some(case.body.1 + case.bonus.1)
            ),
            "{}: printed bonus",
            case.id
        );
        let second = inject_permanent_on_battlefield(&mut e, 0, case.subtype);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (
                Some(case.body.0 + case.bonus.0),
                Some(case.body.1 + case.bonus.1)
            ),
            "{}: still binary",
            case.id
        );
        for removed in [land, second] {
            e.state.players[0].battlefield.retain(|oid| *oid != removed);
            e.state.players[0].graveyard.push(removed);
            e.state.objects.get_mut(&removed).expect("land").zone = Zone::Graveyard;
        }
        e.initial_response_batch();
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: losing the land removes the bonus",
            case.id
        );
    }
}

#[test]
fn q44_basic_land_pump_requires_the_printed_subtype() {
    let mut e = base_engine(44_072_001, &["kird_ape"]);
    let ape = put_card(&mut e, 0, "kird_ape");
    inject_permanent_on_battlefield(&mut e, 0, "island");
    assert_eq!(
        (e.effective_power(ape), e.effective_toughness(ape)),
        (Some(1), Some(1))
    );
    inject_permanent_on_battlefield(&mut e, 1, "forest");
    assert_eq!(
        (e.effective_power(ape), e.effective_toughness(ape)),
        (Some(1), Some(1)),
        "an opponent's Forest does not satisfy 'you control'"
    );
    inject_permanent_on_battlefield(&mut e, 0, "forest");
    assert_eq!(
        (e.effective_power(ape), e.effective_toughness(ape)),
        (Some(2), Some(3))
    );
    let mut e = base_engine(44_072_002, &["mire_kavu"]);
    let kavu = put_card(&mut e, 0, "mire_kavu");
    inject_permanent_on_battlefield(&mut e, 0, "forest");
    assert_eq!(
        (e.effective_power(kavu), e.effective_toughness(kavu)),
        (Some(3), Some(2))
    );
}

// ── 08 Haste end-step return ────────────────────────────────────────────────

struct HasteReturn {
    id: &'static str,
    body: (u32, u32),
    mana: ManaGift,
}

const HASTE_RETURN: [HasteReturn; 4] = [
    HasteReturn {
        id: "glitterfang",
        body: (1, 1),
        mana: ManaGift {
            w: 0,
            u: 0,
            b: 0,
            r: 1,
            g: 0,
            c: 0,
        },
    },
    HasteReturn {
        id: "viashino_cutthroat",
        body: (5, 3),
        mana: ManaGift {
            w: 0,
            u: 0,
            b: 0,
            r: 2,
            g: 0,
            c: 2,
        },
    },
    HasteReturn {
        id: "viashino_sandstalker",
        body: (4, 2),
        mana: ManaGift {
            w: 0,
            u: 0,
            b: 0,
            r: 2,
            g: 0,
            c: 1,
        },
    },
    HasteReturn {
        id: "viashino_sandscout",
        body: (2, 1),
        mana: ManaGift {
            w: 0,
            u: 0,
            b: 0,
            r: 1,
            g: 0,
            c: 1,
        },
    },
];

fn cast_creature(e: &mut GameEngine, card: &str, mana: ManaGift) -> u32 {
    inject_card_into_hand(e, 0, card);
    give_mana(e, 0, mana);
    let slot = hand_index_for_card(e, 0, card);
    e.apply_scenario_command(0, &cast_spell(slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {card}: {error:?}"));
    resolve_entire_stack_two_player(e);
    battlefield_object_for_card(e, 0, card)
}

#[test]
fn q44_haste_return_canary_returns_the_creature_at_the_end_step() {
    let mut e = base_engine(44_080_001, &[]);
    let creature = cast_creature(
        &mut e,
        "glitterfang",
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    assert!(e
        .characteristics(creature)
        .expect("creature")
        .keywords
        .contains(&Keyword::Haste));
    assert_eq!(
        (e.effective_power(creature), e.effective_toughness(creature)),
        (Some(1), Some(1))
    );
    end_turn_with_end_step_triggers(&mut e, 0);
    assert_eq!(
        e.state.objects[&creature].zone,
        Zone::Hand,
        "the end-step trigger returns it"
    );
    assert!(e.state.players[0].hand.contains(&creature));
}

#[test]
fn q44_haste_return_cards_repeat_their_printed_body_and_return() {
    for (index, case) in HASTE_RETURN.iter().enumerate() {
        let mut e = base_engine(44_081_000 + index as u64, &[]);
        let creature = cast_creature(&mut e, case.id, gift(&case.mana));
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.body.0), Some(case.body.1)),
            "{}: printed body",
            case.id
        );
        assert!(
            e.characteristics(creature)
                .expect("creature")
                .keywords
                .contains(&Keyword::Haste),
            "{}",
            case.id
        );
        end_turn_with_end_step_triggers(&mut e, 0);
        assert_eq!(e.state.objects[&creature].zone, Zone::Hand, "{}", case.id);
        assert!(e.state.players[0].hand.contains(&creature), "{}", case.id);
    }
}

#[test]
fn q44_haste_return_cards_do_not_return_from_the_graveyard() {
    let mut e = base_engine(44_082_001, &[]);
    let creature = cast_creature(
        &mut e,
        "glitterfang",
        ManaGift {
            r: 1,
            ..Default::default()
        },
    );
    e.state.players[0]
        .battlefield
        .retain(|oid| *oid != creature);
    e.state.players[0].graveyard.push(creature);
    e.state.objects.get_mut(&creature).expect("creature").zone = Zone::Graveyard;
    end_turn_with_end_step_triggers(&mut e, 0);
    assert_eq!(e.state.objects[&creature].zone, Zone::Graveyard);
    assert!(e.state.players[0].graveyard.contains(&creature));
}

#[test]
fn q44_haste_return_triggers_in_an_opponents_end_step() {
    let mut e = base_engine(44_083_001, &[]);
    end_active_turn(&mut e, 0);
    advance_to_main1_from_game_start(&mut e);
    assert_eq!(e.state.active_player_id(), 1);
    let creature = put_card(&mut e, 0, "glitterfang");
    assert_eq!(e.state.objects[&creature].zone, Zone::Battlefield);
    end_turn_with_end_step_triggers(&mut e, 1);
    assert_eq!(
        e.state.objects[&creature].zone,
        Zone::Hand,
        "the printed trigger is any end step, not only the controller's"
    );
    assert!(e.state.players[0].hand.contains(&creature));
}

// ── 09 Mirage fetch lands ───────────────────────────────────────────────────

struct Fetch {
    id: &'static str,
    allowed: [&'static str; 2],
    wrong: &'static str,
}

const FETCHES: [Fetch; 4] = [
    Fetch {
        id: "flood_plain",
        allowed: ["plains", "island"],
        wrong: "forest",
    },
    Fetch {
        id: "grasslands",
        allowed: ["forest", "plains"],
        wrong: "island",
    },
    Fetch {
        id: "mountain_valley",
        allowed: ["mountain", "forest"],
        wrong: "swamp",
    },
    Fetch {
        id: "rocky_tar_pit",
        allowed: ["swamp", "mountain"],
        wrong: "plains",
    },
];

#[test]
fn q44_fetch_canary_searches_an_allowed_subtype_untapped() {
    let mut e = base_engine(44_090_001, &[]);
    let land = inject_permanent_on_battlefield(&mut e, 0, "flood_plain");
    let tundra = inject_library_card(&mut e, 0, "tundra");
    let forest = inject_library_card(&mut e, 0, "forest");
    e.apply_scenario_command(0, &activate_ability_for(&e, land, 0, vec![]))
        .expect("activate the fetch");
    assert_eq!(
        e.state.objects[&land].zone,
        Zone::Graveyard,
        "sacrificed as a cost"
    );
    assert_eq!(pool(&e), (0, 0, 0, 0, 0, 0), "no mana was paid");
    e.apply_scenario_command(0, &pass()).expect("caster passes");
    let batch = e
        .apply_scenario_command(1, &pass())
        .expect("search resolves");
    let choice = find_resolution_choice(&batch).expect("land search choice");
    assert_eq!(choice.choice_kind(), ChoiceKind::LibrarySearch);
    assert_eq!((choice.min, choice.max), (0, 1));
    assert!(
        choice.candidate_object_ids.contains(&tundra),
        "a Plains Island card qualifies"
    );
    assert!(!choice.candidate_object_ids.contains(&forest));
    e.apply_scenario_command(0, &submit_resolution_choice(vec![tundra]))
        .expect("choose Tundra");
    assert_eq!(e.state.objects[&tundra].zone, Zone::Battlefield);
    assert!(
        !e.state.objects[&tundra].tapped,
        "the found land enters untapped"
    );
    assert_eq!(
        e.state.players[0]
            .battlefield
            .iter()
            .filter(|oid| **oid == tundra)
            .count(),
        1
    );
}

#[test]
fn q44_fetch_lands_search_their_printed_subtype_pairs() {
    for (index, case) in FETCHES.iter().enumerate() {
        let mut e = base_engine(44_091_000 + index as u64, &[]);
        let land = inject_permanent_on_battlefield(&mut e, 0, case.id);
        let first = inject_library_card(&mut e, 0, case.allowed[0]);
        let second = inject_library_card(&mut e, 0, case.allowed[1]);
        let wrong = inject_library_card(&mut e, 0, case.wrong);
        e.apply_scenario_command(0, &activate_ability_for(&e, land, 0, vec![]))
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
        e.apply_scenario_command(0, &pass()).expect("caster passes");
        let batch = e
            .apply_scenario_command(1, &pass())
            .expect("search resolves");
        let choice = find_resolution_choice(&batch).expect("land search choice");
        assert!(choice.candidate_object_ids.contains(&first), "{}", case.id);
        assert!(choice.candidate_object_ids.contains(&second), "{}", case.id);
        assert!(!choice.candidate_object_ids.contains(&wrong), "{}", case.id);
        e.apply_scenario_command(0, &submit_resolution_choice(vec![first]))
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
        assert_eq!(
            e.state.objects[&first].zone,
            Zone::Battlefield,
            "{}",
            case.id
        );
        assert!(!e.state.objects[&first].tapped, "{}: untapped", case.id);
        assert_eq!(e.state.objects[&land].zone, Zone::Graveyard, "{}", case.id);
    }
}

#[test]
fn q44_fetch_lands_enter_tapped_from_hand() {
    let mut e = base_engine(44_093_001, &[]);
    inject_card_into_hand(&mut e, 0, "flood_plain");
    let slot = hand_index_for_card(&e, 0, "flood_plain");
    e.apply_scenario_command(0, &play_land(slot))
        .expect("play Flood Plain");
    let land = battlefield_object_for_card(&e, 0, "flood_plain");
    assert!(e.state.objects[&land].tapped, "the land enters tapped");
}

#[test]
fn q44_fetch_lands_allow_declining_the_search() {
    let mut e = base_engine(44_092_001, &[]);
    let land = inject_permanent_on_battlefield(&mut e, 0, "flood_plain");
    let plains = inject_library_card(&mut e, 0, "plains");
    let library_before = e.state.players[0].library.len();
    e.apply_scenario_command(0, &activate_ability_for(&e, land, 0, vec![]))
        .expect("activate the fetch");
    e.apply_scenario_command(0, &pass()).expect("caster passes");
    let batch = e
        .apply_scenario_command(1, &pass())
        .expect("search resolves");
    let choice = find_resolution_choice(&batch).expect("land search choice");
    assert_eq!(choice.min, 0, "the player may fail to find");
    let declined = e
        .apply_scenario_command(0, &submit_resolution_choice(vec![]))
        .expect("fail to find");
    assert_eq!(
        e.state.objects[&plains].zone,
        Zone::Library,
        "no card moved"
    );
    assert_eq!(
        e.state.players[0].library.len(),
        library_before,
        "the library size is unchanged"
    );
    assert!(
        declined.events.iter().any(|event| matches!(
            &event.ev,
            Some(Ev::Log(log)) if log.text.contains("shuffles their library")
        )),
        "the search still shuffles after failing to find"
    );
    assert_eq!(e.state.objects[&land].zone, Zone::Graveyard);
}
