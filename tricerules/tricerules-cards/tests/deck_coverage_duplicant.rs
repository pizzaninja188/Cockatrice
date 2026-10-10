#[test]
fn duplicant_has_its_exact_printed_identity() {
    let registry = tricerules_cards::registry::global();
    let definition = registry
        .get("duplicant")
        .expect("Duplicant is implemented in the card registry");
    assert_eq!(registry.id_for_name("Duplicant"), Some("duplicant"));
    assert_eq!(definition.name, "Duplicant");
    assert_eq!(definition.face_count(), 1);

    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), "duplicant");
    assert_eq!(face.mana_cost.to_string(), "{6}");
    assert_eq!(face.types, ["Artifact", "Creature", "Shapeshifter"]);
    assert!(face.supertypes.is_empty());
    assert_eq!((face.power, face.toughness), (Some(2), Some(4)));
    assert!(face.colors().is_empty());
}
