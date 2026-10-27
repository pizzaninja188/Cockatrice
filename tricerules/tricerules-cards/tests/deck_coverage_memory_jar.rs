//! The hand cohort belongs only to Memory Jar's complete activated/delayed composition.

const JAR: &str = include_str!("../data/memory_jar.ron");

fn reject_valid_ron(label: &str, definition: &str) {
    match tricerules_cards::CardRegistry::from_chunks_and_tokens(&[definition], &[]) {
        Ok(_) => panic!("{label} unexpectedly admitted a Memory Jar cohort operation"),
        Err(error) => assert!(
            !error.to_string().contains("ron parse:"),
            "{label} must reach validation rather than fail parsing: {error}"
        ),
    }
}

#[test]
fn memory_jar_legacy_cohort_operations_require_the_closed_composition() {
    use tricerules_cards::primitives::AbilityCost;
    let registry = tricerules_cards::CardRegistry::from_chunks_and_tokens(&[JAR], &[])
        .expect("the actual Memory Jar composition is admitted");
    let card = registry
        .get("memory_jar")
        .expect("the actual Jar is registered");
    let face = card.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{5}");
    assert_eq!(face.types, ["Artifact"]);
    assert_eq!(face.activated_abilities.len(), 1);
    assert_eq!(
        face.activated_abilities[0].costs,
        [AbilityCost::Tap, AbilityCost::SacrificeSelf]
    );

    let wrong_cost = JAR.replacen("costs: [Tap, SacrificeSelf]", "costs: [Tap]", 1);
    let wrong_child = JAR.replacen(
        "trigger: AtBeginningOfNextEndStep",
        "trigger: AtBeginningOfNextTurnUpkeep",
        1,
    );
    let wrapped = JAR.replacen(
        "MemoryJarExileHands,",
        "Conditional(condition: ControlsCreatureTiedForGreatestPower, effect: MemoryJarExileHands),",
        1,
    );
    for (label, definition) in [
        ("wrong activation cost", wrong_cost.as_str()),
        ("wrong delayed trigger", wrong_child.as_str()),
        ("conditional wrapper", wrapped.as_str()),
    ] {
        assert_ne!(definition, JAR, "the {label} fixture must change the card");
        reject_valid_ron(label, definition);
    }

    let standalone_child = r#"(
        id: "jar_standalone_child", name: "Jar Standalone Child", face_id: "jar_standalone_child",
        types: ["Artifact"], triggered_abilities: [(
            ability_id: "triggered_01", presentation: Fallback,
            trigger: AtBeginningOfNextEndStep,
            effect: [Discard(who: EachPlayer, quantity: All), ReturnMemoryJarHands],
        )],
    )"#;
    reject_valid_ron("standalone delayed child", standalone_child);

    let wrong_context = r#"(
        id: "jar_spell_leaf", name: "Jar Spell Leaf", face_id: "jar_spell_leaf",
        types: ["Sorcery"], spell_effect: [MemoryJarExileHands],
    )"#;
    reject_valid_ron("spell effect", wrong_context);
}

#[test]
fn memory_jar_delayed_child_cannot_be_smuggled_through_a_spell_program() {
    let definition = r#"(
        id: "jar_nested_program", name: "Jar Nested Program", face_id: "jar_nested_program",
        types: ["Sorcery"], spell_program: Some((body: (steps: [
            (id: "create_delay", kind: Perform(CreateDelayedTrigger(
                subject: None,
                ability: (
                    ability_id: "delayed_01", presentation: Fallback,
                    trigger: AtBeginningOfNextEndStep,
                    effect: [Discard(who: EachPlayer, quantity: All), ReturnMemoryJarHands],
                ),
            ))),
        ]))),
    )"#;
    match tricerules_cards::CardRegistry::from_chunks_and_tokens(&[definition], &[]) {
        Ok(_) => panic!("spell Program admitted a Jar delayed child without a captured cohort"),
        Err(error) => assert!(
            error
                .to_string()
                .contains("Memory Jar hand cohorts cannot occur in a Program"),
            "the Program must reject its nested Jar leaf explicitly: {error}"
        ),
    }
}
