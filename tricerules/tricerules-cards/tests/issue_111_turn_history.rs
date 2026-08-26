use tricerules_cards::primitives::{
    ActivationLimit, DrawDiscardOrder, EffectSubject, GameCondition, RelativePlayerSet,
    SpellCostModifier, SpellEffectKind,
};
use tricerules_cards::{AbilityCost, CardRegistry, CastTriggerPlayer, TriggerCondition};

#[test]
fn second_event_cards_use_shared_ordinal_triggers() {
    let erudite = CardRegistry::global()
        .get("erudite_wizard")
        .expect("Erudite Wizard must be registered")
        .primary_face();
    assert_eq!(erudite.mana_cost.to_string(), "{2}{U}");
    assert_eq!(erudite.types, ["Creature", "Human", "Wizard"]);
    assert_eq!((erudite.power, erudite.toughness), (Some(2), Some(3)));
    assert_eq!(erudite.triggered_abilities.len(), 1);
    assert_eq!(
        erudite.triggered_abilities[0].trigger,
        TriggerCondition::WheneverPlayerDrawsNthCard {
            drawer: CastTriggerPlayer::Controller,
            ordinal: 2,
        }
    );
    assert!(matches!(
        erudite.triggered_abilities[0].effect.as_slice(),
        [SpellEffectKind::PutCounters {
            count: 1,
            subject: EffectSubject::Source,
            ..
        }]
    ));

    let poised = CardRegistry::global()
        .get("poised_practitioner")
        .expect("Poised Practitioner must be registered")
        .primary_face();
    assert_eq!(poised.mana_cost.to_string(), "{2}{W}");
    assert_eq!(poised.types, ["Creature", "Human", "Monk"]);
    assert_eq!((poised.power, poised.toughness), (Some(2), Some(3)));
    assert_eq!(
        poised.triggered_abilities[0].trigger,
        TriggerCondition::WheneverPlayerCastsSpell {
            caster: CastTriggerPlayer::Controller,
            spell_type: None,
            ordinal: Some(2),
            min_mana_value: None,
            max_mana_value: None,
        }
    );
    assert!(matches!(
        poised.triggered_abilities[0].effect.as_slice(),
        [
            SpellEffectKind::PutCounters { .. },
            SpellEffectKind::Scry { count: 1 }
        ]
    ));

    let jeskai = CardRegistry::global()
        .get("jeskai_devotee")
        .expect("Jeskai Devotee must be registered")
        .primary_face();
    assert_eq!(jeskai.mana_cost.to_string(), "{1}{R}");
    assert_eq!(jeskai.types, ["Creature", "Orc", "Monk"]);
    assert_eq!((jeskai.power, jeskai.toughness), (Some(2), Some(2)));
    assert_eq!(
        jeskai.triggered_abilities[0].trigger,
        TriggerCondition::WheneverPlayerCastsSpell {
            caster: CastTriggerPlayer::Controller,
            spell_type: None,
            ordinal: Some(2),
            min_mana_value: None,
            max_mana_value: None,
        }
    );
    assert!(matches!(
        jeskai.triggered_abilities[0].effect.as_slice(),
        [SpellEffectKind::PumpTarget {
            power: 1,
            toughness: 1,
            subject: EffectSubject::Source,
            scale: None,
        }]
    ));
    let mana_ability = &jeskai.activated_abilities[0];
    assert!(matches!(
        mana_ability.costs.as_slice(),
        [AbilityCost::Mana(cost)] if cost.to_string() == "{1}"
    ));
    assert_eq!(
        mana_ability.activation_limit,
        Some(ActivationLimit::PerTurn { max_activations: 1 })
    );
    assert!(matches!(
        mana_ability.effect.as_slice(),
        [SpellEffectKind::ProduceMana { options, .. }]
            if options.len() == 3
                && options[0].u == 1
                && options[1].r == 1
                && options[2].w == 1
    ));
}

#[test]
fn focus_the_mind_uses_per_player_cast_history_for_its_complete_effect() {
    let focus = CardRegistry::global()
        .get("focus_the_mind")
        .expect("Focus the Mind must be registered")
        .primary_face();
    assert_eq!(focus.mana_cost.to_string(), "{4}{U}");
    assert_eq!(focus.types, ["Instant"]);
    assert_eq!(
        focus.cost_modifiers,
        [SpellCostModifier::ConditionalGenericReduction {
            amount: 2,
            condition: GameCondition::SpellsCastThisTurn {
                players: RelativePlayerSet::Controller,
                min: Some(1),
                max: None,
            },
        }]
    );
    assert!(matches!(
        focus.spell_effect.as_slice(),
        [SpellEffectKind::DrawDiscard {
            draw_count: 3,
            discard_count: 1,
            order: DrawDiscardOrder::DrawThenDiscard,
            optional: false,
            ..
        }]
    ));
}
