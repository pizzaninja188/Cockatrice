use tricerules_cards::primitives::{
    AbilityCost, EffectSubject, SpellEffectKind, StaticAbilityDef, TargetController, TargetFilter,
    TargetKind,
};
use tricerules_cards::{
    AbilityPresentation, AbilitySourceZone, ActivationTiming, CardRegistry, Keyword,
    TriggerCondition,
};

fn defending_creature_target() -> TargetFilter {
    TargetFilter {
        kind: TargetKind::Creature,
        controller: TargetController::DefendingPlayer,
        ..TargetFilter::default()
    }
}

fn assert_attack_tap_trigger(ability: &tricerules_cards::TriggeredAbilityDef, id: &str, line: u16) {
    assert_eq!(ability.ability_id.as_str(), id);
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![line])
    );
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverAttachedObjectAttacks
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::Tap {
            subject: EffectSubject::Chosen(Box::new(defending_creature_target()))
        }]
    );
    assert!(!ability.may);
    assert!(ability.modal.is_none());
    assert!(ability.intervening_if.is_none());
    let targeting = ability.targeting.as_ref().expect("attack target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("attack trigger must have one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(
        group.prompt,
        "Choose target creature defending player controls"
    );
    assert_eq!(group.effect_indices, [0]);
    assert!(group.distinct_from.is_empty());
}

fn assert_equip(ability: &tricerules_cards::ActivatedAbilityDef, line: u16) {
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![line])
    );
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(ability.timing, ActivationTiming::Normal);
    assert!(matches!(
        ability.costs.as_slice(),
        [AbilityCost::Mana(cost)] if cost.to_string() == "{2}"
    ));
    assert!(matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::Equip { target }]
            if target.kind == TargetKind::Creature
                && target.controller == TargetController::You
    ));
    assert!(ability.targeting.is_none());
}

fn assert_common_identity(
    id: &str,
    name: &str,
    face_id: &str,
    mana_cost: &str,
    supertypes: &[&str],
) -> tricerules_cards::FaceRef<'static> {
    let registry = CardRegistry::global();
    let definition = registry
        .get(id)
        .unwrap_or_else(|| panic!("missing issue #286 card {id}"));
    assert_eq!(definition.name, name);
    assert_eq!(registry.id_for_name(name), Some(id));
    assert_eq!(definition.face_count(), 1);
    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), face_id);
    assert_eq!(face.name, name);
    assert_eq!(face.mana_cost.to_string(), mana_cost);
    assert_eq!(face.supertypes.as_slice(), supertypes);
    assert_eq!(
        face.types.iter().map(String::as_str).collect::<Vec<_>>(),
        ["Artifact", "Equipment"]
    );
    assert!(face.spell_effect.is_empty());
    assert!(face.custom_effect.is_none());
    assert!(face.modal_spell.is_none());
    assert!(face.targeting.is_none());
    assert!(face.characteristic_defining_abilities.is_empty());
    face
}

#[test]
fn issue_286_generated_cards_preserve_identity_and_all_equipment_abilities() {
    let registry = CardRegistry::global();

    let shield = assert_common_identity(
        "captain_americas_shield",
        "Captain America's Shield",
        "captain_america_s_shield",
        "{2}",
        &["Legendary"],
    );
    assert_eq!(shield.keywords, [Keyword::Indestructible]);
    let [shield_modifier] = shield.static_abilities.as_slice() else {
        panic!("Captain America's Shield should have one attached modifier");
    };
    assert_eq!(
        shield_modifier.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(matches!(
        &shield_modifier.definition,
        StaticAbilityDef::AttachedModifier {
            delta_power: 0,
            delta_toughness: 8,
            keywords,
            ..
        } if keywords == &[Keyword::Vigilance]
    ));
    let [shield_trigger] = shield.triggered_abilities.as_slice() else {
        panic!("Captain America's Shield should have one attack trigger");
    };
    assert_attack_tap_trigger(shield_trigger, "triggered_01", 3);
    let [shield_equip] = shield.activated_abilities.as_slice() else {
        panic!("Captain America's Shield should have one Equip ability");
    };
    assert_equip(shield_equip, 4);
    let shield_metadata = registry
        .presentation_face("captain_americas_shield", shield.face_id.as_str())
        .expect("Captain America's Shield presentation metadata");
    assert_eq!(shield_metadata.card_name, "Captain America's Shield");
    assert_eq!(shield_metadata.face_name, "Captain America's Shield");
    assert_eq!(
        shield_metadata.oracle_text_sha256,
        "373eecb8a0e58501b7ef1de7c774c6c01c122b4b6ce0212ce29d76f5bffbbcf9"
    );

    let lasso = assert_common_identity(
        "thunder_lasso",
        "Thunder Lasso",
        "thunder_lasso",
        "{2}{W}",
        &[],
    );
    assert!(lasso.keywords.is_empty());
    let [lasso_modifier] = lasso.static_abilities.as_slice() else {
        panic!("Thunder Lasso should have one attached modifier");
    };
    assert_eq!(
        lasso_modifier.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert!(matches!(
        &lasso_modifier.definition,
        StaticAbilityDef::AttachedModifier {
            delta_power: 1,
            delta_toughness: 1,
            keywords,
            ..
        } if keywords.is_empty()
    ));
    let [lasso_etb, lasso_trigger] = lasso.triggered_abilities.as_slice() else {
        panic!("Thunder Lasso should have ETB and attack triggers");
    };
    assert_eq!(lasso_etb.ability_id.as_str(), "triggered_01");
    assert_eq!(
        lasso_etb.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        lasso_etb.trigger,
        TriggerCondition::WhenSelfEntersBattlefield
    );
    assert!(!lasso_etb.may);
    assert!(matches!(
        lasso_etb.effect.as_slice(),
        [SpellEffectKind::AttachSource { target }]
            if target.kind == TargetKind::Creature
                && target.controller == TargetController::You
    ));
    let etb_targeting = lasso_etb.targeting.as_ref().expect("ETB target group");
    let [etb_group] = etb_targeting.groups.as_slice() else {
        panic!("Thunder Lasso ETB must have one target group");
    };
    assert_eq!((etb_group.min, etb_group.max), (1, 1));
    assert_eq!(etb_group.prompt, "Choose target creature you control");
    assert_eq!(etb_group.effect_indices, [0]);
    assert_attack_tap_trigger(lasso_trigger, "triggered_02", 3);
    let [lasso_equip] = lasso.activated_abilities.as_slice() else {
        panic!("Thunder Lasso should have one Equip ability");
    };
    assert_equip(lasso_equip, 4);
    let lasso_metadata = registry
        .presentation_face("thunder_lasso", lasso.face_id.as_str())
        .expect("Thunder Lasso presentation metadata");
    assert_eq!(lasso_metadata.card_name, "Thunder Lasso");
    assert_eq!(lasso_metadata.face_name, "Thunder Lasso");
    assert_eq!(
        lasso_metadata.oracle_text_sha256,
        "e2648482e9e3fe35bfffaf5cd128d26977f7a97a751294dec2926853a58c970a"
    );
}
