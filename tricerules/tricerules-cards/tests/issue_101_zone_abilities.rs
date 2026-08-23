use tricerules_cards::{
    AbilityCost, AbilitySourceZone, CardRegistry, SearchDestination, SpellEffectKind,
};

#[test]
fn typecycling_is_authored_from_hand_with_self_discard_and_subtype_search() {
    let registry = CardRegistry::global();
    for (card_id, subtype) in [
        ("shepherding_spirits", "Plains"),
        ("slavering_branchsnapper", "Forest"),
        ("daggermaw_megalodon", "Island"),
    ] {
        let face = registry.get(card_id).expect(card_id).primary_face();
        let ability = face
            .activated_abilities
            .first()
            .expect("typecycling ability");
        assert_eq!(ability.source_zone, AbilitySourceZone::Hand);
        assert!(matches!(
            ability.costs.as_slice(),
            [AbilityCost::Mana(_), AbilityCost::DiscardSelf]
        ));
        let [SpellEffectKind::SearchLibrary {
            filter: Some(filter),
            destination: SearchDestination::Hand,
            shuffle: true,
            reveal: true,
            ..
        }] = ability.effect.as_slice()
        else {
            panic!("typecycling uses the shared revealed subtype search")
        };
        assert_eq!(filter.subtype.as_deref(), Some(subtype));
        assert!(filter.card_type.is_none());
    }
}

#[test]
fn renew_is_authored_from_graveyard_with_self_exile() {
    let registry = CardRegistry::global();
    for card_id in ["adorned_crocodile", "sagu_pummeler", "champion_of_dusan"] {
        let face = registry.get(card_id).expect(card_id).primary_face();
        let ability = face.activated_abilities.first().expect("renew ability");
        assert_eq!(ability.source_zone, AbilitySourceZone::Graveyard);
        assert!(matches!(ability.costs.last(), Some(AbilityCost::ExileSelf)));
        assert!(ability.requires_sorcery_speed());
        assert!(ability.targeting.is_some());
    }
}
