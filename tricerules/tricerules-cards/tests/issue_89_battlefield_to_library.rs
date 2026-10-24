use tricerules_cards::primitives::{
    CompiledInstructionKind, EffectSubject, LibraryPlacement, PermanentTypeFilter, SpellEffectKind,
    TargetKind,
};
use tricerules_cards::ProgramOwnerId;

fn owner_library_effect(id: &str) -> SpellEffectKind {
    let registry = tricerules_cards::registry::global();
    let face = registry.get(id).expect("registered card").primary_face();
    assert!(face.spell_effect.is_empty());
    let program = registry
        .compiled_program_for_owner(&ProgramOwnerId::face(id, face.face_id.clone()))
        .expect("card owns a typed owner-library Program");
    assert_eq!(program.target_bindings.len(), 1);
    let [step] = program.body.steps.as_slice() else {
        panic!("owner-library card has one operation");
    };
    let CompiledInstructionKind::Perform(effect) = &step.kind else {
        panic!("owner-library Program uses a Perform leaf");
    };
    (**effect).clone()
}

#[test]
fn issue_89_cards_share_the_owner_library_placement_primitive() {
    let registry = tricerules_cards::registry::global();

    let totally_lost = registry.get("totally_lost").expect("Totally Lost");
    assert_eq!(totally_lost.primary_face().mana_cost.to_string(), "{4}{U}");
    assert!(matches!(
        owner_library_effect("totally_lost"),
        SpellEffectKind::PutInOwnersLibrary { subject: EffectSubject::Chosen(target), placement }
            if target.kind == TargetKind::AnyPermanent
                && target.excluded_permanent_types == [tricerules_cards::PermanentTypeFilter::Land]
                && placement == LibraryPlacement::Top
    ));

    let griptide = registry.get("griptide").expect("Griptide");
    assert_eq!(griptide.primary_face().mana_cost.to_string(), "{3}{U}");
    assert!(matches!(
        owner_library_effect("griptide"),
        SpellEffectKind::PutInOwnersLibrary { subject: EffectSubject::Chosen(target), placement }
            if target.kind == TargetKind::Creature
                && placement == LibraryPlacement::Top
    ));

    for (id, name) in [
        ("deglamer", "Deglamer"),
        ("unravel_the_aether", "Unravel the Aether"),
    ] {
        let definition = registry.get(id).unwrap_or_else(|| panic!("{name}"));
        assert_eq!(definition.primary_face().mana_cost.to_string(), "{1}{G}");
        assert!(definition.primary_face().spell_effect.is_empty());
        assert!(matches!(
            owner_library_effect(id),
            SpellEffectKind::PutInOwnersLibrary { subject: EffectSubject::Chosen(target), placement }
                if target.kind == TargetKind::AnyPermanent
                    && target.permanent_types
                        == [PermanentTypeFilter::Artifact, PermanentTypeFilter::Enchantment]
                    && placement == LibraryPlacement::Shuffle
        ));
    }
}
