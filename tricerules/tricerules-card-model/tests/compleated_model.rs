use tricerules_card_model::card_def::RawCardDefinition;
use tricerules_card_model::primitives::{
    Amount, CounterKind, EntersWithCountersAffected, StaticAbilityDef,
};
use tricerules_card_model::{AbilityPresentation, CardRegistry};

#[test]
fn compleated_raw_ron_preserves_printed_loyalty_and_intrinsic_entry_counter() {
    let probes = [
        (
            "compleated_vraska_model_probe",
            "Compleated Vraska Model Probe",
            "Vraska",
            6,
            r#"(
                id: "compleated_vraska_model_probe",
                name: "Compleated Vraska Model Probe",
                face_id: "compleated_vraska_model_probe",
                mana_cost: "{4}{B}{B/P}",
                supertypes: ["Legendary"],
                types: ["Planeswalker", "Vraska"],
                loyalty: Some(6),
                static_abilities: [(
                    ability_id: "compleated",
                    presentation: OracleLines([1]),
                    definition: Compleated,
                )],
            )"#,
        ),
        (
            "compleated_nissa_model_probe",
            "Compleated Nissa Model Probe",
            "Nissa",
            7,
            r#"(
                id: "compleated_nissa_model_probe",
                name: "Compleated Nissa Model Probe",
                face_id: "compleated_nissa_model_probe",
                mana_cost: "{3}{G}{G}{G/P}{G/P}",
                supertypes: ["Legendary"],
                types: ["Planeswalker", "Nissa"],
                loyalty: Some(7),
                static_abilities: [(
                    ability_id: "compleated",
                    presentation: OracleLines([1]),
                    definition: Compleated,
                )],
            )"#,
        ),
    ];

    for (id, name, subtype, printed_loyalty, ron) in probes {
        let raw = ron::from_str::<RawCardDefinition>(ron)
            .expect("the positive raw RON probe should parse Compleated");
        assert_eq!(raw.id, id);
        assert_eq!(raw.name, name);
        assert_eq!(raw.face_id.as_str(), id);
        assert_eq!(raw.loyalty, Some(printed_loyalty));
        assert!(raw.types.iter().any(|card_type| card_type == subtype));

        let authored_compleated = raw
            .static_abilities
            .iter()
            .find(|ability| ability.ability_id.as_str() == "compleated")
            .expect("the authored Compleated ability should retain its identity");
        assert_eq!(
            authored_compleated.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        let serialized = ron::to_string(&authored_compleated.definition)
            .expect("the Compleated unit definition should serialize");
        assert_eq!(serialized, "Compleated");
        let round_tripped = ron::from_str::<StaticAbilityDef>(&serialized)
            .expect("the serialized unit should round-trip through StaticAbilityDef");
        assert_eq!(round_tripped, authored_compleated.definition);

        let registry = CardRegistry::from_chunks_and_tokens(&[ron], &[])
            .expect("the isolated probe should normalize without token definitions");
        let card = registry
            .get(id)
            .expect("the isolated registry should retain the probe identity");
        assert_eq!(card.id, id);
        assert_eq!(card.name, name);

        let face = card.primary_face();
        assert_eq!(face.face_id.as_str(), id);
        assert_eq!(face.name, name);
        assert_eq!(face.loyalty, Some(printed_loyalty));
        assert!(face.types.iter().any(|card_type| card_type == subtype));
        let normalized_compleated = face
            .static_abilities
            .iter()
            .find(|ability| ability.ability_id.as_str() == "compleated")
            .expect("normalization should preserve the authored Compleated ability");
        assert_eq!(
            normalized_compleated.presentation,
            AbilityPresentation::OracleLines(vec![1])
        );
        assert_eq!(normalized_compleated.definition, round_tripped);

        let intrinsic_loyalty = face
            .static_abilities
            .iter()
            .filter(|ability| ability.ability_id.as_str() == "intrinsic_loyalty")
            .collect::<Vec<_>>();
        assert_eq!(intrinsic_loyalty.len(), 1);
        assert_eq!(
            &intrinsic_loyalty[0].definition,
            &StaticAbilityDef::EntersWithCounters {
                affected: EntersWithCountersAffected::Self_,
                counter: CounterKind::Loyalty,
                amount: Amount::Fixed(printed_loyalty),
                cast_cost_condition: None,
            }
        );
    }
}

#[test]
fn compleated_is_a_closed_parameter_free_static() {
    let compleated =
        ron::from_str::<StaticAbilityDef>("Compleated").expect("the unit RON variant should parse");
    let serialized = ron::to_string(&compleated).expect("the unit RON variant should serialize");
    assert_eq!(serialized, "Compleated");
    assert_eq!(
        ron::from_str::<StaticAbilityDef>(&serialized)
            .expect("the unit RON variant should round-trip"),
        compleated
    );
    assert!(
        ron::from_str::<StaticAbilityDef>("Compleated(reduction: 2)").is_err(),
        "Compleated must not accept authored payload parameters"
    );
}

fn compleated_test_ability() -> tricerules_card_model::card_def::IdentifiedStaticAbility {
    ron::from_str::<RawCardDefinition>(
        r#"(
            id: "compleated_ability_template",
            name: "Compleated Ability Template",
            face_id: "compleated_ability_template",
            static_abilities: [(
                ability_id: "compleated",
                presentation: OracleLines([1]),
                definition: Compleated,
            )],
        )"#,
    )
    .expect("the synthetic ability template should parse")
    .static_abilities
    .into_iter()
    .next()
    .expect("the template should contain one identified static ability")
}

fn invalid_card_reason(raw: &RawCardDefinition) -> String {
    let serialized = ron::to_string(raw).expect("the typed fixture should serialize");
    match CardRegistry::from_chunks_and_tokens(&[serialized.as_str()], &[]) {
        Err(tricerules_card_model::registry::RegistryError::InvalidCard { reason, .. }) => reason,
        Err(error) => panic!("expected semantic InvalidCard rejection, got {error:?}"),
        Ok(_) => panic!("registry accepted the invalid context fixture"),
    }
}

#[test]
fn compleated_rejects_class_level_and_instant_sorcery_contexts() {
    let class_ron = r#"(
        id: "compleated_class_context_probe",
        name: "Compleated Class Context Probe",
        face_id: "compleated_class_context_probe",
        mana_cost: "{1}{U}",
        types: ["Enchantment", "Class"],
        class_level_bars: [
            (
                level: 2,
                level_ability: (
                    ability_id: "level_2",
                    presentation: Fallback,
                    costs: [Mana("{2}{U}")],
                    effect: [SetClassLevel(level: 2)],
                    timing: SorcerySpeed,
                ),
            ),
            (
                level: 3,
                level_ability: (
                    ability_id: "level_3",
                    presentation: Fallback,
                    costs: [Mana("{4}{U}")],
                    effect: [SetClassLevel(level: 3)],
                    timing: SorcerySpeed,
                ),
            ),
        ],
    )"#;
    let mut class_raw = ron::from_str::<RawCardDefinition>(class_ron)
        .expect("the valid Class baseline with levels two and three should parse");
    let level_two = &class_raw.class_level_bars[0];
    assert_eq!(level_two.level, 2);
    assert_eq!(
        level_two.level_ability.timing,
        tricerules_card_model::ActivationTiming::SorcerySpeed
    );
    assert!(matches!(
        level_two.level_ability.effect.as_slice(),
        [tricerules_card_model::SpellEffectKind::SetClassLevel { level }] if *level == 2
    ));
    let class_registry = CardRegistry::from_chunks_and_tokens(&[class_ron], &[])
        .expect("a Class with levels two and three without Compleated should be accepted");
    assert!(class_registry
        .get("compleated_class_context_probe")
        .is_some());

    class_raw.class_level_bars[0]
        .static_abilities
        .push(compleated_test_ability());
    assert_eq!(
        invalid_card_reason(&class_raw),
        "Class section static ability must function while the Class level is active on the battlefield"
    );

    let spell_probes = [
        (
            "compleated_instant_context_probe",
            "Compleated Instant Context Probe",
            r#"(
                id: "compleated_instant_context_probe",
                name: "Compleated Instant Context Probe",
                face_id: "compleated_instant_context_probe",
                mana_cost: "{1}{U}",
                types: ["Instant"],
                spell_effect: [Draw(count: 1)],
            )"#,
        ),
        (
            "compleated_sorcery_context_probe",
            "Compleated Sorcery Context Probe",
            r#"(
                id: "compleated_sorcery_context_probe",
                name: "Compleated Sorcery Context Probe",
                face_id: "compleated_sorcery_context_probe",
                mana_cost: "{1}{U}",
                types: ["Sorcery"],
                spell_effect: [Draw(count: 2)],
            )"#,
        ),
    ];
    for (id, _name, spell_ron) in spell_probes {
        let mut spell_raw = ron::from_str::<RawCardDefinition>(spell_ron)
            .expect("the synthetic spell baseline should parse");
        assert_eq!(spell_raw.id, id);
        let baseline = CardRegistry::from_chunks_and_tokens(&[spell_ron], &[])
            .expect("an Instant or Sorcery without static abilities should be accepted");
        assert!(baseline.get(id).is_some());

        spell_raw.static_abilities.push(compleated_test_ability());
        assert_eq!(
            invalid_card_reason(&spell_raw),
            "only stack-active or madness static abilities are valid on instant/sorcery"
        );
    }
}
