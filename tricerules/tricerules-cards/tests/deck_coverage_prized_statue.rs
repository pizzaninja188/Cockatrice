use tricerules_cards::primitives::{PlayerRecipient, SpellEffectKind, TriggerCondition};
use tricerules_cards::Amount;

const TREASURE_TOKEN: &str = "treasure";

#[test]
fn prized_statue_registers_its_entry_and_graveyard_triggers() {
    let registry = tricerules_cards::registry::global();
    let card = registry.get("prized_statue").expect("Prized Statue");
    let face = card.primary_face();
    assert_eq!(face.name, "Prized Statue");
    assert_eq!(face.mana_cost.to_string(), "{2}");
    assert_eq!(face.types, ["Artifact"]);

    let [enters, goes_to_graveyard] = face.triggered_abilities.as_slice() else {
        panic!("Prized Statue has an entry trigger and a graveyard trigger");
    };
    assert_eq!(enters.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(goes_to_graveyard.trigger, TriggerCondition::WhenSelfDies);
    for trigger in [enters, goes_to_graveyard] {
        assert!(matches!(
            trigger.effect.as_slice(),
            [SpellEffectKind::CreateTokens {
                token,
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                ..
            }] if token == TREASURE_TOKEN
        ));
    }
}
