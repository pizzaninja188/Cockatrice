use tricerules_cards::primitives::{
    AbilitySourceZone, SpellEffectKind, StaticAbilityDef, TargetController, TargetKind,
};
use tricerules_cards::{AbilityCost, AbilityPresentation, Keyword, ManaCost, TriggerCondition};

#[test]
fn mithril_coat_registers_its_legendary_etb_and_any_creature_equip() {
    let card = tricerules_cards::registry::global()
        .get("mithril_coat")
        .expect("Mithril Coat is registered");
    let face = card.primary_face();

    assert_eq!(card.name, "Mithril Coat");
    assert_eq!(face.face_id.as_str(), "mithril_coat");
    assert_eq!(face.mana_cost.to_string(), "{3}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Artifact", "Equipment"]);
    assert!(face.keywords.contains(&Keyword::Flash));
    assert!(face.keywords.contains(&Keyword::Indestructible));

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Mithril Coat has one enters-the-battlefield trigger");
    };
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    let [SpellEffectKind::AttachSource { target }] = trigger.effect.as_slice() else {
        panic!("the trigger attaches Mithril Coat to its target");
    };
    assert_eq!(target.kind, TargetKind::Creature);
    assert_eq!(target.controller, TargetController::You);
    assert_eq!(target.required_supertypes, ["Legendary"]);
    let [group] = trigger
        .targeting
        .as_ref()
        .expect("mandatory ETB target")
        .groups
        .as_slice()
    else {
        panic!("Mithril Coat has one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target legendary creature you control");
    assert_eq!(group.effect_indices, [0]);

    let [equip] = face.activated_abilities.as_slice() else {
        panic!("Mithril Coat has one Equip ability");
    };
    assert_eq!(equip.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        equip.presentation,
        AbilityPresentation::OracleLines(vec![5])
    );
    assert_eq!(
        equip.costs,
        [AbilityCost::Mana(
            ManaCost::parse("{3}").expect("Equip cost")
        )]
    );
    assert!(equip.requires_sorcery_speed());
    assert!(matches!(
        equip.effect.as_slice(),
        [SpellEffectKind::Equip { target }]
            if target.kind == TargetKind::Creature
                && target.controller == TargetController::You
                && target.required_supertypes.is_empty()
    ));

    let [static_ability] = face.static_abilities.as_slice() else {
        panic!("Mithril Coat has an attached-creature modifier");
    };
    assert_eq!(
        static_ability.presentation,
        AbilityPresentation::OracleLines(vec![4])
    );
    assert!(matches!(
        &static_ability.definition,
        StaticAbilityDef::AttachedModifier { keywords, .. }
            if keywords == &[Keyword::Indestructible]
    ));
}
