//! Conservative negative proofs complement the bounded positive payment witnesses.
//! An omitted choice or a state-dependent mana engine is unknown, never an unavailable action.
use super::*;
use crate::engine::payment::transaction::PreparedPaymentCosts;

pub(super) fn announcements_complete(
    engine: &GameEngine,
    mana: &ManaCost,
    costs: &rv1::LegalCostChoices,
    targets: Option<&rv1::SpellTargets>,
    modes: &[rv1::LegalSpellMode],
) -> bool {
    // The positive search samples X and caps products, modes and object-cost cohorts. Only
    // this finite subset supplies negative evidence. Grizzly Bears and Lightning Bolt fit it.
    !mana.has_x()
        && !mana.pips.iter().any(|pip| {
            matches!(
                pip,
                ManaSymbol::Phyrexian(_) | ManaSymbol::HybridPhyrexian(_, _)
            )
        })
        && costs.choices.is_empty()
        && costs.cast_cost_groups.is_empty()
        && modes.is_empty()
        && targets.is_none_or(|targets| {
            !targets.is_damage_targets
                && targets.counter_allocation.is_none()
                && targets.groups.len() <= 1
                && targets.groups.iter().all(|group| {
                    group.max <= 1
                        && group.x_target_choices.is_none()
                        && group.valid_permanent_ids.len()
                            + group.valid_stack_ids.len()
                            + group.valid_graveyard_ids.len()
                            + engine.state.players.len()
                            < 128
                })
        })
}

fn ordinary_tap_mana(ability: &ActivatedAbilityDef) -> bool {
    ability.costs == [AbilityCost::Tap]
        && ability.is_mana_ability()
        && matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::ProduceMana {
                conditional: None,
                ..
            }]
        )
}

fn stable_mana_context(engine: &GameEngine) -> bool {
    // Tapping a source can alter a conditional cost, characteristics or mana multiplier.
    // Do not apply a fixed resource ceiling in a context we have not proved stable.
    engine.state.continuous_effects.is_empty()
        && engine.state.static_emblems.is_empty()
        && engine
            .state
            .players
            .iter()
            .flat_map(|p| &p.battlefield)
            .all(|&oid| {
                engine
                    .effective_face(oid)
                    .is_some_and(|face| face.static_abilities.is_empty())
            })
}

/// None means tapping may matter beyond paying for a current action. Pure tap mana from
/// Forest and Llanowar Elves is ignorable; pain, sacrifice and tap triggers are not.
pub(super) fn mana_only(
    engine: &GameEngine,
    player: PlayerId,
    oid: ObjectId,
    ability: &ActivatedAbilityDef,
) -> Option<bool> {
    if !ordinary_tap_mana(ability) {
        return Some(false);
    }
    if !stable_mana_context(engine) {
        return None;
    }
    // Source/attached trigger conditions can care about remaining untapped even when the tap
    // does not trigger immediately (e.g. Howling Mine with a granted mana ability).
    if engine
        .state
        .players
        .iter()
        .flat_map(|p| &p.battlefield)
        .any(|&source| {
            engine.effective_face(source).is_none_or(|face| {
                face.triggered_abilities
                    .iter()
                    .any(|ability| ability.intervening_if.is_some())
            })
        })
    {
        return None;
    }
    let mut probe = engine.clone();
    let taps = probe.tap_permanents(player, &[oid]);
    Some(probe.collect_event_triggers(&taps).is_empty())
}

/// Overestimate every color independently and ignore spending restrictions. Even this
/// generous pool cannot meet any normalized demand => the selected payment is impossible.
pub(in crate::engine) fn payment_impossible(
    engine: &GameEngine,
    cost: &PreparedPaymentCosts,
    convoke: bool,
    inventory: &AbilityInventory,
) -> bool {
    if convoke || cost.waterbend_limit.is_some() || !stable_mana_context(engine) {
        return false;
    }
    let player = cost.transaction.player();
    let Some(idx) = engine.state.player_idx(player) else {
        return false;
    };
    let pool = &engine.state.players[idx].mana_pool;
    let mut colors = [
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    ]
    .map(u64::from);
    let mut total = colors.iter().sum::<u64>();
    for group in &engine.state.players[idx].restricted_mana {
        let amount = group.amount;
        for (i, value) in [amount.w, amount.u, amount.b, amount.r, amount.g, amount.c]
            .into_iter()
            .enumerate()
        {
            colors[i] += u64::from(value);
            total += u64::from(value);
        }
    }
    for (oid, abilities) in inventory {
        let mut source_colors = [0; 6];
        let mut source_total = 0;
        for effective in abilities {
            let ability = &effective.definition;
            if !ability.is_mana_ability() {
                continue;
            }
            // Complex mana activations can unlock other sources or produce a different amount
            // after an earlier activation. An unavailable-at-present source is not enough proof.
            if !ordinary_tap_mana(ability) {
                return false;
            }
            if engine.check_tappable(*oid, "").is_err() {
                continue;
            }
            if mana_only(engine, player, *oid, ability) != Some(true) {
                return false;
            }
            let Some(options) = engine.active_mana_options(*oid, ability) else {
                return false;
            };
            for amount in options {
                let values =
                    [amount.w, amount.u, amount.b, amount.r, amount.g, amount.c].map(u64::from);
                source_total = source_total.max(values.iter().sum::<u64>());
                for i in 0..6 {
                    source_colors[i] = source_colors[i].max(values[i]);
                }
            }
        }
        total += source_total;
        for i in 0..6 {
            colors[i] += source_colors[i];
        }
    }
    let Ok(demands) = crate::engine::payment::demand::normalize(
        &cost.mana,
        cost.x_value,
        cost.extra_generic,
        cost.generic_reduction,
        &cost.flex_payments,
    ) else {
        return false;
    };
    let any_color = engine.may_spend_mana_as_any_color(player);
    demands.iter().all(|demand| {
        demand.amounts.iter().map(|&n| u64::from(n)).sum::<u64>() > total
            || u64::from(demand.amounts[5]) > colors[5]
            || (!any_color && (0..5).any(|i| u64::from(demand.amounts[i]) > colors[i]))
    })
}
