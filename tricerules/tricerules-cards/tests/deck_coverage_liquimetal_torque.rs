use tricerules_cards::primitives::{
    AbilityCost, EffectSubject, PermanentTypeFilter, SpellEffectKind,
};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, CardRegistry};

#[test]
fn liquimetal_torque_registers_both_activated_abilities() {
    let registry = CardRegistry::global();
    let card = registry
        .get("liquimetal_torque")
        .expect("Liquimetal Torque registry definition");

    assert_eq!(card.name, "Liquimetal Torque");
    assert_eq!(
        registry.id_for_name("Liquimetal Torque"),
        Some("liquimetal_torque")
    );
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "liquimetal_torque");
    assert_eq!(face.mana_cost.to_string(), "{2}");
    assert_eq!(face.types, ["Artifact"]);
    assert!(face.colors().is_empty());
    assert!(face.spell_effect.is_empty());
    let [mana_ability, type_ability] = face.activated_abilities.as_slice() else {
        panic!("Liquimetal Torque has two activated abilities");
    };

    assert_eq!(mana_ability.ability_id.as_str(), "activated_01");
    assert_eq!(mana_ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        mana_ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(mana_ability.costs, [AbilityCost::Tap]);
    let [SpellEffectKind::ProduceMana {
        commander_color_identity: false,
        options,
        restriction: None,
        conditional: None,
    }] = mana_ability.effect.as_slice()
    else {
        panic!("Liquimetal Torque's first ability produces mana");
    };
    assert_eq!(options.len(), 1);
    assert_eq!(
        (
            options[0].w,
            options[0].u,
            options[0].b,
            options[0].r,
            options[0].g,
            options[0].c
        ),
        (0, 0, 0, 0, 0, 1)
    );

    assert_eq!(type_ability.ability_id.as_str(), "activated_02");
    assert_eq!(type_ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        type_ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(type_ability.costs, [AbilityCost::Tap]);
    let [SpellEffectKind::AddTypes { subject, addition }] = type_ability.effect.as_slice() else {
        panic!("Liquimetal Torque's second ability adds a card type");
    };
    let EffectSubject::Chosen(filter) = subject else {
        panic!("Liquimetal Torque targets a chosen permanent");
    };
    assert_eq!(
        filter.kind,
        tricerules_cards::primitives::TargetKind::AnyPermanent
    );
    assert_eq!(filter.excluded_permanent_types, [PermanentTypeFilter::Land]);
    assert_eq!(addition.card_types, [PermanentTypeFilter::Artifact]);
    assert!(type_ability.targeting.is_some());
}
