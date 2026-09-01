use tricerules_cards::{
    AbilityCost, AdditionalCost, CardRegistry, ObjectContributionKind, ObjectPaymentConstraint,
};

#[test]
fn aggregate_object_payments_are_typed_card_data() {
    let tap = ron::from_str::<AbilityCost>(
        r#"TapPermanents(constraint: AggregateMinimum(minimum: 10, contribution: CurrentPower), filter: (kind: Creature, controller: You), exclude_source: true)"#,
    );
    assert!(tap.is_ok(), "total-power tap cost must parse: {tap:?}");

    let exile = ron::from_str::<AbilityCost>(
        r#"ExileGraveyardCards(constraint: AggregateMinimum(minimum: 3, contribution: ManaValue), filter: (), exclude_source: false)"#,
    );
    assert!(
        exile.is_ok(),
        "collect-evidence activation cost must parse: {exile:?}"
    );

    let spell = ron::from_str::<AdditionalCost>(
        r#"ExileGraveyardCards(constraint: AggregateMinimum(minimum: 3, contribution: ManaValue), filter: (), exclude_source: false)"#,
    );
    assert!(
        spell.is_ok(),
        "collect-evidence spell cost must parse: {spell:?}"
    );
}

#[test]
fn issue_178_cards_use_the_shared_aggregate_constraints() {
    let registry = CardRegistry::global();
    let forensic = registry
        .get("forensic_researcher")
        .expect("Forensic Researcher");
    assert!(matches!(
        forensic.primary_face().activated_abilities[1].costs[1],
        AbilityCost::ExileGraveyardCards {
            constraint: ObjectPaymentConstraint::AggregateMinimum {
                minimum: 3,
                contribution: ObjectContributionKind::ManaValue,
            },
            ..
        }
    ));

    let mossbridge = registry.get("mossbridge_troll").expect("Mossbridge Troll");
    assert!(matches!(
        mossbridge.primary_face().activated_abilities[0].costs[0],
        AbilityCost::TapPermanents {
            constraint: ObjectPaymentConstraint::AggregateMinimum {
                minimum: 10,
                contribution: ObjectContributionKind::CurrentPower,
            },
            exclude_source: true,
            ..
        }
    ));
}
