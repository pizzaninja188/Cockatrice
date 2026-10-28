use tricerules_cards::{
    AbilityCost, AbilityPresentation, CastTriggerPlayer, Layout, ManaCost, SpellEffectKind,
    TriggerCondition,
};

const INNKEEPERS_TALENT: &str = "innkeepers_talent";

#[test]
fn innkeepers_talent_registers_exact_class_costs_combat_trigger_and_counter_scaled_ward() {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get(INNKEEPERS_TALENT)
        .expect("Innkeeper's Talent must have a complete registered definition");
    assert_eq!(
        registry.id_for_name("Innkeeper's Talent"),
        Some(INNKEEPERS_TALENT)
    );
    assert_eq!(card.id, INNKEEPERS_TALENT);
    assert_eq!(card.name, "Innkeeper's Talent");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "innkeepers_talent");
    assert_eq!(face.mana_cost, ManaCost::parse("{1}{G}").unwrap());
    assert_eq!(face.types, ["Enchantment", "Class"]);
    assert_eq!(face.class_level_bars.len(), 2);
    assert_eq!(
        face.class_level_bars
            .iter()
            .map(|bar| bar.level)
            .collect::<Vec<_>>(),
        [2, 3]
    );

    let [level_two, level_three] = face.class_level_bars.as_slice() else {
        panic!("Innkeeper's Talent has level II and III bars");
    };
    assert_eq!(
        level_two.level_ability.costs,
        [AbilityCost::Mana(ManaCost::parse("{G}").unwrap())]
    );
    assert_eq!(
        level_two.level_ability.effect,
        [SpellEffectKind::SetClassLevel { level: 2 }]
    );
    assert_eq!(
        level_three.level_ability.costs,
        [AbilityCost::Mana(ManaCost::parse("{3}{G}").unwrap())]
    );
    assert_eq!(
        level_three.level_ability.effect,
        [SpellEffectKind::SetClassLevel { level: 3 }]
    );

    let [combat_trigger] = face.triggered_abilities.as_slice() else {
        panic!("Innkeeper's Talent has one beginning-of-combat trigger");
    };
    assert_eq!(
        combat_trigger.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        combat_trigger.trigger,
        TriggerCondition::AtBeginningOfCombat {
            player: CastTriggerPlayer::Controller,
        }
    );

    assert_eq!(level_two.static_abilities.len(), 1);
    assert_eq!(
        level_two.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![4])
    );
    let ward = ron::to_string(&level_two.static_abilities[0].definition)
        .expect("serialize the level-II granted Ward ability");
    assert!(
        ward.contains("requires_any_counter:true") || ward.contains("requires_any_counter: true"),
        "the Ward grant must require at least one physical counter: {ward}"
    );
    assert!(ward.contains("CounterTriggeringStackObjectUnlessPays"));
    assert!(ward.contains("Mana(\"{1}\")"));

    assert_eq!(level_three.static_abilities.len(), 1);
    assert_eq!(
        level_three.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![6])
    );
    let level_three_static = format!("{:?}", level_three.static_abilities[0].definition);
    assert!(
        level_three_static.contains("DoubleCountersYouWouldPut"),
        "level III must reuse the shared counter-placement static: {level_three_static}"
    );
}

#[test]
fn innkeepers_talent_is_admitted_for_ordinary_mainboard_use() {
    let card = tricerules_cards::registry::global()
        .get(INNKEEPERS_TALENT)
        .expect("Innkeeper's Talent must be registered");
    assert!(
        !card.commander_setup_only,
        "the complete Innkeeper's Talent definition is admitted as an ordinary mainboard card"
    );
}
