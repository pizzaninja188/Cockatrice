use tricerules_cards::primitives::{TargetController, TargetKind};
use tricerules_cards::{AbilityCost, CardRegistry, CounterKind, CounterRemovalPaymentSource};

#[test]
fn selected_permanent_counter_removal_is_typed_card_data() {
    let cost = ron::from_str::<AbilityCost>(
        r#"RemoveCounters(
            counter: Some(PlusOnePlusOne),
            count: 1,
            payment_source: SelectedPermanent((kind: Creature, controller: You)),
        )"#,
    );

    assert!(
        cost.is_ok(),
        "selected-permanent counter removal must parse: {cost:?}"
    );
    let AbilityCost::RemoveCounters {
        counter,
        count,
        payment_source: CounterRemovalPaymentSource::SelectedPermanent(filter),
    } = cost.expect("checked above")
    else {
        panic!("the selected payment source must survive deserialization");
    };
    assert_eq!(counter, Some(CounterKind::PlusOnePlusOne));
    assert_eq!(count, 1);
    assert_eq!(filter.kind, TargetKind::Creature);
    assert_eq!(filter.controller, TargetController::You);
}

#[test]
fn selected_counter_removal_rejects_ambiguous_kind_and_event_only_controller_context() {
    for payment in [
        "counter: None, count: 1, payment_source: SelectedPermanent((kind: Creature, controller: You))",
        "counter: Some(PlusOnePlusOne), count: 1, payment_source: SelectedPermanent((kind: Creature, controller: DefendingPlayer))",
    ] {
        let card = format!(
            r#"(id: "invalid", name: "Invalid", face_id: "invalid", types: ["Creature"], power: 1, toughness: 1,
                activated_abilities: [(ability_id: "activated_01", presentation: Fallback,
                    costs: [RemoveCounters({payment})], effect: [Draw(count: 1)])])"#
        );
        assert!(
            CardRegistry::from_chunks_and_tokens(&[card.as_str()], &[]).is_err(),
            "invalid selected counter vocabulary must fail registry validation: {payment}"
        );
    }
}

#[test]
fn issue_193_oracle_presentations_have_external_face_fingerprints() {
    let registry = CardRegistry::global();
    for card_id in ["ray_fillet,_man_ray", "sage_of_fables"] {
        let card = registry.get(card_id).expect("issue 193 card");
        let face = card.primary_face();
        let metadata = registry
            .presentation_face(card_id, face.face_id.as_str())
            .expect("OracleLines require external face metadata");
        assert_eq!(metadata.card_name, face.name);
        assert_eq!(metadata.face_name, face.name);
        assert_eq!(metadata.oracle_text_sha256.len(), 64);
    }
}
