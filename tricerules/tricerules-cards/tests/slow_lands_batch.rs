//! Registry conformance for the reviewed slow-land cycle.
//!
//! Every included card is a normal-layout land that enters tapped unless its
//! controller has two or more other lands, then has a two-option `{T}` mana
//! ability. Each fingerprint is SHA-256 of the exact pinned Oracle lines.

use tricerules_cards::primitives::{
    AbilityCost, BattlefieldAggregate, CardTypeFilter, EntersTappedAffected, GameCondition,
    RelativePlayerSet, SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::{AbilityPresentation, Layout};

type Mana = (u32, u32, u32, u32, u32, u32);

struct Card {
    id: &'static str,
    name: &'static str,
    options: [Mana; 2],
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "deserted_beach",
        name: "Deserted Beach",
        options: [(1, 0, 0, 0, 0, 0), (0, 1, 0, 0, 0, 0)],
        fingerprint: "acbc370e2672670d7b879a0bb4e3dbc46c58c074d4f449e1d248aa5636064494",
    },
    Card {
        id: "haunted_ridge",
        name: "Haunted Ridge",
        options: [(0, 0, 1, 0, 0, 0), (0, 0, 0, 1, 0, 0)],
        fingerprint: "46662ba273d097518f63a14ef94bef1d98de9df69c3c61c7fe3362c3f35e9fc8",
    },
    Card {
        id: "overgrown_farmland",
        name: "Overgrown Farmland",
        options: [(0, 0, 0, 0, 1, 0), (1, 0, 0, 0, 0, 0)],
        fingerprint: "c62b2cd0cad0b0eae88d42920abc632b82fd744baaf83eaf8f4ef2898bde2b42",
    },
    Card {
        id: "rockfall_vale",
        name: "Rockfall Vale",
        options: [(0, 0, 0, 1, 0, 0), (0, 0, 0, 0, 1, 0)],
        fingerprint: "593bb328d8a22257c17920c28bd781d6eeb5c4ff5d4a0a5bf4e4a5a144605313",
    },
    Card {
        id: "shipwreck_marsh",
        name: "Shipwreck Marsh",
        options: [(0, 1, 0, 0, 0, 0), (0, 0, 1, 0, 0, 0)],
        fingerprint: "b1d57599fa84359169e826ee7f482c385e3552048224797e576020ba6cb631c9",
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
fn slow_lands_batch_maps_definitions() {
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
        assert_eq!(f.mana_cost.to_string(), "", "{} mana cost", card.id);
        assert_eq!(f.types, ["Land"], "{} types", card.id);
        assert!(f.supertypes.is_empty(), "{}", card.id);
        assert_eq!(f.power, None, "{}", card.id);
        assert_eq!(f.toughness, None, "{}", card.id);
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);
        assert!(f.targeting.is_none(), "{}", card.id);

        let [entry] = f.static_abilities.as_slice() else {
            panic!("{}: exactly one static ability", card.id);
        };
        assert_eq!(entry.ability_id.as_str(), "static_01", "{}", card.id);
        assert_eq!(
            entry.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{} entry presentation",
            card.id
        );
        let StaticAbilityDef::EntersTapped {
            affected: EntersTappedAffected::Self_,
            condition:
                Some(GameCondition::BattlefieldAggregate {
                    filter,
                    aggregate: BattlefieldAggregate::Count,
                    min,
                    max,
                    ..
                }),
            unless_cost: None,
        } = &entry.definition
        else {
            panic!("{}: conditional self enters-tapped", card.id);
        };
        assert_eq!(
            filter.controllers,
            RelativePlayerSet::Controller,
            "{}",
            card.id
        );
        assert_eq!(filter.card_type, Some(CardTypeFilter::Land), "{}", card.id);
        assert!(filter.exclude_source, "{}", card.id);
        assert_eq!((*min, *max), (None, Some(1)), "{}", card.id);

        let [mana] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
        };
        assert_eq!(mana.ability_id.as_str(), "activated_01", "{}", card.id);
        assert_eq!(
            mana.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{} mana presentation",
            card.id
        );
        assert_eq!(mana.costs, [AbilityCost::Tap], "{} mana cost", card.id);
        assert!(
            matches!(
                mana.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 2
                        && [(options[0].w, options[0].u, options[0].b,
                             options[0].r, options[0].g, options[0].c),
                            (options[1].w, options[1].u, options[1].b,
                             options[1].r, options[1].g, options[1].c)] == card.options
            ),
            "{} mana options",
            card.id
        );
        assert!(mana.targeting.is_none(), "{}", card.id);
    }
}

#[test]
fn slow_lands_batch_fingerprints_match_the_pinned_oracle_text() {
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
