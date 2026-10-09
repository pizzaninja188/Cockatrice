use tricerules_cards::primitives::{
    AbilityCost, Amount, AttackGroupDestination, AttackGroupTriggerTiming, EffectSubject,
    PlayerRecipient, SpellEffectKind, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, ManaAmount};

#[test]
fn coveted_jewel_is_registered_with_its_complete_rules_text() {
    let card = tricerules_cards::registry::global()
        .get("coveted_jewel")
        .expect("Coveted Jewel must have a complete ruled-card definition");
    assert_eq!(card.name, "Coveted Jewel");
    let face = card.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{6}");
    assert_eq!(face.types, ["Artifact"]);
    assert!(face.spell_effect.is_empty());

    let [mana_ability] = face.activated_abilities.as_slice() else {
        panic!("Coveted Jewel has exactly one mana ability");
    };
    assert_eq!(
        mana_ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(mana_ability.costs, [AbilityCost::Tap]);
    let [SpellEffectKind::ProduceMana { options, .. }] = mana_ability.effect.as_slice() else {
        panic!("Coveted Jewel's ability adds three mana of one chosen color");
    };
    assert_eq!(
        options,
        &[
            ManaAmount {
                w: 3,
                ..Default::default()
            },
            ManaAmount {
                u: 3,
                ..Default::default()
            },
            ManaAmount {
                b: 3,
                ..Default::default()
            },
            ManaAmount {
                r: 3,
                ..Default::default()
            },
            ManaAmount {
                g: 3,
                ..Default::default()
            },
        ]
    );

    let [entry, attack] = face.triggered_abilities.as_slice() else {
        panic!("Coveted Jewel has an entry trigger and an unblocked attack trigger");
    };
    assert_eq!(
        entry.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(entry.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        entry.effect,
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(3),
        }]
    );
    assert_eq!(
        attack.presentation,
        AbilityPresentation::OracleLines(vec![3])
    );
    assert_eq!(
        attack.trigger,
        TriggerCondition::WheneverOpponentAttackGroup {
            timing: AttackGroupTriggerTiming::AfterBlockersDeclared,
            destination: AttackGroupDestination::Controller,
        }
    );
    assert_eq!(
        attack.effect,
        [
            SpellEffectKind::Draw {
                who: PlayerRecipient::TriggeringAttackingPlayer,
                count: Amount::Fixed(3),
            },
            SpellEffectKind::GiveControlOfSourceToAttackingPlayer,
            SpellEffectKind::Untap {
                subject: EffectSubject::Source,
            },
        ]
    );
    assert!(attack.targeting.is_none());
}
