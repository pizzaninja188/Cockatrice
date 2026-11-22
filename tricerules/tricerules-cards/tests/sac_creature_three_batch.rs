//! Registry conformance for the reviewed sacrifice-for-three-mana attendant cohort.
//!
//! Every included card is a normal-layout artifact creature with a `{1}` mana
//! ability that sacrifices the source and produces three printed mana.
//! Each fingerprint is SHA-256 of the exact pinned Oracle lines.

use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, CardRegistry, Layout};

type Mana = (u32, u32, u32, u32, u32, u32);

struct Card {
    id: &'static str,
    name: &'static str,
    produced: Mana,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "crosiss_attendant",
        name: "Crosis's Attendant",
        produced: (0, 1, 1, 1, 0, 0),
        fingerprint: "e669f544dea0f2a465af49573adf88e11793b367ea43a93c0eeb8e8d06531b56",
    },
    Card {
        id: "darigaazs_attendant",
        name: "Darigaaz's Attendant",
        produced: (0, 0, 1, 1, 1, 0),
        fingerprint: "1a9fa1170c6cdcb43b6e4d8939e3b83d9c5401825a0e89ceaaca864c68ff6183",
    },
    Card {
        id: "dromars_attendant",
        name: "Dromar's Attendant",
        produced: (1, 1, 1, 0, 0, 0),
        fingerprint: "e078a429d47932dff117264d379246604be402882a5f6946237249b31dc4d188",
    },
    Card {
        id: "riths_attendant",
        name: "Rith's Attendant",
        produced: (1, 0, 0, 1, 1, 0),
        fingerprint: "3ec06659a942cf4dab813093789c6218e5e6661e363e42e83e511d0d483ad3d4",
    },
    Card {
        id: "trevas_attendant",
        name: "Treva's Attendant",
        produced: (1, 1, 0, 0, 1, 0),
        fingerprint: "ccacebd4e60559791ae38790be9abf7931b1623b4d518867a88fa5671148072c",
    },
];

fn face<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn sac_creature_three_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(
            registry.id_for_name(card.name),
            Some(card.id),
            "{}",
            card.id
        );
        let f = face(registry, card.id);
        assert_eq!(f.face_id.as_str(), card.id, "{} face id", card.id);
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(f.mana_cost.to_string(), "{5}", "{} mana cost", card.id);
        assert_eq!(
            f.types,
            ["Artifact", "Creature", "Golem"],
            "{} types",
            card.id
        );
        assert!(f.supertypes.is_empty(), "{} no supertype", card.id);
        assert_eq!(f.power, Some(3), "{} power", card.id);
        assert_eq!(f.toughness, Some(3), "{} toughness", card.id);
        assert!(f.keywords.is_empty(), "{} no keyword", card.id);
        assert!(
            f.static_abilities.is_empty(),
            "{} no static ability",
            card.id
        );
        assert!(f.triggered_abilities.is_empty(), "{} no trigger", card.id);
        assert!(f.spell_effect.is_empty(), "{} no spell effect", card.id);

        let [mana] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
        };
        assert_eq!(mana.ability_id.as_str(), "activated_01", "{}", card.id);
        assert_eq!(
            mana.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{} presentation",
            card.id
        );
        assert!(
            matches!(
                mana.costs.as_slice(),
                [AbilityCost::Mana(cost), AbilityCost::SacrificeSelf] if cost.to_string() == "{1}"
            ),
            "{}: printed activation cost",
            card.id
        );
        assert!(
            matches!(
                mana.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 1
                        && (options[0].w, options[0].u, options[0].b,
                            options[0].r, options[0].g, options[0].c) == card.produced
            ),
            "{}: printed three mana",
            card.id
        );
        assert!(mana.targeting.is_none(), "{} targetless", card.id);
    }
}

#[test]
fn sac_creature_three_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
