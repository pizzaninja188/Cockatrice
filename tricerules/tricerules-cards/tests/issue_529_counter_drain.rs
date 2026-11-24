use crate::common;

use common::FaceExpectation;
use tricerules_cards::primitives::{
    CardTypeFilter, InstructionKind, LifeAmount, PlayerRecipient, ProgramTargetRole,
    SpellEffectKind,
};

fn check_counter_drain(
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    filter: Option<CardTypeFilter>,
    loss: u32,
    gain: Option<u32>,
) {
    let face = FaceExpectation {
        id,
        name,
        face_id: id,
        mana_cost,
        types: &["Instant"],
        keywords: &[],
        power_toughness: None,
    }
    .check();
    assert!(
        face.spell_effect.is_empty(),
        "{id}: the Program owns resolution"
    );
    let program = face.spell_program.as_ref().expect("typed spell Program");
    let expected_steps = if gain.is_some() { 3 } else { 2 };
    assert_eq!(program.body.steps.len(), expected_steps, "{id}");

    let counter = &program.body.steps[0];
    assert_eq!(counter.id, "counter_spell");
    assert_eq!(counter.bind, ["target_controller"]);
    let InstructionKind::Perform(counter_effect) = &counter.kind else {
        panic!("{id}: the controller receipt comes from a counter instruction");
    };
    let SpellEffectKind::CounterTargetSpell {
        spell_filter,
        unless_controller_pays,
        unless_controller_pays_by_cast_cost,
    } = counter_effect.as_ref()
    else {
        panic!("{id}: the controller receipt comes from a stack-spell counter");
    };
    assert_eq!(spell_filter.card_type, filter, "{id}");
    assert!(unless_controller_pays.is_none() && unless_controller_pays_by_cast_cost.is_none());

    let drain = &program.body.steps[1];
    assert_eq!(drain.input.as_deref(), Some("target_controller"));
    let InstructionKind::Perform(drain_effect) = &drain.kind else {
        panic!("{id}: the typed receipt feeds the life-loss instruction");
    };
    assert_eq!(
        drain_effect.as_ref(),
        &SpellEffectKind::LoseLife {
            amount: LifeAmount::Fixed(loss),
            who: PlayerRecipient::ProgramInputPlayer,
        },
        "{id}"
    );

    if let Some(gain) = gain {
        let gain_step = &program.body.steps[2];
        assert!(
            gain_step.input.is_none(),
            "{id}: the caster gain takes no receipt"
        );
        let InstructionKind::Perform(gain_effect) = &gain_step.kind else {
            panic!("{id}: the caster gains life");
        };
        let SpellEffectKind::GainLife { amount, who } = gain_effect.as_ref() else {
            panic!("{id}: the caster gains life");
        };
        assert_eq!(*who, PlayerRecipient::Controller);
        assert_eq!(*amount, tricerules_cards::Amount::Fixed(gain));
    }

    assert_eq!(program.target_bindings.len(), 1);
    assert_eq!(program.target_bindings[0].instruction_id, "counter_spell");
    assert_eq!(program.target_bindings[0].role, ProgramTargetRole::Spell);
    assert_eq!(program.target_bindings[0].target_group_index, 0);
    let groups = &face.targeting.as_ref().unwrap().groups;
    assert_eq!(groups.len(), 1);
    assert_eq!((groups[0].min, groups[0].max), (1, 1));
}

#[test]
fn issue_529_registers_counter_drain_cohort() {
    check_counter_drain(
        "countersquall",
        "Countersquall",
        "{U}{B}",
        Some(CardTypeFilter::Noncreature),
        2,
        None,
    );
    check_counter_drain("undermine", "Undermine", "{U}{U}{B}", None, 3, None);
    check_counter_drain(
        "psychic_barrier",
        "Psychic Barrier",
        "{U}{U}",
        Some(CardTypeFilter::Creature),
        1,
        None,
    );
    check_counter_drain(
        "punish_ignorance",
        "Punish Ignorance",
        "{W}{U}{U}{B}",
        None,
        3,
        Some(3),
    );
}
