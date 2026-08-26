//! Shared CR 120/615 damage-event preprocessing.
//!
//! Damage producers construct events here before mutating life, marked damage, deathtouch
//! history, lifelink totals, or damage-trigger state. This keeps prevention and prohibitions out
//! of incidental spell/combat iteration order and gives later CR 616 choices one event shape.

use super::events::{ev_log, finish_with_events, object_display_name};
use super::targeting::TargetSourceIdentity;
use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DamageSourceSnapshot {
    pub object_id: ObjectId,
    pub controller: PlayerId,
    pub label: String,
    pub colors: Vec<Color>,
    pub types: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DamageClassification {
    Combat,
    Noncombat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
                object_id: source_id,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DamageResult {
    pub attempted: u32,
    pub dealt: u32,
    pub prevented: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct DamageSpec {
    pub event: DamageEvent,
    pub source_has_deathtouch: bool,
    pub source_has_lifelink: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct DamageApplicationChoice {
    pub choice_id: u32,
    pub application: DamagePreventionApplication,
    pub event_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DamagePreventionApplication {
    Effect(u32),
    Protection(ProtectionQuality),
}

#[derive(Debug, Clone)]
pub(crate) struct PendingDamageBatch {
    pub damage: Vec<PendingDamageEvent>,
    pub applications: Vec<DamageApplicationChoice>,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingDamageEvent {
    pub spec: DamageSpec,
    pub remaining: u32,
    pub applied_applications: Vec<DamagePreventionApplication>,
}

#[derive(Debug, Clone)]
pub(crate) struct CompletedDamage {
    pub spec: DamageSpec,
    pub result: DamageResult,
}

enum DamageBatchProgress {
    Complete(Vec<CompletedDamage>),
    NeedsChoice {
        batch: PendingDamageBatch,
        raw_candidates: Vec<(usize, DamagePreventionApplication, String)>,
    },
}

impl GameEngine {
    fn damage_batch_needs_ordering(&self, damage: &[DamageSpec]) -> bool {
        let by_event: Vec<Vec<DamagePreventionApplication>> = damage
            .iter()
            .map(|spec| self.prevention_applications(&spec.event))
            .collect();
        if by_event.iter().any(|candidates| candidates.len() > 1) {
            return true;
        }
        self.state.damage_prevention_effects.iter().any(|effect| {
            matches!(effect.amount, DamagePreventionAmount::Remaining(_))
                && by_event
                    .iter()
                    .filter(|candidates| {
                        candidates.contains(&DamagePreventionApplication::Effect(effect.id))
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

    fn prevention_applications(&self, event: &DamageEvent) -> Vec<DamagePreventionApplication> {
        self.state
            .damage_prevention_effects
            .iter()
            .filter(|effect| self.prevention_effect_applies(effect, event))
            .map(|effect| DamagePreventionApplication::Effect(effect.id))
            .chain(
                self.protection_applications(event)
                    .into_iter()
                    .map(DamagePreventionApplication::Protection),
            )
            .collect()
    }

    fn prevention_effect_applies(
        &self,
        effect: &ActiveDamagePrevention,
        event: &DamageEvent,
    ) -> bool {
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

    /// Process immediately when zero or one prevention effect applies; otherwise park the damage
    /// event and reuse the generic resolution-choice protocol for CR 616 ordering.
    pub(crate) fn process_or_park_damage_event(
        &mut self,
        item: &StackItem,
        event: DamageEvent,
        source_has_deathtouch: bool,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Option<DamageResult> {
        self.process_or_park_damage_batch(
            item,
            vec![DamageSpec {
                event,
                source_has_deathtouch,
                source_has_lifelink: false,
            }],
            events,
        )
        .and_then(|mut completed| completed.pop().map(|damage| damage.result))
    }

    pub(crate) fn process_or_park_damage_batch(
        &mut self,
        item: &StackItem,
        mut damage: Vec<DamageSpec>,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Option<Vec<CompletedDamage>> {
        for spec in &mut damage {
            let item_source = item.source_permanent_id.unwrap_or(item.id);
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
                    remaining: spec.event.amount,
                    spec,
                    applied_applications: Vec::new(),
                })
                .collect(),
            applications: Vec::new(),
        };
        match self.advance_damage_batch(pending, events) {
            DamageBatchProgress::Complete(completed) => Some(completed),
            DamageBatchProgress::NeedsChoice {
                batch,
                raw_candidates,
            } => {
                self.park_damage_prevention_choice(
                    item.clone(),
                    None,
                    batch,
                    raw_candidates,
                    events,
                );
                None
            }
        }
    }

    fn pending_prevention_candidates(
        &self,
        batch: &PendingDamageBatch,
    ) -> Vec<Vec<(DamagePreventionApplication, String)>> {
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
                            .contains(&DamagePreventionApplication::Effect(effect.id))
                            && self.prevention_effect_applies(effect, &damage.spec.event)
                    })
                    .map(|effect| {
                        (
                            DamagePreventionApplication::Effect(effect.id),
                            effect.source_label.clone(),
                        )
                    });
                let protection = self
                    .protection_applications(&damage.spec.event)
                    .into_iter()
                    .filter(|quality| {
                        !damage
                            .applied_applications
                            .contains(&DamagePreventionApplication::Protection(*quality))
                    })
                    .map(|quality| {
                        (
                            DamagePreventionApplication::Protection(quality),
                            quality.label(),
                        )
                    });
                effects.chain(protection).collect()
            })
            .collect()
    }

    fn next_prevention_ordering_choice(
        &self,
        by_event: &[Vec<(DamagePreventionApplication, String)>],
    ) -> Vec<(usize, DamagePreventionApplication, String)> {
        if let Some((event_index, candidates)) = by_event
            .iter()
            .enumerate()
            .find(|(_, candidates)| candidates.len() > 1)
        {
            return candidates
                .iter()
                .map(|(application, label)| (event_index, *application, label.clone()))
                .collect();
        }

        for effect in &self.state.damage_prevention_effects {
            if !matches!(effect.amount, DamagePreventionAmount::Remaining(_)) {
                continue;
            }
            let occurrences: Vec<_> = by_event
                .iter()
                .enumerate()
                .filter(|(_, candidates)| {
                    candidates.iter().any(|(application, _)| {
                        *application == DamagePreventionApplication::Effect(effect.id)
                    })
                })
                .map(|(event_index, _)| {
                    (
                        event_index,
                        DamagePreventionApplication::Effect(effect.id),
                        effect.source_label.clone(),
                    )
                })
                .collect();
            if occurrences.len() > 1 {
                return occurrences;
            }
        }
        Vec::new()
    }

    fn advance_damage_batch(
        &mut self,
        mut batch: PendingDamageBatch,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> DamageBatchProgress {
        loop {
            let by_event = self.pending_prevention_candidates(&batch);
            let raw_candidates = self.next_prevention_ordering_choice(&by_event);
            if !raw_candidates.is_empty() {
                return DamageBatchProgress::NeedsChoice {
                    batch,
                    raw_candidates,
                };
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
                return DamageBatchProgress::Complete(
                    batch
                        .damage
                        .into_iter()
                        .map(|damage| CompletedDamage {
                            result: DamageResult {
                                attempted: damage.spec.event.amount,
                                dealt: damage.remaining,
                                prevented: damage.spec.event.amount - damage.remaining,
                            },
                            spec: damage.spec,
                        })
                        .collect(),
                );
            };
            let applied =
                self.apply_prevention_application(&mut batch, event_index, application, events);
            debug_assert!(applied);
        }
    }

    fn apply_prevention_application(
        &mut self,
        batch: &mut PendingDamageBatch,
        event_index: usize,
        application: DamagePreventionApplication,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> bool {
        let Some(damage) = batch.damage.get(event_index) else {
            return false;
        };
        if damage.remaining == 0 || damage.applied_applications.contains(&application) {
            return false;
        }
        let event = damage.spec.event.clone();
        if let DamagePreventionApplication::Protection(quality) = application {
            if !self.protection_applications(&event).contains(&quality) {
                return false;
            }
            let prevented = if self.state.damage_prevention_prohibitions.is_empty() {
                damage.remaining
            } else {
                0
            };
            let damage = &mut batch.damage[event_index];
            damage.remaining -= prevented;
            damage.applied_applications.push(application);
            if prevented > 0 {
                events.push(ev_log(format!(
                    "{} prevents {prevented} damage from {}.",
                    quality.label(),
                    event.source.label
                )));
            }
            return true;
        }
        let DamagePreventionApplication::Effect(effect_id) = application else {
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
            return false;
        };
        let unpreventable = !self.state.damage_prevention_prohibitions.is_empty();
        let application_attempted = damage.remaining;
        let effect = &mut self.state.damage_prevention_effects[effect_index];
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
        if let DamagePreventionAmount::Remaining(remaining) = &mut effect.amount {
            *remaining -= prevented;
        }
        let source_label = effect.source_label.clone();
        let additional_effect = effect.additional_effect;
        let exhausted = matches!(effect.amount, DamagePreventionAmount::Remaining(0));

        let damage = &mut batch.damage[event_index];
        damage.remaining -= prevented;
        damage.applied_applications.push(application);
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
                    let timestamp = self.state.command_index;
                    if let Some(object) = self.state.objects.get_mut(&recipient) {
                        object.add_counters(counter, amount, timestamp);
                    }
                    events.push(ev_log(format!(
                        "{source_label} puts {amount} {} counter(s) on {recipient_label}.",
                        counter.label()
                    )));
                }
            }
        }
        if exhausted {
            self.state
                .damage_prevention_effects
                .retain(|effect| effect.id != effect_id);
        }
        true
    }

    fn park_damage_prevention_choice(
        &mut self,
        item: StackItem,
        resume_effect_index: Option<u32>,
        mut batch: PendingDamageBatch,
        raw_candidates: Vec<(usize, DamagePreventionApplication, String)>,
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let deciding_event = &batch.damage[raw_candidates[0].0].spec.event;
        let deciding_player = match deciding_event.recipient {
            DamageRecipient::Player(player) => player,
            DamageRecipient::Permanent(permanent) => self
                .state
                .objects
                .get(&permanent)
                .map(|object| object.controller)
                .unwrap_or(item.controller),
        };
        let mut applications = Vec::new();
        let mut candidates = Vec::new();
        let mut candidate_names = Vec::new();
        let mut candidate_effect_ids = Vec::new();
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
                DamagePreventionApplication::Effect(effect_id) => effect_id,
                DamagePreventionApplication::Protection(_) => 0,
            });
            let damage = &batch.damage[event_index];
            candidate_names.push(format!(
                "{effect_label} — {} damage from {}",
                damage.remaining, damage.spec.event.source.label
            ));
        }
        let prompt = "Choose the next damage-prevention effect to apply.".to_string();
        events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::ResolutionChoiceRequired(
                rv1::ResolutionChoiceRequired {
                    deciding_player_id: deciding_player,
                    source_object_id: item.id,
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
                    reveal_audience: 0,
                    revealed_zone_owner_player_id: None,
                    candidate_source_zones: Vec::new(),
                    combat_defender_options: Vec::new(),
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
                source_object_id: item.id,
                candidates,
                min: 1,
                max: 1,
                ordered: false,
                prompt,
                choice_kind: rv1::ChoiceKind::ReplacementEffect,
                unique_names: false,
            },
            continuation: ResolutionContinuation::DamageReplacement {
                stack: ParkedStackResolution {
                    item,
                    resume_effect_index,
                    previous_result: CardResultCohort::default(),
                },
                effect_ids: candidate_effect_ids,
            },
        });
    }

    pub(crate) fn process_or_park_combat_damage(
        &mut self,
        event: DamageEvent,
        source_has_deathtouch: bool,
        source_has_lifelink: bool,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> Option<DamageResult> {
        let source_id = event.source.object_id;
        let controller = event.source.controller;
        let card_id = self
            .state
            .objects
            .get(&source_id)
            .map(|object| object.card_id.clone())
            .unwrap_or_default();
        let item = StackItem {
            id: source_id,
            controller,
            card_id,
            targets: Vec::new(),
            ability_text: Some("combat damage".to_string()),
            source_permanent_id: Some(source_id),
            source_zone_change: self
                .state
                .zone_change_generation
                .get(&source_id)
                .copied()
                .unwrap_or(0),
            source_face_change: self
                .state
                .face_change_generation
                .get(&source_id)
                .copied()
                .unwrap_or(0),
            ability_index: None,
            activated_ability: None,
            triggered_ability: None,
            is_triggered: false,
            is_copy: false,
            face_index: 0,
            cast_method: SpellCastMethod::Normal,
            chosen_x: 0,
            chosen_modes: Vec::new(),
            cast_cost_receipts: Vec::new(),
            payment_result: CardResultCohort::default(),
            resolution_branch_choices: Default::default(),
            trigger_context: TriggerContext::default(),
        };
        let result = self.process_or_park_damage_event(&item, event, source_has_deathtouch, events);
        if result.is_none() {
            if let Some(super::replacement::PendingReplacementEvent::Damage(batch)) =
                self.state.pending_replacement_event.as_mut()
            {
                if let Some(damage) = batch.damage.first_mut() {
                    damage.spec.source_has_lifelink = source_has_lifelink;
                }
            }
        }
        result
    }

    /// Preflight a simultaneous combat-damage batch. The legacy commit loop remains the fast path
    /// when no ordering decision exists; when CR 616 or finite-shield allocation creates a real
    /// choice, the whole batch is parked before any damage is committed.
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
                let source_characteristics = engine.characteristics(source);
                let mut event = DamageEvent::combat(
                    source,
                    controller,
                    object_display_name(&engine.state, engine.registry, source),
                    recipient,
                    amount,
                );
                if let Some(characteristics) = source_characteristics {
                    event.source.colors = characteristics.colors;
                    event.source.types = characteristics.types;
                }
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

        if !self.damage_batch_needs_ordering(&damage) {
            return Ok(false);
        }
        let first = damage
            .first()
            .ok_or(EngineError::Illegal("empty ordered combat-damage batch"))?;
        let item = StackItem {
            id: first.event.source.object_id,
            controller: first.event.source.controller,
            card_id: self
                .state
                .objects
                .get(&first.event.source.object_id)
                .map(|object| object.card_id.clone())
                .unwrap_or_default(),
            targets: Vec::new(),
            ability_text: Some("combat damage".to_string()),
            source_permanent_id: Some(first.event.source.object_id),
            source_zone_change: 0,
            source_face_change: 0,
            ability_index: None,
            activated_ability: None,
            triggered_ability: None,
            is_triggered: false,
            is_copy: false,
            face_index: 0,
            cast_method: SpellCastMethod::Normal,
            chosen_x: 0,
            chosen_modes: Vec::new(),
            cast_cost_receipts: Vec::new(),
            payment_result: CardResultCohort::default(),
            resolution_branch_choices: Default::default(),
            trigger_context: TriggerContext::default(),
        };
        let completed = self.process_or_park_damage_batch(&item, damage, events);
        debug_assert!(completed.is_none());
        if completed.is_some() {
            return Err(EngineError::Illegal(
                "ordered combat damage unexpectedly completed without a choice",
            ));
        }
        Ok(true)
    }

    pub(crate) fn finish_damage_prevention_choice(
        &mut self,
        pending: PendingResolution,
        chosen_application_id: u32,
    ) -> Result<RuledEventBatch, EngineError> {
        let stack = match &pending.continuation {
            ResolutionContinuation::DamageReplacement { stack, .. } => stack.clone(),
            _ => {
                return Err(EngineError::Illegal(
                    "damage-replacement continuation missing",
                ))
            }
        };
        let Some(pending_event) = self.state.pending_replacement_event.take() else {
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal("damage-prevention choice is stale"));
        };
        let mut batch = match pending_event {
            super::replacement::PendingReplacementEvent::Damage(batch) => batch,
            other => {
                self.state.pending_replacement_event = Some(other);
                self.state.pending_resolution = Some(pending);
                return Err(EngineError::Illegal("damage-prevention choice is stale"));
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
                "damage-prevention application is stale",
            ));
        };
        let mut events = Vec::new();
        if !self.apply_prevention_application(
            &mut batch,
            application.event_index,
            application.application,
            &mut events,
        ) {
            self.state.pending_replacement_event =
                Some(super::replacement::PendingReplacementEvent::Damage(batch));
            self.state.pending_resolution = Some(pending);
            return Err(EngineError::Illegal(
                "damage-prevention effect is no longer active",
            ));
        }
        let completed = match self.advance_damage_batch(batch, &mut events) {
            DamageBatchProgress::Complete(completed) => completed,
            DamageBatchProgress::NeedsChoice {
                batch,
                raw_candidates,
            } => {
                self.park_damage_prevention_choice(
                    stack.item,
                    stack.resume_effect_index,
                    batch,
                    raw_candidates,
                    &mut events,
                );
                return Ok(finish_with_events(self, events));
            }
        };
        self.commit_completed_damage_batch(&completed, &mut events);
        self.complete_parked_resolution(stack.item, stack.resume_effect_index, events)
    }

    pub(crate) fn commit_damage_result(
        &mut self,
        event: &DamageEvent,
        result: DamageResult,
        source_has_deathtouch: bool,
        events: &mut Vec<rv1::RuledEvent>,
    ) -> u32 {
        match event.recipient {
            DamageRecipient::Player(player) => {
                let Some(index) = self.state.player_idx(player) else {
                    return 0;
                };
                self.state.players[index].life -= result.dealt as i32;
                events.push(rv1::RuledEvent {
                    ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                        player_id: player,
                        new_total: self.state.players[index].life,
                        delta: -(result.dealt as i32),
                    })),
                });
                if result.dealt > 0 {
                    events.push(ev_log(format!(
                        "{} deals {} damage to P{player}",
                        event.source.label, result.dealt
                    )));
                }
                result.dealt
            }
            DamageRecipient::Permanent(permanent) => {
                let label = object_display_name(&self.state, self.registry, permanent);
                let Some(characteristics) = self.characteristics(permanent) else {
                    return 0;
                };
                let is_creature = characteristics.is_creature();
                let is_planeswalker = characteristics.has_type("Planeswalker");
                let is_battle = characteristics.has_type("Battle");
                let was_defended = self
                    .state
                    .objects
                    .get(&permanent)
                    .is_some_and(|object| object.counter_count(CounterKind::Defense) > 0);
                let Some(object) = self.state.objects.get_mut(&permanent) else {
                    return 0;
                };
                if object.zone != Zone::Battlefield
                    || !(is_creature || is_planeswalker || is_battle)
                {
                    return 0;
                }
                if is_creature {
                    object.damage = object.damage.saturating_add(result.dealt);
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
                if defeated_siege {
                    self.stage_siege_defeat_trigger(permanent);
                }
                result.dealt
            }
        }
    }

    pub(crate) fn commit_completed_damage_batch(
        &mut self,
        completed: &[CompletedDamage],
        events: &mut Vec<rv1::RuledEvent>,
    ) {
        let mut lifelink_by_source: BTreeMap<(ObjectId, PlayerId), u32> = BTreeMap::new();
        let mut trigger_events = Vec::new();
        for damage in completed {
            let dealt = self.commit_damage_result(
                &damage.spec.event,
                damage.result,
                damage.spec.source_has_deathtouch,
                events,
            );
            if dealt == 0 {
                continue;
            }
            if damage.spec.source_has_lifelink {
                *lifelink_by_source
                    .entry((
                        damage.spec.event.source.object_id,
                        damage.spec.event.source.controller,
                    ))
                    .or_insert(0) += dealt;
            }
            let mut event = damage.spec.event.clone();
            event.amount = dealt;
            trigger_events.push(GameEvent::DamageDealt { event });
        }
        for ((_, controller), dealt) in lifelink_by_source {
            if let Some(event) = super::resolution::life::apply_life_gain_without_triggers(
                self, events, controller, dealt, "lifelink",
            ) {
                trigger_events.push(event);
            }
        }
        self.fire_triggers(&trigger_events);
    }

    pub(crate) fn add_damage_prevention(
        &mut self,
        source_id: Option<ObjectId>,
        source_label: impl Into<String>,
        scope: DamagePreventionScope,
        amount: DamagePreventionAmount,
    ) -> u32 {
        let id = self.state.next_damage_prevention_effect_id;
        self.state.next_damage_prevention_effect_id = id.saturating_add(1);
        self.state
            .damage_prevention_effects
            .push(ActiveDamagePrevention {
                id,
                source_id,
                source_label: source_label.into(),
                scope,
                amount,
                duration: EffectDuration::UntilEndOfTurn,
                additional_effect: None,
            });
        id
    }
}
