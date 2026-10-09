use tricerules_cards::{
    primitives::StaticAbilityDef, CastTriggerPlayer, Layout, ManaCost, TriggerCondition,
};

#[test]
fn wizard_class_is_registered_as_a_complete_card() {
    let card = tricerules_cards::registry::global()
        .get("wizard_class")
        .expect("Wizard Class needs a complete definition");
    assert_eq!(card.name, "Wizard Class");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "wizard_class");
    assert_eq!(face.mana_cost, ManaCost::parse("{U}").unwrap());
    assert_eq!(face.types, ["Enchantment", "Class"]);
    assert!(face.supertypes.is_empty());
    assert!(matches!(
        face.static_abilities.as_slice(),
        [ability]
            if ability.definition
                == StaticAbilityDef::NoMaximumHandSize {
                    players: tricerules_cards::primitives::NoMaximumHandSizeScope::Controller,
                }
    ));
    assert_eq!(
        face.class_level_bars
            .iter()
            .map(|bar| bar.level)
            .collect::<Vec<_>>(),
        [2, 3]
    );
    assert_eq!(
        face.class_level_bars[0].triggered_abilities[0].trigger,
        TriggerCondition::WhenThisClassBecomesLevel { level: 2 }
    );
    assert_eq!(
        face.class_level_bars[1].triggered_abilities[0].trigger,
        TriggerCondition::WheneverPlayerDrawsCard {
            drawer: CastTriggerPlayer::Controller,
        }
    );
}
