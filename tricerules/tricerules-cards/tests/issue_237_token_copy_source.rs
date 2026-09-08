use tricerules_cards::primitives::{EffectContext, SpellEffectKind, TargetSchema};

#[test]
fn issue_237_source_copy_is_untargeted_and_requires_an_ability() {
    let effect: SpellEffectKind =
        ron::from_str("CreateTokenCopies(count: 1, source: Source)").unwrap();
    assert!(effect.validate(EffectContext::Ability).is_ok());
    assert!(effect.validate(EffectContext::Spell).is_err());
    assert!(!TargetSchema::compile(&[effect], None)
        .unwrap()
        .has_targets());
}
