use tricerules_cards::registry;

#[test]
fn the_mycosynth_gardens_is_registered_as_a_complete_card() {
    let card = registry::global()
        .get("the_mycosynth_gardens")
        .expect("The Mycosynth Gardens must have a complete ruled-card definition");

    assert_eq!(card.name, "The Mycosynth Gardens");
    let face = card.face(0).expect("the land face is present");
    assert_eq!(face.name, "The Mycosynth Gardens");
    assert_eq!(face.types, ["Land", "Sphere"]);
    assert_eq!(face.activated_abilities.len(), 3);
}
