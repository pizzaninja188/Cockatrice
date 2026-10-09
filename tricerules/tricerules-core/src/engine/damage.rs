//! Shared CR 120/615 damage-event preprocessing.
//!
//! Damage producers construct events here before mutating life, marked damage, deathtouch
//! history, lifelink totals, or damage-trigger state. This keeps replacements, prevention, and
//! prohibitions out of incidental spell/combat iteration order and gives CR 616 one event shape.

use super::events::{ev_log, finish_with_events, object_display_name};
use super::targeting::TargetSourceIdentity;
use super::*;

#[derive(serde::Serialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct DamageSourceSnapshot {
    pub wither: bool,
    pub object_id: ObjectId,
    /// Captured incarnation of the object that dealt this damage. `None` is reserved for
    /// sources without a physical object, such as a copy that has no backing card.
    #[serde(default)]
    pub zone_change_generation: Option<u64>,
    pub controller: PlayerId,
    pub label: String,
    pub colors: Vec<Color>,
    pub types: Vec<String>,
}

#[derive(serde::Serialize, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DamageRecipient {
    Player(PlayerId),
    Permanent(ObjectId),
}

impl DamageRecipient {
    fn prevention_key(self) -> ObjectId {
        match self {
            Self::Player(player) => player as ObjectId,
            Self::Permanent(object) => object,
        }
    }
}

#[derive(serde::Serialize, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DamageClassification {
    Combat,
    Noncombat,
}

#[derive(serde::Serialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct DamageEvent {
    pub source: DamageSourceSnapshot,
    pub recipient: DamageRecipient,
    pub amount: u32,
    pub classification: DamageClassification,
}

impl DamageEvent {
    pub(crate) fn combat(
        source_id: ObjectId,
        controller: PlayerId,
        label: impl Into<String>,
        recipient: DamageRecipient,
        amount: u32,
    ) -> Self {
        Self::new(
            source_id,
            controller,
            label,
            recipient,
            amount,
            DamageClassification::Combat,
        )
    }

    pub(crate) fn noncombat(
        source_id: ObjectId,
        controller: PlayerId,
        label: impl Into<String>,
        recipient: DamageRecipient,
        amount: u32,
    ) -> Self {
        Self::new(
            source_id,
            controller,
            label,
            recipient,
            amount,
            DamageClassification::Noncombat,
        )
    }

    fn new(
        source_id: ObjectId,
        controller: PlayerId,
        label: impl Into<String>,
        recipient: DamageRecipient,
        amount: u32,
        classification: DamageClassification,
    ) -> Self {
        Self {
            source: DamageSourceSnapshot {
                wither: false,
                object_id: source_id,
                zone_change_generation: None,
                controller,
                label: label.into(),
                colors: Vec::new(),
                types: Vec::new(),
            },
            recipient,
            amount,
            classification,
        }
    }
}

#[derive(serde::Serialize, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DamageResult {
    pub attempted: u32,
    pub dealt: u32,
    /// Total damage removed by prevention applications. This can exceed `attempted` when
    /// prevention and doubling effects are ordered more than once during one event.
    pub prevented: u64,
}

#[derive(serde::Serialize, Debug, Clone)]
pub(crate) struct DamageSpec {
    pub event: DamageEvent,
    pub source_has_deathtouch: bool,
    pub source_has_lifelink: bool,
}

#[derive(Debug, Clone)]
enum DamageBatchContinuation {
    Combat,
    Stack {
        item: Box<StackItem>,
        resume_effect_index: Option<u32>,
    },
    ManaAbility {
        actor: PlayerId,
        source_object_id: ObjectId,
        source_zone_change_generation: u64,
        resume_resolution: Option<Box<PendingResolution>>,
    },
}

impl DamageBatchContinuation {
    fn source_object_id(&self, batch: &PendingDamageBatch) -> ObjectId {
        match self {
            Self::Combat => batch
                .damage
                .first()
                .map(|damage| damage.spec.event.source.object_id)
                .unwrap_or(0),
            Self::Stack { item, .. } => item.source_permanent_id.unwrap_or(item.id),
            Self::ManaAbility {
                source_object_id, ..
            } => *source_object_id,
        }
    }

    fn fallback_controller(&self) -> PlayerId {
        match self {
            Self::Combat => unreachable!("combat choices require a surviving affected recipient"),
            Self::Stack { item, .. } => item.controller,
            Self::ManaAbility { actor, .. } => *actor,
        }
    }
}

#[derive(serde::Serialize, Debug, Clone)]
pub(crate) struct DamageApplicationChoice {
    pub choice_id: u32,
    pub application: DamageReplacementApplication,
    pub event_index: usize,
}

#[derive(serde::Serialize, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DamageReplacementApplication {
    Effect(u32),
    Protection(ProtectionQuality),
    DoubleEffect(u32),
}

#[derive(serde::Serialize, Debug, Clone)]
pub(crate) struct PendingDamageBatch {
    pub damage: Vec<PendingDamageEvent>,
    pub applications: Vec<DamageApplicationChoice>,
}

#[derive(serde::Serialize, Debug, Clone)]
pub(crate) struct PendingDamageEvent {
    pub spec: DamageSpec,
    pub remaining: u32,
    pub prevented_total: u64,
    pub applied_applications: Vec<DamageReplacementApplication>,
    /// Exact finite shield amounts consumed while reducing this damage occurrence.
    pub prevention_debits: Vec<(u32, u32)>,
    pub combat: Option<PendingCombatDamageOccurrence>,
    /// Original noncombat recipient occurrence; never rebound during a parked choice.
    pub recipient_generation: Option<u64>,
}

/// The original combat relation, distinct from the presentation source and current controller.
#[derive(serde::Serialize, Debug, Clone)]
pub(crate) struct PendingCombatDamageOccurrence {
    pub attacker: TriggerObjectRef,
    pub recipient_generation: Option<u64>,
}

#[derive(serde::Serialize, Debug, Clone)]
pub(crate) struct CompletedDamage {
    pub spec: DamageSpec,
    pub result: DamageResult,
    /// Finite prevention resources consumed while producing this exact result.
    pub prevention_debits: Vec<(u32, u32)>,
}

enum DamageBatchProgress {
    Complete(Vec<CompletedDamage>),
    NeedsChoice {
        batch: PendingDamageBatch,
        raw_candidates: Vec<(usize, DamageReplacementApplication, String)>,
    },
}

impl GameEngine {
    fn damage_batch_needs_ordering(&self, damage: &[DamageSpec]) -> bool {
        let by_event: Vec<Vec<DamageReplacementApplication>> = damage
            .iter()
            .map(|spec| self.damage_replacement_applications(&spec.event))
            .collect();
        if by_event.iter().any(|candidates| candidates.len() > 1) {
            return true;
        }
        if by_event.iter().any(|candidates| {
            candidates
                .iter()
                .any(|candidate| matches!(candidate, DamageReplacementApplication::DoubleEffect(_)))
        }) {
            // Combat's legacy fast path commits each assigned event directly. Even a single
            // doubler must route the whole simultaneous batch through the shared event pipeline.
            return true;
        }
        self.state.damage_prevention_effects.iter().any(|effect| {
            matches!(effect.amount, DamagePreventionAmount::Remaining(_))
                && by_event
                    .iter()
                    .filter(|candidates| {
                        candidates.contains(&DamageReplacementApplication::Effect(effect.id))
                    })
                    .count()
                    > 1
        })
    }

    fn protection_applications(&self, event: &DamageEvent) -> Vec<ProtectionQuality> {
        let DamageRecipient::Permanent(recipient) = event.recipient else {
            return Vec::new();
        };
        self.characteristics(recipient)
            .map(|characteristics| {
                characteristics
                    .protections
                    .into_iter()
                    .filter(|quality| quality.matches(&event.source.colors, &event.source.types))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn damage_replacement_applications(
        &self,
        event: &DamageEvent,
    ) -> Vec<DamageReplacementApplication> {
        self.state
            .damage_prevention_effects
            .iter()
            .filter(|effect| self.prevention_effect_applies(effect, event))
            .map(|effect| DamageReplacementApplication::Effect(effect.id))
            .chain(
                self.state
                    .damage_doubling_effects
                    .iter()
                    .filter(|effect| self.damage_doubling_effect_applies(effect, event))
                    .map(|effect| DamageReplacementApplication::DoubleEffect(effect.id)),
            )
            .chain(
                self.protection_applications(event)
                    .into_iter()
                    .map(DamageReplacementApplication::Protection),
            )
            .collect()
    }

    fn damage_doubling_effect_applies(
        &self,
        effect: &ActiveDamageDoubling,
        event: &DamageEvent,
    ) -> bool {
        if effect.duration == EffectDuration::WhileSourceOnBattlefield {
            let Some(source) = effect.source_id else {
                return false;
            };
            if effect.static_origin.as_ref().is_some_and(|origin| {
                super::characteristics::normalized_static_origin(&self.state, self.registry, origin)
                    .is_none()
            }) || !super::characteristics::printed_static_source_is_available(
                &self.state,
                self.registry,
                source,
            ) {
                return false;
            }
        }
        match effect.scope {
            DamageDoublingScope::AnySource => true,
            DamageDoublingScope::CreatureYouControl { ability_source } => {
                event
                    .source
                    .types
                    .iter()
                    .any(|kind| kind.eq_ignore_ascii_case("Creature"))
                    && self.controller_of(ability_source) == Some(event.source.controller)
            }
        }
    }

    fn prevention_effect_applies(
        &self,
        effect: &ActiveDamagePrevention,
        event: &DamageEvent,
    ) -> bool {
        if let Some(origin) = &effect.static_origin {
            let Some(source) = effect.source_id else {
                return false;
            };
            if super::characteristics::normalized_static_origin(&self.state, self.registry, origin)
                .is_none()
                || !super::characteristics::printed_static_source_is_available(
                    &self.state,
                    self.registry,
                    source,
                )
            {
                return false;
            }
        }
        let key = event.recipient.prevention_key();
        match effect.scope {
            DamagePreventionScope::Recipient(recipient) => recipient == key,
            DamagePreventionScope::CombatRecipient {
                object_id,
                zone_change_generation,
            } => {
                event.classification == DamageClassification::Combat
                    && event.recipient == DamageRecipient::Permanent(object_id)
                    && self
                        .state
                        .zone_change_generation
                        .get(&object_id)
                        .copied()
                        .unwrap_or(0)
                        == zone_change_generation
            }
            DamagePreventionScope::CombatSource {
                object_id,
                zone_change_generation,
            } => {
                event.classification == DamageClassification::Combat
                    && event.source.object_id == object_id
                    && event.source.zone_change_generation == Some(zone_change_generation)
            }
            DamagePreventionScope::Combat => event.classification == DamageClassification::Combat,
            DamagePreventionScope::OtherCreaturesYouControl {
                source_id,
                controller,
            } => match event.recipient {
                DamageRecipient::Permanent(recipient) => {
                    let controller = self.controller_of(source_id).unwrap_or(controller);
                    recipient != source_id
                        && self
                            .state
                            .objects
                            .get(&recipient)
                            .is_some_and(|object| object.controller == controller)
                        && self
                            .characteristics(recipient)
                            .is_some_and(|characteristics| characteristics.is_creature())
                }
                DamageRecipient::Player(_) => false,
            },
        }
    }

    pub(crate) fn process_or_park_damage_batch(
        &mut self,
        item: &StackItem,
        mut damage: Vec<DamageSpec>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<Option<Vec<CompletedDamage>>, EngineError> {
        for spec in &mut damage {
            let item_source = item.source_permanent_id.unwrap_or(item.id);
            spec.event.source.zone_change_generation = if spec.event.source.object_id == item_source
                && item.source_permanent_id.is_some()
            {
                Some(item.source_zone_change)
            } else if spec.event.source.object_id == item.id && item.is_copy {
                None
            } else {
                self.state
                    .objects
                    .contains_key(&spec.event.source.object_id)
                    .then(|| {
                        self.state
                            .zone_change_generation
                            .get(&spec.event.source.object_id)
                            .copied()
                            .unwrap_or(0)
                    })
            };
            spec.event.source.controller = self.damage_source_controller(&spec.event);
            spec.event.source.wither = if spec.event.source.object_id == item_source {
                self.resolving_source_has_keyword(item, Keyword::Wither)
            } else {
                self.effective_has_keyword(spec.event.source.object_id, Keyword::Wither)
            };
            let source = if spec.event.source.object_id == item_source {
                TargetSourceIdentity::for_stack_item(self, item)
            } else {
                TargetSourceIdentity::current(self, spec.event.source.object_id)
            };
            let (colors, types) = source.quality_values(self);
            spec.event.source.colors = colors;
            spec.event.source.types = types;
        }
        let pending = PendingDamageBatch {
            damage: damage
                .into_iter()
                .map(|spec| PendingDamageEvent {
                    recipient_generation: self.damage_recipient_generation(&spec.event),
                    remaining: spec.event.amount,
                    prevented_total: 0,
                    spec,
                    applied_applications: Vec::new(),
                    prevention_debits: Vec::new(),
                    combat: None,
                })
                .collect(),
            applications: Vec::new(),
        };
        match self.advance_damage_batch(pending, events)? {
            DamageBatchProgress::Complete(completed) => Ok(Some(completed)),
            DamageBatchProgress::NeedsChoice {
                batch,
                raw_candidates,
            } => {
                self.park_damage_replacement_choice(
                    DamageBatchContinuation::Stack {
                        item: Box::new(item.clone()),
                        resume_effect_index: None,
                    },
                    batch,
                    raw_candidates,
                    events,
                );
                Ok(None)
            }
        }
    }

    fn damage_source_controller(&self, event: &DamageEvent) -> PlayerId {
        let Some(generation) = event.source.zone_change_generation else {
            return event.source.controller;
        };
        let is_current_battlefield_generation = self
            .state
            .zone_change_generation
            .get(&event.source.object_id)
            .copied()
            .unwrap_or(0)
            == generation
            && self
                .state
                .objects
                .get(&event.source.object_id)
                .is_some_and(|object| object.zone == Zone::Battlefield);
        let current_controller = match is_current_battlefield_generation {
            true => self.controller_of(event.source.object_id),
            false => None,
        };
        if let Some(controller) = current_controller {
            return controller;
        }
        self.state
            .last_known_controller_by_generation
            .get(&(event.source.object_id, generation))
            .copied()
            .unwrap_or(event.source.controller)
    }

    /// Capture source qualities before activated costs commit, then resolve this fixed damage
    /// directly through the shared replacement, prevention, damage-trigger, and life pipeline.
    pub(crate) fn prepare_mana_ability_damage(
        &self,
        source_object_id: ObjectId,
        actor: PlayerId,
        source_label: impl Into<String>,
        amount: u32,
    ) -> DamageSpec {
        let generation = self
            .state
            .zone_change_generation
            .get(&source_object_id)
            .copied()
            .unwrap_or(0);
        let source = TargetSourceIdentity::captured(source_object_id, generation);
        let (colors, types) = source.quality_values(self);
        let mut event = DamageEvent::noncombat(
            source_object_id,
            actor,
            source_label,
            DamageRecipient::Player(actor),
            amount,
        );
        event.source.zone_change_generation = Some(generation);
        event.source.wither = self.effective_has_keyword(source_object_id, Keyword::Wither);
        event.source.colors = colors;
        event.source.types = types;
        DamageSpec {
            event,
            source_has_deathtouch: self
                .effective_has_keyword(source_object_id, Keyword::Deathtouch),
            source_has_lifelink: self.effective_has_keyword(source_object_id, Keyword::Lifelink),
        }
    }

    /// Resolve a mana ability's additional damage effect without creating a stack object or
    /// handing priority to another player.
    pub(super) fn resolve_mana_ability_damage(
        &mut self,
        damage: DamageSpec,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(Vec<CompletedDamage>, Vec<super::triggers::CollectedTrigger>), EngineError> {
        let continuation = DamageBatchContinuation::ManaAbility {
            actor: damage.event.source.controller,
            source_object_id: damage.event.source.object_id,
            source_zone_change_generation: damage.event.source.zone_change_generation.unwrap_or(0),
            resume_resolution: None,
        };
        let pending = PendingDamageBatch {
            damage: vec![PendingDamageEvent {
                recipient_generation: self.damage_recipient_generation(&damage.event),
                remaining: damage.event.amount,
                prevented_total: 0,
                spec: damage,
                applied_applications: Vec::new(),
                prevention_debits: Vec::new(),
                combat: None,
            }],
            applications: Vec::new(),
        };
        match self.advance_damage_batch(pending, events)? {
            DamageBatchProgress::Complete(completed) => {
                let triggers =
                    self.commit_completed_damage_batch_collecting_triggers(&completed, events)?;
                Ok((completed, triggers))
            }
            DamageBatchProgress::NeedsChoice {
                batch,
                raw_candidates,
            } => {
                let actor = continuation.fallback_controller();
                let suspend_payment =
                    self.state
                        .pending_resolution
                        .as_ref()
                        .is_some_and(|pending| {
                            pending.deciding_player == actor
                                && pending.continuation.mana_window_undo_start().is_some()
                        });
                let continuation = match continuation {
                    DamageBatchContinuation::ManaAbility {
                        actor,
                        source_object_id,
                        source_zone_change_generation,
                        ..
                    } => DamageBatchContinuation::ManaAbility {
                        actor,
                        source_object_id,
                        source_zone_change_generation,
                        resume_resolution: if suspend_payment {
                            self.state.pending_resolution.take().map(Box::new)
                        } else {
                            None
                        },
                    },
                    stack => stack,
                };
                self.park_damage_replacement_choice(continuation, batch, raw_candidates, events);
                Ok((Vec::new(), Vec::new()))
            }
        }
    }

    fn pending_damage_replacement_candidates(
        &self,
        batch: &PendingDamageBatch,
    ) -> Vec<Vec<(DamageReplacementApplication, String)>> {
        batch
            .damage
            .iter()
            .map(|damage| {
                if damage.remaining == 0 {
                    return Vec::new();
                }
                let effects = self
                    .state
                    .damage_prevention_effects
                    .iter()
                    .filter(|effect| {
                        !damage
                            .applied_applications
                            .contains(&DamageReplacementApplication::Effect(effect.id))
                            && self.prevention_effect_applies(effect, &damage.spec.event)
                    })
                    .map(|effect| {
                        (
                            DamageReplacementApplication::Effect(effect.id),
                            effect.source_label.clone(),
                        )
                    });
                let doublers = self
                    .state
                    .damage_doubling_effects
                    .iter()
                    .filter(|effect| {
                        !damage
                            .applied_applications
                            .contains(&DamageReplacementApplication::DoubleEffect(effect.id))
                            && self.damage_doubling_effect_applies(effect, &damage.spec.event)
                    })
                    .map(|effect| {
                        (
                            DamageReplacementApplication::DoubleEffect(effect.id),
                            effect.source_label.clone(),
                        )
                    });
                let protection = self
                    .protection_applications(&damage.spec.event)
                    .into_iter()
                    .filter(|quality| {
                        !damage
                            .applied_applications
                            .contains(&DamageReplacementApplication::Protection(*quality))
                    })
                    .map(|quality| {
                        (
                            DamageReplacementApplication::Protection(quality),
                            quality.label(),
                        )
                    });
                effects.chain(doublers).chain(protection).collect()
            })
            .collect()
    }

    fn damage_affected_player(&self, event: &DamageEvent) -> Option<PlayerId> {
        let player = match event.recipient {
            DamageRecipient::Player(player) => player,
            DamageRecipient::Permanent(oid) => self.controller_of(oid)?,
        };
        self.state
            .player_idx(player)
            .filter(|&index| !self.state.players[index].has_lost)
            .map(|_| player)
    }

    fn next_damage_ordering_choice(
        &self,
        batch: &PendingDamageBatch,
        by_event: &[Vec<(DamageReplacementApplication, String)>],
    ) -> Vec<(usize, DamageReplacementApplication, String)> {
        let mut pairs = Vec::new();
        for (event_index, candidates) in by_event.iter().enumerate() {
            if candidates.len() > 1 {
                pairs.extend(
                    candidates
                        .iter()
                        .map(|(application, label)| (event_index, *application, label.clone())),
                );
            }
        }
        for effect in &self.state.damage_prevention_effects {
            if !matches!(effect.amount, DamagePreventionAmount::Remaining(_)) {
                continue;
            }
            let application = DamageReplacementApplication::Effect(effect.id);
            let occurrences: Vec<_> = by_event
                .iter()
                .enumerate()
                .filter(|(_, candidates)| {
                    candidates
                        .iter()
                        .any(|(candidate, _)| *candidate == application)
                })
                .map(|(index, _)| (index, application, effect.source_label.clone()))
                .collect();
            if occurrences.len() > 1 {
                for pair in occurrences {
                    if !pairs.iter().any(
                        |existing: &(usize, DamageReplacementApplication, String)| {
                            existing.0 == pair.0 && existing.1 == pair.1
                        },
                    ) {
                        pairs.push(pair);
                    }
                }
            }
        }
        let first_player = pairs
            .iter()
            .filter_map(|(index, _, _)| {
                self.damage_affected_player(&batch.damage[*index].spec.event)
            })
            .min_by_key(|&player| self.state.apnap_rank(player));
        pairs.retain(|(index, _, _)| {
            first_player.is_some()
                && self.damage_affected_player(&batch.damage[*index].spec.event) == first_player
        });
        pairs
    }

    fn advance_damage_batch(
        &mut self,
        mut batch: PendingDamageBatch,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<DamageBatchProgress, EngineError> {
        loop {
            let by_event = self.pending_damage_replacement_candidates(&batch);
            let raw_candidates = self.next_damage_ordering_choice(&batch, &by_event);
            if !raw_candidates.is_empty() {
                return Ok(DamageBatchProgress::NeedsChoice {
                    batch,
                    raw_candidates,
                });
            }
            let Some((event_index, application)) =
                by_event
                    .iter()
                    .enumerate()
                    .find_map(|(event_index, candidates)| {
                        candidates
                            .first()
                            .map(|(application, _)| (event_index, *application))
                    })
            else {
                return Ok(DamageBatchProgress::Complete(
                    batch
                        .damage
                        .into_iter()
                        .map(|damage| CompletedDamage {
                            result: DamageResult {
                                attempted: damage.spec.event.amount,
                                dealt: damage.remaining,
                                prevented: damage.prevented_total,
                            },
                            prevention_debits: damage.prevention_debits,
                            spec: damage.spec,
                        })
                        .collect(),
                ));
            };
            let applied = self.apply_damage_replacement_application(
                &mut batch,
                event_index,
                application,
                events,
            )?;
            debug_assert!(applied);
        }
    }

    fn apply_damage_replacement_application(
        &mut self,
        batch: &mut PendingDamageBatch,
        event_index: usize,
        application: DamageReplacementApplication,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let Some(damage) = batch.damage.get(event_index) else {
            return Ok(false);
        };
        if damage.remaining == 0 || damage.applied_applications.contains(&application) {
            return Ok(false);
        }
        let event = damage.spec.event.clone();
        if let DamageReplacementApplication::DoubleEffect(effect_id) = application {
            let Some(effect) = self
                .state
                .damage_doubling_effects
                .iter()
                .find(|effect| effect.id == effect_id)
                .filter(|effect| self.damage_doubling_effect_applies(effect, &event))
            else {
                return Ok(false);
            };
            let source_label = effect.source_label.clone();
            let Some(doubled) = damage.remaining.checked_mul(2) else {
                return Err(EngineError::DamageNumericRange(
                    "damage doubler multiplication",
                ));
            };
            let damage = &mut batch.damage[event_index];
            damage.remaining = doubled;
            damage.applied_applications.push(application);
            events.push(ev_log(format!(
                "{source_label} doubles damage from {} to {doubled}.",
                event.source.label
            )));
            return Ok(true);
        }
        if let DamageReplacementApplication::Protection(quality) = application {
            if !self.protection_applications(&event).contains(&quality) {
                return Ok(false);
            }
            let prevented = if self.state.damage_prevention_prohibitions.is_empty() {
                damage.remaining
            } else {
                0
            };
            let prevented_total = damage
                .prevented_total
                .checked_add(u64::from(prevented))
                .ok_or(EngineError::DamageNumericRange("prevented damage total"))?;
            let damage = &mut batch.damage[event_index];
            damage.remaining -= prevented;
            damage.prevented_total = prevented_total;
            damage.applied_applications.push(application);
            if prevented > 0 {
                events.push(ev_log(format!(
                    "{} prevents {prevented} damage from {}.",
                    quality.label(),
                    event.source.label
                )));
            }
            return Ok(true);
        }
        let DamageReplacementApplication::Effect(effect_id) = application else {
            unreachable!();
        };
        let Some(effect_index) = self
            .state
            .damage_prevention_effects
            .iter()
            .position(|effect| {
                effect.id == effect_id && self.prevention_effect_applies(effect, &event)
            })
        else {
            return Ok(false);
        };
        let unpreventable = !self.state.damage_prevention_prohibitions.is_empty();
        let application_attempted = damage.remaining;
        let effect = &mut self.state.damage_prevention_effects[effect_index];
        let finite_remaining = matches!(effect.amount, DamagePreventionAmount::Remaining(_));
        let prevented = if unpreventable {
            0
        } else {
            match effect.amount {
                DamagePreventionAmount::All => application_attempted,
                DamagePreventionAmount::FixedPerEvent(capacity)
                | DamagePreventionAmount::Remaining(capacity) => {
                    application_attempted.min(capacity)
                }
            }
        };
        let prevented_total = damage
            .prevented_total
            .checked_add(u64::from(prevented))
            .ok_or(EngineError::DamageNumericRange("prevented damage total"))?;
        if let DamagePreventionAmount::Remaining(remaining) = &mut effect.amount {
            *remaining -= prevented;
        }
        let source_label = effect.source_label.clone();
        let additional_effect = effect.additional_effect;
        let exhausted = matches!(effect.amount, DamagePreventionAmount::Remaining(0));

        let damage = &mut batch.damage[event_index];
        damage.remaining -= prevented;
        damage.prevented_total = prevented_total;
        damage.applied_applications.push(application);
        if finite_remaining && prevented > 0 {
            damage.prevention_debits.push((effect_id, prevented));
        }
        if prevented > 0 {
            events.push(ev_log(format!(
                "{source_label} prevents {prevented} damage from {}.",
                event.source.label
            )));
        }
        if let Some(DamagePreventionAdditionalEffect::PutCounters { counter, basis }) =
            additional_effect
        {
            let amount = match basis {
                PreventionAmountBasis::Attempted => application_attempted,
                PreventionAmountBasis::Prevented => prevented,
            };
            if amount > 0 {
                if let DamageRecipient::Permanent(recipient) = event.recipient {
                    let recipient_label =
                        object_display_name(&self.state, self.registry, recipient);
                    if self.place_counters(
                        recipient,
                        counter,
                        amount,
                        super::continuous::CounterPlacementOrigin::Effect,
                    ) > 0
                    {
                        events.push(ev_log(format!(
                            "{source_label} puts {amount} {} counter(s) on {recipient_label}.",
                            counter.label()
                        )));
                    }
                }
            }
        }
        if exhausted {
            self.state
                .damage_prevention_effects
                .retain(|effect| effect.id != effect_id);
        }
        Ok(true)
    }

    fn park_damage_replacement_choice(
        &mut self,
        completion: DamageBatchContinuation,
        mut batch: PendingDamageBatch,
        raw_candidates: Vec<(usize, DamageReplacementApplication, String)>,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let deciding_event = &batch.damage[raw_candidates[0].0].spec.event;
        let deciding_player = self
            .damage_affected_player(deciding_event)
            .unwrap_or_else(|| completion.fallback_controller());
        let source_object_id = completion.source_object_id(&batch);
        let mut applications = Vec::new();
        let mut candidates = Vec::new();
        let mut candidate_names = Vec::new();
        let mut candidate_effect_ids = Vec::new();
        let mut replacement_options = Vec::new();
        for (event_index, application, effect_label) in raw_candidates {
            let choice_id = self.state.next_replacement_application_id;
            self.state.next_replacement_application_id = choice_id.saturating_add(1);
            applications.push(DamageApplicationChoice {
                choice_id,
                application,
                event_index,
            });
            candidates.push(choice_id);
            candidate_effect_ids.push(match application {
                DamageReplacementApplication::Effect(effect_id) => effect_id,
                DamageReplacementApplication::DoubleEffect(effect_id) => effect_id,
                DamageReplacementApplication::Protection(_) => 0,
            });
            let damage = &batch.damage[event_index];
            let (source, description) = match application {
                DamageReplacementApplication::Effect(effect_id) => self
                    .state
                    .damage_prevention_effects
                    .iter()
                    .find(|effect| effect.id == effect_id)
                    .map(|effect| {
                        let mut description = match effect.amount {
                            DamagePreventionAmount::All => "Prevent all damage.".to_string(),
                            DamagePreventionAmount::FixedPerEvent(amount) =>
                                format!("Prevent {amount} damage from this event."),
                            DamagePreventionAmount::Remaining(amount) =>
                                format!("Prevent the next {amount} damage."),
                        };
                        if let Some(DamagePreventionAdditionalEffect::PutCounters { counter, basis }) =
                            effect.additional_effect
                        {
                            let quantity = match basis {
                                PreventionAmountBasis::Attempted => "that would be dealt",
                                PreventionAmountBasis::Prevented => "prevented this way",
                            };
                            description.push_str(&format!(
                                " Put a {} counter on the affected permanent for each 1 damage {quantity}.",
                                counter.label()
                            ));
                        }
                        (effect.source_presentation.clone(), description)
                    })
                    .unwrap_or_else(|| (Default::default(), effect_label.clone())),
                DamageReplacementApplication::DoubleEffect(effect_id) => self
                    .state
                    .damage_doubling_effects
                    .iter()
                    .find(|effect| effect.id == effect_id)
                    .map(|effect| {
                        (
                            effect.source_presentation.clone(),
                            "Double this damage.".to_string(),
                        )
                    })
                    .unwrap_or_else(|| (Default::default(), effect_label.clone())),
                DamageReplacementApplication::Protection(_) => {
                    let source = match damage.spec.event.recipient {
                        DamageRecipient::Permanent(object_id) => {
                            self.replacement_source_presentation(object_id)
                        }
                        DamageRecipient::Player(_) => Default::default(),
                    };
                    (source, format!("{effect_label}: Prevent all damage."))
                },
            };
            let recipient = match damage.spec.event.recipient {
                DamageRecipient::Player(player) => format!("player {player}"),
                DamageRecipient::Permanent(oid) => format!(
                    "{} (object {oid})",
                    object_display_name(&self.state, self.registry, oid)
                ),
            };
            let summary = format!(
                "{description} ({} damage from {} to {recipient}.)",
                damage.remaining, damage.spec.event.source.label
            );
            replacement_options.push(source.option(choice_id, summary.clone()));
            candidate_names.push(summary);
        }
        let prompt = "Choose the next damage replacement or prevention effect. Your choice applies immediately; then remaining effects are reevaluated.".to_string();
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    variable_mana_contribution: false,
                    candidate_token_identities: Vec::new(),
                    candidate_player_ids: Vec::new(),
                    deciding_player_id: deciding_player,
                    source_object_id,
                    prompt_text: prompt.clone(),
                    choice_kind: rv1::ChoiceKind::ReplacementEffect as i32,
                    candidate_object_ids: candidates.clone(),
                    candidate_card_ids: vec![String::new(); candidates.len()],
                    min: 1,
                    max: 1,
                    ordered: false,
                    candidate_names,
                    candidate_server_card_ids: Vec::new(),
                    candidate_selectable: Vec::new(),
                    resolution_branches: Vec::new(),
                    mana_cost: String::new(),
                    unique_names: false,
                    generic_mana_cost: 0,
                    payment_currently_legal: false,
                    public_reveal: None,
                    candidate_source_zones: Vec::new(),
                    combat_defender_options: Vec::new(),
                    waterbend: false,
                    selection_slots: Vec::new(),
                    replacement_options,
                    selection_alternatives: Vec::new(),
                },
            )),
        });
        events.push(ev_log(prompt.clone()));
        batch.applications = applications;
        self.state.pending_replacement_event =
            Some(super::replacement::PendingReplacementEvent::Damage(batch));
        self.state.pending_resolution = Some(PendingResolution {
            deciding_player,
            presentation: PendingResolutionPresentation {
                source_object_id,
                candidates,
                min: 1,
                max: 1,
                ordered: false,
                prompt,
                choice_kind: rv1::ChoiceKind::ReplacementEffect,
                unique_names: false,
            },
            continuation: match completion {
                DamageBatchContinuation::Combat => {
                    ResolutionContinuation::CombatDamageReplacement {
                        effect_ids: candidate_effect_ids,
                    }
                }
                DamageBatchContinuation::Stack {
                    item,
                    resume_effect_index,
                } => ResolutionContinuation::DamageReplacement {
                    stack: ParkedStackResolution {
                        item: *item,
                        resume_effect_index,
                        previous_result: EffectResult::default(),
                    },
                    effect_ids: candidate_effect_ids,
                },
                DamageBatchContinuation::ManaAbility {
                    actor,
                    source_object_id,
                    source_zone_change_generation,
                    resume_resolution,
                } => ResolutionContinuation::ManaAbilityDamageReplacement {
                    actor,
                    source_object_id,
                    source_zone_change_generation,
                    resume_resolution,
                },
            },
        });
    }

    pub(crate) fn process_or_park_combat_damage(
        &mut self,
        event: DamageEvent,
        source_has_deathtouch: bool,
        source_has_lifelink: bool,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<Option<DamageResult>, EngineError> {
        self.process_prepared_combat_damage_batch(
            vec![DamageSpec {
                event,
                source_has_deathtouch,
                source_has_lifelink,
            }],
            events,
        )
        .map(|completed| {
            completed.and_then(|completed| completed.first().map(|damage| damage.result))
        })
    }

    fn capture_combat_damage_source(&self, event: &mut DamageEvent) {
        let source = event.source.object_id;
        event.source.zone_change_generation = self.state.objects.contains_key(&source).then(|| {
            self.state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0)
        });
        event.source.wither = self.effective_has_keyword(source, Keyword::Wither);
        if let Some(controller) = self
            .state
            .objects
            .get(&source)
            .filter(|object| object.zone == Zone::Battlefield)
            .and_then(|_| self.controller_of(source))
        {
            event.source.controller = controller;
        }
        if let Some(characteristics) = self.characteristics(source) {
            event.source.colors = characteristics.colors;
            event.source.types = characteristics.types;
        }
    }

    fn combat_damage_occurrence(
        &self,
        event: &DamageEvent,
    ) -> Option<PendingCombatDamageOccurrence> {
        let combat = self.state.combat.as_ref()?;
        let attacker = if combat.attacking.contains(&event.source.object_id) {
            event.source.object_id
        } else if let DamageRecipient::Permanent(recipient) = event.recipient {
            recipient
        } else {
            return None;
        };
        let assignment = combat.attack_assignments.get(&attacker)?;
        Some(PendingCombatDamageOccurrence {
            attacker: assignment.attacker,
            recipient_generation: match event.recipient {
                DamageRecipient::Player(_) => None,
                DamageRecipient::Permanent(oid) => {
                    self.state.objects.contains_key(&oid).then(|| {
                        self.state
                            .zone_change_generation
                            .get(&oid)
                            .copied()
                            .unwrap_or(0)
                    })
                }
            },
        })
    }

    fn process_prepared_combat_damage_batch(
        &mut self,
        damage: Vec<DamageSpec>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<Option<Vec<CompletedDamage>>, EngineError> {
        let pending = PendingDamageBatch {
            damage: damage
                .into_iter()
                .map(|mut spec| {
                    // Capture before any application, then retain unchanged through every choice.
                    self.capture_combat_damage_source(&mut spec.event);
                    PendingDamageEvent {
                        recipient_generation: self.damage_recipient_generation(&spec.event),
                        remaining: spec.event.amount,
                        prevented_total: 0,
                        combat: self.combat_damage_occurrence(&spec.event),
                        spec,
                        applied_applications: Vec::new(),
                        prevention_debits: Vec::new(),
                    }
                })
                .collect(),
            applications: Vec::new(),
        };
        match self.advance_damage_batch(pending, events)? {
            DamageBatchProgress::Complete(completed) => Ok(Some(completed)),
            DamageBatchProgress::NeedsChoice {
                batch,
                raw_candidates,
            } => {
                self.state.combat_damage_priority_pending = true;
                self.park_damage_replacement_choice(
                    DamageBatchContinuation::Combat,
                    batch,
                    raw_candidates,
                    events,
                );
                Ok(None)
            }
        }
    }

    /// Preflight a simultaneous combat-damage batch. The legacy commit loop remains the fast path
    /// when there is no doubling, Wither, or ordering decision. Damage replacements use the
    /// shared result pipeline; CR 616 ordering parks the whole batch before any damage is committed.
    pub(super) fn try_park_ordered_combat_damage(
        &mut self,
        combat: &CombatState,
        pass: super::combat::DamagePass,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<bool, EngineError> {
        let active = self.state.active_player_id();
        let mut damage = Vec::new();

        let mut push =
            |engine: &GameEngine, source: ObjectId, recipient: DamageRecipient, amount: u32| {
                if amount == 0 {
                    return;
                }
                let controller = engine
                    .state
                    .objects
                    .get(&source)
                    .map(|object| object.controller)
                    .unwrap_or(active);
                let mut event = DamageEvent::combat(
                    source,
                    controller,
                    object_display_name(&engine.state, engine.registry, source),
                    recipient,
                    amount,
                );
                engine.capture_combat_damage_source(&mut event);
                damage.push(DamageSpec {
                    event,
                    source_has_deathtouch: engine
                        .effective_has_keyword(source, Keyword::Deathtouch),
                    source_has_lifelink: engine.effective_has_keyword(source, Keyword::Lifelink),
                });
            };

        for &attacker in &combat.attacking {
            if self.state.objects.get(&attacker).map(|object| object.zone)
                != Some(Zone::Battlefield)
            {
                continue;
            }
            let attacker_participates =
                super::combat::object_participates_in_pass(self, combat, pass, attacker, true);
            let attacker_power = self.effective_power(attacker).unwrap_or(0);
            let has_trample = self.effective_has_keyword(attacker, Keyword::Trample);
            let blockers = combat
                .blockers
                .get(&attacker)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            if blockers.is_empty() {
                if attacker_participates {
                    if let Some(recipient) = self.combat_defender_recipient(combat, attacker) {
                        push(self, attacker, recipient, attacker_power);
                    }
                }
                continue;
            }

            for &blocker in blockers {
                let participates =
                    super::combat::object_participates_in_pass(self, combat, pass, blocker, false)
                        && self.state.objects.get(&blocker).map(|object| object.zone)
                            == Some(Zone::Battlefield);
                if participates {
                    push(
                        self,
                        blocker,
                        DamageRecipient::Permanent(attacker),
                        self.effective_power(blocker).unwrap_or(0),
                    );
                }
            }

            if !attacker_participates {
                continue;
            }
            if blockers.len() == 1 && !has_trample {
                push(
                    self,
                    attacker,
                    DamageRecipient::Permanent(blockers[0]),
                    attacker_power,
                );
            } else {
                let assignments =
                    combat
                        .damage_assignments
                        .get(&attacker)
                        .ok_or(EngineError::Illegal(
                            "combat damage assignments missing for multiply-blocked attacker",
                        ))?;
                for &(blocker, amount) in assignments {
                    push(self, attacker, DamageRecipient::Permanent(blocker), amount);
                }
                let player_damage = combat
                    .trample_player_damage
                    .get(&attacker)
                    .copied()
                    .unwrap_or(0);
                if let Some(recipient) = self.combat_defender_recipient(combat, attacker) {
                    push(self, attacker, recipient, player_damage);
                }
            }
        }

        if !self.damage_batch_needs_ordering(&damage)
            && !damage
                .iter()
                .any(|d| self.effective_has_keyword(d.event.source.object_id, Keyword::Wither))
        {
            return Ok(false);
        }
        let completed = self.process_prepared_combat_damage_batch(damage, events)?;
        if let Some(completed) = completed {
            self.commit_completed_damage_batch(&completed, events)?;
        }
        Ok(true)
    }

    fn combat_damage_occurrence_survives(&self, damage: &PendingDamageEvent) -> bool {
        let Some(original) = &damage.combat else {
            return false;
        };
        let event = &damage.spec.event;
        let current_permanent = |oid, generation| {
            self.state
                .objects
                .get(&oid)
                .is_some_and(|object| object.zone == Zone::Battlefield)
                && Some(
                    self.state
                        .zone_change_generation
                        .get(&oid)
                        .copied()
                        .unwrap_or(0),
                ) == generation
        };
        if !current_permanent(event.source.object_id, event.source.zone_change_generation)
            || !current_permanent(
                original.attacker.object_id,
                Some(original.attacker.zone_change_generation),
            )
        {
            return false;
        }
        match event.recipient {
            DamageRecipient::Player(player) => {
                if self
                    .state
                    .player_idx(player)
                    .is_none_or(|index| self.state.players[index].has_lost)
                {
                    return false;
                }
            }
            DamageRecipient::Permanent(oid) => {
                if !current_permanent(oid, original.recipient_generation) {
                    return false;
                }
            }
        }
        let Some(combat) = &self.state.combat else {
            return false;
        };
        let attacker = original.attacker.object_id;
        let Some(assignment) = combat.attack_assignments.get(&attacker) else {
            return false;
        };
        if assignment.attacker != original.attacker
            || !combat.attacking.contains(&attacker)
            || self
                .state
                .player_idx(assignment.defending_player)
                .is_none_or(|index| self.state.players[index].has_lost)
        {
            return false;
        }
        let blockers = combat
            .blockers
            .get(&attacker)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if event.source.object_id == attacker {
            match (event.recipient, assignment.defender) {
                (DamageRecipient::Player(player), CombatDefenderTarget::Player(defender)) => {
                    player == defender
                }
                (DamageRecipient::Permanent(oid), _) if blockers.contains(&oid) => true,
                (DamageRecipient::Permanent(oid), CombatDefenderTarget::Permanent(defender)) => {
                    oid == defender.object_id
                        && original.recipient_generation == Some(defender.zone_change_generation)
                }
                _ => false,
            }
        } else {
            blockers.contains(&event.source.object_id)
                && event.recipient == DamageRecipient::Permanent(attacker)
        }
    }

    fn damage_recipient_generation(&self, event: &DamageEvent) -> Option<u64> {
        match event.recipient {
            DamageRecipient::Player(_) => None,
            DamageRecipient::Permanent(oid) => Some(
                self.state
                    .zone_change_generation
                    .get(&oid)
                    .copied()
                    .unwrap_or(0),
            ),
        }
    }

    fn stack_damage_occurrence_survives(&self, damage: &PendingDamageEvent) -> bool {
        match damage.spec.event.recipient {
            DamageRecipient::Player(player) => self
                .state
                .player_idx(player)
                .is_some_and(|index| !self.state.players[index].has_lost),
            DamageRecipient::Permanent(oid) => {
                self.state
                    .objects
                    .get(&oid)
                    .is_some_and(|object| object.zone == Zone::Battlefield)
                    && self.damage_recipient_generation(&damage.spec.event)
                        == damage.recipient_generation
            }
        }
    }

    /// Reconcile recipients without cancelling independently owed damage from an absent source.
    pub(super) fn refresh_damage_departure(
        &mut self,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        let Some(pending) = self.state.pending_resolution.clone() else {
            return Ok(());
        };
        let completion = match &pending.continuation {
            ResolutionContinuation::CombatDamageReplacement { .. } => {
                DamageBatchContinuation::Combat
            }
            ResolutionContinuation::DamageReplacement { stack, .. } => {
                DamageBatchContinuation::Stack {
                    item: Box::new(stack.item.clone()),
                    resume_effect_index: stack.resume_effect_index,
                }
            }
            _ => return Ok(()),
        };
        let Some(super::replacement::PendingReplacementEvent::Damage(mut batch)) =
            self.state.pending_replacement_event.take()
        else {
            return Err(EngineError::Illegal("damage continuation has no batch"));
        };
        let mut indices = Vec::with_capacity(batch.damage.len());
        let mut retained = Vec::new();
        for damage in std::mem::take(&mut batch.damage) {
            let survives = match completion {
                DamageBatchContinuation::Combat => self.combat_damage_occurrence_survives(&damage),
                _ => self.stack_damage_occurrence_survives(&damage),
            };
            if survives {
                indices.push(Some(retained.len()));
                retained.push(damage);
            } else {
                indices.push(None);
            }
        }
        batch.damage = retained;
        batch.applications.retain_mut(|application| {
            let Some(index) = indices[application.event_index] else {
                return false;
            };
            application.event_index = index;
            true
        });
        let old_applications = batch.applications.clone();
        self.state.pending_resolution = None;
        match self.advance_damage_batch(batch, events)? {
            DamageBatchProgress::NeedsChoice {
                mut batch,
                raw_candidates,
            } => {
                let deciding_player =
                    self.damage_affected_player(&batch.damage[raw_candidates[0].0].spec.event);
                let same_published_candidates = old_applications.len()
                    == pending.presentation.candidates.len()
                    && old_applications
                        .iter()
                        .all(|old| pending.presentation.candidates.contains(&old.choice_id));
                let same_pairs = same_published_candidates
                    && old_applications.len() == raw_candidates.len()
                    && old_applications.iter().all(|old| {
                        raw_candidates.iter().any(|(index, application, _)| {
                            *index == old.event_index && *application == old.application
                        })
                    });
                let same_anchor = batch.damage.iter().any(|damage| {
                    damage.spec.event.source.object_id == pending.presentation.source_object_id
                });
                if deciding_player == Some(pending.deciding_player) && same_pairs && same_anchor {
                    batch.applications = old_applications;
                    self.state.pending_replacement_event =
                        Some(super::replacement::PendingReplacementEvent::Damage(batch));
                    self.state.pending_resolution = Some(pending);
                } else {
                    self.park_damage_replacement_choice(completion, batch, raw_candidates, events);
                }
            }
            DamageBatchProgress::Complete(completed) => {
                self.commit_completed_damage_batch(&completed, events)?;
                match completion {
                    DamageBatchContinuation::Combat => {
                        self.state.combat_damage_priority_pending = true
                    }
                    DamageBatchContinuation::Stack {
                        item,
                        resume_effect_index,
                    } => {
                        let completed = self.complete_parked_resolution(
                            *item,
                            resume_effect_index,
                            Vec::new(),
                        )?;
                        events.extend(completed.events);
                    }
                    DamageBatchContinuation::ManaAbility { .. } => {
                        unreachable!("departure refresh excludes mana abilities")
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) fn finish_damage_replacement_choice(
        &mut self,
        pending: PendingResolution,
        chosen_application_id: u32,
    ) -> Result<RuledEventBatch, EngineError> {
        let completion = match &pending.continuation {
            ResolutionContinuation::CombatDamageReplacement { .. } => {
                DamageBatchContinuation::Combat
            }
            ResolutionContinuation::DamageReplacement { stack, .. } => {
                DamageBatchContinuation::Stack {
                    item: Box::new(stack.item.clone()),
                    resume_effect_index: stack.resume_effect_index,
                }
            }
            ResolutionContinuation::ManaAbilityDamageReplacement {
                actor,
                source_object_id,
                source_zone_change_generation,
                resume_resolution,
            } => DamageBatchContinuation::ManaAbility {
                actor: *actor,
                source_object_id: *source_object_id,
                source_zone_change_generation: *source_zone_change_generation,
                resume_resolution: resume_resolution.clone(),
            },
            _ => {
                return Err(EngineError::Illegal(
                    "damage-replacement continuation missing",
                ))
            }
        };
        let Some(pending_event) = self.state.pending_replacement_event.take() else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("damage-replacement choice is stale"));
        };
        let mut batch = match pending_event {
            super::replacement::PendingReplacementEvent::Damage(batch) => batch,
            other => {
                self.state.pending_replacement_event = Some(other);
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("damage-replacement choice is stale"));
            }
        };
        let Some(application) = batch
            .applications
            .iter()
            .find(|application| application.choice_id == chosen_application_id)
            .cloned()
        else {
            self.state.pending_replacement_event =
                Some(super::replacement::PendingReplacementEvent::Damage(batch));
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "damage-replacement application is stale",
            ));
        };
        let mut events = Vec::new();
        if !self.apply_damage_replacement_application(
            &mut batch,
            application.event_index,
            application.application,
            &mut events,
        )? {
            self.state.pending_replacement_event =
                Some(super::replacement::PendingReplacementEvent::Damage(batch));
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "damage-replacement effect is no longer active",
            ));
        }
        let completed = match self.advance_damage_batch(batch, &mut events)? {
            DamageBatchProgress::Complete(completed) => completed,
            DamageBatchProgress::NeedsChoice {
                batch,
                raw_candidates,
            } => {
                self.park_damage_replacement_choice(
                    completion.clone(),
                    batch,
                    raw_candidates,
                    &mut events,
                );
                return Ok(finish_with_events(self, events));
            }
        };
        let damage_triggers =
            self.commit_completed_damage_batch_collecting_triggers(&completed, &mut events)?;
        if let DamageBatchContinuation::ManaAbility {
            actor,
            source_object_id,
            ..
        } = &completion
        {
            self.record_attack_mana_damage_results(
                *actor,
                *source_object_id,
                &completed,
                damage_triggers,
            );
        }
        match completion {
            DamageBatchContinuation::Combat => {
                self.state.combat_damage_priority_pending = true;
                Ok(finish_with_events(self, events))
            }
            DamageBatchContinuation::Stack {
                item,
                resume_effect_index,
            } => self.complete_parked_resolution(*item, resume_effect_index, events),
            DamageBatchContinuation::ManaAbility {
                resume_resolution, ..
            } => {
                if let Some(resume_resolution) = resume_resolution {
                    self.state.pending_resolution = Some(*resume_resolution);
                    events.push(
                        self.resolution_payment_choice_event()
                            .expect("suspended resolution payment has a prompt"),
                    );
                }
                Ok(finish_with_events(self, events))
            }
        }
    }

    pub(crate) fn commit_damage_result(
        &mut self,
        event: &DamageEvent,
        result: DamageResult,
        source_has_deathtouch: bool,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<u32, EngineError> {
        match event.recipient {
            DamageRecipient::Player(player) => {
                let Some(index) = self.state.player_idx(player) else {
                    return Ok(0);
                };
                if self.state.players[index].has_lost {
                    return Ok(0);
                }
                let delta = super::life_numeric::loss_delta(result.dealt)?;
                super::history::commit_life_change_checked(&mut self.state, index, delta)?;
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                        player_id: player,
                        new_total: self.state.players[index].life,
                        delta,
                    })),
                });
                if result.dealt > 0 {
                    events.push(ev_log(format!(
                        "{} deals {} damage to P{player}",
                        event.source.label, result.dealt
                    )));
                }
                Ok(result.dealt)
            }
            DamageRecipient::Permanent(permanent) => {
                let label = object_display_name(&self.state, self.registry, permanent);
                let Some(characteristics) = self.characteristics(permanent) else {
                    return Ok(0);
                };
                let is_creature = characteristics.is_creature();
                let is_planeswalker = characteristics.has_type("Planeswalker");
                let is_battle = characteristics.has_type("Battle");
                let Some(object) = self.state.objects.get(&permanent) else {
                    return Ok(0);
                };
                if object.zone != Zone::Battlefield
                    || !(is_creature || is_planeswalker || is_battle)
                {
                    return Ok(0);
                }
                let was_defended = object.counter_count(CounterKind::Defense) > 0;
                let marked_damage = if is_creature && !event.source.wither {
                    Some(
                        object
                            .damage
                            .checked_add(result.dealt)
                            .ok_or(EngineError::DamageNumericRange("marked damage total"))?,
                    )
                } else {
                    None
                };
                if is_creature
                    && event.source.wither
                    && result.dealt > 0
                    && self.can_receive_counters(permanent)
                {
                    object
                        .counter_count(CounterKind::MinusOneMinusOne)
                        .checked_add(result.dealt)
                        .ok_or(EngineError::DamageNumericRange("wither counter total"))?;
                }
                let Some(object) = self.state.objects.get_mut(&permanent) else {
                    return Ok(0);
                };
                if is_creature {
                    if let Some(marked_damage) = marked_damage {
                        object.damage = marked_damage;
                    }
                    if source_has_deathtouch && result.dealt > 0 {
                        object.deathtouch_damage = true;
                    }
                }
                if is_planeswalker {
                    let loyalty = object.counter_count(CounterKind::Loyalty);
                    object.set_counter(CounterKind::Loyalty, loyalty.saturating_sub(result.dealt));
                }
                if is_battle {
                    let defense = object.counter_count(CounterKind::Defense);
                    object.set_counter(CounterKind::Defense, defense.saturating_sub(result.dealt));
                }
                if result.dealt > 0 {
                    events.push(ev_log(format!(
                        "{} deals {} damage to {label}",
                        event.source.label, result.dealt
                    )));
                }
                let defeated_siege = is_battle
                    && was_defended
                    && object.counter_count(CounterKind::Defense) == 0
                    && characteristics.has_type("Siege");
                let _ = object;
                if is_creature && event.source.wither {
                    self.place_counters(
                        permanent,
                        CounterKind::MinusOneMinusOne,
                        result.dealt,
                        super::continuous::CounterPlacementOrigin::Damage,
                    );
                }
                if defeated_siege {
                    self.stage_siege_defeat_trigger(permanent);
                }
                Ok(result.dealt)
            }
        }
    }

    pub(crate) fn commit_completed_damage_batch(
        &mut self,
        completed: &[CompletedDamage],
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        self.commit_completed_damage_batch_internal(completed, events, None)
            .map(|_| ())
    }

    fn commit_completed_damage_batch_collecting_triggers(
        &mut self,
        completed: &[CompletedDamage],
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<Vec<super::triggers::CollectedTrigger>, EngineError> {
        self.commit_completed_damage_batch_internal(completed, events, None)
    }

    pub(super) fn reapply_completed_damage_batch_with_triggers(
        &mut self,
        completed: &[CompletedDamage],
        trigger_snapshot: &[super::triggers::CollectedTrigger],
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Result<(), EngineError> {
        self.commit_completed_damage_batch_internal(completed, events, Some(trigger_snapshot))
            .map(|_| ())
    }

    fn commit_completed_damage_batch_internal(
        &mut self,
        completed: &[CompletedDamage],
        events: &mut Vec<rv1::RuledEvent>,
        replay_trigger_snapshot: Option<&[super::triggers::CollectedTrigger]>,
    ) -> Result<Vec<super::triggers::CollectedTrigger>, EngineError> {
        let mut lifelink_by_source: BTreeMap<(ObjectId, PlayerId), u32> = BTreeMap::new();
        let mut trigger_events = Vec::new();
        for damage in completed {
            let dealt = self.commit_damage_result(
                &damage.spec.event,
                damage.result,
                damage.spec.source_has_deathtouch,
                events,
            )?;
            if dealt == 0 {
                continue;
            }
            if damage.spec.source_has_lifelink {
                let total = lifelink_by_source
                    .entry((
                        damage.spec.event.source.object_id,
                        damage.spec.event.source.controller,
                    ))
                    .or_insert(0);
                *total = total
                    .checked_add(dealt)
                    .ok_or(EngineError::LifeNumericRange("lifelink source sum"))?;
            }
            let mut event = damage.spec.event.clone();
            event.amount = dealt;
            trigger_events.push(GameEvent::DamageDealt { event });
        }
        for ((_, controller), dealt) in lifelink_by_source {
            if let Some(event) = super::resolution::life::apply_life_gain_without_triggers(
                self, events, controller, dealt, "lifelink",
            )? {
                trigger_events.push(event);
            }
        }
        if let Some(snapshot) = replay_trigger_snapshot {
            self.refresh_enduring_story_designations();
            self.reconcile_combat_characteristics(events);
            self.record_committed_events(&trigger_events);
            self.stage_triggers(snapshot.to_vec());
            Ok(snapshot.to_vec())
        } else {
            Ok(self.fire_triggers_collecting(&trigger_events, events))
        }
    }

    pub(crate) fn add_damage_prevention(
        &mut self,
        source: Option<&StackItem>,
        source_label: impl Into<String>,
        scope: DamagePreventionScope,
        amount: DamagePreventionAmount,
    ) -> u32 {
        let id = self.state.next_damage_prevention_effect_id;
        self.state.next_damage_prevention_effect_id = id.saturating_add(1);
        let source_presentation = source
            .map(|item| self.replacement_stack_source_presentation(item))
            .unwrap_or_default();
        self.state
            .damage_prevention_effects
            .push(ActiveDamagePrevention {
                id,
                static_origin: None,
                source_id: source.map(|item| item.id),
                source_label: source_label.into(),
                source_presentation,
                scope,
                amount,
                duration: EffectDuration::UntilEndOfTurn,
                additional_effect: None,
            });
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controlled_creature_damage_doubler_replaces_noncombat_damage() {
        let (mut engine, source) = damage_doubler_engine(
            r#"(ability_id: "double_damage", presentation: Fallback,
                definition: DoubleDamage(subject: CreatureYouControl))"#,
        );
        assert_eq!(engine.state.damage_doubling_effects.len(), 1);

        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let mut item = source_item(source, generation);
        item.card_id = "damage_doubler_fixture".into();
        let completed = engine
            .process_or_park_damage_batch(
                &item,
                vec![DamageSpec {
                    event: DamageEvent::noncombat(
                        source,
                        0,
                        "Damage Doubler Fixture",
                        DamageRecipient::Player(1),
                        3,
                    ),
                    source_has_deathtouch: false,
                    source_has_lifelink: false,
                }],
                &mut Vec::new(),
            )
            .expect("damage batch processes")
            .expect("unprevented replacement completes immediately");

        assert_eq!(completed[0].result.attempted, 3);
        assert_eq!(completed[0].result.dealt, 6);
        assert_eq!(completed[0].result.prevented, 0);
        engine
            .commit_completed_damage_batch(&completed, &mut Vec::new())
            .expect("damage commits");
        assert_eq!(engine.state.players[1].life, 14);
    }

    #[test]
    fn damage_doubler_follows_its_current_controller_and_stops_when_suppressed() {
        let (mut engine, source) = damage_doubler_engine(
            r#"(ability_id: "double_damage", presentation: Fallback,
                definition: DoubleDamage(subject: CreatureYouControl))"#,
        );
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(source),
            kind: ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(1),
            },
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 1,
        });

        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let mut item = source_item(source, generation);
        item.card_id = "damage_doubler_fixture".into();
        let controlled = engine
            .process_or_park_damage_batch(
                &item,
                vec![DamageSpec {
                    event: DamageEvent::noncombat(
                        source,
                        1,
                        "Damage Doubler Fixture",
                        DamageRecipient::Player(0),
                        3,
                    ),
                    source_has_deathtouch: false,
                    source_has_lifelink: false,
                }],
                &mut Vec::new(),
            )
            .expect("controlled source damage processes")
            .expect("controlled source damage completes immediately");
        assert_eq!(controlled[0].result.dealt, 6);

        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(source),
            kind: ContinuousEffectKind::Layer6RemoveAllAbilities,
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 2,
        });
        let suppressed = engine
            .process_or_park_damage_batch(
                &item,
                vec![DamageSpec {
                    event: DamageEvent::noncombat(
                        source,
                        1,
                        "Damage Doubler Fixture",
                        DamageRecipient::Player(0),
                        3,
                    ),
                    source_has_deathtouch: false,
                    source_has_lifelink: false,
                }],
                &mut Vec::new(),
            )
            .expect("suppressed source damage processes")
            .expect("suppressed source damage completes immediately");
        assert_eq!(suppressed[0].result.dealt, 3);
    }

    #[test]
    fn combat_damage_uses_the_effective_controller_of_a_stolen_creature() {
        let (mut engine, doubler) = damage_doubler_engine(
            r#"(ability_id: "double_damage", presentation: Fallback,
                definition: DoubleDamage(subject: CreatureYouControl))"#,
        );
        let attacker = move_fixture_source_to_battlefield(&mut engine, "plain_creature_fixture", 0);
        for (timestamp, object_id) in [(1, doubler), (2, attacker)] {
            engine.state.continuous_effects.push(ContinuousEffect {
                trigger_grant_origin: None,
                source_id: None,
                affected: AffectedScope::Single(object_id),
                kind: ContinuousEffectKind::Layer2Control {
                    controller: ControllerReference::Fixed(1),
                },
                condition: None,
                duration: EffectDuration::UntilEndOfTurn,
                timestamp,
            });
        }

        let result = engine
            .process_or_park_combat_damage(
                DamageEvent::combat(
                    attacker,
                    0,
                    "stolen creature",
                    DamageRecipient::Player(0),
                    2,
                ),
                false,
                false,
                &mut Vec::new(),
            )
            .expect("combat damage processes")
            .expect("one static doubler needs no ordering choice");

        assert_eq!(result.attempted, 2);
        assert_eq!(result.dealt, 4);
    }

    #[test]
    fn controlled_permanent_orders_damage_replacements_by_its_effective_controller() {
        let (mut engine, source) = damage_doubler_engine(
            r#"(ability_id: "double_damage", presentation: Fallback,
                definition: DoubleDamage(subject: AnySource))"#,
        );
        let target = move_fixture_source_to_battlefield(&mut engine, "plain_creature_fixture", 0);
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(target),
            kind: ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(1),
            },
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 1,
        });
        engine.add_damage_prevention(
            None,
            "one damage shield",
            DamagePreventionScope::Recipient(target),
            DamagePreventionAmount::FixedPerEvent(1),
        );
        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let mut item = source_item(source, generation);
        item.card_id = "damage_doubler_fixture".into();

        assert!(engine
            .process_or_park_damage_batch(
                &item,
                vec![DamageSpec {
                    event: DamageEvent::noncombat(
                        source,
                        0,
                        "damage source",
                        DamageRecipient::Permanent(target),
                        2,
                    ),
                    source_has_deathtouch: false,
                    source_has_lifelink: false,
                }],
                &mut Vec::new(),
            )
            .expect("replacement choice is valid")
            .is_none());
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .expect("two replacements require an order")
                .deciding_player,
            1
        );
    }

    #[test]
    fn leaving_battlefield_removes_a_static_damage_doubler() {
        let (mut engine, source) = damage_doubler_engine(
            r#"(ability_id: "double_damage", presentation: Fallback,
                definition: DoubleDamage(subject: CreatureYouControl))"#,
        );
        assert_eq!(engine.state.damage_doubling_effects.len(), 1);

        super::super::resolution::move_object_to_zone(
            &mut engine.state,
            engine.registry,
            source,
            Zone::Graveyard,
            None,
        )
        .expect("source leaves the battlefield");

        assert!(
            engine.state.damage_doubling_effects.is_empty(),
            "the battlefield static is removed with its source"
        );
    }

    fn damage_doubler_engine(abilities: &str) -> (GameEngine, ObjectId) {
        let fixture = format!(
            r#"(
                id: "damage_doubler_fixture",
                name: "Damage Doubler Fixture",
                face_id: "damage_doubler_fixture",
                types: ["Creature"],
                power: 2,
                toughness: 2,
                static_abilities: [{abilities}],
            )"#
        );
        let plain_creature = r#"(
            id: "plain_creature_fixture",
            name: "Plain Creature Fixture",
            face_id: "plain_creature_fixture",
            types: ["Creature"],
            power: 2,
            toughness: 2,
        )"#;
        let noncreature = r#"(
            id: "noncreature_source_fixture",
            name: "Noncreature Source Fixture",
            face_id: "noncreature_source_fixture",
            types: ["Enchantment"],
        )"#;
        let registry =
            CardRegistry::from_chunks_and_tokens(&[&fixture, plain_creature, noncreature], &[])
                .expect("damage-doubling static ability fixture");
        let registry = Box::leak(Box::new(registry));
        let decks = Some(vec![
            vec!["damage_doubler_fixture".into(); 7],
            vec!["damage_doubler_fixture".into(); 7],
        ]);
        let mut engine =
            GameEngine::new(registry, 614_501, &[0, 1], 20, decks, true).expect("engine");
        let source = engine.state.players[0].hand.remove(0);
        engine.state.players[0].battlefield.push(source);
        engine.state.objects.get_mut(&source).unwrap().zone = Zone::Battlefield;
        engine.emit_static_abilities_on_enter(source);
        (engine, source)
    }

    fn move_fixture_source_to_battlefield(
        engine: &mut GameEngine,
        card_id: &str,
        controller: PlayerId,
    ) -> ObjectId {
        let source = engine.state.players[0].hand.remove(0);
        engine.state.players[controller as usize]
            .battlefield
            .push(source);
        let object = engine.state.objects.get_mut(&source).unwrap();
        object.card_id = card_id.into();
        object.controller = controller;
        object.base_controller = controller;
        object.zone = Zone::Battlefield;
        source
    }

    fn grant_wither_for_damage_test(engine: &mut GameEngine, source: ObjectId) {
        engine.state.continuous_effects.push(ContinuousEffect {
            source_id: None,
            affected: AffectedScope::Single(source),
            kind: ContinuousEffectKind::Layer6AddKeyword(Keyword::Wither),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 0,
            trigger_grant_origin: None,
        });
    }

    #[test]
    fn prevention_and_doubling_order_changes_the_dealt_amount_and_prevented_total() {
        fn result_when_first_application_is_doubler(double_first: bool) -> DamageResult {
            let (mut engine, source) = damage_doubler_engine(
                r#"(ability_id: "double_damage", presentation: Fallback,
                    definition: DoubleDamage(subject: CreatureYouControl))"#,
            );
            let shield = engine.add_damage_prevention(
                None,
                "one damage shield",
                DamagePreventionScope::Recipient(1),
                DamagePreventionAmount::FixedPerEvent(1),
            );
            let generation = engine
                .state
                .zone_change_generation
                .get(&source)
                .copied()
                .unwrap_or(0);
            let mut item = source_item(source, generation);
            item.card_id = "damage_doubler_fixture".into();
            assert!(engine
                .process_or_park_damage_batch(
                    &item,
                    vec![DamageSpec {
                        event: DamageEvent::noncombat(
                            source,
                            0,
                            "Damage Doubler Fixture",
                            DamageRecipient::Player(1),
                            3,
                        ),
                        source_has_deathtouch: false,
                        source_has_lifelink: false,
                    }],
                    &mut Vec::new(),
                )
                .unwrap()
                .is_none());
            let Some(super::super::replacement::PendingReplacementEvent::Damage(mut batch)) =
                engine.state.pending_replacement_event.take()
            else {
                panic!("damage order choice must park the event")
            };
            engine.state.pending_resolution = None;
            let chosen = batch
                .applications
                .iter()
                .find(|application| {
                    matches!(
                        application.application,
                        DamageReplacementApplication::DoubleEffect(_)
                    ) == double_first
                })
                .expect("requested first application is offered")
                .clone();
            if !double_first {
                assert!(matches!(
                    chosen.application,
                    DamageReplacementApplication::Effect(id) if id == shield
                ));
            }
            engine
                .apply_damage_replacement_application(
                    &mut batch,
                    chosen.event_index,
                    chosen.application,
                    &mut Vec::new(),
                )
                .unwrap();
            let DamageBatchProgress::Complete(completed) =
                engine.advance_damage_batch(batch, &mut Vec::new()).unwrap()
            else {
                panic!("the remaining single application resolves without another choice")
            };
            completed[0].result
        }

        let prevention_first = result_when_first_application_is_doubler(false);
        assert_eq!(prevention_first.attempted, 3);
        assert_eq!(prevention_first.dealt, 4);
        assert_eq!(prevention_first.prevented, 1);

        let doubling_first = result_when_first_application_is_doubler(true);
        assert_eq!(doubling_first.attempted, 3);
        assert_eq!(doubling_first.dealt, 5);
        assert_eq!(doubling_first.prevented, 1);
    }

    #[test]
    fn each_doubler_applies_once_and_two_instances_are_ordered() {
        let (mut engine, source) = damage_doubler_engine(
            r#"(ability_id: "first_double", presentation: Fallback,
                definition: DoubleDamage(subject: AnySource)),
                (ability_id: "second_double", presentation: Fallback,
                definition: DoubleDamage(subject: AnySource))"#,
        );
        assert_eq!(engine.state.damage_doubling_effects.len(), 2);
        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let mut item = source_item(source, generation);
        item.card_id = "damage_doubler_fixture".into();
        assert!(engine
            .process_or_park_damage_batch(
                &item,
                vec![DamageSpec {
                    event: DamageEvent::noncombat(
                        source,
                        0,
                        "Damage Doubler Fixture",
                        DamageRecipient::Player(1),
                        2,
                    ),
                    source_has_deathtouch: false,
                    source_has_lifelink: false,
                }],
                &mut Vec::new(),
            )
            .unwrap()
            .is_none());
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            1,
            "the damaged player orders both replacements"
        );
        let Some(super::super::replacement::PendingReplacementEvent::Damage(mut batch)) =
            engine.state.pending_replacement_event.take()
        else {
            panic!("two doublers require an ordering choice")
        };
        engine.state.pending_resolution = None;
        let chosen = batch.applications[0].clone();
        engine
            .apply_damage_replacement_application(
                &mut batch,
                chosen.event_index,
                chosen.application,
                &mut Vec::new(),
            )
            .unwrap();
        let DamageBatchProgress::Complete(completed) =
            engine.advance_damage_batch(batch, &mut Vec::new()).unwrap()
        else {
            panic!("the remaining single doubler resolves without another choice")
        };
        assert_eq!(completed[0].result.attempted, 2);
        assert_eq!(completed[0].result.dealt, 8);
        assert_eq!(completed[0].result.prevented, 0);
    }

    #[test]
    fn creature_you_control_scope_uses_the_event_source_and_ability_controller() {
        for (card_id, controller, expected) in [
            ("plain_creature_fixture", 1, 3),
            ("noncreature_source_fixture", 0, 3),
        ] {
            let (mut engine, doubler) = damage_doubler_engine(
                r#"(ability_id: "double_damage", presentation: Fallback,
                    definition: DoubleDamage(subject: CreatureYouControl))"#,
            );
            let source = move_fixture_source_to_battlefield(&mut engine, card_id, controller);
            let generation = engine
                .state
                .zone_change_generation
                .get(&doubler)
                .copied()
                .unwrap_or(0);
            let mut item = source_item(doubler, generation);
            item.card_id = "damage_doubler_fixture".into();
            let completed = engine
                .process_or_park_damage_batch(
                    &item,
                    vec![DamageSpec {
                        event: DamageEvent::noncombat(
                            source,
                            0,
                            "test source",
                            DamageRecipient::Player(1),
                            3,
                        ),
                        source_has_deathtouch: false,
                        source_has_lifelink: false,
                    }],
                    &mut Vec::new(),
                )
                .unwrap()
                .expect("one nonmatching source has no ordering choice");
            assert_eq!(completed[0].result.dealt, expected, "{card_id}");
        }
    }

    #[test]
    fn any_source_scope_doubles_noncreature_damage_from_an_opponent_controlled_source() {
        let (mut engine, doubler) = damage_doubler_engine(
            r#"(ability_id: "double_damage", presentation: Fallback,
                definition: DoubleDamage(subject: AnySource))"#,
        );
        let source =
            move_fixture_source_to_battlefield(&mut engine, "noncreature_source_fixture", 1);
        let generation = engine
            .state
            .zone_change_generation
            .get(&doubler)
            .copied()
            .unwrap_or(0);
        let mut item = source_item(doubler, generation);
        item.card_id = "damage_doubler_fixture".into();
        let completed = engine
            .process_or_park_damage_batch(
                &item,
                vec![DamageSpec {
                    event: DamageEvent::noncombat(
                        source,
                        1,
                        "opponent's enchantment source",
                        DamageRecipient::Player(0),
                        2,
                    ),
                    source_has_deathtouch: false,
                    source_has_lifelink: false,
                }],
                &mut Vec::new(),
            )
            .unwrap()
            .expect("one all-source doubler applies immediately");
        assert_eq!(completed[0].result.dealt, 4);
    }

    #[test]
    fn full_prevention_ends_the_event_before_an_unapplied_doubler() {
        let (mut engine, source) = damage_doubler_engine(
            r#"(ability_id: "double_damage", presentation: Fallback,
                definition: DoubleDamage(subject: AnySource))"#,
        );
        let shield = engine.add_damage_prevention(
            None,
            "all damage shield",
            DamagePreventionScope::Recipient(1),
            DamagePreventionAmount::All,
        );
        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let item = source_item(source, generation);
        assert!(engine
            .process_or_park_damage_batch(
                &item,
                vec![DamageSpec {
                    event: DamageEvent::noncombat(
                        source,
                        0,
                        "Damage Doubler Fixture",
                        DamageRecipient::Player(1),
                        3,
                    ),
                    source_has_deathtouch: false,
                    source_has_lifelink: false,
                }],
                &mut Vec::new(),
            )
            .unwrap()
            .is_none());
        let Some(super::super::replacement::PendingReplacementEvent::Damage(mut batch)) =
            engine.state.pending_replacement_event.take()
        else {
            panic!("prevention and doubling require an ordering choice");
        };
        engine.state.pending_resolution = None;
        let shield_choice = batch
            .applications
            .iter()
            .find(|application| {
                application.application == DamageReplacementApplication::Effect(shield)
            })
            .unwrap()
            .clone();
        engine
            .apply_damage_replacement_application(
                &mut batch,
                shield_choice.event_index,
                shield_choice.application,
                &mut Vec::new(),
            )
            .unwrap();
        let DamageBatchProgress::Complete(completed) =
            engine.advance_damage_batch(batch, &mut Vec::new()).unwrap()
        else {
            panic!("fully prevented damage cannot leave a pending replacement");
        };
        assert_eq!(completed[0].result.attempted, 3);
        assert_eq!(completed[0].result.dealt, 0);
        assert_eq!(completed[0].result.prevented, 3);
    }

    #[test]
    fn damage_prevention_prohibition_does_not_block_a_doubler() {
        let (mut engine, source) = damage_doubler_engine(
            r#"(ability_id: "double_damage", presentation: Fallback,
                definition: DoubleDamage(subject: AnySource))"#,
        );
        let shield = engine.add_damage_prevention(
            None,
            "all damage shield",
            DamagePreventionScope::Recipient(1),
            DamagePreventionAmount::All,
        );
        engine
            .state
            .damage_prevention_prohibitions
            .push(DamagePreventionProhibition { source_id: None });
        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let item = source_item(source, generation);
        assert!(engine
            .process_or_park_damage_batch(
                &item,
                vec![DamageSpec {
                    event: DamageEvent::noncombat(
                        source,
                        0,
                        "Damage Doubler Fixture",
                        DamageRecipient::Player(1),
                        3,
                    ),
                    source_has_deathtouch: false,
                    source_has_lifelink: false,
                }],
                &mut Vec::new(),
            )
            .unwrap()
            .is_none());
        let Some(super::super::replacement::PendingReplacementEvent::Damage(mut batch)) =
            engine.state.pending_replacement_event.take()
        else {
            panic!("the prohibited shield and doubler are both applicable choices");
        };
        engine.state.pending_resolution = None;
        let shield_choice = batch
            .applications
            .iter()
            .find(|application| {
                application.application == DamageReplacementApplication::Effect(shield)
            })
            .unwrap()
            .clone();
        engine
            .apply_damage_replacement_application(
                &mut batch,
                shield_choice.event_index,
                shield_choice.application,
                &mut Vec::new(),
            )
            .unwrap();
        let DamageBatchProgress::Complete(completed) =
            engine.advance_damage_batch(batch, &mut Vec::new()).unwrap()
        else {
            panic!("the remaining single doubler resolves automatically");
        };
        assert_eq!(completed[0].result.dealt, 6);
        assert_eq!(completed[0].result.prevented, 0);
        assert!(engine
            .state
            .damage_prevention_effects
            .iter()
            .any(|effect| effect.id == shield));
    }

    #[test]
    fn doubled_damage_cannot_saturate_marked_damage_total() {
        let (mut engine, source) = damage_doubler_engine(
            r#"(ability_id: "double_damage", presentation: Fallback,
                definition: DoubleDamage(subject: CreatureYouControl))"#,
        );
        let target = move_fixture_source_to_battlefield(&mut engine, "plain_creature_fixture", 1);
        engine.state.objects.get_mut(&target).unwrap().damage = u32::MAX;
        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let item = source_item(source, generation);
        let completed = engine
            .process_or_park_damage_batch(
                &item,
                vec![DamageSpec {
                    event: DamageEvent::noncombat(
                        source,
                        0,
                        "Damage Doubler Fixture",
                        DamageRecipient::Permanent(target),
                        1,
                    ),
                    source_has_deathtouch: false,
                    source_has_lifelink: false,
                }],
                &mut Vec::new(),
            )
            .unwrap()
            .expect("a single doubler resolves without an ordering choice");
        assert_eq!(completed[0].result.dealt, 2);
        assert!(matches!(
            engine.commit_completed_damage_batch(&completed, &mut Vec::new()),
            Err(EngineError::DamageNumericRange("marked damage total"))
        ));
        assert_eq!(engine.state.objects[&target].damage, u32::MAX);
    }

    #[test]
    fn doubled_wither_damage_places_counters_for_the_final_amount() {
        let (mut engine, source) = damage_doubler_engine(
            r#"(ability_id: "double_damage", presentation: Fallback,
                definition: DoubleDamage(subject: CreatureYouControl))"#,
        );
        grant_wither_for_damage_test(&mut engine, source);
        let target = move_fixture_source_to_battlefield(&mut engine, "plain_creature_fixture", 1);
        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let item = source_item(source, generation);
        let completed = engine
            .process_or_park_damage_batch(
                &item,
                vec![DamageSpec {
                    event: DamageEvent::noncombat(
                        source,
                        0,
                        "Damage Doubler Fixture",
                        DamageRecipient::Permanent(target),
                        1,
                    ),
                    source_has_deathtouch: false,
                    source_has_lifelink: false,
                }],
                &mut Vec::new(),
            )
            .unwrap()
            .expect("one controlled-creature doubler resolves immediately");
        assert_eq!(completed[0].result.dealt, 2);
        assert!(completed[0].spec.event.source.wither);
        engine
            .commit_completed_damage_batch(&completed, &mut Vec::new())
            .expect("doubled wither damage commits as counters");
        assert_eq!(engine.state.objects[&target].damage, 0);
        assert_eq!(
            engine.state.objects[&target].counter_count(CounterKind::MinusOneMinusOne),
            2
        );
    }

    #[test]
    fn doubled_wither_damage_reports_counter_total_overflow() {
        let (mut engine, source) = damage_doubler_engine(
            r#"(ability_id: "double_damage", presentation: Fallback,
                definition: DoubleDamage(subject: CreatureYouControl))"#,
        );
        grant_wither_for_damage_test(&mut engine, source);
        let target = move_fixture_source_to_battlefield(&mut engine, "plain_creature_fixture", 1);
        engine
            .state
            .objects
            .get_mut(&target)
            .unwrap()
            .set_counter(CounterKind::MinusOneMinusOne, u32::MAX);
        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let item = source_item(source, generation);
        let completed = engine
            .process_or_park_damage_batch(
                &item,
                vec![DamageSpec {
                    event: DamageEvent::noncombat(
                        source,
                        0,
                        "Damage Doubler Fixture",
                        DamageRecipient::Permanent(target),
                        1,
                    ),
                    source_has_deathtouch: false,
                    source_has_lifelink: false,
                }],
                &mut Vec::new(),
            )
            .unwrap()
            .expect("one controlled-creature doubler resolves immediately");
        assert!(matches!(
            engine.commit_completed_damage_batch(&completed, &mut Vec::new()),
            Err(EngineError::DamageNumericRange("wither counter total"))
        ));
        assert_eq!(
            engine.state.objects[&target].counter_count(CounterKind::MinusOneMinusOne),
            u32::MAX
        );
        assert_eq!(engine.state.objects[&target].damage, 0);
    }

    #[test]
    fn damage_overflow_during_choice_restores_the_entire_command_checkpoint() {
        let abilities = (0..32)
            .map(|index| {
                format!(
                    r#"(ability_id: "double_{index}", presentation: Fallback,
                    definition: DoubleDamage(subject: AnySource))"#
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let (mut engine, source) = damage_doubler_engine(&abilities);
        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let item = source_item(source, generation);
        assert!(engine
            .process_or_park_damage_batch(
                &item,
                vec![DamageSpec {
                    event: DamageEvent::noncombat(
                        source,
                        0,
                        "Damage Doubler Fixture",
                        DamageRecipient::Player(1),
                        1,
                    ),
                    source_has_deathtouch: false,
                    source_has_lifelink: false,
                }],
                &mut Vec::new(),
            )
            .unwrap()
            .is_none());

        for _ in 0..30 {
            let choice = engine
                .state
                .pending_resolution
                .as_ref()
                .expect("remaining doublers stay ordered")
                .presentation
                .candidates[0];
            engine
                .apply_command(
                    1,
                    &RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                            rv1::SubmitResolutionChoice {
                                chosen_object_ids: vec![choice],
                                ..Default::default()
                            },
                        )),
                    },
                )
                .unwrap();
        }
        let pending_before = engine.state.pending_resolution.clone();
        let replacement_before = engine.state.pending_replacement_event.clone();
        assert_eq!(
            pending_before
                .as_ref()
                .unwrap()
                .presentation
                .candidates
                .len(),
            2
        );
        let snapshot_before = engine.diagnostic_snapshot().unwrap();
        let choice = pending_before.as_ref().unwrap().presentation.candidates[0];
        assert!(matches!(
            engine.apply_command(
                1,
                &RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            chosen_object_ids: vec![choice],
                            ..Default::default()
                        },
                    )),
                }
            ),
            Err(EngineError::DamageNumericRange(
                "damage doubler multiplication"
            ))
        ));
        assert_eq!(engine.diagnostic_snapshot().unwrap(), snapshot_before);
        assert_eq!(
            format!("{:?}", engine.state.pending_resolution),
            format!("{:?}", pending_before)
        );
        assert_eq!(
            format!("{:?}", engine.state.pending_replacement_event),
            format!("{:?}", replacement_before)
        );
    }

    #[test]
    fn one_combat_doubler_routes_and_doubles_the_whole_simultaneous_batch() {
        let decks = Some(vec![vec!["grizzly_bears".into(); 7]; 2]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            614_502,
            &[0, 1],
            20,
            decks,
            true,
        )
        .unwrap();
        engine.state.turn_step = TurnStep::CombatDamage;
        engine.state.active_player_idx = 0;
        let first = move_bear_to_battlefield(&mut engine);
        let second = move_bear_to_battlefield(&mut engine);
        let assignment = |attacker| CombatAttackAssignment {
            attacker: TriggerObjectRef {
                object_id: attacker,
                zone_change_generation: 0,
                controller_at_event: 0,
            },
            defender: CombatDefenderTarget::Player(1),
            defending_player: 1,
        };
        let combat = CombatState {
            attacking: vec![first, second],
            attack_assignments: HashMap::from([
                (first, assignment(first)),
                (second, assignment(second)),
            ]),
            blockers: HashMap::new(),
            damage_assignments: HashMap::new(),
            trample_player_damage: HashMap::new(),
            damage_assignment_needed: false,
            attackers_declared: true,
            blockers_declared_by: vec![1],
            blockers_declared: true,
            assign_combat_damage_phase: false,
            first_strike_attackers: Vec::new(),
            first_strike_blockers: HashMap::new(),
            first_strike_damage_done: false,
        };
        engine.state.combat = Some(combat.clone());
        engine
            .state
            .damage_doubling_effects
            .push(ActiveDamageDoubling {
                id: 1,
                static_origin: None,
                source_id: None,
                source_label: "test global doubler".into(),
                source_presentation: Default::default(),
                scope: DamageDoublingScope::AnySource,
                duration: EffectDuration::UntilEndOfTurn,
            });
        let mut events = Vec::new();
        assert!(engine
            .try_park_ordered_combat_damage(
                &combat,
                super::super::combat::DamagePass::Normal,
                &mut events,
            )
            .unwrap());
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(
            engine.state.players[1].life, 12,
            "both 2-power attackers deal doubled damage in the shared simultaneous batch"
        );
    }

    #[test]
    fn outgoing_combat_prevention_uses_captured_generation_and_excludes_noncombat_and_copies() {
        let decks = Some(vec![
            vec!["grizzly_bears".into(); 7],
            vec!["island".into(); 7],
        ]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            305_513,
            &[0, 1],
            20,
            decks,
            true,
        )
        .unwrap();
        let source = move_bear_to_battlefield(&mut engine);
        let id = engine.add_damage_prevention(
            None,
            "Maze outgoing",
            DamagePreventionScope::CombatSource {
                object_id: source,
                zone_change_generation: 0,
            },
            DamagePreventionAmount::All,
        );
        let effect = engine
            .state
            .damage_prevention_effects
            .iter()
            .find(|effect| effect.id == id)
            .unwrap()
            .clone();
        let mut event =
            DamageEvent::combat(source, 0, "Grizzly Bears", DamageRecipient::Player(1), 2);
        engine.capture_combat_damage_source(&mut event);
        assert_eq!(
            event.source.zone_change_generation,
            Some(0),
            "physical generation zero is distinct from no object"
        );
        assert!(engine.prevention_effect_applies(&effect, &event));
        engine.state.objects.get_mut(&source).unwrap().controller = 1;
        engine.state.objects.get_mut(&source).unwrap().face_down = true;
        assert!(
            engine.prevention_effect_applies(&effect, &event),
            "controller/face do not change captured incarnation"
        );
        engine.state.zone_change_generation.insert(source, 1);
        assert!(
            engine.prevention_effect_applies(&effect, &event),
            "a retained damage snapshot is not rebound to current generation"
        );
        event.source.zone_change_generation = Some(1);
        assert!(!engine.prevention_effect_applies(&effect, &event));
        event.source.zone_change_generation = None;
        assert!(
            !engine.prevention_effect_applies(&effect, &event),
            "copy/no-physical-object does not match"
        );
        event.source.zone_change_generation = Some(0);
        event.classification = DamageClassification::Noncombat;
        assert!(!engine.prevention_effect_applies(&effect, &event));
        let pending = engine
            .process_or_park_damage_batch(
                &source_item(source, 0),
                vec![DamageSpec {
                    event,
                    source_has_deathtouch: false,
                    source_has_lifelink: false,
                }],
                &mut Vec::new(),
            )
            .unwrap()
            .expect("unprevented damage completes");
        assert_eq!(
            pending[0].result.dealt, 2,
            "shielded creature still deals noncombat damage"
        );
    }

    #[test]
    fn combat_departure_refresh_preserves_unchanged_choices_and_applied_prevention() {
        let mut engine = parked_departure_combat(1, false);
        let original = engine.state.pending_resolution.clone().unwrap();
        let before_id = engine.state.next_replacement_application_id;
        engine.refresh_damage_departure(&mut Vec::new()).unwrap();
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .candidates,
            original.presentation.candidates
        );
        assert_eq!(engine.state.next_replacement_application_id, before_id);
        let Some(super::super::replacement::PendingReplacementEvent::Damage(batch)) =
            engine.state.pending_replacement_event.as_ref()
        else {
            panic!("damage")
        };
        let DamageRecipient::Permanent(attacker) = batch.damage[0].spec.event.recipient else {
            panic!("attacker")
        };
        let fixed = engine.add_damage_prevention(
            None,
            "fixed shield",
            DamagePreventionScope::Recipient(attacker),
            DamagePreventionAmount::FixedPerEvent(1),
        );
        let finite = engine.add_damage_prevention(
            None,
            "finite shield",
            DamagePreventionScope::Recipient(attacker),
            DamagePreventionAmount::Remaining(3),
        );
        engine.refresh_damage_departure(&mut Vec::new()).unwrap();
        let Some(super::super::replacement::PendingReplacementEvent::Damage(batch)) =
            engine.state.pending_replacement_event.as_ref()
        else {
            panic!("damage")
        };
        let fixed_choice = batch
            .applications
            .iter()
            .find(|application| {
                application.application == DamageReplacementApplication::Effect(fixed)
            })
            .unwrap()
            .choice_id;
        let command = rv1::RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                rv1::SubmitResolutionChoice {
                    chosen_object_ids: vec![fixed_choice],
                    ..Default::default()
                },
            )),
        };
        engine.apply_command(0, &command).unwrap();
        let before_departure = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates
            .clone();
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            1
        );
        engine
            .apply_command(
                2,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
                },
            )
            .unwrap();
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .presentation
                .candidates,
            before_departure,
            "unrelated occurrence removal remaps indices without replacing valid opaque IDs"
        );
        let choice = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates[0];
        engine
            .apply_command(
                1,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            chosen_object_ids: vec![choice],
                            ..Default::default()
                        },
                    )),
                },
            )
            .unwrap();
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(
            engine
                .state
                .damage_prevention_effects
                .iter()
                .find(|effect| effect.id == finite)
                .unwrap()
                .amount,
            DamagePreventionAmount::Remaining(2),
            "fixed prevention applied once; the finite shield covers only the remaining one damage"
        );
    }

    fn parked_departure_combat(blocker_owner: usize, shield_attacker: bool) -> GameEngine {
        let decks = Some(vec![vec!["grizzly_bears".into(); 7]; 3]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            305_512,
            &[0, 1, 2],
            20,
            decks,
            true,
        )
        .unwrap();
        engine.state.opening = None;
        engine.state.turn_step = TurnStep::CombatDamage;
        let mut move_card = |owner: usize, controller: usize| {
            let oid = engine.state.players[owner].hand.remove(0);
            engine.state.players[controller].battlefield.push(oid);
            let object = engine.state.objects.get_mut(&oid).unwrap();
            object.zone = Zone::Battlefield;
            object.controller = controller as PlayerId;
            oid
        };
        let attacker = move_card(0, 0);
        let unrelated = move_card(0, 0);
        let blocker = move_card(blocker_owner, 1);
        let shielded = if shield_attacker { attacker } else { blocker };
        engine.state.add_damage_prevention_shield(shielded, 1);
        engine.state.add_damage_prevention_shield(shielded, 1);
        let unrelated_defender = if blocker_owner == 1 { 2 } else { 1 };
        let assignments = [(attacker, 1), (unrelated, unrelated_defender)]
            .into_iter()
            .map(|(oid, defender)| {
                (
                    oid,
                    CombatAttackAssignment {
                        attacker: TriggerObjectRef {
                            object_id: oid,
                            zone_change_generation: 0,
                            controller_at_event: 0,
                        },
                        defender: CombatDefenderTarget::Player(defender),
                        defending_player: defender,
                    },
                )
            })
            .collect();
        let combat = CombatState {
            attacking: vec![attacker, unrelated],
            attack_assignments: assignments,
            blockers: HashMap::from([(attacker, vec![blocker])]),
            damage_assignments: HashMap::new(),
            trample_player_damage: HashMap::new(),
            damage_assignment_needed: false,
            attackers_declared: true,
            blockers_declared_by: vec![1, unrelated_defender],
            blockers_declared: true,
            assign_combat_damage_phase: false,
            first_strike_attackers: vec![],
            first_strike_blockers: HashMap::new(),
            first_strike_damage_done: false,
        };
        engine.state.combat = Some(combat.clone());
        assert!(engine
            .try_park_ordered_combat_damage(
                &combat,
                super::super::combat::DamagePass::Normal,
                &mut Vec::new()
            )
            .unwrap());
        assert!(engine.state.pending_resolution.is_some());
        assert_eq!(engine.state.players[unrelated_defender as usize].life, 20);
        engine
    }

    #[test]
    fn combat_departure_refreshes_removed_candidates_with_surviving_anchor_and_chooser() {
        let mut engine = parked_departure_combat(1, true);
        engine.state.pending_resolution = None;
        engine.state.pending_replacement_event = None;
        let combat = engine.state.combat.as_mut().unwrap();
        let unrelated = combat.attacking[1];
        let assignment = combat.attack_assignments.get_mut(&unrelated).unwrap();
        assignment.defender = CombatDefenderTarget::Player(1);
        assignment.defending_player = 1;
        combat.blockers_declared_by = vec![1];
        let departing_blocker = engine.state.players[2].hand.remove(0);
        engine.state.players[1].battlefield.push(departing_blocker);
        let object = engine.state.objects.get_mut(&departing_blocker).unwrap();
        object.zone = Zone::Battlefield;
        object.controller = 1;
        combat.blockers.insert(unrelated, vec![departing_blocker]);
        let combat = combat.clone();
        engine.state.add_damage_prevention_shield(unrelated, 1);
        engine.state.add_damage_prevention_shield(unrelated, 1);
        assert!(engine
            .try_park_ordered_combat_damage(
                &combat,
                super::super::combat::DamagePass::Normal,
                &mut Vec::new()
            )
            .unwrap());
        let original = engine.state.pending_resolution.clone().unwrap();
        assert_eq!(original.deciding_player, 0);
        assert_eq!(original.presentation.candidates.len(), 4);
        engine
            .apply_command(
                2,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
                },
            )
            .unwrap();
        let refreshed = engine.state.pending_resolution.as_ref().unwrap();
        assert_eq!(refreshed.deciding_player, 0);
        assert_eq!(
            refreshed.presentation.source_object_id,
            original.presentation.source_object_id
        );
        assert_eq!(
            refreshed.presentation.candidates.len(),
            2,
            "removed simultaneous occurrences must disappear from the published prompt"
        );
        assert!(refreshed
            .presentation
            .candidates
            .iter()
            .all(|choice| !original.presentation.candidates.contains(choice)));
        let choice = refreshed.presentation.candidates[0];
        for stale in original.presentation.candidates {
            let before = engine.diagnostic_snapshot().unwrap();
            assert!(engine
                .apply_command(
                    0,
                    &rv1::RuledCommand {
                        cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                            rv1::SubmitResolutionChoice {
                                chosen_object_ids: vec![stale],
                                ..Default::default()
                            }
                        )),
                    }
                )
                .is_err());
            assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        }
        engine
            .apply_command(
                0,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            chosen_object_ids: vec![choice],
                            ..Default::default()
                        },
                    )),
                },
            )
            .unwrap();
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_replacement_event.is_none());
        assert_eq!(engine.state.objects[&unrelated].damage, 0);
    }

    #[test]
    fn combat_departed_chooser_refreshes_to_surviving_player_and_rejects_old_ids() {
        let mut engine = parked_departure_combat(1, false);
        engine.state.add_damage_prevention_shield(2, 1);
        engine.state.add_damage_prevention_shield(2, 1);
        engine.refresh_damage_departure(&mut Vec::new()).unwrap();
        let old = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates[0];
        engine
            .apply_command(
                1,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
                },
            )
            .unwrap();
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            2
        );
        assert_eq!(
            engine.state.players[2].life, 20,
            "surviving damage waits for the refreshed choice"
        );
        let before = engine.diagnostic_snapshot().unwrap();
        assert!(engine
            .apply_command(
                2,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            chosen_object_ids: vec![old],
                            ..Default::default()
                        }
                    ))
                }
            )
            .is_err());
        assert_eq!(engine.diagnostic_snapshot().unwrap(), before);
        let choice = engine
            .state
            .pending_resolution
            .as_ref()
            .unwrap()
            .presentation
            .candidates[0];
        engine
            .apply_command(
                2,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::SubmitResolutionChoice(
                        rv1::SubmitResolutionChoice {
                            chosen_object_ids: vec![choice],
                            ..Default::default()
                        },
                    )),
                },
            )
            .unwrap();
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(engine.state.players[2].life, 20);
    }

    #[test]
    fn defending_player_departure_removes_relation_even_when_stolen_blocker_survives() {
        let mut engine = parked_departure_combat(2, true);
        let combat = engine.state.combat.as_ref().unwrap();
        let attacker = combat.attacking[0];
        let blocker = combat.blockers[&attacker][0];
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(blocker),
            kind: ContinuousEffectKind::Layer2Control {
                controller: ControllerReference::Fixed(1),
            },
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 1,
        });
        engine
            .apply_command(
                1,
                &rv1::RuledCommand {
                    cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
                },
            )
            .unwrap();
        assert_eq!(engine.state.objects[&blocker].controller, 2);
        assert_eq!(
            engine.state.objects[&blocker].zone,
            Zone::Battlefield,
            "the surviving blocker is no longer in the departed defender's combat relation"
        );
        assert_eq!(engine.state.objects[&attacker].damage, 0);
        assert_eq!(engine.state.objects[&blocker].damage, 0);
        assert!(engine.state.pending_resolution.is_none());
    }

    #[test]
    fn combat_prevention_chooser_departure_settles_surviving_damage_once() {
        let mut engine = parked_departure_combat(1, false);
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            1
        );
        let command = rv1::RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
        };
        engine.apply_command(1, &command).unwrap();
        assert!(
            engine.state.pending_resolution.is_none(),
            "departed chooser cannot strand combat"
        );
        assert!(engine.state.pending_replacement_event.is_none());
        assert_eq!(
            engine.state.players[2].life, 18,
            "unrelated simultaneous damage remains owed"
        );
        engine.reconcile_departed_players(&mut Vec::new()).unwrap();
        assert_eq!(
            engine.state.players[2].life, 18,
            "departure refresh cannot commit damage twice"
        );
        assert_eq!(engine.state.turn_step, TurnStep::CombatDamage);
        assert_eq!(engine.state.priority_player_id(), 0);
    }

    #[test]
    fn combat_prevention_presentation_source_owner_departure_preserves_other_damage() {
        let mut engine = parked_departure_combat(2, true);
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            0
        );
        let command = rv1::RuledCommand {
            cmd: Some(rv1::ruled_command::Cmd::Concede(rv1::Concede {})),
        };
        engine.apply_command(2, &command).unwrap();
        assert_eq!(
            engine.state.players[1].life, 18,
            "combat is not an owned stack resolution that disappears with its presentation source"
        );
        assert!(engine.state.pending_resolution.is_none());
        assert!(engine.state.pending_replacement_event.is_none());
    }

    #[test]
    fn simultaneous_prevention_choices_follow_apnap_and_allow_own_event_order() {
        let decks = Some(vec![vec!["grizzly_bears".into(); 7]; 3]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            305_511,
            &[7, 13, 29],
            20,
            decks,
            true,
        )
        .unwrap();
        let mut move_card = |player_index: usize| {
            let oid = engine.state.players[player_index].hand.remove(0);
            engine.state.players[player_index].battlefield.push(oid);
            engine.state.objects.get_mut(&oid).unwrap().zone = Zone::Battlefield;
            oid
        };
        let first = move_card(0);
        let second = move_card(1);
        let third = move_card(0);
        let source = move_card(2);
        for recipient in [first, second, third] {
            engine.state.add_damage_prevention_shield(recipient, 1);
            engine.state.add_damage_prevention_shield(recipient, 1);
        }
        let damage = [first, second, third]
            .into_iter()
            .map(|recipient| DamageSpec {
                event: DamageEvent::noncombat(
                    source,
                    29,
                    "Grizzly Bears",
                    DamageRecipient::Permanent(recipient),
                    3,
                ),
                source_has_deathtouch: false,
                source_has_lifelink: false,
            })
            .collect();
        let mut item = source_item(source, 0);
        item.controller = 29;
        item.source_owner = Some(29);
        assert!(engine
            .process_or_park_damage_batch(&item, damage, &mut Vec::new())
            .unwrap()
            .is_none());
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            7
        );
        let Some(super::super::replacement::PendingReplacementEvent::Damage(batch)) =
            engine.state.pending_replacement_event.as_ref()
        else {
            panic!("parked damage batch");
        };
        assert_eq!(batch.applications.len(), 4, "active player can choose either of their simultaneous events before the defender decides");
        let chosen = batch
            .applications
            .iter()
            .find(|application| application.event_index == 2)
            .unwrap()
            .choice_id;
        let pending = engine.state.pending_resolution.take().unwrap();
        engine
            .finish_damage_replacement_choice(pending, chosen)
            .unwrap();
        assert_eq!(
            engine
                .state
                .pending_resolution
                .as_ref()
                .unwrap()
                .deciding_player,
            7,
            "all remaining active-player choices precede the defender's choice"
        );
        assert_eq!(
            engine.state.objects[&first].damage, 0,
            "no simultaneous damage commits during choices"
        );
        assert_eq!(engine.state.objects[&second].damage, 0);
        assert_eq!(engine.state.objects[&third].damage, 0);
    }

    fn source_item(source: ObjectId, generation: u64) -> StackItem {
        StackItem {
            mana_colors_spent_to_cast: Default::default(),
            id: source,
            controller: 0,
            card_id: "grizzly_bears".into(),
            targets: Vec::new(),
            ability_text: Some("test damage".into()),
            source_permanent_id: Some(source),
            source_owner: Some(0),
            source_zone_change: generation,
            source_face_change: 0,
            ability_index: None,
            activated_ability: None,
            triggered_ability: None,
            is_triggered: false,
            is_copy: false,
            face_index: 0,
            cast_method: SpellCastMethod::Normal,
            returned_attacker_assignment: None,
            chosen_x: 0,
            chosen_modes: Vec::new(),
            cast_condition_results: Vec::new(),
            cast_occurrence: None,
            cast_by: None,
            cast_cost_receipts: Vec::new(),
            payment_result: CardResultCohort::default(),
            search_results: Default::default(),
            exiled_cohorts: Default::default(),
            chaos_warp_owner_instructions: Default::default(),
            resolution_branch_choices: Default::default(),
            blight_receipts: Vec::new(),
            trigger_context: TriggerContext::default(),
        }
    }

    fn move_bear_to_battlefield(engine: &mut GameEngine) -> ObjectId {
        let object_id = engine.state.players[0].hand.remove(0);
        engine.state.players[0].battlefield.push(object_id);
        engine.state.objects.get_mut(&object_id).unwrap().zone = Zone::Battlefield;
        object_id
    }

    #[test]
    fn early_layer_static_prevention_suppression_preserves_resolved_shields() {
        let decks = Some(vec![
            vec!["anti-venom,_horrifying_healer".into(); 7],
            vec!["island".into(); 7],
        ]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            305_010,
            &[0, 1],
            20,
            decks,
            true,
        )
        .unwrap();
        let source = move_bear_to_battlefield(&mut engine);
        engine.emit_static_abilities_on_enter(source);
        let shield = engine.add_damage_prevention(
            None,
            "independent shield",
            DamagePreventionScope::Recipient(source),
            DamagePreventionAmount::All,
        );
        let event =
            DamageEvent::noncombat(source, 0, "test", DamageRecipient::Permanent(source), 1);
        assert_eq!(engine.damage_replacement_applications(&event).len(), 2);
        engine.state.continuous_effects.push(ContinuousEffect {
            trigger_grant_origin: None,
            source_id: None,
            affected: AffectedScope::Single(source),
            kind: ContinuousEffectKind::Layer4SetTypeLine(
                tricerules_card_model::TypeLineReplacement {
                    card_types: vec![PermanentTypeFilter::Land],
                    creature_types: Vec::new(),
                    land_types: vec![BasicLandType::Forest],
                },
            ),
            condition: None,
            duration: EffectDuration::UntilEndOfTurn,
            timestamp: 5,
        });
        engine.refresh_source_static_abilities(source);
        assert_eq!(
            engine.damage_replacement_applications(&event),
            vec![DamageReplacementApplication::Effect(shield)],
            "printed prevention is suppressed; independent resolving prevention remains"
        );
        engine
            .state
            .continuous_effects
            .retain(|effect| !matches!(effect.kind, ContinuousEffectKind::Layer4SetTypeLine(_)));
        assert_eq!(engine.damage_replacement_applications(&event).len(), 2);
        *engine
            .state
            .zone_change_generation
            .entry(source)
            .or_default() += 1;
        assert_eq!(
            engine.damage_replacement_applications(&event),
            vec![DamageReplacementApplication::Effect(shield)],
            "a retained static record cannot bind to a later incarnation"
        );
    }

    #[test]
    fn issue_464_damage_pipeline_records_only_positive_source_incarnations() {
        let decks = Some(vec![
            vec!["grizzly_bears".into(); 7],
            vec!["island".into(); 7],
        ]);
        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            464_003,
            &[0, 1],
            20,
            decks,
            true,
        )
        .expect("engine");
        let source = move_bear_to_battlefield(&mut engine);
        let generation = engine
            .state
            .zone_change_generation
            .get(&source)
            .copied()
            .unwrap_or(0);
        let completed = engine
            .process_or_park_damage_batch(
                &source_item(source, generation),
                vec![DamageSpec {
                    event: DamageEvent::noncombat(
                        source,
                        0,
                        "Grizzly Bears",
                        DamageRecipient::Player(1),
                        1,
                    ),
                    source_has_deathtouch: false,
                    source_has_lifelink: false,
                }],
                &mut Vec::new(),
            )
            .expect("damage batch processes")
            .expect("unprevented damage completes immediately");
        assert_eq!(completed[0].result.dealt, 1);
        engine
            .commit_completed_damage_batch(&completed, &mut Vec::new())
            .unwrap();
        assert_eq!(
            engine.state.turn_history.current.dealt_damage_objects,
            vec![(source, generation)]
        );

        let prevented_source = move_bear_to_battlefield(&mut engine);
        engine.add_damage_prevention(
            None,
            "test prevention",
            DamagePreventionScope::Recipient(1),
            DamagePreventionAmount::All,
        );
        let prevented_generation = engine
            .state
            .zone_change_generation
            .get(&prevented_source)
            .copied()
            .unwrap_or(0);
        let prevented = engine
            .process_or_park_damage_batch(
                &source_item(prevented_source, prevented_generation),
                vec![DamageSpec {
                    event: DamageEvent::noncombat(
                        prevented_source,
                        0,
                        "Grizzly Bears",
                        DamageRecipient::Player(1),
                        1,
                    ),
                    source_has_deathtouch: false,
                    source_has_lifelink: false,
                }],
                &mut Vec::new(),
            )
            .expect("damage batch processes")
            .expect("fully prevented damage completes immediately");
        assert_eq!(prevented[0].result.dealt, 0);
        engine
            .commit_completed_damage_batch(&prevented, &mut Vec::new())
            .unwrap();
        assert_eq!(
            engine.state.turn_history.current.dealt_damage_objects,
            vec![(source, generation)],
            "damage prevented to zero does not create a source fact"
        );
    }
}
