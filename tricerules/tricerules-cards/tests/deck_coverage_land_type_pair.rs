use tricerules_cards::primitives::{SpellEffectKind, StaticAbilityDef, TargetKind};
use tricerules_cards::{AbilityPresentation, BasicLandType, CardRegistry, PermanentTypeFilter};

#[test]
fn yavimaya_exact_identity_adds_forest_to_every_land_without_authored_mana() {
    let registry = CardRegistry::global();
    let id = registry
        .id_for_name("Yavimaya, Cradle of Growth")
        .expect("exact Yavimaya identity");
    assert_eq!(id, "yavimaya,_cradle_of_growth");
    let card = registry.get(id).unwrap();
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.name, "Yavimaya, Cradle of Growth");
    assert_eq!(face.face_id.as_str(), "yavimaya_cradle_of_growth");
    assert_eq!(face.types, ["Land"]);
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.mana_cost.to_string(), "");
    assert!(face.activated_abilities.is_empty());
    let [ability] = face.static_abilities.as_slice() else {
        panic!("one complete static ability");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    let StaticAbilityDef::AddTypesToPermanents { filter, addition } = &ability.definition else {
        panic!("all-land subtype addition");
    };
    assert_eq!(filter.kind, TargetKind::AnyPermanent);
    assert_eq!(filter.permanent_types, [PermanentTypeFilter::Land]);
    assert!(addition.card_types.is_empty() && addition.creature_types.is_empty());
    assert_eq!(addition.land_types, [BasicLandType::Forest]);
}

#[test]
fn song_exact_identity_enchants_any_permanent_and_sets_colorless_forest() {
    let registry = CardRegistry::global();
    assert_eq!(
        registry.id_for_name("Song of the Dryads"),
        Some("song_of_the_dryads")
    );
    let card = registry.get("song_of_the_dryads").expect("Song identity");
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{2}{G}");
    assert_eq!(face.types, ["Enchantment", "Aura"]);
    let [SpellEffectKind::AuraAttach { target }] = face.spell_effect.as_slice() else {
        panic!("enchant permanent");
    };
    assert_eq!(target.kind, TargetKind::AnyPermanent);
    let [ability] = face.static_abilities.as_slice() else {
        panic!("one transformation static");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    let StaticAbilityDef::AttachedModifier {
        set_types: Some(types),
        set_colors: Some(colors),
        remove_all_abilities,
        ..
    } = &ability.definition
    else {
        panic!("layer-four/five transformation");
    };
    assert_eq!(types.card_types, [PermanentTypeFilter::Land]);
    assert_eq!(types.land_types, [BasicLandType::Forest]);
    assert!(types.creature_types.is_empty() && colors.is_empty());
    assert!(
        !remove_all_abilities,
        "basic-land setting preserves independently granted abilities"
    );
}
