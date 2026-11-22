//! Registry conformance for the queue-v36 damage-and-scry cohort.
use tricerules_cards::primitives::{Amount, SpellEffectKind, TargetKind};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    fingerprint: &'static str,
    types: &'static [&'static str],
    damage: u32,
    scry: u32,
}

const CARDS: &[Card] = &[
    Card {
        id: "bolt_of_keranos",
        name: "Bolt of Keranos",
        mana_cost: "{1}{R}{R}",
        fingerprint: "b7e2e97169f00f87d57cedf3e5def558196dc8887b94cbf8756560e00c027217",
        types: &["Sorcery"],
        damage: 3,
        scry: 1,
    },
    Card {
        id: "fateful_end",
        name: "Fateful End",
        mana_cost: "{2}{R}",
        fingerprint: "b65ec19d3bf058ae9acd556e2ff0b730b69497d68a4085d18ee99119a0224c6c",
        types: &["Instant"],
        damage: 3,
        scry: 1,
    },
    Card {
        id: "lightning_javelin",
        name: "Lightning Javelin",
        mana_cost: "{3}{R}",
        fingerprint: "ba4769b91fa437af13b8fd0166de48f63a323248d8f31be416b90644eef0011a",
        types: &["Sorcery"],
        damage: 3,
        scry: 1,
    },
    Card {
        id: "magma_jet",
        name: "Magma Jet",
        mana_cost: "{1}{R}",
        fingerprint: "55186d38c54eba8e95bfeb226ce701f8ee8b5c296e4053b39c23713929cd149a",
        types: &["Instant"],
        damage: 2,
        scry: 2,
    },
    Card {
        id: "spark_jolt",
        name: "Spark Jolt",
        mana_cost: "{R}",
        fingerprint: "7a75b9720eda3f7bcb6f7beb82e2560d2da987c02fbc1ca70de3529f305b0439",
        types: &["Instant"],
        damage: 1,
        scry: 1,
    },
];

#[test]
fn q36_damage_scry_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.face_id.as_str(), card.id);
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost);
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            card.types.to_vec()
        );
        assert_eq!((f.power, f.toughness), (None, None), "{}", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert_eq!(f.spell_effect.len(), 2, "{}", card.id);
        assert!(
            matches!(
                &f.spell_effect[0],
                SpellEffectKind::DamageTarget {
                    amount: Amount::Fixed(n),
                    target,
                } if *n == card.damage && target.kind == TargetKind::AnyTarget
            ),
            "{}: printed damage to any target",
            card.id
        );
        assert!(
            matches!(
                &f.spell_effect[1],
                SpellEffectKind::Scry { count: Amount::Fixed(n) } if *n == card.scry
            ),
            "{}: printed scry",
            card.id
        );
        let groups = &f.targeting.as_ref().expect("target group").groups;
        assert_eq!(groups.len(), 1, "{}", card.id);
        assert_eq!((groups[0].min, groups[0].max), (1, 1), "{}", card.id);
        assert_eq!(groups[0].effect_indices, [0], "{}", card.id);
    }
}

#[test]
fn q36_damage_scry_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], card.id, "face id drift for {}", card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
