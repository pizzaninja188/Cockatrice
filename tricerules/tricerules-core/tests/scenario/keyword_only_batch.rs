//! Actual-card scenarios for the queue-v28 keyword-only creature cohort.
//!
//! Expectations are printed Oracle values. Each card is moved onto the battlefield through
//! the engine, its printed body and keyword set are asserted, and the keywords with an
//! observable combat consequence (flying, vigilance, defender, haste) are exercised.
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

fn cases() -> [Case; 10] {
    [
        Case {
            id: "thunder_spirit",
            power: 2,
            toughness: 2,
            keywords: &[Keyword::Flying, Keyword::FirstStrike],
        },
        Case {
            id: "sabertooth_wyvern",
            power: 3,
            toughness: 2,
            keywords: &[Keyword::Flying, Keyword::FirstStrike],
        },
        Case {
            id: "carnivorous_plant",
            power: 4,
            toughness: 5,
            keywords: &[Keyword::Defender],
        },
        Case {
            id: "ambush_party",
            power: 3,
            toughness: 1,
            keywords: &[Keyword::FirstStrike, Keyword::Haste],
        },
        Case {
            id: "raging_minotaur",
            power: 3,
            toughness: 3,
            keywords: &[Keyword::Haste],
        },
        Case {
            id: "brimstone_dragon",
            power: 6,
            toughness: 6,
            keywords: &[Keyword::Flying, Keyword::Haste],
        },
        Case {
            id: "windseeker_centaur",
            power: 2,
            toughness: 2,
            keywords: &[Keyword::Vigilance],
        },
        Case {
            id: "angel_of_light",
            power: 3,
            toughness: 3,
            keywords: &[Keyword::Flying, Keyword::Vigilance],
        },
        Case {
            id: "mugato",
            power: 6,
            toughness: 6,
            keywords: &[Keyword::Reach, Keyword::Vigilance, Keyword::Deathtouch],
        },
        Case {
            id: "blor_the_impervious",
            power: 3,
            toughness: 3,
            keywords: &[Keyword::Trample, Keyword::Hexproof, Keyword::Indestructible],
        },
    ]
}

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
fn keyword_only_batch_cards_enter_with_printed_bodies_and_keywords() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(28_010_000 + index as u64, case.id);
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
fn keyword_only_batch_flying_cards_cannot_be_blocked_by_ground_creatures() {
    for (index, case) in cases().into_iter().enumerate() {
        if !case.keywords.contains(&Keyword::Flying) {
            continue;
        }
        let mut e = engine(28_011_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        let ground = move_ready_to_battlefield(&mut e, 1, "grizzly_bears");
        let flyer = inject_creature_on_battlefield(&mut e, 1, "duskrider_falcon");
        advance_to_declare_attackers(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare attacker");
        e.apply_scenario_command(0, &pass())
            .expect("active player passes");
        e.apply_scenario_command(1, &pass())
            .expect("defender passes");
        assert_eq!(e.state.turn_step, TurnStep::DeclareBlockers);
        let pairs = &e.initial_response_batch().legal_by_player[&1].legal_block_pairs;
        assert!(
            !pairs.iter().any(|pair| pair.blocker_id == ground),
            "{}: a ground creature cannot block",
            case.id
        );
        assert!(
            pairs.iter().any(|pair| pair.blocker_id == flyer),
            "{}: a flying creature can block",
            case.id
        );
    }
}

#[test]
fn keyword_only_batch_defender_cannot_attack_and_haste_can_attack_immediately() {
    for (index, case) in cases().into_iter().enumerate() {
        let mut e = engine(28_012_000 + index as u64, case.id);
        let creature = move_ready_to_battlefield(&mut e, 0, case.id);
        // A legal attacker keeps beginning-of-combat from skipping straight to EndCombat.
        let _decoy = inject_creature_on_battlefield(&mut e, 0, "grizzly_bears");
        e.state
            .objects
            .get_mut(&creature)
            .expect("creature")
            .summoning_sick = true;
        advance_to_declare_attackers(&mut e);
        let result = e.apply_scenario_command(0, &declare_attackers(vec![creature]));
        if case.keywords.contains(&Keyword::Defender) {
            assert!(
                result.is_err(),
                "{}: a defender cannot be declared as an attacker",
                case.id
            );
        } else if case.keywords.contains(&Keyword::Haste) {
            result.unwrap_or_else(|error| {
                panic!("{}: haste ignores summoning sickness: {error:?}", case.id)
            });
        }
    }
}

#[test]
fn keyword_only_batch_vigilance_creatures_do_not_tap_to_attack() {
    for (index, case) in cases().into_iter().enumerate() {
        if !case.keywords.contains(&Keyword::Vigilance) {
            continue;
        }
        let mut e = engine(28_013_000 + index as u64, case.id);
        let attacker = move_ready_to_battlefield(&mut e, 0, case.id);
        advance_to_declare_attackers(&mut e);
        e.apply_scenario_command(0, &declare_attackers(vec![attacker]))
            .expect("declare attacker");
        assert!(
            !e.state.objects[&attacker].tapped,
            "{}: vigilance does not tap to attack",
            case.id
        );
    }
}
