use tricerules_cards::primitives::{
    EffectSubject, SpellEffectKind, StaticAbilityDef, TargetController, TargetFilter, TargetKind,
};
use tricerules_cards::{CardRegistry, PermanentTypeFilter, TriggerCondition};

#[test]
fn glaring_aegis_has_complete_oracle_behavior() {
    let definition = CardRegistry::global()
        .get("glaring_aegis")
        .expect("Glaring Aegis must be registered");
    let face = definition.primary_face();

    assert_eq!(definition.name, "Glaring Aegis");
    assert_eq!(face.mana_cost.to_string(), "{W}");
    assert_eq!(face.types, ["Enchantment", "Aura"]);
    assert!(definition.partial.is_none());
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::AuraAttach {
            target: TargetFilter {
                kind: TargetKind::Creature,
                ..TargetFilter::default()
            },
        }]
    );
    assert_eq!(face.triggered_abilities.len(), 1);
    assert_eq!(
        face.triggered_abilities[0].trigger,
        TriggerCondition::WhenSelfEntersBattlefield
    );
    assert_eq!(
        face.triggered_abilities[0].effect,
        [SpellEffectKind::Tap {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::Opponent,
                ..TargetFilter::default()
            })),
        }]
    );
    assert_eq!(
        face.static_abilities,
        [StaticAbilityDef::AttachedModifier {
            condition: None,
            add_types: Default::default(),
            set_types: None,
            set_name: None,
            set_colors: None,
            set_power: None,
            set_toughness: None,
            delta_power: 1,
            delta_toughness: 3,
            remove_all_abilities: false,
            keywords: vec![],
            activated_abilities: vec![],
            triggered_abilities: vec![],
            cant_attack: false,
            cant_block: false,
            doesnt_untap_during_untap_step: false,
        }]
    );
}

#[test]
fn rambunctious_mutt_has_complete_oracle_behavior() {
    let definition = CardRegistry::global()
        .get("rambunctious_mutt")
        .expect("Rambunctious Mutt must be registered");
    let face = definition.primary_face();

    assert_eq!(definition.name, "Rambunctious Mutt");
    assert_eq!(face.mana_cost.to_string(), "{3}{W}{W}");
    assert_eq!(face.types, ["Creature", "Dog"]);
    assert_eq!((face.power, face.toughness), (Some(3), Some(4)));
    assert!(definition.partial.is_none());
    assert_eq!(face.triggered_abilities.len(), 1);
    assert_eq!(
        face.triggered_abilities[0].trigger,
        TriggerCondition::WhenSelfEntersBattlefield
    );
    assert_eq!(
        face.triggered_abilities[0].effect,
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::AnyPermanent,
                controller: TargetController::Opponent,
                permanent_types: vec![
                    PermanentTypeFilter::Artifact,
                    PermanentTypeFilter::Enchantment,
                ],
                ..TargetFilter::default()
            })),
        }]
    );
}
