//! Generic data-driven vocabulary used by card definitions.
//!
//! The submodules group effects, targeting, costs, abilities, and keywords while the
//! re-exports below preserve the established `primitives::X` API and RON serde shapes.

mod abilities;
mod costs;
mod effects;
mod keywords;
mod targeting;

pub use abilities::*;
pub use costs::*;
pub use effects::*;
pub use keywords::*;
pub use targeting::*;

#[cfg(test)]
mod tests {
    #[test]
    fn earthbend_nonland_permanent_discard_filter_is_shared() {
        let filter: super::CardTypeFilter = ron::from_str("NonlandPermanent")
            .expect("Dai Li Indoctrination and Auntie's Sentence need this filter");
        let registry = crate::CardRegistry::global();
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
    fn issue_176_graveyard_cost_reduction_is_a_typed_target_filter() {
        let card = r#"(id: "test", name: "Test", mana_cost: "{4}{B}", types: ["Sorcery"], cost_modifiers: [TargetMatchGenericReduction(amount: 3, filter: Graveyard((card_type: Some(Creature), max_mana_value: Some(3))))], spell_effect: [MoveGraveyardCards(filter: (card_type: Some(Creature)), destination: Battlefield())])"#;
        assert!(crate::CardRegistry::from_chunks_and_tokens(&[card], &[]).is_ok());
    }
    #[test]
    fn issue_153_cast_cost_amount_roundtrips_and_checks_references() {
        let source =
            "CastCost(cast_cost: (group_index: 0, option_index: 0), when_true: 4, otherwise: 2)";
        let amount: super::Amount = ron::from_str(source).unwrap();
        let encoded = ron::to_string(&amount).unwrap();
        assert_eq!(ron::from_str::<super::Amount>(&encoded).unwrap(), amount);
        let invalid = format!(
            r#"(id: "test", name: "Test", types: ["Instant"], spell_effect: [DamageAll(amount: {source})])"#
        );
        assert!(crate::CardRegistry::from_chunks_and_tokens(&[&invalid], &[]).is_err());
        let ability = format!(
            r#"(id: "test", name: "Test", types: ["Creature"], power: 1, toughness: 1, activated_abilities: [(text: "Draw", costs: [Tap], effect: [Draw(count: {source})])])"#
        );
        assert!(
            crate::CardRegistry::from_chunks_and_tokens(&[&ability], &[]).is_err(),
            "cast cost receipts are not an ability context"
        );
        let zero = r#"(id: "test", name: "Test", types: ["Creature"], power: 1, toughness: 1, activated_abilities: [(text: "Blight", costs: [Blight(count: 0)], effect: [Draw(count: 1)])])"#;
        assert!(crate::CardRegistry::from_chunks_and_tokens(&[zero], &[]).is_err());
    }
    #[test]
    fn issue_166_cast_quantity_and_filter_validate() {
        let amount: super::Amount = ron::from_str(
            "Count(SpellsCastThisTurn(players: AffectedPlayer, filter: (any_of: Some([(card_type: Some(Noncreature)), (required_subtypes: [\"Otter\"])]))))"
        ).expect("cast history is a shared quantity");
        let effects = [super::SpellEffectKind::DamagePlayer {
            amount,
            who: super::PlayerRecipient::AffectedPlayer,
        }];
        assert!(super::SpellEffectKind::validate_list(&effects).is_ok());
    }

    #[test]
    fn issue_166_invalid_cast_filters_are_rejected() {
        for filter in [
            "(min_mana_value: Some(4), max_mana_value: Some(2))",
            "(required_subtypes: [\"\"])",
            "(any_of: Some([]))",
            "(any_of: Some([()]))",
        ] {
            let trigger: super::TriggerCondition =
                ron::from_str(&format!("WheneverPlayerCastsSpell(filter: {filter})")).unwrap();
            assert!(trigger.validate().is_err(), "{filter}");
        }
    }

    #[test]
    fn issue_165_dynamic_consumers_reject_unavailable_result_contexts() {
        for template in [
            "Scry(count: AMOUNT)",
            "DamageAll(amount: AMOUNT)",
            "CounterTargetSpell(unless_controller_pays: Some(AMOUNT))",
        ] {
            let previous = template.replace("AMOUNT", "Count(CardsMatchingResult(filter: (source: PreviousEffect, action: Discard, players: Controller)))");
            let effect: super::SpellEffectKind = ron::from_str(&previous).unwrap();
            assert!(
                super::SpellEffectKind::validate_list(&[effect]).is_err(),
                "missing previous effect: {template}"
            );
            let payment = template.replace("AMOUNT", "Count(CardsMatchingResult(filter: (source: Payment, action: Discard, players: Controller)))");
            let card = format!(
                r#"(id: "test", name: "Test", types: ["Instant"], spell_effect: [{payment}])"#
            );
            assert!(
                crate::CardRegistry::from_chunks_and_tokens(&[&card], &[]).is_err(),
                "missing payment: {template}"
            );
        }
    }

    #[test]
    fn issue_165_quantities_reject_unsupported_shapes_and_source_contexts() {
        for source in [
            "Affine(terms: [])",
            "Affine(terms: [(coefficient: 0, quantity: SourcePower)])",
            "Affine(terms: [(coefficient: 1, quantity: Affine(terms: [(coefficient: 1, quantity: SourcePower)]))])",
            "Affine(terms: [(coefficient: 1, quantity: CardsMatchingResult(filter: (source: Payment, action: Discard, players: Controller)))])",
        ] {
            let expression: super::CountExpression = ron::from_str(source).unwrap();
            assert!(expression.validate().is_err(), "{source}");
        }
        let amount = super::Amount::Count(super::CountExpression::SourcePower);
        assert!(super::SpellEffectKind::GainLife {
            amount: amount.clone()
        }
        .validate(super::EffectContext::Spell)
        .is_err());
        assert!(super::SpellCostModifier::GenericReduction { amount }
            .validate()
            .is_err());
        for count in [
            "SourcePower",
            "BattlefieldMaximum(filter: (controllers: Controller), characteristic: Power)",
        ] {
            let expression: super::CountExpression = ron::from_str(count).unwrap();
            assert!(expression.validate_static_count().is_err());
        }
        let card = r#"(id: "test", name: "Test", types: ["Creature"], power: 1, toughness: 2,
            static_abilities: [EntersWithCounters(counter: PlusOnePlusOne, amount: Count(SourcePower))])"#;
        assert!(
            crate::CardRegistry::from_chunks_and_tokens(&[card], &[]).is_err(),
            "entry replacement has no battlefield source power"
        );
    }

    #[test]
    fn issue_165_dynamic_consumers_accept_quantities() {
        for effect in [
            "Scry(count: Count(SourcePower))",
            "DamageAll(amount: Count(GraveyardCards(owners: Controller)))",
            "CounterTargetSpell(unless_controller_pays: Some(Count(BattlefieldMaximum(filter: (controllers: Controller, card_type: Some(Creature)), characteristic: Power))))",
        ] {
            let effect: super::SpellEffectKind = ron::from_str(effect).expect("dynamic consumer");
            effect.validate(super::EffectContext::Ability).expect("valid dynamic consumer");
        }
    }
    #[test]
    fn issue_165_static_quantity_accepts_permanents_without_pt_recursion() {
        let card = r#"(id: "test", name: "Test", types: ["Creature"], power: 1, toughness: 2,
            static_abilities: [CountScaledSelfPt(count: BattlefieldPermanents(filter: (controllers: Controller, required_subtypes: ["Desert"])),
                power_per_match: 1, toughness_per_match: 1)])"#;
        assert!(crate::CardRegistry::from_chunks_and_tokens(&[card], &[]).is_ok());
    }
    #[test]
    fn issue_165_public_quantities_roundtrip() {
        for source in [
            "Count(BattlefieldPermanents(filter: (controllers: Controller, required_subtypes: [\"Island\"])))",
            "Count(GraveyardCards(owners: Controller, filter: Some((subtype: Some(\"Cave\")))))",
            "Count(BattlefieldMaximum(filter: (controllers: Controller, card_type: Some(Creature)), characteristic: Toughness))",
            "Count(SourcePower)",
            "Count(DeclaredAttackers(players: All))",
            "Count(Affine(constant: 2, terms: [(coefficient: 2, quantity: SourcePower)]))",
        ] {
            let amount: super::Amount = ron::from_str(source)
                .unwrap_or_else(|error| panic!("quantity must deserialize: {source}: {error}"));
            amount.validate().expect("valid public quantity");
            let encoded = ron::to_string(&amount).unwrap();
            assert_eq!(ron::from_str::<super::Amount>(&encoded).unwrap(), amount);
        }
    }

    #[test]
    fn issue_169_tap_trigger_cardinalities_are_typed() {
        for cardinality in ["EachObject", "OneOrMorePerAction"] {
            let definition = format!(
                "WheneverPlayerTapsCreature(player: Controller, controllers: Opponents, cardinality: {cardinality})"
            );
            assert!(
                ron::from_str::<super::TriggerCondition>(&definition).is_ok(),
                "actor-aware tap trigger must deserialize: {definition}"
            );
        }
    }

    #[test]
    fn issue_169_grouped_taps_cannot_pretend_to_supply_one_observed_object() {
        for (cardinality, valid) in [("EachObject", true), ("OneOrMorePerAction", false)] {
            let definition = format!(
                r#"(
                trigger: WheneverPlayerTapsCreature(player: Controller, controllers: Opponents, cardinality: {cardinality}),
                effect: [PutCounters(counter: Stun, count: 1, subject: TriggerObject)],
                text: "Observed creature counter",
            )"#
            );
            let ability: super::TriggeredAbilityDef = ron::from_str(&definition).unwrap();
            assert_eq!(ability.validate_shape().is_ok(), valid);
        }
    }

    use super::*;

    #[test]
    fn issue_172_expend_threshold_is_positive_and_defaults_to_controller() {
        for amount in [0, 1, 4, u32::MAX] {
            let trigger: TriggerCondition =
                ron::from_str(&format!("WheneverPlayerExpendsMana(amount: {amount})")).unwrap();
            assert_eq!(
                trigger,
                TriggerCondition::WheneverPlayerExpendsMana {
                    player: CastTriggerPlayer::Controller,
                    amount,
                }
            );
            assert_eq!(trigger.validate().is_ok(), amount != 0);
        }
    }

    #[test]
    fn special_action_mana_restrictions_validate_without_spell_or_ability_permissions() {
        for purpose in ["UnlockRoomDoor", "TurnFaceUp"] {
            let restriction: ManaSpendingRestriction = ron::from_str(&format!(
                "(label: \"Special action only\", special_actions: [{purpose}])"
            ))
            .expect("typed special-action restriction");
            assert!(restriction.validate().is_ok(), "{purpose}");
            assert!(restriction.cast_spell.is_empty());
            assert!(restriction.activate_ability.is_empty());
        }
    }

    #[test]
    fn special_action_mana_restrictions_reject_unknown_purposes() {
        assert!(ron::from_str::<ManaSpendingRestriction>(
            "(label: \"Invalid\", special_actions: [CastAnything])"
        )
        .is_err());
    }

    #[test]
    fn firebending_uses_a_resolving_combat_retained_mana_effect() {
        let effect = SpellEffectKind::AddMana {
            amount: ManaAmount {
                r: 2,
                ..Default::default()
            },
            retention: ManaRetention::EndOfCombat,
        };

        assert!(effect.validate(EffectContext::Ability).is_ok());
        assert!(!matches!(effect, SpellEffectKind::ProduceMana { .. }));
    }

    #[test]
    fn soft_counter_payment_must_be_nonzero() {
        let invalid = SpellEffectKind::CounterTargetSpell {
            spell_filter: None,
            unless_controller_pays: Some(Amount::Fixed(0)),
            unless_controller_pays_by_cast_cost: None,
        };
        assert!(invalid.validate(EffectContext::Spell).is_err());
        let mana_leak_shape = SpellEffectKind::CounterTargetSpell {
            spell_filter: None,
            unless_controller_pays: Some(Amount::Fixed(3)),
            unless_controller_pays_by_cast_cost: None,
        };
        assert!(mana_leak_shape.validate(EffectContext::Spell).is_ok());
    }

    #[test]
    fn zone_card_filters_validate_leaf_and_recursive_or_shapes() {
        let living_phone = ZoneCardFilter {
            card_type: Some(CardTypeFilter::Creature),
            printed_power: Some(PowerComparison::AtMost(2)),
            ..Default::default()
        };
        assert!(living_phone.validate().is_ok());

        let say_its_name = ZoneCardFilter {
            any_of: Some(vec![
                ZoneCardFilter {
                    card_type: Some(CardTypeFilter::Creature),
                    ..Default::default()
                },
                ZoneCardFilter {
                    card_type: Some(CardTypeFilter::Land),
                    ..Default::default()
                },
            ]),
            ..Default::default()
        };
        assert!(say_its_name.validate().is_ok());

        let tempest_hawk = ZoneCardFilter {
            exact_name: Some("Tempest Hawk".into()),
            ..Default::default()
        };
        assert!(tempest_hawk.validate().is_ok());

        assert!(ZoneCardFilter::default().validate().is_err());
        assert!(ZoneCardFilter {
            exact_name: Some(" ".into()),
            ..Default::default()
        }
        .validate()
        .is_err());
        assert!(ZoneCardFilter {
            any_of: Some(vec![tempest_hawk]),
            ..Default::default()
        }
        .validate()
        .is_err());
        assert!(ZoneCardFilter {
            any_of: Some(vec![
                ZoneCardFilter {
                    card_type: Some(CardTypeFilter::Creature),
                    ..Default::default()
                },
                ZoneCardFilter {
                    card_type: Some(CardTypeFilter::Land),
                    ..Default::default()
                },
            ]),
            subtype: Some("Bird".into()),
            ..Default::default()
        }
        .validate()
        .is_err());
    }

    #[test]
    fn graveyard_card_cost_requires_a_positive_bounded_filtered_cohort() {
        let namesake = ZoneCardFilter {
            exact_name: Some("Say Its Name".into()),
            ..Default::default()
        };
        assert!(AbilityCost::ExileGraveyardCards {
            count: 2,
            filter: namesake.clone(),
            exclude_source: true,
        }
        .validate()
        .is_ok());
        assert!(AbilityCost::ExileGraveyardCards {
            count: 0,
            filter: namesake,
            exclude_source: true,
        }
        .validate()
        .is_err());
        assert!(AbilityCost::ExileGraveyardCards {
            count: 1,
            filter: ZoneCardFilter::default(),
            exclude_source: false,
        }
        .validate()
        .is_err());
    }

    #[test]
    fn optional_draw_discard_is_only_single_card_discard_then_draw() {
        let effect = |who, discard_count, order| SpellEffectKind::DrawDiscard {
            who,
            draw_count: 1,
            discard_count,
            order,
            optional: true,
        };
        assert!(effect(
            PlayerRecipient::Controller,
            1,
            DrawDiscardOrder::DiscardThenDraw
        )
        .validate(EffectContext::Ability)
        .is_ok());

        for invalid in [
            effect(
                PlayerRecipient::Controller,
                1,
                DrawDiscardOrder::DrawThenDiscard,
            ),
            effect(
                PlayerRecipient::Controller,
                2,
                DrawDiscardOrder::DiscardThenDraw,
            ),
            effect(
                PlayerRecipient::EachPlayer,
                1,
                DrawDiscardOrder::DiscardThenDraw,
            ),
        ] {
            assert!(invalid.validate(EffectContext::Ability).is_err());
        }
    }

    #[test]
    fn amount_serde_preserves_literals_x_and_named_conditionals() {
        assert_eq!(ron::from_str::<Amount>("4").unwrap(), Amount::Fixed(4));
        assert_eq!(ron::from_str::<Amount>(r#""X""#).unwrap(), Amount::X);

        let amount = Amount::Conditional {
            condition: GameCondition::CreatureDeathsThisTurn {
                min: Some(1),
                max: None,
            },
            when_true: 8,
            otherwise: 4,
        };
        let encoded = ron::to_string(&amount).unwrap();
        assert_eq!(ron::from_str::<Amount>(&encoded).unwrap(), amount);

        for amount in [
            Amount::Count(CountExpression::BattlefieldCreatures {
                filter: BattlefieldCreatureCountFilter {
                    controllers: RelativePlayerSet::Controller,
                    subtype: None,
                    required_keywords: vec![],
                    tapped: None,
                    requires_any_counter: false,
                    required_counter: None,
                    exclude_source: false,
                },
            }),
            Amount::Count(CountExpression::GraveyardCardsNamed {
                owners: RelativePlayerSet::Controller,
                name: "Growth Cycle".into(),
            }),
            Amount::Count(CountExpression::CreatureDeathsThisTurn),
            Amount::Count(CountExpression::CardsMatchingResult {
                filter: CardResultFilter {
                    source: CardResultSource::PreviousEffect,
                    action: CardResultAction::Mill,
                    players: RelativePlayerSet::Controller,
                    card_type: Some(CardTypeFilter::Creature),
                },
            }),
        ] {
            let encoded = ron::to_string(&amount).unwrap();
            assert_eq!(ron::from_str::<Amount>(&encoded).unwrap(), amount);
        }
    }

    #[test]
    fn counted_amounts_reject_empty_authored_filters() {
        assert!(Amount::Count(CountExpression::BattlefieldCreatures {
            filter: BattlefieldCreatureCountFilter {
                controllers: RelativePlayerSet::Controller,
                subtype: Some(" ".into()),
                required_keywords: vec![],
                tapped: None,
                requires_any_counter: false,
                required_counter: None,
                exclude_source: false,
            },
        })
        .validate()
        .is_err());
        assert!(Amount::Count(CountExpression::GraveyardCardsNamed {
            owners: RelativePlayerSet::Controller,
            name: String::new(),
        })
        .validate()
        .is_err());
    }

    #[test]
    fn lose_life_serde_supports_explicit_and_default_player_recipients() {
        let explicit: SpellEffectKind =
            ron::from_str("LoseLife(amount: Fixed(2), who: EachOpponent)")
                .expect("each-opponent life loss should deserialize");
        assert!(matches!(
            explicit,
            SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(2),
                ..
            }
        ));
        assert!(ron::to_string(&explicit)
            .expect("each-opponent life loss should serialize")
            .contains("who:EachOpponent"));

        let legacy: SpellEffectKind = ron::from_str("LoseLife(amount: Fixed(1))")
            .expect("legacy controller life loss should deserialize");
        assert!(ron::to_string(&legacy)
            .expect("legacy controller life loss should serialize")
            .contains("who:Controller"));
    }

    #[test]
    fn turn_history_condition_requires_a_valid_bound() {
        assert!(GameCondition::CreatureDeathsThisTurn {
            min: None,
            max: None,
        }
        .validate()
        .is_err());
        assert!(GameCondition::CreatureDeathsThisTurn {
            min: Some(2),
            max: Some(1),
        }
        .validate()
        .is_err());
        assert!(GameCondition::CreatureDeathsThisTurn {
            min: Some(1),
            max: None,
        }
        .validate()
        .is_ok());
        assert!(GameCondition::SpellsCastThisTurn {
            players: RelativePlayerSet::Controller,
            filter: Default::default(),
            min: None,
            max: None,
        }
        .validate()
        .is_err());
        assert!(GameCondition::SpellsCastThisTurn {
            players: RelativePlayerSet::Controller,
            filter: Default::default(),
            min: Some(1),
            max: None,
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn first_applicable_resolution_branches_require_a_costless_final_fallback() {
        let conditional = ResolutionBranchDef {
            label: "Conditional".into(),
            cost: ResolutionCost::None,
            requirement: ResolutionBranchRequirement::GameCondition(
                GameCondition::CreatureDeathsThisTurn {
                    min: Some(1),
                    max: None,
                },
            ),
            effects: vec![SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }],
        };
        let fallback = ResolutionBranchDef {
            label: "Fallback".into(),
            cost: ResolutionCost::None,
            requirement: ResolutionBranchRequirement::Always,
            effects: vec![SpellEffectKind::GainLife {
                amount: Amount::Fixed(1),
            }],
        };
        let automatic = |optional, chooser, branches| SpellEffectKind::ChooseResolutionBranch {
            chooser,
            optional,
            selection: ResolutionBranchSelection::FirstApplicable,
            branches,
        };

        assert!(automatic(
            false,
            PlayerRecipient::Controller,
            vec![conditional.clone(), fallback.clone()],
        )
        .validate(EffectContext::Ability)
        .is_ok());
        assert!(automatic(
            true,
            PlayerRecipient::Controller,
            vec![conditional.clone(), fallback.clone()],
        )
        .validate(EffectContext::Ability)
        .is_err());
        assert!(automatic(
            false,
            PlayerRecipient::SourceController,
            vec![conditional.clone(), fallback.clone()],
        )
        .validate(EffectContext::Ability)
        .is_err());
        assert!(automatic(
            false,
            PlayerRecipient::Controller,
            vec![conditional.clone()],
        )
        .validate(EffectContext::Ability)
        .is_err());

        let mut costed = conditional;
        costed.cost = ResolutionCost::DiscardCard { filter: None };
        assert!(
            automatic(false, PlayerRecipient::Controller, vec![costed, fallback],)
                .validate(EffectContext::Ability)
                .is_err()
        );
    }

    #[test]
    fn turn_history_trigger_ordinals_must_be_positive() {
        assert!(TriggerCondition::WheneverPlayerCastsSpell {
            caster: CastTriggerPlayer::Controller,
            filter: crate::SpellCastFilter {
                card_type: None,
                ..Default::default()
            },
            ordinal: Some(0),
            ordinal_scope: Default::default(),
        }
        .validate()
        .is_err());
        assert!(TriggerCondition::WheneverPlayerDrawsNthCard {
            drawer: CastTriggerPlayer::Controller,
            ordinal: 0,
        }
        .validate()
        .is_err());
        assert!(TriggerCondition::WheneverPlayerDrawsNthCard {
            drawer: CastTriggerPlayer::Controller,
            ordinal: 2,
        }
        .validate()
        .is_ok());
    }

    #[test]
    fn graveyard_aggregate_condition_requires_valid_bounds_and_round_trips() {
        let condition = |min, max| GameCondition::GraveyardAggregate {
            owners: RelativePlayerSet::Controller,
            aggregate: GraveyardAggregate::DistinctCardTypes,
            filter: None,
            min,
            max,
        };
        assert!(condition(None, None).validate().is_err());
        assert!(condition(Some(4), Some(3)).validate().is_err());

        let delirium = condition(Some(4), None);
        delirium.validate().expect("valid delirium condition");
        let serialized = ron::to_string(&delirium).expect("serialize delirium condition");
        let decoded: GameCondition =
            ron::from_str(&serialized).expect("deserialize delirium condition");
        assert_eq!(decoded, delirium);
    }

    #[test]
    fn activation_conditions_require_valid_count_bounds() {
        let condition = |min, max| ActivationCondition::BattlefieldCreatureCount {
            filter: BattlefieldCreatureCountFilter {
                controllers: RelativePlayerSet::Controller,
                subtype: None,
                required_keywords: vec![Keyword::Flying],
                tapped: None,
                requires_any_counter: false,
                required_counter: None,
                exclude_source: false,
            },
            min,
            max,
        };
        assert!(condition(None, None).validate().is_err());
        assert!(condition(Some(2), Some(1)).validate().is_err());
        assert!(condition(Some(1), None).validate().is_ok());
    }

    #[test]
    fn spell_cost_conditions_require_valid_counts_and_nonzero_reductions() {
        let condition = |min, max| GameCondition::BattlefieldCreatureCount {
            filter: BattlefieldCreatureCountFilter {
                controllers: RelativePlayerSet::Controller,
                subtype: None,
                required_keywords: vec![Keyword::Flying],
                tapped: None,
                requires_any_counter: false,
                required_counter: None,
                exclude_source: false,
            },
            min,
            max,
        };
        assert!(condition(None, None).validate().is_err());
        assert!(condition(Some(2), Some(1)).validate().is_err());

        let zero = SpellCostModifier::ConditionalGenericReduction {
            amount: 0,
            condition: condition(Some(1), None),
        };
        assert!(zero.validate().is_err());
        let winged_words = SpellCostModifier::ConditionalGenericReduction {
            amount: 1,
            condition: condition(Some(1), None),
        };
        assert!(winged_words.validate().is_ok());
    }

    #[test]
    fn explicit_and_intrinsic_sorcery_speed_share_one_query() {
        let explicit = ActivatedAbilityDef {
            source_zone: AbilitySourceZone::Battlefield,
            costs: vec![],
            cost_modifiers: vec![],
            effect: vec![],
            targeting: None,
            timing: ActivationTiming::SorcerySpeed,
            conditions: vec![],
            activation_limit: None,
            text: String::new(),
        };
        assert!(explicit.requires_sorcery_speed());

        let equip = ActivatedAbilityDef {
            source_zone: AbilitySourceZone::Battlefield,
            costs: vec![],
            cost_modifiers: vec![],
            effect: vec![SpellEffectKind::Equip {
                target: TargetFilter::default(),
            }],
            targeting: None,
            timing: ActivationTiming::Normal,
            conditions: vec![],
            activation_limit: None,
            text: String::new(),
        };
        assert!(equip.requires_sorcery_speed());
    }

    #[test]
    fn activation_limit_rejects_zero_maximum_for_every_scope() {
        assert_eq!(
            ron::from_str::<ActivationLimit>("PerObject(max_activations: 1)")
                .expect("deserialize the authored Exhaust limit"),
            ActivationLimit::PerObject { max_activations: 1 }
        );
        let ability_with = |activation_limit| ActivatedAbilityDef {
            source_zone: AbilitySourceZone::Battlefield,
            costs: vec![],
            cost_modifiers: vec![],
            effect: vec![SpellEffectKind::ProduceMana {
                options: vec![ManaAmount {
                    g: 1,
                    ..Default::default()
                }],
                restriction: None,
                conditional: None,
            }],
            targeting: None,
            timing: ActivationTiming::Normal,
            conditions: vec![],
            activation_limit: Some(activation_limit),
            text: "Add {G}.".into(),
        };
        assert!(
            ability_with(ActivationLimit::PerTurn { max_activations: 0 })
                .validate_shape()
                .is_err()
        );
        assert!(
            ability_with(ActivationLimit::PerObject { max_activations: 0 })
                .validate_shape()
                .is_err()
        );
    }

    #[test]
    fn damage_targets_rejects_conditional_amounts() {
        let effect = SpellEffectKind::DamageTargets {
            amount: Amount::Conditional {
                condition: GameCondition::CreatureDeathsThisTurn {
                    min: Some(1),
                    max: None,
                },
                when_true: 5,
                otherwise: 3,
            },
            target: TargetFilter::default(),
            division: DamageDivision::ChooseAtCast,
            extra_mana_per_target: 0,
        };
        assert!(effect.validate(EffectContext::Spell).is_err());
    }

    #[test]
    fn previous_result_amount_requires_an_immediately_preceding_compatible_effect() {
        let counted_gain = SpellEffectKind::GainLife {
            amount: Amount::Count(CountExpression::CardsMatchingResult {
                filter: CardResultFilter {
                    source: CardResultSource::PreviousEffect,
                    action: CardResultAction::Mill,
                    players: RelativePlayerSet::Controller,
                    card_type: Some(CardTypeFilter::Creature),
                },
            }),
        };
        let mill = SpellEffectKind::Mill {
            count: Amount::Fixed(4),
            who: PlayerRecipient::Controller,
        };

        assert!(SpellEffectKind::validate_list(&[mill.clone(), counted_gain.clone()]).is_ok());
        assert!(SpellEffectKind::validate_list(std::slice::from_ref(&counted_gain)).is_err());
        assert!(SpellEffectKind::validate_list(&[
            mill,
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            },
            counted_gain,
        ])
        .is_err());
    }

    #[test]
    fn player_effect_accepts_player_spec() {
        assert!(SpellEffectKind::TargetPlayerLosesLife {
            amount: 3,
            target: TargetFilter {
                kind: TargetKind::OpponentPlayer,
                ..Default::default()
            },
        }
        .validate(EffectContext::Spell)
        .is_ok());
    }

    #[test]
    fn player_effect_rejects_nonplayer_spec() {
        assert!(SpellEffectKind::TargetPlayerGainsLife {
            amount: 3,
            target: TargetFilter {
                kind: TargetKind::Creature,
                ..Default::default()
            },
        }
        .validate(EffectContext::Spell)
        .is_err());
    }

    #[test]
    fn damage_accepts_any_kind() {
        for kind in [
            TargetKind::AnyTarget,
            TargetKind::Creature,
            TargetKind::AnyPlayer,
            TargetKind::OpponentPlayer,
        ] {
            assert!(SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(3),
                target: TargetFilter {
                    kind,
                    ..Default::default()
                },
            }
            .validate(EffectContext::Spell)
            .is_ok());
        }
    }

    /// Issue #39: untargeted mass selection has no activating player, so a controller relation can
    /// never be honored there — the effect's own `players` scope is the correct knob. Reject the
    /// filter at load rather than silently ignoring the field.
    #[test]
    fn mass_effect_rejects_controller_relative_filter() {
        let scoped = TargetFilter {
            kind: TargetKind::Creature,
            controller: TargetController::You,
            ..Default::default()
        };
        assert!(SpellEffectKind::DestroyAll {
            kind: scoped.clone(),
            prevent_regeneration: false,
        }
        .validate(EffectContext::Spell)
        .is_err());
        assert!(SpellEffectKind::DamageAll {
            amount: Amount::Fixed(2),
            kind: scoped,
        }
        .validate(EffectContext::Spell)
        .is_err());

        // The same effect without the controller restriction still loads.
        assert!(SpellEffectKind::DestroyAll {
            kind: TargetFilter {
                kind: TargetKind::Creature,
                ..Default::default()
            },
            prevent_regeneration: true,
        }
        .validate(EffectContext::Spell)
        .is_ok());
    }

    #[test]
    fn source_subject_rejected_in_spell_context_allowed_in_ability() {
        let pump_self = SpellEffectKind::PumpTarget {
            power: 1,
            toughness: 1,
            scale: None,
            subject: EffectSubject::Source,
        };
        assert!(pump_self.validate(EffectContext::Spell).is_err());
        assert!(pump_self.validate(EffectContext::Ability).is_ok());

        let fight_from_source = SpellEffectKind::Fight {
            first: EffectSubject::Source,
            second: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::NotYou,
                ..Default::default()
            })),
        };
        assert_eq!(fight_from_source.target_filters().len(), 1);
        assert!(fight_from_source.validate(EffectContext::Spell).is_err());
        assert!(fight_from_source.validate(EffectContext::Ability).is_ok());

        let invalid_fight = SpellEffectKind::Fight {
            first: EffectSubject::Source,
            second: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::AnyPermanent,
                ..Default::default()
            })),
        };
        assert!(invalid_fight.validate(EffectContext::Ability).is_err());
    }

    #[test]
    fn combat_restriction_scopes_validate_by_context_and_subject_kind() {
        let cant_block = CombatRestriction {
            cant_block: true,
            ..Default::default()
        };
        let source = SpellEffectKind::ApplyCombatRestriction {
            scope: CombatRestrictionScope::Source,
            restriction: cant_block.clone(),
        };
        assert!(source.validate(EffectContext::Spell).is_err());
        assert!(source.validate(EffectContext::Ability).is_ok());

        let chosen_player = SpellEffectKind::ApplyCombatRestriction {
            scope: CombatRestrictionScope::Chosen(TargetFilter {
                kind: TargetKind::OpponentPlayer,
                ..Default::default()
            }),
            restriction: cant_block.clone(),
        };
        assert!(chosen_player.validate(EffectContext::Spell).is_err());

        let matching_creatures = SpellEffectKind::ApplyCombatRestriction {
            scope: CombatRestrictionScope::Matching(TargetFilter {
                kind: TargetKind::Creature,
                excluded_keywords: vec![Keyword::Flying],
                ..Default::default()
            }),
            restriction: cant_block.clone(),
        };
        assert!(matching_creatures.validate(EffectContext::Spell).is_ok());

        let matching_permanents = SpellEffectKind::ApplyCombatRestriction {
            scope: CombatRestrictionScope::Matching(TargetFilter {
                kind: TargetKind::AnyPermanent,
                ..Default::default()
            }),
            restriction: cant_block.clone(),
        };
        assert!(matching_permanents.validate(EffectContext::Spell).is_err());

        let empty = SpellEffectKind::ApplyCombatRestriction {
            scope: CombatRestrictionScope::Chosen(TargetFilter::default_creature()),
            restriction: CombatRestriction::default(),
        };
        assert!(empty.validate(EffectContext::Spell).is_err());
    }

    #[test]
    fn return_to_owners_hand_uses_a_validated_permanent_subject() {
        let opponent_creature = SpellEffectKind::ReturnToOwnersHand {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::Opponent,
                ..Default::default()
            })),
        };
        assert_eq!(opponent_creature.target_filters().len(), 1);
        assert!(opponent_creature.validate(EffectContext::Ability).is_ok());

        let player = SpellEffectKind::ReturnToOwnersHand {
            subject: EffectSubject::Chosen(Box::new(TargetFilter {
                kind: TargetKind::OpponentPlayer,
                ..Default::default()
            })),
        };
        assert!(player.validate(EffectContext::Ability).is_err());

        let source = SpellEffectKind::ReturnToOwnersHand {
            subject: EffectSubject::Source,
        };
        assert!(source.validate(EffectContext::Ability).is_ok());
        assert!(source.validate(EffectContext::Spell).is_err());
    }

    #[test]
    fn chosen_subject_defaults_to_creature_target() {
        assert_eq!(
            EffectSubject::default(),
            EffectSubject::Chosen(Box::new(TargetFilter::default_creature()))
        );
    }

    #[test]
    fn target_controller_relation_is_a_composable_filter_dimension() {
        let filter = TargetFilter {
            kind: TargetKind::Creature,
            controller: TargetController::Opponent,
            tapped: Some(true),
            not_artifact: true,
            ..TargetFilter::default()
        };

        assert_eq!(filter.controller, TargetController::Opponent);
        assert_eq!(filter.tapped, Some(true));
        assert!(filter.not_artifact);
        assert_eq!(TargetFilter::default().controller, TargetController::Any);
    }

    #[test]
    fn controller_relative_target_filter_requires_a_permanent_kind() {
        for kind in [
            TargetKind::AnyTarget,
            TargetKind::AnyPlayer,
            TargetKind::OpponentPlayer,
        ] {
            let effect = SpellEffectKind::DamageTarget {
                amount: Amount::Fixed(1),
                target: TargetFilter {
                    kind,
                    controller: TargetController::Opponent,
                    ..TargetFilter::default()
                },
            };
            assert!(effect.validate(EffectContext::Spell).is_err());
        }
    }

    #[test]
    fn source_exclusion_requires_an_object_capable_target_kind() {
        for kind in [TargetKind::AnyPlayer, TargetKind::OpponentPlayer] {
            let effect = SpellEffectKind::TargetPlayerGainsLife {
                amount: 1,
                target: TargetFilter {
                    kind,
                    excluded_objects: vec![crate::TargetObjectExclusion::Source],
                    ..TargetFilter::default()
                },
            };
            assert!(effect.validate(EffectContext::Spell).is_err());
        }

        let object_capable = SpellEffectKind::DamageTarget {
            amount: Amount::Fixed(1),
            target: TargetFilter {
                kind: TargetKind::AnyTarget,
                excluded_objects: vec![crate::TargetObjectExclusion::Source],
                ..TargetFilter::default()
            },
        };
        assert!(object_capable.validate(EffectContext::Ability).is_ok());
    }

    #[test]
    fn untargeted_filters_reject_source_exclusion() {
        let filter = TargetFilter {
            kind: TargetKind::Creature,
            excluded_objects: vec![crate::TargetObjectExclusion::Source],
            ..TargetFilter::default()
        };
        assert!(SpellEffectKind::DestroyAll {
            kind: filter.clone(),
            prevent_regeneration: false,
        }
        .validate(EffectContext::Spell)
        .is_err());
        assert!(SpellEffectKind::GrantKeywordsAllPermanents {
            filter,
            keywords: vec![Keyword::Flying],
        }
        .validate(EffectContext::Ability)
        .is_err());
    }

    #[test]
    fn untargeted_filter_preserves_you_scope_but_defers_opponent_scope() {
        let keywords = vec![Keyword::Indestructible];
        let filter = |controller| TargetFilter {
            kind: TargetKind::AnyPermanent,
            controller,
            ..TargetFilter::default()
        };

        assert!(SpellEffectKind::GrantKeywordsAllPermanents {
            filter: filter(TargetController::You),
            keywords: keywords.clone(),
        }
        .validate(EffectContext::Spell)
        .is_ok());
        assert!(SpellEffectKind::GrantKeywordsAllPermanents {
            filter: filter(TargetController::Opponent),
            keywords,
        }
        .validate(EffectContext::Spell)
        .is_err());
        assert!(SpellEffectKind::TargetPlayerSacrifices {
            target: TargetFilter {
                kind: TargetKind::OpponentPlayer,
                ..TargetFilter::default()
            },
            filter: TargetFilter {
                kind: TargetKind::Creature,
                controller: TargetController::You,
                ..TargetFilter::default()
            },
        }
        .validate(EffectContext::Spell)
        .is_err());
    }
}
