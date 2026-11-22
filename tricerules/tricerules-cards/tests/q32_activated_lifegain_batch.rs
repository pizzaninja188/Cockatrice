//! Registry conformance for the queue-v32 activated life-gain cohort.
use tricerules_cards::primitives::{AbilityCost, Amount, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    body: Option<(u32, u32)>,
    activation: &'static str,
    amount: u32,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "fountain_of_youth",
        name: "Fountain of Youth",
        mana_cost: "{0}",
        types: &["Artifact"],
        body: None,
        activation: "{2}",
        amount: 1,
        fingerprint: "2d5ad87bfd6ae4e8150dd3c8b52edcf2376fe0cdcbcaa2c320aa4246682fd0d1",
    },
    Card {
        id: "stone_haven_medic",
        name: "Stone Haven Medic",
        mana_cost: "{1}{W}",
        types: &["Creature", "Kor", "Cleric"],
        body: Some((1, 3)),
        activation: "{W}",
        amount: 1,
        fingerprint: "393fda746fb256a583eed2d9f559e311e35d597fa7a404d4b8357f6f91e3f98e",
    },
    Card {
        id: "tanglebloom",
        name: "Tanglebloom",
        mana_cost: "{1}",
        types: &["Artifact"],
        body: None,
        activation: "{1}",
        amount: 1,
        fingerprint: "0fb0d817204d585b28b5cec2eb9f84fc538b84cb05adbff2af7f4bef8022e1c3",
    },
    Card {
        id: "tower_of_eons",
        name: "Tower of Eons",
        mana_cost: "{4}",
        types: &["Artifact"],
        body: None,
        activation: "{8}",
        amount: 10,
        fingerprint: "4aee9613b8d1be843a1f09e64e8940356dc2ee6ccada4bb12ef5bc2e70499159",
    },
];

#[test]
fn q32_activated_lifegain_batch_maps_definitions() {
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
            card.types.to_vec()
        );
        assert_eq!(
            (f.power, f.toughness),
            match card.body {
                Some((power, toughness)) => (Some(power), Some(toughness)),
                None => (None, None),
            }
        );
        assert!(f.keywords.is_empty());
        assert!(f.triggered_abilities.is_empty());
        assert!(f.static_abilities.is_empty());
        assert!(f.spell_effect.is_empty());
        assert!(f.targeting.is_none());
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: one activated ability", card.id)
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert!(
            matches!(&ability.costs[..], [AbilityCost::Mana(cost), AbilityCost::Tap] if cost.to_string() == card.activation),
            "{}: printed mana plus tap",
            card.id
        );
        assert!(
            matches!(ability.effect.as_slice(), [SpellEffectKind::GainLife { amount, .. }] if amount == &Amount::Fixed(card.amount)),
            "{}: printed life",
            card.id
        );
    }
}

#[test]
fn q32_activated_lifegain_batch_fingerprints_match_the_pinned_oracle_text() {
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
