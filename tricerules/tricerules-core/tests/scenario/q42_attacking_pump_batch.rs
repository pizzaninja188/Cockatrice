//! Actual-card scenarios for the queue-v42 attacking-creature pump spell cohort.
//!
//! Expectations are printed Oracle values. CR 508.1k and 611.2c make the affected set the
//! creatures attacking as the spell resolves; CR 613.4c applies the layer-7c pump and CR 613.1f
//! the layer-6 keyword grant. Nonattacking creatures are unaffected.
use super::helpers::*;
use tricerules_cards::primitives::Keyword;

struct Case {
    id: &'static str,
    mana: ManaGift,
    pump: (i32, i32),
    keyword: Option<Keyword>,
}

fn cases() -> [Case; 5] {
    [
        Case {
            id: "army_of_allah",
            mana: ManaGift {
                w: 2,
                c: 1,
                ..Default::default()
            },
            pump: (2, 0),
            keyword: None,
        },
        Case {
            id: "morale",
            mana: ManaGift {
                w: 2,
                c: 1,
                ..Default::default()
            },
            pump: (1, 1),
            keyword: None,
        },
        Case {
            id: "rally_the_forces",
            mana: ManaGift {
                r: 1,
                c: 2,
                ..Default::default()
            },
            pump: (1, 0),
            keyword: Some(Keyword::FirstStrike),
        },
        Case {
            id: "stampede",
            mana: ManaGift {
                g: 2,
                c: 1,
                ..Default::default()
            },
            pump: (1, 0),
            keyword: Some(Keyword::Trample),
        },
        Case {
            id: "volcanic_rush",
            mana: ManaGift {
                r: 1,
                c: 4,
                ..Default::default()
            },
            pump: (2, 0),
            keyword: Some(Keyword::Trample),
        },
    ]
}

fn engine(seed: u64) -> GameEngine {
    let decks = Some(vec![deck_with("mountain", &[]), deck_with("forest", &[])]);
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        seed,
        &[0, 1],
        20,
        decks,
        true,
    )
    .expect("engine");
    advance_to_declare_attackers(&mut e);
    e
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

fn pool(e: &GameEngine) -> (u32, u32, u32, u32, u32, u32) {
    let p = &e.state.players[0].mana_pool;
    (p.white, p.blue, p.black, p.red, p.green, p.colorless)
}

fn cast_pump(e: &mut GameEngine, case: &Case, attacker: u32, nonattacker: u32, opponent: u32) {
    give_mana(e, 0, gift(&case.mana));
    let slot = inject_card_into_hand(e, 0, case.id);
    let hand_slot = e.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == slot)
        .expect("the injected card is in hand");
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .unwrap_or_else(|error| panic!("cast {}: {error}", case.id));
    assert_eq!(pool(e), (0, 0, 0, 0, 0, 0), "{}: exact cast cost", case.id);
    resolve_entire_stack_two_player(e);
    assert_eq!(
        e.effective_power(attacker),
        Some((2 + case.pump.0) as u32),
        "{}: attacking creature is pumped",
        case.id
    );
    assert_eq!(
        e.effective_toughness(attacker),
        Some((2 + case.pump.1) as u32),
        "{}: attacking creature pump toughness",
        case.id
    );
    assert_eq!(
        (
            e.effective_power(nonattacker),
            e.effective_toughness(nonattacker)
        ),
        (Some(2), Some(2)),
        "{}: a nonattacking creature is unaffected",
        case.id
    );
    assert_eq!(
        (e.effective_power(opponent), e.effective_toughness(opponent)),
        (Some(2), Some(2)),
        "{}: an opponent's creature is unaffected",
        case.id
    );
    match case.keyword {
        Some(keyword) => assert!(
            e.effective_has_keyword(attacker, keyword),
            "{}: keyword granted to the attacker",
            case.id
        ),
        None => assert!(
            !e.effective_has_keyword(attacker, Keyword::FirstStrike)
                && !e.effective_has_keyword(attacker, Keyword::Trample),
            "{}: no keyword is granted",
            case.id
        ),
    }
}

#[test]
fn q42_attacking_pump_canary_simplest_pumps_only_the_attacker() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "army_of_allah")
        .expect("Army of Allah case");
    let mut e = engine(42_080_001);
    let attacker = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
    let nonattacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare the attacker");
    cast_pump(&mut e, &case, attacker, nonattacker, opponent);
}

#[test]
fn q42_attacking_pump_canary_hardest_grants_trample() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "volcanic_rush")
        .expect("Volcanic Rush case");
    let mut e = engine(42_080_002);
    let attacker = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
    let nonattacker = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare the attacker");
    cast_pump(&mut e, &case, attacker, nonattacker, opponent);
    assert!(
        !e.effective_has_keyword(nonattacker, Keyword::Trample),
        "the trample grant is attacking-only"
    );
}

#[test]
fn q42_attacking_pump_cards_apply_their_printed_pump_and_keyword() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(42_081_000 + index as u64);
        let attacker = inject_creature_on_battlefield(&mut e, 0, "savannah_lions");
        let second = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        let nonattacker = inject_creature_on_battlefield(&mut e, 0, "coral_merfolk");
        let opponent = inject_creature_on_battlefield(&mut e, 1, "grizzly_bears");
        e.apply_scenario_command(0, &declare_attackers(vec![attacker, second]))
            .expect("declare two attackers");
        give_mana(&mut e, 0, case.mana);
        let slot = inject_card_into_hand(&mut e, 0, case.id);
        let hand_slot = e.state.players[0]
            .hand
            .iter()
            .position(|oid| *oid == slot)
            .expect("the injected card is in hand");
        e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
            .unwrap_or_else(|error| panic!("cast {}: {error}", case.id));
        resolve_entire_stack_two_player(&mut e);
        for (label, creature) in [("first", attacker), ("second", second)] {
            assert_eq!(
                e.effective_power(creature),
                Some((2 + case.pump.0) as u32),
                "{}: {label} attacker power",
                case.id
            );
            assert_eq!(
                e.effective_toughness(creature),
                Some((2 + case.pump.1) as u32),
                "{}: {label} attacker toughness",
                case.id
            );
            if let Some(keyword) = case.keyword {
                assert!(
                    e.effective_has_keyword(creature, keyword),
                    "{}: {label} attacker keyword",
                    case.id
                );
            }
        }
        assert_eq!(
            (
                e.effective_power(nonattacker),
                e.effective_toughness(nonattacker)
            ),
            (Some(2), Some(2)),
            "{}: nonattacking creature unchanged",
            case.id
        );
        assert_eq!(
            (e.effective_power(opponent), e.effective_toughness(opponent)),
            (Some(2), Some(2)),
            "{}: opponent creature unchanged",
            case.id
        );
    }
}

#[test]
fn q42_attacking_pump_cards_do_nothing_when_no_creature_is_attacking() {
    let case = cases()
        .into_iter()
        .find(|case| case.id == "morale")
        .expect("Morale case");
    let mut e = GameEngine::new(
        tricerules_cards::registry::global(),
        42_082_001,
        &[0, 1],
        20,
        Some(vec![deck_with("plains", &[]), deck_with("forest", &[])]),
        true,
    )
    .expect("engine");
    advance_to_main1_from_game_start(&mut e);
    let bystander = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    give_mana(&mut e, 0, case.mana);
    let slot = inject_card_into_hand(&mut e, 0, case.id);
    let hand_slot = e.state.players[0]
        .hand
        .iter()
        .position(|oid| *oid == slot)
        .expect("the injected card is in hand");
    e.apply_scenario_command(0, &cast_spell(hand_slot, vec![]))
        .expect("cast with no attackers");
    resolve_entire_stack_two_player(&mut e);
    assert_eq!(
        (
            e.effective_power(bystander),
            e.effective_toughness(bystander)
        ),
        (Some(2), Some(2)),
        "the spell has no effect outside combat"
    );
}
