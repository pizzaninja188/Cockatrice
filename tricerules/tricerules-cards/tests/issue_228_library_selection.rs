use tricerules_cards::primitives::{
    CompiledInstructionKind, CompiledProgramCondition, EffectContext, GameCondition,
    LibraryBottomOrder, SpellEffectKind, ZoneCardFilter,
};
use tricerules_cards::ProgramOwnerId;

#[test]
fn issue_228_bounds_and_unfiltered_selection_are_validated() {
    for (count, min, max, valid) in [
        (3, 2, 2, true),
        (3, 0, 1, true),
        (0, 0, 1, false),
        (2, 2, 1, false),
        (2, 0, 3, false),
        (2, 0, 0, false),
    ] {
        let mut effect = SpellEffectKind::LookChooseToHand {
            count,
            filter: None,
            min,
            max,
            reveal: false,
            bottom_order: LibraryBottomOrder::Chosen,
        };
        assert_eq!(effect.validate(EffectContext::Spell).is_ok(), valid);
        if let SpellEffectKind::LookChooseToHand { filter, .. } = &mut effect {
            *filter = Some(ZoneCardFilter::default());
        }
        assert!(
            effect.validate(EffectContext::Spell).is_err(),
            "empty filters still fail closed"
        );
    }
}

#[test]
fn issue_228_cards_and_existing_revealed_consumers_are_complete() {
    let registry = tricerules_cards::registry::global();
    for (id, name, mana) in [
        ("sleight_of_hand", "Sleight of Hand", "{U}"),
        ("flow_state", "Flow State", "{1}{U}"),
    ] {
        let card = registry.get(id).unwrap();
        assert_eq!(registry.id_for_name(name), Some(id));
        assert_eq!(card.primary_face().mana_cost.to_string(), mana);
        assert_eq!(card.primary_face().types, ["Sorcery"]);
    }
    let flow_face = registry.get("flow_state").unwrap().primary_face();
    assert!(flow_face.spell_effect.is_empty());
    let flow_owner = ProgramOwnerId::face("flow_state", flow_face.face_id.clone());
    let flow_program = registry
        .compiled_program_for_owner(&flow_owner)
        .expect("Flow State has a compiled face-owned Program");
    assert_eq!(flow_program.owner, flow_owner);
    let [gate] = flow_program.body.steps.as_slice() else {
        panic!("Flow State checks its graveyard when the spell resolves");
    };
    let CompiledInstructionKind::If { condition, yes, no } = &gate.kind else {
        panic!("Flow State uses an automatic ProgramIf");
    };
    assert!(matches!(
        condition.as_ref(),
        CompiledProgramCondition::Game(GameCondition::AllOf(conditions))
            if conditions.len() == 2
    ));
    assert!(yes.yields.is_empty() && no.yields.is_empty());
    for (block, expected_count) in [(yes, 2), (no, 1)] {
        let [look] = block.steps.as_slice() else {
            panic!("each Flow State branch performs one library look");
        };
        let CompiledInstructionKind::Perform(operation) = &look.kind else {
            panic!("each Flow State branch performs its private library look");
        };
        let SpellEffectKind::LookChooseToHand {
            count,
            filter,
            min,
            max,
            reveal,
            bottom_order,
        } = operation.as_ref()
        else {
            panic!("each Flow State branch performs its private library look");
        };
        assert_eq!(*count, 3);
        assert!(filter.is_none());
        assert_eq!((*min, *max), (expected_count, expected_count));
        assert!(!reveal);
        assert_eq!(*bottom_order, LibraryBottomOrder::Chosen);
    }
    for id in [
        "commune_with_nature",
        "brightwood_tracker",
        "living_phone",
        "star_charter",
    ] {
        let face = registry.get(id).unwrap().primary_face();
        let effects = if id == "commune_with_nature" {
            &face.spell_effect
        } else if id == "brightwood_tracker" {
            &face.activated_abilities[0].effect
        } else {
            &face.triggered_abilities[0].effect
        };
        assert!(matches!(
            effects.as_slice(),
            [SpellEffectKind::LookChooseToHand {
                min: 0,
                max: 1,
                reveal: true,
                filter: Some(_),
                ..
            }]
        ));
    }
}
