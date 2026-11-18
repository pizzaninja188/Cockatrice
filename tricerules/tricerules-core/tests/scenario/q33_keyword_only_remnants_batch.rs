//! Actual-card scenarios for the queue-v33 printed-keyword-only cohort.
//!
//! Expectations are printed Oracle values. Each card has no rules text beyond
//! its printed keyword; Defender and Vigilance have observable combat effects.
use crate::helpers::ScenarioCommandExt;

use super::helpers::*;
use tricerules_cards::Keyword;
use tricerules_core::TurnStep;

struct Case {
    id: &'static str,
    power: u32,
    toughness: u32,
    keywords: &'static [Keyword],
}

const CASES: [Case; 7] = [
    Case {
        id: "alaborn_musketeer",
        power: 2,
        toughness: 1,
        keywords: &[Keyword::Reach],
    },
    Case {
        id: "kolinahr_priest",
        power: 3,
        toughness: 2,
        keywords: &[Keyword::Flash],
    },
    Case {
        id: "mekleth_berserker",
        power: 4,
        toughness: 3,
        keywords: &[Keyword::Menace],
    },
    Case {
        id: "sickbay_orderly",
        power: 3,
        toughness: 2,
        keywords: &[Keyword::Lifelink],
    },
    Case {
        id: "steel_wall",
        power: 0,
        toughness: 4,
        keywords: &[Keyword::Defender],
    },
    Case {
        id: "talarian_hook_spider",
        power: 2,
        toughness: 2,
        keywords: &[Keyword::Reach],
    },
    Case {
        id: "worf,_chief_tactical_officer",
        power: 2,
        toughness: 3,
        keywords: &[Keyword::Vigilance],
    },
];

fn engine(seed: u64, card: &str) -> GameEngine {
    let decks = Some(vec![
        deck_with("forest", &[card]),
        deck_with("forest", &["grizzly_bears"]),
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

fn advance_to_declare_attackers(e: &mut GameEngine) {
    e.apply_scenario_command(0, &primitive_yield())
        .expect("main phase to beginning of combat");
    e.apply_scenario_command(0, &pass())
        .expect("active player passes in beginning of combat");
    e.apply_scenario_command(1, &pass())
        .expect("defender passes in beginning of combat");
    assert_eq!(e.state.turn_step, TurnStep::DeclareAttackers);
}

#[test]
fn q33_keyword_only_remnants_canary_simplest() {
    let case = &CASES[5];
    let mut e = engine(33_090_001, case.id);
    let creature = move_ready_to_battlefield(&mut e, 0, case.id);
    assert_eq!(
        (e.effective_power(creature), e.effective_toughness(creature)),
        (Some(case.power), Some(case.toughness))
    );
    assert!(e.effective_has_keyword(creature, Keyword::Reach));
}

#[test]
fn q33_keyword_only_remnants_canary_hardest() {
    let case = &CASES[4];
    let mut e = engine(33_090_002, case.id);
    let creature = move_ready_to_battlefield(&mut e, 0, case.id);
    assert!(e.effective_has_keyword(creature, Keyword::Defender));
    let _decoy = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
    advance_to_declare_attackers(&mut e);
    assert!(
        e.apply_scenario_command(0, &declare_attackers(vec![creature]))
            .is_err(),
        "a defender cannot attack"
    );
}

#[test]
fn q33_keyword_only_remnants_cards_enter_with_printed_bodies_and_keywords() {
    for (index, case) in CASES.iter().enumerate() {
        let mut e = engine(33_091_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, case.id);
        assert_eq!(
            (e.effective_power(creature), e.effective_toughness(creature)),
            (Some(case.power), Some(case.toughness)),
            "{}: printed body",
            case.id
        );
        for keyword in case.keywords {
            assert!(
                e.effective_has_keyword(creature, *keyword),
                "{}: {keyword:?}",
                case.id
            );
        }
    }
}

#[test]
fn q33_keyword_only_remnants_vigilance_creatures_do_not_tap_to_attack() {
    let case = &CASES[6];
    let mut e = engine(33_092_000, case.id);
    let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
    advance_to_declare_attackers(&mut e);
    e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
        .expect("declare attacker");
    assert!(!e.state.objects[&attacker].tapped, "vigilance does not tap");
}
