//! Registry conformance for the reviewed team-pump-on-enter cohort.
//!
//! Every included card is a normal-layout creature with one "another creature you
//! control enters" trigger that pumps only the source by the printed amount.
use tricerules_cards::primitives::{
    CastTriggerPlayer, EffectSubject, PermanentTypeFilter, SpellEffectKind, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, Layout};

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
        id: "bronzebeak_moa",
        name: "Bronzebeak Moa",
        mana_cost: "{2}{G}{W}",
        types: &["Creature", "Bird"],
        body: (2, 2),
        pump: (3, 3),
        oracle_text_sha256: "b7167363911c993895b82192d70cfd5789a60e9cd5b0137be4e528429bcac675",
    },
    ExpectedCard {
        id: "herd_gnarr",
        name: "Herd Gnarr",
        mana_cost: "{3}{G}",
        types: &["Creature", "Beast"],
        body: (2, 2),
        pump: (2, 2),
        oracle_text_sha256: "4f1f662971b54683942543e37897bb936eee21d1836eceadd6b75766c0f17f75",
    },
    ExpectedCard {
        id: "llanowar_stalker",
        name: "Llanowar Stalker",
        mana_cost: "{G}",
        types: &["Creature", "Elf", "Warrior"],
        body: (1, 1),
        pump: (1, 0),
        oracle_text_sha256: "83fc6ab5479d7170dc8a4df5c25307181e861c9fcfafd7d6121b0ea013835500",
    },
];

#[test]
fn team_pump_on_enter_batch_maps_definitions() {
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
        assert!(face.keywords.is_empty(), "{id}");
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
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        let TriggerCondition::WheneverPermanentEntersBattlefield {
            controller, filter, ..
        } = &ability.trigger
        else {
            panic!("{id}: creature entry trigger");
        };
        assert_eq!(*controller, CastTriggerPlayer::Controller, "{id}");
        assert_eq!(
            filter.permanent_type,
            Some(PermanentTypeFilter::Creature),
            "{id}"
        );
        assert!(
            filter.exclude_source,
            "{id}: \"another\" excludes the source"
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
fn team_pump_on_enter_batch_fingerprints_match_the_pinned_oracle_text() {
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
