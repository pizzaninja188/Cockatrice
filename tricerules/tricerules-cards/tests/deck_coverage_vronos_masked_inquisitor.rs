//! Exact Oracle 86851bdb-9b2f-4407-97a1-a6a9f2e0ef66, all three loyalty clauses.
use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::AbilityPresentation;

#[test]
fn vronos_exact_identity_and_all_three_complete_loyalty_clauses() {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get("vronos,_masked_inquisitor")
        .expect("complete actual Vronos must be registered");
    assert_eq!(
        registry.id_for_name("Vronos, Masked Inquisitor"),
        Some("vronos,_masked_inquisitor")
    );
    assert!(!card.commander_setup_only);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "vronos_masked_inquisitor");
    assert_eq!(face.mana_cost.to_string(), "{3}{U}{U}");
    assert_eq!(face.mana_cost.mana_value(), 5);
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Planeswalker", "Vronos"]);
    assert_eq!(face.loyalty, Some(5));
    assert_eq!(face.activated_abilities.len(), 3);
    for (index, delta) in [1, -2, -7].into_iter().enumerate() {
        let ability = &face.activated_abilities[index];
        assert_eq!(ability.costs, [AbilityCost::Loyalty(delta)]);
        assert_eq!(
            ability.ability_id.as_str(),
            format!("activated_{:02}", index + 1)
        );
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![index as u16 + 1])
        );
        assert!(ability.program.is_none() && ability.modal.is_none());
    }
    let expected: Vec<SpellEffectKind> = ron::from_str(r#"[
        CreateDelayedTrigger(subject:Some(Chosen((kind:AnyPermanent,controller:You,permanent_types:[Planeswalker],excluded_objects:[Source]))),
            ability:(ability_id:"delayed_phase_out",presentation:OracleLines([1]),trigger:AtBeginningOfNextEndStep,effect:[PhaseOutObservedObjects]))
    ]"#).unwrap();
    assert_eq!(face.activated_abilities[0].effect, expected);
    let group = &face.activated_abilities[0]
        .targeting
        .as_ref()
        .unwrap()
        .groups[0];
    assert_eq!((group.min, group.max), (0, 2));
    assert_eq!(group.effect_indices, [0]);
    let expected: Vec<SpellEffectKind> = ron::from_str(r#"[
        ReturnToOwnersHand(subject:Chosen((kind:AnyPermanent,controller:Opponent,excluded_permanent_types:[Land])))
    ]"#).unwrap();
    assert_eq!(face.activated_abilities[1].effect, expected);
    let group = &face.activated_abilities[1]
        .targeting
        .as_ref()
        .unwrap()
        .groups[0];
    assert_eq!((group.min, group.max), (0, 1));
    assert!(group.for_each_opponent);
    assert_eq!(group.effect_indices, [0]);
    let expected: Vec<SpellEffectKind> = ron::from_str(r#"[
        ApplyPermanentModifier(subject:Chosen((kind:AnyPermanent,controller:You,permanent_types:[Artifact])),modifier:AddTypes((card_types:[Artifact,Creature])),duration:Indefinite),
        ApplyPermanentModifier(subject:Chosen((kind:AnyPermanent,controller:You,permanent_types:[Artifact])),modifier:SetCreatureTypes(["Construct"]),duration:Indefinite),
        ApplyPermanentModifier(subject:Chosen((kind:AnyPermanent,controller:You,permanent_types:[Artifact])),modifier:SetBasePowerToughness(power:9,toughness:9),duration:Indefinite),
        ApplyPermanentModifier(subject:Chosen((kind:AnyPermanent,controller:You,permanent_types:[Artifact])),modifier:GrantKeywords([Vigilance,Indestructible]),duration:Indefinite),
        ApplyPermanentModifier(subject:Chosen((kind:AnyPermanent,controller:You,permanent_types:[Artifact])),modifier:GrantCantBeBlocked,duration:Indefinite)
    ]"#).unwrap();
    assert_eq!(face.activated_abilities[2].effect, expected);
    let group = &face.activated_abilities[2]
        .targeting
        .as_ref()
        .unwrap()
        .groups[0];
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.effect_indices, [0, 1, 2, 3, 4]);
}
