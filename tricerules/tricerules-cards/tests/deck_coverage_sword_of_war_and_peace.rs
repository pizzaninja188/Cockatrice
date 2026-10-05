use tricerules_cards::primitives::{ProtectionQuality, StaticAbilityDef};
use tricerules_cards::{AbilityPresentation, CardRegistry, Color, TriggerCondition};

#[test]
fn sword_registers_exact_single_face_and_three_clauses() {
    let card = CardRegistry::global()
        .get("sword_of_war_and_peace")
        .expect("the exact missing Sword of War and Peace must be implemented");
    let face = card.primary_face();
    assert_eq!(card.name, "Sword of War and Peace");
    assert_eq!(face.mana_cost.to_string(), "{3}");
    assert_eq!(face.types, ["Artifact", "Equipment"]);
    assert!(face.colors().is_empty());
    assert_eq!(card.faces_iter().count(), 1);
    assert_eq!(face.static_abilities.len(), 1);
    assert_eq!(face.triggered_abilities.len(), 1);
    assert_eq!(face.activated_abilities.len(), 1);
    assert_eq!(
        face.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        face.triggered_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        face.activated_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(
        face.triggered_abilities[0].trigger,
        TriggerCondition::WheneverAttachedObjectDealsCombatDamageToPlayer
    );
    assert!(face.protections.is_empty() && face.triggered_abilities[0].targeting.is_none());
    let StaticAbilityDef::AttachedModifier {
        delta_power,
        delta_toughness,
        protections,
        triggered_abilities,
        ..
    } = &face.static_abilities[0].definition
    else {
        panic!("attached modifier required")
    };
    assert_eq!((*delta_power, *delta_toughness), (2, 2));
    assert_eq!(
        protections.as_ref(),
        &[
            ProtectionQuality::Color(Color::Red),
            ProtectionQuality::Color(Color::White)
        ]
    );
    assert!(triggered_abilities.is_empty(), "Sword owns its trigger");
    let effects = [
        "DamagePlayer(amount: Count(CardsInHand(players: AffectedPlayer)), who: AffectedPlayer)",
        "GainLife(amount: Count(CardsInHand(players: Relative(Controller))))",
    ]
    .map(|s| ron::from_str::<tricerules_cards::SpellEffectKind>(s).unwrap());
    assert_eq!(face.triggered_abilities[0].effect, effects);
    let cost: tricerules_cards::AbilityCost = ron::from_str("Mana(\"{2}\")").unwrap();
    assert_eq!(face.activated_abilities[0].costs, [cost]);
    let equip: tricerules_cards::SpellEffectKind =
        ron::from_str("Equip(target: (kind: Creature, controller: You))").unwrap();
    assert_eq!(face.activated_abilities[0].effect, [equip]);
}

#[test]
fn sword_attached_protection_defaults_roundtrips_and_validates_conditions() {
    let definition = |modifier: &str| {
        format!(
            r#"(id: "protection_fixture", name: "Protection Fixture", face_id: "protection_fixture", types: ["Artifact", "Equipment"], static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: {modifier})])"#
        )
    };
    let old = CardRegistry::from_chunks_and_tokens(
        &[&definition("AttachedModifier(delta_power: 1)")],
        &[],
    )
    .unwrap();
    let StaticAbilityDef::AttachedModifier { protections, .. } = &old
        .get("protection_fixture")
        .unwrap()
        .primary_face()
        .static_abilities[0]
        .definition
    else {
        panic!("modifier")
    };
    assert!(protections.is_empty());
    let modifier: StaticAbilityDef =
        ron::from_str("AttachedModifier(protections: [Color(Red), Color(White)])").unwrap();
    assert_eq!(
        ron::from_str::<StaticAbilityDef>(&ron::to_string(&modifier).unwrap()).unwrap(),
        modifier
    );
    assert!(CardRegistry::from_chunks_and_tokens(
        &[&definition("AttachedModifier(protections: [Color(Red)])")],
        &[]
    )
    .is_ok());
    let dependency = "AttachedModifier(protections: [Color(Red)], condition: Some(BattlefieldAggregate(filter: (controllers: Controller), aggregate: TotalPower, min: Some(1))))";
    let error = CardRegistry::from_chunks_and_tokens(&[&definition(dependency)], &[])
        .unwrap_err()
        .to_string();
    assert!(error.contains("dependency ordering"), "{error}");
}
