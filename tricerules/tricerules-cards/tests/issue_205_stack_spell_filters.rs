use tricerules_cards::primitives::{SpellEffectKind, StackSpellFilter};

fn counter_filter(card_id: &str) -> &StackSpellFilter {
    let face = tricerules_cards::registry::global()
        .get(card_id)
        .unwrap()
        .primary_face();
    let [SpellEffectKind::CounterTargetSpell { spell_filter, .. }] = face.spell_effect.as_slice()
    else {
        panic!("{card_id} must have one counter-spell effect")
    };
    spell_filter
}

#[test]
fn issue_205_cards_author_exact_and_minimum_mana_value_filters() {
    assert_eq!(
        counter_filter("spell_snare"),
        &StackSpellFilter {
            min_mana_value: Some(2),
            max_mana_value: Some(2),
            ..Default::default()
        }
    );
    assert_eq!(
        counter_filter("disdainful_stroke"),
        &StackSpellFilter {
            min_mana_value: Some(4),
            ..Default::default()
        }
    );
}
