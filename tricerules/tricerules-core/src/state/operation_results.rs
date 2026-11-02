//! Private typed facts at the primitive execution boundary.
//!
//! An explicit Program input retains its original value and admitted type. Completed operations
//! publish tagged values; an empty bundle is not proof that an operation completed with no objects.
use super::*;
use crate::engine::EngineError;

/// Read-only facts shared by amount/condition evaluation and primitive input readers.
/// Identity/card views never establish completion or producer provenance; those require `value`.
pub(crate) trait ResolutionFacts: std::fmt::Debug {
    fn value(&self, predicate: fn(&ProgramValue) -> bool) -> Option<&ProgramValue>;

    fn admitted_type(&self) -> Option<&ResultType> {
        None
    }

    fn card_entries(&self) -> &[CardResultEntry] {
        match self.value(|value| matches!(value, ProgramValue::CardResultCohort(_))) {
            Some(ProgramValue::CardResultCohort(cohort)) => &cohort.cards,
            _ => &[],
        }
    }

    fn discard_result(&self) -> Option<&ProgramDiscardResult> {
        match self.value(|value| matches!(value, ProgramValue::DiscardResult(_))) {
            Some(ProgramValue::DiscardResult(result)) => Some(result),
            _ => None,
        }
    }

    fn token_copy_batch(&self) -> Option<&[TriggerObjectRef]> {
        match self.value(|value| matches!(value, ProgramValue::TokenCopyBatch(_))) {
            Some(ProgramValue::TokenCopyBatch(objects)) => Some(objects),
            _ => None,
        }
    }

    fn hand_cards(&self) -> &[TriggerObjectRef] {
        match self.value(|value| matches!(value, ProgramValue::HandSelection(_))) {
            Some(ProgramValue::HandSelection(objects)) => objects,
            _ => &[],
        }
    }

    fn object_value(&self) -> Option<&ProgramValue> {
        self.value(|value| {
            matches!(
                value,
                ProgramValue::DrawReceipt(_)
                    | ProgramValue::PermanentSelection(_)
                    | ProgramValue::ManifestedPermanent(_)
                    | ProgramValue::CounteredAbilitySource(_)
                    | ProgramValue::ExiledObject(_)
                    | ProgramValue::TriggerObjectLinkedExileReceipt(_)
                    | ProgramValue::TokenCopyBatch(_)
                    | ProgramValue::CreatedTokenBatch(_)
                    | ProgramValue::ProliferatedPermanentSelection(_)
            )
        })
    }

    fn object_refs(&self) -> &[TriggerObjectRef] {
        match self.object_value() {
            Some(
                ProgramValue::DrawReceipt(objects)
                | ProgramValue::PermanentSelection(objects)
                | ProgramValue::TokenCopyBatch(objects)
                | ProgramValue::CreatedTokenBatch(objects)
                | ProgramValue::ProliferatedPermanentSelection(objects),
            ) => objects,
            Some(
                ProgramValue::ManifestedPermanent(Some(object))
                | ProgramValue::CounteredAbilitySource(Some(object))
                | ProgramValue::ExiledObject(Some(object))
                | ProgramValue::TriggerObjectLinkedExileReceipt(Some(object)),
            ) => std::slice::from_ref(object),
            _ => &[],
        }
    }

    fn counter_placements(&self) -> &[CounterPlacementReceipt] {
        &[]
    }
    fn was_sacrificed(&self, _object_id: ObjectId, _generation: u64, _actor: PlayerId) -> bool {
        false
    }
}

/// One explicitly bound Program slot, never projected into an untyped previous-result carrier.
#[derive(Debug)]
pub(crate) struct OperationInput {
    pub(crate) result_type: ResultType,
    pub(crate) value: ProgramValue,
}

impl ResolutionFacts for OperationInput {
    fn value(&self, predicate: fn(&ProgramValue) -> bool) -> Option<&ProgramValue> {
        predicate(&self.value).then_some(&self.value)
    }
    fn admitted_type(&self) -> Option<&ResultType> {
        Some(&self.result_type)
    }
    fn counter_placements(&self) -> &[CounterPlacementReceipt] {
        match &self.value {
            ProgramValue::CounterPlacementReceipt(Some(receipt)) => std::slice::from_ref(receipt),
            _ => &[],
        }
    }
    fn was_sacrificed(&self, object_id: ObjectId, generation: u64, actor: PlayerId) -> bool {
        self.card_entries().iter().any(|entry| {
            entry.action == CardResultAction::Sacrifice
                && entry.affected_player == actor
                && entry.object_id == object_id
                && matches!(entry.provenance, CardResultProvenance::CommittedMovement {
                    input_zone: Zone::Battlefield, input_generation, ..
                } if input_generation == generation)
        })
    }
}

#[derive(serde::Serialize, Debug, Clone)]
enum OperationValue {
    Program(ProgramValue),
    CounterPlacements(Vec<CounterPlacementReceipt>),
    SacrificedObjects(Vec<TriggerObjectRef>),
    ExiledObjects(Vec<TriggerObjectRef>),
    Count {
        kind: tricerules_card_model::primitives::CountResultKind,
        value: u32,
    },
}

/// Multiple independently tagged results of one operation, including genuine dual-output leaves.
#[derive(serde::Serialize, Debug, Clone, Default)]
pub(crate) struct OperationResults {
    values: Vec<OperationValue>,
}

impl OperationResults {
    /// Start the actual primitive's empty facts. Publication into Program slots still occurs
    /// only after that primitive (and any parked domain transaction) has completed.
    pub(crate) fn begin_effect(
        &mut self,
        effect: &tricerules_card_model::primitives::SpellEffectKind,
    ) -> Result<(), EngineError> {
        use tricerules_card_model::primitives::{SpellEffectKind as E, TokenCopySource};
        match effect {
            E::Draw { .. } | E::TargetPlayerDraws { .. } => {
                self.set(ProgramValue::DrawReceipt(Vec::new()))
            }
            E::CounterTargetSpell {
                unless_controller_pays,
                ..
            } => {
                self.set(ProgramValue::TargetedSpellController(None));
                if unless_controller_pays.is_some() {
                    self.set(ProgramValue::CounterPaymentOutcome(
                        CounterPaymentOutcome::NotOffered,
                    ));
                }
            }
            E::CounterTargetAbility => self.set(ProgramValue::CounteredAbilitySource(None)),
            E::ChoosePermanents { .. } => self.set(ProgramValue::PermanentSelection(Vec::new())),
            E::SelectHandCards { .. } => self.set(ProgramValue::HandSelection(Vec::new())),
            E::ManifestDread => self.set(ProgramValue::ManifestedPermanent(None)),
            E::PutCounters { .. } | E::Proliferate => self.record_counter_placements([]),
            E::CreateTokens { .. } => self.set(ProgramValue::CreatedTokenBatch(Vec::new())),
            E::CreateTokenCopies {
                source: TokenCopySource::TriggerObject | TokenCopySource::LinkedExile { .. },
                ..
            } => self.set(ProgramValue::TokenCopyBatch(Vec::new())),
            E::CreateTokenCopies { .. } => self.set(ProgramValue::CreatedTokenBatch(Vec::new())),
            E::Exile { .. } => {
                self.record_exiled_objects([]);
                self.record_cards([])?;
            }
            E::Discard { .. }
            | E::ChooseHandCards { .. }
            | E::ChooseLinkedExiledCard { .. }
            | E::Sacrifice { .. }
            | E::MoveGraveyardCards { .. }
            | E::Destroy { .. }
            | E::DestroyPreventingRegeneration { .. } => self.record_cards([])?,
            E::SearchLibrary {
                destination:
                    tricerules_card_model::primitives::SearchDestination::Battlefield { .. },
                ..
            } => self.set(ProgramValue::PermanentSelection(Vec::new())),
            E::SearchLibrary {
                destination: tricerules_card_model::primitives::SearchDestination::KeepInLibrary,
                ..
            } => self.set(ProgramValue::FoundLibraryCohort(Vec::new())),
            _ => {}
        }
        Ok(())
    }
    pub(crate) fn from_value(value: ProgramValue) -> Self {
        if let ProgramValue::CardResultCohort(cohort) = value {
            return Self::from(cohort);
        }
        Self {
            values: vec![OperationValue::Program(value)],
        }
    }

    pub(crate) fn publish(&mut self, value: ProgramValue) -> Result<(), EngineError> {
        if let ProgramValue::CardResultCohort(cohort) = &value {
            if cohort.program_discard_result.is_some() {
                return Err(EngineError::ProgramInvariant(
                    "card-action facts contain an implicit discard result",
                ));
            }
        }
        let tag = std::mem::discriminant(&value);
        if self.values.iter().any(|existing| {
            matches!(existing,
            OperationValue::Program(existing) if std::mem::discriminant(existing) == tag)
        }) {
            return Err(EngineError::ProgramInvariant(
                "operation published a duplicate result tag",
            ));
        }
        self.values.push(OperationValue::Program(value));
        Ok(())
    }

    pub(crate) fn set(&mut self, value: ProgramValue) {
        let normalized = Self::from_value(value);
        for value in normalized.values {
            let OperationValue::Program(value) = value else {
                unreachable!()
            };
            let tag = std::mem::discriminant(&value);
            if let Some(OperationValue::Program(existing)) = self.values.iter_mut().find(|existing|
                matches!(existing, OperationValue::Program(existing) if std::mem::discriminant(existing) == tag)) {
                *existing = value;
            } else { self.values.push(OperationValue::Program(value)); }
        }
    }

    pub(crate) fn with_counter_placements(
        mut self,
        receipts: impl IntoIterator<Item = CounterPlacementReceipt>,
    ) -> Self {
        self.record_counter_placements(receipts);
        self
    }

    #[cfg(test)]
    pub(crate) fn from_exiled_objects(objects: Vec<TriggerObjectRef>) -> Self {
        Self {
            values: vec![OperationValue::ExiledObjects(objects)],
        }
    }

    pub(crate) fn exiled_objects(&self) -> Option<&[TriggerObjectRef]> {
        self.values.iter().find_map(|value| match value {
            OperationValue::ExiledObjects(objects) => Some(objects.as_slice()),
            _ => None,
        })
    }

    pub(crate) fn record_exiled_objects(
        &mut self,
        objects: impl IntoIterator<Item = TriggerObjectRef>,
    ) {
        if let Some(OperationValue::ExiledObjects(existing)) = self
            .values
            .iter_mut()
            .find(|value| matches!(value, OperationValue::ExiledObjects(_)))
        {
            existing.extend(objects);
        } else {
            self.values
                .push(OperationValue::ExiledObjects(objects.into_iter().collect()));
        }
    }

    pub(crate) fn from_count(
        kind: tricerules_card_model::primitives::CountResultKind,
        value: u32,
    ) -> Self {
        Self {
            values: vec![OperationValue::Count { kind, value }],
        }
    }

    pub(crate) fn record_count(
        &mut self,
        kind: tricerules_card_model::primitives::CountResultKind,
        value: u32,
    ) {
        if let Some(OperationValue::Count {
            kind: existing_kind,
            value: existing_value,
        }) = self
            .values
            .iter_mut()
            .find(|value| matches!(value, OperationValue::Count { .. }))
        {
            *existing_kind = kind;
            *existing_value = value;
        } else {
            self.values.push(OperationValue::Count { kind, value });
        }
    }

    pub(crate) fn count(
        &self,
        kind: tricerules_card_model::primitives::CountResultKind,
    ) -> Option<u32> {
        self.values.iter().find_map(|value| match value {
            OperationValue::Count {
                kind: actual,
                value,
            } if *actual == kind => Some(*value),
            _ => None,
        })
    }

    /// Accumulate one homogeneous collection while executing the same operation.
    pub(crate) fn append(&mut self, value: ProgramValue) -> Result<(), EngineError> {
        let tag = std::mem::discriminant(&value);
        let existing = self.values.iter_mut().find_map(|existing| match existing {
            OperationValue::Program(existing) if std::mem::discriminant(existing) == tag => {
                Some(existing)
            }
            _ => None,
        });
        let Some(existing) = existing else {
            return self.publish(value);
        };
        match (existing, value) {
            (ProgramValue::DrawReceipt(existing), ProgramValue::DrawReceipt(mut incoming))
            | (
                ProgramValue::PermanentSelection(existing),
                ProgramValue::PermanentSelection(mut incoming),
            )
            | (
                ProgramValue::CreatedTokenBatch(existing),
                ProgramValue::CreatedTokenBatch(mut incoming),
            )
            | (
                ProgramValue::TokenCopyBatch(existing),
                ProgramValue::TokenCopyBatch(mut incoming),
            )
            | (ProgramValue::HandSelection(existing), ProgramValue::HandSelection(mut incoming)) => {
                existing.append(&mut incoming)
            }
            (
                ProgramValue::CardResultCohort(existing),
                ProgramValue::CardResultCohort(mut incoming),
            ) => {
                if existing.program_discard_result.is_some()
                    || incoming.program_discard_result.is_some()
                {
                    return Err(EngineError::ProgramInvariant(
                        "card-action facts contain an implicit discard result",
                    ));
                }
                existing.cards.append(&mut incoming.cards);
            }
            _ => {
                return Err(EngineError::ProgramInvariant(
                    "operation cannot append a singleton result",
                ))
            }
        }
        Ok(())
    }

    pub(crate) fn record_cards(
        &mut self,
        cards: impl IntoIterator<Item = CardResultEntry>,
    ) -> Result<(), EngineError> {
        self.append(ProgramValue::CardResultCohort(CardResultCohort {
            cards: cards.into_iter().collect(),
            program_discard_result: None,
        }))
    }

    pub(crate) fn record_counter_placements(
        &mut self,
        receipts: impl IntoIterator<Item = CounterPlacementReceipt>,
    ) {
        if let Some(OperationValue::CounterPlacements(existing)) = self
            .values
            .iter_mut()
            .find(|value| matches!(value, OperationValue::CounterPlacements(_)))
        {
            existing.extend(receipts);
        } else {
            self.values.push(OperationValue::CounterPlacements(
                receipts.into_iter().collect(),
            ));
        }
    }

    pub(crate) fn counter_placement_facts(&self) -> Option<&[CounterPlacementReceipt]> {
        self.values.iter().find_map(|value| match value {
            OperationValue::CounterPlacements(receipts) => Some(receipts.as_slice()),
            _ => None,
        })
    }

    pub(crate) fn record_sacrifice(&mut self, receipt: TriggerObjectRef) {
        if let Some(OperationValue::SacrificedObjects(existing)) = self
            .values
            .iter_mut()
            .find(|value| matches!(value, OperationValue::SacrificedObjects(_)))
        {
            existing.push(receipt);
        } else {
            self.values
                .push(OperationValue::SacrificedObjects(vec![receipt]));
        }
    }

    /// A parked operation retains actual partial facts separately from its parent's run context.
    /// Token-copy origin comes from its executor's typed partial value, never the requested slot.
    pub(crate) fn with_completion(mut self, mut completion: Self) -> Result<Self, EngineError> {
        if self
            .value(|value| matches!(value, ProgramValue::TokenCopyBatch(_)))
            .is_some()
        {
            for value in &mut completion.values {
                if let OperationValue::Program(ProgramValue::CreatedTokenBatch(objects)) = value {
                    *value = OperationValue::Program(ProgramValue::TokenCopyBatch(std::mem::take(
                        objects,
                    )));
                }
            }
        }
        for value in completion.values {
            match value {
                OperationValue::Program(
                    value @ (ProgramValue::DrawReceipt(_)
                    | ProgramValue::PermanentSelection(_)
                    | ProgramValue::CreatedTokenBatch(_)
                    | ProgramValue::TokenCopyBatch(_)
                    | ProgramValue::HandSelection(_)
                    | ProgramValue::CardResultCohort(_)),
                ) => self.append(value)?,
                OperationValue::Program(value) => self.set(value),
                OperationValue::CounterPlacements(receipts) => {
                    self.record_counter_placements(receipts)
                }
                OperationValue::SacrificedObjects(receipts) => {
                    for receipt in receipts {
                        self.record_sacrifice(receipt);
                    }
                }
                OperationValue::ExiledObjects(objects) => self.record_exiled_objects(objects),
                OperationValue::Count { kind, value } => self.record_count(kind, value),
            }
        }
        Ok(self)
    }
}

impl ResolutionFacts for OperationResults {
    fn value(&self, predicate: fn(&ProgramValue) -> bool) -> Option<&ProgramValue> {
        self.values.iter().find_map(|value| match value {
            OperationValue::Program(value) if predicate(value) => Some(value),
            _ => None,
        })
    }
    fn counter_placements(&self) -> &[CounterPlacementReceipt] {
        self.counter_placement_facts().unwrap_or(&[])
    }
    fn was_sacrificed(&self, object_id: ObjectId, generation: u64, _actor: PlayerId) -> bool {
        self.values.iter().any(|value| match value {
            OperationValue::SacrificedObjects(receipts) => receipts.iter().any(|receipt| {
                receipt.object_id == object_id && receipt.zone_change_generation == generation
            }),
            _ => false,
        })
    }
}

impl From<CardResultCohort> for OperationResults {
    fn from(mut cohort: CardResultCohort) -> Self {
        let discard = cohort.program_discard_result.take();
        let mut results = Self {
            values: vec![OperationValue::Program(ProgramValue::CardResultCohort(
                cohort,
            ))],
        };
        if let Some(discard) = discard {
            results
                .values
                .push(OperationValue::Program(ProgramValue::DiscardResult(
                    discard,
                )));
        }
        results
    }
}
