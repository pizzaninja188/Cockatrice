use tricerules_cards::primitives::{
    AbilityCost, CastTriggerPlayer, CreatureScopeController, CreatureScopeFilter, EffectSubject,
    PermanentTypeFilter, PlayerRecipient, SpellEffectKind, StaticAbilityDef, TargetFilter,
    TargetKind, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Color, GameCondition, Keyword};

#[test]
fn devotion_gods_register_exact_faces_costs_types_keywords_and_oracle_lines() {
    for (id, name, color, cost, toughness) in [
        (
            "nylea,_god_of_the_hunt",
            "Nylea, God of the Hunt",
            Color::Green,
            "{3}{G}",
            6,
        ),
        (
            "purphoros,_god_of_the_forge",
            "Purphoros, God of the Forge",
            Color::Red,
            "{3}{R}",
            5,
        ),
    ] {
        let card = tricerules_cards::registry::global()
            .get(id)
            .expect("the complete God must be registered");
        assert_eq!(card.name, name);
        assert_eq!(card.faces_iter().count(), 1);
        let face = card.primary_face();
        assert_eq!(
            face.face_id.as_str(),
            if color == Color::Green {
                "nylea_god_of_the_hunt"
            } else {
                "purphoros_god_of_the_forge"
            }
        );
        assert_eq!(face.mana_cost.to_string(), cost);
        assert_eq!(face.types, ["Enchantment", "Creature", "God"]);
        assert_eq!(face.supertypes, ["Legendary"]);
        assert_eq!((face.power, face.toughness), (Some(6), Some(toughness)));
        assert_eq!(face.keywords, [Keyword::Indestructible]);
        assert!(face.spell_effect.is_empty() && face.targeting.is_none());
        assert_eq!(
            face.static_abilities[0].presentation,
            AbilityPresentation::OracleLines(vec![2])
        );
        let StaticAbilityDef::ConditionalSelfModifier {
            condition,
            remove_creature,
            set_types,
            add_types,
            ..
        } = &face.static_abilities[0].definition
        else {
            panic!("narrow devotion type removal required");
        };
        assert_eq!(
            condition,
            &GameCondition::Devotion {
                color,
                min: None,
                max: Some(4)
            }
        );
        assert!(*remove_creature && set_types.is_none() && add_types.is_empty());
        assert_eq!(face.activated_abilities.len(), 1);
        assert_eq!(
            face.activated_abilities[0].presentation,
            AbilityPresentation::OracleLines(vec![4])
        );
        assert!(face.activated_abilities[0].targeting.is_none());
        assert!(
            matches!(face.activated_abilities[0].costs.as_slice(), [AbilityCost::Mana(mana)] if mana.to_string() == if color == Color::Green { "{3}{G}" } else { "{2}{R}" })
        );
        if color == Color::Green {
            assert_eq!(face.static_abilities.len(), 2);
            assert_eq!(
                face.static_abilities[1].presentation,
                AbilityPresentation::OracleLines(vec![3])
            );
            assert!(face.triggered_abilities.is_empty());
            let StaticAbilityDef::AnthemKeyword {
                filter,
                keyword,
                condition,
            } = &face.static_abilities[1].definition
            else {
                panic!("other own creatures get trample");
            };
            assert!(condition.is_none());
            assert_eq!(*keyword, Keyword::Trample);
            assert!(filter.exclude_self);
            assert_eq!(
                *filter,
                CreatureScopeFilter {
                    controller: Some(CreatureScopeController::YouControl),
                    exclude_self: true,
                    ..Default::default()
                }
            );
            assert_eq!(
                face.activated_abilities[0].effect,
                vec![SpellEffectKind::PumpTarget {
                    power: 2,
                    toughness: 2,
                    scale: None,
                    subject: EffectSubject::Chosen(Box::new(TargetFilter {
                        kind: TargetKind::Creature,
                        ..Default::default()
                    })),
                }]
            );
        } else {
            assert_eq!(face.static_abilities.len(), 1);
            assert_eq!(face.triggered_abilities.len(), 1);
            assert_eq!(
                face.triggered_abilities[0].presentation,
                AbilityPresentation::OracleLines(vec![3])
            );
            let TriggerCondition::WheneverPermanentEntersBattlefield {
                controller,
                filter,
                creature_filter,
            } = &face.triggered_abilities[0].trigger
            else {
                panic!("other controlled creature entrants required");
            };
            assert!(creature_filter.is_none());
            assert_eq!(*controller, CastTriggerPlayer::Controller);
            assert!(filter.exclude_source);
            assert_eq!(filter.permanent_type, Some(PermanentTypeFilter::Creature));
            assert_eq!(
                face.triggered_abilities[0].effect,
                vec![SpellEffectKind::DamagePlayer {
                    amount: tricerules_cards::Amount::Fixed(2),
                    who: PlayerRecipient::EachOpponent
                }]
            );
            assert_eq!(
                face.activated_abilities[0].effect,
                vec![SpellEffectKind::PumpAll {
                    filter: CreatureScopeFilter {
                        controller: Some(CreatureScopeController::YouControl),
                        ..Default::default()
                    },
                    power: 1,
                    toughness: 0,
                }]
            );
        }
    }
}

#[test]
fn devotion_rejects_inverted_bounds_and_empty_self_modifiers() {
    for definition in [
        "ConditionalSelfModifier(condition: Devotion(color: Green, min: Some(5), max: Some(4)), remove_creature: true)",
        "ConditionalSelfModifier(condition: Devotion(color: Red, max: Some(4)))",
    ] {
        let text = format!("(id: \"devotion_validation\", name: \"Devotion Validation\", face_id: \"devotion_validation\", types: [\"Enchantment\"], static_abilities: [(ability_id: \"static_01\", presentation: Fallback, definition: {definition})])");
        assert!(CardRegistry::from_chunks_and_tokens(&[&text], &[]).is_err());
    }
}
