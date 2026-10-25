use crate::common;

use common::FaceExpectation;
use tricerules_cards::primitives::{
    CardTypeFilter, InstructionKind, PlayerRecipient, ProgramTargetRole, SpellEffectKind,
};
use tricerules_cards::{Color, Keyword};

const BIRD_TOKEN: &str = "bird_u_2_2_flying";

#[test]
fn swan_song_registers_its_exact_spell_filter_and_bird_token() {
    let face = FaceExpectation {
        id: "swan_song",
        name: "Swan Song",
        face_id: "swan_song",
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
    let [counter, bird] = program.body.steps.as_slice() else {
        panic!("Swan Song counters one spell and creates one Bird for that spell's controller");
    };
    assert_eq!(counter.bind, ["target_controller"]);
    let InstructionKind::Perform(counter_effect) = &counter.kind else {
        panic!("the controller receipt comes from a counter instruction");
    };
    let SpellEffectKind::CounterTargetSpell {
        spell_filter,
        unless_controller_pays,
        unless_controller_pays_by_cast_cost,
    } = counter_effect.as_ref()
    else {
        panic!("the controller receipt comes from a stack-spell counter");
    };
    assert_eq!(bird.input.as_deref(), Some("target_controller"));
    let InstructionKind::Perform(bird_effect) = &bird.kind else {
        panic!("the typed receipt feeds the token instruction");
    };
    let SpellEffectKind::CreateTokens {
        token, count, who, ..
    } = bird_effect.as_ref()
    else {
        panic!("the counter result feeds the Bird token");
    };

    let branches = spell_filter
        .any_of
        .as_ref()
        .expect("the three permitted spell types form an OR filter");
    assert_eq!(
        branches
            .iter()
            .map(|branch| branch.card_type)
            .collect::<Vec<_>>(),
        [
            Some(CardTypeFilter::Enchantment),
            Some(CardTypeFilter::Instant),
            Some(CardTypeFilter::Sorcery),
        ]
    );
    assert!(branches.iter().all(|branch| {
        branch.any_of.is_none()
            && branch.is_color.is_none()
            && branch.min_mana_value.is_none()
            && branch.max_mana_value.is_none()
    }));
    assert!(spell_filter.card_type.is_none());
    assert_eq!(*unless_controller_pays, None);
    assert_eq!(*unless_controller_pays_by_cast_cost, None);
    assert_eq!(token, BIRD_TOKEN);
    assert_eq!(*count, tricerules_cards::Amount::Fixed(1));
    assert_eq!(*who, PlayerRecipient::ProgramInputPlayer);
    assert_eq!(program.target_bindings.len(), 1);
    assert_eq!(program.target_bindings[0].instruction_id, "counter_spell");
    assert_eq!(program.target_bindings[0].role, ProgramTargetRole::Spell);
    assert_eq!(program.target_bindings[0].target_group_index, 0);
    assert!(face.targeting.as_ref().unwrap().groups[0]
        .effect_indices
        .is_empty());

    let registry = tricerules_cards::registry::global();
    assert!(registry.is_token(BIRD_TOKEN));
    let bird = registry.get(BIRD_TOKEN).expect("blue Bird token");
    assert_eq!(bird.name, "Bird");
    let bird_face = bird.primary_face();
    assert_eq!(bird_face.types, ["Creature", "Bird"]);
    assert_eq!(bird_face.colors_override.as_ref().unwrap(), &[Color::Blue]);
    assert_eq!((bird_face.power, bird_face.toughness), (Some(2), Some(2)));
    assert_eq!(bird_face.keywords, [Keyword::Flying]);
}
