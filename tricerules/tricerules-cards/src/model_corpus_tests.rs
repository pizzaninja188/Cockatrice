//! Characterization that requires the shipped corpus.
#[test]
fn windfall_is_registered() {
    assert!(crate::registry::global().get("windfall").is_some());
}

#[test]
fn waterbending_lesson_and_chaos_spewer_embed_one_paid_choose_branch_each() {
    use crate::primitives::{CompiledInstructionKind, ResolutionCost};

    let registry = crate::registry::from_embedded().expect("current embedded card corpus");

    let lesson = registry
        .compiled_program("waterbending_lesson", "waterbending_lesson")
        .expect("Waterbending Lesson face program is embedded");
    assert_eq!(lesson.body.steps.len(), 2);
    let CompiledInstructionKind::Choose {
        choice,
        branches,
        otherwise,
        ..
    } = &lesson.body.steps[1].kind
    else {
        panic!("Waterbending Lesson's second instruction is Choose");
    };
    assert_eq!(choice.as_str(), "waterbend_or_discard");
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].branch_id.as_str(), "waterbend_2");
    assert!(matches!(&branches[0].cost, ResolutionCost::Waterbend(_)));
    assert_eq!(branches[0].body.steps.len(), 0);
    assert_eq!(otherwise.steps.len(), 1);
    assert_eq!(otherwise.steps[0].id, "discard_a_card");

    let spewer_owner =
        crate::identity::ProgramOwnerId::ability(crate::identity::AbilityDefinitionId {
            card_id: "chaos_spewer".into(),
            face_id: crate::identity::CardFaceId::new("chaos_spewer").unwrap(),
            ability_path: vec![crate::identity::AbilityId::new("triggered_01").unwrap()],
        });
    let spewer = registry
        .compiled_program_for_owner(&spewer_owner)
        .expect("Chaos Spewer triggered program is embedded");
    assert_eq!(spewer.body.steps.len(), 1);
    let CompiledInstructionKind::Choose {
        choice,
        branches,
        otherwise,
        ..
    } = &spewer.body.steps[0].kind
    else {
        panic!("Chaos Spewer's triggered program is a Choose");
    };
    assert_eq!(choice.as_str(), "pay_or_blight");
    assert_eq!(branches.len(), 1);
    assert_eq!(branches[0].branch_id.as_str(), "pay_2");
    assert!(matches!(&branches[0].cost, ResolutionCost::Mana(_)));
    assert!(branches[0].body.steps.is_empty());
    assert_eq!(otherwise.steps.len(), 1);
    assert_eq!(otherwise.steps[0].id, "blight_2");
}

#[test]
fn earthbend_nonland_permanent_discard_filter_is_shared() {
    let filter: crate::primitives::CardTypeFilter = ron::from_str("NonlandPermanent")
        .expect("Dai Li Indoctrination and Auntie's Sentence need this filter");
    let registry = crate::registry::global();
    for (card, expected) in [
        ("grizzly_bears", true),
        ("liquimetal_coating", true),
        ("unholy_indenture", true),
        ("forest", false),
        ("lightning_bolt", false),
        ("divination", false),
    ] {
        assert_eq!(
            registry
                .get(card)
                .unwrap()
                .matches_card_type_outside_stack(filter),
            expected,
            "{card}"
        );
    }
}

#[test]
fn simple_ability_fallbacks_describe_typed_costs_and_effects() {
    let registry = crate::registry::from_embedded().unwrap();
    let forest = registry.get("forest").unwrap();
    assert_eq!(
        forest.faces[0].activated_abilities[0].fallback_text("Forest"),
        "{T}: Add {G}."
    );
    let ability: crate::primitives::ActivatedAbilityDef = ron::from_str(
        r#"(ability_id: "synthetic_draw", presentation: Fallback, costs: [Mana("{2}"), Tap, SacrificeSelf], effect: [Draw(count: 1)])"#,
    ).unwrap();
    assert_eq!(
        ability.fallback_text("Clue"),
        "{2}, {T}, Sacrifice Clue: Draw a card."
    );
    let trigger: crate::primitives::TriggeredAbilityDef = ron::from_str(
        r#"(ability_id: "triggered_01", presentation: Fallback, trigger: WhenSelfEntersBattlefield, effect: [GainLife(amount: 3)])"#,
    ).unwrap();
    assert_eq!(
        trigger.fallback_text("Healer"),
        "When Healer enters, you gain 3 life."
    );
}

#[test]
fn simple_targeted_fallbacks_cover_map_and_granted_damage_abilities() {
    let map = crate::registry::global().get("map").unwrap();
    assert_eq!(map.faces[0].activated_abilities[0].fallback_text("Map"),
        "{1}, {T}, Sacrifice Map: Target creature you control explores. Activate only as a sorcery.");
    let ability: crate::primitives::ActivatedAbilityDef = ron::from_str(
        r#"(ability_id: "granted_01", presentation: Fallback, costs: [Tap], effect: [DamageTarget(amount: 1, target: (kind: AnyTarget))])"#,
    ).unwrap();
    assert_eq!(
        ability.fallback_text("Source"),
        "{T}: Deal 1 damage to any target."
    );
}
