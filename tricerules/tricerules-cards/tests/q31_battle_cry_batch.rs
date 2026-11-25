//! Registry conformance for the queue-v31 battle cry cohort.
use tricerules_cards::primitives::{CreatureScopeFilter, SpellEffectKind, TriggerCondition};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    body: (u32, u32),
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "accorder_paladin",
        name: "Accorder Paladin",
        mana: "{1}{W}",
        body: (3, 1),
        fingerprint: "77abaff7c9ae487b4665831b039b513a7cb50ba96e860165cdd7a9cbdeb34619",
    },
    Card {
        id: "goblin_wardriver",
        name: "Goblin Wardriver",
        mana: "{R}{R}",
        body: (2, 2),
        fingerprint: "77abaff7c9ae487b4665831b039b513a7cb50ba96e860165cdd7a9cbdeb34619",
    },
    Card {
        id: "loxodon_partisan",
        name: "Loxodon Partisan",
        mana: "{4}{W}",
        body: (3, 4),
        fingerprint: "77abaff7c9ae487b4665831b039b513a7cb50ba96e860165cdd7a9cbdeb34619",
    },
];

#[test]
fn q31_battle_cry_batch_maps_definitions() {
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
        assert_eq!(
            (f.power, f.toughness),
            (Some(card.body.0), Some(card.body.1))
        );
        assert_eq!(definition.layout, Layout::Normal);
        assert!(f.keywords.is_empty());
        assert!(f.activated_abilities.is_empty());
        assert!(f.spell_effect.is_empty());
        let [triggered] = f.triggered_abilities.as_slice() else {
            panic!("{}: one trigger", card.id)
        };
        assert_eq!(
            triggered.trigger,
            TriggerCondition::WheneverSelfAttacks {
                minimum_other_attackers: 0
            }
        );
        let [SpellEffectKind::PumpAll {
            filter,
            power,
            toughness,
        }] = triggered.effect.as_slice()
        else {
            panic!("{}: pump all attackers", card.id);
        };
        assert_eq!((*power, *toughness), (1, 0));
        assert_eq!(
            *filter,
            CreatureScopeFilter {
                exclude_self: true,
                matching: Box::new(tricerules_cards::primitives::TargetFilter {
                    kind: tricerules_cards::primitives::TargetKind::Creature,
                    combat_role: Some(tricerules_cards::primitives::CombatRole::Attacking),
                    ..Default::default()
                }),
                ..CreatureScopeFilter::default()
            }
        );
    }
}

#[test]
fn q31_battle_cry_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap();
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
