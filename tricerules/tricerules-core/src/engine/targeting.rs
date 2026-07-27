use super::combat::is_attacking_or_blocking;
use super::*;
use tricerules_cards::primitives::{GraveyardCardType, GraveyardFilter, GraveyardOwner};

/// Player or creature permanent on the battlefield (matches cast validation for `bolt`).
fn damage_spell_target_legal(engine: &GameEngine, tid: ObjectId) -> bool {
    if engine.state.player_idx(tid as i32).is_some() {
        return true;
    }
    engine
        .state
        .objects
        .get(&tid)
        .is_some_and(|o| o.zone == Zone::Battlefield)
        && engine
            .characteristics(tid)
            .is_some_and(|value| value.is_creature())
}

fn destroy_spell_target_legal(engine: &GameEngine, tid: ObjectId) -> bool {
    engine
        .state
        .objects
        .get(&tid)
        .is_some_and(|o| o.zone == Zone::Battlefield)
        && engine
            .characteristics(tid)
            .is_some_and(|value| value.is_creature())
}

/// Target must be an active player (not lost).
fn player_target_legal(state: &GameState, tid: ObjectId) -> bool {
    state
        .player_idx(tid as i32)
        .is_some_and(|pi| !state.players[pi].has_lost)
}

/// Any battlefield permanent (creature, land, etc.) — for broad bounce like Boomerang.
fn any_battlefield_permanent_target_legal(state: &GameState, tid: ObjectId) -> bool {
    state
        .objects
        .get(&tid)
        .is_some_and(|o| o.zone == Zone::Battlefield)
}

/// Check if `oid` is a legal graveyard target for a [`GraveyardFilter`].
/// Graveyard cards have no Hexproof/Shroud (those keywords only apply on the battlefield).
pub(super) fn graveyard_target_legal(
    engine: &GameEngine,
    filter: &GraveyardFilter,
    oid: ObjectId,
    caster: PlayerId,
) -> bool {
    let Some(obj) = engine.state.objects.get(&oid) else {
        return false;
    };
    if obj.zone != Zone::Graveyard {
        return false;
    }
    // Owner restriction: "your graveyard" vs. any player's graveyard.
    match filter.owner {
        GraveyardOwner::Controller => {
            if obj.owner != caster {
                return false;
            }
        }
        GraveyardOwner::AnyPlayer => {}
    }
    // Card-type restriction.
    if let Some(card_type) = filter.card_type {
        let Some(def) = engine.registry.get(&obj.card_id) else {
            return false;
        };
        match card_type {
            GraveyardCardType::Creature => {
                // A card is a creature card if it has "Creature" in its type line on any face.
                if !def.any_face(|f| f.is_creature) {
                    return false;
                }
            }
        }
    }
    true
}

/// CR 702.16 / CR 702.18: returns false when `tid` is a permanent that the `caster` cannot
/// legally target due to Shroud or Hexproof. Players are never shielded by these keywords.
fn object_targetable_by(engine: &GameEngine, tid: ObjectId, caster: PlayerId) -> bool {
    let Some(_obj) = engine.state.objects.get(&tid) else {
        return true; // object gone — legality checked elsewhere
    };
    let Some(characteristics) = engine.characteristics(tid) else {
        return true;
    };
    if characteristics.has_keyword(Keyword::Shroud) {
        return false;
    }
    if characteristics.has_keyword(Keyword::Hexproof) && characteristics.controller != caster {
        return false;
    }
    true
}

/// Legality of a single target against a [`TargetFilter`].
/// `caster` is needed only to enforce the opponent-only restriction.
/// True if `oid` is a battlefield permanent selected by a mass effect's `kind` filter
/// (DestroyAll / DamageAll). Unlike [`target_filter_legal`] this is **not** targeting: it
/// ignores hexproof/shroud (CR 702.11e — untargeted effects affect them normally) and only
/// honors the object kinds and characteristic constraints the filter carries.
pub(super) fn object_matches_mass_filter(
    engine: &GameEngine,
    oid: ObjectId,
    filter: &TargetFilter,
) -> bool {
    let Some(o) = engine.state.objects.get(&oid) else {
        return false;
    };
    if o.zone != Zone::Battlefield {
        return false;
    }
    let Some(characteristics) = engine.characteristics(oid) else {
        return false;
    };
    let kind_ok = match filter.kind {
        TargetKind::Creature => characteristics.is_creature(),
        TargetKind::AnyPermanent => true,
        // Player / AnyTarget / Self_ kinds are rejected at registry load for mass effects.
        _ => false,
    };
    if !kind_ok {
        return false;
    }
    if filter.not_artifact && characteristics.is_artifact() {
        return false;
    }
    if let Some(tapped_req) = filter.tapped {
        if o.tapped != tapped_req {
            return false;
        }
    }
    true
}

/// Collect every battlefield permanent matching a mass-effect filter, in deterministic
/// player-then-battlefield order (no HashMap iteration, so replays stay reproducible).
pub(super) fn battlefield_objects_matching(
    engine: &GameEngine,
    filter: &TargetFilter,
) -> Vec<ObjectId> {
    let mut out = Vec::new();
    for p in &engine.state.players {
        for &oid in &p.battlefield {
            if object_matches_mass_filter(engine, oid, filter) {
                out.push(oid);
            }
        }
    }
    out
}

/// Public wrapper used by resolution for per-target legality checks in `DamageTargets`.
pub(super) fn target_filter_legal_at_resolution(
    engine: &GameEngine,
    filter: &TargetFilter,
    tid: ObjectId,
    caster: PlayerId,
) -> bool {
    target_filter_legal(engine, filter, tid, caster)
}

fn target_filter_legal(
    engine: &GameEngine,
    filter: &TargetFilter,
    tid: ObjectId,
    caster: PlayerId,
) -> bool {
    let kind_ok = match filter.kind {
        TargetKind::AnyTarget => damage_spell_target_legal(engine, tid),
        TargetKind::Creature => destroy_spell_target_legal(engine, tid),
        TargetKind::AnyPlayer => player_target_legal(&engine.state, tid),
        TargetKind::OpponentPlayer => {
            player_target_legal(&engine.state, tid) && tid as i32 != caster
        }
        TargetKind::AnyPermanent => any_battlefield_permanent_target_legal(&engine.state, tid),
        // `Self_` is auto-bound to the ability's source, never a chosen target (CR 115), so it
        // is never legal to *pick*. The engine binds it directly at resolution.
        TargetKind::Self_ => false,
    };
    if !kind_ok {
        return false;
    }
    // Characteristic filters — only apply to non-player targets.
    if !filter.is_player() {
        if !object_targetable_by(engine, tid, caster) {
            return false;
        }
        if filter.not_artifact
            && engine
                .characteristics(tid)
                .is_some_and(|value| value.is_artifact())
        {
            return false;
        }
        if let Some(tapped_req) = filter.tapped {
            match engine.state.objects.get(&tid) {
                Some(obj) if obj.tapped != tapped_req => return false,
                None => return false,
                _ => {}
            }
        }
        // CR 105/202.2: "nonblack", "nonwhite", … — reject a target of the excluded color.
        if let Some(c) = filter.not_color {
            match engine.characteristics(tid) {
                Some(value) if value.colors.contains(&c) => return false,
                None => return false,
                Some(_) => {}
            }
        }
        // CR 508/509: "target attacking or blocking creature" — must be in combat right now.
        if filter.attacking_or_blocking && !is_attacking_or_blocking(&engine.state, tid) {
            return false;
        }
        // "target creature you control" (equip, regenerate, …). Ownership == control until
        // control-changing effects exist (CR 109.4).
        if filter.only_controller {
            if let Some(value) = engine.characteristics(tid) {
                if value.controller != caster {
                    return false;
                }
            } else {
                return false;
            }
        }
    }
    true
}

/// CR 701.5/707.10: legality of `tid` as the target of a counter/copy spell. The object must be a
/// spell on the stack (not an activated/triggered ability), and — when `spell_filter` is `Some` —
/// must be a spell of that type (Essence Scatter = Creature, Negate = Noncreature, Twincast =
/// InstantOrSorcery). `None` accepts any spell (Counterspell).
fn stack_spell_target_legal(
    state: &GameState,
    registry: &CardRegistry,
    tid: ObjectId,
    spell_filter: Option<SpellTypeFilter>,
) -> bool {
    let Some(item) = state
        .stack
        .iter()
        .find(|s| s.id == tid && s.ability_text.is_none())
    else {
        return false;
    };
    let Some(filter) = spell_filter else {
        return true;
    };
    let Some(face) = registry
        .get(&item.card_id)
        .and_then(|d| d.face(item.face_index))
    else {
        return false;
    };
    match filter {
        SpellTypeFilter::Creature => face.is_creature,
        SpellTypeFilter::Instant => face.is_instant,
        SpellTypeFilter::Sorcery => face.is_sorcery,
        SpellTypeFilter::InstantOrSorcery => face.is_instant || face.is_sorcery,
        SpellTypeFilter::Enchantment => face.is_enchantment,
        SpellTypeFilter::Artifact => face.is_artifact,
        SpellTypeFilter::Noncreature => !face.is_creature,
    }
}

/// CR 608.2b-style: if any targeted effect has no legal target, the whole spell fizzles.
/// For `DamageTargets`, the spell fizzles only when ALL chosen targets are gone (partial-fizzle
/// per CR 608.2b — if at least one is still legal, the spell still resolves for it).
pub(super) fn spell_has_no_legal_targets_at_resolution(
    engine: &GameEngine,
    effects: &[SpellEffectKind],
    targets: &[ObjectId],
    caster: PlayerId,
) -> bool {
    effects.iter().any(|effect| {
        if !spell_effect_kind_needs_target(effect) {
            return false; // untargeted effects never fizzle
        }
        // DamageTargets: spell fizzles only if ALL targets are now illegal.
        if matches!(effect, SpellEffectKind::DamageTargets { .. }) {
            if targets.is_empty() {
                return true;
            }
            return targets
                .iter()
                .all(|&tid| !effect_target_legal_at_resolution(engine, effect, tid, caster));
        }
        let Some(&tid) = targets.first() else {
            return true; // needs target but none provided
        };
        !effect_target_legal_at_resolution(engine, effect, tid, caster)
    })
}

/// Returns true if `tid` is a legal target for `effect` at resolution time.
fn effect_target_legal_at_resolution(
    engine: &GameEngine,
    effect: &SpellEffectKind,
    tid: ObjectId,
    caster: PlayerId,
) -> bool {
    match effect {
        SpellEffectKind::DamageTarget { target, .. }
        | SpellEffectKind::DamageTargets { target, .. }
        | SpellEffectKind::TargetPlayerGainsLife { target, .. }
        | SpellEffectKind::TargetPlayerLosesLife { target, .. }
        | SpellEffectKind::MillTargetPlayer { target, .. }
        | SpellEffectKind::TapTarget { target } => target_filter_legal(engine, target, tid, caster),
        SpellEffectKind::DestroyTarget { target }
        | SpellEffectKind::PumpTarget { target, .. }
        | SpellEffectKind::PutCounters { target, .. }
        | SpellEffectKind::Equip { target }
        | SpellEffectKind::Regenerate { target } => {
            target_filter_legal(engine, target, tid, caster)
        }
        SpellEffectKind::ExileTarget
        | SpellEffectKind::ExileTargetGainLifeEqualToPower
        | SpellEffectKind::ReturnTargetCreatureToHand => {
            destroy_spell_target_legal(engine, tid) && object_targetable_by(engine, tid, caster)
        }
        SpellEffectKind::ReturnTargetPermanentToHand => {
            any_battlefield_permanent_target_legal(&engine.state, tid)
                && object_targetable_by(engine, tid, caster)
        }
        // CR 115.2 / 707.10b: counter and copy effects target *spells* on the stack, not
        // activated/triggered abilities. The optional `spell_filter` further restricts which
        // spell types are legal (Essence Scatter, Negate, Twincast).
        SpellEffectKind::CounterTargetSpell { spell_filter } => {
            stack_spell_target_legal(&engine.state, engine.registry, tid, *spell_filter)
        }
        SpellEffectKind::CopyTargetSpell { spell_filter, .. } => {
            stack_spell_target_legal(&engine.state, engine.registry, tid, *spell_filter)
        }
        SpellEffectKind::AuraAttach { target } => target_filter_legal(engine, target, tid, caster),
        SpellEffectKind::ReturnFromGraveyard { filter, .. } => {
            graveyard_target_legal(engine, filter, tid, caster)
        }
        _ => true,
    }
}

pub(super) fn spell_effect_kind_needs_target(kind: &SpellEffectKind) -> bool {
    match kind {
        // A `Self_`-filtered pump or counter-placement is auto-bound to its source (CR 115) — it
        // takes no chosen target and prompts nobody; any other filter requires a selected target.
        SpellEffectKind::PumpTarget { target, .. }
        | SpellEffectKind::PutCounters { target, .. }
        | SpellEffectKind::Regenerate { target } => !matches!(target.kind, TargetKind::Self_),
        SpellEffectKind::DamageTarget { .. }
        | SpellEffectKind::DamageTargets { .. }
        | SpellEffectKind::DestroyTarget { .. }
        | SpellEffectKind::ExileTarget
        | SpellEffectKind::ExileTargetGainLifeEqualToPower
        | SpellEffectKind::ReturnTargetCreatureToHand
        | SpellEffectKind::ReturnTargetPermanentToHand
        | SpellEffectKind::ReturnFromGraveyard { .. }
        | SpellEffectKind::TargetPlayerGainsLife { .. }
        | SpellEffectKind::TargetPlayerLosesLife { .. }
        | SpellEffectKind::DrainTarget { .. }
        | SpellEffectKind::MillTargetPlayer { .. }
        | SpellEffectKind::DiscardCards { .. }
        | SpellEffectKind::TapTarget { .. }
        | SpellEffectKind::CounterTargetSpell { .. }
        | SpellEffectKind::CopyTargetSpell { .. }
        | SpellEffectKind::AuraAttach { .. }
        // CR 702.6a: equip targets "target creature you control" — always targeted.
        | SpellEffectKind::Equip { .. }
        | SpellEffectKind::TargetPlayerSacrifices { .. }
        | SpellEffectKind::PreventNextDamage { .. } => true,
        _ => false,
    }
}

/// Validate targets for a `SpellEffectKind` directly (used by ability activation/trigger target selection).
pub(super) fn validate_effect_targets(
    engine: &GameEngine,
    caster: PlayerId,
    effect: &SpellEffectKind,
    targets: &[rv1::TargetRef],
) -> Result<(), EngineError> {
    match effect {
        SpellEffectKind::DestroyTarget { target: filter } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one target"));
            }
            if !target_filter_legal(engine, filter, targets[0].object_id, caster) {
                return Err(EngineError::Illegal(
                    "target must be a creature on the battlefield",
                ));
            }
        }
        SpellEffectKind::TapTarget { target: filter } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one target"));
            }
            if !target_filter_legal(engine, filter, targets[0].object_id, caster) {
                return Err(EngineError::Illegal(
                    "target must be a permanent on the battlefield",
                ));
            }
        }
        SpellEffectKind::DamageTarget { target: filter, .. } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one target"));
            }
            if !target_filter_legal(engine, filter, targets[0].object_id, caster) {
                return Err(EngineError::Illegal("illegal target for damage effect"));
            }
        }
        SpellEffectKind::DamageTargets { target: filter, max_targets, .. } => {
            if targets.is_empty() {
                return Err(EngineError::Illegal("requires at least one target"));
            }
            if let Some(max) = max_targets {
                if targets.len() > *max as usize {
                    return Err(EngineError::Illegal("too many targets for this effect"));
                }
            }
            let mut seen = std::collections::HashSet::new();
            for t in targets {
                if !seen.insert(t.object_id) {
                    return Err(EngineError::Illegal("duplicate target"));
                }
                if !target_filter_legal(engine, filter, t.object_id, caster) {
                    return Err(EngineError::Illegal("illegal target for damage effect"));
                }
            }
        }
        SpellEffectKind::PumpTarget { target: filter, .. }
        | SpellEffectKind::PutCounters { target: filter, .. }
        | SpellEffectKind::Regenerate { target: filter } => {
            // `Self_` pumps / counter placements / regen are auto-bound and take no chosen target.
            if matches!(filter.kind, TargetKind::Self_) {
                if !targets.is_empty() {
                    return Err(EngineError::Illegal("this effect takes no targets"));
                }
            } else {
                if targets.len() != 1 {
                    return Err(EngineError::Illegal("requires exactly one target"));
                }
                if !target_filter_legal(engine, filter, targets[0].object_id, caster) {
                    return Err(EngineError::Illegal(
                        "target must be a creature on the battlefield",
                    ));
                }
            }
        }
        SpellEffectKind::ExileTarget
        | SpellEffectKind::ExileTargetGainLifeEqualToPower
        | SpellEffectKind::ReturnTargetCreatureToHand => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one creature target"));
            }
            if !destroy_spell_target_legal(engine, targets[0].object_id) {
                return Err(EngineError::Illegal(
                    "target must be a creature on the battlefield",
                ));
            }
            if !object_targetable_by(engine, targets[0].object_id, caster) {
                return Err(EngineError::Illegal("target has hexproof or shroud"));
            }
        }
        SpellEffectKind::ReturnTargetPermanentToHand => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal(
                    "requires exactly one permanent target",
                ));
            }
            if !any_battlefield_permanent_target_legal(&engine.state, targets[0].object_id) {
                return Err(EngineError::Illegal(
                    "target must be a permanent on the battlefield",
                ));
            }
            if !object_targetable_by(engine, targets[0].object_id, caster) {
                return Err(EngineError::Illegal("target has hexproof or shroud"));
            }
        }
        SpellEffectKind::TargetPlayerGainsLife { target: filter, .. }
        | SpellEffectKind::TargetPlayerLosesLife { target: filter, .. }
        | SpellEffectKind::DrainTarget { target: filter, .. }
        | SpellEffectKind::MillTargetPlayer { target: filter, .. }
        | SpellEffectKind::DiscardCards { target: filter, .. }
        | SpellEffectKind::TargetPlayerSacrifices { target: filter, .. } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one player target"));
            }
            if !target_filter_legal(engine, filter, targets[0].object_id, caster) {
                return Err(EngineError::Illegal("target must be a player in the game"));
            }
            if matches!(filter.kind, TargetKind::OpponentPlayer)
                && targets[0].object_id as i32 == caster
            {
                return Err(EngineError::Illegal("cannot target yourself"));
            }
        }
        SpellEffectKind::AuraAttach { target: filter } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("aura requires exactly one enchant target"));
            }
            if !target_filter_legal(engine, filter, targets[0].object_id, caster) {
                return Err(EngineError::Illegal(
                    "enchant target must be a valid permanent on the battlefield",
                ));
            }
        }
        SpellEffectKind::Equip { target: filter } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one target"));
            }
            if !target_filter_legal(engine, filter, targets[0].object_id, caster) {
                return Err(EngineError::Illegal(
                    "equip target must be a creature you control on the battlefield",
                ));
            }
        }
        SpellEffectKind::PreventNextDamage { target: filter, .. } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one target"));
            }
            if !target_filter_legal(engine, filter, targets[0].object_id, caster) {
                return Err(EngineError::Illegal("illegal target for damage prevention"));
            }
        }
        SpellEffectKind::ReturnFromGraveyard { filter, .. } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one graveyard card target"));
            }
            if !graveyard_target_legal(engine, filter, targets[0].object_id, caster) {
                return Err(EngineError::Illegal(
                    "target must be a matching card in the correct graveyard",
                ));
            }
        }
        // Non-targeted effects require no targets.
        SpellEffectKind::Draw { .. }
        | SpellEffectKind::GainLife { .. }
        | SpellEffectKind::EachOpponentLosesLifeYouGainEqual { .. }
        // Counter/copy target *spells*; they are never put on an ability, so this ability-only
        // validator only needs them present for exhaustiveness (spell targets go through
        // `spell_target_legality_error`).
        | SpellEffectKind::CounterTargetSpell { .. }
        | SpellEffectKind::CopyTargetSpell { .. }
        | SpellEffectKind::DestroyAll { .. }
        | SpellEffectKind::DamageAll { .. }
        | SpellEffectKind::PumpAll { .. }
        | SpellEffectKind::GrantKeywordsAll { .. }
        | SpellEffectKind::CreateTokens { .. }
        | SpellEffectKind::PreventAllCombatDamageTurn
        // CR 605.1a: a mana ability is untargeted by definition.
        | SpellEffectKind::ProduceMana { .. }
        // CR 701.18: library search is untargeted; the library card is chosen via a pending
        // interrupt, not a target declared at cast time.
        | SpellEffectKind::SearchLibrary { .. }
        | SpellEffectKind::None => {
            if !targets.is_empty() {
                return Err(EngineError::Illegal("this effect takes no targets"));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_spell_targets(
    engine: &GameEngine,
    caster: PlayerId,
    effects: &[SpellEffectKind],
    targets: &[rv1::TargetRef],
) -> Result<(), EngineError> {
    // DamageTargets needs multi-target validation (1..=max_targets) — handled via
    // validate_effect_targets, which already supports variable count.
    let has_multi_target = effects
        .iter()
        .any(|e| matches!(e, SpellEffectKind::DamageTargets { .. }));
    if has_multi_target {
        for effect in effects {
            if matches!(effect, SpellEffectKind::DamageTargets { .. }) {
                validate_effect_targets(engine, caster, effect, targets)?;
            }
        }
        return Ok(());
    }

    let needs_target = effects.iter().any(spell_effect_kind_needs_target);
    if needs_target {
        if targets.len() != 1 {
            return Err(EngineError::Illegal("spell requires exactly one target"));
        }
        let tid = targets[0].object_id;
        for effect in effects {
            if !spell_effect_kind_needs_target(effect) {
                continue;
            }
            spell_target_legality_error(engine, effect, tid, caster)?;
        }
    } else if !targets.is_empty() {
        return Err(EngineError::Illegal("this spell takes no targets"));
    }
    Ok(())
}

/// Returns `Err` with a specific human-readable message when `tid` is not a legal target for `effect`.
pub(super) fn spell_target_legality_error(
    engine: &GameEngine,
    effect: &SpellEffectKind,
    tid: ObjectId,
    caster: PlayerId,
) -> Result<(), EngineError> {
    match effect {
        // Filter-based targeted effects share one legality path; the filter carries any
        // characteristic restriction (creature/player, `tapped`, `not_artifact`, hexproof/shroud).
        SpellEffectKind::DestroyTarget { target: filter }
        | SpellEffectKind::DamageTarget { target: filter, .. }
        | SpellEffectKind::DamageTargets { target: filter, .. }
        | SpellEffectKind::TapTarget { target: filter }
        | SpellEffectKind::PumpTarget { target: filter, .. }
        | SpellEffectKind::PutCounters { target: filter, .. }
        | SpellEffectKind::PreventNextDamage { target: filter, .. }
            if !target_filter_legal(engine, filter, tid, caster) =>
        {
            return Err(EngineError::Illegal(
                "target must be a creature or player on the battlefield",
            ));
        }
        SpellEffectKind::ExileTarget
        | SpellEffectKind::ExileTargetGainLifeEqualToPower
        | SpellEffectKind::ReturnTargetCreatureToHand
            if !destroy_spell_target_legal(engine, tid) =>
        {
            return Err(EngineError::Illegal(
                "target must be a creature on the battlefield",
            ));
        }
        SpellEffectKind::ExileTarget
        | SpellEffectKind::ExileTargetGainLifeEqualToPower
        | SpellEffectKind::ReturnTargetCreatureToHand
            if !object_targetable_by(engine, tid, caster) =>
        {
            return Err(EngineError::Illegal("target has hexproof or shroud"));
        }
        SpellEffectKind::ReturnTargetPermanentToHand
            if !any_battlefield_permanent_target_legal(&engine.state, tid) =>
        {
            return Err(EngineError::Illegal(
                "target must be a permanent on the battlefield",
            ));
        }
        SpellEffectKind::ReturnTargetPermanentToHand
            if !object_targetable_by(engine, tid, caster) =>
        {
            return Err(EngineError::Illegal("target has hexproof or shroud"));
        }
        SpellEffectKind::TargetPlayerGainsLife { target: filter, .. }
        | SpellEffectKind::TargetPlayerLosesLife { target: filter, .. }
        | SpellEffectKind::DrainTarget { target: filter, .. }
        | SpellEffectKind::MillTargetPlayer { target: filter, .. }
        | SpellEffectKind::DiscardCards { target: filter, .. }
        | SpellEffectKind::TargetPlayerSacrifices { target: filter, .. } => {
            if !player_target_legal(&engine.state, tid) {
                return Err(EngineError::Illegal("target must be a player in the game"));
            }
            if matches!(filter.kind, TargetKind::OpponentPlayer) && tid as i32 == caster {
                return Err(EngineError::Illegal(
                    "target must be an opponent (cannot target yourself)",
                ));
            }
        }
        // CR 115.2 / 707.10b: counter and copy effects target spells, not abilities. The optional
        // `spell_filter` further restricts the spell type (Essence Scatter, Negate, Twincast).
        SpellEffectKind::CounterTargetSpell { spell_filter }
            if !stack_spell_target_legal(&engine.state, engine.registry, tid, *spell_filter) =>
        {
            return Err(EngineError::Illegal(
                "target must be a spell of the required type on the stack",
            ));
        }
        SpellEffectKind::CopyTargetSpell { spell_filter, .. }
            if !stack_spell_target_legal(&engine.state, engine.registry, tid, *spell_filter) =>
        {
            return Err(EngineError::Illegal(
                "target must be a spell of the required type on the stack",
            ));
        }
        SpellEffectKind::AuraAttach { target: filter }
            if !target_filter_legal(engine, filter, tid, caster) =>
        {
            return Err(EngineError::Illegal(
                "enchant target must be a valid permanent on the battlefield",
            ));
        }
        SpellEffectKind::ReturnFromGraveyard { filter, .. }
            if !graveyard_target_legal(engine, filter, tid, caster) =>
        {
            return Err(EngineError::Illegal(
                "target must be a matching card in the correct graveyard",
            ));
        }
        _ => {}
    }
    Ok(())
}

/// Compute `SpellTargets` for a spell or activated ability — which objects/players `caster`
/// can legally send this set of effects at, given the current game state. Only effects that
/// need a target are considered; non-targeted effects in the same spell are ignored.
pub(super) fn compute_spell_targets(
    engine: &GameEngine,
    caster: PlayerId,
    effects: &[SpellEffectKind],
) -> rv1::SpellTargets {
    let mut valid_permanent_ids = Vec::new();
    let mut valid_stack_ids = Vec::new();
    let mut valid_graveyard_ids = Vec::new();
    let mut can_target_self = false;
    let mut can_target_opponent = false;

    for obj in engine.state.objects.values() {
        let legal = effects
            .iter()
            .filter(|e| spell_effect_kind_needs_target(e))
            .all(|e| spell_target_legality_error(engine, e, obj.id, caster).is_ok());
        if legal {
            match obj.zone {
                Zone::Battlefield => valid_permanent_ids.push(obj.id),
                Zone::Stack => valid_stack_ids.push(obj.id),
                Zone::Graveyard => valid_graveyard_ids.push(obj.id),
                _ => {}
            }
        }
    }

    for p in &engine.state.players {
        if p.has_lost {
            continue;
        }
        let tid = p.id as ObjectId;
        let legal = effects
            .iter()
            .filter(|e| spell_effect_kind_needs_target(e))
            .all(|e| spell_target_legality_error(engine, e, tid, caster).is_ok());
        if legal {
            if p.id == caster {
                can_target_self = true;
            } else {
                can_target_opponent = true;
            }
        }
    }

    // DamageTargets: expose max_targets / fixed_damage / is_damage_targets so the client can
    // collect multiple targets and prompt for the per-target damage split.
    let mut max_targets: u32 = 0;
    let mut fixed_damage: u32 = 0;
    let mut is_damage_targets = false;
    let mut extra_mana_per_target: u32 = 0;
    for effect in effects {
        if let SpellEffectKind::DamageTargets {
            amount,
            max_targets: mt,
            extra_mana_per_target: empt,
            ..
        } = effect
        {
            is_damage_targets = true;
            max_targets = mt.unwrap_or(0);
            // resolve(0) gives the fixed total for Fixed amounts; X amounts resolve to 0
            // (the client will use the player's chosen x_value instead).
            fixed_damage = amount.resolve(0);
            extra_mana_per_target = *empt;
        }
    }

    rv1::SpellTargets {
        valid_permanent_ids,
        valid_stack_ids,
        valid_graveyard_ids,
        can_target_self,
        can_target_opponent,
        max_targets,
        fixed_damage,
        is_damage_targets,
        extra_mana_per_target,
    }
}
