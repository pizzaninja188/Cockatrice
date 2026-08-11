use tricerules_cards::primitives::{
    EffectSubject, PlayerRecipient, SpellEffectKind, TargetFilter, TargetKind,
};
use tricerules_cards::{AbilityCost, Amount, CardRegistry, Keyword, TriggerCondition};

fn assert_tap_and_mana(costs: &[AbilityCost], expected: &str) {
    match costs {
        [AbilityCost::Mana(mana), AbilityCost::Tap] => assert_eq!(mana.to_string(), expected),
        other => panic!("expected [Mana({expected}), Tap], got {other:?}"),
    }
}

#[test]
fn pegasus_courser_has_complete_oracle_behavior() {
    let definition = CardRegistry::global()
        .get("pegasus_courser")
        .expect("Pegasus Courser must be registered");
    let face = definition.primary_face();

    assert_eq!(definition.name, "Pegasus Courser");
    assert_eq!(face.mana_cost.to_string(), "{2}{W}");
    assert_eq!(face.types, ["Creature", "Pegasus"]);
    assert_eq!((face.power, face.toughness), (Some(1), Some(3)));
    assert_eq!(face.keywords, [Keyword::Flying]);
    assert!(definition.partial.is_none());
    assert_eq!(face.triggered_abilities.len(), 1);
    assert_eq!(
        face.triggered_abilities[0].trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0
        }
    );
    assert_eq!(
        face.triggered_abilities[0].effect,
        [SpellEffectKind::GrantKeywords {
            subject: EffectSubject::Chosen(TargetFilter {
                kind: TargetKind::Creature,
                attacking_or_blocking: true,
                exclude_source: true,
                ..TargetFilter::default()
            }),
            keywords: vec![Keyword::Flying],
        }]
    );
}

#[test]
fn legion_guildmage_has_complete_oracle_behavior() {
    let definition = CardRegistry::global()
        .get("legion_guildmage")
        .expect("Legion Guildmage must be registered");
    let face = definition.primary_face();

    assert_eq!(definition.name, "Legion Guildmage");
    assert_eq!(face.mana_cost.to_string(), "{R}{W}");
    assert_eq!(face.types, ["Creature", "Human", "Wizard"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(2)));
    assert!(definition.partial.is_none());
    assert_eq!(face.activated_abilities.len(), 2);

    assert_tap_and_mana(&face.activated_abilities[0].costs, "{5}{R}");
    assert_eq!(
        face.activated_abilities[0].effect,
        [SpellEffectKind::DamagePlayer {
            amount: Amount::Fixed(3),
            who: PlayerRecipient::EachOpponent,
        }]
    );

    assert_tap_and_mana(&face.activated_abilities[1].costs, "{2}{W}");
    assert_eq!(
        face.activated_abilities[1].effect,
        [SpellEffectKind::TapTarget {
            target: TargetFilter {
                kind: TargetKind::Creature,
                exclude_source: true,
                ..TargetFilter::default()
            },
        }]
    );
}
