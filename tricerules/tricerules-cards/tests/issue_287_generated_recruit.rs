use tricerules_cards::primitives::{
    CardResultAction, CardResultFilter, CardResultSource, CardTypeFilter, DrawDiscardOrder,
    InstructionKind, PlayerRecipient, ProgramResultCondition, RelativePlayerSet,
    ResolutionBranchRequirement, ResolutionBranchSelection, ResolutionCost, SpellEffectKind,
};
use tricerules_cards::{
    AbilityDefinitionId, AbilityId, AbilityPresentation, Amount, CardFaceId, ChoiceId,
    ProgramOwnerId, TriggerCondition,
};

#[test]
fn issue_287_registers_both_generated_recruit_etb_cards_with_exact_programs() {
    let registry = tricerules_cards::registry::global();
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
            ability.effect.is_empty(),
            "{id} uses its authored program body"
        );
        let program = ability
            .program
            .as_ref()
            .unwrap_or_else(|| panic!("{id} must register its authored ETB program"));
        assert_eq!(program.target_bindings, []);
        assert!(program.body.yields.is_empty());
        let [draw, discard, condition] = program.body.steps.as_slice() else {
            panic!("{id} must register the three-step Recruit program");
        };
        assert_eq!(draw.id, "draw_one");
        assert_eq!(
            draw.kind,
            InstructionKind::Perform(Box::new(SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }))
        );
        assert_eq!(discard.id, "discard_one");
        assert_eq!(discard.bind, ["discarded"]);
        assert_eq!(
            discard.kind,
            InstructionKind::Perform(Box::new(SpellEffectKind::Discard {
                who: PlayerRecipient::Controller,
                quantity: tricerules_cards::primitives::DiscardQuantity::Exact(1),
            }))
        );
        assert_eq!(condition.id, "if_discarded_nonland");
        let InstructionKind::IfResult { condition, yes, no } = &condition.kind else {
            panic!("{id} must test the typed discard result");
        };
        assert_eq!(
            *condition,
            ProgramResultCondition::DiscardCount {
                result: "discarded".into(),
                players: RelativePlayerSet::Controller,
                card_type: Some(CardTypeFilter::Nonland),
                min: Some(1),
                max: None,
            }
        );
        assert!(yes.yields.is_empty());
        let [create_soldier] = yes.steps.as_slice() else {
            panic!("{id} must create one Soldier only in the successful discard branch");
        };
        assert_eq!(create_soldier.id, "create_soldier");
        assert_eq!(
            create_soldier.kind,
            InstructionKind::Perform(Box::new(SpellEffectKind::CreateTokens {
                token: "human_soldier_w_1_1".into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            }))
        );
        assert!(no.steps.is_empty());
        assert!(no.yields.is_empty());

        let owner = ProgramOwnerId::ability(AbilityDefinitionId {
            card_id: id.into(),
            face_id: CardFaceId::new(id).unwrap(),
            ability_path: vec![AbilityId::new("triggered_01").unwrap()],
        });
        assert_eq!(
            registry
                .compiled_program_for_owner(&owner)
                .map(|compiled| &compiled.owner),
            Some(&owner),
            "{id} must compile under its exact triggered ability owner"
        );
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
