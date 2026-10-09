use tricerules_cards::primitives::{EffectSubject, TargetController, TargetKind};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, AbilitySourceZone, Amount, CountExpression,
    PermanentTypeFilter, SpellCostModifier, SpellEffectKind,
};

#[test]
fn metalwork_registers_exact_single_face_and_both_clauses() {
    let card = tricerules_cards::registry::global()
        .get("metalwork_colossus")
        .expect("the exact missing Metalwork Colossus must be implemented");
    let face = card.primary_face();
    assert_eq!(card.name, "Metalwork Colossus");
    assert_eq!(face.mana_cost.to_string(), "{11}");
    assert_eq!(face.types, ["Artifact", "Creature", "Construct"]);
    assert_eq!((face.power, face.toughness), (Some(10), Some(10)));
    assert_eq!(face.cost_modifiers.len(), 1);
    assert_eq!(face.activated_abilities.len(), 1);
    assert_eq!(card.faces_iter().count(), 1);
    assert!(face.colors().is_empty() && face.supertypes.is_empty() && face.keywords.is_empty());
    assert_eq!(
        face.cost_modifiers,
        [SpellCostModifier::GenericReduction {
            amount: Amount::Count(CountExpression::ControlledNoncreatureArtifactManaValueSum),
        }]
    );
    let ability = &face.activated_abilities[0];
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(ability.source_zone, AbilitySourceZone::Graveyard);
    let [AbilityCost::SacrificePermanent { filter, count }] = ability.costs.as_slice() else {
        panic!("exact counted artifact sacrifice cost required")
    };
    assert_eq!(*count, 2);
    assert_eq!(filter.kind, TargetKind::AnyPermanent);
    assert_eq!(filter.controller, TargetController::You);
    assert_eq!(filter.permanent_types, [PermanentTypeFilter::Artifact]);
    assert_eq!(
        ability.effect,
        [SpellEffectKind::ReturnToOwnersHand {
            subject: EffectSubject::Source,
        }]
    );
}
