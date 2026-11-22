//! Registry conformance for the reviewed pump / first strike / scry cohort.
//!
//! Every included card is a normal-layout Instant that pumps a chosen creature,
//! grants first strike to it until end of turn, then scries exactly one.
use tricerules_cards::primitives::{EffectSubject, SpellEffectKind};
use tricerules_cards::{Amount, Keyword, Layout};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    power: i32,
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "coming_in_hot",
        name: "Coming In Hot",
        mana_cost: "{R}",
        power: 1,
        oracle_text_sha256: "15678e30c802733f22cda66cd605948e5dac0b72f895443d4c03703180fbb6a8",
    },
    ExpectedCard {
        id: "daring_escape",
        name: "Daring Escape",
        mana_cost: "{R}",
        power: 1,
        oracle_text_sha256: "531698895b8430113a5d8d4548687e28b401a72e099c95f808ed6b31fa513fc7",
    },
    ExpectedCard {
        id: "furious_bellow",
        name: "Furious Bellow",
        mana_cost: "{1}{R}",
        power: 3,
        oracle_text_sha256: "3cf3a35a5db5bcd26b8fa5723fdfa7f422672979dbbce334fe833523aa2f0be9",
    },
    ExpectedCard {
        id: "storm_strike",
        name: "Storm Strike",
        mana_cost: "{R}",
        power: 1,
        oracle_text_sha256: "531698895b8430113a5d8d4548687e28b401a72e099c95f808ed6b31fa513fc7",
    },
];

#[test]
fn first_strike_scry_batch_maps_definitions() {
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
        assert_eq!(face.types, ["Instant"], "{id}");
        assert!(face.supertypes.is_empty(), "{id}");
        assert_eq!((face.power, face.toughness), (None, None), "{id}");
        assert!(face.keywords.is_empty(), "{id}");
        assert!(face.activated_abilities.is_empty(), "{id}");
        assert!(face.triggered_abilities.is_empty(), "{id}");
        assert!(face.static_abilities.is_empty(), "{id}");
        let chosen = EffectSubject::Chosen(Box::new(
            tricerules_cards::primitives::TargetFilter::default_creature(),
        ));
        assert!(
            matches!(
                face.spell_effect.as_slice(),
                [SpellEffectKind::PumpTarget {
                    power,
                    toughness,
                    subject,
                    ..
                }, SpellEffectKind::GrantKeywords {
                    subject: grant_subject,
                    keywords,
                }, SpellEffectKind::Scry {
                    count: Amount::Fixed(1)
                }] if *power == expected.power
                    && *toughness == 0
                    && *subject == chosen
                    && *grant_subject == chosen
                    && keywords.as_slice() == [Keyword::FirstStrike]
            ),
            "{id}: printed pump, first strike, then scry one"
        );
        let targeting = face.targeting.as_ref().expect("{id}: one target group");
        let [group] = targeting.groups.as_slice() else {
            panic!("{id}: one group");
        };
        assert_eq!((group.min, group.max), (1, 1), "{id}");
        assert_eq!(
            group.effect_indices,
            [0, 1],
            "{id}: one shared creature target"
        );
    }
}

#[test]
fn first_strike_scry_batch_fingerprints_match_the_pinned_oracle_text() {
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
