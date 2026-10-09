use tricerules_cards::primitives::*;
use tricerules_cards::AbilityPresentation;

#[test]
fn boseiju_exact_identity_maps_printed_mana_and_complete_channel() {
    let registry = tricerules_cards::registry::global();
    assert_eq!(
        registry.id_for_name("Boseiju, Who Endures"),
        Some("boseiju,_who_endures")
    );
    let card = registry.get("boseiju,_who_endures").unwrap();
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "boseiju_who_endures");
    assert_eq!(face.types, ["Land"]);
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.mana_cost.to_string(), "");
    assert!(face.spell_effect.is_empty() && face.static_abilities.is_empty());
    let [mana, channel] = face.activated_abilities.as_slice() else {
        panic!("two complete abilities")
    };
    assert_eq!(mana.presentation, AbilityPresentation::OracleLines(vec![1]));
    assert_eq!(mana.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(mana.costs, [AbilityCost::Tap]);
    let [SpellEffectKind::ProduceMana { options, .. }] = mana.effect.as_slice() else {
        panic!("printed green mana")
    };
    assert_eq!(options.len(), 1);
    assert_eq!(options[0].g, 1);
    assert_eq!(
        channel.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(channel.source_zone, AbilitySourceZone::Hand);
    let [AbilityCost::Mana(cost), AbilityCost::DiscardSelf] = channel.costs.as_slice() else {
        panic!("paid discard")
    };
    assert_eq!(cost.to_string(), "{1}{G}");
    let [ActivatedCostModifier::GenericReduction {
        amount: Amount::Count(CountExpression::BattlefieldPermanents { filter }),
    }] = channel.cost_modifiers.as_slice()
    else {
        panic!("counted reduction")
    };
    assert_eq!(filter.controllers, RelativePlayerSet::Controller);
    assert_eq!(filter.card_type, Some(CardTypeFilter::Creature));
    assert_eq!(filter.required_supertypes, ["Legendary"]);
    let [SpellEffectKind::Destroy { .. }, SpellEffectKind::SearchLibrary {
        who,
        optional,
        count,
        filter: Some(filter),
        destination,
        shuffle,
        reveal,
        ..
    }] = channel.effect.as_slice()
    else {
        panic!("destroy then optional search")
    };
    assert_eq!(
        *who,
        PlayerRecipient::ControllerOfTargetGroup { group_index: 0 }
    );
    assert!(*optional && *shuffle && !*reveal);
    assert_eq!(*count, 1);
    assert_eq!(
        *destination,
        SearchDestination::Battlefield { tapped: false }
    );
    let branches = filter.any_of.as_ref().unwrap();
    assert_eq!(branches.len(), 5);
    for (branch, subtype) in branches
        .iter()
        .zip(["Plains", "Island", "Swamp", "Mountain", "Forest"])
    {
        assert_eq!(branch.card_type, Some(CardTypeFilter::Land));
        assert_eq!(branch.required_subtypes, [subtype]);
        assert!(branch.required_supertypes.is_empty());
    }
    let [group] = channel.targeting.as_ref().unwrap().groups.as_slice() else {
        panic!("one target")
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.effect_indices, [0]);
}
