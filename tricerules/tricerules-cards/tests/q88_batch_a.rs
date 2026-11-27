//! Registry conformance for the queue-v88 batch A card data.
//!
//! Module 1 covers the ten Planet refuge lands (enters tapped, gains one life, taps for one of
//! two printed colors), module 2 the three sacrifice-for-a-card creatures, and module 3 the
//! three activated +1/+1 counter creatures. Fingerprints are asserted against the pinned bulk
//! after the presentation metadata refresh.

use tricerules_cards::primitives::{
    AbilityCost, ActivationTiming, Amount, EntersTappedAffected, Keyword, PlayerRecipient,
    SpellEffectKind, StaticAbilityDef, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, CardDefinition, Layout};

fn face(definition: &CardDefinition) -> &tricerules_cards::CardFace {
    definition.primary_face()
}

// ── 01 Planet refuges ─────────────────────────────────────────────────────

struct RefugeCase {
    id: &'static str,
    name: &'static str,
    /// The face id is canonical snake_case; the card id keeps the printed comma
    /// (engine slugify preserves it, like `kavaron,_memorial_world`).
    face_id: &'static str,
    first: char,
    second: char,
    fingerprint: &'static str,
}

const REFUGES: &[RefugeCase] = &[
    RefugeCase {
        id: "andoria",
        name: "Andoria",
        face_id: "andoria",
        first: 'u',
        second: 'r',
        fingerprint: "c83bd094bbf6951818da0cb250c8ffa4b7440a07b565a5795a064b4d5624d483",
    },
    RefugeCase {
        id: "bajor",
        name: "Bajor",
        face_id: "bajor",
        first: 'r',
        second: 'w',
        fingerprint: "20d2cc01324bed5aedaa76a604570d6964118c090971e66f421fba1653984d5c",
    },
    RefugeCase {
        id: "cardassia_prime",
        name: "Cardassia Prime",
        face_id: "cardassia_prime",
        first: 'w',
        second: 'b',
        fingerprint: "5adf44c804519e14a0fb8c435cedcd3a1572b52ee356181fe6a4db309606aa27",
    },
    RefugeCase {
        id: "earth,_sector_001",
        name: "Earth, Sector 001",
        face_id: "earth_sector_001",
        first: 'g',
        second: 'u',
        fingerprint: "88fcd5ec810a6cfffda0513b70d22c3929ebcbddd85547f8a4599dfafa9ef22c",
    },
    RefugeCase {
        id: "ferenginar",
        name: "Ferenginar",
        face_id: "ferenginar",
        first: 'b',
        second: 'g',
        fingerprint: "df30fa0d9d7ed6325dddabf3a0faf16b22e4d572142a5171cd3f906c64d67656",
    },
    RefugeCase {
        id: "orion",
        name: "Orion",
        face_id: "orion",
        first: 'r',
        second: 'g',
        fingerprint: "297bc9080795f4c61217d465fa4455faf56dc50c9287f405e653e0ab39d36c67",
    },
    RefugeCase {
        id: "qonos",
        name: "Qo'noS",
        face_id: "qonos",
        first: 'b',
        second: 'r',
        fingerprint: "65355c8593f50e489d5854db246a42df2dcf8303d23907e4b8fe4634b2fb48d3",
    },
    RefugeCase {
        id: "risa",
        name: "Risa",
        face_id: "risa",
        first: 'g',
        second: 'w',
        fingerprint: "e1e5ffb5492349ccf51aeff240bce97e2394327430ace8f9ed3b6542b2aba7d7",
    },
    RefugeCase {
        id: "romulus",
        name: "Romulus",
        face_id: "romulus",
        first: 'u',
        second: 'b',
        fingerprint: "40ce155786d374fbfcba12b09354ce2195c5b221fabaf77189a8669128301542",
    },
    RefugeCase {
        id: "vulcan",
        name: "Vulcan",
        face_id: "vulcan",
        first: 'w',
        second: 'u',
        fingerprint: "084b0dfe00f7740663f01f2305d1f366c0f241b00560a45d53d8ecd38e5eb818",
    },
];

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

#[test]
fn q88_planet_refuges_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in REFUGES {
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
        assert_eq!(f.face_id.as_str(), case.face_id, "{} face id", case.id);
        assert_eq!(f.mana_cost.to_string(), "", "{} mana cost", case.id);
        assert_eq!(f.types, ["Land", "Planet"], "{} types", case.id);
        assert_eq!((f.power, f.toughness), (None, None), "{} body", case.id);
        assert!(f.keywords.is_empty(), "{}", case.id);
        assert!(f.spell_effect.is_empty(), "{}", case.id);
        assert!(f.targeting.is_none(), "{}", case.id);

        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{}: exactly one static ability", case.id);
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{}",
            case.id
        );
        // Exhaustive: an unconditional self replacement with no entry-cost escape.
        assert_eq!(
            static_ability.definition,
            StaticAbilityDef::EntersTapped {
                affected: EntersTappedAffected::Self_,
                condition: None,
                unless_cost: None,
            },
            "{}: enters tapped unconditionally",
            case.id
        );
        assert_eq!(
            static_ability.ability_id.as_str(),
            "static_01",
            "{} static id",
            case.id
        );

        let [triggered] = f.triggered_abilities.as_slice() else {
            panic!("{}: exactly one entry trigger", case.id);
        };
        assert_eq!(
            triggered.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{}",
            case.id
        );
        assert_eq!(
            triggered.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{}",
            case.id
        );
        assert_eq!(
            triggered.effect,
            [SpellEffectKind::GainLife {
                amount: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
            }],
            "{}: gains exactly one life",
            case.id
        );

        let [activated] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one mana ability", case.id);
        };
        assert_eq!(
            activated.presentation,
            AbilityPresentation::OracleLines(vec![3]),
            "{}",
            case.id
        );
        assert_eq!(activated.costs, [AbilityCost::Tap], "{}", case.id);
        assert!(activated.is_mana_ability(), "{}", case.id);
        assert!(activated.targeting.is_none(), "{}", case.id);
        assert_eq!(
            activated.timing,
            ActivationTiming::Normal,
            "{}: no printed timing restriction",
            case.id
        );
        assert!(
            activated.conditions.is_empty(),
            "{}: no printed activation condition",
            case.id
        );
        assert!(
            activated.activation_limit.is_none(),
            "{}: no printed activation limit",
            case.id
        );
        assert_eq!(
            activated.ability_id.as_str(),
            "activated_01",
            "{} ability id",
            case.id
        );
        let [SpellEffectKind::ProduceMana { options, .. }] = activated.effect.as_slice() else {
            panic!("{}: one mana production", case.id);
        };
        assert_eq!(options.len(), 2, "{}", case.id);
        assert_eq!(
            amounts(&options[0]),
            mask(case.first),
            "{} first color",
            case.id
        );
        assert_eq!(
            amounts(&options[1]),
            mask(case.second),
            "{} second color",
            case.id
        );
    }
}

// ── 02 sacrifice for a card ───────────────────────────────────────────────

struct SacCase {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    body: (u32, u32),
    keywords: &'static [Keyword],
    line: u16,
    fingerprint: &'static str,
}

const SAC_CARDS: &[SacCase] = &[
    SacCase {
        id: "brass_secretary",
        name: "Brass Secretary",
        mana: "{3}",
        types: &["Artifact", "Creature", "Construct"],
        body: (2, 1),
        keywords: &[],
        line: 1,
        fingerprint: "65fdbe5efe5a44c2936b669fe0ab015e24435ea5c81660ce5ec5700bd3bf5135",
    },
    SacCase {
        id: "parcel_myr",
        name: "Parcel Myr",
        mana: "{1}{U}",
        types: &["Artifact", "Creature", "Clue", "Myr"],
        body: (2, 1),
        keywords: &[],
        line: 1,
        fingerprint: "65fdbe5efe5a44c2936b669fe0ab015e24435ea5c81660ce5ec5700bd3bf5135",
    },
    SacCase {
        id: "slinking_skirge",
        name: "Slinking Skirge",
        mana: "{3}{B}",
        types: &["Creature", "Phyrexian", "Imp"],
        body: (2, 1),
        keywords: &[Keyword::Flying],
        line: 2,
        fingerprint: "bce267f002c96c4e6fdb4045dc820e088c33380b5ea03cf9a695eb4ee5d60dc1",
    },
];

#[test]
fn q88_sac_draw_cards_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in SAC_CARDS {
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
        assert_eq!(f.mana_cost.to_string(), case.mana, "{} mana", case.id);
        assert_eq!(f.types, case.types, "{} types", case.id);
        assert_eq!(f.power, Some(case.body.0), "{} power", case.id);
        assert_eq!(f.toughness, Some(case.body.1), "{} toughness", case.id);
        assert_eq!(f.keywords, case.keywords, "{} keywords", case.id);
        assert!(
            f.triggered_abilities.is_empty()
                && f.static_abilities.is_empty()
                && f.spell_effect.is_empty(),
            "{}: only the activated ability remains",
            case.id
        );

        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", case.id);
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![case.line]),
            "{}",
            case.id
        );
        assert_eq!(
            ability.costs,
            [
                AbilityCost::Mana(tricerules_cards::ManaCost::parse("{2}").expect("printed {{2}}")),
                AbilityCost::SacrificeSelf,
            ],
            "{}: exactly the printed {{2}} and the sacrifice, with no {{T}}",
            case.id
        );
        assert_eq!(
            ability.timing,
            ActivationTiming::Normal,
            "{}: no printed timing restriction",
            case.id
        );
        assert!(
            ability.conditions.is_empty() && ability.activation_limit.is_none(),
            "{}: no printed condition or activation limit",
            case.id
        );
        assert!(ability.targeting.is_none(), "{}: no target", case.id);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }],
            "{}: draw one",
            case.id
        );
    }
}

// ── 03 activated +1/+1 counters ───────────────────────────────────────────

struct CounterCase {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    body: (u32, u32),
    keywords: &'static [Keyword],
    activation: &'static str,
    line: u16,
    fingerprint: &'static str,
}

const COUNTER_CARDS: &[CounterCase] = &[
    CounterCase {
        id: "jungle_delver",
        name: "Jungle Delver",
        mana: "{G}",
        types: &["Creature", "Merfolk", "Warrior"],
        body: (1, 1),
        keywords: &[],
        activation: "{3}{G}",
        line: 1,
        fingerprint: "23318b634be3f234cfa0e512abac3ca5f63279dc63bb08d2f3302ec6ec8c020d",
    },
    CounterCase {
        id: "unholy_officiant",
        name: "Unholy Officiant",
        mana: "{W}",
        types: &["Creature", "Vampire", "Cleric"],
        body: (1, 2),
        keywords: &[Keyword::Vigilance],
        activation: "{4}{W}",
        line: 2,
        fingerprint: "cbc995f00a20a01c1bf5a274e9f31c5fbb53f323352f2641ceedee221386c8ab",
    },
    CounterCase {
        id: "verdant_automaton",
        name: "Verdant Automaton",
        mana: "{2}",
        types: &["Artifact", "Creature", "Construct"],
        body: (1, 2),
        keywords: &[],
        activation: "{3}{G}",
        line: 1,
        fingerprint: "23318b634be3f234cfa0e512abac3ca5f63279dc63bb08d2f3302ec6ec8c020d",
    },
];

#[test]
fn q88_counter_cards_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in COUNTER_CARDS {
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
        assert_eq!(f.mana_cost.to_string(), case.mana, "{} mana", case.id);
        assert_eq!(f.types, case.types, "{} types", case.id);
        assert_eq!(f.power, Some(case.body.0), "{} power", case.id);
        assert_eq!(f.toughness, Some(case.body.1), "{} toughness", case.id);
        assert_eq!(f.keywords, case.keywords, "{} keywords", case.id);
        assert!(
            f.triggered_abilities.is_empty()
                && f.static_abilities.is_empty()
                && f.spell_effect.is_empty(),
            "{}: only the activated ability remains",
            case.id
        );

        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", case.id);
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![case.line]),
            "{}",
            case.id
        );
        assert_eq!(
            ability.costs,
            [AbilityCost::Mana(
                tricerules_cards::ManaCost::parse(case.activation)
                    .expect("printed activation cost")
            )],
            "{}: exactly one mana cost and no {{T}}",
            case.id
        );
        assert_eq!(
            ability.timing,
            ActivationTiming::Normal,
            "{}: no printed timing restriction",
            case.id
        );
        assert!(
            ability.conditions.is_empty() && ability.activation_limit.is_none(),
            "{}: no printed condition or activation limit",
            case.id
        );
        assert!(ability.targeting.is_none(), "{}: no target", case.id);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::PutCounters {
                counter: tricerules_cards::CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: tricerules_cards::primitives::EffectSubject::Source,
            }],
            "{}: one +1/+1 counter on the source",
            case.id
        );
    }
}

// ── fingerprints ──────────────────────────────────────────────────────────

/// The registered face's own id. The card id keeps printed punctuation (`slugify` preserves a
/// comma) while the face id is canonical snake_case, so the presentation row is looked up with
/// the face's id and the test cannot silently paper over a mismatch with a literal table.
fn face_id_of(id: &str) -> String {
    let definition = tricerules_cards::registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"));
    definition.primary_face().face_id.as_str().to_string()
}

#[test]
fn q88_batch_a_fingerprints_match_the_pinned_oracle_text() {
    let registry = tricerules_cards::registry::global();
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    let mut seen = 0usize;
    let mut assert_one = |id: &str, expected: &str| {
        // The presentation row is keyed by (card id, face id); the two differ only for
        // names whose printed punctuation the engine keeps in the card id.
        let face_id = face_id_of(id);
        let presentation = registry
            .presentation_face(id, &face_id)
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
        assert_eq!(fields[0], id);
        assert_eq!(fields[2], face_id, "face id column for {id}");
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
        seen += 1;
    };
    for case in REFUGES {
        assert_one(case.id, case.fingerprint);
    }
    for case in SAC_CARDS {
        assert_one(case.id, case.fingerprint);
    }
    for case in COUNTER_CARDS {
        assert_one(case.id, case.fingerprint);
    }
    assert_eq!(seen, 16, "all sixteen new cards are asserted");
}
