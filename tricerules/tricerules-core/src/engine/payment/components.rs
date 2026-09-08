//! Object-backed payment requirements. Authoring vocabularies keep their rules context;
//! these requirements share candidate selection and generation-bound validation before
//! lowering to the transaction's resource debits.

use super::transaction::CostDebit;
use super::*;

/// Resolution filters use mass-effect matching; announced costs use the existing cost
/// predicate (including its source-relative exclusions). Neither is a targeting action.
#[derive(Clone)]
pub(in crate::engine) enum PermanentPaymentFilter {
    Controlled,
    Announced {
        source: Option<ObjectId>,
        filter: Box<TargetFilter>,
    },
    Resolution(Box<TargetFilter>),
}

#[derive(Clone)]
pub(in crate::engine) enum ObjectPaymentComponent {
    Discard {
        filter: Option<CardTypeFilter>,
        excluded: Option<ObjectId>,
    },
    Sacrifice {
        filter: PermanentPaymentFilter,
        only_source: Option<rv1::CostObjectRef>,
    },
    Tap {
        constraint: ObjectPaymentConstraint,
        filter: PermanentPaymentFilter,
        excluded: Option<ObjectId>,
        cast_cost_kind: Option<ObjectCastCostKind>,
    },
    Blight {
        count: u32,
    },
}

pub(super) struct PlannedObjectPayment {
    pub component: ObjectPaymentComponent,
    pub objects: Vec<rv1::CostObjectRef>,
}

impl ObjectPaymentComponent {
    pub(in crate::engine) fn discard(excluded: Option<ObjectId>) -> Self {
        Self::Discard {
            filter: None,
            excluded,
        }
    }

    pub(in crate::engine) fn announced_sacrifice(
        source: Option<ObjectId>,
        filter: &TargetFilter,
    ) -> Self {
        Self::Sacrifice {
            filter: PermanentPaymentFilter::Announced {
                source,
                filter: Box::new(filter.clone()),
            },
            only_source: None,
        }
    }

    pub(in crate::engine) fn announced_tap(
        source: Option<ObjectId>,
        filter: &TargetFilter,
        constraint: ObjectPaymentConstraint,
        excluded: Option<ObjectId>,
        cast_cost_kind: Option<ObjectCastCostKind>,
    ) -> Self {
        Self::Tap {
            filter: PermanentPaymentFilter::Announced {
                source,
                filter: Box::new(filter.clone()),
            },
            constraint,
            excluded,
            cast_cost_kind,
        }
    }

    pub(in crate::engine) fn resolution(
        source: rv1::CostObjectRef,
        cost: &ResolutionCost,
    ) -> Option<Self> {
        Some(match cost {
            ResolutionCost::DiscardCard { filter } => Self::Discard {
                filter: *filter,
                excluded: None,
            },
            ResolutionCost::SacrificePermanent {
                filter,
                source_only,
            } => Self::Sacrifice {
                filter: PermanentPaymentFilter::Resolution(Box::new(filter.clone())),
                only_source: source_only.then_some(source),
            },
            ResolutionCost::TapPermanents {
                count,
                filter,
                exclude_source,
            } => Self::Tap {
                constraint: ObjectPaymentConstraint::ExactCount(*count),
                filter: PermanentPaymentFilter::Resolution(Box::new(filter.clone())),
                excluded: exclude_source.then_some(source.object_id),
                cast_cost_kind: None,
            },
            ResolutionCost::Blight { count } => Self::Blight { count: *count },
            ResolutionCost::None | ResolutionCost::Mana(_) | ResolutionCost::Waterbend(_) => {
                return None
            }
        })
    }

    fn matches(&self, engine: &GameEngine, player: PlayerId, oid: ObjectId) -> bool {
        let Some(object) = engine.state.objects.get(&oid) else {
            return false;
        };
        match self {
            Self::Discard { filter, excluded } => {
                object.zone == Zone::Hand
                    && object.owner == player
                    && *excluded != Some(oid)
                    && resolution::card_matches_type_filter(
                        &engine.state,
                        engine.registry,
                        oid,
                        filter.as_ref(),
                    )
            }
            Self::Sacrifice {
                filter,
                only_source,
            } => {
                only_source.is_none_or(|source| {
                    source.object_id == oid && engine.payment_object_ref(oid) == source
                }) && filter.matches(engine, player, oid)
            }
            Self::Tap {
                filter, excluded, ..
            } => !object.tapped && *excluded != Some(oid) && filter.matches(engine, player, oid),
            Self::Blight { .. } => engine.can_blight_creature(player, oid),
        }
    }

    pub(in crate::engine) fn candidates(
        &self,
        engine: &GameEngine,
        player: PlayerId,
    ) -> Vec<ObjectId> {
        let Some(index) = engine.state.player_idx(player) else {
            return Vec::new();
        };
        if matches!(self, Self::Blight { .. }) {
            // Blight publishes ascending object IDs, independently of battlefield vector order.
            return engine.blight_candidates(player);
        }
        if matches!(self, Self::Discard { .. }) {
            engine.state.players[index]
                .hand
                .iter()
                .copied()
                .filter(|oid| self.matches(engine, player, *oid))
                .collect()
        } else {
            engine
                .state
                .players
                .iter()
                .flat_map(|p| p.battlefield.iter().copied())
                .filter(|oid| self.matches(engine, player, *oid))
                .collect()
        }
    }

    pub(super) fn validate(
        &self,
        engine: &GameEngine,
        player: PlayerId,
        objects: &[rv1::CostObjectRef],
    ) -> Result<(), EngineError> {
        let count_valid = match self {
            Self::Tap { constraint, .. } => {
                engine.object_payment_selection_satisfies(*constraint, objects)
            }
            _ => objects.len() == 1,
        };
        let candidates = self.candidates(engine, player);
        let mut distinct = HashSet::new();
        if !count_valid
            || !objects.iter().all(|object| {
                distinct.insert(object.object_id)
                    && candidates.contains(&object.object_id)
                    && engine.payment_object_ref(object.object_id) == *object
            })
        {
            return Err(EngineError::Illegal("illegal or stale object payment"));
        }
        if let Self::Blight { count } = self {
            engine.validate_blight(player, *count, &objects[0])?;
        }
        Ok(())
    }
}

impl PermanentPaymentFilter {
    fn matches(&self, engine: &GameEngine, player: PlayerId, oid: ObjectId) -> bool {
        if !engine
            .state
            .objects
            .get(&oid)
            .is_some_and(|object| object.zone == Zone::Battlefield && object.controller == player)
        {
            return false;
        }
        match self {
            Self::Controlled => true,
            Self::Announced { source, filter } => {
                engine.ability_cost_permanent_matches(player, *source, oid, filter)
            }
            Self::Resolution(filter) => super::super::targeting::object_matches_scoped_mass_filter(
                engine, oid, filter, player,
            ),
        }
    }
}

impl GameEngine {
    pub(super) fn plan_object_payment(
        &self,
        player: PlayerId,
        component: ObjectPaymentComponent,
        objects: &[rv1::CostObjectRef],
    ) -> Result<CostDebit, EngineError> {
        component.validate(self, player, objects)?;
        Ok(CostDebit::Objects(PlannedObjectPayment {
            component,
            objects: objects.to_vec(),
        }))
    }

    /// Lower only after revalidation, before any mutation. Keeping resource debits separate
    /// lets mana, Convoke/Waterbend taps, and other non-object requirements retain their own
    /// planning windows while sharing the same ordered commit.
    pub(super) fn object_payment_debit(
        &self,
        payment: PlannedObjectPayment,
    ) -> Result<CostDebit, EngineError> {
        let objects = payment.objects;
        Ok(match payment.component {
            ObjectPaymentComponent::Discard { .. } => CostDebit::Discard {
                object_id: objects[0].object_id,
                generation: objects[0].zone_change_generation,
                owner: self.state.objects[&objects[0].object_id].owner,
            },
            ObjectPaymentComponent::Sacrifice { .. } => CostDebit::Sacrifice {
                snapshot: self
                    .sacrifice_snapshot(objects[0].object_id)
                    .ok_or(EngineError::Illegal("sacrifice permanent missing"))?,
                owner: self.state.objects[&objects[0].object_id].owner,
            },
            ObjectPaymentComponent::Tap {
                constraint,
                cast_cost_kind,
                ..
            } => CostDebit::TapGroup {
                objects: objects
                    .into_iter()
                    .map(|object| (object.object_id, object.zone_change_generation))
                    .collect(),
                constraint: Some(constraint),
                cast_cost_kind,
            },
            ObjectPaymentComponent::Blight { count } => CostDebit::Blight {
                object: objects[0],
                count,
            },
        })
    }
}
