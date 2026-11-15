//! Registry conformance for the reviewed trample / landfall cohort.
//!
//! Every included card is a normal-layout creature with Trample and one landfall
//! trigger that pumps only the source by the printed amount.
use tricerules_cards::primitives::{
    CastTriggerPlayer, EffectSubject, PermanentTypeFilter, SpellEffectKind, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, Keyword, Layout};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    body: (u32, u32),
    pump: (i32, i32),
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "grove_rumbler",
        name: "Grove Rumbler",
        mana_cost: "{2}{R}{G}",
        types: &["Creature", "Elemental"],
        body: (3, 3),
        pump: (2, 2),
        oracle_text_sha256: "bcb4f3e339e63cc04d22ad15a1d8f1d9a68834ac294731e4afb458abef4a913a",
    },
    ExpectedCard {
        id: "makindi_sliderunner",
        name: "Makindi Sliderunner",
        mana_cost: "{1}{R}",
        types: &["Creature", "Beast"],
        body: (2, 1),
        pump: (1, 1),
        oracle_text_sha256: "e267053aaa19f063903a29f2439b6c0d6e2fbcc8b134d96dcf46f85b34c7ca9c",
    },
    ExpectedCard {
        id: "skyclave_geopede",
        name: "Skyclave Geopede",
        mana_cost: "{2}{R}",
        types: &["Creature", "Insect"],
        body: (3, 1),
        pump: (2, 2),
        oracle_text_sha256: "bcb4f3e339e63cc04d22ad15a1d8f1d9a68834ac294731e4afb458abef4a913a",
    },
];

#[test]
fn landfall_trample_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for expected in CARDS {
        let id = expected.id;
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(definition.name, expected.name);
        assert_eq!(registry.id_for_name(expected.name), Some(id));
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id.replace('-', "_"));
        assert_eq!(face.mana_cost.to_string(), expected.mana_cost, "{id}");
        assert_eq!(face.types, expected.types, "{id}");
        assert!(face.supertypes.is_empty(), "{id}");
        assert_eq!(
            (face.power, face.toughness),
            (Some(expected.body.0), Some(expected.body.1)),
            "{id}"
        );
        assert_eq!(face.keywords, [Keyword::Trample], "{id}");
        assert!(face.activated_abilities.is_empty(), "{id}");
        assert!(face.static_abilities.is_empty(), "{id}");
        assert!(face.spell_effect.is_empty(), "{id}");
        assert!(face.targeting.is_none(), "{id}");
        let [ability] = face.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one triggered ability");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id} presentation"
        );
        let TriggerCondition::WheneverPermanentEntersBattlefield {
            controller, filter, ..
        } = &ability.trigger
        else {
            panic!("{id}: land entry trigger");
        };
        assert_eq!(*controller, CastTriggerPlayer::Controller, "{id}");
        assert_eq!(
            filter.permanent_type,
            Some(PermanentTypeFilter::Land),
            "{id}"
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::PumpTarget {
                    power,
                    toughness,
                    subject: EffectSubject::Source,
                    ..
                }] if *power == expected.pump.0 && *toughness == expected.pump.1
            ),
            "{id}: printed source pump"
        );
        assert!(ability.targeting.is_none(), "{id}");
    }
}

#[test]
fn landfall_trample_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for expected in CARDS {
        let id = expected.id;
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], expected.name);
        assert_eq!(fields[2], id.replace('-', "_"));
        assert_eq!(
            fields[4], expected.oracle_text_sha256,
            "fingerprint drift for {id}"
        );
    }
}
