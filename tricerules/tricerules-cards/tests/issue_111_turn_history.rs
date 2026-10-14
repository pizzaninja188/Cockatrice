use crate::common::draw_discard_program::{
    assert_face_draw_discard_steps, AuthoredDrawDiscardExpectation, AuthoredDrawDiscardOrder,
};
use tricerules_cards::primitives::{
    ActivationLimit, EffectSubject, GameCondition, PlayerRecipient, RelativePlayerSet,
    SpellCostModifier, SpellEffectKind,
};
use tricerules_cards::{AbilityCost, CastTriggerPlayer, TriggerCondition};

#[test]
fn second_event_cards_use_shared_ordinal_triggers() {
    let erudite = tricerules_cards::registry::global()
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
            count: tricerules_cards::Amount::Fixed(1),
            subject: EffectSubject::Source,
            ..
        }]
    ));

    let poised = tricerules_cards::registry::global()
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
            filter: tricerules_cards::SpellCastFilter {
                card_type: None,
                ..Default::default()
            },
            ordinal: Some(2),
            ordinal_scope: Default::default(),
        }
    );
    assert!(matches!(
        poised.triggered_abilities[0].effect.as_slice(),
        [
            SpellEffectKind::PutCounters { .. },
            SpellEffectKind::Scry {
                count: tricerules_cards::Amount::Fixed(1)
            }
        ]
    ));

    let jeskai = tricerules_cards::registry::global()
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
            filter: tricerules_cards::SpellCastFilter {
                card_type: None,
                ..Default::default()
            },
            ordinal: Some(2),
            ordinal_scope: Default::default(),
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
    let registry = tricerules_cards::registry::global();
    let focus = registry
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
                filter: Default::default(),
                min: Some(1),
                max: None,
            },
        }]
    );
    assert!(focus.spell_effect.is_empty() && focus.spell_program.is_some());
    assert_face_draw_discard_steps(
        registry,
        "focus_the_mind",
        &focus.face_id,
        AuthoredDrawDiscardExpectation {
            first_instruction: 0,
            trailing_instructions: 0,
            who: PlayerRecipient::Controller,
            draw_count: 3,
            discard_count: 1,
            order: AuthoredDrawDiscardOrder::DrawThenDiscard,
        },
    );
}
