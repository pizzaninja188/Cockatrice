use tricerules_cards::primitives::{
    Amount, EffectContext, EffectSubject, GameCondition, SpellEffectKind, SpellManaSpentComparison,
};
use tricerules_cards::{CardRegistry, CounterKind};

#[test]
fn issue_181_cards_use_the_shared_spell_spending_context() {
    let tackle = CardRegistry::global()
        .get("tackle_artist")
        .expect("Tackle Artist must be registered")
        .primary_face();
    let [tackle_trigger] = tackle.triggered_abilities.as_slice() else {
        panic!("Tackle Artist has one triggered ability");
    };
    assert!(matches!(
        tackle_trigger.effect.as_slice(),
        [SpellEffectKind::PutCounters {
            counter: CounterKind::PlusOnePlusOne,
            count: Amount::Conditional {
                condition: GameCondition::TriggeringSpellManaSpent {
                    comparison: SpellManaSpentComparison::AtLeast(5),
                },
                when_true: 2,
                otherwise: 1,
            },
            subject: EffectSubject::Source,
        }]
    ));

    let graffalon = CardRegistry::global()
        .get("hungry_graffalon")
        .expect("Hungry Graffalon must be registered")
        .primary_face();
    let [increment] = graffalon.triggered_abilities.as_slice() else {
        panic!("Hungry Graffalon has one triggered ability");
    };
    assert!(matches!(
        increment.intervening_if,
        Some(GameCondition::TriggeringSpellManaSpent {
            comparison: SpellManaSpentComparison::GreaterThanSourcePowerOrToughness,
        })
    ));
}

#[test]
fn put_counters_preserves_bare_integer_ron_and_accepts_conditional_amounts() {
    let fixed: SpellEffectKind =
        ron::from_str("PutCounters(counter: PlusOnePlusOne, count: 1, subject: Source)")
            .expect("legacy fixed counter count");
    assert!(matches!(
        fixed,
        SpellEffectKind::PutCounters {
            count: Amount::Fixed(1),
            ..
        }
    ));

    let conditional: SpellEffectKind = ron::from_str(
        "PutCounters(counter: PlusOnePlusOne, count: Conditional(condition: TriggeringSpellManaSpent(comparison: AtLeast(5)), when_true: 2, otherwise: 1), subject: Source)",
    )
    .expect("conditional counter count");
    assert!(matches!(
        conditional,
        SpellEffectKind::PutCounters {
            count: Amount::Conditional { .. },
            ..
        }
    ));
}

#[test]
fn triggering_spell_spending_is_rejected_without_a_cast_trigger() {
    let enters = r#"(
        id: "bad_increment_etb",
        name: "Bad Increment ETB",
        face_id: "bad_increment_etb",
        types: ["Creature"],
        power: 1,
        toughness: 1,
        triggered_abilities: [(
            ability_id: "triggered_01",
            presentation: Fallback,
            trigger: WhenSelfEntersBattlefield,
            effect: [GainLife(amount: Conditional(
                condition: TriggeringSpellManaSpent(comparison: AtLeast(1)),
                when_true: 2,
                otherwise: 1,
            ))],
        )],
    )"#;
    assert!(CardRegistry::from_chunks_and_tokens(&[enters], &[]).is_err());

    let activated = r#"(
        id: "bad_increment_activation",
        name: "Bad Increment Activation",
        face_id: "bad_increment_activation",
        types: ["Creature"],
        power: 1,
        toughness: 1,
        activated_abilities: [(
            ability_id: "activated_01",
            presentation: Fallback,
            costs: [Tap],
            effect: [GainLife(amount: Conditional(
                condition: TriggeringSpellManaSpent(comparison: AtLeast(1)),
                when_true: 2,
                otherwise: 1,
            ))],
        )],
    )"#;
    assert!(CardRegistry::from_chunks_and_tokens(&[activated], &[]).is_err());

    let spell: SpellEffectKind = ron::from_str(
        "GainLife(amount: Conditional(condition: TriggeringSpellManaSpent(comparison: AtLeast(1)), when_true: 2, otherwise: 1))",
    )
    .expect("typed effect parses before contextual validation");
    assert!(spell.validate(EffectContext::Spell).is_err());
}
