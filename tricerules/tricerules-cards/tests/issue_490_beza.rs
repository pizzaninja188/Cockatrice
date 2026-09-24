//! Complete-card registry evidence for Beza, the Bounding Spring.
//!
//! The exact pinned Oracle identity and current rulings were checked against the 2026-08-25
//! Scryfall corpus on 2026-09-24. CR 608.2c governs the printed order: each comparison is made
//! separately as its effect resolves, and an earlier Treasure token can affect a later creature
//! count if it enters as a creature.

use tricerules_cards::primitives::{
    GameCondition, PlayerComparisonMetric, SpellEffectKind, TriggerCondition,
};
use tricerules_cards::{Amount, CardRegistry};

#[test]
fn issue_490_registers_beza_as_a_complete_standard_identity() {
    let registry = CardRegistry::global();
    let definition = registry
        .get("beza,_the_bounding_spring")
        .expect("Beza is registered");
    assert_eq!(
        registry.id_for_name("Beza, the Bounding Spring"),
        Some("beza,_the_bounding_spring")
    );
    assert_eq!(definition.name, "Beza, the Bounding Spring");
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}{W}{W}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Creature", "Elemental", "Elk"]);
    assert_eq!((face.power, face.toughness), (Some(4), Some(5)));
    assert!(registry.is_token("treasure"));
    assert!(registry.is_token("fish_u_1_1"));

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Beza has one ordered ETB ability");
    };
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(ability.intervening_if, None);
    let expected = [
        (PlayerComparisonMetric::LandCount, 1, "treasure"),
        (PlayerComparisonMetric::LifeTotal, 4, "life"),
        (PlayerComparisonMetric::CreatureCount, 2, "fish_u_1_1"),
        (PlayerComparisonMetric::HandSize, 1, "draw"),
    ];
    assert_eq!(ability.effect.len(), expected.len());
    for (effect, (metric, count, result)) in ability.effect.iter().zip(expected) {
        let condition = GameCondition::OpponentHasMoreThanYou { metric };
        match (effect, result) {
            (
                SpellEffectKind::CreateTokens {
                    token,
                    count: amount,
                    ..
                },
                token_result,
            ) if token == token_result && token_result != "life" && token_result != "draw" => {
                assert_eq!(
                    amount,
                    &Amount::Conditional {
                        condition,
                        when_true: count,
                        otherwise: 0,
                    }
                );
            }
            (SpellEffectKind::GainLife { amount }, "life") => assert_eq!(
                amount,
                &Amount::Conditional {
                    condition,
                    when_true: count,
                    otherwise: 0,
                }
            ),
            (SpellEffectKind::Draw { count: amount, .. }, "draw") => assert_eq!(
                amount,
                &Amount::Conditional {
                    condition,
                    when_true: count,
                    otherwise: 0,
                }
            ),
            _ => panic!("Beza clause order or result differs at {metric:?}"),
        }
    }
}
