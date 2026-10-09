use tricerules_cards::primitives::{
    Amount, CastOrdinalScope, CastTriggerPlayer, EffectContext, PlayerRecipient, SpellCastFilter,
    SpellEffectKind, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Layout};

fn probe(body: &str) -> Result<CardRegistry, String> {
    let data = format!(
        r#"(id:"zenith_probe", name:"Zenith Probe", face_id:"zenith_probe",
        types:["Creature"],power:3,toughness:1,{body})"#
    );
    CardRegistry::from_chunks_and_tokens(&[&data], &[]).map_err(|error| error.to_string())
}

#[test]
fn zenith_complete_definition_and_presentation() {
    let registry = tricerules_cards::registry::global();
    let card = registry.get("zenith_chronicler").unwrap();
    assert_eq!(card.name, "Zenith Chronicler");
    assert_eq!(
        registry.id_for_name("Zenith Chronicler"),
        Some("zenith_chronicler")
    );
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "zenith_chronicler");
    assert_eq!(face.mana_cost.to_string(), "{2}");
    assert_eq!(
        face.types,
        ["Artifact", "Creature", "Phyrexian", "Construct"]
    );
    assert_eq!((face.power, face.toughness), (Some(3), Some(1)));
    assert!(face.supertypes.is_empty() && face.colors().is_empty() && face.keywords.is_empty());
    assert!(face.activated_abilities.is_empty() && face.static_abilities.is_empty());
    assert!(face.spell_effect.is_empty() && face.targeting.is_none() && face.modal_spell.is_none());
    assert!(face.custom_effect.is_none());
    assert_eq!(face.triggered_abilities.len(), 1);
    let ability = &face.triggered_abilities[0];
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.trigger,
        TriggerCondition::WheneverPlayerCastsSpell {
            caster: CastTriggerPlayer::AnyPlayer,
            filter: SpellCastFilter {
                is_multicolored: true,
                ..Default::default()
            },
            ordinal: Some(1),
            ordinal_scope: CastOrdinalScope::MatchingFilter,
        }
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::Draw {
            count: Amount::Fixed(1),
            who: PlayerRecipient::EachOtherPlayerThanAffectedPlayer,
        }]
    );
    assert!(
        ability.targeting.is_none() && ability.modal.is_none() && ability.intervening_if.is_none()
    );
    assert!(ability.max_triggers_per_turn.is_none());
}

#[test]
fn multicolor_filter_defaults_and_roundtrips_without_changing_old_data() {
    let old: SpellCastFilter = ron::from_str("(is_color:Some(Red))").unwrap();
    assert!(!old.is_multicolored);
    assert!(!ron::to_string(&old).unwrap().contains("is_multicolored"));
    let multi: SpellCastFilter = ron::from_str("(is_multicolored:true)").unwrap();
    assert!(multi.is_multicolored);
    assert_eq!(
        ron::from_str::<SpellCastFilter>(&ron::to_string(&multi).unwrap()).unwrap(),
        multi
    );
    assert!(ron::from_str::<SpellCastFilter>("(is_multicolour:true)").is_err());
}

#[test]
fn caster_complement_refuses_every_non_draw_recipient_slot() {
    let recipient = "EachOtherPlayerThanAffectedPlayer";
    for data in [
        format!("DamagePlayer(amount:1,who:{recipient})"),
        format!("Discard(quantity:Exact(1),who:{recipient})"),
        format!("DrawDiscard(draw_count:1,discard_count:1,order:DrawThenDiscard,who:{recipient})"),
        format!("ChooseResolutionBranch(branches:[],chooser:{recipient})"),
        format!("ChoosePermanents(filter:(kind:Creature),min:0,max:1,chooser:{recipient})"),
        format!("LoseLife(amount:Fixed(1),who:{recipient})"),
        format!("ExileTopWithPlayPermission(player:{recipient})"),
        format!("Mill(count:1,who:{recipient})"),
        format!("CreateTokens(token:\"soldier\",count:1,who:{recipient})"),
        format!("MayBehold(choice_id:\"behold\",hand_filter:(),permanent_filter:(kind:Creature),who:{recipient})"),
        format!("SearchLibrary(who:{recipient})"),
    ] {
        let effect:SpellEffectKind=ron::from_str(&data).unwrap_or_else(|error|panic!("{data}: {error}"));
        for context in [EffectContext::Spell,EffectContext::Ability] {
            assert!(effect.validate(context).unwrap_err().contains("supported only by Draw"),"{data}");
        }
    }
}

#[test]
fn caster_complement_requires_its_own_cast_trigger_context() {
    let draw = "Draw(count:1,who:EachOtherPlayerThanAffectedPlayer)";
    let cast = "WheneverPlayerCastsSpell(caster:AnyPlayer)";
    for trigger in [cast, "WhenSelfEntersBattlefield", "WhenSelfDies"] {
        let result=probe(&format!("triggered_abilities:[(ability_id:\"triggered_01\",presentation:Fallback,trigger:{trigger},effect:[{draw}])]"));
        assert_eq!(result.is_ok(), trigger == cast, "{trigger}");
    }
    for body in [
        format!("spell_effect:[{draw}]"),
        format!("activated_abilities:[(ability_id:\"activated_01\",presentation:Fallback,costs:[],effect:[{draw}])]"),
    ] { assert!(probe(&body).is_err(),"{body}"); }
    for child_trigger in [cast, "WhenSelfDies"] {
        let child = format!(
            "(ability_id:\"child\",presentation:Fallback,trigger:{child_trigger},effect:[{draw}])"
        );
        let grant = format!("GrantTriggeredAbility(subject:Source,ability:{child})");
        // A granted ability owns its future event; neither inherits nor requires its parent's cast.
        for outer_trigger in [cast, "WhenSelfEntersBattlefield"] {
            let result=probe(&format!("triggered_abilities:[(ability_id:\"outer\",presentation:Fallback,trigger:{outer_trigger},effect:[{grant}])]"));
            assert_eq!(
                result.is_ok(),
                child_trigger == cast,
                "outer {outer_trigger}, child {child_trigger}"
            );
        }
    }
}

#[test]
fn caster_complement_context_validation_descends_wrappers_and_modal_modes() {
    let draw = "Draw(count:1,who:EachOtherPlayerThanAffectedPlayer)";
    for effect in [
        format!("Conditional(condition:ActivePlayer(players:All),effect:{draw})"),
        format!("ChooseResolutionBranch(selection:FirstApplicable,branches:[(branch_id:\"branch\",presentation:Fallback,cost:None,effects:[{draw}])])"),
    ] {
        for trigger in ["WheneverPlayerCastsSpell(caster:AnyPlayer)","WhenSelfDies"] {
            let result=probe(&format!("triggered_abilities:[(ability_id:\"trigger\",presentation:Fallback,trigger:{trigger},effect:[{effect}])]"));
            assert_eq!(result.is_ok(),trigger.starts_with("WheneverPlayerCastsSpell"),"{trigger}: {effect}");
        }
    }
    for trigger in ["WheneverPlayerCastsSpell(caster:AnyPlayer)", "WhenSelfDies"] {
        let body=format!("triggered_abilities:[(ability_id:\"trigger\",presentation:Fallback,trigger:{trigger},modal:Some((min_modes:1,max_modes:1,modes:[(mode_id:\"mode\",presentation:Fallback,effects:[{draw}])])))]");
        let result = probe(&body);
        assert_eq!(
            result.is_ok(),
            trigger.starts_with("WheneverPlayerCastsSpell"),
            "{body}"
        );
    }
    for effect in [
        format!("CreateDelayedTrigger(subject:Some(Source),ability:(ability_id:\"delayed\",presentation:Fallback,trigger:AtBeginningOfNextEndStep,effect:[{draw}]))"),
        format!("CreateReflexiveTrigger(ability:(ability_id:\"reflexive\",presentation:Fallback,effect:[{draw}]))"),
    ] {
        let body=format!("triggered_abilities:[(ability_id:\"trigger\",presentation:Fallback,trigger:WheneverPlayerCastsSpell(caster:AnyPlayer),effect:[{effect}])] ");
        let error = match probe(&body) {
            Err(error) => error,
            Ok(_) => panic!("future nested triggers cannot borrow their parent's cast context"),
        };
        assert!(error.contains("triggering-spell context"),"{error}");
    }
    let wrapped:SpellEffectKind=ron::from_str(&format!("ConditionalCastCost(condition:(group_id:\"bargain\",option_id:\"bargain\",expected_selected:true),effect:{draw})")).unwrap();
    assert!(wrapped
        .validate(EffectContext::Spell)
        .unwrap_err()
        .contains("triggering-spell context"));
    assert!(
        wrapped.validate(EffectContext::Ability).is_err(),
        "Draw remains unsupported under ConditionalCastCost"
    );
}

#[test]
fn caster_complement_context_validation_descends_may_behold() {
    let effect = r#"MayBehold(choice_id:"behold",hand_filter:(required_subtypes:["Elf"]),
        permanent_filter:(kind:Creature,controller:You),
        if_beheld:[Draw(count:1,who:EachOtherPlayerThanAffectedPlayer)])"#;
    let cast = "WheneverPlayerCastsSpell(caster:AnyPlayer)";
    for trigger in [cast, "WhenSelfEntersBattlefield", "WhenSelfDies"] {
        let body = format!(
            "triggered_abilities:[(ability_id:\"trigger\",presentation:Fallback,trigger:{trigger},effect:[{effect}])]"
        );
        let result = probe(&body);
        assert_eq!(
            result.is_ok(),
            trigger == cast,
            "{body}: {}",
            result.as_ref().err().map_or("accepted", String::as_str)
        );
    }
    for body in [
        format!("activated_abilities:[(ability_id:\"activated\",presentation:Fallback,costs:[],effect:[{effect}])]"),
        format!("triggered_abilities:[(ability_id:\"outer\",presentation:Fallback,trigger:{cast},effect:[CreateReflexiveTrigger(ability:(ability_id:\"reflexive\",presentation:Fallback,effect:[{effect}]))])]"),
    ] {
        let error = match probe(&body) {
            Err(error) => error,
            Ok(_) => panic!("non-cast abilities must reject the wrapped caster recipient: {body}"),
        };
        assert!(error.contains("triggering-spell context"), "{error}");
    }
}
