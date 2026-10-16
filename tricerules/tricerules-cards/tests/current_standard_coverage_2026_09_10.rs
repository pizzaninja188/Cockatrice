use crate::common::draw_discard_program::{
    assert_ability_draw_discard_steps, AuthoredDrawDiscardExpectation, AuthoredDrawDiscardOrder,
};
use tricerules_cards::primitives::{
    CardTypeFilter, CompiledInstructionKind, CompiledProgramCondition,
    CompiledProgramResultCondition, CreatureScopeController, EffectSubject, GraveyardDestination,
    InstructionKind, RelativePlayerSet, SpellEffectKind, TargetController, TargetKind,
};
use tricerules_cards::{Amount, Color, Keyword, SearchDestination, TriggerCondition};

#[test]
fn audited_cards_have_exact_oracle_characteristics() {
    let registry = tricerules_cards::registry::global();
    let cases = [
        (
            "environmental_scientist",
            "Environmental Scientist",
            "{1}{G}",
            vec!["Creature", "Human", "Druid"],
            Some((2, 2)),
        ),
        (
            "hire_a_crew",
            "Hire a Crew",
            "{2}{R}",
            vec!["Instant"],
            None,
        ),
        (
            "front_porch_sentries",
            "Front Porch Sentries",
            "{1}{B}",
            vec!["Creature", "Goblin", "Soldier"],
            Some((2, 2)),
        ),
        (
            "the_mountain-kings_return",
            "The Mountain-king's Return",
            "{2}{W}",
            vec!["Enchantment", "Saga"],
            None,
        ),
    ];

    for (id, name, mana_cost, types, stats) in cases {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing audited card {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id.replace('-', "_").as_str());
        assert_eq!(face.mana_cost.to_string(), mana_cost);
        assert_eq!(face.types, types);
        assert_eq!(face.power.zip(face.toughness), stats);
    }
}

#[test]
fn mountain_kings_return_composes_recruit_reanimation_and_counter_chapters() {
    let registry = tricerules_cards::registry::global();
    let face = registry
        .get("the_mountain-kings_return")
        .expect("The Mountain-king's Return")
        .primary_face();
    assert_eq!(face.triggered_abilities.len(), 3);
    let chapter_one = &face.triggered_abilities[0];
    assert!(chapter_one.effect.is_empty() && chapter_one.program.is_some());
    let program = assert_ability_draw_discard_steps(
        registry,
        "the_mountain-kings_return",
        &face.face_id,
        &chapter_one.ability_id,
        AuthoredDrawDiscardExpectation {
            first_instruction: 0,
            trailing_instructions: 1,
            who: tricerules_cards::primitives::PlayerRecipient::Controller,
            draw_count: 1,
            discard_count: 1,
            order: AuthoredDrawDiscardOrder::DrawThenDiscard,
        },
    );
    let [_, discarded, branch] = program.body.steps.as_slice() else {
        panic!("Chapter I preserves draw, discard, then its conditional token branch");
    };
    let CompiledInstructionKind::If { condition, yes, no } = &branch.kind else {
        panic!("Chapter I creates a Soldier through a discard-result branch");
    };
    let CompiledProgramCondition::Result(CompiledProgramResultCondition::DiscardCount {
        result,
        players: RelativePlayerSet::Controller,
        card_type: Some(CardTypeFilter::Nonland),
        min: Some(1),
        max: None,
    }) = condition.as_ref()
    else {
        panic!("Chapter I checks for a committed nonland discard");
    };
    assert_eq!(Some(*result), discarded.outputs.first().copied());
    assert!(no.steps.is_empty() && no.yields.is_empty());
    let [soldier] = yes.steps.as_slice() else {
        panic!("a qualifying discard creates exactly one Soldier");
    };
    assert!(matches!(
        &soldier.kind,
        CompiledInstructionKind::Perform(effect)
            if **effect == (SpellEffectKind::CreateTokens {
                token: "human_soldier_w_1_1".into(),
                count: Amount::Fixed(1),
                who: tricerules_cards::primitives::PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            })
    ));
    assert!(matches!(
        face.triggered_abilities[1].effect.as_slice(),
        [SpellEffectKind::MoveGraveyardCards { filter, destination: GraveyardDestination::Battlefield { tapped: false, .. }, .. }]
            if filter.card.as_ref().is_some_and(|card| card.card_type == Some(CardTypeFilter::Creature) && card.max_mana_value == Some(3))
    ));
    assert!(matches!(
        face.triggered_abilities[2].effect.as_slice(),
        [SpellEffectKind::PutCounters { subject: EffectSubject::Chosen(target), .. }]
            if target.kind == TargetKind::Creature
    ));
}

#[test]
fn environmental_scientist_uses_optional_basic_land_search() {
    let face = tricerules_cards::registry::global()
        .get("environmental_scientist")
        .expect("Environmental Scientist")
        .primary_face();
    let ability = &face.triggered_abilities[0];
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(ability.effect.is_empty());
    let program = ability
        .program
        .as_ref()
        .expect("typed entry-search program");
    let [step] = program.body.steps.as_slice() else {
        panic!("one typed entry-search step");
    };
    assert!(matches!(
        &step.kind,
        InstructionKind::Choose { optional: true, branches, otherwise, .. }
            if branches.len() == 1
                && otherwise.steps.is_empty()
                && matches!(branches[0].body.steps.as_slice(),
                    [step] if matches!(&step.kind,
                        InstructionKind::Perform(effect)
                            if matches!(effect.as_ref(), SpellEffectKind::SearchLibrary {
                                filter: Some(filter), destination: SearchDestination::Hand,
                                shuffle: true, reveal: true, ..
                            } if filter.card_type == Some(CardTypeFilter::BasicLand))))
    ));
}

#[test]
fn hire_a_crew_creates_exact_villain_then_pumps_controlled_creatures() {
    let registry = tricerules_cards::registry::global();
    let face = registry
        .get("hire_a_crew")
        .expect("Hire a Crew")
        .primary_face();
    assert!(matches!(
        face.spell_effect.as_slice(),
        [
            SpellEffectKind::CreateTokens { token, count: tricerules_cards::Amount::Fixed(1), .. },
            SpellEffectKind::PumpAll { filter, power: 1, toughness: 0 },
        ] if token == "villain_b_2_1_menace" && filter.controller == Some(CreatureScopeController::YouControl)
    ));

    let token = registry
        .get("villain_b_2_1_menace")
        .expect("black 2/1 Villain token")
        .primary_face();
    assert!(registry.is_token("villain_b_2_1_menace"));
    assert_eq!(token.name, "Villain");
    assert_eq!(token.types, ["Creature", "Villain"]);
    assert_eq!(token.colors_override, Some(vec![Color::Black]));
    assert_eq!((token.power, token.toughness), (Some(2), Some(1)));
    assert_eq!(token.keywords, [Keyword::Menace]);
}

#[test]
fn front_porch_sentries_has_an_opponent_creature_death_trigger() {
    let face = tricerules_cards::registry::global()
        .get("front_porch_sentries")
        .expect("Front Porch Sentries")
        .primary_face();
    let ability = &face.triggered_abilities[0];
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfDies);
    assert!(matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::PumpTarget { power: -1, toughness: -1, subject: EffectSubject::Chosen(target), .. }]
            if target.kind == TargetKind::Creature && target.controller == TargetController::Opponent
    ));
}
