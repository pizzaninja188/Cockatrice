//! Registry conformance for the queue-v32 +N/-N Aura cohort.
use tricerules_cards::primitives::{SpellEffectKind, StaticAbilityDef, TargetKind};
use tricerules_cards::{AbilityPresentation, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    delta: (i32, i32),
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "boon_of_emrakul",
        name: "Boon of Emrakul",
        mana_cost: "{2}{B}",
        delta: (3, -3),
        fingerprint: "2955d285ea6f996b2694a6e557678c69c6e4896863a5b9d6cc2f63662bd9265a",
    },
    Card {
        id: "immolation",
        name: "Immolation",
        mana_cost: "{R}",
        delta: (2, -2),
        fingerprint: "f9d7f257168d6806b44f869f7392c574cc0a2ad04e8c807f2ed14c03b2bd598c",
    },
    Card {
        id: "sangrite_backlash",
        name: "Sangrite Backlash",
        mana_cost: "{B/G}{R}",
        delta: (3, -3),
        fingerprint: "2955d285ea6f996b2694a6e557678c69c6e4896863a5b9d6cc2f63662bd9265a",
    },
    Card {
        id: "twisted_experiment",
        name: "Twisted Experiment",
        mana_cost: "{1}{B}",
        delta: (3, -1),
        fingerprint: "16066ba95406c3a806b8cda9553358b9585bfbcedc6abf5b7339e0edfc6b8e98",
    },
];

#[test]
fn q32_plus_minus_auras_batch_maps_definitions() {
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
        assert_eq!(f.types, ["Enchantment", "Aura"]);
        assert_eq!((f.power, f.toughness), (None, None));
        assert!(f.keywords.is_empty());
        assert!(f.triggered_abilities.is_empty());
        assert!(f.activated_abilities.is_empty());
        assert!(f.targeting.is_none());
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
        assert!(
            matches!(
                &static_ability.definition,
                StaticAbilityDef::AttachedModifier { delta_power, delta_toughness, keywords, .. }
                    if *delta_power == card.delta.0
                        && *delta_toughness == card.delta.1
                        && keywords.is_empty()
            ),
            "{}: printed modifier",
            card.id
        );
    }
}

#[test]
fn q32_plus_minus_auras_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
