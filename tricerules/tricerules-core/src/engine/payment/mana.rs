use super::super::*;
use crate::state::RestrictedManaContribution;

/// Pool counts indexed `[W, U, B, R, G, C]`. `C` is the colorless slot.
pub(in crate::engine) type PoolVec = [u32; 6];
pub(in crate::engine) const POOL_C: usize = 5;

/// Index into a [`PoolVec`] for a colored pip.
pub(in crate::engine) fn color_index(c: ColorPip) -> usize {
    match c {
        ColorPip::W => 0,
        ColorPip::U => 1,
        ColorPip::B => 2,
        ColorPip::R => 3,
        ColorPip::G => 4,
    }
}

/// A flexible pip resolved against the pool after fixed colored/colorless demands are met.
pub(in crate::engine) enum FlexPip {
    /// Pay one mana of either color (hybrid `{G/U}`).
    Hybrid(ColorPip, ColorPip),
    /// Pay one mana of the color, or `n` generic (mono-hybrid `{2/W}`).
    Mono(u32, ColorPip),
    /// Pay one mana of the color (Phyrexian `{B/P}` paid with mana, not life).
    Color(ColorPip),
}

/// Backtracking solve: can `flex` plus `generic` generic mana be paid from `pool`?
pub(in crate::engine) fn solve_flex(
    pool: PoolVec,
    flex: &[FlexPip],
    idx: usize,
    generic: u32,
) -> Option<PoolVec> {
    if idx == flex.len() {
        let mut p = pool;
        let mut g = generic;
        for &i in &[POOL_C, 0, 1, 2, 3, 4] {
            let t = g.min(p[i]);
            p[i] -= t;
            g -= t;
        }
        return (g == 0).then_some(p);
    }
    match &flex[idx] {
        FlexPip::Hybrid(a, b) => {
            for &c in &[*a, *b] {
                let i = color_index(c);
                if pool[i] > 0 {
                    let mut p = pool;
                    p[i] -= 1;
                    if let Some(r) = solve_flex(p, flex, idx + 1, generic) {
                        return Some(r);
                    }
                }
            }
            None
        }
        FlexPip::Color(c) => {
            let i = color_index(*c);
            if pool[i] == 0 {
                return None;
            }
            let mut p = pool;
            p[i] -= 1;
            solve_flex(p, flex, idx + 1, generic)
        }
        FlexPip::Mono(n, c) => {
            let i = color_index(*c);
            if pool[i] > 0 {
                let mut p = pool;
                p[i] -= 1;
                if let Some(r) = solve_flex(p, flex, idx + 1, generic) {
                    return Some(r);
                }
            }
            solve_flex(pool, flex, idx + 1, generic + n)
        }
    }
}

pub(in crate::engine) fn solve_flex_with_required_spend(
    pool: PoolVec,
    flex: &[FlexPip],
    idx: usize,
    generic: u32,
    max_remaining: PoolVec,
) -> Option<PoolVec> {
    if idx == flex.len() {
        let mut p = pool;
        let mut g = generic;
        for i in 0..6 {
            let required = p[i].saturating_sub(max_remaining[i]);
            if required > g {
                return None;
            }
            p[i] -= required;
            g -= required;
        }
        for &i in &[POOL_C, 0, 1, 2, 3, 4] {
            let t = g.min(p[i]);
            p[i] -= t;
            g -= t;
        }
        return (g == 0 && (0..6).all(|i| p[i] <= max_remaining[i])).then_some(p);
    }
    match &flex[idx] {
        FlexPip::Hybrid(a, b) => {
            for &color in &[*a, *b] {
                let i = color_index(color);
                if pool[i] > 0 {
                    let mut next = pool;
                    next[i] -= 1;
                    if let Some(result) =
                        solve_flex_with_required_spend(next, flex, idx + 1, generic, max_remaining)
                    {
                        return Some(result);
                    }
                }
            }
            None
        }
        FlexPip::Color(color) => {
            let i = color_index(*color);
            if pool[i] == 0 {
                return None;
            }
            let mut next = pool;
            next[i] -= 1;
            solve_flex_with_required_spend(next, flex, idx + 1, generic, max_remaining)
        }
        FlexPip::Mono(amount, color) => {
            let i = color_index(*color);
            if pool[i] > 0 {
                let mut next = pool;
                next[i] -= 1;
                if let Some(result) =
                    solve_flex_with_required_spend(next, flex, idx + 1, generic, max_remaining)
                {
                    return Some(result);
                }
            }
            solve_flex_with_required_spend(
                pool,
                flex,
                idx + 1,
                generic.saturating_add(*amount),
                max_remaining,
            )
        }
    }
}

#[derive(Debug, Clone)]
pub(in crate::engine) struct ManaPaymentPlan {
    pub(in crate::engine) remaining: PoolVec,
    pub(in crate::engine) life_cost: u32,
    pub(in crate::engine) restricted_spent: Vec<(u32, ManaAmount)>,
    expected_pool: PoolVec,
    remaining_retained_combat: PoolVec,
    expected_retained_combat: PoolVec,
    expected_life: i32,
    expected_restricted: Vec<RestrictedManaContribution>,
}

/// Validate a mana payment without mutating the pool or life total. Activated costs use this
/// plan as one component of their all-or-nothing CR 601.2h transaction.
pub(in crate::engine) fn plan_mana_payment(
    state: &GameState,
    player_idx: usize,
    cost: &ManaCost,
    x_value: u32,
    extra_generic: u32,
    flex_payments: &[rv1::FlexPipPayment],
) -> Result<ManaPaymentPlan, EngineError> {
    plan_mana_payment_with_reduction(
        state,
        player_idx,
        cost,
        x_value,
        extra_generic,
        0,
        flex_payments,
    )
}

/// Determine a mana payment after applying CR 601.2f's generic increases-then-reductions order.
/// The reduction is applied only after the printed generic pips, X, and dynamic increases have
/// been combined, and `saturating_sub` implements CR 118.7's {0} floor.
#[allow(clippy::too_many_arguments)]
pub(in crate::engine) fn plan_mana_payment_with_reduction(
    state: &GameState,
    player_idx: usize,
    cost: &ManaCost,
    x_value: u32,
    extra_generic: u32,
    generic_reduction: u32,
    flex_payments: &[rv1::FlexPipPayment],
) -> Result<ManaPaymentPlan, EngineError> {
    plan_mana_payment_with_restricted_reduction(
        state,
        player_idx,
        cost,
        x_value,
        extra_generic,
        generic_reduction,
        flex_payments,
        &[],
        &[],
    )
}

#[allow(clippy::too_many_arguments)]
pub(in crate::engine) fn plan_mana_payment_with_restricted_reduction(
    state: &GameState,
    player_idx: usize,
    cost: &ManaCost,
    x_value: u32,
    extra_generic: u32,
    generic_reduction: u32,
    flex_payments: &[rv1::FlexPipPayment],
    restricted_selections: &[rv1::ManaSpendSelection],
    eligible_group_ids: &[u32],
) -> Result<ManaPaymentPlan, EngineError> {
    let life_pips: HashSet<usize> = flex_payments
        .iter()
        .filter(|fp| fp.pay_life)
        .map(|fp| fp.pip_index as usize)
        .collect();

    let mut need_color: PoolVec = [0; 6];
    let mut need_generic = extra_generic;
    let mut life_cost = 0u32;
    let mut flex: Vec<FlexPip> = Vec::new();
    for (i, pip) in cost.pips.iter().enumerate() {
        match pip {
            ManaSymbol::W => need_color[color_index(ColorPip::W)] += 1,
            ManaSymbol::U => need_color[color_index(ColorPip::U)] += 1,
            ManaSymbol::B => need_color[color_index(ColorPip::B)] += 1,
            ManaSymbol::R => need_color[color_index(ColorPip::R)] += 1,
            ManaSymbol::G => need_color[color_index(ColorPip::G)] += 1,
            ManaSymbol::C => need_color[POOL_C] += 1,
            ManaSymbol::Generic(n) => need_generic = need_generic.saturating_add(*n),
            ManaSymbol::X => need_generic = need_generic.saturating_add(x_value),
            ManaSymbol::Hybrid(a, b) => flex.push(FlexPip::Hybrid(*a, *b)),
            ManaSymbol::MonoHybrid(n, c) => flex.push(FlexPip::Mono(*n, *c)),
            ManaSymbol::Phyrexian(c) => {
                if life_pips.contains(&i) {
                    life_cost += 2;
                } else {
                    flex.push(FlexPip::Color(*c));
                }
            }
        }
    }
    need_generic = need_generic.saturating_sub(generic_reduction);

    // CR 119.4: a player can pay life only if they have at least that much.
    if life_cost > 0 && state.players[player_idx].life < life_cost as i32 {
        return Err(EngineError::Illegal(
            "not enough life to pay Phyrexian cost",
        ));
    }

    let player = &state.players[player_idx];
    let pool = &player.mana_pool;
    let unrestricted: PoolVec = [
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    ];
    let mut working = unrestricted;
    let mut restricted_spent = Vec::with_capacity(restricted_selections.len());
    let mut seen_groups = HashSet::new();
    for selection in restricted_selections {
        let group_id = selection.restriction_group_id;
        if group_id == 0 || !seen_groups.insert(group_id) || !eligible_group_ids.contains(&group_id)
        {
            return Err(EngineError::Illegal(
                "invalid or ineligible restricted mana selection",
            ));
        }
        let amount = ManaAmount {
            w: selection.w,
            u: selection.u,
            b: selection.b,
            r: selection.r,
            g: selection.g,
            c: selection.c,
        };
        if [amount.w, amount.u, amount.b, amount.r, amount.g, amount.c]
            .iter()
            .all(|count| *count == 0)
        {
            return Err(EngineError::Illegal("restricted mana selection is empty"));
        }
        let available = player
            .restricted_mana
            .iter()
            .filter(|entry| entry.restriction_group_id == group_id)
            .fold(ManaAmount::default(), |mut total, entry| {
                total.w += entry.amount.w;
                total.u += entry.amount.u;
                total.b += entry.amount.b;
                total.r += entry.amount.r;
                total.g += entry.amount.g;
                total.c += entry.amount.c;
                total
            });
        if amount.w > available.w
            || amount.u > available.u
            || amount.b > available.b
            || amount.r > available.r
            || amount.g > available.g
            || amount.c > available.c
        {
            return Err(EngineError::Illegal(
                "restricted mana selection exceeds pool",
            ));
        }
        for (slot, count) in [amount.w, amount.u, amount.b, amount.r, amount.g, amount.c]
            .into_iter()
            .enumerate()
        {
            working[slot] = working[slot].saturating_add(count);
        }
        restricted_spent.push((group_id, amount));
    }
    for i in 0..6 {
        if working[i] < need_color[i] {
            return Err(EngineError::Illegal(
                "not enough mana in pool; tap your lands first",
            ));
        }
        working[i] -= need_color[i];
    }
    let remaining = if restricted_spent.is_empty() {
        solve_flex(working, &flex, 0, need_generic)
    } else {
        solve_flex_with_required_spend(working, &flex, 0, need_generic, unrestricted)
    };
    let Some(remaining) = remaining else {
        return Err(EngineError::Illegal(
            "not enough mana in pool; tap your lands first",
        ));
    };
    let retained = &player.retained_combat_mana;
    let expected_retained_combat = [
        retained.white,
        retained.blue,
        retained.black,
        retained.red,
        retained.green,
        retained.colorless,
    ];
    debug_assert!((0..6).all(|i| expected_retained_combat[i] <= unrestricted[i]));
    let remaining_retained_combat =
        std::array::from_fn(|i| expected_retained_combat[i].min(remaining[i]));

    Ok(ManaPaymentPlan {
        remaining,
        life_cost,
        restricted_spent,
        expected_pool: unrestricted,
        remaining_retained_combat,
        expected_retained_combat,
        expected_life: player.life,
        expected_restricted: player.restricted_mana.clone(),
    })
}

pub(super) fn mana_payment_still_valid(
    state: &GameState,
    player_idx: usize,
    plan: &ManaPaymentPlan,
) -> bool {
    let Some(player) = state.players.get(player_idx) else {
        return false;
    };
    let pool = &player.mana_pool;
    plan.expected_pool
        == [
            pool.white,
            pool.blue,
            pool.black,
            pool.red,
            pool.green,
            pool.colorless,
        ]
        && plan.expected_life == player.life
        && plan.expected_retained_combat
            == [
                player.retained_combat_mana.white,
                player.retained_combat_mana.blue,
                player.retained_combat_mana.black,
                player.retained_combat_mana.red,
                player.retained_combat_mana.green,
                player.retained_combat_mana.colorless,
            ]
        && plan.expected_restricted == player.restricted_mana
}

pub(in crate::engine) fn commit_mana_payment(
    state: &mut GameState,
    player_idx: usize,
    plan: ManaPaymentPlan,
) {
    let pool = &mut state.players[player_idx].mana_pool;
    pool.white = plan.remaining[0];
    pool.blue = plan.remaining[1];
    pool.black = plan.remaining[2];
    pool.red = plan.remaining[3];
    pool.green = plan.remaining[4];
    pool.colorless = plan.remaining[POOL_C];
    let retained = &mut state.players[player_idx].retained_combat_mana;
    retained.white = plan.remaining_retained_combat[0];
    retained.blue = plan.remaining_retained_combat[1];
    retained.black = plan.remaining_retained_combat[2];
    retained.red = plan.remaining_retained_combat[3];
    retained.green = plan.remaining_retained_combat[4];
    retained.colorless = plan.remaining_retained_combat[POOL_C];
    for (group_id, spent) in plan.restricted_spent {
        let entries = &mut state.players[player_idx].restricted_mana;
        for (slot, mut remaining) in [spent.w, spent.u, spent.b, spent.r, spent.g, spent.c]
            .into_iter()
            .enumerate()
        {
            for entry in entries
                .iter_mut()
                .filter(|entry| entry.restriction_group_id == group_id)
            {
                if remaining == 0 {
                    break;
                }
                let field = match slot {
                    0 => &mut entry.amount.w,
                    1 => &mut entry.amount.u,
                    2 => &mut entry.amount.b,
                    3 => &mut entry.amount.r,
                    4 => &mut entry.amount.g,
                    _ => &mut entry.amount.c,
                };
                let take = remaining.min(*field);
                *field -= take;
                remaining -= take;
            }
            debug_assert_eq!(remaining, 0);
        }
        entries.retain(|entry| {
            let amount = entry.amount;
            amount.w + amount.u + amount.b + amount.r + amount.g + amount.c > 0
        });
    }
    state.players[player_idx].life -= plan.life_cost as i32;
}
