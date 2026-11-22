//! Registry conformance for the queue-v32 self-sacrifice ping cohort.
use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    body: Option<(u32, u32)>,
    amount: u32,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "mogg_fanatic",
        name: "Mogg Fanatic",
        mana_cost: "{R}",
        types: &["Creature", "Goblin"],
        body: Some((1, 1)),
        amount: 1,
        fingerprint: "a2a953fd2207b880535299ee3a8c062c0bdd7f77a7431a31b42fc75673d0d7dc",
    },
    Card {
        id: "moonglove_extract",
        name: "Moonglove Extract",
        mana_cost: "{3}",
        types: &["Artifact"],
        body: None,
        amount: 2,
        fingerprint: "964659ef806c3edbc6f60dec3a31d40b793481d84edba1f2b7afbc0d0eb7540c",
    },
    Card {
        id: "seal_of_fire",
        name: "Seal of Fire",
        mana_cost: "{R}",
        types: &["Enchantment"],
        body: None,
        amount: 2,
        fingerprint: "69ad86ca00bf6ac2b26b2d7d0514894ac175556f4020b0f400110c217eb1c239",
    },
    Card {
        id: "shock_troops",
        name: "Shock Troops",
        mana_cost: "{3}{R}",
        types: &["Creature", "Human", "Soldier"],
        body: Some((2, 2)),
        amount: 2,
        fingerprint: "6cb1745bd621840bd2a652f05a395e1cda94f8ed6b0b78d50f15d2ddce9350dc",
    },
];

#[test]
fn q32_self_sac_ping_batch_maps_definitions() {
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
        assert_eq!(ability.costs, [AbilityCost::SacrificeSelf]);
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::DamageTarget { amount, target }]
                    if amount == &tricerules_cards::primitives::Amount::Fixed(card.amount)
                        && target.kind == tricerules_cards::primitives::TargetKind::AnyTarget
            ),
            "{}: any-target damage",
            card.id
        );
    }
}

#[test]
fn q32_self_sac_ping_batch_fingerprints_match_the_pinned_oracle_text() {
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
