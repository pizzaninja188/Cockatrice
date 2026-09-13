use tricerules_cards::primitives::{
    Amount, CardTypeFilter, CastTriggerPlayer, CreatureScopeController, DrawDiscardOrder,
    GameCondition, GraveyardDestination, Keyword, PermanentTypeFilter, PlayerRecipient,
    RelativePlayerSet, SearchDestination, SpellEffectKind, StaticAbilityDef, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Layout};

fn assert_oracle_lines(presentation: &AbilityPresentation, expected: &[u16]) {
    assert_eq!(
        presentation,
        &AbilityPresentation::OracleLines(expected.to_vec())
    );
}

#[test]
fn issue_267_registers_exactly_the_reviewed_cohort() {
    let registry = CardRegistry::global();
    for (id, name) in [
        ("anthem_of_champions", "Anthem of Champions"),
        ("bearer_of_glory", "Bearer of Glory"),
        ("daring_thunder-thief", "Daring Thunder-Thief"),
        ("diregraf_ghoul", "Diregraf Ghoul"),
        ("feisty_spikeling", "Feisty Spikeling"),
        ("piranha_fly", "Piranha Fly"),
        ("vampire_interloper", "Vampire Interloper"),
        ("icetill_explorer", "Icetill Explorer"),
        ("inspiring_paladin", "Inspiring Paladin"),
        (
            "null_group_biological_assets",
            "Null Group Biological Assets",
        ),
        (
            "ratcatcher_trainee_pest_problem",
            "Ratcatcher Trainee // Pest Problem",
        ),
        (
            "the_arkenstone_seek_the_heart",
            "The Arkenstone // Seek the Heart",
        ),
        ("vampire_soulcaller", "Vampire Soulcaller"),
        ("warleaders_call", "Warleader's Call"),
    ] {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.name, name);
        assert_eq!(registry.id_for_name(name), Some(id));
    }
}

#[test]
fn issue_267_static_abilities_keep_typed_conditions_and_presentation() {
    let registry = CardRegistry::global();

    for (id, lines) in [
        ("bearer_of_glory", &[1]),
        ("feisty_spikeling", &[2]),
        ("null_group_biological_assets", &[1]),
        ("ratcatcher_trainee_pest_problem", &[1]),
    ] {
        let face = registry.get(id).unwrap().primary_face();
        let ability = face
            .static_abilities
            .iter()
            .find(|ability| ability.ability_id.as_str() == "static_01")
            .unwrap_or_else(|| panic!("{id} missing first-strike static"));
        assert_oracle_lines(&ability.presentation, lines);
        assert!(matches!(
            &ability.definition,
            StaticAbilityDef::ConditionalSelfModifier {
                condition: GameCondition::ActivePlayer {
                    players: RelativePlayerSet::Controller
                },
                keywords,
                ..
            } if keywords == &[Keyword::FirstStrike]
        ));
    }

    let paladin = registry.get("inspiring_paladin").unwrap().primary_face();
    let countered = &paladin.static_abilities[1];
    assert_eq!(countered.ability_id.as_str(), "static_02");
    assert_oracle_lines(&countered.presentation, &[2]);
    assert!(matches!(
        &countered.definition,
        StaticAbilityDef::AnthemKeyword {
            filter,
            condition: Some(GameCondition::ActivePlayer {
                players: RelativePlayerSet::Controller
            }),
            keyword: Keyword::FirstStrike,
        } if filter.controller == Some(CreatureScopeController::YouControl)
            && filter.required_counter == Some(tricerules_cards::CounterKind::PlusOnePlusOne)
    ));

    for id in ["anthem_of_champions", "warleaders_call"] {
        let ability = &registry.get(id).unwrap().primary_face().static_abilities[0];
        assert_oracle_lines(&ability.presentation, &[1]);
        assert!(matches!(
            &ability.definition,
            StaticAbilityDef::AnthemPt {
                filter,
                condition: None,
                delta_power: 1,
                delta_toughness: 1,
            } if filter.controller == Some(CreatureScopeController::YouControl)
        ));
    }

    for (id, lines) in [
        ("daring_thunder-thief", &[2]),
        ("diregraf_ghoul", &[1]),
        ("piranha_fly", &[2]),
    ] {
        let ability = &registry.get(id).unwrap().primary_face().static_abilities[0];
        assert_oracle_lines(&ability.presentation, lines);
        assert!(matches!(
            &ability.definition,
            StaticAbilityDef::EntersTapped {
                affected: tricerules_cards::primitives::EntersTappedAffected::Self_,
                condition: None,
                unless_cost: None,
            }
        ));
    }

    for id in ["vampire_interloper", "vampire_soulcaller"] {
        let face = registry.get(id).unwrap().primary_face();
        let ability = face
            .static_abilities
            .iter()
            .find(|ability| ability.ability_id.as_str() == "static_01")
            .unwrap();
        assert_oracle_lines(&ability.presentation, &[2]);
        assert!(matches!(
            &ability.definition,
            StaticAbilityDef::SelfCombatRestriction {
                restriction,
                condition: None,
            } if restriction.cant_block
        ));
    }

    let icetill = registry.get("icetill_explorer").unwrap().primary_face();
    assert!(matches!(
        &icetill.static_abilities[0].definition,
        StaticAbilityDef::ExtraLandPlays { count: 1 }
    ));
    assert_oracle_lines(&icetill.static_abilities[0].presentation, &[1]);
    assert!(matches!(
        &icetill.static_abilities[1].definition,
        StaticAbilityDef::PlayLandsFromOwnGraveyard
    ));
    assert_oracle_lines(&icetill.static_abilities[1].presentation, &[2]);
}

#[test]
fn issue_267_trigger_and_spell_effects_keep_exact_filters_and_order() {
    let registry = CardRegistry::global();

    let icetill = &registry
        .get("icetill_explorer")
        .unwrap()
        .primary_face()
        .triggered_abilities[0];
    assert_eq!(
        icetill.trigger,
        TriggerCondition::WheneverPermanentEntersBattlefield {
            controller: CastTriggerPlayer::Controller,
            filter: tricerules_cards::primitives::PermanentEventFilter {
                permanent_type: Some(PermanentTypeFilter::Land),
                ..Default::default()
            },
            creature_filter: None,
        }
    );
    assert_eq!(
        icetill.effect,
        [SpellEffectKind::Mill {
            count: Amount::Fixed(1),
            who: PlayerRecipient::Controller,
        }]
    );

    let null_group = &registry
        .get("null_group_biological_assets")
        .unwrap()
        .primary_face()
        .triggered_abilities[0];
    assert_eq!(
        null_group.trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0
        }
    );
    assert_eq!(
        null_group.effect,
        [SpellEffectKind::DrawDiscard {
            who: PlayerRecipient::Controller,
            draw_count: 1,
            discard_count: 1,
            order: DrawDiscardOrder::DiscardThenDraw,
            optional: true,
        }]
    );

    let soulcaller = &registry
        .get("vampire_soulcaller")
        .unwrap()
        .primary_face()
        .triggered_abilities[0];
    assert_eq!(
        soulcaller.trigger,
        TriggerCondition::WhenSelfEntersBattlefield
    );
    assert!(matches!(
        soulcaller.effect.as_slice(),
        [SpellEffectKind::MoveGraveyardCards {
            filter,
            destination: GraveyardDestination::Hand,
            linked_exile_id: None,
        }] if filter.card.as_ref().is_some_and(|card| card.card_type == Some(CardTypeFilter::Creature))
    ));

    let call = &registry
        .get("warleaders_call")
        .unwrap()
        .primary_face()
        .triggered_abilities[0];
    assert_eq!(
        call.trigger,
        TriggerCondition::WheneverPermanentEntersBattlefield {
            controller: CastTriggerPlayer::Controller,
            filter: tricerules_cards::primitives::PermanentEventFilter {
                permanent_type: Some(PermanentTypeFilter::Creature),
                ..Default::default()
            },
            creature_filter: None,
        }
    );
    assert_eq!(
        call.effect,
        [SpellEffectKind::DamagePlayer {
            amount: Amount::Fixed(1),
            who: PlayerRecipient::EachOpponent,
        }]
    );

    let adventure = registry.get("ratcatcher_trainee_pest_problem").unwrap();
    assert_eq!(adventure.layout, Layout::Adventure);
    assert!(matches!(
        adventure.faces[1].spell_effect.as_slice(),
        [SpellEffectKind::CreateTokens {
            token,
            count: Amount::Fixed(2),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }] if token == "rat_b_1_1_cant_block"
    ));

    let seek = registry.get("the_arkenstone_seek_the_heart").unwrap();
    assert_eq!(seek.layout, Layout::Adventure);
    assert!(matches!(
        seek.faces[1].spell_effect.as_slice(),
        [SpellEffectKind::SearchLibrary {
            filter: Some(filter),
            destination: SearchDestination::Hand,
            shuffle: true,
            reveal: true,
            optional: false,
            ..
        }] if filter.card_type == Some(CardTypeFilter::Creature)
            && filter.required_supertypes == ["Legendary"]
    ));
    assert_eq!(
        seek.faces[0].triggered_abilities[0].trigger,
        TriggerCondition::AtBeginningOfEndStep {
            player: CastTriggerPlayer::Controller,
        }
    );
}
