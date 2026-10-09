use tricerules_cards::primitives::*;
use tricerules_cards::{AbilityPresentation, CardRegistry};

#[test]
fn myr_battlesphere_complete_definition_and_rules_named_token_are_exact() {
    let registry = tricerules_cards::registry::global();
    let card = registry.get("myr_battlesphere").unwrap();
    assert_eq!(
        registry.id_for_name("Myr Battlesphere"),
        Some("myr_battlesphere")
    );
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "myr_battlesphere");
    assert_eq!(face.mana_cost.to_string(), "{7}");
    assert_eq!(face.types, ["Artifact", "Creature", "Myr", "Construct"]);
    assert_eq!((face.power, face.toughness), (Some(4), Some(7)));
    assert!(
        face.spell_effect.is_empty()
            && face.activated_abilities.is_empty()
            && face.static_abilities.is_empty()
    );
    assert_eq!(face.triggered_abilities.len(), 2);
    let entry = &face.triggered_abilities[0];
    assert_eq!(entry.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        entry.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(
        matches!(entry.effect.as_slice(), [SpellEffectKind::CreateTokens { token, count: Amount::Fixed(4), .. }] if token == "myr_token_c_1_1")
    );
    let attack = &face.triggered_abilities[1];
    assert_eq!(
        attack.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        attack.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0
        }
    );
    assert!(!attack.may && attack.targeting.is_none());
    assert!(
        matches!(attack.effect.as_slice(), [SpellEffectKind::ChoosePermanents {
        chooser: PlayerRecipient::Controller, filter, min: 0, max: u32::MAX, constraints,
    }, SpellEffectKind::MyrBattlesphereAttack] if filter.kind == TargetKind::AnyPermanent
        && filter.controller == TargetController::You && filter.tapped == Some(false)
        && filter.required_subtypes == ["Myr"] && constraints.is_empty())
    );
    let token = registry.get("myr_token_c_1_1").unwrap();
    assert!(registry.is_token("myr_token_c_1_1"));
    assert_eq!(token.name, "Myr Token");
    assert_eq!(token.primary_face().types, ["Artifact", "Creature", "Myr"]);
    assert_eq!(
        (token.primary_face().power, token.primary_face().toughness),
        (Some(1), Some(1))
    );
}

#[test]
fn myr_attack_consumer_rejects_missing_incompatible_and_wrong_trigger_producers() {
    let registry = tricerules_cards::registry::global();
    let mut effects = registry
        .get("myr_battlesphere")
        .unwrap()
        .primary_face()
        .triggered_abilities[1]
        .effect
        .clone();
    assert!(SpellEffectKind::validate_list(&effects).is_ok());
    assert!(SpellEffectKind::validate_list(&effects[1..]).is_err());
    for case in 0..6 {
        let mut bad = effects.clone();
        let SpellEffectKind::ChoosePermanents {
            chooser,
            filter,
            min,
            max,
            ..
        } = &mut bad[0]
        else {
            unreachable!()
        };
        match case {
            0 => *chooser = PlayerRecipient::EachOpponent,
            1 => filter.kind = TargetKind::Creature,
            2 => filter.controller = TargetController::Any,
            3 => filter.tapped = None,
            4 => *min = 1,
            _ => *max = 4,
        }
        assert!(SpellEffectKind::validate_list(&bad).is_err(), "case {case}");
    }
    effects.insert(
        1,
        SpellEffectKind::Draw {
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
        },
    );
    assert!(SpellEffectKind::validate_list(&effects).is_err());
    let card = include_str!("../data/myr_battlesphere.ron");
    let token = include_str!("../data/tokens/myr_token_c_1_1.ron");
    let wrong = card.replace(
        "WheneverSelfAttacks(minimum_other_attackers: 0)",
        "AtBeginningOfUpkeep(player: Controller)",
    );
    let error = CardRegistry::from_chunks_and_tokens(&[&wrong], &[token]).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("MyrBattlesphereAttack requires WheneverSelfAttacks"),
        "{error}"
    );
}

#[test]
fn myr_attack_consumer_rejects_reflexive_trigger_context() {
    let card = include_str!("../data/myr_battlesphere.ron").replace("\r\n", "\n");
    let token = include_str!("../data/tokens/myr_token_c_1_1.ron");
    let wrong = card.replace(
        "effect: [\n      ChoosePermanents(",
        "effect: [CreateReflexiveTrigger(ability: (ability_id: \"reflexive_01\", presentation: OracleLines([2]), effect: [\n      ChoosePermanents(",
    ).replace(
        "      MyrBattlesphereAttack,\n    ],",
        "      MyrBattlesphereAttack,\n    ]))],",
    );
    let error = CardRegistry::from_chunks_and_tokens(&[&wrong], &[token]).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("reflexive triggered abilities cannot reference attack context"),
        "{error}"
    );
}
