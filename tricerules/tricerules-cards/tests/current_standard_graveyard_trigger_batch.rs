//! Direct-RON conformance for two corrected Standard graveyard ETBs. Their source identities and
//! Oracle text are pinned in Scryfall `oracle_cards` snapshot
//! `27bf3214-1271-490b-bdfe-c0be6c23d02e` (SHA-256
//! `9611b5d93b20478a0ee46bae8b20a9eb39ee980f0ef4f5f6f6aaa8f7ab010ab2`). Rooftop Percher's
//! 2025-11-17 ruling confirms that choosing no targets is legal; its Oracle plural “graveyards”
//! permits targets in different graveyards. Soul-Shackled Zombie's Oracle says “from a single
//! graveyard” and “loses 2 life”; its current rulings list adds no card-specific clarification.
//! Governed rules: triggered abilities (CR 603.1), target declaration/revalidation (CR 115.1,
//! 608.2b), and effects that cause life loss rather than damage (CR 119.3).

use tricerules_cards::primitives::*;
use tricerules_cards::{AbilityDefinitionId, AbilityPresentation, ProgramOwnerId};

#[test]
fn rooftop_percher_allows_zero_targets_across_graveyards() {
    let face = tricerules_cards::registry::global()
        .get("rooftop_percher")
        .expect("Rooftop Percher is registered")
        .primary_face();
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Rooftop Percher has one ETB ability");
    };
    let targeting = ability.targeting.as_ref().expect("graveyard targets");
    let [group] = targeting.groups.as_slice() else {
        panic!("Rooftop Percher uses one target group");
    };

    assert_eq!((group.min, group.max), (0, 2));
    assert_eq!(
        group.prompt,
        "Choose up to two target cards from graveyards"
    );
    assert!(!group.same_graveyard);
    assert!(TargetSchema::compile(&ability.effect, Some(targeting)).is_ok());
}

#[test]
fn soul_shackled_zombie_loses_life_only_when_a_creature_was_exiled() {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get("soul-shackled_zombie")
        .expect("Soul-Shackled Zombie is registered");
    let face = card.primary_face();
    let [ability] = face.triggered_abilities.as_slice() else {
        panic!("Soul-Shackled Zombie has one ETB ability");
    };
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(ability.effect.is_empty(), "the ETB uses its Program body");
    assert!(
        ability.program.is_some(),
        "the complete ETB owner is authored"
    );

    let owner = ProgramOwnerId::ability(AbilityDefinitionId {
        card_id: "soul-shackled_zombie".into(),
        face_id: face.face_id.clone(),
        ability_path: vec![ability.ability_id.clone()],
    });
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("Soul-Shackled Zombie's exact ETB owner has a compiled Program");
    assert_eq!(program.owner, owner);
    assert_eq!(
        program.target_projection,
        ProgramTargetProjection::OwnerLocal
    );
    assert_eq!(
        program.target_bindings,
        [ProgramTargetBinding {
            instruction_address: 0,
            target_group_index: 0,
            role_index: 0,
        }]
    );

    let [exile, count_gate] = program.body.steps.as_slice() else {
        panic!("the ETB binds its Exile result before the creature count");
    };
    assert_eq!(exile.id, "exile_graveyard_cards");
    assert_eq!(exile.outputs.len(), 1);
    let exile_result = exile.outputs[0];
    assert_eq!(
        program.slot(exile_result).unwrap().result_type,
        ResultType::TargetCardResultCohort {
            action: CardResultAction::Exile,
            cardinality: Cardinality {
                min: 0,
                max: Some(2),
            },
            target_group_index: 0,
        }
    );
    let CompiledInstructionKind::Perform(effect) = &exile.kind else {
        panic!("the first Program instruction performs the Exile");
    };
    assert!(matches!(
        effect.as_ref(),
        SpellEffectKind::MoveGraveyardCards {
            filter: GraveyardFilter {
                owner: GraveyardOwner::AnyPlayer,
                card: None,
                mana_value_less_than_trigger_object: false,
                ..
            },
            destination: GraveyardDestination::Exile,
            linked_exile_id: None,
            entry_modifiers,
        } if entry_modifiers.is_empty()
    ));

    let CompiledInstructionKind::If { condition, yes, no } = &count_gate.kind else {
        panic!("the named Exile result gates the life effects");
    };
    assert!(matches!(
        condition.as_ref(),
        CompiledProgramCondition::Result(
            CompiledProgramResultCondition::CardResultCount {
                result,
                card_type: Some(CardTypeFilter::Creature),
                min: Some(1),
                max: None,
            }
        ) if *result == exile_result
    ));
    let [lose_life, gain_life] = yes.steps.as_slice() else {
        panic!("a creature Exile causes one opponent life-loss leaf and one controller gain");
    };
    assert!(matches!(
        &lose_life.kind,
        CompiledInstructionKind::Perform(effect)
            if **effect == SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(2),
                who: PlayerRecipient::EachOpponent,
            }
    ));
    assert!(matches!(
        &gain_life.kind,
        CompiledInstructionKind::Perform(effect)
            if **effect == SpellEffectKind::GainLife {
                amount: Amount::Fixed(2),
                who: tricerules_cards::primitives::PlayerRecipient::Controller,
            }
    ));
    assert!(no.steps.is_empty() && no.yields.is_empty());

    let targeting = ability.targeting.as_ref().expect("graveyard targets");
    let [group] = targeting.groups.as_slice() else {
        panic!("Soul-Shackled Zombie uses one target group");
    };
    assert_eq!((group.min, group.max), (0, 2));
    assert!(group.same_graveyard);
    assert!(group.effect_indices.is_empty());
    let schema = TargetSchema::compile_program(program, Some(targeting))
        .expect("the Program target belongs to one graveyard-card role");
    assert_eq!(schema.groups.len(), 1);
    assert_eq!(schema.groups[0].bindings.len(), 1);
    assert!(matches!(
        schema.groups[0].bindings[0].role,
        TargetRole::GraveyardCard(_)
    ));
}
