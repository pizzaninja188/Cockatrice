use tricerules_cards::primitives::{
    EffectSubject, SpellEffectKind, TargetController, TargetFilter, TargetKind,
    TargetObjectExclusion,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Keyword, TriggerCondition};

fn assert_attack_pump_and_indestructible(id: &str, name: &str, mana_cost: &str, types: &[&str]) {
    let registry = CardRegistry::global();
    let definition = registry
        .get(id)
        .unwrap_or_else(|| panic!("missing issue #285 card {id}"));
    assert_eq!(definition.name, name);
    assert_eq!(registry.id_for_name(name), Some(id));
    assert_eq!(definition.face_count(), 1);

    let face = definition.primary_face();
    assert_eq!(face.face_id.as_str(), id);
    assert_eq!(face.name, name);
    assert_eq!(face.mana_cost.to_string(), mana_cost);
    assert_eq!(
        face.types.iter().map(String::as_str).collect::<Vec<_>>(),
        types
    );
    assert_eq!((face.power, face.toughness), (Some(2), Some(4)));
    assert!(face.keywords.is_empty());
    assert!(face.spell_effect.is_empty());
    assert!(face.custom_effect.is_none());
    assert!(face.modal_spell.is_none());
    assert!(face.targeting.is_none());
    assert!(face.activated_abilities.is_empty());
    assert!(face.static_abilities.is_empty());
    assert!(face.characteristic_defining_abilities.is_empty());
    assert_eq!(face.triggered_abilities.len(), 1);

    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("issue #285 has one attack trigger");
    };
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0,
        }
    );
    assert!(!ability.may);
    assert!(ability.modal.is_none());
    assert!(ability.intervening_if.is_none());

    let target = TargetFilter {
        kind: TargetKind::Creature,
        controller: TargetController::You,
        excluded_objects: vec![TargetObjectExclusion::Source],
        ..TargetFilter::default()
    };
    let [SpellEffectKind::PumpTarget {
        power,
        toughness,
        scale,
        subject: pump_subject,
    }, SpellEffectKind::GrantKeywords {
        subject: keyword_subject,
        keywords,
    }] = ability.effect.as_slice()
    else {
        panic!("issue #285 must pump then grant indestructible");
    };
    assert_eq!((*power, *toughness), (1, 0));
    assert!(scale.is_none());
    assert_eq!(
        pump_subject,
        &EffectSubject::Chosen(Box::new(target.clone()))
    );
    assert_eq!(
        keyword_subject,
        &EffectSubject::Chosen(Box::new(target.clone()))
    );
    assert_eq!(keywords, &[Keyword::Indestructible]);

    let targeting = ability.targeting.as_ref().expect("issue #285 target group");
    assert_eq!(targeting.groups.len(), 1);
    let [group] = targeting.groups.as_slice() else {
        panic!("issue #285 must have one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose another target creature you control");
    assert_eq!(group.effect_indices, [0, 1]);
    assert!(group.distinct_from.is_empty());

    let metadata = registry
        .presentation_face(id, face.face_id.as_str())
        .unwrap_or_else(|| panic!("missing presentation metadata for {id}"));
    assert_eq!(metadata.card_name, name);
    assert_eq!(metadata.face_name, name);
    assert_eq!(
        metadata.oracle_text_sha256,
        "14c9a60cb9f811b4ca0cc334c2f7e12747a66bb0eb736a95ee21e73d7f61159a"
    );
}

#[test]
fn issue_285_cards_have_exact_registry_characteristics_and_typed_tree() {
    assert_attack_pump_and_indestructible(
        "foot_elite",
        "Foot Elite",
        "{2}{W/B}",
        &["Creature", "Human", "Ninja"],
    );
    assert_attack_pump_and_indestructible(
        "hardened_escort",
        "Hardened Escort",
        "{2}{W}",
        &["Creature", "Human", "Soldier"],
    );
}
