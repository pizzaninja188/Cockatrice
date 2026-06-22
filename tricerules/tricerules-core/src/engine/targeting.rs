use super::combat::is_attacking_or_blocking;
use super::*;

/// Player or creature permanent on the battlefield (matches cast validation for `bolt`).
fn damage_spell_target_legal(state: &GameState, registry: &CardRegistry, tid: ObjectId) -> bool {
    if state.player_idx(tid as i32).is_some() {
        return true;
    }
    state
        .objects
        .get(&tid)
        .is_some_and(|o| o.zone == Zone::Battlefield && o.is_creature(registry))
}

fn destroy_spell_target_legal(state: &GameState, registry: &CardRegistry, tid: ObjectId) -> bool {
    state
        .objects
        .get(&tid)
        .is_some_and(|o| o.zone == Zone::Battlefield && o.is_creature(registry))
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

/// CR 702.16 / CR 702.18: returns false when `tid` is a permanent that the `caster` cannot
/// legally target due to Shroud or Hexproof. Players are never shielded by these keywords.
fn object_targetable_by(
    state: &GameState,
    registry: &CardRegistry,
    tid: ObjectId,
    caster: PlayerId,
) -> bool {
    let Some(obj) = state.objects.get(&tid) else {
        return true; // object gone — legality checked elsewhere
    };
    let Some(def) = registry.get(&obj.card_id) else {
        return true;
    };
    if def.keywords.contains(&Keyword::Shroud) {
        return false;
    }
    if def.keywords.contains(&Keyword::Hexproof) && obj.owner != caster {
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
fn object_matches_mass_filter(
    state: &GameState,
    registry: &CardRegistry,
    oid: ObjectId,
    filter: &TargetFilter,
) -> bool {
    let Some(o) = state.objects.get(&oid) else {
        return false;
    };
    if o.zone != Zone::Battlefield {
        return false;
    }
    let kind_ok = match filter.kind {
        TargetKind::Creature => o.is_creature(registry),
        TargetKind::AnyPermanent => true,
        // Player / AnyTarget / Self_ kinds are rejected at registry load for mass effects.
        _ => false,
    };
    if !kind_ok {
        return false;
    }
    if filter.not_artifact
        && registry
            .get(&o.card_id)
            .map(|d| d.is_artifact)
            .unwrap_or(false)
    {
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
    state: &GameState,
    registry: &CardRegistry,
    filter: &TargetFilter,
) -> Vec<ObjectId> {
    let mut out = Vec::new();
    for p in &state.players {
        for &oid in &p.battlefield {
            if object_matches_mass_filter(state, registry, oid, filter) {
                out.push(oid);
            }
        }
    }
    out
}

fn target_filter_legal(
    state: &GameState,
    registry: &CardRegistry,
    filter: &TargetFilter,
    tid: ObjectId,
    caster: PlayerId,
) -> bool {
    let kind_ok = match filter.kind {
        TargetKind::AnyTarget => damage_spell_target_legal(state, registry, tid),
        TargetKind::Creature => destroy_spell_target_legal(state, registry, tid),
        TargetKind::AnyPlayer => player_target_legal(state, tid),
        TargetKind::OpponentPlayer => player_target_legal(state, tid) && tid as i32 != caster,
        TargetKind::AnyPermanent => any_battlefield_permanent_target_legal(state, tid),
        // `Self_` is auto-bound to the ability's source, never a chosen target (CR 115), so it
        // is never legal to *pick*. The engine binds it directly at resolution.
        TargetKind::Self_ => false,
    };
    if !kind_ok {
        return false;
    }
    // Characteristic filters — only apply to non-player targets.
    if !filter.is_player() {
        if !object_targetable_by(state, registry, tid, caster) {
            return false;
        }
        if filter.not_artifact {
            if let Some(obj) = state.objects.get(&tid) {
                if registry
                    .get(&obj.card_id)
                    .map(|d| d.is_artifact)
                    .unwrap_or(false)
                {
                    return false;
                }
            }
        }
        if let Some(tapped_req) = filter.tapped {
            match state.objects.get(&tid) {
                Some(obj) if obj.tapped != tapped_req => return false,
                None => return false,
                _ => {}
            }
        }
        // CR 105/202.2: "nonblack", "nonwhite", … — reject a target of the excluded color.
        if let Some(c) = filter.not_color {
            match state.objects.get(&tid) {
                Some(obj)
                    if registry
                        .get(&obj.card_id)
                        .is_some_and(|d| d.colors().contains(&c)) =>
                {
                    return false
                }
                None => return false,
                _ => {}
            }
        }
        // CR 508/509: "target attacking or blocking creature" — must be in combat right now.
        if filter.attacking_or_blocking && !is_attacking_or_blocking(state, tid) {
            return false;
        }
        // "target creature you control" (equip, regenerate, …). Ownership == control until
        // control-changing effects exist (CR 109.4).
        if filter.only_controller {
            if let Some(obj) = state.objects.get(&tid) {
                if obj.owner != caster {
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
/// (With a shared single-target list this is equivalent to "all targets illegal".)
pub(super) fn spell_has_no_legal_targets_at_resolution(
    state: &GameState,
    registry: &CardRegistry,
    effects: &[SpellEffectKind],
    targets: &[ObjectId],
    caster: PlayerId,
) -> bool {
    effects.iter().any(|effect| {
        if !spell_effect_kind_needs_target(effect) {
            return false; // untargeted effects never fizzle
        }
        let Some(&tid) = targets.first() else {
            return true; // needs target but none provided
        };
        !effect_target_legal_at_resolution(state, registry, effect, tid, caster)
    })
}

/// Returns true if `tid` is a legal target for `effect` at resolution time.
fn effect_target_legal_at_resolution(
    state: &GameState,
    registry: &CardRegistry,
    effect: &SpellEffectKind,
    tid: ObjectId,
    caster: PlayerId,
) -> bool {
    match effect {
        SpellEffectKind::DamageTarget { target, .. }
        | SpellEffectKind::TargetPlayerGainsLife { target, .. }
        | SpellEffectKind::TargetPlayerLosesLife { target, .. }
        | SpellEffectKind::MillTargetPlayer { target, .. }
        | SpellEffectKind::TapTarget { target } => {
            target_filter_legal(state, registry, target, tid, caster)
        }
        SpellEffectKind::DestroyTarget { target }
        | SpellEffectKind::PumpTarget { target, .. }
        | SpellEffectKind::PutCounters { target, .. }
        | SpellEffectKind::Equip { target } => {
            target_filter_legal(state, registry, target, tid, caster)
        }
        SpellEffectKind::ExileTarget
        | SpellEffectKind::ExileTargetGainLifeEqualToPower
        | SpellEffectKind::ReturnTargetCreatureToHand => {
            destroy_spell_target_legal(state, registry, tid)
                && object_targetable_by(state, registry, tid, caster)
        }
        SpellEffectKind::ReturnTargetPermanentToHand => {
            any_battlefield_permanent_target_legal(state, tid)
                && object_targetable_by(state, registry, tid, caster)
        }
        // CR 115.2 / 707.10b: counter and copy effects target *spells* on the stack, not
        // activated/triggered abilities. The optional `spell_filter` further restricts which
        // spell types are legal (Essence Scatter, Negate, Twincast).
        SpellEffectKind::CounterTargetSpell { spell_filter } => {
            stack_spell_target_legal(state, registry, tid, *spell_filter)
        }
        SpellEffectKind::CopyTargetSpell { spell_filter, .. } => {
            stack_spell_target_legal(state, registry, tid, *spell_filter)
        }
        _ => true,
    }
}

pub(super) fn spell_effect_kind_needs_target(kind: &SpellEffectKind) -> bool {
    match kind {
        // A `Self_`-filtered pump or counter-placement is auto-bound to its source (CR 115) — it
        // takes no chosen target and prompts nobody; any other filter requires a selected target.
        SpellEffectKind::PumpTarget { target, .. }
        | SpellEffectKind::PutCounters { target, .. } => !matches!(target.kind, TargetKind::Self_),
        SpellEffectKind::DamageTarget { .. }
        | SpellEffectKind::DestroyTarget { .. }
        | SpellEffectKind::ExileTarget
        | SpellEffectKind::ExileTargetGainLifeEqualToPower
        | SpellEffectKind::ReturnTargetCreatureToHand
        | SpellEffectKind::ReturnTargetPermanentToHand
        | SpellEffectKind::TargetPlayerGainsLife { .. }
        | SpellEffectKind::TargetPlayerLosesLife { .. }
        | SpellEffectKind::MillTargetPlayer { .. }
        | SpellEffectKind::TapTarget { .. }
        | SpellEffectKind::CounterTargetSpell { .. }
        | SpellEffectKind::CopyTargetSpell { .. }
        // CR 702.6a: equip targets "target creature you control" — always targeted.
        | SpellEffectKind::Equip { .. } => true,
        _ => false,
    }
}

/// Validate targets for a `SpellEffectKind` directly (used by ability activation/trigger target selection).
pub(super) fn validate_effect_targets(
    state: &GameState,
    registry: &CardRegistry,
    caster: PlayerId,
    effect: &SpellEffectKind,
    targets: &[rv1::TargetRef],
) -> Result<(), EngineError> {
    match effect {
        SpellEffectKind::DestroyTarget { target: filter } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one target"));
            }
            if !target_filter_legal(state, registry, filter, targets[0].object_id, caster) {
                return Err(EngineError::Illegal(
                    "target must be a creature on the battlefield",
                ));
            }
        }
        SpellEffectKind::TapTarget { target: filter } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one target"));
            }
            if !target_filter_legal(state, registry, filter, targets[0].object_id, caster) {
                return Err(EngineError::Illegal(
                    "target must be a permanent on the battlefield",
                ));
            }
        }
        SpellEffectKind::DamageTarget { target: filter, .. } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one target"));
            }
            if !target_filter_legal(state, registry, filter, targets[0].object_id, caster) {
                return Err(EngineError::Illegal("illegal target for damage effect"));
            }
        }
        SpellEffectKind::PumpTarget { target: filter, .. }
        | SpellEffectKind::PutCounters { target: filter, .. } => {
            // `Self_` pumps / counter placements are auto-bound and take no chosen target.
            if matches!(filter.kind, TargetKind::Self_) {
                if !targets.is_empty() {
                    return Err(EngineError::Illegal("this effect takes no targets"));
                }
            } else {
                if targets.len() != 1 {
                    return Err(EngineError::Illegal("requires exactly one target"));
                }
                if !target_filter_legal(state, registry, filter, targets[0].object_id, caster) {
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
            if !destroy_spell_target_legal(state, registry, targets[0].object_id) {
                return Err(EngineError::Illegal(
                    "target must be a creature on the battlefield",
                ));
            }
            if !object_targetable_by(state, registry, targets[0].object_id, caster) {
                return Err(EngineError::Illegal("target has hexproof or shroud"));
            }
        }
        SpellEffectKind::ReturnTargetPermanentToHand => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal(
                    "requires exactly one permanent target",
                ));
            }
            if !any_battlefield_permanent_target_legal(state, targets[0].object_id) {
                return Err(EngineError::Illegal(
                    "target must be a permanent on the battlefield",
                ));
            }
            if !object_targetable_by(state, registry, targets[0].object_id, caster) {
                return Err(EngineError::Illegal("target has hexproof or shroud"));
            }
        }
        SpellEffectKind::TargetPlayerGainsLife { target: filter, .. }
        | SpellEffectKind::TargetPlayerLosesLife { target: filter, .. }
        | SpellEffectKind::MillTargetPlayer { target: filter, .. } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one player target"));
            }
            if !target_filter_legal(state, registry, filter, targets[0].object_id, caster) {
                return Err(EngineError::Illegal("target must be a player in the game"));
            }
            if matches!(filter.kind, TargetKind::OpponentPlayer)
                && targets[0].object_id as i32 == caster
            {
                return Err(EngineError::Illegal("cannot target yourself"));
            }
        }
        SpellEffectKind::Equip { target: filter } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one target"));
            }
            if !target_filter_legal(state, registry, filter, targets[0].object_id, caster) {
                return Err(EngineError::Illegal(
                    "equip target must be a creature you control on the battlefield",
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
        | SpellEffectKind::CreateTokens { .. }
        // CR 605.1a: a mana ability is untargeted by definition.
        | SpellEffectKind::ProduceMana { .. }
        | SpellEffectKind::None => {
            if !targets.is_empty() {
                return Err(EngineError::Illegal("this effect takes no targets"));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_spell_targets(
    state: &GameState,
    registry: &CardRegistry,
    caster: PlayerId,
    effects: &[SpellEffectKind],
    targets: &[rv1::TargetRef],
) -> Result<(), EngineError> {
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
            spell_target_legality_error(state, registry, effect, tid, caster)?;
        }
    } else if !targets.is_empty() {
        return Err(EngineError::Illegal("this spell takes no targets"));
    }
    Ok(())
}

/// Returns `Err` with a specific human-readable message when `tid` is not a legal target for `effect`.
pub(super) fn spell_target_legality_error(
    state: &GameState,
    registry: &CardRegistry,
    effect: &SpellEffectKind,
    tid: ObjectId,
    caster: PlayerId,
) -> Result<(), EngineError> {
    match effect {
        // Filter-based targeted effects share one legality path; the filter carries any
        // characteristic restriction (creature/player, `tapped`, `not_artifact`, hexproof/shroud).
        SpellEffectKind::DestroyTarget { target: filter }
        | SpellEffectKind::DamageTarget { target: filter, .. }
        | SpellEffectKind::TapTarget { target: filter }
        | SpellEffectKind::PumpTarget { target: filter, .. }
        | SpellEffectKind::PutCounters { target: filter, .. }
            if !target_filter_legal(state, registry, filter, tid, caster) =>
        {
            return Err(EngineError::Illegal(
                "target must be a creature or player on the battlefield",
            ));
        }
        SpellEffectKind::ExileTarget
        | SpellEffectKind::ExileTargetGainLifeEqualToPower
        | SpellEffectKind::ReturnTargetCreatureToHand
            if !destroy_spell_target_legal(state, registry, tid) =>
        {
            return Err(EngineError::Illegal(
                "target must be a creature on the battlefield",
            ));
        }
        SpellEffectKind::ExileTarget
        | SpellEffectKind::ExileTargetGainLifeEqualToPower
        | SpellEffectKind::ReturnTargetCreatureToHand
            if !object_targetable_by(state, registry, tid, caster) =>
        {
            return Err(EngineError::Illegal("target has hexproof or shroud"));
        }
        SpellEffectKind::ReturnTargetPermanentToHand
            if !any_battlefield_permanent_target_legal(state, tid) =>
        {
            return Err(EngineError::Illegal(
                "target must be a permanent on the battlefield",
            ));
        }
        SpellEffectKind::ReturnTargetPermanentToHand
            if !object_targetable_by(state, registry, tid, caster) =>
        {
            return Err(EngineError::Illegal("target has hexproof or shroud"));
        }
        SpellEffectKind::TargetPlayerGainsLife { target: filter, .. }
        | SpellEffectKind::TargetPlayerLosesLife { target: filter, .. }
        | SpellEffectKind::MillTargetPlayer { target: filter, .. } => {
            if !player_target_legal(state, tid) {
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
            if !stack_spell_target_legal(state, registry, tid, *spell_filter) =>
        {
            return Err(EngineError::Illegal(
                "target must be a spell of the required type on the stack",
            ));
        }
        SpellEffectKind::CopyTargetSpell { spell_filter, .. }
            if !stack_spell_target_legal(state, registry, tid, *spell_filter) =>
        {
            return Err(EngineError::Illegal(
                "target must be a spell of the required type on the stack",
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
    state: &GameState,
    registry: &CardRegistry,
    caster: PlayerId,
    effects: &[SpellEffectKind],
) -> rv1::SpellTargets {
    let mut valid_permanent_ids = Vec::new();
    let mut valid_stack_ids = Vec::new();
    let mut can_target_self = false;
    let mut can_target_opponent = false;

    for obj in state.objects.values() {
        let legal = effects
            .iter()
            .filter(|e| spell_effect_kind_needs_target(e))
            .all(|e| spell_target_legality_error(state, registry, e, obj.id, caster).is_ok());
        if legal {
            match obj.zone {
                Zone::Battlefield => valid_permanent_ids.push(obj.id),
                Zone::Stack => valid_stack_ids.push(obj.id),
                _ => {}
            }
        }
    }

    for p in &state.players {
        if p.has_lost {
            continue;
        }
        let tid = p.id as ObjectId;
        let legal = effects
            .iter()
            .filter(|e| spell_effect_kind_needs_target(e))
            .all(|e| spell_target_legality_error(state, registry, e, tid, caster).is_ok());
        if legal {
            if p.id == caster {
                can_target_self = true;
            } else {
                can_target_opponent = true;
            }
        }
    }

    rv1::SpellTargets {
        valid_permanent_ids,
        valid_stack_ids,
        can_target_self,
        can_target_opponent,
    }
}
