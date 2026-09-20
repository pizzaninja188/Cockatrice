//! Issue #455 registry and presentation conformance for the ETB opponent-pump family cohort.
//!
//! Burrog Befuddler, Ambush Gigapede, and Sinister Cryologist each print exactly
//! `When this creature enters, target creature an opponent controls gets <power>/<toughness> until
//! end of turn.` in the pinned Scryfall snapshot `27bf3214-1271-490b-bdfe-c0be6c23d02e`. Exact
//! Scryfall records and `rulings_uri` responses were fetched 2026-09-20; none returned a ruling.
//! The expectations below are the reviewed printed Oracle behavior and the shipped typed
//! vocabulary, not a copy of generator output. CR 603.6a (entry triggers), CR 115.1 (targets and
//! controller scope), CR 613.4c (power/toughness modifying effects), and CR 702.185 (Warp) govern
//! the asserted shapes.

mod common;

use common::FaceExpectation;
use tricerules_cards::primitives::{
    EffectSubject, SpellEffectKind, TargetController, TargetFilter, TargetKind,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Keyword, ManaCost, TriggerCondition};

struct PumpCase {
    id: &'static str,
    name: &'static str,
    face_id: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power_toughness: (u32, u32),
    keywords: &'static [Keyword],
    warp_cost: Option<&'static str>,
    presentation_line: u16,
    power: i32,
    toughness: i32,
}

const PUMP_CASES: [PumpCase; 3] = [
    PumpCase {
        id: "burrog_befuddler",
        name: "Burrog Befuddler",
        face_id: "burrog_befuddler",
        mana_cost: "{1}{U}",
        types: &["Creature", "Frog", "Wizard"],
        power_toughness: (2, 1),
        keywords: &[Keyword::Flash],
        warp_cost: None,
        presentation_line: 2,
        power: -1,
        toughness: 0,
    },
    PumpCase {
        id: "ambush_gigapede",
        name: "Ambush Gigapede",
        face_id: "ambush_gigapede",
        mana_cost: "{4}{B}{B}",
        types: &["Creature", "Insect"],
        power_toughness: (6, 2),
        keywords: &[Keyword::Flash],
        warp_cost: None,
        presentation_line: 2,
        power: -2,
        toughness: -2,
    },
    PumpCase {
        id: "sinister_cryologist",
        name: "Sinister Cryologist",
        face_id: "sinister_cryologist",
        mana_cost: "{2}{U}",
        types: &["Creature", "Jellyfish", "Wizard"],
        power_toughness: (2, 3),
        keywords: &[],
        warp_cost: Some("{U}"),
        presentation_line: 1,
        power: -3,
        toughness: 0,
    },
];

#[test]
fn issue_455_registers_the_three_reviewed_identities() {
    let registry = CardRegistry::global();
    for case in &PUMP_CASES {
        assert_eq!(
            registry.id_for_name(case.name),
            Some(case.id),
            "{}",
            case.id
        );
        FaceExpectation {
            id: case.id,
            name: case.name,
            face_id: case.face_id,
            mana_cost: case.mana_cost,
            types: case.types,
            keywords: case.keywords,
            power_toughness: Some(case.power_toughness),
        }
        .check();
    }
}

#[test]
fn issue_455_sinister_cryologist_has_a_blue_warp_cost() {
    for case in &PUMP_CASES {
        let face = CardRegistry::global()
            .get(case.id)
            .unwrap_or_else(|| panic!("missing reviewed card {}", case.id))
            .primary_face();
        let expected = case
            .warp_cost
            .map(|cost| ManaCost::parse(cost).expect("reviewed Warp cost"));
        assert_eq!(face.warp_cost, expected, "{} printed Warp cost", case.id);
        assert!(face.flashback_cost.is_none(), "{}", case.id);
    }
    let face = CardRegistry::global()
        .get("sinister_cryologist")
        .expect("Sinister Cryologist")
        .primary_face();
    assert!(
        face.keywords.is_empty(),
        "Warp is carried by the warp cost field, not the keyword list"
    );
}

#[test]
fn issue_455_each_identity_prints_the_typed_opponent_pump_trigger() {
    for case in &PUMP_CASES {
        let face = CardRegistry::global()
            .get(case.id)
            .unwrap_or_else(|| panic!("missing reviewed card {}", case.id))
            .primary_face();
        assert!(
            face.spell_effect.is_empty(),
            "{} is a permanent, not a spell",
            case.id
        );
        let [ability] = face.triggered_abilities.as_slice() else {
            panic!("{} must have exactly one triggered ability", case.id);
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01", "{}", case.id);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![case.presentation_line]),
            "{}",
            case.id
        );
        assert_eq!(
            ability.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{}",
            case.id
        );
        assert!(!ability.may, "{}", case.id);
        assert!(ability.intervening_if.is_none(), "{}", case.id);

        let [SpellEffectKind::PumpTarget {
            power,
            toughness,
            scale,
            subject,
        }] = ability.effect.as_slice()
        else {
            panic!("{} must print exactly one typed pump", case.id);
        };
        assert_eq!(
            (*power, *toughness),
            (case.power, case.toughness),
            "{}",
            case.id
        );
        assert_eq!(*scale, None, "{}", case.id);
        let EffectSubject::Chosen(filter) = subject else {
            panic!("{} pumps a chosen target", case.id);
        };
        assert_eq!(filter.kind, TargetKind::Creature, "{}", case.id);
        assert_eq!(filter.controller, TargetController::Opponent, "{}", case.id);
        assert_eq!(
            **filter,
            TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::Opponent,
                ..TargetFilter::default()
            },
            "{} keeps only the opponent-controller creature scope",
            case.id
        );

        let targeting = ability
            .targeting
            .as_ref()
            .unwrap_or_else(|| panic!("{} must target", case.id));
        let [group] = targeting.groups.as_slice() else {
            panic!("{} must have exactly one target group", case.id);
        };
        assert_eq!((group.min, group.max), (1, 1), "{}", case.id);
        assert_eq!(
            group.prompt, "Choose target creature an opponent controls",
            "{}",
            case.id
        );
        assert_eq!(group.effect_indices, vec![0], "{}", case.id);
    }
}

#[test]
fn issue_455_shipped_minus_two_zero_identities_are_unchanged() {
    for id in ["cogwork_wrestler", "humbling_elder"] {
        let face = CardRegistry::global()
            .get(id)
            .unwrap_or_else(|| panic!("missing shipped {id}"))
            .primary_face();
        let [SpellEffectKind::PumpTarget {
            power,
            toughness,
            subject,
            ..
        }] = face.triggered_abilities[0].effect.as_slice()
        else {
            panic!("{id} must keep its typed pump");
        };
        assert_eq!((*power, *toughness), (-2, 0), "{id}");
        let EffectSubject::Chosen(filter) = subject else {
            panic!("{id} pumps a chosen target");
        };
        assert_eq!(filter.controller, TargetController::Opponent, "{id}");
    }
}
