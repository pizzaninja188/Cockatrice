use super::combat::is_attacking_or_blocking;
use super::*;
use tricerules_cards::primitives::{GraveyardFilter, GraveyardOwner, PermanentTypeFilter};

/// The object that sourced a targeted spell or ability, captured at the moment targets are
/// chosen. Object ids are stable across zone changes in this engine, so CR 400.7 identity also
/// requires the source's zone-change generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TargetSourceIdentity {
    object_id: ObjectId,
    zone_change_generation: Option<u64>,
}

impl TargetSourceIdentity {
    pub(super) fn current(engine: &GameEngine, object_id: ObjectId) -> Self {
        Self::captured(
            object_id,
            engine
                .state
                .zone_change_generation
                .get(&object_id)
                .copied()
                .unwrap_or(0),
        )
    }

    pub(super) fn captured(object_id: ObjectId, zone_change_generation: u64) -> Self {
        Self {
            object_id,
            zone_change_generation: Some(zone_change_generation),
        }
    }

    /// Spell copies and ability stack items have no backing game object. Their allocated stack id
    /// is still globally unique, so id equality is sufficient and cannot accidentally identify a
    /// physical permanent.
    pub(super) fn virtual_stack(object_id: ObjectId) -> Self {
        Self {
            object_id,
            zone_change_generation: None,
        }
    }

    pub(super) fn for_stack_item(engine: &GameEngine, item: &StackItem) -> Self {
        if let Some(source_id) = item.source_permanent_id {
            Self::captured(source_id, item.source_zone_change)
        } else if item.is_copy {
            Self::virtual_stack(item.id)
        } else {
            Self::current(engine, item.id)
        }
    }

    fn is_current_object(self, engine: &GameEngine, candidate_id: ObjectId) -> bool {
        if self.object_id != candidate_id {
            return false;
        }
        self.zone_change_generation.is_none_or(|generation| {
            engine
                .state
                .zone_change_generation
                .get(&candidate_id)
                .copied()
                .unwrap_or(0)
                == generation
        })
    }
}

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
        if !def.matches_card_type_outside_stack(card_type) {
            return false;
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
    if characteristics.has_keyword(Keyword::Hexproof)
        && engine
            .state
            .are_opponents(characteristics.controller, caster)
    {
        return false;
    }
    true
}

/// Every [`TargetFilter`] characteristic restriction that reads the same whether `oid` was
/// *targeted* (CR 115) or *selected* by an untargeted mass effect (CR 701.7). The single owner of
/// these predicates: [`target_filter_legal`] and [`object_matches_mass_filter`] both call it, so a
/// new filter field lands on both paths or neither.
///
/// Deliberately excluded, because they are **not** shared:
/// - hexproof/shroud ([`object_targetable_by`]) — CR 702.11e, untargeted effects affect those
///   permanents normally, so only the targeted caller applies it;
/// - `controller` — needs an activating player, which untargeted mass selection does not have
///   (non-`Any` values are rejected in a mass filter at registry load);
/// - `exclude_source` — needs the source object's captured CR 400.7 identity, which untargeted
///   mass selection does not have (and registry validation rejects on mass filters);
/// - the [`TargetKind`] mapping — the two paths accept different kinds and check zone differently.
pub(super) fn filter_characteristics_match(
    engine: &GameEngine,
    filter: &TargetFilter,
    oid: ObjectId,
) -> bool {
    let Some(object) = engine.state.objects.get(&oid) else {
        return false;
    };
    let Some(characteristics) = engine.characteristics(oid) else {
        return false;
    };
    if !filter.permanent_types.is_empty()
        && !filter.permanent_types.iter().any(|kind| match kind {
            PermanentTypeFilter::Creature => characteristics.is_creature(),
            PermanentTypeFilter::Artifact => characteristics.is_artifact(),
            PermanentTypeFilter::Enchantment => characteristics.has_type("Enchantment"),
            PermanentTypeFilter::Land => characteristics.has_type("Land"),
        })
    {
        return false;
    }
    // CR 205.3: a "non-[Subtype]" restriction (Eyeblight's Ending) skips the excluded subtypes.
    if filter
        .excluded_subtypes
        .iter()
        .any(|subtype| characteristics.has_type(subtype))
    {
        return false;
    }
    if !filter
        .required_keywords
        .iter()
        .all(|keyword| characteristics.has_keyword(*keyword))
    {
        return false;
    }
    if filter.not_artifact && characteristics.is_artifact() {
        return false;
    }
    if let Some(tapped_req) = filter.tapped {
        if object.tapped != tapped_req {
            return false;
        }
    }
    // CR 105/202.2: "nonblack", "nonwhite", … — reject an object of the excluded color.
    if let Some(c) = filter.not_color {
        if characteristics.colors.contains(&c) {
            return false;
        }
    }
    // CR 105/202.2: the inclusive mirror — "all green creatures" (Perish), "target red permanent".
    if let Some(c) = filter.is_color {
        if !characteristics.colors.contains(&c) {
            return false;
        }
    }
    // CR 508/509: "attacking or blocking creature" — must be in combat right now.
    if filter.attacking_or_blocking && !is_attacking_or_blocking(&engine.state, oid) {
        return false;
    }
    true
}

/// Match a permanent's current derived controller against a target restriction.
fn target_controller_matches(
    state: &GameState,
    relation: TargetController,
    ability_controller: PlayerId,
    target_controller: PlayerId,
) -> bool {
    match relation {
        TargetController::Any => true,
        TargetController::You => target_controller == ability_controller,
        TargetController::Opponent => state.are_opponents(target_controller, ability_controller),
    }
}

/// Whether a battlefield permanent still satisfies an Aura's printed enchant restriction.
/// Unlike spell-target legality, an existing attachment is unaffected by hexproof or shroud.
/// `controller` remains relevant because controller-qualified enchant restrictions are continuous
/// restrictions evaluated against the Aura's current controller.
pub(super) fn attachment_filter_legal(
    engine: &GameEngine,
    filter: &TargetFilter,
    oid: ObjectId,
    attachment_id: ObjectId,
    attachment_controller: PlayerId,
) -> bool {
    let Some(object) = engine.state.objects.get(&oid) else {
        return false;
    };
    if object.zone != Zone::Battlefield {
        return false;
    }
    let Some(characteristics) = engine.characteristics(oid) else {
        return false;
    };
    let kind_ok = match filter.kind {
        TargetKind::Creature => characteristics.is_creature(),
        TargetKind::AnyPermanent => true,
        _ => false,
    };
    kind_ok
        && (!filter.exclude_source || oid != attachment_id)
        && filter_characteristics_match(engine, filter, oid)
        && target_controller_matches(
            &engine.state,
            filter.controller,
            attachment_controller,
            characteristics.controller,
        )
}

/// Legality of a single target against a [`TargetFilter`].
/// `caster` supplies the reference player for hexproof and controller-relative restrictions.
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
        // Player / AnyTarget kinds are rejected at registry load for mass effects.
        _ => false,
    };
    if !kind_ok {
        return false;
    }
    filter_characteristics_match(engine, filter, oid)
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
    source: TargetSourceIdentity,
) -> bool {
    target_filter_legal(engine, filter, tid, caster, source)
}

fn target_filter_legal(
    engine: &GameEngine,
    filter: &TargetFilter,
    tid: ObjectId,
    caster: PlayerId,
    source: TargetSourceIdentity,
) -> bool {
    let kind_ok = match filter.kind {
        TargetKind::AnyTarget => damage_spell_target_legal(engine, tid),
        TargetKind::Creature => destroy_spell_target_legal(engine, tid),
        TargetKind::AnyPlayer => player_target_legal(&engine.state, tid),
        TargetKind::OpponentPlayer => {
            player_target_legal(&engine.state, tid)
                && engine.state.are_opponents(tid as i32, caster)
        }
        TargetKind::AnyPermanent => any_battlefield_permanent_target_legal(&engine.state, tid),
    };
    if !kind_ok {
        return false;
    }
    // Characteristic filters — only apply to non-player targets. `is_player()` covers the
    // player-only kinds; `AnyTarget` is decided per target, since the same filter accepts both a
    // creature and a player (Lightning Bolt) and a player carries no characteristics.
    let target_is_player = engine.state.player_idx(tid as i32).is_some();
    if !target_is_player && filter.exclude_source && source.is_current_object(engine, tid) {
        return false;
    }
    if !filter.is_player() && !target_is_player {
        // CR 702.16/702.18 — targeting only; the untargeted mass path deliberately skips this.
        if !object_targetable_by(engine, tid, caster) {
            return false;
        }
        if !filter_characteristics_match(engine, filter, tid) {
            return false;
        }
        // Controller-relative restrictions read CR 110.2 control through the layer pipeline, so
        // current control rather than ownership decides. Targeting-only: an untargeted mass effect
        // has no activating player to compare against.
        let Some(characteristics) = engine.characteristics(tid) else {
            return false;
        };
        if !target_controller_matches(
            &engine.state,
            filter.controller,
            caster,
            characteristics.controller,
        ) {
            return false;
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
    spell_filter: Option<CardTypeFilter>,
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
    face.matches_card_type(filter)
}

pub(super) fn effect_has_legal_target_at_resolution(
    engine: &GameEngine,
    effect: &SpellEffectKind,
    targets: &[ObjectId],
    caster: PlayerId,
    source: TargetSourceIdentity,
) -> bool {
    if !spell_effect_kind_needs_target(effect) {
        return true;
    }
    if matches!(effect, SpellEffectKind::DamageTargets { .. }) {
        return targets.iter().any(|&target| {
            effect_target_legal_at_resolution(engine, effect, target, caster, source)
        });
    }
    targets.first().is_some_and(|&target| {
        effect_target_legal_at_resolution(engine, effect, target, caster, source)
    })
}

/// Returns true if `tid` is a legal target for `effect` at resolution time.
fn effect_target_legal_at_resolution(
    engine: &GameEngine,
    effect: &SpellEffectKind,
    tid: ObjectId,
    caster: PlayerId,
    source: TargetSourceIdentity,
) -> bool {
    match effect {
        SpellEffectKind::DamageTarget { target, .. }
        | SpellEffectKind::DamageTargets { target, .. }
        | SpellEffectKind::TargetPlayerGainsLife { target, .. }
        | SpellEffectKind::TargetPlayerLosesLife { target, .. }
        | SpellEffectKind::DrainTarget { target, .. }
        | SpellEffectKind::MillTargetPlayer { target, .. }
        | SpellEffectKind::DiscardCards { target, .. }
        | SpellEffectKind::TargetPlayerSacrifices { target, .. }
        | SpellEffectKind::TapTarget { target }
        | SpellEffectKind::PreventNextDamage { target, .. } => {
            target_filter_legal(engine, target, tid, caster, source)
        }
        SpellEffectKind::DestroyTarget { target }
        | SpellEffectKind::GrantKeywordsTarget { target, .. }
        | SpellEffectKind::Equip { target } => {
            target_filter_legal(engine, target, tid, caster, source)
        }
        SpellEffectKind::PumpTarget {
            subject: EffectSubject::Chosen(target),
            ..
        }
        | SpellEffectKind::PutCounters {
            subject: EffectSubject::Chosen(target),
            ..
        }
        | SpellEffectKind::Untap {
            subject: EffectSubject::Chosen(target),
        }
        | SpellEffectKind::Regenerate {
            subject: EffectSubject::Chosen(target),
        } => target_filter_legal(engine, target, tid, caster, source),
        SpellEffectKind::PumpTarget {
            subject: EffectSubject::Source,
            ..
        }
        | SpellEffectKind::PutCounters {
            subject: EffectSubject::Source,
            ..
        }
        | SpellEffectKind::Untap {
            subject: EffectSubject::Source,
        }
        | SpellEffectKind::Regenerate {
            subject: EffectSubject::Source,
        } => false,
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
        SpellEffectKind::AuraAttach { target } => {
            target_filter_legal(engine, target, tid, caster, source)
        }
        SpellEffectKind::ReturnFromGraveyard { filter, .. } => {
            graveyard_target_legal(engine, filter, tid, caster)
        }
        _ => true,
    }
}

pub(super) fn spell_effect_kind_needs_target(kind: &SpellEffectKind) -> bool {
    match kind {
        SpellEffectKind::PumpTarget { subject, .. }
        | SpellEffectKind::PutCounters { subject, .. }
        | SpellEffectKind::Untap { subject }
        | SpellEffectKind::Regenerate { subject } => {
            matches!(subject, EffectSubject::Chosen(_))
        }
        SpellEffectKind::DamageTarget { .. }
        | SpellEffectKind::DamageTargets { .. }
        | SpellEffectKind::DestroyTarget { .. }
        | SpellEffectKind::GrantKeywordsTarget { .. }
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
    source: TargetSourceIdentity,
    effect: &SpellEffectKind,
    targets: &[rv1::TargetRef],
) -> Result<(), EngineError> {
    match effect {
        SpellEffectKind::DestroyTarget { target: filter } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one target"));
            }
            if !target_filter_legal(engine, filter, targets[0].object_id, caster, source) {
                return Err(EngineError::Illegal(
                    "target must be a creature on the battlefield",
                ));
            }
        }
        SpellEffectKind::TapTarget { target: filter }
        | SpellEffectKind::Untap {
            subject: EffectSubject::Chosen(filter),
        } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one target"));
            }
            if !target_filter_legal(engine, filter, targets[0].object_id, caster, source) {
                return Err(EngineError::Illegal(
                    "target must be a permanent on the battlefield",
                ));
            }
        }
        SpellEffectKind::DamageTarget { target: filter, .. } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one target"));
            }
            if !target_filter_legal(engine, filter, targets[0].object_id, caster, source) {
                return Err(EngineError::Illegal("illegal target for damage effect"));
            }
        }
        SpellEffectKind::DamageTargets {
            target: filter,
            max_targets,
            division,
            ..
        } => {
            if targets.is_empty() && !matches!(division, DamageDivision::EvenAtResolution) {
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
                if !target_filter_legal(engine, filter, t.object_id, caster, source) {
                    return Err(EngineError::Illegal("illegal target for damage effect"));
                }
            }
        }
        SpellEffectKind::PumpTarget { subject, .. }
        | SpellEffectKind::PutCounters { subject, .. }
        | SpellEffectKind::Regenerate { subject } => match subject {
            EffectSubject::Source => {
                if !targets.is_empty() {
                    return Err(EngineError::Illegal("this effect takes no targets"));
                }
            }
            EffectSubject::Chosen(filter) => {
                if targets.len() != 1 {
                    return Err(EngineError::Illegal("requires exactly one target"));
                }
                if !target_filter_legal(engine, filter, targets[0].object_id, caster, source) {
                    return Err(EngineError::Illegal(
                        "target must be a creature on the battlefield",
                    ));
                }
            }
        },
        SpellEffectKind::GrantKeywordsTarget { target: filter, .. } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one target"));
            }
            if !target_filter_legal(engine, filter, targets[0].object_id, caster, source) {
                return Err(EngineError::Illegal(
                    "target must be a creature on the battlefield",
                ));
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
            if !target_filter_legal(engine, filter, targets[0].object_id, caster, source) {
                return Err(EngineError::Illegal("target must be a player in the game"));
            }
            if matches!(filter.kind, TargetKind::OpponentPlayer)
                && !engine
                    .state
                    .are_opponents(targets[0].object_id as i32, caster)
            {
                return Err(EngineError::Illegal("cannot target yourself"));
            }
        }
        SpellEffectKind::AuraAttach { target: filter } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("aura requires exactly one enchant target"));
            }
            if !target_filter_legal(engine, filter, targets[0].object_id, caster, source) {
                return Err(EngineError::Illegal(
                    "enchant target must be a valid permanent on the battlefield",
                ));
            }
        }
        SpellEffectKind::Equip { target: filter } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one target"));
            }
            if !target_filter_legal(engine, filter, targets[0].object_id, caster, source) {
                return Err(EngineError::Illegal(
                    "equip target must be a creature you control on the battlefield",
                ));
            }
        }
        SpellEffectKind::PreventNextDamage { target: filter, .. } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one target"));
            }
            if !target_filter_legal(engine, filter, targets[0].object_id, caster, source) {
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
        // CR 115.1: "you lose life" does not target. `LifeAmount::TargetManaValue` reads a
        // *sibling* effect's target, so LoseLife itself never declares one.
        | SpellEffectKind::LoseLife { .. }
        | SpellEffectKind::EachOpponentLosesLifeYouGainEqual { .. }
        // Counter/copy target *spells*; they are never put on an ability, so this ability-only
        // validator only needs them present for exhaustiveness (spell targets go through
        // `spell_target_legality_error`).
        | SpellEffectKind::CounterTargetSpell { .. }
        | SpellEffectKind::CopyTargetSpell { .. }
        | SpellEffectKind::DestroyAll { .. }
        | SpellEffectKind::DamageAll { .. }
        | SpellEffectKind::TapAllCreatures { .. }
        | SpellEffectKind::UntapAll { .. }
        | SpellEffectKind::Untap {
            subject: EffectSubject::Source,
        }
        | SpellEffectKind::PumpAll { .. }
        | SpellEffectKind::GrantKeywordsAll { .. }
        | SpellEffectKind::GrantKeywordsAllPermanents { .. }
        | SpellEffectKind::CreateTokens { .. }
        | SpellEffectKind::PreventAllCombatDamageTurn
        | SpellEffectKind::DamageCantBePreventedThisTurn
        // CR 605.1a: a mana ability is untargeted by definition.
        | SpellEffectKind::ProduceMana { .. }
        // CR 115.1: "deals N damage to that player / to you" names a player, it does not target.
        | SpellEffectKind::DamagePlayer { .. }
        // CR 701.18: library search is untargeted; the library card is chosen via a pending
        // interrupt, not a target declared at cast time. Scry is the same shape — the cards it
        // acts on are the top of the controller's own library, decided at resolution.
        | SpellEffectKind::SearchLibrary { .. }
        | SpellEffectKind::Scry { .. }
        | SpellEffectKind::ChangeSourceFace { .. }
        | SpellEffectKind::None => {
            if !targets.is_empty() {
                return Err(EngineError::Illegal("this effect takes no targets"));
            }
        }
    }
    Ok(())
}

/// Target validation for an activated or triggered ability's effect list (CR 608.2).
///
/// Deliberately **not** `validate_spell_targets`: that one delegates to
/// [`spell_target_legality_error`], which covers only the effects a *spell* can carry — it has no
/// arm for `Equip` or `AuraAttach`, so routing abilities through it silently drops equip's
/// "target creature you control" check. This walks [`validate_effect_targets`], the exhaustive
/// per-effect validator, so a one-effect ability behaves exactly as it did before effect lists.
pub(super) fn validate_ability_targets(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
    effects: &[SpellEffectKind],
    targets: &[rv1::TargetRef],
) -> Result<(), EngineError> {
    let mut any_targeting = false;
    for effect in effects {
        if !spell_effect_kind_needs_target(effect) {
            continue;
        }
        any_targeting = true;
        validate_effect_targets(engine, caster, source, effect, targets)?;
    }
    // Every effect is untargeted (Phyrexian Arena's `[Draw, LoseLife]`), so a client that sent
    // targets anyway is wrong — same rejection the untargeted arms of `validate_effect_targets`
    // would have produced when the ability held a single effect.
    if !any_targeting && !targets.is_empty() {
        return Err(EngineError::Illegal("this effect takes no targets"));
    }
    Ok(())
}

pub(super) fn validate_spell_targets(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
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
                validate_effect_targets(engine, caster, source, effect, targets)?;
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
            spell_target_legality_error(engine, effect, tid, caster, source)?;
        }
    } else if !targets.is_empty() {
        return Err(EngineError::Illegal("this spell takes no targets"));
    }
    Ok(())
}

/// Returns `Err` with a specific human-readable message when `tid` is not a legal target for `effect`.
///
/// **Fails closed.** Every arm below matches on the effect alone and checks legality in its body,
/// so the trailing arm means "this effect has no spell-side target validation" rather than "this
/// target is fine". A targeted primitive added without an arm here is rejected (and trips a
/// `debug_assert` in tests) instead of silently accepting graveyard cards, players and stack
/// objects — the CR 115.1 defect that issue #42 recorded for `GrantKeywordsTarget`.
pub(super) fn spell_target_legality_error(
    engine: &GameEngine,
    effect: &SpellEffectKind,
    tid: ObjectId,
    caster: PlayerId,
    source: TargetSourceIdentity,
) -> Result<(), EngineError> {
    match effect {
        // Filter-based targeted effects share one legality path; the filter carries any
        // characteristic restriction (creature/player, `tapped`, `not_artifact`, hexproof/shroud).
        SpellEffectKind::DestroyTarget { target: filter }
        | SpellEffectKind::DamageTarget { target: filter, .. }
        | SpellEffectKind::DamageTargets { target: filter, .. }
        | SpellEffectKind::TapTarget { target: filter }
        | SpellEffectKind::Untap {
            subject: EffectSubject::Chosen(filter),
        }
        | SpellEffectKind::PumpTarget {
            subject: EffectSubject::Chosen(filter),
            ..
        }
        | SpellEffectKind::GrantKeywordsTarget { target: filter, .. }
        | SpellEffectKind::PutCounters {
            subject: EffectSubject::Chosen(filter),
            ..
        }
        | SpellEffectKind::PreventNextDamage { target: filter, .. } => {
            if !target_filter_legal(engine, filter, tid, caster, source) {
                return Err(EngineError::Illegal(
                    "target must be a creature or player on the battlefield",
                ));
            }
        }
        SpellEffectKind::PumpTarget {
            subject: EffectSubject::Source,
            ..
        }
        | SpellEffectKind::PutCounters {
            subject: EffectSubject::Source,
            ..
        }
        | SpellEffectKind::Untap {
            subject: EffectSubject::Source,
        } => {
            return Err(EngineError::Illegal(
                "source-bound effects are only valid on activated or triggered abilities",
            ));
        }
        SpellEffectKind::ExileTarget
        | SpellEffectKind::ExileTargetGainLifeEqualToPower
        | SpellEffectKind::ReturnTargetCreatureToHand => {
            if !destroy_spell_target_legal(engine, tid) {
                return Err(EngineError::Illegal(
                    "target must be a creature on the battlefield",
                ));
            }
            if !object_targetable_by(engine, tid, caster) {
                return Err(EngineError::Illegal("target has hexproof or shroud"));
            }
        }
        SpellEffectKind::ReturnTargetPermanentToHand => {
            if !any_battlefield_permanent_target_legal(&engine.state, tid) {
                return Err(EngineError::Illegal(
                    "target must be a permanent on the battlefield",
                ));
            }
            if !object_targetable_by(engine, tid, caster) {
                return Err(EngineError::Illegal("target has hexproof or shroud"));
            }
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
            if matches!(filter.kind, TargetKind::OpponentPlayer)
                && !engine.state.are_opponents(tid as i32, caster)
            {
                return Err(EngineError::Illegal(
                    "target must be an opponent (cannot target yourself)",
                ));
            }
        }
        // CR 115.2 / 707.10b: counter and copy effects target spells, not abilities. The optional
        // `spell_filter` further restricts the spell type (Essence Scatter, Negate, Twincast).
        SpellEffectKind::CounterTargetSpell { spell_filter }
        | SpellEffectKind::CopyTargetSpell { spell_filter, .. } => {
            if !stack_spell_target_legal(&engine.state, engine.registry, tid, *spell_filter) {
                return Err(EngineError::Illegal(
                    "target must be a spell of the required type on the stack",
                ));
            }
        }
        SpellEffectKind::AuraAttach { target: filter } => {
            if !target_filter_legal(engine, filter, tid, caster, source) {
                return Err(EngineError::Illegal(
                    "enchant target must be a valid permanent on the battlefield",
                ));
            }
        }
        SpellEffectKind::ReturnFromGraveyard { filter, .. } => {
            if !graveyard_target_legal(engine, filter, tid, caster) {
                return Err(EngineError::Illegal(
                    "target must be a matching card in the correct graveyard",
                ));
            }
        }
        // Ability-only effects (CR 702.6a equip, CR 701.15 regenerate). Registry load already
        // rejects them in `spell_effect`, so reaching here means a mis-routed call rather than a
        // bad target — reject rather than fall through to the fail-closed arm's generic message.
        SpellEffectKind::Equip { .. } | SpellEffectKind::Regenerate { .. } => {
            return Err(EngineError::Illegal(
                "this effect is only valid on an activated or triggered ability",
            ));
        }
        // Fail closed. Untargeted effects legitimately land here (`compute_spell_targets` and the
        // copy-with-new-targets path do not pre-filter as strictly as `validate_spell_targets`),
        // but an effect that declares it needs a target and has no arm above is a wiring bug.
        other => {
            if spell_effect_kind_needs_target(other) {
                debug_assert!(
                    false,
                    "targeted effect {other:?} has no arm in spell_target_legality_error"
                );
                return Err(EngineError::Illegal(
                    "this effect has no spell-side target validation",
                ));
            }
        }
    }
    Ok(())
}

/// Compute `SpellTargets` for a spell or activated ability — which objects/players `caster`
/// can legally send this set of effects at, given the current game state. Only effects that
/// need a target are considered; non-targeted effects in the same spell are ignored.
pub(super) fn compute_spell_targets(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
    effects: &[SpellEffectKind],
) -> rv1::SpellTargets {
    let mut valid_permanent_ids = Vec::new();
    let mut valid_stack_ids = Vec::new();
    let mut valid_graveyard_ids = Vec::new();
    let mut can_target_self = false;
    let mut can_target_opponent = false;

    let candidate_is_legal = |object_id| {
        effects
            .iter()
            .filter(|e| spell_effect_kind_needs_target(e))
            .all(|e| spell_target_legality_error(engine, e, object_id, caster, source).is_ok())
    };

    for offset in 0..engine.state.players.len() {
        let player_idx = (engine.state.active_player_idx + offset) % engine.state.players.len();
        let player = &engine.state.players[player_idx];
        for &object_id in &player.battlefield {
            if candidate_is_legal(object_id) {
                valid_permanent_ids.push(object_id);
            }
        }
        for &object_id in &player.graveyard {
            if candidate_is_legal(object_id) {
                valid_graveyard_ids.push(object_id);
            }
        }
    }

    for item in &engine.state.stack {
        if candidate_is_legal(item.id) {
            valid_stack_ids.push(item.id);
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
            .all(|e| spell_target_legality_error(engine, e, tid, caster, source).is_ok());
        if legal {
            if p.id == caster {
                can_target_self = true;
            } else if engine.state.are_opponents(p.id, caster) {
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
    let mut damage_division = rv1::DamageDivision::ChooseAtCast;
    for effect in effects {
        if let SpellEffectKind::DamageTargets {
            amount,
            max_targets: mt,
            extra_mana_per_target: empt,
            division,
            ..
        } = effect
        {
            is_damage_targets = true;
            max_targets = mt.unwrap_or(0);
            // Resolving with X=0 gives the fixed total for literal amounts; X amounts become 0
            // (the client will use the player's chosen x_value instead).
            fixed_damage = amount.resolve_unconditional(0).unwrap_or(0);
            extra_mana_per_target = *empt;
            // Fireball divides on resolution, so the client must not prompt for a split it would
            // only discard (CR 601.2d applies to "divided as you choose", not "divided evenly").
            damage_division = match division {
                DamageDivision::ChooseAtCast => rv1::DamageDivision::ChooseAtCast,
                DamageDivision::EvenAtResolution => rv1::DamageDivision::EvenAtResolution,
            };
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
        damage_division: damage_division as i32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opponent_relation_is_state_backed_and_independent_of_loss() {
        let mut engine = GameEngine::new(96905, &[10, 20], 20, None, true).expect("new");
        assert!(target_controller_matches(
            &engine.state,
            TargetController::Opponent,
            10,
            20
        ));
        assert!(!target_controller_matches(
            &engine.state,
            TargetController::Opponent,
            10,
            10
        ));
        assert!(target_controller_matches(
            &engine.state,
            TargetController::You,
            10,
            10
        ));
        assert!(!target_controller_matches(
            &engine.state,
            TargetController::You,
            10,
            20
        ));
        assert!(target_controller_matches(
            &engine.state,
            TargetController::Any,
            10,
            10
        ));
        assert!(target_controller_matches(
            &engine.state,
            TargetController::Any,
            10,
            20
        ));

        engine.state.players[1].has_lost = true;
        assert!(
            engine.state.are_opponents(10, 20),
            "relationship membership is independent of whether a player has lost"
        );
    }

    #[test]
    fn attachment_controller_relation_uses_current_derived_control() {
        let decks = Some(vec![
            vec!["grizzly_bears".into(); 7],
            vec!["forest".into(); 7],
        ]);
        let mut engine = GameEngine::new(96906, &[0, 1], 20, decks, true).expect("new");
        let bear = engine.state.players[0]
            .hand
            .iter()
            .copied()
            .find(|oid| {
                engine
                    .state
                    .objects
                    .get(oid)
                    .is_some_and(|o| o.card_id == "grizzly_bears")
            })
            .expect("bear in hand");
        engine.state.players[0].hand.retain(|oid| *oid != bear);
        engine.state.players[0].battlefield.push(bear);
        engine.state.objects.get_mut(&bear).expect("bear").zone = Zone::Battlefield;

        let opponent_only = TargetFilter {
            kind: TargetKind::Creature,
            controller: TargetController::Opponent,
            ..TargetFilter::default()
        };
        assert!(!attachment_filter_legal(
            &engine,
            &opponent_only,
            bear,
            u32::MAX,
            0,
        ));

        engine.state.players[0]
            .battlefield
            .retain(|oid| *oid != bear);
        engine.state.players[1].battlefield.push(bear);
        engine
            .state
            .objects
            .get_mut(&bear)
            .expect("bear")
            .controller = 1;
        assert!(attachment_filter_legal(
            &engine,
            &opponent_only,
            bear,
            u32::MAX,
            0,
        ));
        assert!(!attachment_filter_legal(
            &engine,
            &opponent_only,
            bear,
            u32::MAX,
            1,
        ));
    }

    #[test]
    fn source_exclusion_uses_zone_change_generation() {
        let decks = Some(vec![
            vec!["grizzly_bears".into(); 7],
            vec!["forest".into(); 7],
        ]);
        let mut engine = GameEngine::new(70070, &[0, 1], 20, decks, true).expect("new");
        let bear = engine.state.players[0].hand[0];
        engine.state.players[0].hand.remove(0);
        engine.state.players[0].battlefield.push(bear);
        engine.state.objects.get_mut(&bear).expect("bear").zone = Zone::Battlefield;

        let filter = TargetFilter {
            kind: TargetKind::Creature,
            exclude_source: true,
            ..TargetFilter::default()
        };
        let original_source = TargetSourceIdentity::current(&engine, bear);
        assert!(!target_filter_legal(
            &engine,
            &filter,
            bear,
            0,
            original_source,
        ));

        *engine.state.zone_change_generation.entry(bear).or_default() += 1;
        assert!(target_filter_legal(
            &engine,
            &filter,
            bear,
            0,
            original_source,
        ));
        assert!(!target_filter_legal(
            &engine,
            &filter,
            bear,
            0,
            TargetSourceIdentity::current(&engine, bear),
        ));

        assert!(!attachment_filter_legal(&engine, &filter, bear, bear, 0));
        assert!(attachment_filter_legal(&engine, &filter, bear, u32::MAX, 0,));
    }
}
