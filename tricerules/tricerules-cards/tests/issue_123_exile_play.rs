use tricerules_cards::primitives::{
    Amount, CompiledInstructionKind, CompiledProgramCondition, GameCondition, GraveyardAggregate,
    PlayerRecipient, ProgramTargetProjection, RelativePlayerSet, SpellEffectKind, TargetFilter,
    TargetKind,
};
use tricerules_cards::{EffectContext, ProgramOwnerId, TriggerCondition};

#[test]
fn issue_123_cards_use_the_generic_exile_play_effect() {
    let registry = tricerules_cards::registry::global();

    let percussionist = registry
        .get("clockwork_percussionist")
        .expect("Clockwork Percussionist");
    let trigger = &percussionist.primary_face().triggered_abilities[0];
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfDies);
    assert!(matches!(
        trigger.effect.as_slice(),
        [SpellEffectKind::ExileTopWithPlayPermission {
            player: PlayerRecipient::Controller,
            ..
        }]
    ));
}

#[test]
fn impossible_inferno_uses_the_exact_targeted_delirium_program() {
    let registry = tricerules_cards::registry::global();
    let inferno = registry
        .get("impossible_inferno")
        .expect("Impossible Inferno");
    let face = inferno.primary_face();
    assert!(face.spell_effect.is_empty());
    let owner = ProgramOwnerId::face("impossible_inferno", face.face_id.clone());
    let program = registry
        .compiled_program_for_owner(&owner)
        .expect("Impossible Inferno's exact face has a compiled spell Program");
    assert_eq!(program.owner, owner);
    assert_eq!(program.context, EffectContext::Spell);
    assert_eq!(
        program.target_projection,
        ProgramTargetProjection::OwnerLocal
    );

    let [damage, delirium] = program.body.steps.as_slice() else {
        panic!("Impossible Inferno damages its target before checking Delirium");
    };
    let CompiledInstructionKind::Perform(effect) = &damage.kind else {
        panic!("the first Program instruction deals target damage");
    };
    assert_eq!(
        **effect,
        SpellEffectKind::DamageTarget {
            amount: Amount::Fixed(6),
            target: TargetFilter {
                kind: TargetKind::Creature,
                ..TargetFilter::default()
            },
        }
    );
    assert!(damage.input.is_none() && damage.outputs.is_empty());
    assert_eq!(program.target_bindings.len(), 1);
    assert_eq!(
        program.target_bindings[0].instruction_address,
        damage.instruction_address
    );
    assert_eq!(program.target_bindings[0].target_group_index, 0);
    assert_eq!(program.target_bindings[0].role_index, 0);

    let CompiledInstructionKind::If { condition, yes, no } = &delirium.kind else {
        panic!("Delirium is an engine-evaluated ProgramIf, not a player choice");
    };
    assert!(delirium.input.is_none() && delirium.outputs.is_empty());
    assert!(matches!(
        condition.as_ref(),
        CompiledProgramCondition::Game(GameCondition::GraveyardAggregate {
            owners: RelativePlayerSet::Controller,
            aggregate: GraveyardAggregate::DistinctCardTypes,
            filter: None,
            min: Some(4),
            max: None,
        })
    ));
    let [exile_top] = yes.steps.as_slice() else {
        panic!("four distinct graveyard card types select exactly one exile effect");
    };
    let CompiledInstructionKind::Perform(effect) = &exile_top.kind else {
        panic!("the Delirium arm is one resultless effect");
    };
    assert_eq!(
        **effect,
        SpellEffectKind::ExileTopWithPlayPermission {
            player: PlayerRecipient::Controller,
            count: 1,
            count_by_cast_cost: None,
        }
    );
    assert!(exile_top.input.is_none() && exile_top.outputs.is_empty());
    assert!(no.steps.is_empty() && no.yields.is_empty());

    let targeting = face.targeting.as_ref().expect("one target group");
    let [group] = targeting.groups.as_slice() else {
        panic!("Impossible Inferno has one target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert!(group.effect_indices.is_empty());
    assert_eq!(group.prompt, "Choose target creature");
}
