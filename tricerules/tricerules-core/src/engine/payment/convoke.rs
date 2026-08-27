//! Convoke (CR 702.51), shared by Unexpected Assistance and Merrow Skyswimmer.
//! Oracle/rulings checked 2026-08-26; CR 601.2f-h, 400.7, 702.51 in the official 2026-08-19 text.
//! Queries never reserve objects, mutate game state, or advance replay. Only CastSpell commits.
use super::super::*;
use super::demand::{normalize, Demand};
use super::transaction::PreparedSpellCosts;

pub(in crate::engine) fn mana_counts(mana: Option<&rv1::SpellPaymentMana>) -> [u32; 6] {
    mana.map_or([0; 6], |m| [m.w, m.u, m.b, m.r, m.g, m.c])
}

fn mana_message(v: [u32; 6]) -> rv1::SpellPaymentMana {
    rv1::SpellPaymentMana {
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
    cost: &PreparedSpellCosts,
    selection: &rv1::SpellPaymentSelection,
) -> Result<Vec<Demand>, EngineError> {
    let mut mana = mana_counts(selection.mana.as_ref());
    for r in &cost.restricted_mana {
        for (i, n) in [r.w, r.u, r.b, r.r, r.g, r.c].into_iter().enumerate() {
            mana[i] = mana[i]
                .checked_add(n)
                .ok_or(EngineError::Illegal("payment overflow"))?;
        }
    }
    let creatures = selection
        .convoke
        .iter()
        .map(|c| slot(c.kind).ok_or(EngineError::Illegal("invalid Convoke contribution")))
        .collect::<Result<Vec<_>, _>>()?;
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
    fn payment_object_ref(&self, oid: ObjectId) -> rv1::CostObjectRef {
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
        costs: &PreparedSpellCosts,
        c: &rv1::ConvokeContribution,
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

    pub(in crate::engine) fn validate_explicit_spell_payment(
        &self,
        player: PlayerId,
        source: ObjectId,
        has_convoke: bool,
        costs: &PreparedSpellCosts,
        selection: &rv1::SpellPaymentSelection,
    ) -> Result<u32, EngineError> {
        if selection.expected_state_revision != self.state.command_index
            || selection.source.as_ref() != Some(&self.payment_object_ref(source))
        {
            return Err(EngineError::Illegal("stale spell payment"));
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
        residuals(costs, selection)?
            .into_iter()
            .find(Demand::complete)
            .map(|d| d.life)
            .ok_or(EngineError::Illegal(
                "spell payment is incomplete or excessive",
            ))
    }

    pub fn preview_spell_payment(
        &self,
        player: PlayerId,
        request: &rv1::PreviewSpellPayment,
    ) -> rv1::SpellPaymentPreview {
        let mut response = rv1::SpellPaymentPreview {
            transaction_id: request.transaction_id,
            revision: request.revision,
            ..Default::default()
        };
        if let Err(error) = self.fill_spell_payment_preview(player, request, &mut response) {
            response.error = error.to_string();
        }
        response
    }

    fn fill_spell_payment_preview(
        &self,
        player: PlayerId,
        request: &rv1::PreviewSpellPayment,
        response: &mut rv1::SpellPaymentPreview,
    ) -> Result<(), EngineError> {
        if self.state.priority_player_id() != player || self.state.blocking_choice().is_some() {
            return Err(EngineError::Illegal("spell payment is not available now"));
        }
        let command = request
            .cast_spell
            .as_ref()
            .ok_or(EngineError::Illegal("missing proposed cast"))?;
        let mut prepared = self.prepare_spell_cast(player, command)?;
        if !prepared.convoke {
            return Err(EngineError::Illegal("spell has no Convoke payment"));
        }
        let source = self.payment_object_ref(prepared.oid);
        let mut selection = command.payment.clone().unwrap_or_default();
        if selection.source.as_ref().is_some_and(|old| *old != source) {
            return Err(EngineError::Illegal("cast source changed"));
        }
        selection.source = Some(source);
        selection.expected_state_revision = self.state.command_index;
        let idx = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;
        let requested_mana = mana_counts(selection.mana.as_ref());
        let requested_restricted = std::mem::take(&mut prepared.payment.restricted_mana);
        selection.mana = Some(mana_message([0; 6]));
        let initial = normalize(
            &prepared.payment.mana,
            prepared.payment.x_value,
            prepared.payment.extra_generic,
            prepared.payment.generic_reduction,
            &prepared.payment.flex_payments,
        )?;
        response.total_cost = initial
            .iter()
            .map(Demand::label)
            .collect::<Vec<_>>()
            .join(" or ");
        let old_creatures = std::mem::take(&mut selection.convoke);
        let mut seen = HashSet::new();
        for c in old_creatures {
            let valid = self.convoke_candidate(player, &prepared.payment, &c)
                && seen.insert(c.object.as_ref().map(|o| o.object_id));
            if valid {
                selection.convoke.push(c);
                if !residuals(&prepared.payment, &selection)?.is_empty() {
                    continue;
                }
                selection.convoke.pop();
            }
            response.selection_changed = true;
        }
        // Retain creatures first, then only remove unavailable or excess staged mana. The exact
        // resource planner also bounds restricted groups shared by multiple contributions.
        let fits = |costs: &PreparedSpellCosts, selection: &rv1::SpellPaymentSelection| {
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
                fits(&prepared.payment, &selection)
            });
            response.selection_changed |= retained != requested_mana[i];
        }
        for requested in requested_restricted {
            if !prepared
                .payment
                .eligible_restricted_mana
                .contains(&requested.restriction_group_id)
            {
                response.selection_changed = true;
                continue;
            }
            prepared
                .payment
                .restricted_mana
                .push(rv1::ManaSpendSelection {
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
                    let entry = prepared.payment.restricted_mana.last_mut().unwrap();
                    match i {
                        0 => entry.w = amount,
                        1 => entry.u = amount,
                        2 => entry.b = amount,
                        3 => entry.r = amount,
                        4 => entry.g = amount,
                        _ => entry.c = amount,
                    }
                    fits(&prepared.payment, &selection)
                });
                response.selection_changed |= retained != amount;
            }
            let last = prepared.payment.restricted_mana.last().unwrap();
            if [last.w, last.u, last.b, last.r, last.g, last.c]
                .iter()
                .all(|n| *n == 0)
            {
                prepared.payment.restricted_mana.pop();
            }
        }
        let mut remaining = residuals(&prepared.payment, &selection)?;
        let life = remaining
            .first()
            .ok_or(EngineError::Illegal("invalid payment selection"))?
            .life;
        super::mana::plan_exact_mana_payment(
            &self.state,
            idx,
            mana_counts(selection.mana.as_ref()),
            &prepared.payment.restricted_mana,
            &prepared.payment.eligible_restricted_mana,
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
            let mut options = Vec::new();
            for kind in 1..=6 {
                let contribution = rv1::ConvokeContribution {
                    object: Some(reference),
                    kind,
                };
                if !self.convoke_candidate(player, &prepared.payment, &contribution) {
                    continue;
                }
                selection.convoke.push(contribution);
                if !residuals(&prepared.payment, &selection)?.is_empty() {
                    options.push(kind);
                }
                selection.convoke.pop();
            }
            if !options.is_empty() {
                response.candidates.push(rv1::ConvokeCandidate {
                    object: Some(reference),
                    options,
                });
            }
        }
        response.restricted_mana = prepared.payment.restricted_mana;
        response.selection = Some(selection);
        response.valid = true;
        if response.selection_changed {
            response.error =
                "Payment changed; stale or unavailable selections were removed.".into();
        }
        Ok(())
    }
}
