//! Registry conformance for the reviewed shrink / draw cohort.
//!
//! Every included card is a normal-layout Instant that gives a chosen creature the
//! printed negative P/T change until end of turn, then draws exactly one card.
use tricerules_cards::primitives::{EffectSubject, PlayerRecipient, SpellEffectKind};
use tricerules_cards::{Amount, Layout};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    power: i32,
    toughness: i32,
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "afflict",
        name: "Afflict",
        mana_cost: "{2}{B}",
        power: -1,
        toughness: -1,
        oracle_text_sha256: "7ca60fb109bb0b9a45e644c26f1a13723854b544131705cb2bb75b7586447555",
    },
    ExpectedCard {
        id: "befuddle",
        name: "Befuddle",
        mana_cost: "{2}{U}",
        power: -4,
        toughness: 0,
        oracle_text_sha256: "4e0ba55d9b68384259050a117717b6786d7c314df43baf619b8bd709cf7465c3",
    },
    ExpectedCard {
        id: "bewilder",
        name: "Bewilder",
        mana_cost: "{2}{U}",
        power: -3,
        toughness: 0,
        oracle_text_sha256: "1311401b5fceec4ad56e9867c28af5ff6996010095f6553f983995b5c07983b5",
    },
    ExpectedCard {
        id: "shocking_grasp",
        name: "Shocking Grasp",
        mana_cost: "{1}{U}",
        power: -2,
        toughness: 0,
        oracle_text_sha256: "e6ffae2380247ee0f4c61251ef7c157171a50cbd887bfda38ece689a2cd8b0d1",
    },
];

#[test]
fn shrink_draw_batch_maps_definitions() {
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
        assert!(
            matches!(
                face.spell_effect.as_slice(),
                [SpellEffectKind::PumpTarget {
                    power,
                    toughness,
                    subject: EffectSubject::Chosen(_),
                    ..
                }, SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(1),
                    ..
                }] if *power == expected.power && *toughness == expected.toughness
            ),
            "{id}: printed shrink then one controller draw"
        );
        let targeting = face.targeting.as_ref().expect("{id}: one target group");
        let [group] = targeting.groups.as_slice() else {
            panic!("{id}: one group");
        };
        assert_eq!((group.min, group.max), (1, 1), "{id}");
        assert_eq!(
            group.effect_indices,
            [0],
            "{id}: only the shrink is targeted"
        );
    }
}

#[test]
fn shrink_draw_batch_fingerprints_match_the_pinned_oracle_text() {
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
