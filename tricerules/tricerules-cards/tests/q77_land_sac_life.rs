//! Registry conformance for the queue-v77 02-land-sac-life cohort.
//!
//! Every card has one activated ability whose only cost is sacrificing a matching land and whose
//! only effect is life gain. Fingerprints are checked after the presentation metadata refresh.

use tricerules_cards::primitives::{
    AbilityCost, Amount, PermanentTypeFilter, PlayerRecipient, SpellEffectKind, TargetController,
    TargetFilter, TargetKind,
};
use tricerules_cards::{AbilityPresentation, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    life: u32,
    forest_only: bool,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "zuran_orb",
        name: "Zuran Orb",
        mana_cost: "{0}",
        types: &["Artifact"],
        life: 2,
        forest_only: false,
        fingerprint: "b7c7a3c5c35970b2e4bc2d5fd18ebf3906a96e6170b0482ee18b0412154e26ed",
    },
    Card {
        id: "overgrown_estate",
        name: "Overgrown Estate",
        mana_cost: "{W}{B}{G}",
        types: &["Enchantment"],
        life: 3,
        forest_only: false,
        fingerprint: "6b3dea61f94c5080bef0932dbb5514f5a7c6647171277b3836f26aafeee77105",
    },
    Card {
        id: "dark_heart_of_the_wood",
        name: "Dark Heart of the Wood",
        mana_cost: "{B}{G}",
        types: &["Enchantment"],
        life: 3,
        forest_only: true,
        fingerprint: "065e328633f132a3b50df90d4031d2d3396b27f0b93b2a49d9ef159645c29a16",
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

#[test]
fn q77_land_sac_life_map_definitions() {
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
        assert_eq!(f.mana_cost.to_string(), card.mana_cost, "{} mana", card.id);
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert_eq!((f.power, f.toughness), (None, None), "{} body", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        assert!(f.targeting.is_none(), "{}", card.id);

        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{}", card.id);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{}",
            card.id
        );
        assert_eq!(
            ability.effect.as_slice(),
            [SpellEffectKind::GainLife {
                amount: Amount::Fixed(card.life),
                who: PlayerRecipient::Controller,
            }],
            "{}",
            card.id
        );
        let [AbilityCost::SacrificePermanent { filter, count }] = ability.costs.as_slice() else {
            panic!("{}: exactly one sacrifice cost", card.id);
        };
        assert_eq!(*count, 1, "{}", card.id);
        let expected = TargetFilter {
            kind: TargetKind::AnyPermanent,
            controller: TargetController::You,
            permanent_types: vec![PermanentTypeFilter::Land],
            required_subtypes: if card.forest_only {
                vec!["Forest".to_string()]
            } else {
                Vec::new()
            },
            ..Default::default()
        };
        assert_eq!(filter, &expected, "{} sacrifice filter", card.id);
    }
}

#[test]
fn q77_land_sac_life_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], card.id, "face id drift for {}", card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
