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
    let item = cx.top.clone();
    let engine = &mut *cx.engine;
    let events = &mut *cx.events;
    let controller = cx.controller;
    let spell_label = cx.spell_label;

    if engine.create_tokens(
        TokenCreationRequest {
            token_id: &token,
            count,
            recipients: who,
            spell_controller: controller,
            spell_label,
            item: &item,
        },
        events,
    )? {
        return Ok(EffectOutcome::Suspended);
    }

    Ok(EffectOutcome::Continue)
}
