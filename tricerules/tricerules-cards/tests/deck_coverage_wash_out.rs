use tricerules_cards::primitives::ResolutionBranchDef;

#[test]
fn chosen_color_return_labels_are_complete_and_reject_extra_constraints() {
    for (color, label) in [
        ("White", "white"),
        ("Blue", "blue"),
        ("Black", "black"),
        ("Red", "red"),
        ("Green", "green"),
    ] {
        let data = format!(
            r#"(branch_id:"chosen_color",presentation:Fallback,cost:None,effects:[ReturnAllToOwnersHand(kind:(kind:AnyPermanent,is_color:Some({color})))])"#
        );
        let branch: ResolutionBranchDef = ron::from_str(&data).unwrap();
        assert_eq!(
            branch.fallback_label(),
            format!("Return all {label} permanents to their owners' hands.")
        );
        for extra in [
            "controller:You,",
            "permanent_types:[Land],",
            "excluded_subtypes:[\"Elemental\"],",
        ] {
            let restricted: ResolutionBranchDef = ron::from_str(
                &data.replace("kind:AnyPermanent,", &format!("kind:AnyPermanent,{extra}")),
            )
            .unwrap();
            assert_eq!(restricted.fallback_label(), "Choice (chosen_color)");
        }
    }
}
