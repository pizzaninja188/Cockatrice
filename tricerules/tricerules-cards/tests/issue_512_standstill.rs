use tricerules_cards::primitives::*;
use tricerules_cards::{AbilityPresentation, CardRegistry};

#[test]
fn standstill_is_complete_one_face_enchantment_with_automatic_sacrifice_gated_opponent_draw() {
    let registry = tricerules_cards::registry::global();
    assert_eq!(registry.id_for_name("Standstill"), Some("standstill"));
    let card = registry.get("standstill").unwrap();
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "standstill");
    assert_eq!(face.mana_cost.to_string(), "{1}{U}");
    assert_eq!(face.types, ["Enchantment"]);
    assert!(
        face.activated_abilities.is_empty()
            && face.static_abilities.is_empty()
            && face.spell_effect.is_empty()
    );
    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("one complete trigger")
    };
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        trigger.trigger,
        TriggerCondition::WheneverPlayerCastsSpell {
            caster: CastTriggerPlayer::AnyPlayer,
            filter: Default::default(),
            ordinal: None,
            ordinal_scope: Default::default(),
        }
    );
    let [SpellEffectKind::Sacrifice {
        subject: EffectSubject::Source,
    }, SpellEffectKind::ChooseResolutionBranch {
        selection,
        branches,
        chooser,
        optional,
        otherwise,
        ..
    }] = trigger.effect.as_slice()
    else {
        panic!("exact sacrifice and conditional draw")
    };
    assert_eq!(*selection, ResolutionBranchSelection::FirstApplicable);
    assert_eq!(*chooser, PlayerRecipient::Controller);
    assert!(!optional && otherwise.is_empty());
    assert_eq!(branches.len(), 2);
    assert_eq!(branches[0].cost, ResolutionCost::None);
    assert_eq!(
        branches[0].requirement,
        ResolutionBranchRequirement::CardResultCount {
            filter: CardResultFilter {
                source: CardResultSource::PreviousEffect,
                action: CardResultAction::Sacrifice,
                players: RelativePlayerSet::Controller,
                card_type: None
            },
            min: Some(1),
            max: None,
        }
    );
    assert_eq!(
        branches[0].effects,
        [SpellEffectKind::Draw {
            count: Amount::Fixed(3),
            who: PlayerRecipient::EachOtherPlayerThanAffectedPlayer
        }]
    );
    assert_eq!(branches[1].cost, ResolutionCost::None);
    assert_eq!(branches[1].requirement, ResolutionBranchRequirement::Always);
    assert!(branches[1].effects.is_empty());
}

#[test]
fn sacrifice_receipt_validation_accepts_real_producers_and_rejects_wrong_or_missing_predecessor() {
    let source = include_str!("../data/standstill.ron");
    for producer in [
        "Sacrifice(subject: Source)",
        "SacrificeAll(players: Controller, filter: (kind: AnyPermanent))",
    ] {
        let ron = source.replace("Sacrifice(subject: Source)", producer);
        assert!(
            CardRegistry::from_chunks_and_tokens(&[&ron], &[]).is_ok(),
            "{producer}"
        );
    }
    for ron in [
        source.replace("Sacrifice(subject: Source),", ""),
        source.replace("Sacrifice(subject: Source)", "Draw(count: 1)"),
        source.replace("action: Sacrifice", "action: Discard"),
    ] {
        assert!(matches!(CardRegistry::from_chunks_and_tokens(&[&ron], &[]),
            Err(tricerules_cards::registry::RegistryError::InvalidCard { reason, .. })
                if reason.contains("immediately preceding compatible")));
    }
}
