use tricerules_cards::primitives::{
    CardTypeFilter, GraveyardDestination, GraveyardOwner, TargetRole, TargetSchema,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, SpellEffectKind, TriggerCondition};

fn assert_recursion_ability(ability: &tricerules_cards::TriggeredAbilityDef, oracle_line: u16) {
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![oracle_line])
    );
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(!ability.may);
    assert!(ability.targeting.is_none());
    let [SpellEffectKind::MoveGraveyardCards {
        filter,
        destination,
        linked_exile_id,
    }] = ability.effect.as_slice()
    else {
        panic!("expected one graveyard-to-hand effect");
    };
    assert_eq!(filter.owner, GraveyardOwner::Controller);
    assert_eq!(
        filter.card.as_ref().and_then(|card| card.card_type),
        Some(CardTypeFilter::InstantOrSorcery)
    );
    assert_eq!(*destination, GraveyardDestination::Hand);
    assert!(linked_exile_id.is_none());

    let schema = TargetSchema::compile(&ability.effect, ability.targeting.as_ref())
        .expect("generated recursion target schema");
    assert_eq!(schema.groups.len(), 1);
    let group = &schema.groups[0];
    assert_eq!((group.min, group.max), (1, 1));
    let [binding] = group.bindings.as_slice() else {
        panic!("expected one graveyard target binding");
    };
    assert!(matches!(
        binding.role,
        TargetRole::GraveyardCard(filter)
            if filter.owner == GraveyardOwner::Controller
                && filter.card.as_ref().and_then(|card| card.card_type)
                    == Some(CardTypeFilter::InstantOrSorcery)
    ));
}

#[test]
fn issue_281_registers_exact_etb_recursion_and_preserves_prowess() {
    let registry = CardRegistry::global();

    let shipwreck = registry
        .get("shipwreck_dowser")
        .expect("Shipwreck Dowser should be generated")
        .primary_face();
    assert_eq!(shipwreck.face_id.as_str(), "shipwreck_dowser");
    assert_eq!(shipwreck.name, "Shipwreck Dowser");
    assert_eq!(shipwreck.mana_cost.to_string(), "{3}{U}{U}");
    assert_eq!(shipwreck.types, ["Creature", "Merfolk", "Wizard"]);
    assert_eq!((shipwreck.power, shipwreck.toughness), (Some(3), Some(3)));
    assert!(shipwreck.keywords.is_empty());
    let [prowess, recursion] = shipwreck.triggered_abilities.as_slice() else {
        panic!("Shipwreck Dowser should preserve Prowess and emit recursion");
    };
    assert_eq!(
        prowess.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        prowess.trigger,
        TriggerCondition::WheneverPlayerCastsSpell { .. }
    ));
    assert_recursion_ability(recursion, 2);

    let zealous = registry
        .get("zealous_lorecaster")
        .expect("Zealous Lorecaster should be generated")
        .primary_face();
    assert_eq!(zealous.face_id.as_str(), "zealous_lorecaster");
    assert_eq!(zealous.name, "Zealous Lorecaster");
    assert_eq!(zealous.mana_cost.to_string(), "{5}{R}");
    assert_eq!(zealous.types, ["Creature", "Giant", "Sorcerer"]);
    assert_eq!((zealous.power, zealous.toughness), (Some(4), Some(4)));
    assert!(zealous.keywords.is_empty());
    let [recursion] = zealous.triggered_abilities.as_slice() else {
        panic!("Zealous Lorecaster should emit one recursion ability");
    };
    assert_recursion_ability(recursion, 1);
}
