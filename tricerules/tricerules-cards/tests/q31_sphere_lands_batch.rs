//! Registry conformance for the queue-v31 Sphere land cycle.
use tricerules_cards::primitives::{
    AbilityCost, Amount, PlayerRecipient, SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::Layout;
use tricerules_cards::ManaCost;

struct Card {
    id: &'static str,
    name: &'static str,
    color: char,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "the_dross_pits",
        name: "The Dross Pits",
        color: 'b',
        fingerprint: "24d51a755eff7deecf62dbad7cd50a91717a0069c0fc980f103a099fc9a5031c",
    },
    Card {
        id: "the_fair_basilica",
        name: "The Fair Basilica",
        color: 'w',
        fingerprint: "68ca917867e79ee7a6814e787d995f8c787de77b3ee4a5a2ef0d5532b4df61ee",
    },
    Card {
        id: "the_hunter_maze",
        name: "The Hunter Maze",
        color: 'g',
        fingerprint: "f169b3c99047d4e0b4c7c498479c278728dcff3e934606186259f9041d1e79ab",
    },
    Card {
        id: "the_surgical_bay",
        name: "The Surgical Bay",
        color: 'u',
        fingerprint: "080609cd1348e5a8a68c2e89e8b9907a61bd05c053942ce649da4df7e73c71a5",
    },
    Card {
        id: "the_autonomous_furnace",
        name: "The Autonomous Furnace",
        color: 'r',
        fingerprint: "a8f7f03b41ef420ea11b5ad6b4ce2393dd6aef679a99a03071afeaeddd14347d",
    },
];

#[test]
fn q31_sphere_lands_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        let f = definition.primary_face();
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        assert_eq!(f.face_id.as_str(), card.id.replace('-', "_"));
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), "");
        assert_eq!(f.types, ["Land", "Sphere"], "{}", card.id);
        assert_eq!(definition.layout, Layout::Normal);
        assert!(f.triggered_abilities.is_empty());
        assert!(f.spell_effect.is_empty());
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{}: one static", card.id);
        };
        assert!(matches!(
            static_ability.definition,
            StaticAbilityDef::EntersTapped {
                affected: tricerules_cards::primitives::EntersTappedAffected::Self_,
                ..
            }
        ));
        let expected_color = match card.color {
            'w' => tricerules_cards::primitives::Color::White,
            'u' => tricerules_cards::primitives::Color::Blue,
            'b' => tricerules_cards::primitives::Color::Black,
            'r' => tricerules_cards::primitives::Color::Red,
            'g' => tricerules_cards::primitives::Color::Green,
            _ => unreachable!(),
        };
        assert_eq!(
            definition.color_identity(),
            vec![expected_color],
            "{}: Commander color identity",
            card.id
        );
        assert_eq!(f.activated_abilities.len(), 2, "{}", card.id);
        let mana = &f.activated_abilities[0];
        assert_eq!(mana.costs, [AbilityCost::Tap]);
        let [SpellEffectKind::ProduceMana { options, .. }] = mana.effect.as_slice() else {
            panic!("{}: mana", card.id);
        };
        assert_eq!(options.len(), 1, "{}", card.id);
        let sacrifice = &f.activated_abilities[1];
        assert_eq!(sacrifice.costs.len(), 3, "{}", card.id);
        assert_eq!(
            sacrifice.costs[0],
            AbilityCost::Mana(
                ManaCost::parse(&format!("{{1}}{{{}}}", card.color.to_ascii_uppercase())).unwrap()
            ),
            "{}: exact sacrifice cost",
            card.id
        );
        assert_eq!(sacrifice.costs[1], AbilityCost::Tap);
        assert_eq!(sacrifice.costs[2], AbilityCost::SacrificeSelf);
        assert!(matches!(
            sacrifice.effect.as_slice(),
            [SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }]
        ));
    }
}

#[test]
fn q31_sphere_lands_batch_fingerprints_match_the_pinned_oracle_text() {
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
