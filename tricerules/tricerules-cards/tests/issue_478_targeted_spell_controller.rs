use crate::common;

use common::FaceExpectation;
use tricerules_cards::primitives::{
    CardTypeFilter, InstructionKind, PlayerRecipient, ProgramTargetRole, SpellEffectKind,
};

#[test]
fn issue_478_registers_complete_offer() {
    let face = FaceExpectation {
        id: "an_offer_you_cant_refuse",
        name: "An Offer You Can't Refuse",
        face_id: "an_offer_you_cant_refuse",
        mana_cost: "{U}",
        types: &["Instant"],
        keywords: &[],
        power_toughness: None,
    }
    .check();
    assert!(
        face.spell_effect.is_empty(),
        "the authored Program owns spell resolution"
    );
    let program = face.spell_program.as_ref().expect("typed spell Program");
    let [counter, treasure] = program.body.steps.as_slice() else {
        panic!("noncreature counter feeds the Treasure instruction");
    };
    assert_eq!(counter.id, "counter_spell");
    assert_eq!(counter.bind, ["target_controller"]);
    let InstructionKind::Perform(counter_effect) = &counter.kind else {
        panic!("the controller receipt comes from a counter instruction");
    };
    let SpellEffectKind::CounterTargetSpell { spell_filter, .. } = counter_effect.as_ref() else {
        panic!("the controller receipt comes from a stack-spell counter");
    };
    assert_eq!(treasure.input.as_deref(), Some("target_controller"));
    let InstructionKind::Perform(treasure_effect) = &treasure.kind else {
        panic!("the typed receipt feeds the token instruction");
    };
    let SpellEffectKind::CreateTokens {
        token, count, who, ..
    } = treasure_effect.as_ref()
    else {
        panic!("the counter result feeds two Treasures");
    };
    assert_eq!(spell_filter.card_type, Some(CardTypeFilter::Noncreature));
    assert_eq!(token, "treasure");
    assert_eq!(*count, tricerules_cards::Amount::Fixed(2));
    assert_eq!(*who, PlayerRecipient::ProgramInputPlayer);
    assert_eq!(program.target_bindings.len(), 1);
    assert_eq!(program.target_bindings[0].instruction_id, "counter_spell");
    assert_eq!(program.target_bindings[0].role, ProgramTargetRole::Spell);
    assert_eq!(program.target_bindings[0].target_group_index, 0);
    assert!(face.targeting.as_ref().unwrap().groups[0]
        .effect_indices
        .is_empty());
}
