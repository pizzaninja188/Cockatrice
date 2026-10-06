use tricerules_cards::primitives::{CastTriggerPlayer, Color, Evasion, TriggerCondition};
use tricerules_cards::{AbilityPresentation, CardRegistry, TokenDefinition};

#[test]
fn chasm_registers_exact_card_and_both_clauses() {
    let card = CardRegistry::global()
        .get("chasm_skulker")
        .expect("complete Chasm Skulker");
    let face = card.primary_face();
    assert_eq!(card.name, "Chasm Skulker");
    assert_eq!(card.faces_iter().count(), 1);
    assert_eq!(face.mana_cost.to_string(), "{2}{U}");
    assert_eq!(face.types, ["Creature", "Squid", "Horror"]);
    assert_eq!((face.power, face.toughness), (Some(1), Some(1)));
    assert_eq!(face.colors(), vec![Color::Blue]);
    assert!(face.keywords.is_empty() && face.evasions.is_empty());
    assert!(face.static_abilities.is_empty() && face.activated_abilities.is_empty());
    assert_eq!(face.triggered_abilities.len(), 2);
    assert_eq!(
        face.triggered_abilities[0].trigger,
        TriggerCondition::WheneverPlayerDrawsCard {
            drawer: CastTriggerPlayer::Controller
        }
    );
    assert_eq!(
        face.triggered_abilities[1].trigger,
        TriggerCondition::WhenSelfDies
    );
    for (index, trigger) in face.triggered_abilities.iter().enumerate() {
        assert_eq!(
            trigger.presentation,
            AbilityPresentation::OracleLines(vec![(index + 1) as u16])
        );
        assert!(trigger.targeting.is_none());
    }
    let draw: tricerules_cards::SpellEffectKind =
        ron::from_str("PutCounters(counter: PlusOnePlusOne, count: 1, subject: Source)").unwrap();
    let death: tricerules_cards::SpellEffectKind = ron::from_str("CreateTokens(token: \"squid_u_1_1_islandwalk\", count: Count(SourceCounterCount(counter: PlusOnePlusOne)))").unwrap();
    assert_eq!(face.triggered_abilities[0].effect, [draw]);
    assert_eq!(face.triggered_abilities[1].effect, [death]);
    let squid = CardRegistry::global()
        .get("squid_u_1_1_islandwalk")
        .unwrap()
        .primary_face();
    assert_eq!(squid.name, "Squid");
    assert_eq!(squid.types, ["Creature", "Squid"]);
    assert_eq!((squid.power, squid.toughness), (Some(1), Some(1)));
    assert_eq!(squid.colors(), [Color::Blue]);
    assert_eq!(
        squid.evasions,
        [Evasion::Landwalk {
            land_subtype: "Island".into()
        }]
    );
}

#[test]
fn chasm_token_evasion_roundtrips_and_projects_without_changing_old_tokens() {
    let old = r#"(id: "squid_u_1_1", name: "Squid", face_id: "squid", types: ["Creature", "Squid"], colors: [Blue], power: Some(1), toughness: Some(1))"#;
    let token: TokenDefinition = ron::from_str(old).unwrap();
    assert!(token.to_card_def().primary_face().evasions.is_empty());
    let explicit = old.replacen(
        "toughness: Some(1)",
        "toughness: Some(1), evasions: [Landwalk(land_subtype: \"Island\")]",
        1,
    );
    let token: TokenDefinition = ron::from_str(&explicit).unwrap();
    let expected = vec![Evasion::Landwalk {
        land_subtype: "Island".into(),
    }];
    assert_eq!(token.to_card_def().primary_face().evasions, expected);
    let decoded: TokenDefinition = ron::from_str(&ron::to_string(&token).unwrap()).unwrap();
    assert_eq!(decoded.to_card_def().primary_face().evasions, expected);
    let registry = CardRegistry::from_chunks_and_tokens(&[], &[&explicit]).unwrap();
    assert_eq!(
        registry.get("squid_u_1_1").unwrap().primary_face().evasions,
        expected
    );
}

#[test]
fn chasm_counter_quantity_rejects_missing_and_nonbattlefield_source_contexts() {
    for amount in [
        "Count(SourceCounterCount(counter: PlusOnePlusOne))",
        "Count(Affine(terms: [(coefficient: 2, quantity: SourceCounterCount(counter: PlusOnePlusOne))]))",
        "DivideRoundedDown(amount: Count(SourceCounterCount(counter: PlusOnePlusOne)), divisor: 2)",
    ] {
        let effect: tricerules_cards::SpellEffectKind = ron::from_str(&format!("GainLife(amount: {amount})")).expect("counter quantity parses");
        assert_eq!(ron::from_str::<tricerules_cards::SpellEffectKind>(&ron::to_string(&effect).unwrap()).unwrap(), effect);
        assert!(effect.validate(tricerules_cards::EffectContext::Ability).is_ok());
        assert!(effect.validate(tricerules_cards::EffectContext::Spell).is_err());
        for zone in ["Battlefield", "Hand", "Graveyard"] {
            for effect in [format!("GainLife(amount: {amount})"), format!("Conditional(condition: ActivePlayer(players: Controller), effect: Draw(count: {amount}))")] {
                let card = format!(r#"(id: "counter_source", name: "Counter Source", face_id: "counter_source", types: ["Artifact"], activated_abilities: [(ability_id: "activated_01", presentation: Fallback, source_zone: {zone}, costs: [Mana("{{1}}")], effect: [{effect}])])"#);
                assert_eq!(CardRegistry::from_chunks_and_tokens(&[&card], &[]).is_ok(), zone == "Battlefield", "{zone}: {effect}");
            }
        }
        for body in [
            format!("spell_effect: [GainLife(amount: {amount})]"),
            format!("cost_modifiers: [GenericReduction(amount: {amount})]"),
            format!("static_abilities: [(ability_id: \"static_01\", presentation: Fallback, definition: EntersWithCounters(counter: PlusOnePlusOne, amount: {amount}))]"),
            format!("activated_abilities: [(ability_id: \"activated_01\", presentation: Fallback, costs: [Mana(\"{{1}}\")], cost_modifiers: [GenericReduction(amount: {amount})], effect: [GainLife(amount: 1)])]"),
        ] {
            let card = format!(r#"(id: "counter_source", name: "Counter Source", face_id: "counter_source", types: ["Artifact"], {body})"#);
            assert!(CardRegistry::from_chunks_and_tokens(&[&card], &[]).is_err(), "{body}");
        }
    }
    let static_card = r#"(id: "counter_source", name: "Counter Source", face_id: "counter_source", types: ["Creature"], power: 1, toughness: 1, static_abilities: [(ability_id: "static_01", presentation: Fallback, definition: CountScaledSelfPt(count: SourceCounterCount(counter: PlusOnePlusOne), power_per_match: 1, toughness_per_match: 1))])"#;
    assert!(CardRegistry::from_chunks_and_tokens(&[static_card], &[]).is_err());
    let invalid_counter: tricerules_cards::SpellEffectKind =
        ron::from_str("GainLife(amount: Count(SourceCounterCount(counter: Keyword(Wither))))")
            .unwrap();
    assert!(invalid_counter
        .validate(tricerules_cards::EffectContext::Ability)
        .is_err());
}

#[test]
fn chasm_counter_quantity_cannot_escape_through_mass_counters_or_delayed_spell() {
    let amount = "Count(SourceCounterCount(counter: PlusOnePlusOne))";
    for text in [
        format!("PutCountersAll(counter: PlusOnePlusOne, count: {amount})"),
        format!(
            r#"CreateDelayedTrigger(subject: Some(Chosen((kind: Creature))), ability: (ability_id: "triggered_01", presentation: Fallback, trigger: WhenWatchedObjectDiesThisTurn, effect: [GainLife(amount: {amount})]))"#
        ),
        format!(
            r#"CreateReflexiveTrigger(ability: (ability_id: "triggered_01", presentation: Fallback, effect: [GainLife(amount: {amount})]))"#
        ),
        format!(
            r#"CreateDelayedTrigger(subject: Some(Chosen((kind: Creature))), ability: (ability_id: "triggered_01", presentation: Fallback, trigger: AtBeginningOfNextEndStep, modal: Some((min_modes: 1, max_modes: 1, modes: [(mode_id: "gain", presentation: Fallback, effects: [GainLife(amount: {amount})])]))))"#
        ),
    ] {
        let effect: tricerules_cards::SpellEffectKind = ron::from_str(&text).unwrap();
        assert!(
            effect
                .validate(tricerules_cards::EffectContext::Ability)
                .is_ok(),
            "{text}"
        );
        assert!(
            effect
                .validate(tricerules_cards::EffectContext::Spell)
                .is_err(),
            "{text}"
        );
        for zone in ["Hand", "Graveyard"] {
            let card = format!(
                r#"(id: "counter_source", name: "Counter Source", face_id: "counter_source", types: ["Artifact"], activated_abilities: [(ability_id: "activated_01", presentation: Fallback, source_zone: {zone}, costs: [Mana("{{1}}")], effect: [{text}])])"#
            );
            assert!(
                CardRegistry::from_chunks_and_tokens(&[&card], &[]).is_err(),
                "{zone}: {text}"
            );
        }
    }
}

#[test]
fn chasm_counter_quantity_granted_abilities_bind_to_battlefield_recipient() {
    let amount = "Count(SourceCounterCount(counter: PlusOnePlusOne))";
    for text in [
        format!(
            r#"GrantTriggeredAbility(subject: Chosen((kind: Creature)), ability: (ability_id: "triggered_01", presentation: Fallback, trigger: WhenSelfDies, effect: [GainLife(amount: {amount})]))"#
        ),
        format!(
            r#"ApplyPermanentModifier(subject: Chosen((kind: Creature)), modifier: GrantActivatedAbility((ability_id: "activated_01", presentation: Fallback, costs: [Mana("{{1}}")], effect: [GainLife(amount: {amount})])), duration: UntilEndOfTurn)"#
        ),
    ] {
        let effect: tricerules_cards::SpellEffectKind = ron::from_str(&text).unwrap();
        assert!(
            effect
                .validate(tricerules_cards::EffectContext::Spell)
                .is_ok(),
            "{text}"
        );
        for zone in ["Hand", "Graveyard"] {
            let card = format!(
                r#"(id: "counter_source", name: "Counter Source", face_id: "counter_source", types: ["Artifact"], activated_abilities: [(ability_id: "outer", presentation: Fallback, source_zone: {zone}, costs: [Mana("{{1}}")], effect: [{text}])])"#
            );
            assert!(
                CardRegistry::from_chunks_and_tokens(&[&card], &[]).is_ok(),
                "{zone}: {text}"
            );
        }
    }
}
