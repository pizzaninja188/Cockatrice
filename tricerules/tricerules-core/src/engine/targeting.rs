use super::*;
use tricerules_card_model::primitives::{
    CompiledProgram, GraveyardFilter, GraveyardOwner, TargetObjectExclusion, TargetRole,
    TargetSchema, TargetingDef,
};
use tricerules_card_model::{AbilityDefinitionId, CardFaceId, ModeId, ProgramOwnerId};

/// CR 608.2b/h: current attachment, or generation-scoped LKI after the source leaves.
/// Shared by Due Diligence's target exclusion and untargeted attached-object effects.
pub(super) fn attached_object_identity(
    state: &GameState,
    source_id: ObjectId,
    source_generation: u64,
) -> Option<(ObjectId, u64)> {
    attached_object_lki(state, source_id, source_generation)
        .map(|attachment| (attachment.object_id, attachment.zone_change_generation))
}

/// CR 608.2b/h and 701.24c: current attached object, or the exact source-departure record.
/// The captured owner remains available when that target has since changed zones.
pub(super) fn attached_object_lki(
    state: &GameState,
    source_id: ObjectId,
    source_generation: u64,
) -> Option<crate::state::AttachedObjectLki> {
    if state
        .zone_change_generation
        .get(&source_id)
        .copied()
        .unwrap_or(0)
        == source_generation
        && state
            .objects
            .get(&source_id)
            .is_some_and(|object| object.zone == Zone::Battlefield)
    {
        let AttachmentRecipient::Object(oid) = state.objects.get(&source_id)?.attached_to? else {
            return None;
        };
        let target = state.objects.get(&oid)?;
        (target.zone == Zone::Battlefield).then(|| crate::state::AttachedObjectLki {
            object_id: oid,
            zone_change_generation: state.zone_change_generation.get(&oid).copied().unwrap_or(0),
            owner: target.owner,
        })
    } else {
        state
            .last_known_information
            .attached_object(&(source_id, source_generation))
            .copied()
    }
}

pub(super) fn object_is_excluded(
    state: &GameState,
    exclusions: &[TargetObjectExclusion],
    candidate: ObjectId,
    source: TargetSourceIdentity,
    context: TriggerContext,
) -> bool {
    let generation = state
        .zone_change_generation
        .get(&candidate)
        .copied()
        .unwrap_or(0);
    exclusions.iter().any(|exclusion| match exclusion {
        TargetObjectExclusion::Source => {
            let identity = context
                .source_after_zone_change
                .map(|object| (object.object_id, Some(object.zone_change_generation)))
                .unwrap_or((source.object_id, source.zone_change_generation));
            identity.0 == candidate && identity.1.is_none_or(|expected| expected == generation)
        }
        TargetObjectExclusion::AttachedObject => {
            source
                .zone_change_generation
                .and_then(|expected| attached_object_identity(state, source.object_id, expected))
                == Some((candidate, generation))
        }
    })
}

pub(super) fn cost_target_matches(
    engine: &GameEngine,
    filter: &tricerules_card_model::TargetMatchFilter,
    kind: i32,
    oid: ObjectId,
    actor: PlayerId,
    source: TargetSourceIdentity,
) -> bool {
    let expected_kind = match filter {
        tricerules_card_model::TargetMatchFilter::Battlefield(_) => rv1::TargetRefKind::Permanent,
        tricerules_card_model::TargetMatchFilter::Graveyard(_) => rv1::TargetRefKind::Graveyard,
    };
    (kind == rv1::TargetRefKind::Unspecified as i32 || kind == expected_kind as i32)
        && target_role_legal_at_resolution(
            engine,
            filter.role(),
            oid,
            actor,
            source,
            TriggerContext::default(),
        )
}

pub(super) fn target_schema<'effects, 'targeting>(
    effects: &'effects [SpellEffectKind],
    targeting: Option<&'targeting TargetingDef>,
) -> TargetSchema<'effects, 'targeting> {
    TargetSchema::compile(effects, targeting).expect("card registry validated target schema")
}

pub(super) fn target_schema_with_program<'effects, 'targeting>(
    effects: &'effects [SpellEffectKind],
    targeting: Option<&'targeting TargetingDef>,
    program: Option<&'effects CompiledProgram>,
) -> TargetSchema<'effects, 'targeting> {
    if !effects.is_empty() || program.is_none() {
        return target_schema(effects, targeting);
    }
    TargetSchema::compile_program(program.expect("checked above"), targeting)
        .expect("card registry validated authored program target schema")
}

/// Expand Vronos's one optional target per opponent without changing the wire schema.
/// Seats are never removed/reordered in this engine; allocate lost opponents' empty slots too.
/// Occupied slots additionally retain their controller in the existing StackTarget receipt.
pub(super) fn runtime_target_schema<'effects, 'targeting>(
    engine: &GameEngine,
    controller: PlayerId,
    effects: &'effects [SpellEffectKind],
    targeting: Option<&'targeting TargetingDef>,
    program: Option<&'effects CompiledProgram>,
) -> (TargetSchema<'effects, 'targeting>, Vec<Option<PlayerId>>) {
    let mut schema = target_schema_with_program(effects, targeting, program);
    if !targeting.is_some_and(TargetingDef::requires_per_opponent_targets) {
        return (schema, Vec::new());
    }
    let template = schema.groups.remove(0);
    let mut controllers = Vec::new();
    for player in &engine.state.players {
        if engine.state.are_opponents(controller, player.id) {
            let mut group = template.clone();
            group.prompt = format!("{} (player {})", group.prompt, player.id).into();
            schema.groups.push(group);
            controllers.push(Some(player.id));
        }
    }
    // CR 115.3: this family expands one printed instance of "target". Keeping an illegal
    // original during retargeting must not allow that same incarnation in another slot.
    let count = schema.groups.len();
    for (index, group) in schema.groups.iter_mut().enumerate() {
        group.distinct_from = (0..count)
            .filter(|other| *other != index)
            .map(|other| other as u32)
            .collect::<Vec<_>>()
            .into();
    }
    (schema, controllers)
}

pub(super) fn required_target_controller_is_current(
    engine: &GameEngine,
    object_id: ObjectId,
    required: Option<PlayerId>,
) -> bool {
    required.is_none_or(|player| {
        engine
            .state
            .players
            .iter()
            .any(|state| state.id == player && !state.has_lost)
            && engine
                .characteristics(object_id)
                .is_some_and(|value| value.controller == player)
    })
}

/// The authored/legacy target data and event-time context consumed by one targeting operation.
/// Keeping this together prevents candidate publication and command validation from diverging.
#[derive(Clone, Copy)]
pub(super) struct TargetResolutionContext<'a> {
    pub(super) effects: &'a [SpellEffectKind],
    pub(super) targeting: Option<&'a TargetingDef>,
    pub(super) trigger_context: TriggerContext,
    pub(super) program: Option<&'a CompiledProgram>,
}

pub(super) fn face_program<'a>(
    engine: &'a GameEngine,
    card_id: &str,
    face_id: &CardFaceId,
) -> Option<&'a CompiledProgram> {
    engine
        .registry
        .compiled_program_for_owner(&ProgramOwnerId::face(card_id.to_owned(), face_id.clone()))
}

pub(super) fn face_mode_program<'a>(
    engine: &'a GameEngine,
    card_id: &str,
    face_id: &CardFaceId,
    mode_id: &ModeId,
) -> Option<&'a CompiledProgram> {
    engine
        .registry
        .compiled_program_for_owner(&ProgramOwnerId::face_mode(
            card_id.to_owned(),
            face_id.clone(),
            mode_id.clone(),
        ))
}

pub(super) fn ability_program<'a>(
    engine: &'a GameEngine,
    definition: &AbilityDefinitionId,
) -> Option<&'a CompiledProgram> {
    engine
        .registry
        .compiled_program_for_owner(&ProgramOwnerId::ability(definition.clone()))
}

pub(super) fn ability_mode_program<'a>(
    engine: &'a GameEngine,
    definition: &AbilityDefinitionId,
    mode_id: &ModeId,
) -> Option<&'a CompiledProgram> {
    engine
        .registry
        .compiled_program_for_owner(&ProgramOwnerId::ability_mode(
            definition.clone(),
            mode_id.clone(),
        ))
}

pub(super) fn capture_stack_target(engine: &GameEngine, target: &rv1::TargetRef) -> StackTarget {
    let object_target = engine.state.objects.contains_key(&target.object_id)
        && matches!(
            target_ref_identity(engine, target),
            TargetRefIdentity::Object(..)
        );
    StackTarget {
        counter_amount: target.counter_amount,
        object_id: target.object_id,
        required_controller: None,
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

/// Recheck Nesting Grounds' two exact target roles when its resolution-time counter choice is
/// answered. The targets have already passed CR 608.2b once; this protects the parked continuation
/// if a target's generation, battlefield status, or controller changes while it waits.
pub(in crate::engine) fn counter_move_target_pair_is_current(
    engine: &GameEngine,
    item: &StackItem,
    source_target: &StackTarget,
    destination_target: &StackTarget,
) -> bool {
    if source_target.group_index != 0
        || destination_target.group_index != 1
        || source_target.object_id == destination_target.object_id
        || source_target.zone_change_generation.is_none()
        || destination_target.zone_change_generation.is_none()
    {
        return false;
    }
    let roles = SpellEffectKind::MoveOneCounterBetweenTargets.target_roles();
    let [TargetRole::Filtered(source_filter), TargetRole::Filtered(destination_filter)] =
        roles.as_slice()
    else {
        return false;
    };
    let source = TargetSourceIdentity::for_stack_item(engine, item);
    let is_current = |target: &StackTarget, role: TargetRole<'_>| {
        stack_target_identity_is_current(engine, target)
            && target.required_controller.is_none_or(|required| {
                engine
                    .characteristics(target.object_id)
                    .is_some_and(|characteristics| characteristics.controller == required)
            })
            && target_role_legal_at_resolution(
                engine,
                role,
                target.object_id,
                item.controller,
                source,
                item.trigger_context,
            )
    };
    is_current(source_target, TargetRole::Filtered(source_filter))
        && is_current(destination_target, TargetRole::Filtered(destination_filter))
}

/// The object that sourced a targeted spell or ability, captured at the moment targets are
/// chosen. Object ids are stable across zone changes in this engine, so CR 400.7 identity also
/// requires the source's zone-change generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TargetSourceIdentity {
    object_id: ObjectId,
    zone_change_generation: Option<u64>,
    expected_zone: Zone,
    locked_qualities: Option<SourceQualities>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct SourceQualities {
    colors: [bool; 5],
    types: [bool; 8],
}

impl SourceQualities {
    fn from_values(colors: &[Color], types: &[String]) -> Self {
        let mut result = Self::default();
        for color in colors {
            result.colors[match color {
                Color::White => 0,
                Color::Blue => 1,
                Color::Black => 2,
                Color::Red => 3,
                Color::Green => 4,
            }] = true;
        }
        for (index, card_type) in [
            ProtectionCardType::Artifact,
            ProtectionCardType::Creature,
            ProtectionCardType::Enchantment,
            ProtectionCardType::Instant,
            ProtectionCardType::Kindred,
            ProtectionCardType::Land,
            ProtectionCardType::Planeswalker,
            ProtectionCardType::Sorcery,
        ]
        .into_iter()
        .enumerate()
        {
            result.types[index] = types.iter().any(|value| value == card_type.as_str());
        }
        result
    }

    fn matches(self, protection: ProtectionQuality) -> bool {
        match protection {
            ProtectionQuality::Color(color) => {
                self.colors[match color {
                    Color::White => 0,
                    Color::Blue => 1,
                    Color::Black => 2,
                    Color::Red => 3,
                    Color::Green => 4,
                }]
            }
            ProtectionQuality::CardType(card_type) => {
                let index = match card_type {
                    ProtectionCardType::Artifact => 0,
                    ProtectionCardType::Creature => 1,
                    ProtectionCardType::Enchantment => 2,
                    ProtectionCardType::Instant => 3,
                    ProtectionCardType::Kindred => 4,
                    ProtectionCardType::Land => 5,
                    ProtectionCardType::Planeswalker => 6,
                    ProtectionCardType::Sorcery => 7,
                };
                self.types[index]
            }
        }
    }

    fn values(self) -> (Vec<Color>, Vec<String>) {
        let colors = [
            Color::White,
            Color::Blue,
            Color::Black,
            Color::Red,
            Color::Green,
        ]
        .into_iter()
        .enumerate()
        .filter_map(|(index, color)| self.colors[index].then_some(color))
        .collect();
        let types = [
            ProtectionCardType::Artifact,
            ProtectionCardType::Creature,
            ProtectionCardType::Enchantment,
            ProtectionCardType::Instant,
            ProtectionCardType::Kindred,
            ProtectionCardType::Land,
            ProtectionCardType::Planeswalker,
            ProtectionCardType::Sorcery,
        ]
        .into_iter()
        .enumerate()
        .filter(|(index, _)| self.types[*index])
        .map(|(_, card_type)| card_type.as_str().to_string())
        .collect();
        (colors, types)
    }
}

impl TargetSourceIdentity {
    pub(super) fn object_id(self) -> ObjectId {
        self.object_id
    }

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
            expected_zone: Zone::Battlefield,
            locked_qualities: None,
        }
    }

    pub(super) fn captured_for_ability(
        object_id: ObjectId,
        zone_change_generation: u64,
        ability: &ActivatedAbilityDef,
    ) -> Self {
        let expected_zone = match ability.source_zone {
            AbilitySourceZone::Battlefield => Zone::Battlefield,
            AbilitySourceZone::Hand => Zone::Hand,
            AbilitySourceZone::Graveyard => Zone::Graveyard,
        };
        Self {
            expected_zone,
            ..Self::captured(object_id, zone_change_generation)
        }
    }

    pub(super) fn spell_face(
        engine: &GameEngine,
        object_id: ObjectId,
        face_index: usize,
        controller: PlayerId,
    ) -> Self {
        let locked_qualities = engine
            .state
            .objects
            .get(&object_id)
            .and_then(|object| engine.registry.get(&object.card_id))
            .and_then(|definition| definition.face(face_index))
            .map(|face| {
                let values = super::characteristics::spell_cast_characteristics(
                    &engine.state,
                    engine.registry,
                    object_id,
                    controller,
                    face,
                );
                SourceQualities::from_values(&values.colors, &values.types)
            });
        Self {
            locked_qualities,
            ..Self::current(engine, object_id)
        }
    }

    pub(super) fn for_stack_item(engine: &GameEngine, item: &StackItem) -> Self {
        if let Some(source_id) = item.source_permanent_id {
            item.activated_ability.as_ref().map_or_else(
                || Self::captured(source_id, item.source_zone_change),
                |ability| Self::captured_for_ability(source_id, item.source_zone_change, ability),
            )
        } else {
            let locked_qualities = engine
                .registry
                .get(&item.card_id)
                .and_then(|definition| definition.face(item.face_index))
                .and_then(|printed_face| {
                    let prototype;
                    let face = if item.prototyped {
                        prototype = printed_face.prototyped_face()?;
                        &prototype
                    } else {
                        printed_face
                    };
                    let values = super::characteristics::spell_cast_characteristics(
                        &engine.state,
                        engine.registry,
                        item.id,
                        item.controller,
                        face,
                    );
                    let colors = super::characteristics::stack_spell_colors(
                        &engine.state,
                        engine.registry,
                        item,
                    )?;
                    Some(SourceQualities::from_values(&colors, &values.types))
                });
            Self {
                object_id: item.id,
                expected_zone: Zone::Stack,
                zone_change_generation: (!item.is_copy).then(|| {
                    engine
                        .state
                        .zone_change_generation
                        .get(&item.id)
                        .copied()
                        .unwrap_or(0)
                }),
                locked_qualities,
            }
        }
    }

    fn qualities(self, engine: &GameEngine) -> Option<SourceQualities> {
        if let Some(qualities) = self.locked_qualities {
            return Some(qualities);
        }
        let generation = self.zone_change_generation?;
        let current_generation = engine
            .state
            .zone_change_generation
            .get(&self.object_id)
            .copied()
            .unwrap_or(0);
        if current_generation == generation
            && engine
                .state
                .objects
                .get(&self.object_id)
                .is_some_and(|object| {
                    object.zone == self.expected_zone && object.phase_status.is_none()
                })
        {
            return engine
                .characteristics(self.object_id)
                .map(|characteristics| {
                    SourceQualities::from_values(&characteristics.colors, &characteristics.types)
                });
        }
        let colors = engine
            .state
            .last_known_information
            .colors(&(self.object_id, generation))?;
        let types = engine
            .state
            .last_known_information
            .types(&(self.object_id, generation))?;
        Some(SourceQualities::from_values(colors, types))
    }

    pub(super) fn quality_values(self, engine: &GameEngine) -> (Vec<Color>, Vec<String>) {
        self.qualities(engine)
            .map(SourceQualities::values)
            .unwrap_or_default()
    }
}

fn destroy_spell_target_legal(engine: &GameEngine, tid: ObjectId) -> bool {
    engine
        .state
        .objects
        .get(&tid)
        .is_some_and(|o| o.zone == Zone::Battlefield && o.phase_status.is_none())
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
        .is_some_and(|o| o.zone == Zone::Battlefield && o.phase_status.is_none())
}

/// Check if `oid` is a legal graveyard target for a [`GraveyardFilter`].
/// Graveyard cards have no Hexproof/Shroud (those keywords only apply on the battlefield).
pub(super) fn trigger_object_mana_value(
    engine: &GameEngine,
    trigger_context: TriggerContext,
) -> Option<u32> {
    let observed = trigger_context.observed_object?;
    let current_generation = engine
        .state
        .zone_change_generation
        .get(&observed.object_id)
        .copied()
        .unwrap_or(0);
    if current_generation == observed.zone_change_generation
        && engine
            .state
            .objects
            .get(&observed.object_id)
            .is_some_and(|object| object.zone == Zone::Battlefield)
    {
        engine
            .characteristics(observed.object_id)
            .map(|characteristics| characteristics.mana_value)
    } else {
        engine
            .state
            .last_known_information
            .mana_value(&(observed.object_id, observed.zone_change_generation))
            .copied()
    }
}

pub(super) fn graveyard_target_legal(
    engine: &GameEngine,
    filter: &GraveyardFilter,
    oid: ObjectId,
    caster: PlayerId,
    source: TargetSourceIdentity,
    trigger_context: TriggerContext,
) -> bool {
    let Some(obj) = engine.state.objects.get(&oid) else {
        return false;
    };
    if obj.zone != Zone::Graveyard || obj.is_token() {
        return false;
    }
    if object_is_excluded(
        &engine.state,
        &filter.excluded_objects,
        oid,
        source,
        trigger_context,
    ) {
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
        GraveyardOwner::Opponent => {
            if !engine.state.are_opponents(caster, obj.owner) {
                return false;
            }
        }
    }
    let Some(definition) = engine.registry.get(&obj.card_id) else {
        return false;
    };
    if !super::card_predicates::zone_card_matches_filter(
        &engine.state,
        engine.registry,
        oid,
        filter.card.as_ref(),
    ) {
        return false;
    }
    if filter.mana_value_less_than_trigger_object {
        let Some(event_mana_value) = trigger_object_mana_value(engine, trigger_context) else {
            return false;
        };
        if definition.mana_value_outside_stack() >= event_mana_value {
            return false;
        }
    }
    true
}

/// CR 702.16 / CR 702.18: returns false when `tid` is a permanent that the `caster` cannot
/// legally target due to Shroud or Hexproof. Players are never shielded by these keywords.
fn object_targetable_by(
    engine: &GameEngine,
    tid: ObjectId,
    caster: PlayerId,
    source: TargetSourceIdentity,
) -> bool {
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
    if let Some(source_qualities) = source.qualities(engine) {
        if characteristics
            .protections
            .iter()
            .copied()
            .any(|protection| source_qualities.matches(protection))
        {
            return false;
        }
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
/// - `controller` / `owner` — need a reference player, which untargeted mass selection does not have
///   (non-`Any` values are rejected in a mass filter at registry load);
/// - `excluded_objects` — needs the source object's captured CR 400.7 identity, which untargeted
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

/// CR 110.2: the physical object's owner is stable across control-changing effects.
pub(super) fn target_owner_matches(
    state: &GameState,
    relation: TargetOwner,
    reference_player: PlayerId,
    oid: ObjectId,
) -> bool {
    match relation {
        TargetOwner::Any => true,
        TargetOwner::You => state
            .objects
            .get(&oid)
            .is_some_and(|o| o.owner == reference_player),
    }
}

pub(super) fn target_role_legality_error(
    engine: &GameEngine,
    role: TargetRole<'_>,
    tid: ObjectId,
    caster: PlayerId,
    source: TargetSourceIdentity,
    trigger_context: TriggerContext,
) -> Result<(), EngineError> {
    target_ref_role_legality_error(
        engine,
        role,
        &rv1::TargetRef {
            object_id: tid,
            ..Default::default()
        },
        caster,
        source,
        trigger_context,
    )
}

pub(super) fn target_ref_role_legality_error(
    engine: &GameEngine,
    role: TargetRole<'_>,
    target: &rv1::TargetRef,
    caster: PlayerId,
    source: TargetSourceIdentity,
    trigger_context: TriggerContext,
) -> Result<(), EngineError> {
    let tid = target.object_id;
    let kind = target.kind();
    if kind != rv1::TargetRefKind::Unspecified && !matches!(role, TargetRole::Filtered(_)) {
        let expected = match role {
            TargetRole::CreaturePermanent => rv1::TargetRefKind::Permanent,
            TargetRole::StackSpell(_) | TargetRole::StackAbility => rv1::TargetRefKind::Stack,
            TargetRole::GraveyardCard(_) => rv1::TargetRefKind::Graveyard,
            TargetRole::Filtered(_) => unreachable!(),
        };
        if kind != expected {
            return Err(EngineError::Illegal(
                "target domain does not match its role",
            ));
        }
    }
    let (legal, message) = match role {
        TargetRole::Filtered(filter) => {
            target_filter_legal_for_ref(engine, filter, target, caster, source, trigger_context)
                .then_some(())
                .map_or_else(
                    || {
                        let message = match filter.kind {
                            TargetKind::AnyTarget => {
                                "target must be a player, creature, planeswalker, or battle"
                            }
                            TargetKind::Creature => "target must be a creature on the battlefield",
                            TargetKind::AnyPlayer => "target must be a player in the game",
                            TargetKind::OpponentPlayer => {
                                if player_target_legal(&engine.state, tid) {
                                    "target must be an opponent"
                                } else {
                                    "target must be a player in the game"
                                }
                            }
                            TargetKind::AnyPermanent => {
                                "target must be a permanent on the battlefield"
                            }
                        };
                        (false, message)
                    },
                    |_| (true, ""),
                )
        }
        TargetRole::CreaturePermanent => (
            destroy_spell_target_legal(engine, tid)
                && object_targetable_by(engine, tid, caster, source),
            "target must be a creature on the battlefield",
        ),
        TargetRole::StackSpell(spell_filter) => (
            stack_spell_target_legal(&engine.state, engine.registry, tid, spell_filter),
            "target must be a spell with the required characteristics on the stack",
        ),
        TargetRole::StackAbility => (
            engine
                .state
                .stack
                .iter()
                .any(|item| item.id == tid && item.ability_text.is_some()),
            "target must be an activated or triggered ability on the stack",
        ),
        TargetRole::GraveyardCard(filter) => (
            graveyard_target_legal(engine, filter, tid, caster, source, trigger_context),
            "target must be a matching card in the correct graveyard",
        ),
    };
    if legal {
        Ok(())
    } else {
        Err(EngineError::Illegal(message))
    }
}

pub(super) fn target_role_legal_at_resolution(
    engine: &GameEngine,
    role: TargetRole<'_>,
    tid: ObjectId,
    caster: PlayerId,
    source: TargetSourceIdentity,
    trigger_context: TriggerContext,
) -> bool {
    target_role_legality_error(engine, role, tid, caster, source, trigger_context).is_ok()
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
    if let Some(branches) = &filter.any_of {
        return branches.iter().any(|branch| {
            attachment_filter_legal(
                engine,
                branch,
                recipient,
                attachment_id,
                attachment_controller,
            )
        });
    }
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
    if object.zone != Zone::Battlefield || object.phase_status.is_some() {
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
        && target_owner_matches(&engine.state, filter.owner, attachment_controller, oid)
        && attachment_protection_legal(engine, recipient, attachment_id)
        && !object_is_excluded(
            &engine.state,
            &filter.excluded_objects,
            oid,
            TargetSourceIdentity::current(engine, attachment_id),
            TriggerContext::default(),
        )
        && filter_characteristics_match(engine, filter, oid)
        && target_controller_matches(
            &engine.state,
            filter.controller,
            attachment_controller,
            characteristics.controller,
            None,
        )
}

pub(super) fn attachment_protection_legal(
    engine: &GameEngine,
    recipient: AttachmentRecipient,
    attachment_id: ObjectId,
) -> bool {
    let AttachmentRecipient::Object(recipient_id) = recipient else {
        return true;
    };
    let Some(recipient) = engine.characteristics(recipient_id) else {
        return false;
    };
    if recipient.protections.is_empty() {
        return true;
    }
    let Some(attachment) = engine.characteristics(attachment_id) else {
        return false;
    };
    !recipient
        .protections
        .iter()
        .copied()
        .any(|protection| protection.matches(&attachment.colors, &attachment.types))
}

/// Current legality for an Equipment already on the battlefield to equip one creature. Target
/// legality is deliberately separate: Vow to Erebor makes an untargeted resolution choice, while
/// Swordsman's ability targets through its own source and can still fail to attach because of the
/// Equipment's characteristics or the recipient's protection.
pub(super) fn equipment_attachment_legal(
    engine: &GameEngine,
    equipment_id: ObjectId,
    recipient_id: ObjectId,
) -> bool {
    if equipment_id == recipient_id {
        return false;
    }
    let Some(equipment_object) = engine.state.objects.get(&equipment_id) else {
        return false;
    };
    let Some(recipient_object) = engine.state.objects.get(&recipient_id) else {
        return false;
    };
    if equipment_object.zone != Zone::Battlefield
        || equipment_object.phase_status.is_some()
        || recipient_object.zone != Zone::Battlefield
        || recipient_object.phase_status.is_some()
    {
        return false;
    }
    let Some(equipment) = engine.characteristics(equipment_id) else {
        return false;
    };
    let Some(recipient) = engine.characteristics(recipient_id) else {
        return false;
    };
    equipment.has_type("Equipment")
        // Reconfigure is not modeled. CR 301.5c therefore makes every currently supported
        // creature Equipment unable to equip a creature.
        && !equipment.is_creature()
        && recipient.is_creature()
        && attachment_protection_legal(
            engine,
            AttachmentRecipient::Object(recipient_id),
            equipment_id,
        )
}

/// Convert a validated Aura target into explicit attachment identity. Mixed AnyTarget Auras are
/// rejected by card-data validation, so the filter kind unambiguously owns the conversion.
pub(super) fn attachment_recipient_for_target(
    filter: &TargetFilter,
    target: ObjectId,
) -> Option<AttachmentRecipient> {
    if filter.all_terminal_filters_match(|leaf| {
        matches!(leaf.kind, TargetKind::Creature | TargetKind::AnyPermanent)
    }) {
        Some(AttachmentRecipient::Object(target))
    } else if filter.all_terminal_filters_match(|leaf| {
        matches!(
            leaf.kind,
            TargetKind::AnyPlayer | TargetKind::OpponentPlayer
        )
    }) {
        Some(AttachmentRecipient::Player(target as PlayerId))
    } else {
        None
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
    if let Some(branches) = &filter.any_of {
        return branches
            .iter()
            .any(|branch| object_matches_mass_filter(engine, oid, branch));
    }
    let Some(o) = engine.state.objects.get(&oid) else {
        return false;
    };
    if o.zone != Zone::Battlefield || o.phase_status.is_some() {
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

/// Mass-selection predicate with a reference player for one-shot effects whose filter carries
/// controller-relative leaves. Recursion keeps each branch's controller and characteristics
/// correlated.
pub(super) fn object_matches_scoped_mass_filter(
    engine: &GameEngine,
    oid: ObjectId,
    filter: &TargetFilter,
    reference_player: PlayerId,
) -> bool {
    if let Some(branches) = &filter.any_of {
        return branches.iter().any(|branch| {
            object_matches_scoped_mass_filter(engine, oid, branch, reference_player)
        });
    }
    let Some(characteristics) = engine.characteristics(oid) else {
        return false;
    };
    object_matches_mass_filter(engine, oid, filter)
        && target_owner_matches(&engine.state, filter.owner, reference_player, oid)
        && target_controller_matches(
            &engine.state,
            filter.controller,
            reference_player,
            characteristics.controller,
            None,
        )
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

/// Legality for an untargeted battlefield-permanent choice made during resolution. This shares
/// characteristic, controller, and source-exclusion predicates with targeting, but deliberately
/// ignores hexproof, shroud, and protection because the instruction does not target (CR 115.1).
pub(super) fn permanent_choice_filter_legal(
    engine: &GameEngine,
    filter: &TargetFilter,
    oid: ObjectId,
    chooser: PlayerId,
    source: TargetSourceIdentity,
    trigger_context: TriggerContext,
) -> bool {
    permanent_choice_filter_legal_inner(
        engine,
        filter,
        oid,
        chooser,
        source,
        trigger_context,
        false,
    )
}

/// Legality for a permanent in a departed targeted player's captured control cohort. Cohort
/// membership supplies the former controller predicate; all other current choice restrictions
/// still apply.
pub(super) fn permanent_choice_filter_legal_for_control_cohort(
    engine: &GameEngine,
    filter: &TargetFilter,
    oid: ObjectId,
    chooser: PlayerId,
    source: TargetSourceIdentity,
    trigger_context: TriggerContext,
) -> bool {
    permanent_choice_filter_legal_inner(engine, filter, oid, chooser, source, trigger_context, true)
}

fn permanent_choice_filter_legal_inner(
    engine: &GameEngine,
    filter: &TargetFilter,
    oid: ObjectId,
    chooser: PlayerId,
    source: TargetSourceIdentity,
    trigger_context: TriggerContext,
    controller_is_in_cohort: bool,
) -> bool {
    if let Some(branches) = &filter.any_of {
        return branches.iter().any(|branch| {
            permanent_choice_filter_legal_inner(
                engine,
                branch,
                oid,
                chooser,
                source,
                trigger_context,
                controller_is_in_cohort,
            )
        });
    }
    if filter.is_player()
        || !matches!(filter.kind, TargetKind::Creature | TargetKind::AnyPermanent)
        || !any_battlefield_permanent_target_legal(&engine.state, oid)
        || object_is_excluded(
            &engine.state,
            &filter.excluded_objects,
            oid,
            source,
            trigger_context,
        )
        || !filter_characteristics_match(engine, filter, oid)
    {
        return false;
    }
    let Some(characteristics) = engine.characteristics(oid) else {
        return false;
    };
    target_owner_matches(&engine.state, filter.owner, chooser, oid)
        && (controller_is_in_cohort
            || target_controller_matches(
                &engine.state,
                filter.controller,
                chooser,
                characteristics.controller,
                trigger_context.defending_player,
            ))
}

fn target_filter_legal_with_context(
    engine: &GameEngine,
    filter: &TargetFilter,
    tid: ObjectId,
    caster: PlayerId,
    source: TargetSourceIdentity,
    trigger_context: TriggerContext,
) -> bool {
    target_filter_legal_for_ref(
        engine,
        filter,
        &rv1::TargetRef {
            object_id: tid,
            ..Default::default()
        },
        caster,
        source,
        trigger_context,
    )
}

fn target_filter_legal_for_ref(
    engine: &GameEngine,
    filter: &TargetFilter,
    target: &rv1::TargetRef,
    caster: PlayerId,
    source: TargetSourceIdentity,
    trigger_context: TriggerContext,
) -> bool {
    let tid = target.object_id;
    if let Some(branches) = &filter.any_of {
        return branches.iter().any(|branch| {
            target_filter_legal_for_ref(engine, branch, target, caster, source, trigger_context)
        });
    }
    let target_is_player = match target.kind() {
        rv1::TargetRefKind::Player => true,
        rv1::TargetRefKind::Permanent => false,
        rv1::TargetRefKind::Unspecified => {
            filter.is_player()
                || (filter.kind == TargetKind::AnyTarget
                    && engine.state.player_idx(tid as i32).is_some())
        }
        _ => return false,
    };
    let kind_ok = if target_is_player {
        player_target_legal(&engine.state, tid)
            && match filter.kind {
                TargetKind::AnyTarget | TargetKind::AnyPlayer => true,
                TargetKind::OpponentPlayer => engine.state.are_opponents(tid as PlayerId, caster),
                _ => false,
            }
    } else {
        match filter.kind {
            TargetKind::AnyTarget => {
                any_battlefield_permanent_target_legal(&engine.state, tid)
                    && engine.characteristics(tid).is_some_and(|value| {
                        value.is_creature()
                            || value.has_type("Planeswalker")
                            || value.has_type("Battle")
                    })
            }
            TargetKind::Creature => destroy_spell_target_legal(engine, tid),
            TargetKind::AnyPermanent => any_battlefield_permanent_target_legal(&engine.state, tid),
            TargetKind::AnyPlayer | TargetKind::OpponentPlayer => false,
        }
    };
    if !kind_ok {
        return false;
    }
    // Characteristic filters — only apply to non-player targets. `is_player()` covers the
    // player-only kinds; `AnyTarget` is decided per target, since the same filter accepts both a
    // creature and a player (Lightning Bolt) and a player carries no characteristics.
    if !target_is_player
        && object_is_excluded(
            &engine.state,
            &filter.excluded_objects,
            tid,
            source,
            trigger_context,
        )
    {
        return false;
    }
    if !filter.is_player() && !target_is_player {
        // CR 702.16/702.18 — targeting only; the untargeted mass path deliberately skips this.
        if !object_targetable_by(engine, tid, caster, source) {
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
        if !target_owner_matches(&engine.state, filter.owner, caster, tid)
            || !target_controller_matches(
                &engine.state,
                filter.controller,
                caster,
                characteristics.controller,
                trigger_context.defending_player,
            )
        {
            return false;
        }
    }
    true
}

/// CR 701.6/707.10: legality of `tid` as the target of a counter/copy spell. The object must be a
/// spell on the stack (not an activated/triggered ability). Type and mana-value restrictions are
/// evaluated against the selected face and its announced X value. The default accepts any spell.
pub(super) fn stack_spell_target_legal(
    state: &GameState,
    registry: &'static CardRegistry,
    tid: ObjectId,
    spell_filter: &StackSpellFilter,
) -> bool {
    let Some(item) = state
        .stack
        .iter()
        .find(|s| s.id == tid && s.ability_text.is_none())
    else {
        return false;
    };
    if spell_filter.is_unrestricted() {
        return true;
    }
    if let Some(branches) = &spell_filter.any_of {
        return branches
            .iter()
            .any(|branch| stack_spell_target_legal(state, registry, tid, branch));
    }
    let Some(printed_face) = registry
        .get(&item.card_id)
        .and_then(|d| d.face(item.face_index))
    else {
        return false;
    };
    let prototype_face;
    let face = if item.prototyped {
        let Some(selected) = printed_face.prototyped_face() else {
            return false;
        };
        prototype_face = selected;
        &prototype_face
    } else {
        printed_face
    };
    let current_characteristics = super::characteristics::spell_cast_characteristics(
        state,
        registry,
        item.id,
        item.controller,
        face,
    );
    if spell_filter
        .card_type
        .is_some_and(|filter| !stack_characteristics_match_type(&current_characteristics, filter))
    {
        return false;
    }
    if spell_filter.is_color.is_some_and(|color| {
        super::characteristics::stack_spell_colors(state, registry, item)
            .is_none_or(|colors| !colors.contains(&color))
    }) {
        return false;
    }
    let mana_value = face.mana_cost.mana_value_on_stack(item.chosen_x);
    spell_filter
        .min_mana_value
        .is_none_or(|minimum| mana_value >= minimum)
        && spell_filter
            .max_mana_value
            .is_none_or(|maximum| mana_value <= maximum)
}

pub(super) fn stack_characteristics_match_type(
    characteristics: &super::characteristics::Characteristics,
    filter: CardTypeFilter,
) -> bool {
    let has_type = |card_type: &str| characteristics.has_type(card_type);
    let is_land = has_type("Land");
    match filter {
        CardTypeFilter::BasicLand => {
            is_land
                && characteristics
                    .supertypes
                    .iter()
                    .any(|value| value == "Basic")
        }
        CardTypeFilter::Land => is_land,
        CardTypeFilter::Enchantment => has_type("Enchantment"),
        CardTypeFilter::Instant => has_type("Instant"),
        CardTypeFilter::Sorcery => has_type("Sorcery"),
        CardTypeFilter::InstantOrSorcery => has_type("Instant") || has_type("Sorcery"),
        CardTypeFilter::Creature => has_type("Creature"),
        CardTypeFilter::Artifact => has_type("Artifact"),
        CardTypeFilter::Planeswalker => has_type("Planeswalker"),
        CardTypeFilter::Battle => has_type("Battle"),
        CardTypeFilter::Nonland => !is_land,
        CardTypeFilter::NonlandPermanent => {
            !is_land
                && [
                    "Artifact",
                    "Battle",
                    "Creature",
                    "Enchantment",
                    "Planeswalker",
                ]
                .into_iter()
                .any(has_type)
        }
        CardTypeFilter::Noncreature => !has_type("Creature"),
    }
}

pub(super) fn effect_has_legal_target_at_resolution(
    engine: &GameEngine,
    effect: &SpellEffectKind,
    targets: &[ObjectId],
    caster: PlayerId,
    source: TargetSourceIdentity,
) -> bool {
    let roles = effect.target_roles();
    if roles.is_empty() {
        return true;
    }
    targets.iter().any(|&target| {
        roles.iter().any(|&role| {
            target_role_legal_at_resolution(
                engine,
                role,
                target,
                caster,
                source,
                TriggerContext::default(),
            )
        })
    })
}

pub(super) fn x_target_group_indices(
    effects: &[SpellEffectKind],
    targeting: Option<&TargetingDef>,
    program: Option<&CompiledProgram>,
) -> std::collections::HashSet<usize> {
    // The authored program operation whitelist has no X-dependent target leaf. Program owners
    // can still have ordinary target groups, whose empty legacy effect slice cannot be compiled
    // through the legacy schema just to search for a CopyTargetArtifactWithManaValueX role.
    if effects.is_empty() && program.is_some() {
        return std::collections::HashSet::new();
    }
    target_schema(effects, targeting)
        .groups
        .iter()
        .enumerate()
        .filter_map(|(group_index, group)| {
            group
                .bindings
                .iter()
                .any(|binding| {
                    matches!(
                        effects.get(binding.effect_index),
                        Some(SpellEffectKind::CopyTargetArtifactWithManaValueX { .. })
                    )
                })
                .then_some(group_index)
        })
        .collect()
}

pub(super) fn validate_x_target_mana_value(
    engine: &GameEngine,
    effects: &[SpellEffectKind],
    targeting: Option<&TargetingDef>,
    targets: &[rv1::TargetRef],
    x_value: u32,
    program: Option<&CompiledProgram>,
) -> Result<(), EngineError> {
    for group_index in x_target_group_indices(effects, targeting, program) {
        let mut selected = targets
            .iter()
            .filter(|target| target.group_index as usize == group_index);
        let Some(target) = selected.next() else {
            return Err(EngineError::Illegal("X-bound target is missing"));
        };
        if selected.next().is_some()
            || engine
                .characteristics(target.object_id)
                .is_none_or(|characteristics| characteristics.mana_value != x_value)
        {
            return Err(EngineError::Illegal(
                "X must equal the target artifact's current mana value",
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_ability_targets_with_context_and_program(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
    targets: &[rv1::TargetRef],
    target_context: TargetResolutionContext<'_>,
) -> Result<(), EngineError> {
    validate_grouped_targets(engine, caster, source, targets, target_context)
}

#[cfg(test)]
pub(super) fn validate_spell_targets(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
    effects: &[SpellEffectKind],
    targeting: Option<&TargetingDef>,
    targets: &[rv1::TargetRef],
) -> Result<(), EngineError> {
    validate_spell_targets_with_program(engine, caster, source, effects, targeting, targets, None)
}

pub(super) fn validate_spell_targets_with_program(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
    effects: &[SpellEffectKind],
    targeting: Option<&TargetingDef>,
    targets: &[rv1::TargetRef],
    program: Option<&CompiledProgram>,
) -> Result<(), EngineError> {
    validate_grouped_targets(
        engine,
        caster,
        source,
        targets,
        TargetResolutionContext {
            effects,
            targeting,
            trigger_context: TriggerContext::default(),
            program,
        },
    )
}

fn target_legality_error_for_binding(
    engine: &GameEngine,
    role: TargetRole<'_>,
    target: &rv1::TargetRef,
    caster: PlayerId,
    source: TargetSourceIdentity,
    trigger_context: TriggerContext,
) -> Result<(), EngineError> {
    target_ref_role_legality_error(engine, role, target, caster, source, trigger_context)
}

fn counter_allocation_for_schema(
    effects: &[SpellEffectKind],
    schema: &TargetSchema<'_, '_>,
) -> Option<rv1::CounterAllocation> {
    schema
        .groups
        .iter()
        .enumerate()
        .find_map(|(group_index, group)| {
            group.bindings.iter().find_map(|binding| {
                if let Some(SpellEffectKind::DistributeCounters { count, .. }) =
                    effects.get(binding.effect_index)
                {
                    Some(rv1::CounterAllocation {
                        fixed_total: *count,
                        group_index: group_index as u32,
                    })
                } else {
                    None
                }
            })
        })
}

fn validate_counter_allocation(
    engine: &GameEngine,
    targets: &[rv1::TargetRef],
    allocation: Option<&rv1::CounterAllocation>,
) -> Result<(), EngineError> {
    let Some(allocation) = allocation else {
        return if targets.iter().any(|target| target.counter_amount != 0) {
            Err(EngineError::Illegal(
                "this action has no counter allocation",
            ))
        } else {
            Ok(())
        };
    };
    let mut total = 0u32;
    let mut selected = 0usize;
    for target in targets {
        if target.group_index != allocation.group_index {
            if target.counter_amount != 0 {
                return Err(EngineError::Illegal(
                    "counter quantity belongs to a different target group",
                ));
            }
            continue;
        }
        if target.kind != rv1::TargetRefKind::Permanent as i32
            || target.damage_amount != 0
            || target.counter_amount == 0
            || target.expected_zone_change_generation.is_none()
        {
            return Err(EngineError::Illegal(
                "counter allocation requires positive quantities and exact permanent generations",
            ));
        }
        let expected = target
            .expected_zone_change_generation
            .expect("checked above");
        if !engine
            .state
            .objects
            .get(&target.object_id)
            .is_some_and(|object| {
                object.zone == Zone::Battlefield
                    && engine
                        .state
                        .zone_change_generation
                        .get(&target.object_id)
                        .copied()
                        .unwrap_or(0)
                        == expected
            })
        {
            return Err(EngineError::Illegal(
                "counter allocation target generation is stale",
            ));
        }
        total = total
            .checked_add(target.counter_amount)
            .ok_or(EngineError::Illegal("counter allocation total overflowed"))?;
        selected += 1;
    }
    if selected > 0 && total != allocation.fixed_total {
        return Err(EngineError::Illegal(
            "counter allocation does not equal the fixed total",
        ));
    }
    Ok(())
}

fn validate_grouped_targets(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
    targets: &[rv1::TargetRef],
    target_context: TargetResolutionContext<'_>,
) -> Result<(), EngineError> {
    validate_grouped_targets_at(engine, caster, source, targets, target_context, None)
}

/// Validate one complete announcement group against the same typed target rules as an atomic
/// activation. Earlier accepted receipts participate in distinctness, not fresh announcements.
pub(super) fn validate_ability_announcement_group(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
    targets: &[rv1::TargetRef],
    target_context: TargetResolutionContext<'_>,
    group_index: usize,
) -> Result<(), EngineError> {
    validate_grouped_targets_at(
        engine,
        caster,
        source,
        targets,
        target_context,
        Some(group_index),
    )
}

/// Candidate publication must use the declared target domain, especially when a player's
/// numeric ID is also an object's ID. The ability controller remains the legality reference.
pub(super) fn ability_announcement_target_is_legal(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
    target: &rv1::TargetRef,
    target_context: TargetResolutionContext<'_>,
) -> bool {
    let (schema, _) = runtime_target_schema(
        engine,
        caster,
        target_context.effects,
        target_context.targeting,
        target_context.program,
    );
    schema
        .groups
        .get(target.group_index as usize)
        .is_some_and(|group| {
            target_ref_domain_exists(engine, target)
                && target
                    .expected_zone_change_generation
                    .is_none_or(|generation| {
                        target.kind != rv1::TargetRefKind::Player as i32
                            && engine
                                .state
                                .zone_change_generation
                                .get(&target.object_id)
                                .copied()
                                .unwrap_or(0)
                                == generation
                    })
                && group.bindings.iter().all(|binding| {
                    target_legality_error_for_binding(
                        engine,
                        binding.role,
                        target,
                        caster,
                        source,
                        target_context.trigger_context,
                    )
                    .is_ok()
                })
        })
}

fn validate_grouped_targets_at(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
    targets: &[rv1::TargetRef],
    target_context: TargetResolutionContext<'_>,
    only_group: Option<usize>,
) -> Result<(), EngineError> {
    let (schema, controllers) = runtime_target_schema(
        engine,
        caster,
        target_context.effects,
        target_context.targeting,
        target_context.program,
    );
    let allocation = counter_allocation_for_schema(target_context.effects, &schema);
    if only_group.is_none() {
        validate_counter_allocation(engine, targets, allocation.as_ref())?;
    }
    if only_group.is_some_and(|index| index >= schema.groups.len()) {
        return Err(EngineError::Illegal("unknown announcement target group"));
    }
    for target in targets {
        let fixed_poison = schema
            .groups
            .get(target.group_index as usize)
            .is_some_and(|group| {
                group.bindings.iter().any(|binding| {
                    matches!(
                        target_context.effects.get(binding.effect_index),
                        Some(SpellEffectKind::PutPoisonCountersOnTargetPlayer { .. })
                    )
                })
            });
        if fixed_poison
            && (target.kind != rv1::TargetRefKind::Player as i32
                || target.expected_zone_change_generation.is_some()
                || target.damage_amount != 0
                || target.counter_amount != 0)
        {
            return Err(EngineError::Illegal(
                "fixed poison announcement requires only a player target",
            ));
        }
    }
    if schema.groups.is_empty() {
        return if targets.is_empty() {
            Ok(())
        } else {
            Err(EngineError::Illegal("this action takes no targets"))
        };
    }
    if targets
        .iter()
        .any(|target| target.group_index as usize >= schema.groups.len())
    {
        return Err(EngineError::Illegal("target references an unknown group"));
    }
    let grouped = schema
        .groups
        .iter()
        .enumerate()
        .map(|(group_index, _)| {
            targets
                .iter()
                .filter(|target| target.group_index as usize == group_index)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    for (group_index, group) in schema.groups.iter().enumerate() {
        if only_group.is_some_and(|index| index != group_index) {
            continue;
        }
        let selected = &grouped[group_index];
        if selected.len() < group.min as usize || selected.len() > group.max as usize {
            if target_context.targeting.is_none() && group.min == 1 && group.max == 1 {
                return Err(EngineError::Illegal("spell requires exactly one target"));
            }
            return Err(EngineError::Illegal("target group cardinality is invalid"));
        }
        let mut seen = std::collections::HashSet::new();
        let mut graveyard_owner = None;
        for target in selected {
            if let Some(required) = controllers.get(group_index) {
                if target.kind != rv1::TargetRefKind::Permanent as i32
                    || target.expected_zone_change_generation.is_none()
                    || !required_target_controller_is_current(engine, target.object_id, *required)
                {
                    return Err(EngineError::Illegal("per-opponent target requires its bound controller and exact permanent generation"));
                }
            }
            if !target_ref_domain_exists(engine, target) {
                return Err(EngineError::Illegal(
                    "target kind does not match the referenced game object",
                ));
            }
            if !seen.insert(target_ref_identity(engine, target)) {
                return Err(EngineError::Illegal("duplicate target in target group"));
            }
            if target
                .expected_zone_change_generation
                .is_some_and(|expected| {
                    engine
                        .state
                        .zone_change_generation
                        .get(&target.object_id)
                        .copied()
                        .unwrap_or(0)
                        != expected
                })
            {
                return Err(EngineError::Illegal("target generation is stale"));
            }
            if group.same_graveyard {
                let owner = engine
                    .state
                    .objects
                    .get(&target.object_id)
                    .ok_or(EngineError::Illegal("graveyard target does not exist"))?
                    .owner;
                if graveyard_owner
                    .replace(owner)
                    .is_some_and(|first| first != owner)
                {
                    return Err(EngineError::Illegal(
                        "targets must be cards from the same graveyard",
                    ));
                }
            }
            for binding in &group.bindings {
                if matches!(
                    target_context.effects.get(binding.effect_index),
                    Some(SpellEffectKind::CopyTargetArtifactWithManaValueX { .. })
                ) && target.expected_zone_change_generation.is_none()
                {
                    return Err(EngineError::Illegal(
                        "target generation is required for this ability",
                    ));
                }
                target_legality_error_for_binding(
                    engine,
                    binding.role,
                    target,
                    caster,
                    source,
                    target_context.trigger_context,
                )?;
            }
        }
        for other_index in (0..schema.groups.len()).filter(|other| {
            group.distinct_from.contains(&(*other as u32))
                || schema.groups[*other]
                    .distinct_from
                    .contains(&(group_index as u32))
        }) {
            let other = grouped.get(other_index).ok_or(EngineError::Illegal(
                "target group distinctness index is invalid",
            ))?;
            if selected.iter().any(|target| {
                other.iter().any(|candidate| {
                    target_ref_identity(engine, candidate) == target_ref_identity(engine, target)
                })
            }) {
                return Err(EngineError::Illegal(
                    "target must be distinct across target groups",
                ));
            }
        }
    }
    for (effect_index, effect) in target_context.effects.iter().enumerate() {
        if only_group.is_none() && matches!(effect, SpellEffectKind::ExchangeArtifactWithGraveyard)
        {
            let (first, second) = schema
                .artifact_exchange_groups(effect_index)
                .expect("registry validated artifact exchange roles");
            let departure = grouped[first][0].object_id;
            let incoming = grouped[second][0].object_id;
            if artifact_exchange_player(engine, departure, incoming).is_none() {
                return Err(EngineError::Illegal(
                    "artifact targets must belong to the same player",
                ));
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TargetRefIdentity {
    Player(PlayerId),
    Object(ObjectId, u64),
}

/// CR 400.7 and CR 115.3: an object id alone is not a physical target identity after a zone
/// change, and its integer can also equal a PlayerId. Typed receipts keep those domains separate.
fn target_ref_identity(engine: &GameEngine, target: &rv1::TargetRef) -> TargetRefIdentity {
    match rv1::TargetRefKind::try_from(target.kind) {
        Ok(rv1::TargetRefKind::Player) => TargetRefIdentity::Player(target.object_id as PlayerId),
        Ok(
            rv1::TargetRefKind::Permanent
            | rv1::TargetRefKind::Stack
            | rv1::TargetRefKind::Graveyard,
        ) => TargetRefIdentity::Object(
            target.object_id,
            target.expected_zone_change_generation.unwrap_or_else(|| {
                engine
                    .state
                    .zone_change_generation
                    .get(&target.object_id)
                    .copied()
                    .unwrap_or(0)
            }),
        ),
        Ok(rv1::TargetRefKind::Unspecified) | Err(_) => {
            if target.expected_zone_change_generation.is_none()
                && engine
                    .state
                    .player_idx(target.object_id as PlayerId)
                    .is_some()
            {
                TargetRefIdentity::Player(target.object_id as PlayerId)
            } else {
                TargetRefIdentity::Object(
                    target.object_id,
                    target.expected_zone_change_generation.unwrap_or_else(|| {
                        engine
                            .state
                            .zone_change_generation
                            .get(&target.object_id)
                            .copied()
                            .unwrap_or(0)
                    }),
                )
            }
        }
    }
}

/// Welder's matched player is the current battlefield controller and graveyard owner.
/// Role legality and generation checks are separately required at each caller boundary.
pub(super) fn artifact_exchange_player(
    engine: &GameEngine,
    departure: ObjectId,
    incoming: ObjectId,
) -> Option<PlayerId> {
    if engine.state.objects.get(&departure)?.zone != Zone::Battlefield
        || engine.state.objects.get(&incoming)?.zone != Zone::Graveyard
    {
        return None;
    }
    let controller = engine.characteristics(departure)?.controller;
    (engine.state.objects[&incoming].owner == controller
        && !engine.state.players[engine.state.player_idx(controller)?].has_lost)
        .then_some(controller)
}

fn target_ref_domain_exists(engine: &GameEngine, target: &rv1::TargetRef) -> bool {
    // Unknown wire values must not bypass typed target observers such as Crime. The explicit
    // Unspecified value remains supported for callers without presentation metadata.
    let Ok(kind) = rv1::TargetRefKind::try_from(target.kind) else {
        return false;
    };
    match kind {
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

pub(super) fn legal_target_group_has_minimum(
    state: &GameState,
    group: &rv1::LegalTargetGroup,
) -> bool {
    if group.same_graveyard {
        let mut counts = std::collections::HashMap::<PlayerId, u32>::new();
        for object_id in &group.valid_graveyard_ids {
            if let Some(owner) = state.objects.get(object_id).map(|object| object.owner) {
                *counts.entry(owner).or_default() += 1;
            }
        }
        return group.min == 0 || counts.values().any(|count| *count >= group.min);
    }
    let player_count = u32::from(group.can_target_self) + u32::from(group.can_target_opponent);
    group.min
        <= group.valid_permanent_ids.len() as u32
            + group.valid_stack_ids.len() as u32
            + group.valid_graveyard_ids.len() as u32
            + player_count
}

/// Compute `SpellTargets` for a spell — which objects/players `caster` can legally send this set
/// of effects at, given the current game state. Only effects that need a target are considered;
/// non-targeted effects in the same spell are ignored.
#[cfg(test)]
pub(super) fn compute_spell_targets(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
    effects: &[SpellEffectKind],
    targeting: Option<&TargetingDef>,
    cost_modifiers: &[SpellCostModifier],
) -> rv1::SpellTargets {
    compute_spell_targets_with_program(
        engine,
        caster,
        source,
        effects,
        targeting,
        cost_modifiers,
        None,
    )
}

pub(super) fn compute_spell_targets_with_program(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
    effects: &[SpellEffectKind],
    targeting: Option<&TargetingDef>,
    cost_modifiers: &[SpellCostModifier],
    program: Option<&CompiledProgram>,
) -> rv1::SpellTargets {
    let mut targets = compute_targets_with_context(
        engine,
        caster,
        source,
        TargetResolutionContext {
            effects,
            targeting,
            trigger_context: TriggerContext::default(),
            program,
        },
        Some(TargetingCostAction::Spells),
        true,
    );
    targets.targeted_cost_reduction_applications = engine.targeted_cost_reduction_applications(
        caster,
        source,
        cost_modifiers,
        &targets.groups,
    );
    targets
}

pub(super) fn compute_ability_targets_with_program(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
    effects: &[SpellEffectKind],
    targeting: Option<&TargetingDef>,
    program: Option<&CompiledProgram>,
) -> rv1::SpellTargets {
    compute_targets_with_context(
        engine,
        caster,
        source,
        TargetResolutionContext {
            effects,
            targeting,
            trigger_context: TriggerContext::default(),
            program,
        },
        Some(TargetingCostAction::ActivatedAbilities),
        true,
    )
}

#[cfg(test)]
pub(super) fn compute_ability_targets_with_context(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
    effects: &[SpellEffectKind],
    targeting: Option<&TargetingDef>,
    trigger_context: TriggerContext,
) -> rv1::SpellTargets {
    compute_ability_targets_with_context_and_program(
        engine,
        caster,
        source,
        effects,
        targeting,
        trigger_context,
        None,
    )
}

pub(super) fn compute_ability_targets_with_context_and_program(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
    effects: &[SpellEffectKind],
    targeting: Option<&TargetingDef>,
    trigger_context: TriggerContext,
    program: Option<&CompiledProgram>,
) -> rv1::SpellTargets {
    compute_targets_with_context(
        engine,
        caster,
        source,
        TargetResolutionContext {
            effects,
            targeting,
            trigger_context,
            program,
        },
        None,
        true,
    )
}

/// Retargeting a copy may retain an illegal original target. Offer each currently legal role
/// independently; the completed target vector validates any relationships between them.
pub(super) fn compute_copy_targets_with_context_and_program(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
    effects: &[SpellEffectKind],
    targeting: Option<&TargetingDef>,
    trigger_context: TriggerContext,
    program: Option<&CompiledProgram>,
) -> rv1::SpellTargets {
    compute_targets_with_context(
        engine,
        caster,
        source,
        TargetResolutionContext {
            effects,
            targeting,
            trigger_context,
            program,
        },
        None,
        false,
    )
}

fn compute_targets_with_context(
    engine: &GameEngine,
    caster: PlayerId,
    source: TargetSourceIdentity,
    target_context: TargetResolutionContext<'_>,
    targeting_cost_action: Option<TargetingCostAction>,
    prune_unpaired_artifact_targets: bool,
) -> rv1::SpellTargets {
    // DamageTargets metadata controls the allocation UI. Cardinality lives exclusively on groups.
    let mut fixed_damage: u32 = 0;
    let mut is_damage_targets = false;
    let mut extra_mana_per_target: u32 = 0;
    let mut damage_division = rv1::DamageDivision::ChooseAtCast;
    for effect in target_context.effects {
        if let SpellEffectKind::DamageTargets {
            amount,
            extra_mana_per_target: empt,
            division,
            ..
        } = effect
        {
            is_damage_targets = true;
            // Resolving with X=0 gives the fixed amount for literal amounts; X amounts become 0
            // (the client will use the player's chosen x_value instead).
            fixed_damage = amount.resolve_unconditional(0).unwrap_or(0);
            extra_mana_per_target = *empt;
            // Only ChooseAtCast needs allocation UI (CR 601.2d). Other modes determine
            // their amounts in the engine at resolution.
            damage_division = match division {
                DamageDivision::ChooseAtCast => rv1::DamageDivision::ChooseAtCast,
                DamageDivision::EvenAtResolution => rv1::DamageDivision::EvenAtResolution,
                DamageDivision::PerTarget => rv1::DamageDivision::PerTarget,
            };
        }
    }

    let (schema, controllers) = runtime_target_schema(
        engine,
        caster,
        target_context.effects,
        target_context.targeting,
        target_context.program,
    );
    let mut groups = schema
        .groups
        .iter()
        .enumerate()
        .map(|(group_index, group)| {
            let legal = |object_id| {
                required_target_controller_is_current(
                    engine,
                    object_id,
                    controllers.get(group_index).copied().flatten(),
                ) && group.bindings.iter().all(|binding| {
                    target_role_legality_error(
                        engine,
                        binding.role,
                        object_id,
                        caster,
                        source,
                        target_context.trigger_context,
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
            let x_target_choices = group
                .bindings
                .iter()
                .any(|binding| {
                    matches!(
                        target_context.effects.get(binding.effect_index),
                        Some(SpellEffectKind::CopyTargetArtifactWithManaValueX { .. })
                    )
                })
                .then(|| {
                    let mut choices =
                        std::collections::BTreeMap::<u32, Vec<rv1::XTargetCandidate>>::new();
                    for &object_id in &permanent_ids {
                        let Some(characteristics) = engine.characteristics(object_id) else {
                            continue;
                        };
                        let generation = engine
                            .state
                            .zone_change_generation
                            .get(&object_id)
                            .copied()
                            .unwrap_or(0);
                        choices.entry(characteristics.mana_value).or_default().push(
                            rv1::XTargetCandidate {
                                object_id,
                                zone_change_generation: generation,
                            },
                        );
                    }
                    rv1::XTargetChoices {
                        choices: choices
                            .into_iter()
                            .map(|(x_value, candidates)| rv1::XTargetChoice {
                                x_value,
                                candidates,
                            })
                            .collect(),
                    }
                });
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
                chosen_by_opponent: group.chooser
                    == tricerules_card_model::primitives::TargetChooser::ChosenOpponent,
                group_index: group_index as u32,
                prompt_text: group.prompt.to_string(),
                min: group.min,
                max: group.max,
                valid_permanent_ids: permanent_ids,
                valid_stack_ids: stack_ids,
                can_target_self: self_legal,
                can_target_opponent: opponent_legal,
                valid_graveyard_ids: graveyard_ids,
                valid_permanent_targets: Vec::new(),
                distinct_from_group_indices: group.distinct_from.to_vec(),
                same_graveyard: group.same_graveyard,
                pair_constraint: None,
                x_target_choices,
            }
        })
        .collect::<Vec<_>>();

    for (effect_index, effect) in target_context.effects.iter().enumerate() {
        if !prune_unpaired_artifact_targets
            || !matches!(effect, SpellEffectKind::ExchangeArtifactWithGraveyard)
        {
            continue;
        }
        let (first, second) = schema
            .artifact_exchange_groups(effect_index)
            .expect("registry validated artifact exchange roles");
        let mut pairs = Vec::new();
        for &departure in &groups[first].valid_permanent_ids {
            for &incoming in &groups[second].valid_graveyard_ids {
                if artifact_exchange_player(engine, departure, incoming).is_some() {
                    pairs.push(rv1::TargetPairChoice {
                        prior_target: Some(rv1::TargetCandidateRef {
                            kind: rv1::TargetRefKind::Permanent as i32,
                            object_id: departure,
                        }),
                        candidate: Some(rv1::TargetCandidateRef {
                            kind: rv1::TargetRefKind::Graveyard as i32,
                            object_id: incoming,
                        }),
                    });
                }
            }
        }
        groups[first].valid_permanent_ids.retain(|oid| {
            pairs.iter().any(|pair| {
                pair.prior_target
                    .as_ref()
                    .is_some_and(|target| target.object_id == *oid)
            })
        });
        groups[second].valid_graveyard_ids.retain(|oid| {
            pairs.iter().any(|pair| {
                pair.candidate
                    .as_ref()
                    .is_some_and(|target| target.object_id == *oid)
            })
        });
        groups[second].pair_constraint = Some(rv1::TargetPairConstraint {
            prior_group_index: first as u32,
            compatible_pairs: pairs,
        });
    }

    let counter_allocation = counter_allocation_for_schema(target_context.effects, &schema);
    for (group, authored) in groups.iter_mut().zip(&schema.groups) {
        if authored
            .bindings
            .iter()
            .any(|binding| !binding.role.targets_an_object())
        {
            // A player-only role must not publish physical candidates whose numeric ID happens
            // to equal a legal PlayerId. The explicit target domain remains authoritative.
            group.valid_permanent_ids.clear();
            group.valid_permanent_targets.clear();
            group.valid_stack_ids.clear();
            group.valid_graveyard_ids.clear();
        }
    }
    if let Some(allocation) = &counter_allocation {
        let group = &mut groups[allocation.group_index as usize];
        // This profile targets creatures, even when a physical ObjectId is also a PlayerId.
        group.can_target_self = false;
        group.can_target_opponent = false;
        group.valid_permanent_targets = group
            .valid_permanent_ids
            .iter()
            .map(|&object_id| rv1::LegalPermanentTargetCandidate {
                object_id,
                zone_change_generation: engine
                    .state
                    .zone_change_generation
                    .get(&object_id)
                    .copied()
                    .unwrap_or(0),
            })
            .collect();
    }
    let targeting_cost_applications = targeting_cost_action
        .map(|action| engine.targeting_cost_applications(caster, action, &groups))
        .unwrap_or_default();
    rv1::SpellTargets {
        counter_allocation,
        fixed_damage,
        is_damage_targets,
        extra_mana_per_target,
        damage_division: damage_division as i32,
        groups,
        targeting_cost_applications,
        targeted_cost_reduction_applications: Vec::new(),
        cast_cost_requirements: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn legacy_modal_program_siblings_keep_their_legacy_target_schema() {
        let registry = tricerules_cards::registry::global();
        let face = registry
            .get("three_steps_ahead")
            .expect("Three Steps Ahead is in the generated registry")
            .primary_face();
        let modal = face.modal_spell.as_ref().expect("Spree modes");

        for mode in &modal.modes {
            let owner = ProgramOwnerId::face_mode(
                "three_steps_ahead",
                face.face_id.clone(),
                mode.mode_id.clone(),
            );
            let compiled = registry
                .compiled_program_for_owner(&owner)
                .expect("every Three Steps Ahead mode compiles under its exact authored owner");
            let schema =
                target_schema_with_program(&mode.effects, mode.targeting.as_ref(), Some(compiled));
            assert_eq!(
                schema.groups.len(),
                mode.targeting
                    .as_ref()
                    .map_or(0, |targeting| targeting.groups.len()),
                "{} keeps its legacy target-group projection",
                mode.mode_id,
            );
        }
    }

    fn issue_205_stack_item(
        id: ObjectId,
        card_id: &str,
        face_index: usize,
        chosen_x: u32,
    ) -> StackItem {
        StackItem {
            phyrexian_symbols_paid_with_life: 0,
            prototyped: false,
            mana_colors_spent_to_cast: Default::default(),
            id,
            controller: 1,
            card_id: card_id.into(),
            targets: Vec::new(),
            ability_text: None,
            source_permanent_id: None,
            source_owner: None,
            source_zone_change: 0,
            trigger_use_key: None,
            source_face_change: 0,
            ability_index: None,
            activated_ability: None,
            activated_ability_occurrence: None,
            triggered_ability: None,
            is_triggered: false,
            is_copy: false,
            face_index,
            cast_method: SpellCastMethod::Normal,
            returned_attacker_assignment: None,
            chosen_x,
            chosen_modes: Vec::new(),
            cast_condition_results: Vec::new(),
            cast_occurrence: None,
            cast_by: None,
            cast_cost_receipts: Vec::new(),
            payment_result: CardResultCohort::default(),
            exiled_cohorts: Default::default(),
            chaos_warp_owner_instructions: Default::default(),
            resolution_branch_choices: Default::default(),
            blight_receipts: Vec::new(),
            trigger_context: TriggerContext::default(),
        }
    }

    #[test]
    fn issue_205_stack_spell_mana_value_bounds_filter_targets() {
        let registry = CardRegistry::from_chunks_and_tokens(
            &[
                r#"(id: "mv_one", name: "MV One", face_id: "mv_one", mana_cost: "{U}", types: ["Instant"])"#,
                r#"(id: "mv_two", name: "MV Two", face_id: "mv_two", mana_cost: "{1}{U}", types: ["Instant"])"#,
                r#"(id: "mv_four", name: "MV Four", face_id: "mv_four", mana_cost: "{3}{U}", types: ["Instant"])"#,
                r#"(id: "x_spell", name: "X Spell", face_id: "x_spell", mana_cost: "{X}{R}", types: ["Sorcery"])"#,
                r#"(id: "split_spell", name: "Small // Large", layout: Split, faces: [(name: "Small", face_id: "small", mana_cost: "{U}", types: ["Instant"]), (name: "Large", face_id: "large", mana_cost: "{4}{U}", types: ["Instant"])])"#,
                include_str!("../../../tricerules-cards/data/skitterbeam_battalion.ron"),
            ],
            &[],
        )
        .unwrap();
        let registry: &'static CardRegistry = Box::leak(Box::new(registry));
        let decks = Some(vec![vec!["forest".into(); 7], vec!["forest".into(); 7]]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            205,
            &[0, 1],
            20,
            decks,
            true,
        )
        .unwrap();
        engine.state.stack = vec![
            issue_205_stack_item(101, "mv_one", 0, 0),
            issue_205_stack_item(102, "mv_two", 0, 0),
            issue_205_stack_item(104, "mv_four", 0, 0),
            issue_205_stack_item(105, "split_spell", 1, 0),
            issue_205_stack_item(106, "x_spell", 0, 3),
        ];

        let exact_two = StackSpellFilter {
            min_mana_value: Some(2),
            max_mana_value: Some(2),
            ..Default::default()
        };
        assert!(!stack_spell_target_legal(
            &engine.state,
            registry,
            101,
            &exact_two
        ));
        assert!(stack_spell_target_legal(
            &engine.state,
            registry,
            102,
            &exact_two
        ));
        assert!(!stack_spell_target_legal(
            &engine.state,
            registry,
            104,
            &exact_two
        ));

        let at_least_four = StackSpellFilter {
            min_mana_value: Some(4),
            ..Default::default()
        };
        assert!(!stack_spell_target_legal(
            &engine.state,
            registry,
            102,
            &at_least_four
        ));
        assert!(stack_spell_target_legal(
            &engine.state,
            registry,
            104,
            &at_least_four
        ));
        assert!(stack_spell_target_legal(
            &engine.state,
            registry,
            105,
            &at_least_four
        ));
        assert!(stack_spell_target_legal(
            &engine.state,
            registry,
            106,
            &at_least_four
        ));

        let mut copied_x_spell = engine.state.stack.last().unwrap().clone();
        copied_x_spell.id = 107;
        copied_x_spell.is_copy = true;
        engine.state.stack.push(copied_x_spell);
        assert!(stack_spell_target_legal(
            &engine.state,
            registry,
            107,
            &at_least_four
        ));

        let mut ability = issue_205_stack_item(108, "mv_four", 0, 0);
        ability.ability_text = Some("Activated ability".into());
        engine.state.stack.push(ability);
        assert!(!stack_spell_target_legal(
            &engine.state,
            registry,
            108,
            &at_least_four
        ));

        let at_most_two = StackSpellFilter {
            max_mana_value: Some(2),
            ..Default::default()
        };
        assert!(stack_spell_target_legal(
            &engine.state,
            registry,
            102,
            &at_most_two
        ));
        assert!(!stack_spell_target_legal(
            &engine.state,
            registry,
            104,
            &at_most_two
        ));

        let mut prototyped_spell = issue_205_stack_item(109, "skitterbeam_battalion", 0, 0);
        prototyped_spell.prototyped = true;
        engine.state.stack.push(prototyped_spell);
        let prototype_mana_value = StackSpellFilter {
            min_mana_value: Some(5),
            max_mana_value: Some(5),
            ..Default::default()
        };
        assert!(stack_spell_target_legal(
            &engine.state,
            registry,
            109,
            &prototype_mana_value
        ));
        let printed_mana_value = StackSpellFilter {
            min_mana_value: Some(9),
            max_mana_value: Some(9),
            ..Default::default()
        };
        assert!(!stack_spell_target_legal(
            &engine.state,
            registry,
            109,
            &printed_mana_value
        ));
    }

    #[test]
    fn issue_231_stack_filters_match_faces_copies_and_current_colors() {
        let registry = CardRegistry::from_chunks_and_tokens(
            &[
                r#"(id: "red_creature", name: "Red Creature", face_id: "red_creature", mana_cost: "{R}", types: ["Creature"], power: 1, toughness: 1)"#,
                r#"(id: "artifact", name: "Artifact", face_id: "artifact", mana_cost: "{1}", types: ["Artifact"])"#,
                r#"(id: "enchantment", name: "Enchantment", face_id: "enchantment", mana_cost: "{U}", types: ["Enchantment"])"#,
                r#"(id: "split", name: "Blue // Green", layout: Split, faces: [(name: "Blue", face_id: "blue", mana_cost: "{U}", types: ["Instant"]), (name: "Green", face_id: "green", mana_cost: "{R}{G}", types: ["Instant"])])"#,
            ],
            &[],
        ).unwrap();
        let registry: &'static CardRegistry = Box::leak(Box::new(registry));
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            231,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap();
        engine.state.stack = vec![
            issue_205_stack_item(101, "red_creature", 0, 0),
            issue_205_stack_item(102, "artifact", 0, 0),
            issue_205_stack_item(103, "enchantment", 0, 0),
            issue_205_stack_item(104, "split", 0, 0),
            issue_205_stack_item(105, "split", 1, 0),
        ];
        engine.state.stack[4].is_copy = true;
        let annul = StackSpellFilter {
            any_of: Some(
                [CardTypeFilter::Artifact, CardTypeFilter::Enchantment]
                    .map(|card_type| StackSpellFilter {
                        card_type: Some(card_type),
                        ..Default::default()
                    })
                    .to_vec(),
            ),
            ..Default::default()
        };
        let flashfreeze = StackSpellFilter {
            any_of: Some(
                [Color::Red, Color::Green]
                    .map(|color| StackSpellFilter {
                        is_color: Some(color),
                        ..Default::default()
                    })
                    .to_vec(),
            ),
            ..Default::default()
        };
        let matches = |state: &GameState, filter: &StackSpellFilter| {
            (101..=105)
                .map(|oid| stack_spell_target_legal(state, registry, oid, filter))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            matches(&engine.state, &annul),
            [false, true, true, false, false]
        );
        assert_eq!(
            matches(&engine.state, &flashfreeze),
            [true, false, false, false, true]
        );
        // Stack copies have no GameObject. Their current color still responds to a direct
        // color effect, while a battlefield creature scope must not affect a creature spell.
        for (affected, colors, timestamp) in [
            (AffectedScope::AllCreatures, vec![Color::Blue], 1),
            (AffectedScope::Single(105), vec![Color::Blue], 2),
            (AffectedScope::Single(104), vec![Color::Red], 3),
        ] {
            engine.state.continuous_effects.push(ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected,
                kind: ContinuousEffectKind::Layer5SetColors(colors),
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp,
            });
        }
        assert_eq!(
            matches(&engine.state, &flashfreeze),
            [true, false, false, true, false]
        );
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(105),
            kind: ContinuousEffectKind::Layer5SetColors(vec![Color::Green]),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 4,
        });
        assert!(stack_spell_target_legal(
            &engine.state,
            registry,
            105,
            &flashfreeze
        ));
        let creature_spell = StackSpellFilter {
            card_type: Some(CardTypeFilter::Creature),
            ..Default::default()
        };
        engine.state.continuous_effects.extend([
            ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::Single(101),
                kind: ContinuousEffectKind::Layer4RemoveCreature,
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: 5,
            },
            ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::Single(102),
                kind: ContinuousEffectKind::Layer4AddTypes(
                    tricerules_card_model::primitives::TypeLineAddition {
                        card_types: vec![
                            tricerules_card_model::primitives::PermanentTypeFilter::Creature,
                        ],
                        ..Default::default()
                    },
                ),
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: 6,
            },
        ]);
        assert!(
            stack_spell_target_legal(&engine.state, registry, 102, &creature_spell),
            "an artifact spell that becomes a creature matches its current stack type"
        );
        assert!(
            !stack_spell_target_legal(&engine.state, registry, 101, &creature_spell),
            "a creature spell that loses creature type no longer matches"
        );
        engine.state.stack[4].ability_text = Some("ability".into());
        assert!(!stack_spell_target_legal(
            &engine.state,
            registry,
            105,
            &flashfreeze
        ));
    }

    #[test]
    fn issue_231_ownership_is_independent_of_control_across_permanent_consumers() {
        let decks = Some(vec![
            vec!["grizzly_bears".into(); 7],
            vec!["forest".into(); 7],
        ]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            231002,
            &[0, 1],
            20,
            decks,
            true,
        )
        .unwrap();
        let oid = engine.state.players[0].hand.remove(0);
        engine.state.players[1].battlefield.push(oid);
        let object = engine.state.objects.get_mut(&oid).unwrap();
        object.zone = Zone::Battlefield;
        object.controller = 1;
        object.base_controller = 1;
        let filter = TargetFilter {
            kind: TargetKind::Creature,
            owner: tricerules_card_model::primitives::TargetOwner::You,
            ..Default::default()
        };
        for player in [0, 1] {
            let expected = player == 0;
            let source = TargetSourceIdentity::current(&engine, u32::MAX);
            assert_eq!(
                target_filter_legal_with_context(
                    &engine,
                    &filter,
                    oid,
                    player,
                    source,
                    TriggerContext::default()
                ),
                expected
            );
            assert_eq!(
                permanent_choice_filter_legal(
                    &engine,
                    &filter,
                    oid,
                    player,
                    source,
                    TriggerContext::default()
                ),
                expected
            );
            assert_eq!(
                object_matches_scoped_mass_filter(&engine, oid, &filter, player),
                expected
            );
            assert_eq!(
                engine.counter_payment_permanent_matches(player, u32::MAX, oid, &filter),
                expected
            );
            let effect = ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::PermanentsMatching {
                    reference_player: player,
                    filter: Box::new(filter.clone()),
                    exclude: None,
                },
                kind: ContinuousEffectKind::Layer5SetColors(vec![Color::Blue]),
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp: 0,
            };
            assert_eq!(
                super::super::characteristics::effect_affects(
                    &engine.state,
                    engine.registry,
                    &effect,
                    oid,
                    &engine.characteristics(oid).unwrap()
                ),
                expected
            );
        }
        assert!(!engine.ability_cost_permanent_matches(1, None, oid, &filter));
        let payment = super::super::payment::components::ObjectPaymentComponent::resolution(
            rv1::CostObjectRef::default(),
            &ResolutionCost::SacrificePermanent {
                filter: TargetFilter {
                    controller: TargetController::You,
                    ..filter.clone()
                },
                source_only: false,
            },
        )
        .unwrap();
        assert!(
            !payment.candidates(&engine, 1).contains(&oid),
            "resolution costs must also check ownership"
        );
        let controlled_by_opponent = TargetFilter {
            controller: TargetController::Opponent,
            ..filter.clone()
        };
        assert!(object_matches_scoped_mass_filter(
            &engine,
            oid,
            &controlled_by_opponent,
            0
        ));
        let controlled_by_you = TargetFilter {
            controller: TargetController::You,
            ..filter.clone()
        };
        assert!(!object_matches_scoped_mass_filter(
            &engine,
            oid,
            &controlled_by_you,
            0
        ));
    }

    #[test]
    fn issue_176_exile_role_accepts_noncreature_artifacts() {
        let card = r#"(id: "test", name: "Test", face_id: "test", types: ["Instant"], spell_effect: [Exile(subject: Chosen((kind: AnyPermanent, permanent_types: [Artifact], excluded_permanent_types: [Creature])))])"#;
        let registry = CardRegistry::from_chunks_and_tokens(&[card], &[]).unwrap();
        let effect = &registry.get("test").unwrap().primary_face().spell_effect[0];
        let decks = Some(vec![
            vec!["short_sword".into(); 7],
            vec!["forest".into(); 7],
        ]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            176005,
            &[0, 1],
            20,
            decks,
            true,
        )
        .unwrap();
        let artifact = engine.state.players[0].hand.remove(0);
        engine.state.players[0].battlefield.push(artifact);
        engine.state.objects.get_mut(&artifact).unwrap().zone = Zone::Battlefield;
        assert!(target_role_legal_at_resolution(
            &engine,
            effect.target_roles()[0],
            artifact,
            0,
            TargetSourceIdentity::current(&engine, 999),
            TriggerContext::default()
        ));
    }
    #[test]
    fn issue_176_attached_exclusion_tracks_current_relation_and_lki() {
        let decks = Some(vec![
            vec!["grizzly_bears".into(); 7],
            vec!["forest".into(); 7],
        ]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            176003,
            &[0, 1],
            20,
            decks,
            true,
        )
        .unwrap();
        let ids = engine.state.players[0].hand[..3].to_vec();
        for &oid in &ids {
            engine.state.players[0].hand.retain(|id| *id != oid);
            engine.state.players[0].battlefield.push(oid);
            engine.state.objects.get_mut(&oid).unwrap().zone = Zone::Battlefield;
        }
        let [source_id, first, second] = ids[..] else {
            unreachable!()
        };
        engine
            .state
            .objects
            .get_mut(&source_id)
            .unwrap()
            .attached_to = Some(AttachmentRecipient::Object(first));
        let source = TargetSourceIdentity::current(&engine, source_id);
        let filter = issue_176_target("(kind: Creature, excluded_objects: [AttachedObject])");
        let legal = |engine: &GameEngine, oid| {
            target_filter_legal_with_context(
                engine,
                &filter,
                oid,
                0,
                source,
                TriggerContext::default(),
            )
        };
        assert!(!legal(&engine, first));
        assert!(legal(&engine, second));
        engine
            .state
            .objects
            .get_mut(&source_id)
            .unwrap()
            .attached_to = Some(AttachmentRecipient::Object(second));
        assert!(legal(&engine, first));
        assert!(!legal(&engine, second));
        super::super::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source_id,
            Zone::Graveyard,
            None,
        )
        .unwrap();
        assert!(
            !legal(&engine, second),
            "departed source retains attachment LKI"
        );
        *engine
            .state
            .zone_change_generation
            .entry(second)
            .or_default() += 1;
        assert!(
            legal(&engine, second),
            "LKI must not exclude a new incarnation"
        );
    }

    #[test]
    fn issue_176_graveyard_excludes_exact_source() {
        let decks = Some(vec![
            vec!["grizzly_bears".into(); 7],
            vec!["forest".into(); 7],
        ]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            176004,
            &[0, 1],
            20,
            decks,
            true,
        )
        .unwrap();
        let bear = engine.state.players[0].hand.remove(0);
        engine.state.players[0].graveyard.push(bear);
        engine.state.objects.get_mut(&bear).unwrap().zone = Zone::Graveyard;
        let source = TargetSourceIdentity::current(&engine, bear);
        let filter = issue_176_graveyard("(excluded_objects: [Source])");
        assert!(!target_role_legal_at_resolution(
            &engine,
            TargetRole::GraveyardCard(&filter),
            bear,
            0,
            source,
            TriggerContext::default()
        ));
        *engine.state.zone_change_generation.entry(bear).or_default() += 1;
        assert!(target_role_legal_at_resolution(
            &engine,
            TargetRole::GraveyardCard(&filter),
            bear,
            0,
            source,
            TriggerContext::default()
        ));
        let context = TriggerContext {
            source_after_zone_change: Some(TriggerObjectRef {
                object_id: bear,
                zone_change_generation: 1,
                controller_at_event: 0,
            }),
            ..Default::default()
        };
        assert!(!target_role_legal_at_resolution(
            &engine,
            TargetRole::GraveyardCard(&filter),
            bear,
            0,
            source,
            context
        ));
        *engine.state.zone_change_generation.entry(bear).or_default() += 1;
        assert!(
            target_role_legal_at_resolution(
                &engine,
                TargetRole::GraveyardCard(&filter),
                bear,
                0,
                source,
                context
            ),
            "the death event must not exclude a later incarnation"
        );
    }
    fn issue_176_target(source: &str) -> TargetFilter {
        let card = format!(
            r#"(id: "test", name: "Test", face_id: "test", types: ["Instant"], spell_effect: [DamageTarget(target: {source}, amount: 1)])"#
        );
        let registry = CardRegistry::from_chunks_and_tokens(&[&card], &[]).unwrap();
        let SpellEffectKind::DamageTarget { target, .. } =
            &registry.get("test").unwrap().primary_face().spell_effect[0]
        else {
            panic!("target filter")
        };
        target.clone()
    }

    fn issue_176_graveyard(source: &str) -> GraveyardFilter {
        let card = format!(
            r#"(id: "test", name: "Test", face_id: "test", types: ["Instant"], spell_effect: [MoveGraveyardCards(filter: {source}, destination: Hand)])"#
        );
        let registry = CardRegistry::from_chunks_and_tokens(&[&card], &[]).unwrap();
        let SpellEffectKind::MoveGraveyardCards { filter, .. } =
            &registry.get("test").unwrap().primary_face().spell_effect[0]
        else {
            panic!("graveyard filter")
        };
        filter.clone()
    }
    #[test]
    fn issue_176_battlefield_predicates_use_identity_and_characteristics() {
        let decks = Some(vec![
            vec!["grizzly_bears".into(); 7],
            vec!["forest".into(); 7],
        ]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            176001,
            &[0, 1],
            20,
            decks,
            true,
        )
        .unwrap();
        let bear = engine.state.players[0].hand.remove(0);
        engine.state.players[0].battlefield.push(bear);
        engine.state.objects.get_mut(&bear).unwrap().zone = Zone::Battlefield;
        for predicate in [
            "token: Some(true)",
            "excluded_permanent_types: [Creature]",
            "max_mana_value: Some(1)",
            "min_mana_value: Some(3)",
            "was_dealt_damage_this_turn: Some(true)",
        ] {
            let filter = issue_176_target(&format!("(kind: AnyPermanent, {predicate})"));
            assert!(
                !filter_characteristics_match(&engine, &filter, bear),
                "{predicate}"
            );
        }
        let filter = issue_176_target(
            "(kind: Creature, token: Some(false), min_mana_value: Some(2), max_mana_value: Some(2))",
        );
        assert!(filter_characteristics_match(&engine, &filter, bear));
        let damaged = issue_176_target("(kind: Creature, was_dealt_damage_this_turn: Some(true))");
        let generation = engine
            .state
            .zone_change_generation
            .get(&bear)
            .copied()
            .unwrap_or(0);
        engine
            .state
            .turn_history
            .current
            .damaged_objects
            .push((bear, generation));
        assert!(filter_characteristics_match(&engine, &damaged, bear));
        engine.state.turn_history.finish_turn();
        assert!(
            !filter_characteristics_match(&engine, &damaged, bear),
            "turn history expires"
        );
        engine
            .state
            .turn_history
            .current
            .damaged_objects
            .push((bear, generation));
        *engine.state.zone_change_generation.entry(bear).or_default() += 1;
        assert!(!filter_characteristics_match(&engine, &damaged, bear));
    }

    #[test]
    fn issue_464_target_filter_accepts_dealt_damage_predicate() {
        let filter = issue_176_target("(kind: Creature, dealt_damage_this_turn: Some(true))");
        assert_eq!(filter.kind, TargetKind::Creature);
        let decks = Some(vec![
            vec!["grizzly_bears".into(); 7],
            vec!["island".into(); 7],
        ]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            464_001,
            &[0, 1],
            20,
            decks,
            true,
        )
        .unwrap();
        let bear = engine.state.players[0].hand.remove(0);
        engine.state.players[0].battlefield.push(bear);
        engine.state.objects.get_mut(&bear).unwrap().zone = Zone::Battlefield;
        assert!(
            !filter_characteristics_match(&engine, &filter, bear),
            "a source-damage predicate must not match before this object dealt damage"
        );
        let generation = engine
            .state
            .zone_change_generation
            .get(&bear)
            .copied()
            .unwrap_or(0);
        engine
            .state
            .turn_history
            .current
            .dealt_damage_objects
            .push((bear, generation));
        assert!(filter_characteristics_match(&engine, &filter, bear));
        engine.state.turn_history.finish_turn();
        assert!(!filter_characteristics_match(&engine, &filter, bear));
        engine
            .state
            .turn_history
            .current
            .dealt_damage_objects
            .push((bear, generation));
        *engine.state.zone_change_generation.entry(bear).or_default() += 1;
        assert!(
            !filter_characteristics_match(&engine, &filter, bear),
            "a returned permanent is a different object incarnation"
        );
    }

    #[test]
    fn issue_464_opponent_damage_source_is_a_legal_target_only_this_turn() {
        let filter = issue_176_target(
            "(kind: Creature, controller: Opponent, dealt_damage_this_turn: Some(true))",
        );
        let decks = Some(vec![
            vec!["grizzly_bears".into(); 7],
            vec!["grizzly_bears".into(); 7],
        ]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            464_004,
            &[0, 1],
            20,
            decks,
            true,
        )
        .unwrap();
        let target = engine.state.players[1].hand.remove(0);
        engine.state.players[1].battlefield.push(target);
        engine.state.objects.get_mut(&target).unwrap().zone = Zone::Battlefield;
        let damaged_permanent = engine.state.players[0].hand.remove(0);
        engine.state.players[0].battlefield.push(damaged_permanent);
        engine
            .state
            .objects
            .get_mut(&damaged_permanent)
            .unwrap()
            .zone = Zone::Battlefield;
        let generation = engine
            .state
            .zone_change_generation
            .get(&target)
            .copied()
            .unwrap_or(0);
        let mut event = damage::DamageEvent::combat(
            target,
            1,
            "Grizzly Bears",
            damage::DamageRecipient::Permanent(damaged_permanent),
            1,
        );
        event.source.zone_change_generation = Some(generation);
        engine.fire_triggers(&[GameEvent::DamageDealt { event }], &mut Vec::new());
        engine.state.players[0]
            .battlefield
            .retain(|object_id| *object_id != damaged_permanent);
        engine.state.players[0].graveyard.push(damaged_permanent);
        engine
            .state
            .objects
            .get_mut(&damaged_permanent)
            .unwrap()
            .zone = Zone::Graveyard;
        *engine
            .state
            .zone_change_generation
            .entry(damaged_permanent)
            .or_default() += 1;
        let source = TargetSourceIdentity::current(&engine, 999);
        let effects = [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(Box::new(filter.clone())),
        }];
        let targets = compute_ability_targets_with_context(
            &engine,
            0,
            source,
            &effects,
            None,
            TriggerContext::default(),
        );
        assert_eq!(
            targets.groups[0].valid_permanent_ids,
            vec![target],
            "the ETB trigger publishes the opponent source that dealt damage"
        );
        assert!(target_filter_legal_at_resolution(
            &engine,
            &filter,
            target,
            0,
            source,
            TriggerContext::default()
        ));
        engine.state.turn_history.finish_turn();
        assert!(!target_filter_legal_at_resolution(
            &engine,
            &filter,
            target,
            0,
            source,
            TriggerContext::default()
        ));
    }

    #[test]
    fn issue_176_graveyard_predicates_use_card_characteristics() {
        let decks = Some(vec![
            vec!["grizzly_bears".into(); 7],
            vec!["forest".into(); 7],
        ]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            176002,
            &[0, 1],
            20,
            decks,
            true,
        )
        .unwrap();
        let bear = engine.state.players[0].hand.remove(0);
        engine.state.players[0].graveyard.push(bear);
        engine.state.objects.get_mut(&bear).unwrap().zone = Zone::Graveyard;
        for predicate in [
            "max_mana_value: Some(1)",
            "min_mana_value: Some(3)",
            "required_subtypes: [\"Zombie\"]",
            "excluded_subtypes: [\"Bear\"]",
            "has_adventure: Some(true)",
        ] {
            let filter = issue_176_graveyard(&format!("(card: Some(({predicate})))"));
            assert!(
                !graveyard_target_legal(
                    &engine,
                    &filter,
                    bear,
                    0,
                    TargetSourceIdentity::current(&engine, 999),
                    TriggerContext::default()
                ),
                "{predicate}"
            );
        }
        let filter = issue_176_graveyard(
            "(card: Some((min_mana_value: Some(2), max_mana_value: Some(2), required_subtypes: [\"Bear\"], has_adventure: Some(false))))",
        );
        assert!(graveyard_target_legal(
            &engine,
            &filter,
            bear,
            0,
            TargetSourceIdentity::current(&engine, 999),
            TriggerContext::default()
        ));
    }
    #[test]
    fn issue_176_graveyard_filters_ignore_battlefield_faces_and_use_all_opponents() {
        let cards = [
            "bonecrusher_giant_stomp",
            "changeling_wayfinder",
            "endless_one",
            "village_ironsmith_ironfang",
        ];
        let mut deck = cards.iter().map(|id| id.to_string()).collect::<Vec<_>>();
        deck.extend(vec!["forest".into(); 3]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            176006,
            &[0, 1],
            20,
            Some(vec![deck, vec!["forest".into(); 7]]),
            true,
        )
        .unwrap();
        engine
            .state
            .players
            .push(crate::state::PlayerState::new(20, 20));
        let mut ids = Vec::new();
        for card in cards {
            let oid = engine
                .state
                .objects
                .values()
                .find(|object| object.card_id == card)
                .unwrap()
                .id;
            super::super::resolution::move_object_to_zone(
                &mut engine.state,
                engine.registry,
                oid,
                Zone::Graveyard,
                None,
            )
            .unwrap();
            engine.state.players[0].graveyard.retain(|id| *id != oid);
            engine.state.players[2].graveyard.push(oid);
            let object = engine.state.objects.get_mut(&oid).unwrap();
            object.owner = 20;
            object.controller = 0;
            object.face_up_index = 1;
            ids.push(oid);
        }
        let legal = |filter: &str, oid, actor| {
            graveyard_target_legal(
                &engine,
                &issue_176_graveyard(filter),
                oid,
                actor,
                TargetSourceIdentity::current(&engine, 999),
                TriggerContext::default(),
            )
        };
        assert!(legal(
            "(owner: Opponent, card: Some((has_adventure: Some(true), min_mana_value: Some(3), max_mana_value: Some(3), required_subtypes: [\"Giant\"])))",
            ids[0],
            0
        ));
        assert!(!legal(
            "(owner: Opponent, card: Some((has_adventure: Some(true))))",
            ids[0],
            20
        ));
        assert!(
            legal(
                "(owner: Opponent, card: Some((required_subtypes: [\"Giant\"])))",
                ids[1],
                1
            ),
            "changeling works in a graveyard belonging to a third seat"
        );
        assert!(!legal(
            "(owner: AnyPlayer, card: Some((excluded_subtypes: [\"Giant\"])))",
            ids[1],
            0
        ));
        assert!(
            legal(
                "(owner: AnyPlayer, card: Some((card_type: Some(Creature), max_mana_value: Some(0))))",
                ids[2],
                0
            ),
            "X is zero outside the stack"
        );
        assert!(legal(
            "(owner: AnyPlayer, card: Some((min_mana_value: Some(2), max_mana_value: Some(2), required_subtypes: [\"Human\"])))",
            ids[3],
            0
        ));
    }
    use super::*;
    use tricerules_card_model::{primitives::TargetGroupDef, CounterKind};

    #[test]
    fn phased_out_target_source_quality_uses_saved_lki() {
        let decks = Some(vec![vec!["forest".into(); 7], vec!["forest".into(); 7]]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            2026103101,
            &[0, 1],
            20,
            decks,
            true,
        )
        .unwrap();
        let source_id = engine.state.players[0].hand[0];
        engine.state.players[0].hand.retain(|&id| id != source_id);
        engine.state.players[0].battlefield.push(source_id);
        engine.state.objects.get_mut(&source_id).unwrap().zone = Zone::Battlefield;
        let generation = engine
            .state
            .zone_change_generation
            .get(&source_id)
            .copied()
            .unwrap_or(0);
        let source = TargetSourceIdentity::current(&engine, source_id);
        engine.state.last_known_information.record_colors(
            (source_id, generation),
            vec![tricerules_card_model::Color::Blue],
        );
        engine
            .state
            .last_known_information
            .record_types((source_id, generation), vec!["Artifact".into()]);
        engine
            .state
            .objects
            .get_mut(&source_id)
            .unwrap()
            .phase_status = Some(crate::state::PhasedOutStatus {
            phase_out_under: 0,
            phase_in_due_after_departed_turn: false,
            phase_in_with: None,
            direct_attachment_snapshot: None,
        });

        assert_eq!(
            source.quality_values(&engine),
            (
                vec![tricerules_card_model::Color::Blue],
                vec!["Artifact".to_string()]
            )
        );
    }

    #[test]
    fn phased_out_object_is_not_a_target_mass_or_attachment_recipient() {
        let decks = Some(vec![
            vec!["grizzly_bears".into(); 7],
            vec!["forest".into(); 7],
        ]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            2026103102,
            &[0, 1],
            20,
            decks,
            true,
        )
        .unwrap();
        let target = engine.state.players[0].hand[0];
        engine.state.players[0].hand.retain(|&id| id != target);
        engine.state.players[0].battlefield.push(target);
        engine.state.objects.get_mut(&target).unwrap().zone = Zone::Battlefield;
        let equipment = engine.state.players[1].hand[0];
        engine.state.players[1].hand.retain(|&id| id != equipment);
        engine.state.players[1].battlefield.push(equipment);
        let equipment_object = engine.state.objects.get_mut(&equipment).unwrap();
        equipment_object.card_id = "bonesplitter".into();
        equipment_object.zone = Zone::Battlefield;

        let filter = issue_176_target("(kind: Creature)");
        let source = TargetSourceIdentity::current(&engine, u32::MAX);
        assert!(target_filter_legal_at_resolution(
            &engine,
            &filter,
            target,
            1,
            source,
            TriggerContext::default()
        ));
        assert!(object_matches_mass_filter(&engine, target, &filter));
        assert!(attachment_filter_legal(
            &engine,
            &filter,
            AttachmentRecipient::Object(target),
            u32::MAX,
            1
        ));
        assert!(equipment_attachment_legal(&engine, equipment, target));

        engine.state.objects.get_mut(&target).unwrap().phase_status =
            Some(crate::state::PhasedOutStatus {
                phase_out_under: 0,
                phase_in_due_after_departed_turn: false,
                phase_in_with: None,
                direct_attachment_snapshot: None,
            });
        let target_legal = target_filter_legal_at_resolution(
            &engine,
            &filter,
            target,
            1,
            source,
            TriggerContext::default(),
        );
        let mass_legal = object_matches_mass_filter(&engine, target, &filter);
        let attachment_legal = attachment_filter_legal(
            &engine,
            &filter,
            AttachmentRecipient::Object(target),
            u32::MAX,
            1,
        );
        let equipment_legal = equipment_attachment_legal(&engine, equipment, target);
        assert!(
            !target_legal && !mass_legal && !attachment_legal && !equipment_legal,
            "phased-out recipient remained legal: target={target_legal}, mass={mass_legal}, attachment={attachment_legal}, equipment={equipment_legal}"
        );
    }

    #[test]
    fn artifact_exchange_targets_require_a_current_same_player_pair() {
        let effect: SpellEffectKind = serde_json::from_str("\"ExchangeArtifactWithGraveyard\"")
            .expect("Welder requires an inseparable two-target exchange contract");
        let effects = vec![effect];
        let targeting: TargetingDef = serde_json::from_value(serde_json::json!({"groups": [
            {"min":1,"max":1,"prompt":"Choose battlefield artifact","effect_indices":[0]},
            {"min":1,"max":1,"prompt":"Choose artifact in that player's graveyard","effect_indices":[0]}
        ]})).unwrap();
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            2026100510,
            &[10, 20, 30],
            20,
            None,
            true,
        )
        .unwrap();
        let mut battlefield = Vec::new();
        let mut graveyard = Vec::new();
        for index in 0..3 {
            let player = engine.state.players[index].id;
            let ids = engine.state.players[index].hand[..2].to_vec();
            for &oid in &ids {
                engine.state.objects.get_mut(&oid).unwrap().card_id = "sol_ring".into();
            }
            super::super::resolution::move_object_to_zone(
                &mut engine.state,
                engine.registry,
                ids[0],
                Zone::Battlefield,
                Some(player),
            )
            .unwrap();
            super::super::resolution::move_object_to_zone(
                &mut engine.state,
                engine.registry,
                ids[1],
                Zone::Graveyard,
                None,
            )
            .unwrap();
            battlefield.push(ids[0]);
            graveyard.push(ids[1]);
        }
        let source = TargetSourceIdentity::current(&engine, u32::MAX);
        let refs = |first, second| {
            vec![
                rv1::TargetRef {
                    object_id: first,
                    group_index: 0,
                    kind: rv1::TargetRefKind::Permanent as i32,
                    ..Default::default()
                },
                rv1::TargetRef {
                    object_id: second,
                    group_index: 1,
                    kind: rv1::TargetRefKind::Graveyard as i32,
                    ..Default::default()
                },
            ]
        };
        for index in 0..3 {
            validate_spell_targets(
                &engine,
                10,
                source,
                &effects,
                Some(&targeting),
                &refs(battlefield[index], graveyard[index]),
            )
            .unwrap();
        }
        let published = compute_spell_targets(&engine, 10, source, &effects, Some(&targeting), &[]);
        assert!(published.groups[0].pair_constraint.is_none());
        let constraint = published.groups[1]
            .pair_constraint
            .as_ref()
            .expect("matched target pairs are engine-published");
        assert_eq!(constraint.prior_group_index, 0);
        assert_eq!(constraint.compatible_pairs.len(), 3);
        for index in 0..3 {
            assert!(constraint.compatible_pairs.iter().any(|pair| {
                pair.prior_target.as_ref().is_some_and(|target| {
                    target.object_id == battlefield[index]
                        && target.kind == rv1::TargetRefKind::Permanent as i32
                }) && pair.candidate.as_ref().is_some_and(|target| {
                    target.object_id == graveyard[index]
                        && target.kind == rv1::TargetRefKind::Graveyard as i32
                })
            }));
        }
        assert!(validate_spell_targets(
            &engine,
            10,
            source,
            &effects,
            Some(&targeting),
            &refs(battlefield[1], graveyard[2])
        )
        .is_err());
        let mut malformed = targeting.clone();
        malformed.groups[1].max = 2;
        assert!(
            TargetSchema::compile(&effects, Some(&malformed)).is_err(),
            "a paired role must be singleton"
        );
        malformed = targeting.clone();
        malformed.groups[0].chooser =
            tricerules_card_model::primitives::TargetChooser::ChosenOpponent;
        assert!(
            TargetSchema::compile(&effects, Some(&malformed)).is_err(),
            "the ability controller chooses both targets"
        );
        // Current control, not physical ownership, selects the matching graveyard.
        engine.state.players[1]
            .battlefield
            .retain(|oid| *oid != battlefield[1]);
        engine.state.players[2].battlefield.push(battlefield[1]);
        let artifact = engine.state.objects.get_mut(&battlefield[1]).unwrap();
        artifact.controller = 30;
        artifact.base_controller = 30;
        validate_spell_targets(
            &engine,
            10,
            source,
            &effects,
            Some(&targeting),
            &refs(battlefield[1], graveyard[2]),
        )
        .unwrap();
        assert!(validate_spell_targets(
            &engine,
            10,
            source,
            &effects,
            Some(&targeting),
            &refs(battlefield[1], graveyard[1])
        )
        .is_err());
        for &oid in &graveyard {
            engine.state.objects.get_mut(&oid).unwrap().card_id = "grizzly_bears".into();
        }
        let empty = compute_spell_targets(&engine, 10, source, &effects, Some(&targeting), &[]);
        assert!(
            empty.groups[0].valid_permanent_ids.is_empty(),
            "unmatched first choices are not offered"
        );
        assert!(empty.groups[1].valid_graveyard_ids.is_empty());
        assert!(
            empty.groups[1]
                .pair_constraint
                .as_ref()
                .unwrap()
                .compatible_pairs
                .is_empty(),
            "constrained-empty is retained"
        );
        assert!(!legal_target_group_has_minimum(
            &engine.state,
            &empty.groups[0]
        ));
    }

    #[test]
    fn grouped_targets_publish_independent_candidates_and_validate_distinctness_atomically() {
        let engine = GameEngine::new(
            tricerules_cards::registry::global(),
            73_001,
            &[10, 20],
            20,
            None,
            true,
        )
        .expect("new");
        let effects = vec![
            SpellEffectKind::TargetPlayerGainsLife {
                amount: 1.into(),
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
                    controlled_by_chooser: false,
                    for_each_opponent: false,
                    chooser: Default::default(),
                    min: 1,
                    max: 1,
                    prompt: "Choose any player".into(),
                    effect_indices: vec![0],
                    distinct_from: vec![],
                    same_graveyard: false,
                    cast_cost_expansion: None,
                },
                TargetGroupDef {
                    controlled_by_chooser: false,
                    for_each_opponent: false,
                    chooser: Default::default(),
                    min: 1,
                    max: 1,
                    prompt: "Choose a different opponent".into(),
                    effect_indices: vec![1],
                    distinct_from: vec![0],
                    same_graveyard: false,
                    cast_cost_expansion: None,
                },
            ],
        };
        let source = TargetSourceIdentity::current(&engine, u32::MAX);
        let published = compute_spell_targets(&engine, 10, source, &effects, Some(&targeting), &[]);
        assert_eq!(published.groups.len(), 2);
        assert!(published.groups[0].can_target_self);
        assert!(published.groups[0].can_target_opponent);
        assert!(!published.groups[1].can_target_self);
        assert!(published.groups[1].can_target_opponent);
        assert_eq!(published.groups[1].distinct_from_group_indices, vec![0]);

        let refs = |first, second| {
            vec![
                rv1::TargetRef {
                    counter_amount: 0,
                    object_id: first,
                    damage_amount: 0,
                    group_index: 0,
                    kind: 0,
                    expected_zone_change_generation: None,
                },
                rv1::TargetRef {
                    counter_amount: 0,
                    object_id: second,
                    damage_amount: 0,
                    group_index: 1,
                    kind: 0,
                    expected_zone_change_generation: None,
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
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            96905,
            &[10, 20],
            20,
            None,
            true,
        )
        .expect("new");
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
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            96906,
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("new");
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
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            70070,
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("new");
        let bear = engine.state.players[0].hand[0];
        engine.state.players[0].hand.remove(0);
        engine.state.players[0].battlefield.push(bear);
        engine.state.objects.get_mut(&bear).expect("bear").zone = Zone::Battlefield;

        let filter = TargetFilter {
            kind: TargetKind::Creature,
            excluded_objects: vec![tricerules_card_model::TargetObjectExclusion::Source],
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
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            96907,
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("new");
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
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            71_001,
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("new");
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

    #[test]
    fn issue_114_recursive_filter_is_shared_by_mass_cost_and_attachment_selection() {
        let decks = Some(vec![
            vec![
                "grizzly_bears".into(),
                "wind_drake".into(),
                "short_sword".into(),
                "ornithopter".into(),
                "forest".into(),
                "forest".into(),
                "forest".into(),
            ],
            vec!["forest".into(); 7],
        ]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            114_007,
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("new");
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
            let object = engine.state.objects.get_mut(&oid).expect("object");
            object.zone = Zone::Battlefield;
            object.base_controller = 0;
            object.controller = 0;
            oid
        };
        let bear = deploy("grizzly_bears");
        let drake = deploy("wind_drake");
        let sword = deploy("short_sword");
        let ornithopter = deploy("ornithopter");
        let filter = TargetFilter {
            any_of: Some(vec![
                TargetFilter {
                    kind: TargetKind::Creature,
                    controller: TargetController::You,
                    required_keywords: vec![Keyword::Flying],
                    ..TargetFilter::default()
                },
                TargetFilter {
                    kind: TargetKind::AnyPermanent,
                    controller: TargetController::You,
                    permanent_types: vec![PermanentTypeFilter::Artifact],
                    ..TargetFilter::default()
                },
            ]),
            ..TargetFilter::default()
        };

        assert_eq!(
            battlefield_objects_matching(&engine, &filter),
            vec![drake, sword, ornithopter],
            "a permanent satisfying both leaves is selected once"
        );
        for oid in [drake, sword, ornithopter] {
            assert!(engine.ability_cost_permanent_matches(0, None, oid, &filter));
            assert!(attachment_filter_legal(
                &engine,
                &filter,
                AttachmentRecipient::Object(oid),
                u32::MAX,
                0,
            ));
        }
        assert!(!engine.ability_cost_permanent_matches(0, None, bear, &filter));
        assert!(!attachment_filter_legal(
            &engine,
            &filter,
            AttachmentRecipient::Object(bear),
            u32::MAX,
            0,
        ));
    }

    #[test]
    fn issue_219_stack_target_identity_tracks_physical_generations_but_not_copy_ids() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            219_008,
            &[0, 1],
            20,
            None,
            true,
        )
        .expect("new");
        let physical = engine.state.players[0].hand[0];
        let physical_target = capture_stack_target(
            &engine,
            &rv1::TargetRef {
                object_id: physical,
                kind: rv1::TargetRefKind::Stack as i32,
                ..Default::default()
            },
        );
        assert_eq!(
            physical_target.zone_change_generation,
            Some(
                engine
                    .state
                    .zone_change_generation
                    .get(&physical)
                    .copied()
                    .unwrap_or(0)
            )
        );
        assert!(stack_target_identity_is_current(&engine, &physical_target));
        *engine
            .state
            .zone_change_generation
            .entry(physical)
            .or_default() += 1;
        assert!(!stack_target_identity_is_current(&engine, &physical_target));

        let copy_id = u32::MAX;
        let copy_target = capture_stack_target(
            &engine,
            &rv1::TargetRef {
                object_id: copy_id,
                kind: rv1::TargetRefKind::Stack as i32,
                ..Default::default()
            },
        );
        assert_eq!(copy_target.zone_change_generation, None);
        assert!(stack_target_identity_is_current(&engine, &copy_target));
    }

    #[test]
    fn player_only_roles_omit_physical_candidates_in_every_zone() {
        let any_player = TargetFilter {
            kind: TargetKind::AnyPlayer,
            ..Default::default()
        };
        let opponent = TargetFilter {
            kind: TargetKind::OpponentPlayer,
            ..Default::default()
        };
        let effects = [
            SpellEffectKind::DamageTarget {
                target: any_player.clone(),
                amount: Amount::Fixed(1),
            },
            SpellEffectKind::DamageTarget {
                target: opponent.clone(),
                amount: Amount::Fixed(1),
            },
            SpellEffectKind::DamageTarget {
                target: TargetFilter {
                    any_of: Some(vec![any_player.clone(), opponent]),
                    ..Default::default()
                },
                amount: Amount::Fixed(1),
            },
            SpellEffectKind::PoisonTargetUpToNine {
                target: any_player.clone(),
            },
            SpellEffectKind::PutPoisonCountersOnTargetPlayer {
                target: any_player,
                count: 2,
            },
        ];
        for zone in [Zone::Battlefield, Zone::Graveyard, Zone::Stack] {
            let mut engine = GameEngine::new(
                tricerules_cards::registry::global(),
                219_010,
                &[0, 1],
                20,
                Some(vec![
                    vec!["grizzly_bears".into(); 7],
                    vec!["forest".into(); 7],
                ]),
                true,
            )
            .unwrap();
            let colliding_id = engine.state.players[0].hand[0];
            super::super::resolution::move_object_to_zone(
                &mut engine.state,
                engine.registry,
                colliding_id,
                zone,
                None,
            )
            .unwrap();
            if zone == Zone::Stack {
                engine
                    .state
                    .stack
                    .push(issue_205_stack_item(colliding_id, "grizzly_bears", 0, 0));
            }
            engine
                .state
                .players
                .push(crate::state::PlayerState::new(colliding_id as PlayerId, 20));
            let source = TargetSourceIdentity::current(&engine, u32::MAX);
            for (index, effect) in effects.iter().enumerate() {
                let targets = compute_spell_targets(
                    &engine,
                    0,
                    source,
                    std::slice::from_ref(effect),
                    None,
                    &[],
                );
                let group = &targets.groups[0];
                assert!(
                    group.valid_permanent_ids.is_empty()
                        && group.valid_permanent_targets.is_empty()
                        && group.valid_stack_ids.is_empty()
                        && group.valid_graveyard_ids.is_empty(),
                    "player-only effect {effect:?} published a physical candidate in {zone:?}"
                );
                assert_eq!(group.can_target_self, index != 1);
                assert!(group.can_target_opponent);
            }
            // A mixed role must still offer both players and battlefield creatures.
            let mixed = [SpellEffectKind::DamageTarget {
                target: TargetFilter::default(),
                amount: Amount::Fixed(1),
            }];
            let targets = compute_spell_targets(&engine, 0, source, &mixed, None, &[]);
            assert!(targets.groups[0].can_target_self);
            assert!(targets.groups[0].can_target_opponent);
            if zone == Zone::Battlefield {
                assert!(targets.groups[0]
                    .valid_permanent_ids
                    .contains(&colliding_id));
            }
        }
    }

    #[test]
    fn capture_stack_target_preserves_player_and_object_domains_on_id_collision() {
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            219_009,
            &[0, 1],
            20,
            None,
            true,
        )
        .expect("new");
        let colliding_id = 2;
        assert!(engine.state.objects.contains_key(&colliding_id));
        engine
            .state
            .players
            .push(crate::state::PlayerState::new(colliding_id as PlayerId, 20));

        let legacy_player_target = capture_stack_target(
            &engine,
            &rv1::TargetRef {
                object_id: colliding_id,
                kind: rv1::TargetRefKind::Unspecified as i32,
                ..Default::default()
            },
        );
        assert_eq!(
            legacy_player_target.zone_change_generation, None,
            "legacy player references keep their player domain even when an object shares the id"
        );

        let explicit_player_target = capture_stack_target(
            &engine,
            &rv1::TargetRef {
                object_id: colliding_id,
                kind: rv1::TargetRefKind::Player as i32,
                ..Default::default()
            },
        );
        assert_eq!(explicit_player_target.zone_change_generation, None);

        let explicit_object_target = capture_stack_target(
            &engine,
            &rv1::TargetRef {
                object_id: colliding_id,
                kind: rv1::TargetRefKind::Permanent as i32,
                ..Default::default()
            },
        );
        assert_eq!(
            explicit_object_target.zone_change_generation,
            Some(
                engine
                    .state
                    .zone_change_generation
                    .get(&colliding_id)
                    .copied()
                    .unwrap_or(0)
            ),
            "an explicit object target keeps its object domain when the id also names a player"
        );

        let absent_stack_copy_target = capture_stack_target(
            &engine,
            &rv1::TargetRef {
                object_id: u32::MAX,
                kind: rv1::TargetRefKind::Stack as i32,
                ..Default::default()
            },
        );
        assert_eq!(
            absent_stack_copy_target.zone_change_generation, None,
            "virtual stack copies without GameObjects keep the prior absent-object receipt"
        );
    }
}
