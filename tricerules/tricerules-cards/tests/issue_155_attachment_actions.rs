use tricerules_cards::primitives::{
    AbilitySourceZone, EffectSubject, LibraryPlacement, SpellEffectKind,
};
use tricerules_cards::CardRegistry;

#[test]
fn issue_155_equipment_cards_share_the_one_shot_attach_primitive() {
    let registry = CardRegistry::global();
    for id in [
        "illvoi_light_jammer",
        "squires_lightblade",
        "meltstriders_gear",
        "barbed_bloodletter",
    ] {
        let face = registry
            .get(id)
            .expect("issue #155 Equipment")
            .primary_face();
        assert!(face.types.iter().any(|card_type| card_type == "Equipment"));
        assert!(matches!(
            face.triggered_abilities[0].effect.first(),
            Some(SpellEffectKind::AttachSource { .. })
        ));
        assert!(matches!(
            face.activated_abilities[0].effect.as_slice(),
            [SpellEffectKind::Equip { .. }]
        ));
    }
}

#[test]
fn issue_274_generated_equipment_etb_targets_are_mandatory_and_shared() {
    let registry = CardRegistry::global();
    let scythe = &registry
        .get("malamet_scythe")
        .expect("Malamet Scythe")
        .primary_face()
        .triggered_abilities[0];
    assert_eq!(
        scythe.presentation,
        tricerules_cards::AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        scythe.targeting.as_ref().unwrap().groups[0].effect_indices,
        vec![0]
    );
    assert_eq!(
        (
            scythe.targeting.as_ref().unwrap().groups[0].min,
            scythe.targeting.as_ref().unwrap().groups[0].max
        ),
        (1, 1)
    );
    let coral = &registry
        .get("coral_sword")
        .expect("Coral Sword")
        .primary_face()
        .triggered_abilities[0];
    assert_eq!(
        coral.presentation,
        tricerules_cards::AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        coral.targeting.as_ref().unwrap().groups[0].effect_indices,
        vec![0, 1]
    );
    assert_eq!(
        (
            coral.targeting.as_ref().unwrap().groups[0].min,
            coral.targeting.as_ref().unwrap().groups[0].max
        ),
        (1, 1)
    );
    assert!(
        matches!(coral.effect.as_slice(), [SpellEffectKind::AttachSource { .. }, SpellEffectKind::GrantKeywords { subject: EffectSubject::Chosen(_), keywords }] if keywords == &[tricerules_cards::primitives::Keyword::FirstStrike])
    );
}

#[test]
fn issue_155_auras_use_untargeted_attached_object_zone_actions() {
    let registry = CardRegistry::global();
    for id in ["spiral_into_solitude", "path_to_redemption"] {
        let ability = &registry
            .get(id)
            .expect("issue #155 exile Aura")
            .primary_face()
            .activated_abilities[0];
        assert!(ability.effect.iter().any(|effect| matches!(
            effect,
            SpellEffectKind::Exile {
                subject: EffectSubject::AttachedObject
            }
        )));
        assert!(ability.targeting.is_none());
    }

    let watery = &registry
        .get("watery_grasp")
        .expect("Watery Grasp")
        .primary_face()
        .activated_abilities[0];
    assert!(matches!(
        watery.effect.as_slice(),
        [SpellEffectKind::PutInOwnersLibrary {
            subject: EffectSubject::AttachedObject,
            placement: LibraryPlacement::Shuffle,
        }]
    ));
    assert!(watery.targeting.is_none());
}

#[test]
fn issue_155_merchant_uses_its_exact_graveyard_source() {
    let merchant = &CardRegistry::global()
        .get("merchant_of_many_hats")
        .expect("Merchant of Many Hats")
        .primary_face()
        .activated_abilities[0];
    assert_eq!(merchant.source_zone, AbilitySourceZone::Graveyard);
    assert!(matches!(
        merchant.effect.as_slice(),
        [SpellEffectKind::ReturnToOwnersHand {
            subject: EffectSubject::Source
        }]
    ));
    assert!(merchant.targeting.is_none());
}
