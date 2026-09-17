use tricerules_cards::primitives::{
    CardResultAction, CardResultFilter, CardResultSource, CardTypeFilter, DrawDiscardOrder,
    PlayerRecipient, RelativePlayerSet, ResolutionBranchRequirement, ResolutionBranchSelection,
    ResolutionCost, SpellEffectKind,
};
use tricerules_cards::{AbilityPresentation, Amount, CardRegistry, ChoiceId, TriggerCondition};

#[test]
fn issue_287_registers_both_generated_recruit_etb_cards_with_the_exact_result_branch() {
    let registry = CardRegistry::global();
    for (id, name, mana_cost, types, stats, keyword, oracle_line) in [
        (
            "long_lake_nuisance",
            "Long Lake Nuisance",
            "{3}{U}",
            vec!["Creature", "Bird"],
            (3, 1),
            tricerules_cards::Keyword::Flying,
            2,
        ),
        (
            "patient_instructor",
            "Patient Instructor",
            "{2}{W/U}",
            vec!["Creature", "Human", "Citizen"],
            (2, 2),
            tricerules_cards::Keyword::Vigilance,
            2,
        ),
    ] {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.name, name);
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id);
        assert_eq!(face.mana_cost.to_string(), mana_cost);
        assert_eq!(face.types, types);
        assert_eq!(face.power.zip(face.toughness), Some(stats));
        assert_eq!(face.keywords, [keyword]);

        let [ability] = face.triggered_abilities.as_slice() else {
            panic!("{id} must have exactly one generated ETB trigger");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![oracle_line])
        );
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert!(!ability.may);
        assert!(ability.targeting.is_none());
        assert!(
            ability.effect.len() == 2,
            "{id} must emit draw/discard plus the result branch"
        );

        assert_eq!(
            ability.effect[0],
            SpellEffectKind::DrawDiscard {
                who: PlayerRecipient::Controller,
                draw_count: 1,
                discard_count: 1,
                order: DrawDiscardOrder::DrawThenDiscard,
                optional: false,
            }
        );
        let SpellEffectKind::ChooseResolutionBranch {
            chooser,
            optional,
            selection,
            branches,
            otherwise,
        } = &ability.effect[1]
        else {
            panic!("{id} must emit the engine-owned resolution branch");
        };
        assert_eq!(*chooser, PlayerRecipient::Controller);
        assert!(!*optional);
        assert_eq!(*selection, ResolutionBranchSelection::FirstApplicable);
        assert!(otherwise.is_empty());

        let [soldier, fallback] = branches.as_slice() else {
            panic!("{id} must emit exactly the token branch and its fallback");
        };
        assert_eq!(soldier.branch_id.as_str(), "create_a_soldier");
        assert_eq!(soldier.presentation, AbilityPresentation::Fallback);
        assert_eq!(soldier.cost, ResolutionCost::None);
        assert_eq!(
            soldier.requirement,
            ResolutionBranchRequirement::CardResultCount {
                filter: CardResultFilter {
                    source: CardResultSource::PreviousEffect,
                    action: CardResultAction::Discard,
                    players: RelativePlayerSet::Controller,
                    card_type: Some(CardTypeFilter::Nonland),
                },
                min: Some(1),
                max: None,
            }
        );
        assert_eq!(
            soldier.effects,
            [SpellEffectKind::CreateTokens {
                token: "human_soldier_w_1_1".into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            }]
        );
        assert_eq!(fallback.branch_id, ChoiceId::new("no_soldier").unwrap());
        assert_eq!(fallback.requirement, ResolutionBranchRequirement::Always);
        assert!(fallback.effects.is_empty());
    }
}

#[test]
fn issue_287_recruit_assembly_requires_the_typed_discard_result_producer() {
    // The exact assembly the generator emits is valid...
    let soldier = ResolutionBranchRequirement::CardResultCount {
        filter: CardResultFilter {
            source: CardResultSource::PreviousEffect,
            action: CardResultAction::Discard,
            players: RelativePlayerSet::Controller,
            card_type: Some(CardTypeFilter::Nonland),
        },
        min: Some(1),
        max: None,
    };
    let branch = tricerules_cards::primitives::ResolutionBranchDef {
        branch_id: ChoiceId::new("create_a_soldier").unwrap(),
        presentation: AbilityPresentation::Fallback,
        runtime_fallback: None,
        cost: ResolutionCost::None,
        requirement: soldier.clone(),
        effects: vec![SpellEffectKind::CreateTokens {
            token: "human_soldier_w_1_1".into(),
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }],
    };
    let branch_effect = SpellEffectKind::ChooseResolutionBranch {
        chooser: PlayerRecipient::Controller,
        optional: false,
        selection: ResolutionBranchSelection::FirstApplicable,
        branches: vec![
            branch,
            tricerules_cards::primitives::ResolutionBranchDef {
                branch_id: ChoiceId::new("no_soldier").unwrap(),
                presentation: AbilityPresentation::Fallback,
                runtime_fallback: None,
                cost: ResolutionCost::None,
                requirement: ResolutionBranchRequirement::Always,
                effects: Vec::new(),
            },
        ],
        otherwise: Vec::new(),
    };
    assert_eq!(
        SpellEffectKind::validate_list(&[
            SpellEffectKind::DrawDiscard {
                who: PlayerRecipient::Controller,
                draw_count: 1,
                discard_count: 1,
                order: DrawDiscardOrder::DrawThenDiscard,
                optional: false,
            },
            branch_effect.clone(),
        ]),
        Ok(())
    );

    // ...and a branch that cannot read an immediately preceding discard result is rejected, so a
    // malformed configuration can never silently create the token without the discard dependency.
    assert_eq!(
        SpellEffectKind::validate_list(&[
            SpellEffectKind::GainLife {
                amount: Amount::Fixed(1),
            },
            branch_effect,
        ]),
        Err(
            "PreviousEffect card result requires an immediately preceding compatible card-moving effect"
                .into()
        )
    );
}
