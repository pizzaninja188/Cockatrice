//! Registry conformance for the queue-v31 metalcraft cohort.
use tricerules_cards::primitives::{
    BattlefieldAggregate, GameCondition, RelativePlayerSet, StaticAbilityDef,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    body: (u32, u32),
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "ardent_recruit",
        name: "Ardent Recruit",
        mana: "{W}",
        types: &["Creature", "Human", "Soldier"],
        body: (1, 1),
        fingerprint: "130300829ff3d63494dd7acec639b835b025f9ddebc25584abc39fd796b9b8c3",
    },
    Card {
        id: "carapace_forger",
        name: "Carapace Forger",
        mana: "{1}{G}",
        types: &["Creature", "Elf", "Artificer"],
        body: (2, 2),
        fingerprint: "130300829ff3d63494dd7acec639b835b025f9ddebc25584abc39fd796b9b8c3",
    },
    Card {
        id: "chrome_steed",
        name: "Chrome Steed",
        mana: "{4}",
        types: &["Artifact", "Creature", "Horse"],
        body: (2, 2),
        fingerprint: "130300829ff3d63494dd7acec639b835b025f9ddebc25584abc39fd796b9b8c3",
    },
    Card {
        id: "ghalmas_warden",
        name: "Ghalma's Warden",
        mana: "{3}{W}",
        types: &["Creature", "Elephant", "Soldier"],
        body: (2, 4),
        fingerprint: "130300829ff3d63494dd7acec639b835b025f9ddebc25584abc39fd796b9b8c3",
    },
    Card {
        id: "razorfield_rhino",
        name: "Razorfield Rhino",
        mana: "{6}",
        types: &["Artifact", "Creature", "Rhino"],
        body: (4, 4),
        fingerprint: "130300829ff3d63494dd7acec639b835b025f9ddebc25584abc39fd796b9b8c3",
    },
];

#[test]
fn q31_metalcraft_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        let f = definition.primary_face();
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        assert_eq!(f.face_id.as_str(), card.id.replace('-', "_"));
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana);
        assert_eq!(f.types, card.types, "{}", card.id);
        assert_eq!(
            (f.power, f.toughness),
            (Some(card.body.0), Some(card.body.1))
        );
        assert_eq!(definition.layout, Layout::Normal);
        assert!(f.keywords.is_empty());
        assert!(f.activated_abilities.is_empty());
        assert!(f.triggered_abilities.is_empty());
        assert!(f.spell_effect.is_empty());
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{}: one static", card.id);
        };
        let StaticAbilityDef::ConditionalSelfModifier {
            condition,
            delta_power,
            delta_toughness,
            ..
        } = &static_ability.definition
        else {
            panic!("{}: conditional self modifier", card.id);
        };
        assert_eq!((*delta_power, *delta_toughness), (2, 2), "{}", card.id);
        let GameCondition::BattlefieldAggregate {
            filter,
            aggregate,
            min,
            ..
        } = condition
        else {
            panic!("{}: battlefield aggregate", card.id);
        };
        assert_eq!(*aggregate, BattlefieldAggregate::Count);
        assert_eq!(*min, Some(3));
        assert_eq!(filter.controllers, RelativePlayerSet::Controller);
        assert!(
            filter.matching.permanent_types
                == [tricerules_cards::primitives::PermanentTypeFilter::Artifact]
        );
    }
}

#[test]
fn q31_metalcraft_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5);
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
