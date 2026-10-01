use tricerules_cards::primitives::{Amount, EffectContext, SpellEffectKind, TargetKind};
use tricerules_cards::{CardRegistry, Layout};

fn probe(fields: &str) -> Result<CardRegistry, String> {
    let data = format!(
        r#"(id:"blue_probe",name:"Blue Probe",face_id:"blue_probe",types:["Creature"],power:1,toughness:1,{fields})"#
    );
    CardRegistry::from_chunks_and_tokens(&[&data], &[]).map_err(|error| error.to_string())
}

fn draw(amount: &str) -> String {
    format!("TargetPlayerDraws(count:{amount},target:(kind:AnyPlayer))")
}

#[test]
fn blue_complete_definition_and_presentation() {
    let registry = CardRegistry::global();
    let card = registry.get("blue_suns_zenith").unwrap();
    assert_eq!(card.name, "Blue Sun's Zenith");
    assert_eq!(
        registry.id_for_name("Blue Sun's Zenith"),
        Some("blue_suns_zenith")
    );
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "blue_suns_zenith");
    assert_eq!(face.mana_cost.to_string(), "{X}{U}{U}{U}");
    assert_eq!(face.types, ["Instant"]);
    assert_eq!((face.power, face.toughness), (None, None));
    assert!(face.supertypes.is_empty() && face.keywords.is_empty());
    assert!(
        face.activated_abilities.is_empty()
            && face.static_abilities.is_empty()
            && face.triggered_abilities.is_empty()
    );
    assert!(face.modal_spell.is_none() && face.custom_effect.is_none());
    assert!(
        matches!(face.spell_effect.as_slice(), [SpellEffectKind::TargetPlayerDraws {count: Amount::X, target}, SpellEffectKind::ShuffleResolvingSpellIntoOwnersLibrary] if target.kind == TargetKind::AnyPlayer)
    );
    let groups = &face.targeting.as_ref().unwrap().groups;
    assert_eq!(groups.len(), 1);
    assert_eq!((groups[0].min, groups[0].max), (1, 1));
    assert_eq!(groups[0].effect_indices, [0]);
    assert_eq!(groups[0].prompt, "Choose target player");
}

#[test]
fn targeted_draw_accepts_chosen_x_and_preserves_fixed_ron() {
    let fixed = "TargetPlayerDraws(count:3,target:(kind:AnyPlayer))";
    let effect: SpellEffectKind = ron::from_str(fixed).unwrap();
    assert!(ron::to_string(&effect).unwrap().contains("count:3"));
    let dynamic: SpellEffectKind =
        ron::from_str("TargetPlayerDraws(count:\"X\",target:(kind:AnyPlayer))")
            .expect("targeted draw must accept the captured chosen X");
    assert!(dynamic.validate(EffectContext::Spell).is_ok());
    assert!(ron::to_string(&dynamic).unwrap().contains("count:\"X\""));
}

#[test]
fn unsigned_amount_refuses_literal_overflow() {
    assert!(ron::from_str::<Amount>("4294967296").is_err());
    assert!(ron::from_str::<SpellEffectKind>(
        "TargetPlayerDraws(count:4294967296,target:(kind:AnyPlayer))"
    )
    .is_err());
}

#[test]
fn source_shuffle_is_spell_only_including_wrapped_effects() {
    let effect: SpellEffectKind = ron::from_str("ShuffleResolvingSpellIntoOwnersLibrary")
        .expect("resolving-spell self-shuffle is an authored instruction");
    assert!(effect.validate(EffectContext::Spell).is_ok());
    assert!(effect.validate(EffectContext::Ability).is_err());
    for wrapped in [
        "ShuffleResolvingSpellIntoOwnersLibrary".to_string(),
        "MayBehold(choice_id:\"behold\",hand_filter:(required_subtypes:[\"Elf\"]),permanent_filter:(kind:Creature,controller:You),if_beheld:[ShuffleResolvingSpellIntoOwnersLibrary])".to_string(),
        "ChooseResolutionBranch(selection:FirstApplicable,branches:[(branch_id:\"branch\",presentation:Fallback,cost:None,effects:[ShuffleResolvingSpellIntoOwnersLibrary])])".to_string(),
    ] {
        assert!(probe(&format!("spell_effect:[{wrapped}]")).is_ok(), "valid spell wrapper: {wrapped}: {:?}", probe(&format!("spell_effect:[{wrapped}]")).err());
        for fields in [
            format!("activated_abilities:[(ability_id:\"activated\",presentation:Fallback,costs:[],effect:[{wrapped}])]"),
            format!("triggered_abilities:[(ability_id:\"triggered\",presentation:Fallback,trigger:WhenSelfEntersBattlefield,effect:[{wrapped}])]"),
            format!("spell_effect:[GrantTriggeredAbility(subject:Chosen((kind:Creature)),ability:(ability_id:\"child\",presentation:Fallback,trigger:WhenSelfDies,effect:[{wrapped}]))]"),
        ] {
            assert!(probe(&fields).unwrap_err().contains("spell"), "{fields}");
        }
    }
    let mode = "(min_modes:1,max_modes:1,modes:[(mode_id:\"mode\",presentation:Fallback,effects:[ShuffleResolvingSpellIntoOwnersLibrary])])";
    assert!(probe(&format!("modal_spell:{mode}")).is_ok());
    assert!(probe(&format!("triggered_abilities:[(ability_id:\"triggered\",presentation:Fallback,trigger:WhenSelfDies,modal:Some({mode}))]")).unwrap_err().contains("spell"));
}

#[test]
fn targeted_amount_rejects_missing_source_cast_and_prior_result_contexts() {
    for amount in [
        "Count(SourcePower)",
        "EventCount",
        "Conditional(condition:TriggeringSpellManaSpent(comparison:AtLeast(1)),when_true:2,otherwise:1)",
    ] {
        let effect: SpellEffectKind = ron::from_str(&draw(amount)).unwrap();
        assert!(effect.validate(EffectContext::Spell).is_err(), "{amount}");
    }
    for source in ["PreviousEffect", "Payment"] {
        let amount = format!("Count(CardsMatchingResult(filter:(source:{source},action:Discard,players:Controller)))");
        let fields = format!("spell_effect:[{}]", draw(&amount));
        assert!(probe(&fields).unwrap_err().contains(source), "{fields}");
    }
    let snapshot = draw("Conditional(condition:CastSnapshot(index:1),when_true:2,otherwise:1)");
    assert!(probe(&format!(
        "cast_conditions:[ActivePlayer(players:Controller)],spell_effect:[{snapshot}]"
    ))
    .is_err());
    let cost = draw("Conditional(cast_cost:(group_id:\"missing\",option_id:\"missing\",expected_selected:true),when_true:2,otherwise:0)");
    assert!(probe(&format!("spell_effect:[{cost}]"))
        .unwrap_err()
        .contains("unknown group"));
    let effect: SpellEffectKind = ron::from_str(&cost).unwrap();
    assert!(effect.validate(EffectContext::Ability).is_err());
}

#[test]
fn targeted_amount_requires_the_trigger_that_supplies_its_context() {
    for (amount, required_trigger) in [
        ("EventCount", "WheneverPlayerDiscardsOneOrMoreCards(player:Controller)"),
        ("Conditional(condition:TriggeringSpellManaSpent(comparison:AtLeast(1)),when_true:2,otherwise:1)", "WheneverPlayerCastsSpell(caster:AnyPlayer)"),
    ] {
        for trigger in [required_trigger, "WhenSelfEntersBattlefield"] {
            let fields = format!("triggered_abilities:[(ability_id:\"triggered\",presentation:Fallback,trigger:{trigger},effect:[{}])]", draw(amount));
            let result = probe(&fields);
            assert_eq!(result.is_ok(), trigger == required_trigger, "{fields}: {:?}", result.err());
        }
    }
}
