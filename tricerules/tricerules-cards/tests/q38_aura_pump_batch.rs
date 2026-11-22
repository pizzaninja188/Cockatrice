//! Registry conformance for the queue-v38 09-aura-pump cohort.
//! Printed identity, costs, typed shape and pinned fingerprints.

use tricerules_cards::primitives::{
    AbilityCost, EffectSubject, SpellEffectKind, StaticAbilityDef, TargetKind,
};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: Option<u32>,
    toughness: Option<u32>,
    static_pump: (i32, i32),
    activated_pump: (i32, i32),
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "armor_of_faith",
        name: "Armor of Faith",
        mana_cost: "{W}",
        types: &["Enchantment", "Aura"],
        power: None,
        toughness: None,
        static_pump: (1, 1),
        activated_pump: (0, 1),
        fingerprint: "efa1958a874919cfda881347c556fbf1201c13c5fad60a08625907cc23cc7513",
    },
    Card {
        id: "holy_armor",
        name: "Holy Armor",
        mana_cost: "{W}",
        types: &["Enchantment", "Aura"],
        power: None,
        toughness: None,
        static_pump: (0, 2),
        activated_pump: (0, 1),
        fingerprint: "71dc11f5e87873cd0a0721c672b991f702f69ea701025424fbbb496c62f39981",
    },
    Card {
        id: "stonehands",
        name: "Stonehands",
        mana_cost: "{2}{R}",
        types: &["Enchantment", "Aura"],
        power: None,
        toughness: None,
        static_pump: (0, 2),
        activated_pump: (1, 0),
        fingerprint: "d2c6df330f00d0ca901a7112fd95a65e56de52cea92f8c0d296fe6f90010d48b",
    },
];

use tricerules_cards::{AbilityPresentation, Layout};
#[test]
fn q38_aura_pump_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.face_id.as_str(), card.id.replace('-', "_"));
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost);
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            card.types
        );
        assert_eq!((f.power, f.toughness), (card.power, card.toughness));
        assert!(f.keywords.is_empty());
        assert!(f.triggered_abilities.is_empty());
        let [SpellEffectKind::AuraAttach { target }] = f.spell_effect.as_slice() else {
            panic!("{}: one AuraAttach", card.id)
        };
        assert_eq!(target.kind, TargetKind::Creature, "{}", card.id);
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{}: one static ability", card.id)
        };
        assert_eq!(
            static_ability.presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        assert!(matches!(
            &static_ability.definition,
            StaticAbilityDef::AttachedModifier { delta_power, delta_toughness, .. }
                if *delta_power == card.static_pump.0 && *delta_toughness == card.static_pump.1
        ));
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: one activated ability", card.id)
        };
        assert!(matches!(ability.costs.as_slice(), [AbilityCost::Mana(_)]));
        assert!(matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::PumpTarget { power, toughness, subject: EffectSubject::AttachedObject, .. }]
                if *power == card.activated_pump.0 && *toughness == card.activated_pump.1
        ));
    }
}

#[test]
fn q38_aura_pump_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "{}", card.id);
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.id.replace('-', "_"));
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
