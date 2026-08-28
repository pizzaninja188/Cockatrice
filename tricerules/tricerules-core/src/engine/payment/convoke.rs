//! Convoke (CR 702.51), shared by Unexpected Assistance and Merrow Skyswimmer.
//! Oracle/rulings checked 2026-08-26; CR 601.2f-h, 400.7, 702.51 in the official 2026-08-19 text.
//! Convoke and Waterbend queries never reserve objects, mutate state, or advance replay.
//! CastSpell, ActivateAbility and SubmitResolutionChoice commit the same validated payment plan.
use super::super::*;
use super::demand::{normalize, Demand};
use super::transaction::PreparedPaymentCosts;

pub(in crate::engine) fn mana_counts(mana: Option<&rv1::PaymentMana>) -> [u32; 6] {
    mana.map_or([0; 6], |m| [m.w, m.u, m.b, m.r, m.g, m.c])
}

fn mana_message(v: [u32; 6]) -> rv1::PaymentMana {
    rv1::PaymentMana {
        w: v[0],
        u: v[1],
        b: v[2],
        r: v[3],
        g: v[4],
        c: v[5],
    }
}

// Keep as much of one staged amount as still fits. Binary search keeps a preview bounded even
// for a very large X or a malicious amount; feasibility is monotonic as this amount increases.
fn retain_amount(requested: u32, mut fits: impl FnMut(u32) -> bool) -> u32 {
    let (mut low, mut high) = (0, requested);
    while low < high {
        let mid = low + (high - low) / 2 + (high - low) % 2;
        if fits(mid) {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    fits(low);
    low
}

fn slot(kind: i32) -> Option<usize> {
    match kind {
        1..=5 => Some(kind as usize - 1),
        6 => Some(6),
        _ => None,
    }
}

fn residuals(
    cost: &PreparedPaymentCosts,
    selection: &rv1::PaymentSelection,
) -> Result<Vec<Demand>, EngineError> {
    let mut mana = mana_counts(selection.mana.as_ref());
    for r in &cost.restricted_mana {
        for (i, n) in [r.w, r.u, r.b, r.r, r.g, r.c].into_iter().enumerate() {
            mana[i] = mana[i]
                .checked_add(n)
                .ok_or(EngineError::Illegal("payment overflow"))?;
        }
    }
    let mut creatures = selection
        .convoke
        .iter()
        .map(|c| slot(c.kind).ok_or(EngineError::Illegal("invalid Convoke contribution")))
        .collect::<Result<Vec<_>, _>>()?;
    creatures.extend(std::iter::repeat_n(6, selection.waterbend.len()));
    Ok(normalize(
        &cost.mana,
        cost.x_value,
        cost.extra_generic,
        cost.generic_reduction,
        &cost.flex_payments,
    )?
    .into_iter()
    .filter_map(|d| d.pay(&creatures, mana))
    .collect())
}

impl GameEngine {
    pub(in crate::engine) fn payment_object_ref(&self, oid: ObjectId) -> rv1::CostObjectRef {
        rv1::CostObjectRef {
            object_id: oid,
            zone_change_generation: self
                .state
                .zone_change_generation
                .get(&oid)
                .copied()
                .unwrap_or(0),
        }
    }

    fn convoke_candidate(
        &self,
        player: PlayerId,
        costs: &PreparedPaymentCosts,
        c: &rv1::ObjectPaymentContribution,
    ) -> bool {
        let Some(reference) = c.object.as_ref() else {
            return false;
        };
        let oid = reference.object_id;
        if self.payment_object_ref(oid) != *reference || !costs.can_convoke(oid) {
            return false;
        }
        let Some(object) = self.state.objects.get(&oid) else {
            return false;
        };
        if object.zone != Zone::Battlefield || object.tapped || object.controller != player {
            return false;
        }
        let Some(characteristics) = self.characteristics(oid) else {
            return false;
        };
        if !characteristics.is_creature() {
            return false;
        }
        match slot(c.kind) {
            Some(6) => true,
            Some(i @ 0..=4) => characteristics.colors.contains(
                &[
                    Color::White,
                    Color::Blue,
                    Color::Black,
                    Color::Red,
                    Color::Green,
                ][i],
            ),
            _ => false,
        }
    }

    pub(in crate::engine) fn validate_explicit_payment(
        &self,
        player: PlayerId,
        source: ObjectId,
        has_convoke: bool,
        costs: &PreparedPaymentCosts,
        selection: &rv1::PaymentSelection,
    ) -> Result<u32, EngineError> {
        if selection.expected_state_revision != self.state.command_index
            || selection.source.as_ref() != Some(&self.payment_object_ref(source))
        {
            return Err(EngineError::Illegal("stale payment"));
        }
        let mut seen = HashSet::new();
        for c in &selection.convoke {
            if !has_convoke
                || !self.convoke_candidate(player, costs, c)
                || !seen.insert(c.object.as_ref().map(|o| o.object_id))
            {
                return Err(EngineError::Illegal(
                    "illegal or duplicate Convoke selection",
                ));
            }
        }
        if !selection.waterbend.is_empty() && costs.waterbend_limit.is_none() {
            return Err(EngineError::Illegal("unexpected Waterbend contribution"));
        }
        if selection.waterbend.len() > costs.waterbend_limit.unwrap_or(0) as usize
            || selection.waterbend.iter().any(|reference| {
                !self.waterbend_candidate(player, costs, reference)
                    || !seen.insert(Some(reference.object_id))
            })
        {
            return Err(EngineError::Illegal(
                "illegal or duplicate Waterbend selection",
            ));
        }
        residuals(costs, selection)?
            .into_iter()
            .find(Demand::complete)
            .map(|d| d.life)
            .ok_or(EngineError::Illegal("payment is incomplete or excessive"))
    }

    pub fn preview_payment(
        &self,
        player: PlayerId,
        request: &rv1::PreviewPayment,
    ) -> rv1::PaymentPreview {
        let mut response = rv1::PaymentPreview {
            transaction_id: request.transaction_id,
            revision: request.revision,
            ..Default::default()
        };
        if let Err(error) = self.fill_payment_preview(player, request, &mut response) {
            response.error = error.to_string();
        }
        response
    }

    fn fill_payment_preview(
        &self,
        player: PlayerId,
        request: &rv1::PreviewPayment,
        response: &mut rv1::PaymentPreview,
    ) -> Result<(), EngineError> {
        if [
            request.cast_spell.is_some(),
            request.activate_ability.is_some(),
            request.resolution_choice.is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count()
            != 1
        {
            return Err(EngineError::Illegal(
                "payment preview requires exactly one action",
            ));
        }
        let (mut prepared, source, mut selection) = if let Some(command) = &request.cast_spell {
            if self.state.priority_player_id() != player || self.state.blocking_choice().is_some() {
                return Err(EngineError::Illegal("spell payment is not available now"));
            }
            let cast = self.prepare_spell_cast(player, command)?;
            if !cast.convoke {
                return Err(EngineError::Illegal("spell has no Convoke payment"));
            }
            (
                cast.payment,
                self.payment_object_ref(cast.oid),
                command.payment.clone().unwrap_or_default(),
            )
        } else if let Some(command) = &request.activate_ability {
            (
                self.prepare_activation_payment(player, command)?,
                self.payment_object_ref(command.source_object_id),
                command.payment.clone().unwrap_or_default(),
            )
        } else {
            let command = request.resolution_choice.as_ref().unwrap();
            let pending = self
                .state
                .pending_resolution
                .as_ref()
                .ok_or(EngineError::Illegal("no resolution payment"))?;
            let payment = pending
                .continuation
                .mana_payment()
                .filter(|p| p.waterbend)
                .ok_or(EngineError::Illegal("no Waterbend resolution payment"))?;
            if pending.deciding_player != player
                || command.decision != rv1::ResolutionChoiceDecision::PayMana as i32
                || !command.chosen_object_ids.is_empty()
                || command.cast_spell.is_some()
                || command.chosen_combat_defender.is_some()
            {
                return Err(EngineError::Illegal("invalid resolution payment proposal"));
            }
            (
                self.prepare_resolution_payment_costs(player, payment, &command.restricted_mana)?,
                self.payment_object_ref(pending.presentation.source_object_id),
                command.payment.clone().unwrap_or_default(),
            )
        };
        if selection.source.as_ref().is_some_and(|old| *old != source) {
            return Err(EngineError::Illegal("payment source changed"));
        }
        selection.source = Some(source);
        selection.expected_state_revision = self.state.command_index;
        let idx = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;
        let requested_mana = mana_counts(selection.mana.as_ref());
        let requested_restricted = std::mem::take(&mut prepared.restricted_mana);
        selection.mana = Some(mana_message([0; 6]));
        let initial = normalize(
            &prepared.mana,
            prepared.x_value,
            prepared.extra_generic,
            prepared.generic_reduction,
            &prepared.flex_payments,
        )?;
        response.total_cost = initial
            .iter()
            .map(Demand::label)
            .collect::<Vec<_>>()
            .join(" or ");
        let old_creatures = std::mem::take(&mut selection.convoke);
        let old_waterbend = std::mem::take(&mut selection.waterbend);
        let mut seen = HashSet::new();
        for c in old_creatures {
            let valid = prepared.waterbend_limit.is_none()
                && self.convoke_candidate(player, &prepared, &c)
                && seen.insert(c.object.as_ref().map(|o| o.object_id));
            if valid {
                selection.convoke.push(c);
                if !residuals(&prepared, &selection)?.is_empty() {
                    continue;
                }
                selection.convoke.pop();
            }
            response.selection_changed = true;
        }
        for reference in old_waterbend {
            if prepared
                .waterbend_limit
                .is_some_and(|limit| selection.waterbend.len() < limit as usize)
                && self.waterbend_candidate(player, &prepared, &reference)
                && seen.insert(Some(reference.object_id))
            {
                selection.waterbend.push(reference);
                if !residuals(&prepared, &selection)?.is_empty() {
                    continue;
                }
                selection.waterbend.pop();
            }
            response.selection_changed = true;
        }
        // Retain creatures first, then only remove unavailable or excess staged mana. The exact
        // resource planner also bounds restricted groups shared by multiple contributions.
        let fits = |costs: &PreparedPaymentCosts, selection: &rv1::PaymentSelection| {
            residuals(costs, selection).is_ok_and(|demands| !demands.is_empty())
                && super::mana::plan_exact_mana_payment(
                    &self.state,
                    idx,
                    mana_counts(selection.mana.as_ref()),
                    &costs.restricted_mana,
                    &costs.eligible_restricted_mana,
                    0,
                )
                .is_ok()
        };
        let mut ordinary = [0; 6];
        for i in 0..6 {
            let retained = retain_amount(requested_mana[i], |amount| {
                ordinary[i] = amount;
                selection.mana = Some(mana_message(ordinary));
                fits(&prepared, &selection)
            });
            response.selection_changed |= retained != requested_mana[i];
        }
        for requested in requested_restricted {
            if !prepared
                .eligible_restricted_mana
                .contains(&requested.restriction_group_id)
            {
                response.selection_changed = true;
                continue;
            }
            prepared.restricted_mana.push(rv1::ManaSpendSelection {
                restriction_group_id: requested.restriction_group_id,
                ..Default::default()
            });
            for (i, amount) in [
                requested.w,
                requested.u,
                requested.b,
                requested.r,
                requested.g,
                requested.c,
            ]
            .into_iter()
            .enumerate()
            {
                let retained = retain_amount(amount, |amount| {
                    let entry = prepared.restricted_mana.last_mut().unwrap();
                    match i {
                        0 => entry.w = amount,
                        1 => entry.u = amount,
                        2 => entry.b = amount,
                        3 => entry.r = amount,
                        4 => entry.g = amount,
                        _ => entry.c = amount,
                    }
                    fits(&prepared, &selection)
                });
                response.selection_changed |= retained != amount;
            }
            let last = prepared.restricted_mana.last().unwrap();
            if [last.w, last.u, last.b, last.r, last.g, last.c]
                .iter()
                .all(|n| *n == 0)
            {
                prepared.restricted_mana.pop();
            }
        }
        let mut remaining = residuals(&prepared, &selection)?;
        let life = remaining
            .first()
            .ok_or(EngineError::Illegal("invalid payment selection"))?
            .life;
        super::mana::plan_exact_mana_payment(
            &self.state,
            idx,
            mana_counts(selection.mana.as_ref()),
            &prepared.restricted_mana,
            &prepared.eligible_restricted_mana,
            life,
        )?;
        response.complete = remaining.iter().any(Demand::complete);
        remaining.sort_by_key(|d| {
            (
                d.amounts.iter().map(|n| u64::from(*n)).sum::<u64>(),
                d.amounts,
            )
        });
        response.remaining_cost = remaining
            .iter()
            .map(Demand::label)
            .collect::<Vec<_>>()
            .join(" or ");
        let mut objects = self.state.players[idx].battlefield.clone();
        objects.sort_unstable();
        for oid in objects {
            if selection
                .convoke
                .iter()
                .any(|c| c.object.as_ref().is_some_and(|o| o.object_id == oid))
            {
                continue;
            }
            let reference = self.payment_object_ref(oid);
            if let Some(limit) = prepared.waterbend_limit {
                if selection.waterbend.len() < limit as usize
                    && !selection.waterbend.iter().any(|r| r.object_id == oid)
                    && self.waterbend_candidate(player, &prepared, &reference)
                {
                    selection.waterbend.push(reference);
                    let fits = !residuals(&prepared, &selection)?.is_empty();
                    selection.waterbend.pop();
                    if fits {
                        response.candidates.push(rv1::ObjectPaymentCandidate {
                            object: Some(reference),
                            options: vec![rv1::ObjectPaymentKind::Waterbend as i32],
                        });
                    }
                }
                continue;
            }
            let mut options = Vec::new();
            for kind in 1..=6 {
                let contribution = rv1::ObjectPaymentContribution {
                    object: Some(reference),
                    kind,
                };
                if !self.convoke_candidate(player, &prepared, &contribution) {
                    continue;
                }
                selection.convoke.push(contribution);
                if !residuals(&prepared, &selection)?.is_empty() {
                    options.push(kind);
                }
                selection.convoke.pop();
            }
            if !options.is_empty() {
                response.candidates.push(rv1::ObjectPaymentCandidate {
                    object: Some(reference),
                    options,
                });
            }
        }
        response.restricted_mana = prepared.restricted_mana;
        response.selection = Some(selection);
        response.valid = true;
        if response.selection_changed {
            response.error =
                "Payment changed; stale or unavailable selections were removed.".into();
        }
        Ok(())
    }
}
