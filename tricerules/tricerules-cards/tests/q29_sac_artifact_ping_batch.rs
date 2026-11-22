//! Registry conformance for the queue-v29 artifact-sacrifice ping cohort.
use tricerules_cards::primitives::{
    AbilityCost, Amount, PermanentTypeFilter, SpellEffectKind, TargetController, TargetFilter,
    TargetKind,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
}

const CARDS: &[Card] = &[
    Card {
        id: "orcish_mechanics",
        name: "Orcish Mechanics",
        mana: "{2}{R}",
        types: &["Creature", "Orc"],
        power: 1,
        toughness: 1,
    },
    Card {
        id: "orcish_vandal",
        name: "Orcish Vandal",
        mana: "{1}{R}",
        types: &["Creature", "Orc", "Warrior"],
        power: 1,
        toughness: 1,
    },
    Card {
        id: "barrage_ogre",
        name: "Barrage Ogre",
        mana: "{3}{R}{R}",
        types: &["Creature", "Ogre", "Warrior"],
        power: 3,
        toughness: 3,
    },
];

fn artifact_cost() -> AbilityCost {
    AbilityCost::SacrificePermanent {
        filter: TargetFilter {
            kind: TargetKind::AnyPermanent,
            controller: TargetController::You,
            permanent_types: vec![PermanentTypeFilter::Artifact],
            ..TargetFilter::default()
        },
        count: 1,
    }
}

#[test]
fn q29_sac_artifact_ping_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        let f = definition.primary_face();
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        assert_eq!(f.face_id.as_str(), card.id, "{} face id", card.id);
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(f.mana_cost.to_string(), card.mana, "{} mana", card.id);
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert_eq!(f.power, Some(card.power), "{} power", card.id);
        assert_eq!(f.toughness, Some(card.toughness), "{} toughness", card.id);
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        assert!(f.targeting.is_none(), "{}", card.id);
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
        };
        assert_eq!(
            ability.costs,
            [AbilityCost::Tap, artifact_cost()],
            "{}",
            card.id
        );
        assert!(ability.targeting.is_some(), "{}", card.id);
        let [effect] = ability.effect.as_slice() else {
            panic!("{}: exactly one effect", card.id);
        };
        let SpellEffectKind::DamageTarget { amount, target } = effect else {
            panic!("{}: damage target", card.id);
        };
        assert_eq!(*amount, Amount::Fixed(2), "{}", card.id);
        assert_eq!(target.kind, TargetKind::AnyTarget, "{}", card.id);
        let targeting = ability.targeting.as_ref().expect("targeting");
        let [group] = targeting.groups.as_slice() else {
            panic!("{}: one target group", card.id);
        };
        assert_eq!((group.min, group.max), (1, 1), "{}", card.id);
        assert_eq!(group.effect_indices, [0], "{}", card.id);
    }
}

#[test]
fn q29_sac_artifact_ping_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, expected) in [
        (
            "orcish_mechanics",
            "Orcish Mechanics",
            "a379c5be821eac4c0bda0b596b64e9070da77e155a0f07388b1e25544d854615",
        ),
        (
            "orcish_vandal",
            "Orcish Vandal",
            "a379c5be821eac4c0bda0b596b64e9070da77e155a0f07388b1e25544d854615",
        ),
        (
            "barrage_ogre",
            "Barrage Ogre",
            "a379c5be821eac4c0bda0b596b64e9070da77e155a0f07388b1e25544d854615",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id.replace('-', "_"));
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
