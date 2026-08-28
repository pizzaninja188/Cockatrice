use tricerules_cards::primitives::{
    Amount, BattlefieldCreatureCountFilter, CountExpression, EffectSubject, Keyword,
    PlayerRecipient, PtScale, RelativePlayerSet, SpellEffectKind, TargetFilter, TargetKind,
    TriggerCondition,
};
use tricerules_cards::CardRegistry;

fn creature_target() -> TargetFilter {
    TargetFilter {
        kind: TargetKind::Creature,
        ..TargetFilter::default()
    }
}

#[test]
fn issue_84_cards_have_complete_oracle_characteristics_and_shared_amounts() {
    let registry = CardRegistry::global();
    for (id, name, mana_cost, types, power, toughness) in [
        (
            "aerial_assault",
            "Aerial Assault",
            "{2}{W}",
            &["Sorcery"][..],
            None,
            None,
        ),
        (
            "growth_cycle",
            "Growth Cycle",
            "{1}{G}",
            &["Instant"][..],
            None,
            None,
        ),
        (
            "lavakin_brawler",
            "Lavakin Brawler",
            "{3}{R}",
            &["Creature", "Elemental", "Warrior"][..],
            Some(2),
            Some(4),
        ),
        (
            "undead_servant",
            "Undead Servant",
            "{3}{B}",
            &["Creature", "Zombie"][..],
            Some(3),
            Some(2),
        ),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing Issue #84 card {id}"));
        assert_eq!(definition.name, name, "{id}");
        assert!(definition.partial.is_none(), "{id} must be complete");
        let face = definition.primary_face();
        assert_eq!(face.mana_cost.to_string(), mana_cost, "{id}");
        assert_eq!(
            face.types.iter().map(String::as_str).collect::<Vec<_>>(),
            types,
            "{id}"
        );
        assert_eq!(face.power, power, "{id}");
        assert_eq!(face.toughness, toughness, "{id}");
    }

    let aerial = registry.get("aerial_assault").unwrap().primary_face();
    assert_eq!(
        aerial.spell_effect,
        [
            SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(Box::new(TargetFilter {
                    kind: TargetKind::Creature,
                    tapped: Some(true),
                    ..TargetFilter::default()
                })),
            },
            SpellEffectKind::GainLife {
                amount: Amount::Count(CountExpression::BattlefieldCreatures {
                    filter: BattlefieldCreatureCountFilter {
                        controllers: RelativePlayerSet::Controller,
                        subtype: None,
                        required_keywords: vec![Keyword::Flying],
                        tapped: None,
                        requires_any_counter: false,
                        required_counter: None,
                        exclude_source: false,
                    },
                }),
            },
        ]
    );

    let growth = registry.get("growth_cycle").unwrap().primary_face();
    let same_growths = CountExpression::GraveyardCardsNamed {
        owners: RelativePlayerSet::Controller,
        name: "Growth Cycle".into(),
    };
    assert_eq!(
        growth.spell_effect,
        [SpellEffectKind::PumpTarget {
            power: 3,
            toughness: 3,
            scale: Some(PtScale {
                amount: Amount::Count(same_growths),
                power_per_unit: 2,
                toughness_per_unit: 2,
            }),
            subject: EffectSubject::Chosen(Box::new(creature_target())),
        }]
    );

    let brawler = registry.get("lavakin_brawler").unwrap().primary_face();
    assert_eq!(brawler.triggered_abilities.len(), 1);
    assert_eq!(
        brawler.triggered_abilities[0].trigger,
        TriggerCondition::WheneverSelfAttacks {
            minimum_other_attackers: 0
        }
    );
    assert_eq!(
        brawler.triggered_abilities[0].effect,
        [SpellEffectKind::PumpTarget {
            power: 0,
            toughness: 0,
            scale: Some(PtScale {
                amount: Amount::Count(CountExpression::BattlefieldCreatures {
                    filter: BattlefieldCreatureCountFilter {
                        controllers: RelativePlayerSet::Controller,
                        subtype: Some("Elemental".into()),
                        required_keywords: vec![],
                        tapped: None,
                        requires_any_counter: false,
                        required_counter: None,
                        exclude_source: false,
                    },
                }),
                power_per_unit: 1,
                toughness_per_unit: 0,
            }),
            subject: EffectSubject::Source,
        }]
    );

    let servant = registry.get("undead_servant").unwrap().primary_face();
    assert_eq!(servant.triggered_abilities.len(), 1);
    assert_eq!(
        servant.triggered_abilities[0].trigger,
        TriggerCondition::WhenSelfEntersBattlefield
    );
    assert_eq!(
        servant.triggered_abilities[0].effect,
        [SpellEffectKind::CreateTokens {
            token: "zombie_b_2_2".into(),
            count: Amount::Count(CountExpression::GraveyardCardsNamed {
                owners: RelativePlayerSet::Controller,
                name: "Undead Servant".into(),
            }),
            who: PlayerRecipient::Controller,
            tapped: false,
            sacrifice_timing: None,
        }]
    );
}
