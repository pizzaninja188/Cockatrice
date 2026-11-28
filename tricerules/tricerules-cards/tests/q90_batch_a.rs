//! Registry conformance and actual-card evidence for the queue-v90 card data.
//!
//! Module 1 covers the ten "enters tapped unless you control a planeswalker" dual lands, module 2
//! the three "{T}, Tap an untapped creature you control: Add one mana of any color" creatures, and
//! module 3 the four keyword-plus-"{cost}, {T}: Tap target creature" creatures. Fingerprints are
//! asserted against the pinned bulk after the presentation metadata refresh.

use tricerules_cards::primitives::{
    AbilityCost, BattlefieldAggregate, EffectSubject, EntersTappedAffected, GameCondition, Keyword,
    ObjectPaymentConstraint, PermanentTypeFilter, RelativePlayerSet, SpellEffectKind,
    StaticAbilityDef, TargetKind,
};
use tricerules_cards::{AbilityPresentation, CardDefinition, CardFace, Layout};

fn face(definition: &CardDefinition) -> &CardFace {
    definition.primary_face()
}

fn mask(color: char) -> [u32; 6] {
    match color {
        'w' => [1, 0, 0, 0, 0, 0],
        'u' => [0, 1, 0, 0, 0, 0],
        'b' => [0, 0, 1, 0, 0, 0],
        'r' => [0, 0, 0, 1, 0, 0],
        'g' => [0, 0, 0, 0, 1, 0],
        _ => unreachable!("printed color"),
    }
}

fn amounts(amount: &tricerules_cards::ManaAmount) -> [u32; 6] {
    [amount.w, amount.u, amount.b, amount.r, amount.g, amount.c]
}

// ── 01 Planeswalker-conditional dual lands ───────────────────────────────

struct LandCase {
    id: &'static str,
    name: &'static str,
    first: char,
    second: char,
    fingerprint: &'static str,
}

const LANDS: &[LandCase] = &[
    LandCase {
        id: "theorix_annex",
        name: "Theorix Annex",
        first: 'u',
        second: 'b',
        fingerprint: "a01629f8b9b23430bc9a5aa7d5cf6eff513500adfd112203a93965d7412bc4a4",
    },
    LandCase {
        id: "stingerquill_annex",
        name: "Stingerquill Annex",
        first: 'b',
        second: 'r',
        fingerprint: "b69d714cedcb31f606ea9c78f212c8622077dbf793a28b07dd525131a8b372ce",
    },
    LandCase {
        id: "konstrari_annex",
        name: "Konstrari Annex",
        first: 'r',
        second: 'g',
        fingerprint: "c5f4bcac441fd0e014aa6cedf5fd52828dd10d484e4dc3599b2088467a25cc97",
    },
    LandCase {
        id: "formidable_commons",
        name: "Formidable Commons",
        first: 'b',
        second: 'g',
        fingerprint: "90e95ff2019ec138d39f70c02538f6a221bf1f464336229dcd392b2e0510964e",
    },
    LandCase {
        id: "innovative_commons",
        name: "Innovative Commons",
        first: 'u',
        second: 'r',
        fingerprint: "cc768b2fc776c59869ad0a6bc5d6571156ad85e984b5a1dc791d07031bec63b3",
    },
    LandCase {
        id: "meticulous_commons",
        name: "Meticulous Commons",
        first: 'w',
        second: 'b',
        fingerprint: "c3f14fe377f636f1be4e30401398d166a5d6bf18a175a38af6343c0c49404921",
    },
    LandCase {
        id: "transformative_commons",
        name: "Transformative Commons",
        first: 'g',
        second: 'u',
        fingerprint: "389f7485d371196896507c51bce594c41ee8c5b035c61f5bf28a0d6f9c527cfa",
    },
    LandCase {
        id: "fatehold_annex",
        name: "Fatehold Annex",
        first: 'w',
        second: 'u',
        fingerprint: "49b1532a87877fe389fd2379f29e79e5fff2e68f0e966c17f6e646fe2c9afd18",
    },
    LandCase {
        id: "dedicated_commons",
        name: "Dedicated Commons",
        first: 'r',
        second: 'w',
        fingerprint: "efd57fb9e145b1cb2cc54e8e2bee2e5a18dbf4692797bc857ce58cbfecb9a240",
    },
    LandCase {
        id: "vigorbloom_annex",
        name: "Vigorbloom Annex",
        first: 'g',
        second: 'w',
        fingerprint: "52c878fdb70760c0cf2c9eba9ef3fb5840f3bc171fe8657dc35cd9d470ae9099",
    },
];

#[test]
fn q90_planeswalker_untap_lands_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in LANDS {
        let registered = registry
            .get(case.id)
            .unwrap_or_else(|| panic!("missing {}", case.id));
        assert_eq!(registered.layout, Layout::Normal, "{}", case.id);
        assert_eq!(registered.name, case.name, "{}", case.id);
        assert_eq!(
            registry.id_for_name(case.name),
            Some(case.id),
            "{}",
            case.id
        );
        let f = face(registered);
        assert_eq!(f.face_id.as_str(), case.id, "{} face id", case.id);
        assert_eq!(f.mana_cost.to_string(), "", "{} mana cost", case.id);
        // Plain Land: this cycle has no basic land type, so a basic-land search cannot find it.
        assert_eq!(f.types, ["Land"], "{} types", case.id);
        assert!(f.supertypes.is_empty(), "{}", case.id);
        assert_eq!((f.power, f.toughness), (None, None), "{} body", case.id);
        assert!(f.keywords.is_empty(), "{}", case.id);
        assert!(f.triggered_abilities.is_empty(), "{}", case.id);
        assert!(f.spell_effect.is_empty(), "{}", case.id);
        assert!(f.targeting.is_none(), "{}", case.id);

        let [mana] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", case.id);
        };
        assert_eq!(
            mana.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{}",
            case.id
        );
        assert!(
            !mana.intrinsic_land_mana,
            "{}: no printed basic land type",
            case.id
        );
        assert!(mana.is_mana_ability(), "{}", case.id);
        assert_eq!(mana.costs, [AbilityCost::Tap], "{}", case.id);
        assert!(mana.targeting.is_none(), "{}", case.id);
        let [SpellEffectKind::ProduceMana { options, .. }] = mana.effect.as_slice() else {
            panic!("{}: produce mana", case.id);
        };
        assert_eq!(options.len(), 2, "{}: exactly two colours", case.id);
        assert_eq!(
            amounts(&options[0]),
            mask(case.first),
            "{} first option",
            case.id
        );
        assert_eq!(
            amounts(&options[1]),
            mask(case.second),
            "{} second option",
            case.id
        );

        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{}: exactly one static ability", case.id);
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{}",
            case.id
        );
        let StaticAbilityDef::EntersTapped {
            affected,
            condition,
            unless_cost,
        } = &static_ability.definition
        else {
            panic!("{}: EntersTapped", case.id);
        };
        assert_eq!(affected, &EntersTappedAffected::Self_, "{}", case.id);
        assert!(unless_cost.is_none(), "{}: no entry cost", case.id);
        let Some(GameCondition::BattlefieldAggregate {
            filter,
            aggregate,
            min,
            max,
            exclude_observed_object,
        }) = condition
        else {
            panic!("{}: conditional entry", case.id);
        };
        assert_eq!(*aggregate, BattlefieldAggregate::Count, "{}", case.id);
        // "unless you control a planeswalker": the replacement applies while the count is zero,
        // so the zero bound is the condition under which the replacement applies.
        assert_eq!(*min, None, "{}", case.id);
        assert_eq!(
            *max,
            Some(0),
            "{}: the replacement holds only at a zero planeswalker count",
            case.id
        );
        assert!(!exclude_observed_object, "{}", case.id);
        assert_eq!(
            filter.controllers,
            RelativePlayerSet::Controller,
            "{}",
            case.id
        );
        assert_eq!(
            filter.matching.permanent_types,
            [PermanentTypeFilter::Planeswalker],
            "{}",
            case.id
        );
        assert_eq!(
            filter.matching.kind,
            TargetKind::AnyPermanent,
            "{}",
            case.id
        );
        // The land is not on the battlefield yet, and its own card type is Land, so even without
        // `exclude_source` it could never satisfy a Planeswalker predicate. The default is kept:
        // `exclude_source` is only needed when the source could match its own filter.
        assert!(
            !filter.exclude_source,
            "{}: a Land cannot satisfy a Planeswalker filter",
            case.id
        );
    }
}

#[test]
fn q90_planeswalker_untap_lands_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for case in LANDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", case.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", case.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], case.id);
        assert_eq!(fields[1], case.name);
        assert_eq!(fields[2], case.id);
        assert_eq!(
            fields[4], case.fingerprint,
            "fingerprint drift for {}",
            case.id
        );
    }
}

// ── 02 Any-color mana from tapping a second creature ─────────────────────

struct ManaCase {
    id: &'static str,
    name: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    keywords: &'static [Keyword],
    fingerprint: &'static str,
}

const MANA_CREATURES: &[ManaCase] = &[
    ManaCase {
        id: "loam_dryad",
        name: "Loam Dryad",
        types: &["Creature", "Dryad", "Horror"],
        power: 1,
        toughness: 2,
        keywords: &[],
        fingerprint: "37391aca476431bfd3b1e2fcfe6162657f048ef7d3b7b08b7736aab746fda095",
    },
    ManaCase {
        id: "saruli_caretaker",
        name: "Saruli Caretaker",
        types: &["Creature", "Dryad"],
        power: 0,
        toughness: 3,
        keywords: &[Keyword::Defender],
        fingerprint: "3f61ad65bb887132dbf804dde6f8c51f4139138e4652f367ab90370bf7662de0",
    },
    ManaCase {
        id: "jaspera_sentinel",
        name: "Jaspera Sentinel",
        types: &["Creature", "Elf", "Rogue"],
        power: 1,
        toughness: 2,
        keywords: &[Keyword::Reach],
        fingerprint: "ccc03fc8d74fc212248f22fba57b3e6ce7bcd62e54a4da7ed99965a79c3e3c3a",
    },
];

#[test]
fn q90_any_color_tap_creature_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in MANA_CREATURES {
        let registered = registry
            .get(case.id)
            .unwrap_or_else(|| panic!("missing {}", case.id));
        assert_eq!(registered.layout, Layout::Normal, "{}", case.id);
        assert_eq!(registered.name, case.name, "{}", case.id);
        assert_eq!(
            registry.id_for_name(case.name),
            Some(case.id),
            "{}",
            case.id
        );
        let f = face(registered);
        assert_eq!(f.face_id.as_str(), case.id, "{} face id", case.id);
        assert_eq!(f.mana_cost.to_string(), "{G}", "{} mana cost", case.id);
        assert_eq!(f.types, case.types, "{} types", case.id);
        assert_eq!(
            (f.power, f.toughness),
            (Some(case.power), Some(case.toughness)),
            "{} body",
            case.id
        );
        assert_eq!(f.keywords, case.keywords, "{} keywords", case.id);
        // None of the three mana creatures is Snow; that case lives in the keyword-tap cohort.
        assert!(f.supertypes.is_empty(), "{} supertypes", case.id);
        assert!(f.triggered_abilities.is_empty(), "{}", case.id);
        assert!(f.static_abilities.is_empty(), "{}", case.id);
        assert!(f.spell_effect.is_empty(), "{}", case.id);
        assert!(f.targeting.is_none(), "{}", case.id);

        let [mana] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", case.id);
        };
        // A creature (or a creature with a keyword line above the ability) numbers the ability
        // line after its printed keyword lines.
        let expected_line = if case.keywords.is_empty() { 1 } else { 2 };
        assert_eq!(
            mana.presentation,
            AbilityPresentation::OracleLines(vec![expected_line]),
            "{}",
            case.id
        );
        assert!(!mana.intrinsic_land_mana, "{}", case.id);
        assert!(mana.is_mana_ability(), "{}", case.id);
        let [AbilityCost::Tap, AbilityCost::TapPermanents {
            constraint,
            filter,
            exclude_source,
        }] = mana.costs.as_slice()
        else {
            panic!(
                "{}: tap plus one tapped creature: {:?}",
                case.id, mana.costs
            );
        };
        assert_eq!(
            constraint,
            &ObjectPaymentConstraint::ExactCount(1),
            "{} exactly one creature",
            case.id
        );
        assert!(exclude_source, "{}: another creature", case.id);
        assert_eq!(filter.kind, TargetKind::Creature, "{}", case.id);
        assert!(
            filter.controller == tricerules_cards::primitives::TargetController::You,
            "{}: a creature you control",
            case.id
        );
        assert!(
            mana.targeting.is_none(),
            "{}: a cost is not a target",
            case.id
        );
        let [SpellEffectKind::ProduceMana { options, .. }] = mana.effect.as_slice() else {
            panic!("{}: produce mana", case.id);
        };
        assert_eq!(options.len(), 5, "{}: any colour", case.id);
        for (option, expected) in options.iter().zip(['w', 'u', 'b', 'r', 'g']) {
            assert_eq!(
                amounts(option),
                mask(expected),
                "{} option {}",
                case.id,
                expected
            );
        }
    }
}

// ── 03 Keyword plus "{cost}, {T}: Tap target creature." ──────────────────

struct TapCase {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    activation: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    keywords: &'static [Keyword],
    fingerprint: &'static str,
}

const TAP_CREATURES: &[TapCase] = &[
    TapCase {
        id: "mothrider_patrol",
        name: "Mothrider Patrol",
        mana: "{W}",
        activation: "{3}{W}",
        types: &["Creature", "Fox", "Warrior"],
        power: 1,
        toughness: 1,
        keywords: &[Keyword::Flying],
        fingerprint: "86fc34e5067a1402cfb57fe8ed3e0176acbc2aa6c28852240c1d013c97660795",
    },
    TapCase {
        id: "dazzling_ramparts",
        name: "Dazzling Ramparts",
        mana: "{4}{W}",
        activation: "{1}{W}",
        types: &["Creature", "Wall"],
        power: 0,
        toughness: 7,
        keywords: &[Keyword::Defender],
        fingerprint: "ea44ce67192baa033f3ffd21f94ceddaf839b57113643d7e440daac4bc83c113",
    },
    TapCase {
        id: "squall_drifter",
        name: "Squall Drifter",
        mana: "{1}{W}",
        activation: "{W}",
        types: &["Creature", "Elemental"],
        power: 1,
        toughness: 1,
        keywords: &[Keyword::Flying],
        fingerprint: "bebf171ebde845951d0d88737f41e7be3fedd920b704aae4e942948fb6e5f686",
    },
    TapCase {
        id: "clockwork_drawbridge",
        name: "Clockwork Drawbridge",
        mana: "{W}",
        activation: "{2}{W}",
        types: &["Artifact", "Creature", "Wall"],
        power: 0,
        toughness: 3,
        keywords: &[Keyword::Defender],
        fingerprint: "a6aba27f4cbba933dd4185646c3b51ba2bc1a6d5adb0cb9a7782bcdf698ed495",
    },
];

#[test]
fn q90_keyword_tap_target_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in TAP_CREATURES {
        let registered = registry
            .get(case.id)
            .unwrap_or_else(|| panic!("missing {}", case.id));
        assert_eq!(registered.layout, Layout::Normal, "{}", case.id);
        assert_eq!(registered.name, case.name, "{}", case.id);
        assert_eq!(
            registry.id_for_name(case.name),
            Some(case.id),
            "{}",
            case.id
        );
        let f = face(registered);
        assert_eq!(f.face_id.as_str(), case.id, "{} face id", case.id);
        assert_eq!(f.mana_cost.to_string(), case.mana, "{} mana cost", case.id);
        assert_eq!(f.types, case.types, "{} types", case.id);
        assert_eq!(
            (f.power, f.toughness),
            (Some(case.power), Some(case.toughness)),
            "{} body",
            case.id
        );
        assert_eq!(f.keywords, case.keywords, "{} keywords", case.id);
        // Squall Drifter is the only printed Snow card across both cohorts.
        assert_eq!(
            f.supertypes,
            if case.id == "squall_drifter" {
                vec!["Snow".to_string()]
            } else {
                Vec::new()
            },
            "{} supertypes",
            case.id
        );
        assert!(f.triggered_abilities.is_empty(), "{}", case.id);
        assert!(f.static_abilities.is_empty(), "{}", case.id);
        assert!(f.spell_effect.is_empty(), "{}", case.id);
        assert!(f.targeting.is_none(), "{}", case.id);

        let [tap] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", case.id);
        };
        // Both printed keyword lines here are single lines, so the ability is Oracle line 2.
        assert_eq!(
            tap.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{}",
            case.id
        );
        assert!(!tap.intrinsic_land_mana, "{}", case.id);
        assert!(!tap.is_mana_ability(), "{}: not a mana ability", case.id);
        let [AbilityCost::Mana(printed), AbilityCost::Tap] = tap.costs.as_slice() else {
            panic!("{}: mana plus tap: {:?}", case.id, tap.costs);
        };
        assert_eq!(
            printed.to_string(),
            case.activation,
            "{} printed activation cost",
            case.id
        );
        assert!(tap.targeting.is_none(), "{}", case.id);
        let [SpellEffectKind::Tap {
            subject: EffectSubject::Chosen(filter),
        }] = tap.effect.as_slice()
        else {
            panic!("{}: taps a chosen creature: {:?}", case.id, tap.effect);
        };
        assert_eq!(filter.kind, TargetKind::Creature, "{}", case.id);
        // "Tap target creature" is unrestricted by controller: it may tap either player's creature.
        assert_eq!(
            filter.controller,
            tricerules_cards::primitives::TargetController::Any,
            "{}: either player's creature",
            case.id
        );
        // CR 115.1c: a cost is not a target, so the mana and tap payments are not in the filter.
        assert!(
            filter.tapped.is_none(),
            "{}: any creature, tapped or untapped",
            case.id
        );
    }
}

#[test]
fn q90_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    let cases = MANA_CREATURES
        .iter()
        .map(|case| (case.id, case.name, case.id, case.fingerprint))
        .chain(
            TAP_CREATURES
                .iter()
                .map(|case| (case.id, case.name, case.id, case.fingerprint)),
        );
    for (id, name, face_id, fingerprint) in cases {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], face_id);
        assert_eq!(fields[4], fingerprint, "fingerprint drift for {}", id);
    }
}
