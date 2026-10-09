use super::events::ev_log;
use super::{mass, EffectCx, EffectOutcome};
use crate::EngineError;
use rand::{Rng, SeedableRng};
use tricerules_card_model::primitives::{
    PermanentTypeFilter, SpellEffectKind, TargetFilter, TargetKind,
};

const BOOMPILE_RANDOM_DOMAIN: u64 = 0x424F_4F4D_5049_4C45;

pub(super) fn flip_coin_and_destroy_nonlands(
    cx: &mut EffectCx<'_>,
    effect: SpellEffectKind,
) -> Result<EffectOutcome, EngineError> {
    if !matches!(effect, SpellEffectKind::BoompileFlipCoinAndDestroyNonlands) {
        return Err(EngineError::Illegal("resolution dispatch mismatch"));
    }
    let source = cx.top.source_permanent_id.ok_or(EngineError::Illegal(
        "Boompile coin flip has no source object",
    ))?;
    let mix = cx.engine.state.seed
        ^ cx.engine
            .state
            .command_index
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ u64::from(source).wrapping_mul(0xD1B5_4A32_D192_ED03)
        ^ (cx.controller as u64).rotate_left(32)
        ^ BOOMPILE_RANDOM_DOMAIN;
    let mut rng = rand::rngs::StdRng::seed_from_u64(mix);
    let won = rng.gen::<bool>();
    cx.events.push(ev_log(format!(
        "P{} {} the Boompile coin flip.",
        cx.controller,
        if won { "wins" } else { "loses" }
    )));
    if !won {
        return Ok(EffectOutcome::Continue);
    }

    mass::destroy_all(
        cx,
        SpellEffectKind::DestroyAll {
            kind: TargetFilter {
                kind: TargetKind::AnyPermanent,
                excluded_permanent_types: vec![PermanentTypeFilter::Land],
                ..Default::default()
            },
            prevent_regeneration: false,
        },
    )
}
