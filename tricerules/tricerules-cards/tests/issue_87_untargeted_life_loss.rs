use tricerules_cards::primitives::{LifeAmount, PlayerRecipient, SpellEffectKind};
use tricerules_cards::{CardRegistry, TriggerCondition};

#[test]
fn infectious_horror_has_complete_oracle_behavior() {
    let definition = CardRegistry::global()
        .get("infectious_horror")
        .expect("Infectious Horror must be registered");
    let face = definition.primary_face();

    assert_eq!(definition.name, "Infectious Horror");
    assert_eq!(face.mana_cost.to_string(), "{3}{B}");
    assert_eq!(face.types, ["Creature", "Zombie", "Horror"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(2)));
    assert!(definition.partial.is_none());
    assert_eq!(face.triggered_abilities.len(), 1);
    assert_eq!(
        face.triggered_abilities[0].trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0
        }
    );
    assert_eq!(
        face.triggered_abilities[0].effect,
        [SpellEffectKind::LoseLife {
            amount: LifeAmount::Fixed(2),
            who: PlayerRecipient::EachOpponent,
        }]
    );
}
