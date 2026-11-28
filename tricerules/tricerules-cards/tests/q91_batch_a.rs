//! Registry conformance for the queue-v91 batch A card data.
//!
//! Module 1 covers the four "{one colored mana}: Regenerate this creature" creatures, module 2
//! the three "{1}{colored}: Regenerate this creature" creatures, and module 3 the three
//! "search your library for up to two basic land cards, put them onto the battlefield tapped"
//! sorceries. Fingerprints are asserted against the pinned bulk after the presentation metadata
//! refresh.

use tricerules_cards::primitives::{
    AbilityCost, Evasion, Keyword, ProtectionQuality, SpellEffectKind, SpellKeyword, ZoneCardFilter,
};
use tricerules_cards::{
    AbilityPresentation, CardDefinition, CardFace, CharacteristicDefiningAbility, Layout,
};

fn face(definition: &CardDefinition) -> &CardFace {
    definition.primary_face()
}

// ── 01 One-mana regeneration ─────────────────────────────────────────────

struct OneManaCase {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    keywords: &'static [Keyword],
    fingerprint: &'static str,
}

const ONE_MANA: &[OneManaCase] = &[
    OneManaCase {
        id: "charging_troll",
        name: "Charging Troll",
        mana: "{2}{G}{W}",
        types: &["Creature", "Troll"],
        power: 3,
        toughness: 3,
        keywords: &[Keyword::Vigilance],
        fingerprint: "82cf2737f126f67754caa0efc771fb3f5a36101ee1a03920787b6e34f809b5f2",
    },
    OneManaCase {
        id: "spectral_lynx",
        name: "Spectral Lynx",
        mana: "{1}{W}",
        types: &["Creature", "Cat", "Spirit"],
        power: 2,
        toughness: 1,
        keywords: &[],
        fingerprint: "7bff4241ca0563f466610565cadce95f2a3fd02bb3411f17ed1f4b4bb4e9b7f5",
    },
    OneManaCase {
        id: "kathari_remnant",
        name: "Kathari Remnant",
        mana: "{2}{U}{B}",
        types: &["Creature", "Bird", "Skeleton"],
        power: 0,
        toughness: 1,
        keywords: &[Keyword::Flying],
        fingerprint: "dc0f49f223789c2e90f16a28b98a4a41af1da0511a752830c5cab93065fc757d",
    },
    OneManaCase {
        id: "mire_boa",
        name: "Mire Boa",
        mana: "{1}{G}",
        types: &["Creature", "Snake"],
        power: 2,
        toughness: 1,
        keywords: &[],
        fingerprint: "ef22b0e24d3920ca3b26a5c5f0a374362285a25ceac389adb8d2b7357ff7cf09",
    },
];

#[test]
fn q91_one_mana_regenerate_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in ONE_MANA {
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
        assert!(f.supertypes.is_empty(), "{} supertypes", case.id);
        assert!(f.triggered_abilities.is_empty(), "{}", case.id);
        assert!(f.static_abilities.is_empty(), "{}", case.id);
        assert!(f.spell_effect.is_empty(), "{}", case.id);
        assert!(f.targeting.is_none(), "{}", case.id);

        let [regenerate] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", case.id);
        };
        assert_eq!(
            regenerate.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{}",
            case.id
        );
        assert!(!regenerate.is_mana_ability(), "{}", case.id);
        assert!(regenerate.targeting.is_none(), "{}", case.id);
        let [AbilityCost::Mana(printed)] = regenerate.costs.as_slice() else {
            panic!("{}: one printed mana cost: {:?}", case.id, regenerate.costs);
        };
        let expected = match case.id {
            "charging_troll" | "mire_boa" => "{G}",
            "spectral_lynx" | "kathari_remnant" => "{B}",
            other => unreachable!("{other}"),
        };
        assert_eq!(
            printed.to_string(),
            expected,
            "{} printed activation cost",
            case.id
        );
        let [SpellEffectKind::Regenerate { subject }] = regenerate.effect.as_slice() else {
            panic!("{}: regenerates itself: {:?}", case.id, regenerate.effect);
        };
        assert!(
            matches!(subject, tricerules_cards::primitives::EffectSubject::Source),
            "{}: an untargeted self reference",
            case.id
        );
    }
}

#[test]
fn q91_one_mana_regenerate_cards_keep_their_printed_static_line() {
    let registry = tricerules_cards::registry::global();
    let troll = face(registry.get("charging_troll").unwrap());
    assert!(troll.keywords.contains(&Keyword::Vigilance));

    let lynx = face(registry.get("spectral_lynx").unwrap());
    assert!(lynx.protections.contains(&ProtectionQuality::Color(
        tricerules_cards::primitives::Color::Green
    )));

    let remnant = face(registry.get("kathari_remnant").unwrap());
    assert!(remnant.keywords.contains(&Keyword::Flying));
    assert!(remnant.spell_keywords.contains(&SpellKeyword::Cascade));

    let boa = face(registry.get("mire_boa").unwrap());
    assert!(boa.evasions.contains(&Evasion::Landwalk {
        land_subtype: "Swamp".to_string()
    }));
}

// ── 02 Two-mana regeneration ─────────────────────────────────────────────

struct TwoManaCase {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    keywords: &'static [Keyword],
    fingerprint: &'static str,
}

const TWO_MANA: &[TwoManaCase] = &[
    TwoManaCase {
        id: "carrion_wall",
        name: "Carrion Wall",
        mana: "{1}{B}{B}",
        types: &["Creature", "Wall"],
        power: 3,
        toughness: 2,
        keywords: &[Keyword::Defender],
        fingerprint: "453d1093da8e4a2731c60240755076af5cfc7da37071c5417ddb020eee58dcaf",
    },
    TwoManaCase {
        id: "wolfir_avenger",
        name: "Wolfir Avenger",
        mana: "{1}{G}{G}",
        types: &["Creature", "Wolf", "Warrior"],
        power: 3,
        toughness: 3,
        keywords: &[Keyword::Flash],
        fingerprint: "995d081c491be2cfef1f2a19476786266177fc0e08cd69cacfabd1a65fb542f3",
    },
    TwoManaCase {
        id: "skeletal_changeling",
        name: "Skeletal Changeling",
        mana: "{1}{B}",
        types: &["Creature", "Shapeshifter"],
        power: 1,
        toughness: 1,
        keywords: &[],
        fingerprint: "2c89375b91c1df0e16e9ff09a1febb6428a5e2ab89a4cf584113eba06de9f792",
    },
];

#[test]
fn q91_two_mana_regenerate_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in TWO_MANA {
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
        assert!(f.supertypes.is_empty(), "{} supertypes", case.id);
        assert!(f.triggered_abilities.is_empty(), "{}", case.id);
        assert!(f.static_abilities.is_empty(), "{}", case.id);
        assert!(f.spell_effect.is_empty(), "{}", case.id);
        assert!(f.targeting.is_none(), "{}", case.id);

        let [regenerate] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", case.id);
        };
        assert_eq!(
            regenerate.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{}",
            case.id
        );
        assert!(!regenerate.is_mana_ability(), "{}", case.id);
        let [AbilityCost::Mana(printed)] = regenerate.costs.as_slice() else {
            panic!("{}: one printed mana cost: {:?}", case.id, regenerate.costs);
        };
        let expected = match case.id {
            "carrion_wall" | "skeletal_changeling" => "{1}{B}",
            "wolfir_avenger" => "{1}{G}",
            other => unreachable!("{other}"),
        };
        assert_eq!(
            printed.to_string(),
            expected,
            "{} printed activation cost",
            case.id
        );
        let [SpellEffectKind::Regenerate { subject }] = regenerate.effect.as_slice() else {
            panic!("{}: regenerates itself: {:?}", case.id, regenerate.effect);
        };
        assert!(
            matches!(subject, tricerules_cards::primitives::EffectSubject::Source),
            "{}: an untargeted self reference",
            case.id
        );
    }
}

#[test]
fn q91_skeletal_changeling_authors_changeling_as_a_cda() {
    let registry = tricerules_cards::registry::global();
    let f = face(registry.get("skeletal_changeling").unwrap());
    assert_eq!(
        f.characteristic_defining_abilities,
        [CharacteristicDefiningAbility::Changeling],
        "Changeling is a characteristic-defining ability, not a keyword"
    );
    assert!(
        f.characteristic_defining_abilities
            .iter()
            .all(|cda| *cda == CharacteristicDefiningAbility::Changeling),
        "no other CDA is printed"
    );
}

// ── 03 Basic land pair search ────────────────────────────────────────────

struct SearchCase {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    keywords: &'static [Keyword],
    fingerprint: &'static str,
}

const SEARCHES: &[SearchCase] = &[
    SearchCase {
        id: "explosive_vegetation",
        name: "Explosive Vegetation",
        mana: "{3}{G}",
        keywords: &[],
        fingerprint: "6788285423f2d710e86c15df9b30a663c49eec030b0b462a1aa4b2727fd2db45",
    },
    SearchCase {
        id: "migration_path",
        name: "Migration Path",
        mana: "{3}{G}",
        keywords: &[],
        fingerprint: "06caea4c99e077eb1e801dc5d48f9a83984a234fcc69200d21bc7c76b520ce0a",
    },
    SearchCase {
        id: "nissas_expedition",
        name: "Nissa's Expedition",
        mana: "{4}{G}",
        keywords: &[Keyword::Convoke],
        fingerprint: "1b36ecce1a729caf27006f37f2cac7c30ff6fdc21b89082e30df2e8271ec0fd9",
    },
];

#[test]
fn q91_basic_land_search_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for case in SEARCHES {
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
        assert_eq!(f.types, ["Sorcery"], "{} types", case.id);
        assert_eq!(f.keywords, case.keywords, "{} keywords", case.id);
        assert!(f.supertypes.is_empty(), "{} supertypes", case.id);
        assert!(f.triggered_abilities.is_empty(), "{}", case.id);
        assert!(f.static_abilities.is_empty(), "{}", case.id);
        assert!(f.targeting.is_none(), "{}", case.id);

        let [SpellEffectKind::SearchLibrary {
            count,
            filter,
            destination,
            ..
        }] = f.spell_effect.as_slice()
        else {
            panic!(
                "{}: one direct SearchLibrary payload: {:?}",
                case.id, f.spell_effect
            );
        };
        // "Up to two" is the fixed count-2 search; the engine derives min 0 from the filter.
        assert_eq!(*count, 2, "{}: up to two cards", case.id);
        assert_eq!(
            filter.as_ref(),
            Some(&ZoneCardFilter {
                card_type: Some(tricerules_cards::primitives::CardTypeFilter::BasicLand),
                ..ZoneCardFilter::default()
            }),
            "{}: only basic land cards",
            case.id
        );
        assert_eq!(
            *destination,
            tricerules_cards::primitives::SearchDestination::Battlefield { tapped: true },
            "{}: the found lands enter tapped",
            case.id
        );

        match case.id {
            "migration_path" => {
                let [cycling] = f.activated_abilities.as_slice() else {
                    panic!("{}: exactly one cycling ability", case.id);
                };
                assert_eq!(
                    cycling.presentation,
                    AbilityPresentation::OracleLines(vec![2]),
                    "{}",
                    case.id
                );
                assert_eq!(
                    cycling.source_zone,
                    tricerules_cards::AbilitySourceZone::Hand,
                    "{}: cycling activates from hand",
                    case.id
                );
                let [AbilityCost::Mana(printed), AbilityCost::DiscardSelf] =
                    cycling.costs.as_slice()
                else {
                    panic!("{}: printed mana plus discarding itself", case.id);
                };
                assert_eq!(printed.to_string(), "{2}", "{} cycling cost", case.id);
                assert_eq!(
                    cycling.effect.as_slice(),
                    [SpellEffectKind::Draw {
                        who: tricerules_cards::primitives::PlayerRecipient::Controller,
                        count: tricerules_cards::primitives::Amount::Fixed(1),
                    }],
                    "{}: cycling draws one card",
                    case.id
                );
            }
            _ => {
                assert!(
                    f.activated_abilities.is_empty(),
                    "{}: no hand-zone ability",
                    case.id
                );
            }
        }
    }
}

#[test]
fn q91_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    let cases = ONE_MANA
        .iter()
        .map(|case| (case.id, case.name, case.id, case.fingerprint))
        .chain(
            TWO_MANA
                .iter()
                .map(|case| (case.id, case.name, case.id, case.fingerprint)),
        )
        .chain(
            SEARCHES
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
