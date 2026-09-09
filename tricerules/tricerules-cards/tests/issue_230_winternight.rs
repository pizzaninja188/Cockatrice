use tricerules_cards::primitives::{Amount, CardTypeFilter, DiscardQuantity, SpellEffectKind};
use tricerules_cards::CardRegistry;

#[test]
fn issue_230_winternight_stories_has_both_complete_oracle_instructions_and_harmonize() {
    let definition = CardRegistry::global().get("winternight_stories").unwrap();
    let face = definition.primary_face();
    assert_eq!(definition.name, "Winternight Stories");
    assert_eq!(face.mana_cost.to_string(), "{2}{U}");
    assert_eq!(face.types, ["Sorcery"]);
    assert_eq!(
        face.harmonize_cost.as_ref().map(ToString::to_string),
        Some("{4}{U}".into())
    );
    assert!(matches!(face.spell_effect.as_slice(), [
        SpellEffectKind::Draw { count: Amount::Fixed(3), .. },
        SpellEffectKind::Discard { quantity: DiscardQuantity::UnlessOne { count: 2, filter }, .. },
    ] if filter.card_type == Some(CardTypeFilter::Creature)));
}
