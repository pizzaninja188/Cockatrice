//! Registry conformance for the queue-v28 +X/+0 pump cohort.
//!
//! Each card is a normal-layout printed spell whose complete typed shape is asserted
//! against the pinned Oracle text. Fingerprints are SHA-256 of the exact normalized
//! Oracle lines.

use tricerules_cards::primitives::{
    Amount, EffectSubject, PtScale, PtScaleBasis, SpellEffectKind, TargetGroupDef, TargetKind,
    TargetingDef,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "enrage",
        name: "Enrage",
        mana: "{X}{R}",
        types: &["Instant"],
        fingerprint: "f69a686704164f395d76c853cdf8b4cb0cb7a919a23fbda8265f2358d88358ae",
    },
    Card {
        id: "bloodcurdling_scream",
        name: "Bloodcurdling Scream",
        mana: "{X}{B}",
        types: &["Sorcery"],
        fingerprint: "f69a686704164f395d76c853cdf8b4cb0cb7a919a23fbda8265f2358d88358ae",
    },
    Card {
        id: "howl_from_beyond",
        name: "Howl from Beyond",
        mana: "{X}{B}",
        types: &["Instant"],
        fingerprint: "f69a686704164f395d76c853cdf8b4cb0cb7a919a23fbda8265f2358d88358ae",
    },
];

fn face<'a>(
    registry: &'a tricerules_cards::CardRegistry,
    id: &str,
) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

fn single_group(targeting: &TargetingDef) -> &TargetGroupDef {
    let [group] = targeting.groups.as_slice() else {
        panic!("expected exactly one target group");
    };
    group
}

#[test]
fn x_pumps_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(
            registry.id_for_name(card.name),
            Some(card.id),
            "{}",
            card.id
        );
        let f = face(registry, card.id);
        assert_eq!(f.face_id.as_str(), card.id, "{} face id", card.id);
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(f.mana_cost.to_string(), card.mana, "{} mana", card.id);
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert!(f.activated_abilities.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        let [effect] = f.spell_effect.as_slice() else {
            panic!("{}: exactly one spell effect", card.id);
        };
        let SpellEffectKind::PumpTarget {
            power,
            toughness,
            scale,
            subject,
        } = effect
        else {
            panic!("{}: pump target", card.id);
        };
        assert_eq!((*power, *toughness), (0, 0), "{}", card.id);
        assert!(
            matches!(subject, EffectSubject::Chosen(filter) if filter.kind == TargetKind::Creature),
            "{}: chosen creature",
            card.id
        );
        assert_eq!(
            scale,
            &Some(PtScale {
                basis: PtScaleBasis::Amount(Amount::X),
                power_per_unit: 1,
                toughness_per_unit: 0,
            }),
            "{}",
            card.id
        );
        let targeting = f
            .targeting
            .as_ref()
            .unwrap_or_else(|| panic!("{}: needs one target group", card.id));
        let group = single_group(targeting);
        assert_eq!((group.min, group.max), (1, 1), "{}", card.id);
        assert_eq!(group.effect_indices, [0], "{}", card.id);
    }
}

#[test]
fn x_pumps_batch_fingerprints_match_the_pinned_oracle_text() {
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
