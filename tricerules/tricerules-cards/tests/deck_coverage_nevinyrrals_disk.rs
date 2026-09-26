use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, EntersTappedAffected, PermanentTypeFilter, SpellEffectKind,
    StaticAbilityDef, TargetFilter, TargetKind, TargetSchema,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Layout};

#[test]
fn nevinyrrals_disk_registers_entry_replacement_and_mass_destroy_activation() {
    let registry = CardRegistry::global();
    let card = registry
        .get("nevinyrrals_disk")
        .expect("Nevinyrral's Disk registry definition");

    assert_eq!(card.name, "Nevinyrral's Disk");
    assert_eq!(
        registry.id_for_name("Nevinyrral's Disk"),
        Some("nevinyrrals_disk")
    );
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "nevinyrrals_disk");
    assert_eq!(face.mana_cost.to_string(), "{4}");
    assert_eq!(face.types, ["Artifact"]);
    assert!(face.colors().is_empty());

    let [enters_tapped] = face.static_abilities.as_slice() else {
        panic!("Nevinyrral's Disk has one entry replacement");
    };
    assert_eq!(enters_tapped.ability_id.as_str(), "static_01");
    assert_eq!(
        enters_tapped.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        enters_tapped.definition,
        StaticAbilityDef::EntersTapped {
            affected: EntersTappedAffected::Self_,
            condition: None,
            unless_cost: None,
        }
    );

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Nevinyrral's Disk has one activated ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(matches!(
        ability.costs.as_slice(),
        [AbilityCost::Mana(cost), AbilityCost::Tap] if cost.to_string() == "{1}"
    ));
    assert!(ability.targeting.is_none());

    let destroy_filter = TargetFilter {
        kind: TargetKind::AnyPermanent,
        permanent_types: vec![
            PermanentTypeFilter::Artifact,
            PermanentTypeFilter::Creature,
            PermanentTypeFilter::Enchantment,
        ],
        ..TargetFilter::default()
    };
    assert_eq!(
        ability.effect,
        [SpellEffectKind::DestroyAll {
            kind: destroy_filter,
            prevent_regeneration: false,
        }]
    );
    assert!(TargetSchema::compile(&ability.effect, None).is_ok());
}
