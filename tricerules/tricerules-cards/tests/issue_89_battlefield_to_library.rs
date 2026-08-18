use tricerules_cards::primitives::{
    LibraryPlacement, PermanentTypeFilter, SpellEffectKind, TargetKind,
};
use tricerules_cards::CardRegistry;

#[test]
fn issue_89_cards_share_the_owner_library_placement_primitive() {
    let registry = CardRegistry::global();

    let totally_lost = registry.get("totally_lost").expect("Totally Lost");
    assert_eq!(totally_lost.primary_face().mana_cost.to_string(), "{4}{U}");
    assert!(matches!(
        &totally_lost.primary_face().spell_effect[..],
        [SpellEffectKind::PutTargetPermanentInOwnersLibrary { target, placement }]
            if target.kind == TargetKind::AnyPermanent
                && target.not_land
                && *placement == LibraryPlacement::Top
    ));

    let griptide = registry.get("griptide").expect("Griptide");
    assert_eq!(griptide.primary_face().mana_cost.to_string(), "{3}{U}");
    assert!(matches!(
        &griptide.primary_face().spell_effect[..],
        [SpellEffectKind::PutTargetPermanentInOwnersLibrary { target, placement }]
            if target.kind == TargetKind::Creature
                && *placement == LibraryPlacement::Top
    ));

    for (id, name) in [
        ("deglamer", "Deglamer"),
        ("unravel_the_aether", "Unravel the Aether"),
    ] {
        let definition = registry.get(id).unwrap_or_else(|| panic!("{name}"));
        assert_eq!(definition.primary_face().mana_cost.to_string(), "{1}{G}");
        assert!(matches!(
            &definition.primary_face().spell_effect[..],
            [SpellEffectKind::PutTargetPermanentInOwnersLibrary { target, placement }]
                if target.kind == TargetKind::AnyPermanent
                    && target.permanent_types
                        == [PermanentTypeFilter::Artifact, PermanentTypeFilter::Enchantment]
                    && *placement == LibraryPlacement::Shuffle
        ));
    }
}
