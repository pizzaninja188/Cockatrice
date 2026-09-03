use tricerules_cards::primitives::{PlayerRecipient, TargetController, TargetKind};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, AbilitySourceZone, Amount, BattlefieldAggregate,
    CardRegistry, CastTriggerPlayer, Keyword, PermanentTypeFilter, RelativePlayerSet,
    SpellCostModifier, SpellEffectKind, TriggerCondition,
};

fn fixture(source_zone: &str) -> String {
    format!(
        r#"
            (
              id: "self_exile_test",
              name: "Self Exile Test",
              face_id: "self_exile_test",
              mana_cost: "{{1}}",
              types: ["Enchantment"],
              activated_abilities: [(
                ability_id: "activated_01",
                presentation: Fallback,
                source_zone: {source_zone},
                costs: [Mana("{{1}}"), ExileSelf],
                effect: [GainLife(amount: 1)],
              )],
            )
            "#
    )
}

#[test]
fn issue_218_battlefield_exile_self_cost_loads() {
    CardRegistry::from_chunks_and_tokens(&[&fixture("Battlefield")], &[])
        .expect("battlefield activated abilities may exile their source as a cost");
}

#[test]
fn issue_218_hand_exile_self_cost_remains_invalid() {
    let error = CardRegistry::from_chunks_and_tokens(&[&fixture("Hand")], &[])
        .expect_err("hand self-exile is unsupported");
    assert!(error
        .to_string()
        .contains("battlefield or graveyard source"));
}

#[test]
fn issue_218_sapling_nursery_has_exact_affinity_landfall_and_activation_shape() {
    let card = CardRegistry::global()
        .get("sapling_nursery")
        .expect("Sapling Nursery");
    let face = card.primary_face();

    assert_eq!(card.name, "Sapling Nursery");
    assert_eq!(face.face_id.as_str(), "sapling_nursery");
    assert_eq!(face.mana_cost.to_string(), "{6}{G}{G}");
    assert_eq!(face.types, ["Enchantment"]);

    let [SpellCostModifier::BattlefieldCountGenericReduction {
        amount_per_match,
        filter,
        aggregate,
    }] = face.cost_modifiers.as_slice()
    else {
        panic!("one affinity-for-Forests modifier");
    };
    assert_eq!(*amount_per_match, 1);
    assert_eq!(filter.controllers, RelativePlayerSet::Controller);
    assert_eq!(filter.required_subtypes, ["Forest"]);
    assert_eq!(*aggregate, BattlefieldAggregate::Count);

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("one landfall trigger");
    };
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    let TriggerCondition::WheneverPermanentEntersBattlefield {
        controller,
        filter,
        creature_filter,
    } = &trigger.trigger
    else {
        panic!("landfall trigger");
    };
    assert_eq!(*controller, CastTriggerPlayer::Controller);
    assert_eq!(filter.permanent_type, Some(PermanentTypeFilter::Land));
    assert!(creature_filter.is_none());
    assert!(matches!(
        trigger.effect.as_slice(),
        [SpellEffectKind::CreateTokens {
            token,
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }] if token == "treefolk_g_3_4_reach"
    ));

    let [ability] = face.activated_abilities.as_slice() else {
        panic!("one activated ability");
    };
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert!(matches!(
        ability.costs.as_slice(),
        [AbilityCost::Mana(cost), AbilityCost::ExileSelf]
            if cost.to_string() == "{1}{G}"
    ));
    let [SpellEffectKind::GrantKeywordsAllPermanents { filter, keywords }] =
        ability.effect.as_slice()
    else {
        panic!("one cohort indestructible effect");
    };
    assert_eq!(keywords, &[Keyword::Indestructible]);
    let branches = filter.any_of.as_ref().expect("Treefolk-or-Forest filter");
    assert_eq!(branches.len(), 2);
    assert!(branches.iter().any(|branch| {
        branch.kind == TargetKind::AnyPermanent
            && branch.controller == TargetController::You
            && branch.required_subtypes == ["Treefolk"]
    }));
    assert!(branches.iter().any(|branch| {
        branch.kind == TargetKind::AnyPermanent
            && branch.controller == TargetController::You
            && branch.required_subtypes == ["Forest"]
    }));
}
