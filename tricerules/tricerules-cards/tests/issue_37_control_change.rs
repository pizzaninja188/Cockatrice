use tricerules_cards::primitives::{
    EffectSubject, SpellEffectKind, StaticAbilityDef, TargetController, TargetFilter, TargetKind,
};
use tricerules_cards::{CardRegistry, Keyword};

#[test]
fn issue_37_cards_are_registered_as_complete() {
    let registry = CardRegistry::global();
    for id in [
        "mind_control",
        "confiscate",
        "act_of_treason",
        "threaten",
        "cartouche_of_knowledge",
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("{id} must be registered"));
        assert!(definition.partial.is_none(), "{id} must not be partial");
    }
}

#[test]
fn control_auras_use_the_shared_source_relative_layer_2_primitive() {
    let registry = CardRegistry::global();
    for (id, kind) in [
        ("mind_control", TargetKind::Creature),
        ("confiscate", TargetKind::AnyPermanent),
    ] {
        let face = registry.get(id).expect("control Aura").primary_face();
        assert_eq!(
            face.spell_effect,
            [SpellEffectKind::AuraAttach {
                target: TargetFilter {
                    kind,
                    ..TargetFilter::default()
                },
            }]
        );
        assert_eq!(face.static_abilities, [StaticAbilityDef::ControlsAttached]);
    }
}

#[test]
fn temporary_control_spells_preserve_their_oracle_effect_order() {
    let registry = CardRegistry::global();
    let target = TargetFilter::default_creature();
    let chosen = EffectSubject::Chosen(target.clone());
    let control = SpellEffectKind::GainControlUntilEndOfTurn {
        target: target.clone(),
    };
    let untap = SpellEffectKind::Untap {
        subject: chosen.clone(),
    };
    let haste = SpellEffectKind::GrantKeywords {
        subject: chosen,
        keywords: vec![Keyword::Haste],
    };

    assert_eq!(
        registry
            .get("act_of_treason")
            .expect("Act of Treason")
            .primary_face()
            .spell_effect,
        [control.clone(), untap.clone(), haste.clone()]
    );
    assert_eq!(
        registry
            .get("threaten")
            .expect("Threaten")
            .primary_face()
            .spell_effect,
        [untap, control, haste]
    );
}

#[test]
fn cartouche_requires_a_creature_its_aura_controller_controls() {
    let face = CardRegistry::global()
        .get("cartouche_of_knowledge")
        .expect("Cartouche of Knowledge")
        .primary_face();
    assert_eq!(
        face.spell_effect,
        [SpellEffectKind::AuraAttach {
            target: TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            },
        }]
    );
    assert_eq!(
        face.static_abilities,
        [StaticAbilityDef::AttachedModifier {
            add_types: Default::default(),
            delta_power: 1,
            delta_toughness: 1,
            keywords: vec![Keyword::Flying],
            activated_abilities: Vec::new(),
            triggered_abilities: Vec::new(),
            cant_attack: false,
            cant_block: false,
            doesnt_untap_during_untap_step: false,
        }]
    );
}
