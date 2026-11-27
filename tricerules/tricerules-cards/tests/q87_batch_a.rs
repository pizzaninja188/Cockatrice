//! Registry conformance for the queue-v87 batch A card data.
//!
//! Module 1 covers the three Rebel subtype tutors, module 2 the four Vampires with the
//! combat-damage +1/+1 counter trigger, and module 3 the three Nim Zombies that scale with the
//! controller's artifacts. Fingerprints are asserted against the pinned bulk after the
//! presentation metadata refresh.

use tricerules_cards::primitives::{
    AbilityCost, Amount, BattlefieldPermanentFilter, CardTypeFilter, CountExpression,
    EffectSubject, Keyword, RelativePlayerSet, SearchDestination, SpellEffectKind,
    StaticAbilityDef, TargetKind, TriggerCondition,
};
use tricerules_cards::{
    AbilityPresentation, CardDefinition, CounterKind, Layout, PermanentTypeFilter,
};

fn definition(id: &str) -> &'static CardDefinition {
    tricerules_cards::registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
}

fn mana_cost(id: &str) -> String {
    let costs = &definition(id).primary_face().activated_abilities[0].costs;
    match costs.first() {
        Some(AbilityCost::Mana(cost)) => cost.to_string(),
        other => panic!("{id}: expected a mana cost first, got {other:?}"),
    }
}

// ── 01 Rebel subtype tutors ───────────────────────────────────────────────

struct RebelCase {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    body: (u32, u32),
    keyword: Keyword,
    activation: &'static str,
    max_mana_value: u32,
    line: u16,
}

fn rebel_cases() -> [RebelCase; 3] {
    [
        RebelCase {
            id: "defiant_falcon",
            name: "Defiant Falcon",
            mana: "{1}{W}",
            types: &["Creature", "Rebel", "Bird"],
            body: (1, 1),
            keyword: Keyword::Flying,
            activation: "{4}",
            max_mana_value: 3,
            line: 2,
        },
        RebelCase {
            id: "ramosian_captain",
            name: "Ramosian Captain",
            mana: "{1}{W}{W}",
            types: &["Creature", "Human", "Rebel"],
            body: (2, 2),
            keyword: Keyword::FirstStrike,
            activation: "{5}",
            max_mana_value: 4,
            line: 2,
        },
        RebelCase {
            id: "ramosian_sky_marshal",
            name: "Ramosian Sky Marshal",
            mana: "{3}{W}{W}",
            types: &["Creature", "Human", "Rebel"],
            body: (3, 3),
            keyword: Keyword::Flying,
            activation: "{7}",
            max_mana_value: 6,
            line: 2,
        },
    ]
}

#[test]
fn q87_rebel_tutor_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in rebel_cases() {
        let definition = registry
            .get(case.id)
            .unwrap_or_else(|| panic!("missing {}", case.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", case.id);
        assert_eq!(definition.name, case.name, "{}", case.id);
        assert_eq!(
            registry.id_for_name(case.name),
            Some(case.id),
            "{}",
            case.id
        );
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), case.id, "{} face id", case.id);
        assert_eq!(face.mana_cost.to_string(), case.mana, "{} mana", case.id);
        assert_eq!(face.types, case.types, "{} types", case.id);
        assert_eq!(face.power, Some(case.body.0), "{} power", case.id);
        assert_eq!(face.toughness, Some(case.body.1), "{} toughness", case.id);
        assert_eq!(face.keywords, [case.keyword], "{} keyword", case.id);
        assert!(
            face.triggered_abilities.is_empty()
                && face.static_abilities.is_empty()
                && face.spell_effect.is_empty(),
            "{}: only the activated tutor remains",
            case.id
        );

        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", case.id);
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![case.line]),
            "{} tutor presentation",
            case.id
        );
        assert_eq!(mana_cost(case.id), case.activation, "{} cost", case.id);
        assert_eq!(
            ability.costs[1],
            AbilityCost::Tap,
            "{}: the tap cost",
            case.id
        );
        let [effect] = ability.effect.as_slice() else {
            panic!("{}: exactly one effect", case.id);
        };
        let SpellEffectKind::SearchLibrary {
            count,
            filter,
            destination,
            shuffle,
            ..
        } = effect
        else {
            panic!("{}: printed library search, got {effect:?}", case.id);
        };
        assert_eq!(*count, 1, "{}: one card", case.id);
        assert_eq!(
            *destination,
            SearchDestination::Battlefield { tapped: false },
            "{}: enters untapped",
            case.id
        );
        assert!(*shuffle, "{}: the printed shuffle", case.id);
        let filter = filter
            .as_ref()
            .unwrap_or_else(|| panic!("{} filter", case.id));
        assert_eq!(
            filter.card_type,
            Some(CardTypeFilter::NonlandPermanent),
            "{}: a Rebel permanent card",
            case.id
        );
        assert_eq!(
            filter.required_subtypes,
            vec!["Rebel".to_string()],
            "{}",
            case.id
        );
        assert_eq!(
            filter.max_mana_value,
            Some(case.max_mana_value),
            "{}: printed cap",
            case.id
        );
    }
}

// ── 02 combat-damage +1/+1 counter ────────────────────────────────────────

struct CounterCase {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    body: (u32, u32),
    keywords: &'static [Keyword],
    must_attack: bool,
    has_madness: bool,
    madness_cost: &'static str,
    line: u16,
}

fn counter_cases() -> [CounterCase; 4] {
    [
        CounterCase {
            id: "bloodcrazed_neonate",
            name: "Bloodcrazed Neonate",
            mana: "{1}{R}",
            types: &["Creature", "Vampire"],
            body: (2, 1),
            keywords: &[],
            must_attack: true,
            has_madness: false,
            madness_cost: "",
            line: 2,
        },
        CounterCase {
            id: "bloodmad_vampire",
            name: "Bloodmad Vampire",
            mana: "{2}{R}",
            types: &["Creature", "Vampire", "Berserker"],
            body: (4, 1),
            keywords: &[],
            must_attack: false,
            has_madness: true,
            madness_cost: "{1}{R}",
            line: 1,
        },
        CounterCase {
            id: "heirs_of_stromkirk",
            name: "Heirs of Stromkirk",
            mana: "{2}{R}{R}",
            types: &["Creature", "Vampire"],
            body: (2, 2),
            keywords: &[Keyword::Intimidate],
            must_attack: false,
            has_madness: false,
            madness_cost: "",
            line: 2,
        },
        CounterCase {
            id: "markov_blademaster",
            name: "Markov Blademaster",
            mana: "{1}{R}{R}",
            types: &["Creature", "Vampire", "Warrior"],
            body: (1, 1),
            keywords: &[Keyword::DoubleStrike],
            must_attack: false,
            has_madness: false,
            madness_cost: "",
            line: 2,
        },
    ]
}

#[test]
fn q87_combat_counter_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in counter_cases() {
        let definition = registry
            .get(case.id)
            .unwrap_or_else(|| panic!("missing {}", case.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", case.id);
        assert_eq!(definition.name, case.name, "{}", case.id);
        assert_eq!(
            registry.id_for_name(case.name),
            Some(case.id),
            "{}",
            case.id
        );
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), case.id, "{} face id", case.id);
        assert_eq!(face.mana_cost.to_string(), case.mana, "{} mana", case.id);
        assert_eq!(face.types, case.types, "{} types", case.id);
        assert_eq!(face.power, Some(case.body.0), "{} power", case.id);
        assert_eq!(face.toughness, Some(case.body.1), "{} toughness", case.id);
        assert_eq!(face.keywords, case.keywords, "{} keywords", case.id);
        assert_eq!(
            face.must_attack_if_able, case.must_attack,
            "{}: printed attack requirement",
            case.id
        );
        assert!(
            face.activated_abilities.is_empty() && face.spell_effect.is_empty(),
            "{}: no activated ability or spell effect",
            case.id
        );

        if case.has_madness {
            let [static_ability] = face.static_abilities.as_slice() else {
                panic!("{}: exactly one static ability", case.id);
            };
            assert_eq!(
                static_ability.presentation,
                AbilityPresentation::OracleLines(vec![2]),
                "{} madness presentation",
                case.id
            );
            assert_eq!(
                static_ability.definition,
                StaticAbilityDef::Madness {
                    cost: tricerules_cards::ManaCost::parse(case.madness_cost)
                        .expect("printed madness cost")
                },
                "{}",
                case.id
            );
        } else {
            assert!(
                face.static_abilities.is_empty(),
                "{}: no static ability",
                case.id
            );
        }

        let [ability] = face.triggered_abilities.as_slice() else {
            panic!("{}: exactly one triggered ability", case.id);
        };
        assert_eq!(
            ability.trigger,
            TriggerCondition::WheneverSelfDealsCombatDamageToPlayer,
            "{}",
            case.id
        );
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![case.line]),
            "{} counter-trigger presentation",
            case.id
        );
        let [effect] = ability.effect.as_slice() else {
            panic!("{}: exactly one effect", case.id);
        };
        assert!(
            matches!(
                effect,
                SpellEffectKind::PutCounters {
                    counter: CounterKind::PlusOnePlusOne,
                    count: Amount::Fixed(1),
                    subject: EffectSubject::Source,
                }
            ),
            "{}: one +1/+1 counter on the source",
            case.id
        );
    }
}

// ── 03 counted artifact self pump ─────────────────────────────────────────

struct NimCase {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    body: (u32, u32),
    keywords: &'static [Keyword],
    line: u16,
}

fn nim_cases() -> [NimCase; 3] {
    [
        NimCase {
            id: "nim_lasher",
            name: "Nim Lasher",
            mana: "{2}{B}",
            types: &["Creature", "Zombie"],
            body: (1, 1),
            keywords: &[],
            line: 1,
        },
        NimCase {
            id: "nim_shrieker",
            name: "Nim Shrieker",
            mana: "{3}{B}",
            types: &["Creature", "Zombie"],
            body: (0, 1),
            keywords: &[Keyword::Flying],
            line: 2,
        },
        NimCase {
            id: "nim_grotesque",
            name: "Nim Grotesque",
            mana: "{6}{B}",
            types: &["Creature", "Zombie"],
            body: (3, 6),
            keywords: &[],
            line: 1,
        },
    ]
}

fn artifact_count_filter() -> BattlefieldPermanentFilter {
    BattlefieldPermanentFilter::matching(
        RelativePlayerSet::Controller,
        tricerules_cards::primitives::ObjectFilter {
            kind: TargetKind::AnyPermanent,
            permanent_types: vec![PermanentTypeFilter::Artifact],
            ..Default::default()
        },
    )
}

#[test]
fn q87_artifact_count_pump_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in nim_cases() {
        let definition = registry
            .get(case.id)
            .unwrap_or_else(|| panic!("missing {}", case.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", case.id);
        assert_eq!(definition.name, case.name, "{}", case.id);
        assert_eq!(
            registry.id_for_name(case.name),
            Some(case.id),
            "{}",
            case.id
        );
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), case.id, "{} face id", case.id);
        assert_eq!(face.mana_cost.to_string(), case.mana, "{} mana", case.id);
        assert_eq!(face.types, case.types, "{} types", case.id);
        assert_eq!(face.power, Some(case.body.0), "{} power", case.id);
        assert_eq!(face.toughness, Some(case.body.1), "{} toughness", case.id);
        assert_eq!(face.keywords, case.keywords, "{} keywords", case.id);
        assert!(
            face.activated_abilities.is_empty()
                && face.triggered_abilities.is_empty()
                && face.spell_effect.is_empty(),
            "{}: the static count is the only non-keyword clause",
            case.id
        );

        let [ability] = face.static_abilities.as_slice() else {
            panic!("{}: exactly one static ability", case.id);
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![case.line]),
            "{} presentation",
            case.id
        );
        assert_eq!(
            ability.definition,
            StaticAbilityDef::CountScaledSelfPt {
                count: CountExpression::BattlefieldPermanents {
                    filter: artifact_count_filter(),
                },
                power_per_match: 1,
                toughness_per_match: 0,
            },
            "{}: the printed +1/+0 per artifact you control",
            case.id
        );
    }
}

// ── fingerprints ──────────────────────────────────────────────────────────

#[test]
fn q87_batch_a_fingerprints_match_the_pinned_oracle_text() {
    const FINGERPRINTS: &[(&str, &str)] = &[
        (
            "defiant_falcon",
            "04451f97e6c7d5dcbf18d60e4cb9b1b04ccb3d4a83a09e400cd4834923401dc0",
        ),
        (
            "ramosian_captain",
            "c4c510d10216751cf447f06881de8e19d01e68fcac998d8744cd665f934a7f32",
        ),
        (
            "ramosian_sky_marshal",
            "51ca6eb86c39cc850c06e0ca9e85d64a43d72928a7581ecc644311a89dad9d5a",
        ),
        (
            "bloodcrazed_neonate",
            "7c51c88b9f4101a797b9297b73a4da13a04f968a722934bf9b98cf05c59e099b",
        ),
        (
            "bloodmad_vampire",
            "8619324fe820ae7ea0fa07fb9761db7f09f92418c4ac01d4420a2b0bff2df2b5",
        ),
        (
            "heirs_of_stromkirk",
            "bda9bc5370123e69da7af38d473f3d584ae6ac052bd5789c5541f6d12efd9502",
        ),
        (
            "markov_blademaster",
            "7087be4f5acef662a8e5b39ffdcf7e7002e8467a29adc679947c4c369c14b19a",
        ),
        (
            "nim_lasher",
            "29b0a46555893c7bde657b4a73f9a1052c20a2156a5d66ed8e58605273b5fa57",
        ),
        (
            "nim_shrieker",
            "364e1c9e0ee3223dcaede4bc36ec36e3d402e2158e53d802b9e9a31da75bb993",
        ),
        (
            "nim_grotesque",
            "29b0a46555893c7bde657b4a73f9a1052c20a2156a5d66ed8e58605273b5fa57",
        ),
    ];
    // The literal hashes are the pinned bulk's normalized Oracle text digests; asserting them
    // here keeps a wrong presentation-registry row from passing against itself.
    let registry = tricerules_cards::registry::global();
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    let mut seen = 0usize;
    for (id, expected) in FINGERPRINTS {
        let presentation = registry
            .presentation_face(id, id)
            .unwrap_or_else(|| panic!("missing presentation metadata for {id}"));
        assert_eq!(presentation.oracle_text_sha256.len(), 64, "{id}");
        assert_eq!(
            presentation.card_name,
            registry.get(id).expect("registered").name,
            "{id}"
        );
        assert_eq!(
            &presentation.oracle_text_sha256, expected,
            "pinned bulk digest for {id}"
        );
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], *id);
        assert_eq!(fields[4], *expected, "fingerprint drift for {id}");
        seen += 1;
    }
    assert_eq!(seen, 10, "all ten new cards are asserted");
}
