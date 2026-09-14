use tricerules_cards::primitives::{
    EffectSubject, LifeAmount, PermanentTypeFilter, PlayerRecipient, SpellCostModifier,
    SpellEffectKind, TargetController, TargetKind, TargetMatchFilter,
};
use tricerules_cards::{
    AbilityPresentation, Amount, CardRegistry, Keyword, LibraryPartitionKind, TriggerCondition,
};

#[test]
fn issue_271_registers_exactly_the_reviewed_eleven_card_shapes() {
    let registry = CardRegistry::global();
    for id in [
        "quicksand_whirlpool",
        "grounded_for_life",
        "ajanis_response",
        "liminal_hold",
        "prayer_of_binding",
        "risky_research",
        "diresight",
        "unauthorized_exit",
        "banishing_betrayal",
        "quarrel",
        "rocky_rebuke",
    ] {
        registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
    }

    for (id, exiles) in [
        ("quicksand_whirlpool", true),
        ("grounded_for_life", false),
        ("ajanis_response", false),
    ] {
        let face = registry.get(id).unwrap().primary_face();
        let [SpellCostModifier::TargetMatchGenericReduction { amount, filter }] =
            face.cost_modifiers.as_slice()
        else {
            panic!("{id} must have one target-matching reduction");
        };
        assert_eq!(*amount, 3);
        let TargetMatchFilter::Battlefield(filter) = filter else {
            panic!("{id} must inspect a battlefield target");
        };
        assert_eq!(filter.kind, TargetKind::Creature);
        assert_eq!(filter.tapped, Some(true));
        assert_eq!(face.spell_effect.len(), 1);
        assert_eq!(
            matches!(face.spell_effect[0], SpellEffectKind::Exile { .. }),
            exiles,
            "{id} removal kind"
        );
        let subject = match &face.spell_effect[0] {
            SpellEffectKind::Destroy { subject } | SpellEffectKind::Exile { subject } => subject,
            effect => panic!("{id} unexpected removal effect: {effect:?}"),
        };
        let EffectSubject::Chosen(target) = subject else {
            panic!("{id} removal must choose a target");
        };
        assert_eq!(target.kind, TargetKind::Creature);
    }

    for id in ["liminal_hold", "prayer_of_binding"] {
        let face = registry.get(id).unwrap().primary_face();
        let [ability] = face.triggered_abilities.as_slice() else {
            panic!("{id} must have one ETB ability");
        };
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![if id == "prayer_of_binding" { 2 } else { 1 }])
        );
        let [SpellEffectKind::ExileUntilSourceLeaves { target }, SpellEffectKind::GainLife { amount }] =
            ability.effect.as_slice()
        else {
            panic!("{id} must exile then gain life");
        };
        assert_eq!(target.kind, TargetKind::AnyPermanent);
        assert_eq!(target.controller, TargetController::Opponent);
        assert_eq!(target.excluded_permanent_types, [PermanentTypeFilter::Land]);
        assert_eq!(*amount, Amount::Fixed(2));
        let groups = &ability.targeting.as_ref().unwrap().groups;
        assert_eq!((groups[0].min, groups[0].max), (0, 1));
        assert_eq!(groups[0].effect_indices, [0]);
        assert!(!ability.may);
    }
    assert!(registry
        .get("prayer_of_binding")
        .unwrap()
        .primary_face()
        .keywords
        .contains(&Keyword::Flash));

    for id in ["risky_research", "diresight"] {
        assert_eq!(
            registry.get(id).unwrap().primary_face().spell_effect,
            [
                SpellEffectKind::LibraryPartition {
                    count: 2,
                    top_min: 0,
                    top_max: None,
                    kind: LibraryPartitionKind::Surveil,
                },
                SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(2),
                },
                SpellEffectKind::LoseLife {
                    amount: LifeAmount::Fixed(2),
                    who: PlayerRecipient::Controller,
                },
            ],
            "{id} effect order"
        );
    }

    for id in ["unauthorized_exit", "banishing_betrayal"] {
        let face = registry.get(id).unwrap().primary_face();
        let [SpellEffectKind::ReturnToOwnersHand { subject }, SpellEffectKind::LibraryPartition { count, kind, .. }] =
            face.spell_effect.as_slice()
        else {
            panic!("{id} must bounce then Surveil");
        };
        let EffectSubject::Chosen(target) = subject else {
            panic!("{id} bounce must be targeted");
        };
        assert_eq!(target.kind, TargetKind::AnyPermanent);
        assert_eq!(target.excluded_permanent_types, [PermanentTypeFilter::Land]);
        assert_eq!((*count, *kind), (1, LibraryPartitionKind::Surveil));
        let groups = &face.targeting.as_ref().unwrap().groups;
        assert_eq!((groups[0].min, groups[0].max), (1, 1));
        assert_eq!(groups[0].effect_indices, [0]);
    }

    for id in ["quarrel", "rocky_rebuke"] {
        let face = registry.get(id).unwrap().primary_face();
        let [SpellEffectKind::CreatureDealsDamageEqualToPower { source, target }] =
            face.spell_effect.as_slice()
        else {
            panic!("{id} must use power damage");
        };
        assert_eq!(source.kind, TargetKind::Creature);
        assert_eq!(source.controller, TargetController::You);
        assert_eq!(target.kind, TargetKind::Creature);
        assert_eq!(target.controller, TargetController::Opponent);
        let groups = &face.targeting.as_ref().unwrap().groups;
        assert_eq!(groups.len(), 2);
        assert!(groups.iter().all(|group| (group.min, group.max) == (1, 1)));
        assert!(groups.iter().all(|group| group.effect_indices == [0]));
    }
}
