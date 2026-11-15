//! Registry conformance for the queue-v18 lifelink pump cohort.
//!
//! Every included card is a normal-layout instant that gives one target creature a
//! printed P/T bonus and lifelink until end of turn.
use tricerules_cards::primitives::{EffectSubject, SpellEffectKind};
use tricerules_cards::{Keyword, Layout};

struct ExpectedCard {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    pump: (i32, i32),
    oracle_text_sha256: &'static str,
}

const CARDS: &[ExpectedCard] = &[
    ExpectedCard {
        id: "grotesque_mutation",
        name: "Grotesque Mutation",
        mana_cost: "{1}{B}",
        pump: (3, 1),
        oracle_text_sha256: "e1dc6f139bc258e9677ebb8b8c34fdc0ec66c7791f2f5443b9a5d5877917d22f",
    },
    ExpectedCard {
        id: "moment_of_heroism",
        name: "Moment of Heroism",
        mana_cost: "{1}{W}",
        pump: (2, 2),
        oracle_text_sha256: "106c76a09e838e17182160af35ab63fa82aaade5f6577d611f8b1b84134bf198",
    },
    ExpectedCard {
        id: "mortals_ardor",
        name: "Mortal's Ardor",
        mana_cost: "{W}",
        pump: (1, 1),
        oracle_text_sha256: "55bcc12676c8ed0ea47036d68db20859cc7d31d088a53e4620325d621694c08d",
    },
];

#[test]
fn lifelink_pump_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for expected in CARDS {
        let id = expected.id;
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(definition.name, expected.name);
        assert_eq!(registry.id_for_name(expected.name), Some(id));
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id.replace('-', "_"), "{id}");
        assert_eq!(face.mana_cost.to_string(), expected.mana_cost, "{id}");
        assert_eq!(face.types, ["Instant"], "{id}");
        assert!(face.keywords.is_empty(), "{id}");
        assert!(face.activated_abilities.is_empty(), "{id}");
        assert!(face.static_abilities.is_empty(), "{id}");
        assert!(face.triggered_abilities.is_empty(), "{id}");
        assert!(
            matches!(
                &face.spell_effect[0],
                SpellEffectKind::PumpTarget {
                    power,
                    toughness,
                    subject: EffectSubject::Chosen(_),
                    ..
                } if *power == expected.pump.0 && *toughness == expected.pump.1
            ),
            "{id}: printed target pump"
        );
        assert!(
            matches!(
                &face.spell_effect[1],
                SpellEffectKind::GrantKeywords {
                    subject: EffectSubject::Chosen(_),
                    keywords,
                } if keywords == &[Keyword::Lifelink]
            ),
            "{id}: target gains lifelink"
        );
        let Some(targeting) = &face.targeting else {
            panic!("{id}: one target group");
        };
        assert_eq!(targeting.groups.len(), 1, "{id}");
        assert_eq!(targeting.groups[0].min, 1, "{id}");
        assert_eq!(targeting.groups[0].max, 1, "{id}");
        assert_eq!(targeting.groups[0].effect_indices, [0, 1], "{id}");
    }
}

#[test]
fn lifelink_pump_batch_fingerprints_match_the_pinned_oracle_text() {
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
