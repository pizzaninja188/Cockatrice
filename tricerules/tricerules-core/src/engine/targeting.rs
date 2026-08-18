use super::*;
use tricerules_cards::primitives::{GraveyardFilter, GraveyardOwner, TargetGroupDef, TargetingDef};

pub(super) fn effective_target_groups(
    effects: &[SpellEffectKind],
    targeting: Option<&TargetingDef>,
) -> Vec<TargetGroupDef> {
    if let Some(targeting) = targeting {
        return targeting.groups.clone();
    }
    let effect_indices = effects
        .iter()
        .enumerate()
        .filter_map(|(index, effect)| {
            spell_effect_kind_needs_target(effect).then_some(index as u32)
        })
        .collect::<Vec<_>>();
    if effect_indices.is_empty() {
        Vec::new()
    } else {
        vec![TargetGroupDef {
            min: 1,
            max: 1,
            prompt: "Choose a target".into(),
            effect_indices,
            distinct_from: Vec::new(),
        }]
    }
}

/// Resolve each authored group reference to the target-filter role it binds. Repeating one effect
/// index in successive groups binds successive filters returned by `target_filters()`.
pub(super) fn target_group_bindings(
    effects: &[SpellEffectKind],
    targeting: Option<&TargetingDef>,
) -> Vec<Vec<(usize, usize)>> {
    let groups = effective_target_groups(effects, targeting);
    let mut occurrences = vec![0usize; effects.len()];
    groups
        .iter()
        .map(|group| {
            group
                .effect_indices
                .iter()
                .filter_map(|&effect_index| {
                    let effect_index = effect_index as usize;
                    let filter_index = *occurrences.get(effect_index)?;
                    occurrences[effect_index] += 1;
                    Some((effect_index, filter_index))
                })
                .collect()
        })
        .collect()
}

pub(super) fn capture_stack_target(engine: &GameEngine, target: &rv1::TargetRef) -> StackTarget {
    let kind = rv1::TargetRefKind::try_from(target.kind).unwrap_or(rv1::TargetRefKind::Unspecified);
    let object_target = match kind {
        rv1::TargetRefKind::Player => false,
        rv1::TargetRefKind::Permanent
        | rv1::TargetRefKind::Stack
        | rv1::TargetRefKind::Graveyard => true,
        rv1::TargetRefKind::Unspecified => engine.state.objects.contains_key(&target.object_id),
    };
    StackTarget {
        object_id: target.object_id,
        group_index: target.group_index,
        damage_amount: target.damage_amount,
        kind: target.kind,
        zone_change_generation: object_target.then(|| {
            engine
                .state
                .zone_change_generation
                .get(&target.object_id)
                .copied()
                .unwrap_or(0)
        }),
    }
}

pub(super) fn stack_target_identity_is_current(engine: &GameEngine, target: &StackTarget) -> bool {
    target.zone_change_generation.is_none_or(|generation| {
        engine
            .state
            .zone_change_generation
            .get(&target.object_id)
            .copied()
            .unwrap_or(0)
            == generation
    })
}

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
    let Some(characteristics) = engine.characteristics(oid) else {
        return false;
    };
    super::characteristics::permanent_matches_filter_characteristics(
        &engine.state,
        filter,
        oid,
        &characteristics,
    )
}

/// Match a permanent's current derived controller against a target restriction.
fn target_controller_matches(
    state: &GameState,
    relation: TargetController,
    ability_controller: PlayerId,
    target_controller: PlayerId,
    defending_player: Option<PlayerId>,
) -> bool {
    match relation {
        TargetController::Any => true,
        TargetController::You => target_controller == ability_controller,
        TargetController::Opponent => state.are_opponents(target_controller, ability_controller),
        TargetController::NotYou => target_controller != ability_controller,
        TargetController::DefendingPlayer => defending_player
            .map(|defender| defender == target_controller)
            .unwrap_or(false),
    }
}

pub(super) fn effect_target_legal_for_binding(
    engine: &GameEngine,
    effect: &SpellEffectKind,
    filter_index: usize,
    tid: ObjectId,
    caster: PlayerId,
    source: TargetSourceIdentity,
    trigger_context: TriggerContext,
) -> bool {
    match effect {
        SpellEffectKind::CreatureDealsDamageEqualToPower {
            source: source_filter,
            target,
        } => [source_filter, target]
            .get(filter_index)
            .is_some_and(|filter| {
                target_filter_legal_with_context(
                    engine,
                    filter,
                    tid,
                    caster,
                    source,
                    trigger_context,
                )
            }),
        _ if filter_index == 0 => effect_target_legal_at_resolution_with_context(
            engine,
            effect,
            tid,
            caster,
            source,
            trigger_context,
        ),
        _ => false,
    }
}

fn spell_target_legality_error_for_binding(
    engine: &GameEngine,
    effect: &SpellEffectKind,
    filter_index: usize,
    tid: ObjectId,
    caster: PlayerId,
    source: TargetSourceIdentity,
    trigger_context: TriggerContext,
) -> Result<(), EngineError> {
    match effect {
        SpellEffectKind::CreatureDealsDamageEqualToPower {
            source: source_filter,
            target,
        } => {
            let filter = match filter_index {
                0 => source_filter,
                1 => target,
                _ => {
                    return Err(EngineError::Illegal(
                        "unknown target role for damage effect",
                    ))
                }
            };
            if target_filter_legal_with_context(
                engine,
                filter,
                tid,
                caster,
                source,
                trigger_context,
            ) {
                Ok(())
            } else {
                Err(EngineError::Illegal("illegal creature damage target"))
            }
        }
        _ if filter_index == 0 => spell_target_legality_error_with_context(
            engine,
            effect,
            tid,
            caster,
            source,
            trigger_context,
        ),
        _ => Err(EngineError::Illegal("unknown target role for effect")),
    }
}

/// Whether a battlefield permanent still satisfies an Aura's printed enchant restriction.
/// Unlike spell-target legality, an existing attachment is unaffected by hexproof or shroud.
/// `controller` remains relevant because controller-qualified enchant restrictions are continuous
/// restrictions evaluated against the Aura's current controller.
pub(super) fn attachment_filter_legal(
    engine: &GameEngine,
    filter: &TargetFilter,
    recipient: AttachmentRecipient,
    attachment_id: ObjectId,
    attachment_controller: PlayerId,
) -> bool {
    let AttachmentRecipient::Object(oid) = recipient else {
        let AttachmentRecipient::Player(player_id) = recipient else {
            unreachable!()
        };
        if !player_target_legal(&engine.state, player_id as ObjectId) {
            return false;
        }
        return match filter.kind {
            TargetKind::AnyPlayer => true,
            TargetKind::OpponentPlayer => {
                engine.state.are_opponents(player_id, attachment_controller)
            }
            _ => false,
        };
    };
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
            None,
        )
}

/// Convert a validated Aura target into explicit attachment identity. Mixed AnyTarget Auras are
/// rejected by card-data validation, so the filter kind unambiguously owns the conversion.
pub(super) fn attachment_recipient_for_target(
    filter: &TargetFilter,
    target: ObjectId,
) -> Option<AttachmentRecipient> {
    match filter.kind {
        TargetKind::Creature | TargetKind::AnyPermanent => {
            Some(AttachmentRecipient::Object(target))
        }
        TargetKind::AnyPlayer | TargetKind::OpponentPlayer => {
            Some(AttachmentRecipient::Player(target as PlayerId))
        }
        TargetKind::AnyTarget => None,
    }
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
    trigger_context: TriggerContext,
) -> bool {
    target_filter_legal_with_context(engine, filter, tid, caster, source, trigger_context)
}

fn target_filter_legal_with_context(
    engine: &GameEngine,
    filter: &TargetFilter,
    tid: ObjectId,
    caster: PlayerId,
    source: TargetSourceIdentity,
    trigger_context: TriggerContext,
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
            trigger_context.defending_player,
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
    targets
        .iter()
        .any(|&target| effect_target_legal_at_resolution(engine, effect, target, caster, source))
}

/// Returns true if `tid` is a legal target for `effect` at resolution time.
pub(super) fn effect_target_legal_at_resolution(
    engine: &GameEngine,
    effect: &SpellEffectKind,
    tid: ObjectId,
    caster: PlayerId,
    source: TargetSourceIdentity,
) -> bool {
    effect_target_legal_at_resolution_with_context(
        engine,
        effect,
        tid,
        caster,
        source,
        TriggerContext::default(),
    )
}

fn effect_target_legal_at_resolution_with_context(
    engine: &GameEngine,
    effect: &SpellEffectKind,
    tid: ObjectId,
    caster: PlayerId,
    source: TargetSourceIdentity,
    trigger_context: TriggerContext,
) -> bool {
    match effect {
        SpellEffectKind::CreatureDealsDamageEqualToPower {
            source: damage_source,
            target,
        } => {
            target_filter_legal_with_context(
                engine,
                damage_source,
                tid,
                caster,
                source,
                trigger_context,
            ) || target_filter_legal_with_context(
                engine,
                target,
                tid,
                caster,
                source,
                trigger_context,
            )
        }
        SpellEffectKind::DamageTarget { target, .. }
        | SpellEffectKind::DamageTargets { target, .. }
        | SpellEffectKind::TargetPlayerGainsLife { target, .. }
        | SpellEffectKind::TargetPlayerLosesLife { target, .. }
        | SpellEffectKind::DrainTarget { target, .. }
        | SpellEffectKind::MillTargetPlayer { target, .. }
        | SpellEffectKind::DiscardCards { target, .. }
        | SpellEffectKind::TargetPlayerSacrifices { target, .. }
        | SpellEffectKind::SkipNextUntap { target }
        | SpellEffectKind::GainControlUntilEndOfTurn { target }
        | SpellEffectKind::PreventNextDamage { target, .. } => {
            target_filter_legal_with_context(engine, target, tid, caster, source, trigger_context)
        }
        SpellEffectKind::DestroyTarget { target }
        | SpellEffectKind::DestroyAttached { target, .. }
        | SpellEffectKind::PutTargetPermanentInOwnersLibrary { target, .. }
        | SpellEffectKind::Equip { target } => {
            target_filter_legal_with_context(engine, target, tid, caster, source, trigger_context)
        }
        SpellEffectKind::PumpTarget {
            subject: EffectSubject::Chosen(target),
            ..
        }
        | SpellEffectKind::PutCounters {
            subject: EffectSubject::Chosen(target),
            ..
        }
        | SpellEffectKind::Tap {
            subject: EffectSubject::Chosen(target),
        }
        | SpellEffectKind::GrantKeywords {
            subject: EffectSubject::Chosen(target),
            ..
        }
        | SpellEffectKind::GrantTriggeredAbility {
            subject: EffectSubject::Chosen(target),
            ..
        }
        | SpellEffectKind::CreateDelayedTrigger {
            subject: EffectSubject::Chosen(target),
            ..
        }
        | SpellEffectKind::AddTypes {
            subject: EffectSubject::Chosen(target),
            ..
        }
        | SpellEffectKind::Untap {
            subject: EffectSubject::Chosen(target),
        }
        | SpellEffectKind::Regenerate {
            subject: EffectSubject::Chosen(target),
        }
        | SpellEffectKind::ApplyCombatRestriction {
            scope: CombatRestrictionScope::Chosen(target),
            ..
        } => target_filter_legal_with_context(engine, target, tid, caster, source, trigger_context),
        SpellEffectKind::PumpTarget {
            subject: EffectSubject::Source | EffectSubject::AttachedObject,
            ..
        }
        | SpellEffectKind::PutCounters {
            subject: EffectSubject::Source | EffectSubject::AttachedObject,
            ..
        }
        | SpellEffectKind::GrantKeywords {
            subject: EffectSubject::Source | EffectSubject::AttachedObject,
            ..
        }
        | SpellEffectKind::GrantTriggeredAbility {
            subject: EffectSubject::Source | EffectSubject::AttachedObject,
            ..
        }
        | SpellEffectKind::CreateDelayedTrigger {
            subject: EffectSubject::Source | EffectSubject::AttachedObject,
            ..
        }
        | SpellEffectKind::AddTypes {
            subject: EffectSubject::Source | EffectSubject::AttachedObject,
            ..
        }
        | SpellEffectKind::Untap {
            subject: EffectSubject::Source | EffectSubject::AttachedObject,
        }
        | SpellEffectKind::Tap {
            subject: EffectSubject::Source | EffectSubject::AttachedObject,
        }
        | SpellEffectKind::Regenerate {
            subject: EffectSubject::Source | EffectSubject::AttachedObject,
        }
        | SpellEffectKind::ApplyCombatRestriction {
            scope: CombatRestrictionScope::Source | CombatRestrictionScope::Matching(_),
            ..
        }
        | SpellEffectKind::ReturnTriggeredCardFromGraveyard { .. } => false,
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
        SpellEffectKind::CounterTargetSpell { spell_filter, .. } => {
            stack_spell_target_legal(&engine.state, engine.registry, tid, *spell_filter)
        }
        SpellEffectKind::CopyTargetSpell { spell_filter, .. } => {
            stack_spell_target_legal(&engine.state, engine.registry, tid, *spell_filter)
        }
        SpellEffectKind::AuraAttach { target } => {
            target_filter_legal_with_context(engine, target, tid, caster, source, trigger_context)
        }
        SpellEffectKind::ReturnFromGraveyard { filter, .. } => {
            graveyard_target_legal(engine, filter, tid, caster)
        }
        _ => true,
    }
}

pub(super) fn spell_effect_kind_needs_target(kind: &SpellEffectKind) -> bool {
    kind.needs_target()
}

/// Validate one effect's targets with the event context carried by a triggered ability.
fn validate_effect_targets(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
    effect: &SpellEffectKind,
    targets: &[rv1::TargetRef],
    trigger_context: TriggerContext,
) -> Result<(), EngineError> {
    match effect {
        SpellEffectKind::CreatureDealsDamageEqualToPower { .. } => {
            return Err(EngineError::Illegal(
                "creature damage targets require grouped target-role validation",
            ));
        }
        SpellEffectKind::DestroyTarget { target: filter }
        | SpellEffectKind::DestroyAttached { target: filter, .. }
        | SpellEffectKind::PutTargetPermanentInOwnersLibrary { target: filter, .. } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one target"));
            }
            if !target_filter_legal_with_context(
                engine,
                filter,
                targets[0].object_id,
                caster,
                source,
                trigger_context,
            ) {
                return Err(EngineError::Illegal(
                    "target must be a creature on the battlefield",
                ));
            }
        }
        SpellEffectKind::SkipNextUntap { target: filter }
        | SpellEffectKind::GainControlUntilEndOfTurn { target: filter }
        | SpellEffectKind::Tap {
            subject: EffectSubject::Chosen(filter),
        }
        | SpellEffectKind::Untap {
            subject: EffectSubject::Chosen(filter),
        } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one target"));
            }
            if !target_filter_legal_with_context(
                engine,
                filter,
                targets[0].object_id,
                caster,
                source,
                trigger_context,
            ) {
                return Err(EngineError::Illegal(
                    "target must be a permanent on the battlefield",
                ));
            }
        }
        SpellEffectKind::DamageTarget { target: filter, .. } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one target"));
            }
            if !target_filter_legal_with_context(
                engine,
                filter,
                targets[0].object_id,
                caster,
                source,
                trigger_context,
            ) {
                return Err(EngineError::Illegal("illegal target for damage effect"));
            }
        }
        SpellEffectKind::DamageTargets {
            target: filter,
            division,
            ..
        } => {
            if targets.is_empty() && !matches!(division, DamageDivision::EvenAtResolution) {
                return Err(EngineError::Illegal("requires at least one target"));
            }
            let mut seen = std::collections::HashSet::new();
            for t in targets {
                if !seen.insert(t.object_id) {
                    return Err(EngineError::Illegal("duplicate target"));
                }
                if !target_filter_legal_with_context(
                    engine,
                    filter,
                    t.object_id,
                    caster,
                    source,
                    trigger_context,
                ) {
                    return Err(EngineError::Illegal("illegal target for damage effect"));
                }
            }
        }
        SpellEffectKind::PumpTarget { subject, .. }
        | SpellEffectKind::PutCounters { subject, .. }
        | SpellEffectKind::GrantKeywords { subject, .. }
        | SpellEffectKind::GrantTriggeredAbility { subject, .. }
        | SpellEffectKind::CreateDelayedTrigger { subject, .. }
        | SpellEffectKind::AddTypes { subject, .. }
        | SpellEffectKind::Regenerate { subject } => match subject {
            EffectSubject::Source
            | EffectSubject::AttachedObject
            | EffectSubject::TriggerObject => {
                if !targets.is_empty() {
                    return Err(EngineError::Illegal("this effect takes no targets"));
                }
            }
            EffectSubject::Chosen(filter) => {
                if targets.len() != 1 {
                    return Err(EngineError::Illegal("requires exactly one target"));
                }
                if !target_filter_legal_with_context(
                    engine,
                    filter,
                    targets[0].object_id,
                    caster,
                    source,
                    trigger_context,
                ) {
                    return Err(EngineError::Illegal(
                        "target must be a creature on the battlefield",
                    ));
                }
            }
        },
        SpellEffectKind::ApplyCombatRestriction { scope, .. } => match scope {
            CombatRestrictionScope::Source | CombatRestrictionScope::Matching(_) => {
                if !targets.is_empty() {
                    return Err(EngineError::Illegal("this effect takes no targets"));
                }
            }
            CombatRestrictionScope::Chosen(filter) => {
                if targets.len() != 1 {
                    return Err(EngineError::Illegal("requires exactly one target"));
                }
                if !target_filter_legal_with_context(
                    engine,
                    filter,
                    targets[0].object_id,
                    caster,
                    source,
                    trigger_context,
                ) {
                    return Err(EngineError::Illegal(
                        "target must be a creature on the battlefield",
                    ));
                }
            }
        },
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
            if !target_filter_legal_with_context(
                engine,
                filter,
                targets[0].object_id,
                caster,
                source,
                trigger_context,
            ) {
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
            if !target_filter_legal_with_context(
                engine,
                filter,
                targets[0].object_id,
                caster,
                source,
                trigger_context,
            ) {
                return Err(EngineError::Illegal(
                    "enchant target must be a valid permanent on the battlefield",
                ));
            }
        }
        SpellEffectKind::Equip { target: filter } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one target"));
            }
            if !target_filter_legal_with_context(
                engine,
                filter,
                targets[0].object_id,
                caster,
                source,
                trigger_context,
            ) {
                return Err(EngineError::Illegal(
                    "equip target must be a creature you control on the battlefield",
                ));
            }
        }
        SpellEffectKind::PreventNextDamage { target: filter, .. } => {
            if targets.len() != 1 {
                return Err(EngineError::Illegal("requires exactly one target"));
            }
            if !target_filter_legal_with_context(
                engine,
                filter,
                targets[0].object_id,
                caster,
                source,
                trigger_context,
            ) {
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
        | SpellEffectKind::DrawDiscard { .. }
        | SpellEffectKind::GainLife { .. }
        | SpellEffectKind::Mill { .. }
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
            subject: EffectSubject::Source
                | EffectSubject::AttachedObject
                | EffectSubject::TriggerObject,
        }
        | SpellEffectKind::Tap {
            subject: EffectSubject::Source
                | EffectSubject::AttachedObject
                | EffectSubject::TriggerObject,
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
        | SpellEffectKind::ChooseResolutionBranch { .. }
        | SpellEffectKind::CreateReflexiveTrigger { .. }
        | SpellEffectKind::ChangeSourceFace { .. }
        | SpellEffectKind::ReturnTriggeredCardFromGraveyard { .. }
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
    targeting: Option<&TargetingDef>,
    targets: &[rv1::TargetRef],
) -> Result<(), EngineError> {
    validate_ability_targets_with_context(
        engine,
        caster,
        source,
        effects,
        targeting,
        targets,
        TriggerContext::default(),
    )
}

pub(super) fn validate_ability_targets_with_context(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
    effects: &[SpellEffectKind],
    targeting: Option<&TargetingDef>,
    targets: &[rv1::TargetRef],
    trigger_context: TriggerContext,
) -> Result<(), EngineError> {
    validate_grouped_targets(
        engine,
        caster,
        source,
        effects,
        targeting,
        targets,
        GroupedTargetContext {
            ability: true,
            trigger: trigger_context,
        },
    )
}

pub(super) fn validate_spell_targets(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
    effects: &[SpellEffectKind],
    targeting: Option<&TargetingDef>,
    targets: &[rv1::TargetRef],
) -> Result<(), EngineError> {
    validate_grouped_targets(
        engine,
        caster,
        source,
        effects,
        targeting,
        targets,
        GroupedTargetContext {
            ability: false,
            trigger: TriggerContext::default(),
        },
    )
}

#[derive(Clone, Copy)]
struct GroupedTargetContext {
    ability: bool,
    trigger: TriggerContext,
}

fn validate_grouped_targets(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
    effects: &[SpellEffectKind],
    targeting: Option<&TargetingDef>,
    targets: &[rv1::TargetRef],
    context: GroupedTargetContext,
) -> Result<(), EngineError> {
    let groups = effective_target_groups(effects, targeting);
    let bindings = target_group_bindings(effects, targeting);
    if groups.is_empty() {
        return if targets.is_empty() {
            Ok(())
        } else {
            Err(EngineError::Illegal("this action takes no targets"))
        };
    }
    if targets
        .iter()
        .any(|target| target.group_index as usize >= groups.len())
    {
        return Err(EngineError::Illegal("target references an unknown group"));
    }
    let grouped = groups
        .iter()
        .enumerate()
        .map(|(group_index, _)| {
            targets
                .iter()
                .filter(|target| target.group_index as usize == group_index)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for (group_index, group) in groups.iter().enumerate() {
        let selected = &grouped[group_index];
        if selected.len() < group.min as usize || selected.len() > group.max as usize {
            if targeting.is_none() && group.min == 1 && group.max == 1 {
                return Err(EngineError::Illegal("spell requires exactly one target"));
            }
            return Err(EngineError::Illegal("target group cardinality is invalid"));
        }
        let mut seen = std::collections::HashSet::new();
        for target in selected {
            if !target_ref_domain_exists(engine, target) {
                return Err(EngineError::Illegal(
                    "target kind does not match the referenced game object",
                ));
            }
            if !seen.insert(target.object_id) {
                return Err(EngineError::Illegal("duplicate target in target group"));
            }
            for &(effect_index, filter_index) in &bindings[group_index] {
                let effect = effects.get(effect_index).ok_or(EngineError::Illegal(
                    "target group references an unknown effect",
                ))?;
                if context.ability
                    && !matches!(
                        effect,
                        SpellEffectKind::CreatureDealsDamageEqualToPower { .. }
                    )
                {
                    debug_assert_eq!(filter_index, 0);
                    validate_effect_targets(
                        engine,
                        caster,
                        source,
                        effect,
                        std::slice::from_ref(*target),
                        context.trigger,
                    )?;
                } else {
                    spell_target_legality_error_for_binding(
                        engine,
                        effect,
                        filter_index,
                        target.object_id,
                        caster,
                        source,
                        context.trigger,
                    )?;
                }
            }
        }
        for &other_index in &group.distinct_from {
            let other = grouped
                .get(other_index as usize)
                .ok_or(EngineError::Illegal(
                    "target group distinctness index is invalid",
                ))?;
            if selected.iter().any(|target| {
                other
                    .iter()
                    .any(|candidate| candidate.object_id == target.object_id)
            }) {
                return Err(EngineError::Illegal(
                    "target must be distinct across target groups",
                ));
            }
        }
    }
    Ok(())
}

fn target_ref_domain_exists(engine: &GameEngine, target: &rv1::TargetRef) -> bool {
    match rv1::TargetRefKind::try_from(target.kind).unwrap_or(rv1::TargetRefKind::Unspecified) {
        // Older in-process test helpers omit presentation metadata. Legality is still established
        // below from the engine-owned effect and object state; shipped clients always send a kind.
        rv1::TargetRefKind::Unspecified => true,
        rv1::TargetRefKind::Player => engine
            .state
            .players
            .iter()
            .any(|player| !player.has_lost && player.id as ObjectId == target.object_id),
        rv1::TargetRefKind::Permanent => engine
            .state
            .players
            .iter()
            .any(|player| player.battlefield.contains(&target.object_id)),
        rv1::TargetRefKind::Stack => engine
            .state
            .stack
            .iter()
            .any(|item| item.id == target.object_id),
        rv1::TargetRefKind::Graveyard => engine
            .state
            .players
            .iter()
            .any(|player| player.graveyard.contains(&target.object_id)),
    }
}

/// Returns `Err` with a specific human-readable message when `tid` is not a legal target for `effect`.
///
/// **Fails closed.** Every arm below matches on the effect alone and checks legality in its body,
/// so the trailing arm means "this effect has no spell-side target validation" rather than "this
/// target is fine". A targeted primitive added without an arm here is rejected (and trips a
/// `debug_assert` in tests) instead of silently accepting graveyard cards, players and stack
/// objects — the CR 115.1 defect that issue #42 recorded for targeted keyword grants.
fn spell_target_legality_error_with_context(
    engine: &GameEngine,
    effect: &SpellEffectKind,
    tid: ObjectId,
    caster: PlayerId,
    source: TargetSourceIdentity,
    trigger_context: TriggerContext,
) -> Result<(), EngineError> {
    match effect {
        SpellEffectKind::CreatureDealsDamageEqualToPower { .. } => {
            return Err(EngineError::Illegal(
                "creature damage targets require grouped target-role validation",
            ));
        }
        // Filter-based targeted effects share one legality path; the filter carries any
        // characteristic restriction (creature/player, `tapped`, `not_artifact`, hexproof/shroud).
        SpellEffectKind::DestroyTarget { target: filter }
        | SpellEffectKind::DestroyAttached { target: filter, .. }
        | SpellEffectKind::PutTargetPermanentInOwnersLibrary { target: filter, .. }
        | SpellEffectKind::DamageTarget { target: filter, .. }
        | SpellEffectKind::DamageTargets { target: filter, .. }
        | SpellEffectKind::SkipNextUntap { target: filter }
        | SpellEffectKind::GainControlUntilEndOfTurn { target: filter }
        | SpellEffectKind::Tap {
            subject: EffectSubject::Chosen(filter),
        }
        | SpellEffectKind::Untap {
            subject: EffectSubject::Chosen(filter),
        }
        | SpellEffectKind::PumpTarget {
            subject: EffectSubject::Chosen(filter),
            ..
        }
        | SpellEffectKind::GrantKeywords {
            subject: EffectSubject::Chosen(filter),
            ..
        }
        | SpellEffectKind::GrantTriggeredAbility {
            subject: EffectSubject::Chosen(filter),
            ..
        }
        | SpellEffectKind::CreateDelayedTrigger {
            subject: EffectSubject::Chosen(filter),
            ..
        }
        | SpellEffectKind::AddTypes {
            subject: EffectSubject::Chosen(filter),
            ..
        }
        | SpellEffectKind::ApplyCombatRestriction {
            scope: CombatRestrictionScope::Chosen(filter),
            ..
        }
        | SpellEffectKind::PutCounters {
            subject: EffectSubject::Chosen(filter),
            ..
        }
        | SpellEffectKind::PreventNextDamage { target: filter, .. } => {
            if !target_filter_legal_with_context(
                engine,
                filter,
                tid,
                caster,
                source,
                trigger_context,
            ) {
                return Err(EngineError::Illegal(
                    "target must be a creature or player on the battlefield",
                ));
            }
        }
        SpellEffectKind::PumpTarget {
            subject: EffectSubject::Source | EffectSubject::AttachedObject,
            ..
        }
        | SpellEffectKind::PutCounters {
            subject: EffectSubject::Source | EffectSubject::AttachedObject,
            ..
        }
        | SpellEffectKind::GrantTriggeredAbility {
            subject: EffectSubject::Source | EffectSubject::AttachedObject,
            ..
        }
        | SpellEffectKind::CreateDelayedTrigger {
            subject: EffectSubject::Source | EffectSubject::AttachedObject,
            ..
        }
        | SpellEffectKind::Tap {
            subject: EffectSubject::Source | EffectSubject::AttachedObject,
        }
        | SpellEffectKind::Untap {
            subject: EffectSubject::Source | EffectSubject::AttachedObject,
        }
        | SpellEffectKind::ApplyCombatRestriction {
            scope: CombatRestrictionScope::Source | CombatRestrictionScope::Matching(_),
            ..
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
        SpellEffectKind::CounterTargetSpell { spell_filter, .. }
        | SpellEffectKind::CopyTargetSpell { spell_filter, .. } => {
            if !stack_spell_target_legal(&engine.state, engine.registry, tid, *spell_filter) {
                return Err(EngineError::Illegal(
                    "target must be a spell of the required type on the stack",
                ));
            }
        }
        SpellEffectKind::AuraAttach { target: filter } => {
            if !target_filter_legal_with_context(
                engine,
                filter,
                tid,
                caster,
                source,
                trigger_context,
            ) {
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
        // Ability-only effects (CR 702.6a equip, CR 701.19 regenerate). Registry load already
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
    targeting: Option<&TargetingDef>,
) -> rv1::SpellTargets {
    compute_spell_targets_with_context(
        engine,
        caster,
        source,
        effects,
        targeting,
        TriggerContext::default(),
        Some(TargetingCostAction::Spells),
    )
}

pub(super) fn compute_ability_targets(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
    effects: &[SpellEffectKind],
    targeting: Option<&TargetingDef>,
) -> rv1::SpellTargets {
    compute_spell_targets_with_context(
        engine,
        caster,
        source,
        effects,
        targeting,
        TriggerContext::default(),
        Some(TargetingCostAction::ActivatedAbilities),
    )
}

pub(super) fn compute_spell_targets_with_context(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
    effects: &[SpellEffectKind],
    targeting: Option<&TargetingDef>,
    trigger_context: TriggerContext,
    targeting_cost_action: Option<TargetingCostAction>,
) -> rv1::SpellTargets {
    // DamageTargets metadata controls the allocation UI. Cardinality lives exclusively on groups.
    let mut fixed_damage: u32 = 0;
    let mut is_damage_targets = false;
    let mut extra_mana_per_target: u32 = 0;
    let mut damage_division = rv1::DamageDivision::ChooseAtCast;
    for effect in effects {
        if let SpellEffectKind::DamageTargets {
            amount,
            extra_mana_per_target: empt,
            division,
            ..
        } = effect
        {
            is_damage_targets = true;
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

    let bindings = target_group_bindings(effects, targeting);
    let groups = effective_target_groups(effects, targeting)
        .into_iter()
        .enumerate()
        .map(|(group_index, group)| {
            let referenced = bindings[group_index]
                .iter()
                .filter_map(|&(effect_index, filter_index)| {
                    effects
                        .get(effect_index)
                        .map(|effect| (effect, filter_index))
                })
                .collect::<Vec<_>>();
            let legal = |object_id| {
                referenced.iter().all(|(effect, filter_index)| {
                    spell_target_legality_error_for_binding(
                        engine,
                        effect,
                        *filter_index,
                        object_id,
                        caster,
                        source,
                        trigger_context,
                    )
                    .is_ok()
                })
            };
            let mut permanent_ids = Vec::new();
            let mut graveyard_ids = Vec::new();
            for offset in 0..engine.state.players.len() {
                let player_idx =
                    (engine.state.active_player_idx + offset) % engine.state.players.len();
                let player = &engine.state.players[player_idx];
                permanent_ids.extend(
                    player
                        .battlefield
                        .iter()
                        .copied()
                        .filter(|&object_id| legal(object_id)),
                );
                graveyard_ids.extend(
                    player
                        .graveyard
                        .iter()
                        .copied()
                        .filter(|&object_id| legal(object_id)),
                );
            }
            let stack_ids = engine
                .state
                .stack
                .iter()
                .map(|item| item.id)
                .filter(|&object_id| legal(object_id))
                .collect();
            let mut self_legal = false;
            let mut opponent_legal = false;
            for player in &engine.state.players {
                if !player.has_lost && legal(player.id as ObjectId) {
                    if player.id == caster {
                        self_legal = true;
                    } else if engine.state.are_opponents(player.id, caster) {
                        opponent_legal = true;
                    }
                }
            }
            rv1::LegalTargetGroup {
                group_index: group_index as u32,
                prompt_text: group.prompt,
                min: group.min,
                max: group.max,
                valid_permanent_ids: permanent_ids,
                valid_stack_ids: stack_ids,
                can_target_self: self_legal,
                can_target_opponent: opponent_legal,
                valid_graveyard_ids: graveyard_ids,
                distinct_from_group_indices: group.distinct_from,
            }
        })
        .collect::<Vec<_>>();

    let targeting_cost_applications = targeting_cost_action
        .map(|action| engine.targeting_cost_applications(caster, action, &groups))
        .unwrap_or_default();
    rv1::SpellTargets {
        fixed_damage,
        is_damage_targets,
        extra_mana_per_target,
        damage_division: damage_division as i32,
        groups,
        targeting_cost_applications,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tricerules_cards::CounterKind;

    #[test]
    fn grouped_targets_publish_independent_candidates_and_validate_distinctness_atomically() {
        let engine = GameEngine::new(73_001, &[10, 20], 20, None, true).expect("new");
        let effects = vec![
            SpellEffectKind::TargetPlayerGainsLife {
                amount: 1,
                target: TargetFilter {
                    kind: TargetKind::AnyPlayer,
                    ..TargetFilter::default()
                },
            },
            SpellEffectKind::TargetPlayerLosesLife {
                amount: 1,
                target: TargetFilter {
                    kind: TargetKind::OpponentPlayer,
                    ..TargetFilter::default()
                },
            },
        ];
        let targeting = TargetingDef {
            groups: vec![
                TargetGroupDef {
                    min: 1,
                    max: 1,
                    prompt: "Choose any player".into(),
                    effect_indices: vec![0],
                    distinct_from: vec![],
                },
                TargetGroupDef {
                    min: 1,
                    max: 1,
                    prompt: "Choose a different opponent".into(),
                    effect_indices: vec![1],
                    distinct_from: vec![0],
                },
            ],
        };
        let source = TargetSourceIdentity::current(&engine, u32::MAX);
        let published = compute_spell_targets(&engine, 10, source, &effects, Some(&targeting));
        assert_eq!(published.groups.len(), 2);
        assert!(published.groups[0].can_target_self);
        assert!(published.groups[0].can_target_opponent);
        assert!(!published.groups[1].can_target_self);
        assert!(published.groups[1].can_target_opponent);
        assert_eq!(published.groups[1].distinct_from_group_indices, vec![0]);

        let refs = |first, second| {
            vec![
                rv1::TargetRef {
                    object_id: first,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
                },
                rv1::TargetRef {
                    object_id: second,
                    damage_amount: 0,
                    group_index: 1,
                    kind: 0,
                },
            ]
        };
        validate_spell_targets(
            &engine,
            10,
            source,
            &effects,
            Some(&targeting),
            &refs(10, 20),
        )
        .expect("independently filtered distinct groups are legal");
        assert!(validate_spell_targets(
            &engine,
            10,
            source,
            &effects,
            Some(&targeting),
            &refs(20, 20),
        )
        .is_err());
        assert!(validate_spell_targets(
            &engine,
            10,
            source,
            &effects,
            Some(&targeting),
            &refs(20, 10),
        )
        .is_err());
    }

    #[test]
    fn opponent_relation_is_state_backed_and_independent_of_loss() {
        let mut engine = GameEngine::new(96905, &[10, 20], 20, None, true).expect("new");
        assert!(target_controller_matches(
            &engine.state,
            TargetController::Opponent,
            10,
            20,
            None,
        ));
        assert!(!target_controller_matches(
            &engine.state,
            TargetController::Opponent,
            10,
            10,
            None,
        ));
        assert!(target_controller_matches(
            &engine.state,
            TargetController::You,
            10,
            10,
            None,
        ));
        assert!(!target_controller_matches(
            &engine.state,
            TargetController::You,
            10,
            20,
            None,
        ));
        assert!(target_controller_matches(
            &engine.state,
            TargetController::Any,
            10,
            10,
            None,
        ));
        assert!(target_controller_matches(
            &engine.state,
            TargetController::Any,
            10,
            20,
            None,
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
            AttachmentRecipient::Object(bear),
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
            .base_controller = 1;
        engine
            .state
            .objects
            .get_mut(&bear)
            .expect("bear")
            .controller = 1;
        assert!(attachment_filter_legal(
            &engine,
            &opponent_only,
            AttachmentRecipient::Object(bear),
            u32::MAX,
            0,
        ));
        assert!(!attachment_filter_legal(
            &engine,
            &opponent_only,
            AttachmentRecipient::Object(bear),
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
        assert!(!target_filter_legal_with_context(
            &engine,
            &filter,
            bear,
            0,
            original_source,
            TriggerContext::default(),
        ));

        *engine.state.zone_change_generation.entry(bear).or_default() += 1;
        assert!(target_filter_legal_with_context(
            &engine,
            &filter,
            bear,
            0,
            original_source,
            TriggerContext::default(),
        ));
        assert!(!target_filter_legal_with_context(
            &engine,
            &filter,
            bear,
            0,
            TargetSourceIdentity::current(&engine, bear),
            TriggerContext::default(),
        ));

        assert!(!attachment_filter_legal(
            &engine,
            &filter,
            AttachmentRecipient::Object(bear),
            bear,
            0
        ));
        assert!(attachment_filter_legal(
            &engine,
            &filter,
            AttachmentRecipient::Object(bear),
            u32::MAX,
            0,
        ));
    }

    #[test]
    fn player_attachment_legality_tracks_relationships_and_lost_players() {
        let decks = Some(vec![
            vec!["grizzly_bears".into(); 7],
            vec!["forest".into(); 7],
        ]);
        let mut engine = GameEngine::new(96907, &[0, 1], 20, decks, true).expect("new");
        let any_player = TargetFilter {
            kind: TargetKind::AnyPlayer,
            ..TargetFilter::default()
        };
        let opponent = TargetFilter {
            kind: TargetKind::OpponentPlayer,
            ..TargetFilter::default()
        };

        assert!(attachment_filter_legal(
            &engine,
            &any_player,
            AttachmentRecipient::Player(0),
            u32::MAX,
            0,
        ));
        assert!(attachment_filter_legal(
            &engine,
            &opponent,
            AttachmentRecipient::Player(1),
            u32::MAX,
            0,
        ));
        assert!(!attachment_filter_legal(
            &engine,
            &opponent,
            AttachmentRecipient::Player(0),
            u32::MAX,
            0,
        ));
        assert!(!attachment_filter_legal(
            &engine,
            &opponent,
            AttachmentRecipient::Player(1),
            u32::MAX,
            1,
        ));
        assert!(!attachment_filter_legal(
            &engine,
            &any_player,
            AttachmentRecipient::Object(engine.state.players[0].hand[0]),
            u32::MAX,
            0,
        ));

        engine.state.players[1].has_lost = true;
        assert!(!attachment_filter_legal(
            &engine,
            &any_player,
            AttachmentRecipient::Player(1),
            u32::MAX,
            0,
        ));
    }

    #[test]
    fn shared_characteristic_filter_uses_derived_power_and_keyword_absence() {
        let decks = Some(vec![
            vec![
                "grizzly_bears".into(),
                "wind_drake".into(),
                "forest".into(),
                "forest".into(),
                "forest".into(),
                "forest".into(),
                "forest".into(),
            ],
            vec!["forest".into(); 7],
        ]);
        let mut engine = GameEngine::new(71_001, &[0, 1], 20, decks, true).expect("new");
        let mut deploy = |card_id: &str| {
            let oid = engine.state.players[0]
                .hand
                .iter()
                .copied()
                .find(|oid| engine.state.objects[oid].card_id == card_id)
                .expect("card in hand");
            engine.state.players[0]
                .hand
                .retain(|candidate| *candidate != oid);
            engine.state.players[0].battlefield.push(oid);
            engine.state.objects.get_mut(&oid).expect("object").zone = Zone::Battlefield;
            oid
        };
        let bear = deploy("grizzly_bears");
        let drake = deploy("wind_drake");
        engine
            .state
            .objects
            .get_mut(&bear)
            .expect("bear")
            .counters
            .insert(CounterKind::PlusOnePlusOne, 2);

        let power_four = TargetFilter {
            kind: TargetKind::Creature,
            power: Some(PowerComparison::AtLeast(4)),
            ..TargetFilter::default()
        };
        assert!(filter_characteristics_match(&engine, &power_four, bear));
        assert!(!filter_characteristics_match(&engine, &power_four, drake));

        let without_flying = TargetFilter {
            kind: TargetKind::Creature,
            excluded_keywords: vec![Keyword::Flying],
            ..TargetFilter::default()
        };
        assert!(filter_characteristics_match(&engine, &without_flying, bear));
        assert!(!filter_characteristics_match(
            &engine,
            &without_flying,
            drake
        ));
    }
}
