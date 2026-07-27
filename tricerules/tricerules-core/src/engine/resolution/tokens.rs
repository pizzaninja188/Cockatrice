use super::*;

pub(super) fn create_tokens(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    let SpellEffectKind::CreateTokens {
        token,
        count,
        controller: who,
    } = effect
    else {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    };
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let controller = cx.controller;
    let spell_label = cx.spell_label;

    engine.create_tokens(&token, count, who, controller, spell_label, events);

    Ok(EffectOutcome::Continue)
}
