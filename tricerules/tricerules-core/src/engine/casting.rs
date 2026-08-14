use super::combat::priority_locked_for_combat_declaration;
use super::events::{ev_log, ev_priority_changed, format_spell_targets_log, object_display_name};
use super::legal_actions::fill_legal;
use super::resolution::{permanent_moved_event, sacrifice_permanent};
use super::targeting::{
    capture_stack_target, validate_ability_targets, validate_spell_targets, TargetSourceIdentity,
};
use super::*;

#[derive(Debug, Clone)]
struct SacrificeSnapshot {
    source: TriggerSourceSnapshot,
    was_creature: bool,
}

enum ValidatedAbilityCost {
    Tap,
    Mana(ManaPaymentPlan),
    Discard(ObjectId),
    Sacrifice(ObjectId),
}

struct AbilityCostPayment {
    move_events: Vec<rv1::RuledEvent>,
    sacrificed: Vec<SacrificeSnapshot>,
    paid_card_costs: Vec<PaidCardCost>,
    life_paid: u32,
}

enum PaidCardCost {
    Discard(String),
    Sacrifice(String),
}

impl PaidCardCost {
    fn log_phrase(&self) -> String {
        match self {
            Self::Discard(card_name) => format!("discarding {card_name}"),
            Self::Sacrifice(card_name) => format!("sacrificing {card_name}"),
        }
    }
}

fn format_paid_card_costs_log(costs: &[PaidCardCost]) -> String {
    let phrases: Vec<_> = costs.iter().map(PaidCardCost::log_phrase).collect();
    match phrases.as_slice() {
        [] => String::new(),
        [only] => format!(" {only}"),
        [first, second] => format!(" {first} and {second}"),
        _ => format!(
            " {}, and {}",
            phrases[..phrases.len() - 1].join(", "),
            phrases.last().expect("nonempty cost phrases")
        ),
    }
}

enum ValidatedSpellCost {
    Discard(ObjectId),
    Sacrifice(SacrificeSnapshot),
}

struct SpellCostPaymentPlan {
    mana: ManaPaymentPlan,
    components: Vec<ValidatedSpellCost>,
}

/// CR 702.8b: true if the card face is castable at instant speed (is an instant, or has flash).
pub(super) fn castable_at_instant_speed(face: &tricerules_cards::FaceRef<'_>) -> bool {
    face.is_instant || face.keywords.contains(&tricerules_cards::Keyword::Flash)
}

/// Pool counts indexed `[W, U, B, R, G, C]`. `C` is the colorless slot.
pub(super) type PoolVec = [u32; 6];
pub(super) const POOL_C: usize = 5;

/// Index into a [`PoolVec`] for a colored pip.
pub(super) fn color_index(c: ColorPip) -> usize {
    match c {
        ColorPip::W => 0,
        ColorPip::U => 1,
        ColorPip::B => 2,
        ColorPip::R => 3,
        ColorPip::G => 4,
    }
}

/// A flexible pip resolved against the pool after fixed colored/colorless demands are met.
pub(super) enum FlexPip {
    /// Pay one mana of either color (hybrid `{G/U}`).
    Hybrid(ColorPip, ColorPip),
    /// Pay one mana of the color, or `n` generic (mono-hybrid `{2/W}`).
    Mono(u32, ColorPip),
    /// Pay one mana of the color (Phyrexian `{B/P}` paid with mana, not life).
    Color(ColorPip),
}

/// Backtracking solve: can `flex` plus `generic` generic mana be paid from `pool`?
pub(super) fn solve_flex(
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

fn solve_flex_with_required_spend(
    pool: PoolVec,
    flex: &[FlexPip],
    idx: usize,
    generic: u32,
    max_remaining: PoolVec,
) -> Option<PoolVec> {
    if idx == flex.len() {
        let mut p = pool;
        let mut g = generic;
        // Any amount above `max_remaining` came from an explicitly selected restricted group and
        // must be consumed. Generic mana pays those pips first so the command cannot silently
        // leave selected restricted mana behind while spending unrestricted mana instead.
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
struct ManaPaymentPlan {
    remaining: PoolVec,
    life_cost: u32,
    restricted_spent: Vec<(u32, ManaAmount)>,
}

/// Validate a mana payment without mutating the pool or life total. Activated costs use this
/// plan as one component of their all-or-nothing CR 601.2h transaction.
fn plan_mana_payment(
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
fn plan_mana_payment_with_reduction(
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
fn plan_mana_payment_with_restricted_reduction(
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

    Ok(ManaPaymentPlan {
        remaining,
        life_cost,
        restricted_spent,
    })
}

fn commit_mana_payment(state: &mut GameState, player_idx: usize, plan: ManaPaymentPlan) {
    let pool = &mut state.players[player_idx].mana_pool;
    pool.white = plan.remaining[0];
    pool.blue = plan.remaining[1];
    pool.black = plan.remaining[2];
    pool.red = plan.remaining[3];
    pool.green = plan.remaining[4];
    pool.colorless = plan.remaining[POOL_C];
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

fn mana_filter_matches_face(filter: &ManaSpendFilter, face: &CardFace) -> bool {
    filter
        .card_type
        .is_none_or(|card_type| face.matches_card_type(card_type))
        && filter
            .subtype
            .as_ref()
            .is_none_or(|subtype| face.types.iter().any(|value| value == subtype))
}

fn mana_filter_matches_characteristics(
    filter: &ManaSpendFilter,
    characteristics: &Characteristics,
) -> bool {
    filter.card_type.is_none_or(|card_type| match card_type {
        CardTypeFilter::Enchantment => characteristics.has_type("Enchantment"),
        CardTypeFilter::Instant => characteristics.has_type("Instant"),
        CardTypeFilter::Sorcery => characteristics.has_type("Sorcery"),
        CardTypeFilter::InstantOrSorcery => {
            characteristics.has_type("Instant") || characteristics.has_type("Sorcery")
        }
        CardTypeFilter::Creature => characteristics.is_creature(),
        CardTypeFilter::Artifact => characteristics.is_artifact(),
        CardTypeFilter::Planeswalker => characteristics.has_type("Planeswalker"),
        CardTypeFilter::Noncreature => !characteristics.is_creature(),
    }) && filter
        .subtype
        .as_ref()
        .is_none_or(|subtype| characteristics.has_type(subtype))
}

#[cfg(test)]
mod restricted_mana_filter_tests {
    use super::*;

    #[test]
    fn planeswalker_subtype_filter_requires_both_characteristics() {
        let filter = ManaSpendFilter {
            card_type: Some(CardTypeFilter::Planeswalker),
            subtype: Some("Chandra".into()),
        };
        let chandra = CardFace {
            types: vec!["Planeswalker".into(), "Chandra".into()],
            ..Default::default()
        };
        let jaya = CardFace {
            types: vec!["Planeswalker".into(), "Jaya".into()],
            ..Default::default()
        };
        let elemental_named_chandra = CardFace {
            types: vec!["Creature".into(), "Elemental".into(), "Chandra".into()],
            ..Default::default()
        };

        assert!(mana_filter_matches_face(&filter, &chandra));
        assert!(!mana_filter_matches_face(&filter, &jaya));
        assert!(!mana_filter_matches_face(&filter, &elemental_named_chandra));
    }
}

/// Pays `cost` after first proving the whole mana component is affordable.
#[cfg(test)]
pub(super) fn pay_mana(
    state: &mut GameState,
    player_idx: usize,
    cost: &ManaCost,
    x_value: u32,
    extra_generic: u32,
    flex_payments: &[rv1::FlexPipPayment],
) -> Result<u32, EngineError> {
    let plan = plan_mana_payment(
        state,
        player_idx,
        cost,
        x_value,
        extra_generic,
        flex_payments,
    )?;
    let life_cost = plan.life_cost;
    commit_mana_payment(state, player_idx, plan);
    Ok(life_cost)
}

impl GameEngine {
    pub(super) fn eligible_restricted_mana_for_spell(
        &self,
        player_idx: usize,
        face: &CardFace,
    ) -> Vec<u32> {
        let mut ids: Vec<u32> = self.state.players[player_idx]
            .restricted_mana
            .iter()
            .filter_map(|entry| {
                let restriction = self
                    .state
                    .mana_restrictions
                    .get(entry.restriction_group_id.checked_sub(1)? as usize)?;
                restriction
                    .cast_spell
                    .iter()
                    .any(|filter| mana_filter_matches_face(filter, face))
                    .then_some(entry.restriction_group_id)
            })
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    pub(super) fn eligible_restricted_mana_for_ability(
        &self,
        player_idx: usize,
        source_oid: ObjectId,
    ) -> Vec<u32> {
        let Some(characteristics) = self.characteristics(source_oid) else {
            return Vec::new();
        };
        let mut ids: Vec<u32> = self.state.players[player_idx]
            .restricted_mana
            .iter()
            .filter_map(|entry| {
                let restriction = self
                    .state
                    .mana_restrictions
                    .get(entry.restriction_group_id.checked_sub(1)? as usize)?;
                restriction
                    .activate_ability
                    .iter()
                    .any(|filter| mana_filter_matches_characteristics(filter, &characteristics))
                    .then_some(entry.restriction_group_id)
            })
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    fn spell_generic_reduction(
        &self,
        player: PlayerId,
        source_oid: ObjectId,
        modifiers: &[SpellCostModifier],
    ) -> u32 {
        let context = ConditionContext {
            controller: player,
            source_object_id: source_oid,
            source_zone_change: self
                .state
                .zone_change_generation
                .get(&source_oid)
                .copied()
                .unwrap_or(0),
        };
        modifiers
            .iter()
            .fold(0u32, |total, modifier| match modifier {
                SpellCostModifier::ConditionalGenericReduction { amount, condition }
                    if self.condition_holds(condition, context) =>
                {
                    total.saturating_add(*amount)
                }
                SpellCostModifier::ConditionalGenericReduction { .. } => total,
            })
    }

    /// The fixed cost string published in legal actions. Registry validation currently excludes
    /// X and target-count surcharges on modified faces, so reducing authored generic pips here is
    /// the exact cost the existing client must stage.
    pub(super) fn effective_fixed_spell_cost(
        &self,
        player: PlayerId,
        source_oid: ObjectId,
        base_cost: &ManaCost,
        modifiers: &[SpellCostModifier],
    ) -> ManaCost {
        let mut remaining_reduction = self.spell_generic_reduction(player, source_oid, modifiers);
        let mut pips = Vec::with_capacity(base_cost.pips.len());
        for pip in &base_cost.pips {
            match pip {
                ManaSymbol::Generic(amount) => {
                    let remaining = amount.saturating_sub(remaining_reduction);
                    remaining_reduction = remaining_reduction.saturating_sub(*amount);
                    if remaining > 0 {
                        pips.push(ManaSymbol::Generic(remaining));
                    }
                }
                _ => pips.push(pip.clone()),
            }
        }
        if pips.is_empty() && !base_cost.pips.is_empty() {
            pips.push(ManaSymbol::Generic(0));
        }
        ManaCost { pips }
    }

    pub(super) fn can_pay_generic_mana(&self, player: PlayerId, amount: u32) -> bool {
        self.state.player_idx(player).is_some_and(|player_idx| {
            plan_mana_payment(
                &self.state,
                player_idx,
                &ManaCost::default(),
                0,
                amount,
                &[],
            )
            .is_ok()
        })
    }

    pub(super) fn pay_generic_mana(
        &mut self,
        player: PlayerId,
        amount: u32,
    ) -> Result<(), EngineError> {
        let player_idx = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;
        let plan = plan_mana_payment(
            &self.state,
            player_idx,
            &ManaCost::default(),
            0,
            amount,
            &[],
        )?;
        commit_mana_payment(&mut self.state, player_idx, plan);
        Ok(())
    }

    pub(super) fn cast_spell(
        &mut self,
        player: PlayerId,
        command: &rv1::CastSpell,
    ) -> Result<RuledEventBatch, EngineError> {
        let targets = command.targets.as_slice();
        let x_value = command.x_value;
        let flex_payments = command.flex_payments.as_slice();
        let face_index = command.face_index as usize;
        let selected_modes = command.selected_modes.as_slice();
        let cost_selections = command.cost_selections.as_slice();
        let restricted_mana = command.restricted_mana.as_slice();
        if self.state.turn_step == TurnStep::Cleanup {
            return Err(EngineError::Illegal("no spells during cleanup"));
        }
        let idx = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;
        let source = command
            .source
            .as_ref()
            .and_then(|source| source.location.as_ref())
            .ok_or(EngineError::Illegal("missing cast source"))?;
        let from_hand = matches!(source, rv1::cast_source::Location::HandIndex(_));
        let (oid, flashback, from_adventure) = match source {
            rv1::cast_source::Location::HandIndex(hand_index) => (
                *self.state.players[idx]
                    .hand
                    .get(*hand_index as usize)
                    .ok_or(EngineError::Illegal("bad hand index"))?,
                false,
                false,
            ),
            rv1::cast_source::Location::GraveyardObjectId(source_oid) => {
                if !self.state.players[idx].graveyard.contains(source_oid) {
                    return Err(EngineError::Illegal("card is not in your graveyard"));
                }
                (*source_oid, true, false)
            }
            rv1::cast_source::Location::ExileObjectId(source_oid) => {
                let object = self
                    .state
                    .objects
                    .get(source_oid)
                    .ok_or(EngineError::Illegal("unknown exile object"))?;
                let permission = object
                    .adventure_cast_permission
                    .ok_or(EngineError::Illegal(
                        "card has no cast-from-exile permission",
                    ))?;
                if object.zone != Zone::Exile || permission.player_id != player {
                    return Err(EngineError::Illegal("illegal cast from exile"));
                }
                if permission.face_index != face_index {
                    return Err(EngineError::Illegal("wrong Adventure face"));
                }
                (*source_oid, false, true)
            }
        };
        let has_multiple_cast_options =
            super::legal_actions::cast_option_count_for_source(self, player, source) > 1;
        let target_source = TargetSourceIdentity::current(self, oid);
        let card_id = self.state.objects.get(&oid).unwrap().card_id.clone();
        let def = self
            .registry
            .get(&card_id)
            .ok_or_else(|| EngineError::MissingCard(card_id.clone()))?;
        let face = def
            .face(face_index)
            .ok_or(EngineError::Illegal("bad face index"))?;
        if from_hand && !def.face_available_from_hand(face_index) {
            return Err(EngineError::Illegal("face cannot be cast from hand"));
        }
        if from_adventure && !face.is_permanent() {
            return Err(EngineError::Illegal(
                "Adventure permission requires a permanent face",
            ));
        }
        if face.is_land {
            return Err(EngineError::Illegal("use play land"));
        }
        let face_is_sorcery = face.is_sorcery;
        let face_instant_speed = castable_at_instant_speed(&face);
        let face_mana = if flashback {
            face.flashback_cost
                .clone()
                .ok_or(EngineError::Illegal("card has no flashback cost"))?
        } else {
            face.mana_cost.clone()
        };
        let face_name = face.name.to_string();
        let face_effects: Vec<SpellEffectKind> = face.spell_effect.to_vec();
        let face_targeting = face.targeting.clone();
        let modal_spell = face.modal_spell.clone();
        let additional_costs = face.additional_costs.clone();
        let cost_modifiers = face.cost_modifiers.clone();
        let eligible_restricted_mana = self.eligible_restricted_mana_for_spell(idx, face);
        let sorcery_ok = super::priority::sorcery_speed_available(&self.state, player);
        let instant_ok = super::priority::instant_timing_step_allowed(&self.state);
        if face_is_sorcery {
            if !sorcery_ok {
                return Err(EngineError::Illegal("sorcery speed only"));
            }
        } else if face_instant_speed {
            if !instant_ok {
                return Err(EngineError::Illegal("instant timing"));
            }
        } else if !sorcery_ok {
            return Err(EngineError::Illegal("sorcery speed only"));
        }
        if priority_locked_for_combat_declaration(&self.state) {
            return Err(EngineError::Illegal(
                "cannot cast until attack or block declaration is complete",
            ));
        }
        // As in `pass_priority`: `dispatch_command`'s blocking gate normally catches these first;
        // this is the local refusal with a message that names casting.
        match self.state.blocking_choice() {
            Some(BlockingChoice::TriggerTarget) => {
                return Err(EngineError::Illegal(
                    "must choose trigger target before casting",
                ));
            }
            Some(BlockingChoice::TriggerOrder) => {
                return Err(EngineError::Illegal(
                    "must order simultaneous triggers before casting",
                ));
            }
            Some(BlockingChoice::Resolution) => {
                return Err(EngineError::Illegal(
                    "must submit resolution choice before casting",
                ));
            }
            None => {}
        }
        let has_x = face_mana.has_x();
        if x_value != 0 && !has_x {
            return Err(EngineError::Illegal("x_value given but cost has no {X}"));
        }
        let chosen_x = if has_x { x_value } else { 0 };

        let mut public_targets: Vec<rv1::TargetRef> = Vec::new();
        let mut chosen_modes: Vec<ChosenSpellMode> = Vec::new();
        let mut chosen_mode_indices: Vec<u32> = Vec::new();
        let mut chosen_mode_labels: Vec<String> = Vec::new();
        let mut extra_generic = 0;
        if let Some(modal) = &modal_spell {
            if !targets.is_empty() {
                return Err(EngineError::Illegal(
                    "modal spells use selected_modes targets",
                ));
            }
            if selected_modes.len() < modal.min_modes as usize
                || selected_modes.len() > modal.max_modes as usize
            {
                return Err(EngineError::Illegal("illegal number of selected modes"));
            }
            let mut seen = HashSet::new();
            let mut ordered: Vec<&rv1::SelectedSpellMode> = selected_modes.iter().collect();
            ordered.sort_by_key(|selection| selection.mode_index);
            for selection in ordered {
                if !seen.insert(selection.mode_index) {
                    return Err(EngineError::Illegal("a mode may be selected only once"));
                }
                let mode = modal
                    .modes
                    .get(selection.mode_index as usize)
                    .ok_or(EngineError::Illegal("bad spell mode index"))?;
                validate_spell_targets(
                    self,
                    player,
                    target_source,
                    &mode.effects,
                    mode.targeting.as_ref(),
                    &selection.targets,
                )?;
                for effect in &mode.effects {
                    if let SpellEffectKind::DamageTargets {
                        amount,
                        division,
                        extra_mana_per_target,
                        ..
                    } = effect
                    {
                        if matches!(division, DamageDivision::ChooseAtCast) {
                            let allocated: u32 = selection
                                .targets
                                .iter()
                                .map(|target| target.damage_amount)
                                .sum();
                            if allocated
                                != amount.resolve_unconditional(chosen_x).ok_or(
                                    EngineError::Illegal(
                                        "DamageTargets cannot use a conditional amount",
                                    ),
                                )?
                                || selection
                                    .targets
                                    .iter()
                                    .any(|target| target.damage_amount == 0)
                            {
                                return Err(EngineError::Illegal(
                                    "damage allocations must be positive and sum to the total damage",
                                ));
                            }
                        }
                        if selection.targets.len() > 1 {
                            extra_generic +=
                                *extra_mana_per_target * (selection.targets.len() as u32 - 1);
                        }
                    }
                }
                public_targets.extend(selection.targets.iter().cloned());
                chosen_mode_indices.push(selection.mode_index);
                chosen_mode_labels.push(mode.label.clone());
                chosen_modes.push(ChosenSpellMode {
                    mode_index: selection.mode_index as usize,
                    targets: selection
                        .targets
                        .iter()
                        .map(|target| capture_stack_target(self, target))
                        .collect(),
                });
            }
        } else {
            if !selected_modes.is_empty() {
                return Err(EngineError::Illegal(
                    "selected_modes given for a nonmodal spell",
                ));
            }
            validate_spell_targets(
                self,
                player,
                target_source,
                &face_effects,
                face_targeting.as_ref(),
                targets,
            )?;
            for effect in &face_effects {
                if let SpellEffectKind::DamageTargets {
                    amount,
                    division,
                    extra_mana_per_target,
                    ..
                } = effect
                {
                    if matches!(division, DamageDivision::ChooseAtCast) {
                        let allocated: u32 =
                            targets.iter().map(|target| target.damage_amount).sum();
                        if allocated
                            != amount.resolve_unconditional(chosen_x).ok_or(
                                EngineError::Illegal(
                                    "DamageTargets cannot use a conditional amount",
                                ),
                            )?
                            || targets.iter().any(|target| target.damage_amount == 0)
                        {
                            return Err(EngineError::Illegal(
                                "damage allocations must be positive and sum to the total damage",
                            ));
                        }
                    }
                    if targets.len() > 1 {
                        extra_generic += *extra_mana_per_target * (targets.len() as u32 - 1);
                    }
                }
            }
            public_targets.extend_from_slice(targets);
        }

        let payment_plan = self.plan_spell_costs(
            player,
            idx,
            oid,
            &face_mana,
            chosen_x,
            extra_generic,
            self.spell_generic_reduction(player, oid, &cost_modifiers),
            flex_payments,
            &additional_costs,
            cost_selections,
            restricted_mana,
            &eligible_restricted_mana,
        )?;

        let trefs: Vec<ObjectId> = public_targets
            .iter()
            .map(|target| target.object_id)
            .collect();
        // CR 115.9: target-watchers see the final legal target set. Collect now so the event's
        // battlefield identity is exact, but do not stage anything unless all costs are paid and
        // the spell is successfully cast.
        let mut target_triggers = self.collect_event_triggers(&[GameEvent::TargetsChosen {
            controller: player,
            source: TargetingSourceKind::SpellCast,
            targets: trefs.clone(),
        }]);
        let payment = self.commit_spell_costs(player, idx, payment_plan)?;
        let life_paid = payment.life_paid;
        let paid_costs_line = format_paid_card_costs_log(&payment.paid_card_costs);

        let stack_targets = public_targets
            .iter()
            .map(|target| capture_stack_target(self, target))
            .collect();
        let tgt_line = format_spell_targets_log(&self.state, self.registry, &trefs);

        self.state.stack.push(StackItem {
            id: oid,
            controller: player,
            card_id: card_id.clone(),
            targets: stack_targets,
            ability_text: None,
            source_permanent_id: None,
            source_zone_change: 0,
            source_face_change: 0,
            ability_index: None,
            triggered_ability: None,
            is_triggered: false,
            is_copy: false,
            chosen_x,
            face_index,
            chosen_modes,
            // A spell's effects always act on its controller.
            trigger_player: None,
            trigger_object: None,
            flashback,
        });
        super::resolution::move_object_to_zone(
            &mut self.state,
            self.registry,
            oid,
            Zone::Stack,
            None,
        )?;

        self.state.passes_since_stack_change = 0;
        self.state.priority_idx = idx;

        let cast_card_id = card_id.clone();
        let mut batch = RuledEventBatch::default();
        let x_line = if has_x {
            format!(" (X={chosen_x})")
        } else {
            String::new()
        };
        let modes_line = if chosen_mode_labels.is_empty() {
            String::new()
        } else {
            format!(" [{}]", chosen_mode_labels.join("; "))
        };
        batch.events.push(ev_log(format!(
            "P{} casts {}{}{}{}{}",
            player, face_name, modes_line, x_line, paid_costs_line, tgt_line
        )));
        let mut stack_annotation = match (has_multiple_cast_options, has_x) {
            (true, true) => format!("{face_name} (X = {chosen_x})"),
            (true, false) => face_name.clone(),
            (false, true) => format!("X = {chosen_x}"),
            (false, false) => String::new(),
        };
        if !chosen_mode_labels.is_empty() {
            stack_annotation = chosen_mode_labels.join("\n");
        }
        // CR 702.34: nothing on the card face says a spell was cast for its flashback cost, and it
        // is exiled instead of going to its owner's graveyard when it leaves the stack — so the
        // annotation is the only warning an opponent gets while it is still resolvable. Prepended
        // rather than appended: it changes what the spell *is*, so it reads before the X value or
        // the chosen modes.
        if flashback {
            stack_annotation = if stack_annotation.is_empty() {
                "Flashback".to_string()
            } else {
                format!("Flashback\n{stack_annotation}")
            };
        }
        if life_paid > 0 {
            batch.events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                    player_id: player,
                    new_total: self.state.players[idx].life,
                    delta: -(life_paid as i32),
                })),
            });
            batch.events.push(ev_log(format!(
                "P{player} pays {life_paid} life (Phyrexian mana)."
            )));
        }
        for ev in payment.move_events {
            batch.events.push(ev);
        }
        batch.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::StackPushed(rv1::StackPushed {
                object_id: oid,
                description: face_name.clone(),
                targets: public_targets,
                ability_annotation: stack_annotation,
                card_id: cast_card_id.clone(),
                is_copy: false,
                is_triggered: false,
                copy_source_object_id: 0,
                chosen_mode_indices,
                chosen_mode_labels,
            })),
        });
        self.record_spell_cast();
        target_triggers.extend(self.collect_committed_sacrifice_cost_dies(payment.sacrificed));
        target_triggers.extend(self.collect_event_triggers(&[GameEvent::SpellCast {
            caster: player,
            card_id: cast_card_id,
            face_index,
        }]));
        // Both kinds of triggers are waiting when the cast completes, so they form one CR 603.3b
        // ordering group rather than forcing an artificial target-trigger/cast-trigger order.
        self.stage_triggers(target_triggers);
        batch.events.push(ev_priority_changed(self));
        fill_legal(&mut batch, self);
        Ok(batch)
    }

    pub(super) fn active_mana_options<'a>(
        &self,
        permanent_id: ObjectId,
        ability: &'a tricerules_cards::ActivatedAbilityDef,
    ) -> Option<&'a [tricerules_cards::ManaAmount]> {
        let default_options = ability.mana_options()?;
        let controller = self.state.objects.get(&permanent_id)?.controller;
        Some(
            ability
                .conditional_mana_output()
                .filter(|conditional| {
                    self.condition_holds(
                        &conditional.condition,
                        ConditionContext {
                            controller,
                            source_object_id: permanent_id,
                            source_zone_change: self
                                .state
                                .zone_change_generation
                                .get(&permanent_id)
                                .copied()
                                .unwrap_or(0),
                        },
                    )
                })
                .map(|conditional| conditional.options.as_slice())
                .unwrap_or(default_options.as_slice()),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn plan_spell_costs(
        &self,
        player: PlayerId,
        player_idx: usize,
        source_oid: ObjectId,
        mana_cost: &ManaCost,
        x_value: u32,
        extra_generic: u32,
        generic_reduction: u32,
        flex_payments: &[rv1::FlexPipPayment],
        costs: &[AdditionalCost],
        selections: &[rv1::CostSelection],
        restricted_mana: &[rv1::ManaSpendSelection],
        eligible_restricted_mana: &[u32],
    ) -> Result<SpellCostPaymentPlan, EngineError> {
        use rv1::cost_selection::Selection;

        let mut by_index = HashMap::new();
        for selection in selections {
            let cost_index = selection.cost_index as usize;
            if cost_index >= costs.len() || by_index.insert(cost_index, selection).is_some() {
                return Err(EngineError::Illegal("invalid or duplicate cost selection"));
            }
        }

        let mut components = Vec::with_capacity(costs.len());
        let mut consumed = HashSet::new();
        for (cost_index, cost) in costs.iter().enumerate() {
            let Some(selection) = by_index.get(&cost_index) else {
                return Err(EngineError::Illegal("missing additional cost selection"));
            };
            match cost {
                AdditionalCost::DiscardCard => {
                    let Some(Selection::HandIndex(hand_index)) = selection.selection else {
                        return Err(EngineError::Illegal("discard cost requires a hand card"));
                    };
                    let oid = self.state.players[player_idx]
                        .hand
                        .get(hand_index as usize)
                        .copied()
                        .ok_or(EngineError::Illegal("invalid discard hand slot"))?;
                    if oid == source_oid {
                        return Err(EngineError::Illegal(
                            "a spell cannot discard itself as a cost",
                        ));
                    }
                    if !consumed.insert(oid) {
                        return Err(EngineError::Illegal("one object cannot pay two costs"));
                    }
                    components.push(ValidatedSpellCost::Discard(oid));
                }
                AdditionalCost::SacrificePermanent { filter } => {
                    let Some(Selection::PermanentId(oid)) = selection.selection else {
                        return Err(EngineError::Illegal(
                            "sacrifice cost requires a battlefield permanent",
                        ));
                    };
                    if !self.ability_cost_permanent_matches(player, oid, filter) {
                        return Err(EngineError::Illegal("illegal sacrifice cost selection"));
                    }
                    if !consumed.insert(oid) {
                        return Err(EngineError::Illegal("one object cannot pay two costs"));
                    }
                    let snapshot = self
                        .sacrifice_snapshot(oid)
                        .ok_or(EngineError::Illegal("sacrifice permanent missing"))?;
                    components.push(ValidatedSpellCost::Sacrifice(snapshot));
                }
            }
        }
        if selections.len() != costs.len() {
            return Err(EngineError::Illegal("unexpected cost selection"));
        }

        Ok(SpellCostPaymentPlan {
            mana: plan_mana_payment_with_restricted_reduction(
                &self.state,
                player_idx,
                mana_cost,
                x_value,
                extra_generic,
                generic_reduction,
                flex_payments,
                restricted_mana,
                eligible_restricted_mana,
            )?,
            components,
        })
    }

    fn commit_spell_costs(
        &mut self,
        player: PlayerId,
        player_idx: usize,
        plan: SpellCostPaymentPlan,
    ) -> Result<AbilityCostPayment, EngineError> {
        let life_paid = plan.mana.life_cost;
        commit_mana_payment(&mut self.state, player_idx, plan.mana);
        let mut payment = AbilityCostPayment {
            move_events: vec![],
            sacrificed: vec![],
            paid_card_costs: vec![],
            life_paid,
        };
        for component in plan.components {
            let (oid, sacrificed) = match component {
                ValidatedSpellCost::Discard(oid) => (oid, None),
                ValidatedSpellCost::Sacrifice(snapshot) => {
                    (snapshot.source.object_id, Some(snapshot))
                }
            };
            let card_name = object_display_name(&self.state, self.registry, oid);
            let owner = self
                .state
                .objects
                .get(&oid)
                .map(|object| object.owner)
                .unwrap_or(player);
            if let Some(snapshot) = sacrificed {
                sacrifice_permanent(&mut self.state, self.registry, oid)?;
                payment.sacrificed.push(snapshot);
                payment
                    .paid_card_costs
                    .push(PaidCardCost::Sacrifice(card_name));
            } else {
                super::resolution::move_object_to_zone(
                    &mut self.state,
                    self.registry,
                    oid,
                    Zone::Graveyard,
                    None,
                )?;
                payment
                    .paid_card_costs
                    .push(PaidCardCost::Discard(card_name));
            }
            payment.move_events.push(permanent_moved_event(
                &self.state,
                oid,
                owner,
                rv1::permanent_moved::Destination::Graveyard,
            ));
        }
        Ok(payment)
    }

    pub(super) fn activate_ability(
        &mut self,
        player: PlayerId,
        command: &rv1::ActivateAbility,
    ) -> Result<RuledEventBatch, EngineError> {
        let permanent_id = command.permanent_id;
        let ability_index = command.ability_index as usize;
        let targets = command.targets.as_slice();
        let flex_payments = command.flex_payments.as_slice();
        let mana_option_index = command.mana_option_index;
        let cost_selections = command.cost_selections.as_slice();
        let restricted_mana = command.restricted_mana.as_slice();
        if self.state.priority_player_id() != player {
            return Err(EngineError::Illegal("not your priority"));
        }
        if self.state.turn_step == TurnStep::Cleanup {
            return Err(EngineError::Illegal("no abilities during cleanup"));
        }
        if priority_locked_for_combat_declaration(&self.state) {
            return Err(EngineError::Illegal(
                "cannot activate until attack or block declaration is complete",
            ));
        }
        let idx = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;

        self.state
            .objects
            .get(&permanent_id)
            .filter(|o| o.zone == Zone::Battlefield)
            .ok_or(EngineError::Illegal("permanent not on battlefield"))?;
        if !self.state.players[idx].battlefield.contains(&permanent_id) {
            return Err(EngineError::Illegal("not your permanent"));
        }

        let (card_id, face_up_index) = self
            .effective_card_identity(permanent_id)
            .map(|(card_id, face_index)| (card_id.to_string(), face_index))
            .ok_or(EngineError::Illegal("bad face index on permanent"))?;
        let ability = self
            .effective_face(permanent_id)
            .ok_or(EngineError::Illegal("bad face index on permanent"))?
            .activated_abilities
            .get(ability_index)
            .ok_or(EngineError::Illegal("no such activated ability"))?
            .clone();
        let resolving_mana_payment =
            self.state
                .pending_resolution
                .as_ref()
                .is_some_and(|pending| {
                    pending.mana_payment.is_some() && pending.deciding_player == player
                });
        if resolving_mana_payment {
            if ability.mana_options().is_none() {
                return Err(EngineError::Illegal(
                    "only mana abilities may be activated during a resolution payment",
                ));
            }
        } else if self.state.priority_player_id() != player {
            return Err(EngineError::Illegal("not your priority"));
        }

        // CR 602.5d / 702.6a: explicit activation instructions and equip both use the shared
        // sorcery-speed window.
        if ability.requires_sorcery_speed()
            && !super::priority::sorcery_speed_available(&self.state, player)
        {
            return Err(EngineError::Illegal("ability only at sorcery speed"));
        }
        if !self.activation_conditions_hold(permanent_id, &ability) {
            return Err(EngineError::Illegal("activation condition not met"));
        }

        if ability.mana_options().is_some() {
            let mut batch = self.resolve_mana_ability(
                player,
                idx,
                permanent_id,
                &card_id,
                &ability,
                mana_option_index,
                targets,
                flex_payments,
                cost_selections,
                restricted_mana,
            )?;
            if resolving_mana_payment {
                batch.events.push(
                    self.resolution_payment_choice_event()
                        .expect("resolution payment remains parked after a mana ability"),
                );
            }
            return Ok(batch);
        }

        self.state.undoable_mana_abilities.clear();

        // CR 608.2: an ability's effect list validates exactly like a spell's — same shape,
        // same multi-target handling — so it goes through the list-level entry point.
        let target_source = TargetSourceIdentity::current(self, permanent_id);
        validate_ability_targets(
            self,
            player,
            target_source,
            &ability.effect,
            ability.targeting.as_ref(),
            targets,
        )?;

        let trefs: Vec<ObjectId> = targets.iter().map(|t| t.object_id).collect();
        // Snapshot target-watchers before costs: the source itself can be sacrificed while paying
        // for the activation. Nothing is staged unless payment succeeds and the ability is pushed.
        let mut target_triggers = self.collect_event_triggers(&[GameEvent::TargetsChosen {
            controller: player,
            source: TargetingSourceKind::Ability,
            targets: trefs.clone(),
        }]);

        let source_zone_change = self
            .state
            .zone_change_generation
            .get(&permanent_id)
            .copied()
            .unwrap_or(0);
        let source_face_change = self
            .state
            .face_change_generation
            .get(&permanent_id)
            .copied()
            .unwrap_or(0);
        let payment = self.pay_ability_costs(
            player,
            idx,
            permanent_id,
            &ability.costs,
            flex_payments,
            cost_selections,
            restricted_mana,
        )?;

        let ability_text = ability.text.clone();
        let card_name = self
            .registry
            .get(&card_id)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| card_id.clone());

        let virtual_id = self.state.next_object_id;
        self.state.next_object_id += 1;

        self.state.stack.push(StackItem {
            id: virtual_id,
            controller: player,
            card_id: card_id.clone(),
            targets: targets
                .iter()
                .map(|target| capture_stack_target(self, target))
                .collect(),
            ability_text: Some(ability_text.clone()),
            source_permanent_id: Some(permanent_id),
            source_zone_change,
            source_face_change,
            ability_index: Some(ability_index),
            triggered_ability: None,
            is_triggered: false,
            is_copy: false,
            chosen_x: 0,
            face_index: face_up_index,
            chosen_modes: vec![],
            // An activated ability's effects act on the player who activated it.
            trigger_player: None,
            trigger_object: None,
            flashback: false,
        });
        self.state.passes_since_stack_change = 0;
        self.state.priority_idx = idx;

        let tgt_line = format_spell_targets_log(&self.state, self.registry, &trefs);
        let paid_costs_line = format_paid_card_costs_log(&payment.paid_card_costs);
        let mut batch = RuledEventBatch::default();
        batch.events.push(ev_log(format!(
            "P{player} activates {card_name}{paid_costs_line}: {ability_text}{tgt_line}"
        )));
        if payment.life_paid > 0 {
            batch.events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                    player_id: player,
                    new_total: self.state.players[idx].life,
                    delta: -(payment.life_paid as i32),
                })),
            });
            batch.events.push(ev_log(format!(
                "P{player} pays {} life (Phyrexian mana).",
                payment.life_paid
            )));
        }
        for ev in payment.move_events {
            batch.events.push(ev);
        }
        batch.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::StackPushed(rv1::StackPushed {
                object_id: virtual_id,
                description: card_name,
                targets: targets.to_vec(),
                ability_annotation: ability_text,
                card_id: String::new(),
                is_copy: false,
                is_triggered: false,
                copy_source_object_id: 0,
                chosen_mode_indices: vec![],
                chosen_mode_labels: vec![],
            })),
        });
        // CR 603.3b: a trigger that fired while the cost was being paid goes on the stack *above*
        // the ability it paid for, so its events must follow that ability's StackPushed. Emitting
        // the trigger prompt first also made the client discard it, because an activated ability
        // reaching the stack is its signal that a pending trigger target was just answered.
        target_triggers.extend(self.collect_committed_sacrifice_cost_dies(payment.sacrificed));
        self.stage_triggers(target_triggers);
        batch.events.push(ev_priority_changed(self));
        fill_legal(&mut batch, self);
        Ok(batch)
    }

    #[allow(clippy::too_many_arguments)]
    fn pay_ability_costs(
        &mut self,
        player: PlayerId,
        idx: usize,
        permanent_id: ObjectId,
        costs: &[AbilityCost],
        flex_payments: &[rv1::FlexPipPayment],
        selections: &[rv1::CostSelection],
        restricted_mana: &[rv1::ManaSpendSelection],
    ) -> Result<AbilityCostPayment, EngineError> {
        use rv1::cost_selection::Selection;

        let source_card_id = self
            .state
            .objects
            .get(&permanent_id)
            .map(|object| object.card_id.as_str())
            .ok_or(EngineError::Illegal("permanent missing"))?;
        let mut by_index = HashMap::new();
        for selection in selections {
            let cost_index = selection.cost_index as usize;
            if cost_index >= costs.len() || by_index.insert(cost_index, selection).is_some() {
                return Err(EngineError::Illegal("invalid or duplicate cost selection"));
            }
        }

        let mut validated = Vec::with_capacity(costs.len());
        let mut consumed = HashSet::new();
        let mut expected_selections = 0usize;
        let mut saw_mana = false;
        let mut saw_tap = false;
        let eligible_restricted_mana = self.eligible_restricted_mana_for_ability(idx, permanent_id);
        for (cost_index, cost) in costs.iter().enumerate() {
            match cost {
                AbilityCost::Tap => {
                    if saw_tap {
                        return Err(EngineError::Illegal("duplicate tap cost"));
                    }
                    self.check_tappable(permanent_id, source_card_id)?;
                    saw_tap = true;
                    validated.push(ValidatedAbilityCost::Tap);
                }
                AbilityCost::Mana(cost) => {
                    if saw_mana {
                        return Err(EngineError::Illegal("multiple mana cost components"));
                    }
                    saw_mana = true;
                    validated.push(ValidatedAbilityCost::Mana(
                        plan_mana_payment_with_restricted_reduction(
                            &self.state,
                            idx,
                            cost,
                            0,
                            0,
                            0,
                            flex_payments,
                            restricted_mana,
                            &eligible_restricted_mana,
                        )?,
                    ));
                }
                AbilityCost::Discard => {
                    expected_selections += 1;
                    let Some(selection) = by_index.get(&cost_index) else {
                        return Err(EngineError::Illegal("missing discard cost selection"));
                    };
                    let Some(Selection::HandIndex(hand_index)) = selection.selection else {
                        return Err(EngineError::Illegal("discard cost requires a hand card"));
                    };
                    let oid = self.state.players[idx]
                        .hand
                        .get(hand_index as usize)
                        .copied()
                        .ok_or(EngineError::Illegal("invalid discard hand slot"))?;
                    if !consumed.insert(oid) {
                        return Err(EngineError::Illegal("one object cannot pay two costs"));
                    }
                    validated.push(ValidatedAbilityCost::Discard(oid));
                }
                AbilityCost::SacrificeSelf => {
                    if !consumed.insert(permanent_id) {
                        return Err(EngineError::Illegal("one object cannot pay two costs"));
                    }
                    validated.push(ValidatedAbilityCost::Sacrifice(permanent_id));
                }
                AbilityCost::SacrificePermanent { filter } => {
                    expected_selections += 1;
                    let Some(selection) = by_index.get(&cost_index) else {
                        return Err(EngineError::Illegal("missing sacrifice cost selection"));
                    };
                    let Some(Selection::PermanentId(oid)) = selection.selection else {
                        return Err(EngineError::Illegal(
                            "sacrifice cost requires a battlefield permanent",
                        ));
                    };
                    if !self.ability_cost_permanent_matches(player, oid, filter) {
                        return Err(EngineError::Illegal("illegal sacrifice cost selection"));
                    }
                    if !consumed.insert(oid) {
                        return Err(EngineError::Illegal("one object cannot pay two costs"));
                    }
                    validated.push(ValidatedAbilityCost::Sacrifice(oid));
                }
            }
        }
        if selections.len() != expected_selections {
            return Err(EngineError::Illegal("unexpected cost selection"));
        }
        if !restricted_mana.is_empty() && !saw_mana {
            return Err(EngineError::Illegal(
                "restricted mana supplied for an ability with no mana cost",
            ));
        }

        let mut payment = AbilityCostPayment {
            move_events: vec![],
            sacrificed: vec![],
            paid_card_costs: vec![],
            life_paid: 0,
        };
        for component in validated {
            match component {
                ValidatedAbilityCost::Tap => {
                    super::set_tapped(&mut self.state, permanent_id, true);
                }
                ValidatedAbilityCost::Mana(plan) => {
                    payment.life_paid += plan.life_cost;
                    commit_mana_payment(&mut self.state, idx, plan);
                }
                ValidatedAbilityCost::Discard(oid) => {
                    let card_name = object_display_name(&self.state, self.registry, oid);
                    let owner = self
                        .state
                        .objects
                        .get(&oid)
                        .map(|object| object.owner)
                        .unwrap_or(player);
                    super::resolution::move_object_to_zone(
                        &mut self.state,
                        self.registry,
                        oid,
                        Zone::Graveyard,
                        None,
                    )?;
                    payment.move_events.push(permanent_moved_event(
                        &self.state,
                        oid,
                        owner,
                        rv1::permanent_moved::Destination::Graveyard,
                    ));
                    payment
                        .paid_card_costs
                        .push(PaidCardCost::Discard(card_name));
                }
                ValidatedAbilityCost::Sacrifice(oid) => {
                    let card_name = object_display_name(&self.state, self.registry, oid);
                    let owner = self
                        .state
                        .objects
                        .get(&oid)
                        .map(|object| object.owner)
                        .unwrap_or(player);
                    payment.sacrificed.push(
                        self.sacrifice_snapshot(oid)
                            .ok_or(EngineError::Illegal("sacrifice permanent missing"))?,
                    );
                    sacrifice_permanent(&mut self.state, self.registry, oid)?;
                    payment.move_events.push(permanent_moved_event(
                        &self.state,
                        oid,
                        owner,
                        rv1::permanent_moved::Destination::Graveyard,
                    ));
                    payment
                        .paid_card_costs
                        .push(PaidCardCost::Sacrifice(card_name));
                }
            }
        }
        Ok(payment)
    }

    pub(super) fn ability_cost_permanent_matches(
        &self,
        player: PlayerId,
        oid: ObjectId,
        filter: &TargetFilter,
    ) -> bool {
        let Some(object) = self.state.objects.get(&oid) else {
            return false;
        };
        if object.zone != Zone::Battlefield || object.controller != player {
            return false;
        }
        let Some(characteristics) = self.characteristics(oid) else {
            return false;
        };
        let kind_matches = match filter.kind {
            TargetKind::Creature => characteristics.is_creature(),
            TargetKind::AnyPermanent => true,
            _ => false,
        };
        kind_matches && super::targeting::filter_characteristics_match(self, filter, oid)
    }

    /// Snapshot a permanent about to be sacrificed as an activation cost. Taken *before* the cost
    /// is paid, because CR 603.6 reads the dying object's last-known information and the object is
    /// already in the graveyard (controller reset, characteristics gone) by the time it fires.
    fn sacrifice_snapshot(&self, permanent_id: ObjectId) -> Option<SacrificeSnapshot> {
        let source = self.trigger_source_snapshot(permanent_id)?;
        Some(SacrificeSnapshot {
            source,
            was_creature: self
                .characteristics(permanent_id)
                .is_some_and(|value| value.is_creature()),
        })
    }

    /// CR 603.6a: a permanent sacrificed to pay an activation cost still dies, so leaves-the-
    /// battlefield abilities (Blood Artist, Bottle Gnomes' own controller triggers) see it. The
    /// triggers go on the stack *above* the ability whose cost they paid, so this runs after the
    /// ability has been pushed.
    fn collect_committed_sacrifice_cost_dies(
        &mut self,
        snapshots: Vec<SacrificeSnapshot>,
    ) -> Vec<super::triggers::CollectedTrigger> {
        let events: Vec<_> = snapshots
            .into_iter()
            .map(|snapshot| GameEvent::Dies {
                source: snapshot.source,
                was_creature: snapshot.was_creature,
            })
            .collect();
        self.record_committed_events(&events);
        self.collect_event_triggers(&events)
    }

    fn fire_sacrifice_cost_dies(&mut self, snapshots: Vec<SacrificeSnapshot>) {
        let collected = self.collect_committed_sacrifice_cost_dies(snapshots);
        self.stage_triggers(collected);
    }

    /// Whether `ability` on `permanent_id` could be activated right now, so the client can grey it
    /// out instead of opening a menu and collecting mana for an activation the engine will reject.
    ///
    /// Deliberately ignores mana: the client floats mana *after* choosing the ability, so an
    /// unaffordable ability is still a legal thing to start. What it does mirror is every gate in
    /// [`GameEngine::activate_ability`] that no amount of mana can satisfy — the tap cost
    /// (CR 302.6 summoning sickness, already tapped), public activation conditions, and stricter
    /// timing instructions (CR 602.5d / 702.6a). Priority and seat ownership stay client-side;
    /// they are not properties of the permanent, and this value is broadcast to every player.
    pub(super) fn ability_activatable(
        &self,
        permanent_id: ObjectId,
        ability: &tricerules_cards::ActivatedAbilityDef,
    ) -> bool {
        let Some(object) = self.state.objects.get(&permanent_id) else {
            return false;
        };
        if ability.requires_sorcery_speed()
            && !super::priority::sorcery_speed_available(&self.state, object.controller)
        {
            return false;
        }
        if !self.activation_conditions_hold(permanent_id, ability) {
            return false;
        }
        if ability
            .costs
            .iter()
            .any(|cost| matches!(cost, AbilityCost::Tap))
            && self.check_tappable(permanent_id, &object.card_id).is_err()
        {
            return false;
        }
        true
    }

    fn activation_conditions_hold(
        &self,
        permanent_id: ObjectId,
        ability: &tricerules_cards::ActivatedAbilityDef,
    ) -> bool {
        let Some(controller) = self
            .state
            .objects
            .get(&permanent_id)
            .map(|object| object.controller)
        else {
            return false;
        };
        ability.conditions.iter().all(|condition| match condition {
            tricerules_cards::ActivationCondition::GameCondition(condition) => self
                .condition_holds(
                    condition,
                    ConditionContext {
                        controller,
                        source_object_id: permanent_id,
                        source_zone_change: self
                            .state
                            .zone_change_generation
                            .get(&permanent_id)
                            .copied()
                            .unwrap_or(0),
                    },
                ),
            tricerules_cards::ActivationCondition::BattlefieldCreatureCount {
                filter,
                min,
                max,
            } => {
                let count = self.battlefield_creature_count(filter, controller, permanent_id);
                min.is_none_or(|minimum| count >= minimum)
                    && max.is_none_or(|maximum| count <= maximum)
            }
        })
    }

    pub(super) fn check_tappable(
        &self,
        permanent_id: ObjectId,
        _card_id: &str,
    ) -> Result<(), EngineError> {
        let has_haste = self
            .characteristics(permanent_id)
            .is_some_and(|value| value.has_keyword(tricerules_cards::Keyword::Haste));
        let o = self
            .state
            .objects
            .get(&permanent_id)
            .ok_or(EngineError::Illegal("permanent missing"))?;
        if o.tapped {
            return Err(EngineError::Illegal("permanent is already tapped"));
        }
        let is_creature = self
            .characteristics(permanent_id)
            .is_some_and(|value| value.is_creature());
        if is_creature && o.summoning_sick && !has_haste {
            return Err(EngineError::Illegal(
                "cannot use tap ability due to summoning sickness",
            ));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn resolve_mana_ability(
        &mut self,
        player: PlayerId,
        idx: usize,
        permanent_id: ObjectId,
        card_id: &str,
        ability: &tricerules_cards::ActivatedAbilityDef,
        mana_option_index: u32,
        targets: &[rv1::TargetRef],
        flex_payments: &[rv1::FlexPipPayment],
        cost_selections: &[rv1::CostSelection],
        restricted_mana: &[rv1::ManaSpendSelection],
    ) -> Result<RuledEventBatch, EngineError> {
        if !targets.is_empty() {
            return Err(EngineError::Illegal("mana ability takes no targets"));
        }
        let Some(options) = self.active_mana_options(permanent_id, ability) else {
            return Err(EngineError::Illegal("not a mana ability"));
        };
        let amount = options
            .get(mana_option_index as usize)
            .copied()
            .ok_or(EngineError::Illegal("invalid mana option"))?;

        let payment = self.pay_ability_costs(
            player,
            idx,
            permanent_id,
            &ability.costs,
            flex_payments,
            cost_selections,
            restricted_mana,
        )?;

        let restriction_group_id = ability.mana_restriction().map(|restriction| {
            if let Some(position) = self
                .state
                .mana_restrictions
                .iter()
                .position(|candidate| candidate == restriction)
            {
                (position + 1) as u32
            } else {
                self.state.mana_restrictions.push(restriction.clone());
                self.state.mana_restrictions.len() as u32
            }
        });
        if let Some(group_id) = restriction_group_id {
            self.state.players[idx].restricted_mana.push(
                crate::state::RestrictedManaContribution {
                    restriction_group_id: group_id,
                    amount,
                },
            );
        } else {
            let pool = &mut self.state.players[idx].mana_pool;
            pool.white += amount.w;
            pool.blue += amount.u;
            pool.black += amount.b;
            pool.red += amount.r;
            pool.green += amount.g;
            pool.colorless += amount.c;
        }

        if matches!(ability.costs.as_slice(), [AbilityCost::Tap]) {
            self.state
                .undoable_mana_abilities
                .push(UndoableManaAbility {
                    player,
                    source: permanent_id,
                    produced: amount,
                    restriction_group_id,
                });
        }

        let card_name = self
            .registry
            .get(card_id)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| card_id.to_string());
        let ability_text = ability.text.clone();
        let paid_costs_line = format_paid_card_costs_log(&payment.paid_card_costs);

        let mut batch = RuledEventBatch::default();
        batch.events.push(ev_log(format!(
            "P{player} activates {card_name}{paid_costs_line}: {ability_text}"
        )));
        for ev in payment.move_events {
            batch.events.push(ev);
        }
        if payment.life_paid > 0 {
            batch.events.push(rv1::RuledEvent {
                ev: Some(rv1::ruled_event::Ev::LifeChanged(rv1::LifeChanged {
                    player_id: player,
                    new_total: self.state.players[idx].life,
                    delta: -(payment.life_paid as i32),
                })),
            });
            batch.events.push(ev_log(format!(
                "P{player} pays {} life (Phyrexian mana).",
                payment.life_paid
            )));
        }
        // A mana ability does not use the stack (CR 605.3a), so a permanent sacrificed to pay for
        // one dies immediately rather than under a pushed ability.
        self.fire_sacrifice_cost_dies(payment.sacrificed);
        Ok(batch)
    }

    pub(super) fn undo_mana_ability(
        &mut self,
        player: PlayerId,
    ) -> Result<RuledEventBatch, EngineError> {
        let payment_undo_start = self.state.pending_resolution.as_ref().and_then(|pending| {
            (pending.deciding_player == player)
                .then_some(pending.mana_payment.as_ref()?.undo_history_start)
        });
        if self.state.priority_player_id() != player && payment_undo_start.is_none() {
            return Err(EngineError::Illegal("not your priority"));
        }
        let event =
            self.rewind_last_undoable_mana_ability(player, payment_undo_start.unwrap_or(0))?;

        let mut batch = RuledEventBatch::default();
        batch.events.push(event);
        if payment_undo_start.is_some() {
            batch.events.push(
                self.resolution_payment_choice_event()
                    .expect("resolution payment remains parked after undo"),
            );
        }
        Ok(batch)
    }

    pub(super) fn rewind_last_undoable_mana_ability(
        &mut self,
        player: PlayerId,
        first_allowed_index: usize,
    ) -> Result<rv1::RuledEvent, EngineError> {
        let idx = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;
        let pos = self
            .state
            .undoable_mana_abilities
            .iter()
            .enumerate()
            .rfind(|(index, entry)| *index >= first_allowed_index && entry.player == player)
            .map(|(index, _)| index)
            .ok_or(EngineError::Illegal("no mana ability to undo"))?;
        let entry = self.state.undoable_mana_abilities.remove(pos);

        let p = &entry.produced;
        if let Some(group_id) = entry.restriction_group_id {
            let contribution_pos = self.state.players[idx]
                .restricted_mana
                .iter()
                .rposition(|contribution| {
                    contribution.restriction_group_id == group_id && contribution.amount == *p
                })
                .ok_or(EngineError::Illegal(
                    "floated restricted mana already spent",
                ))?;
            self.state.players[idx]
                .restricted_mana
                .remove(contribution_pos);
        } else {
            let pool = &mut self.state.players[idx].mana_pool;
            if pool.white < p.w
                || pool.blue < p.u
                || pool.black < p.b
                || pool.red < p.r
                || pool.green < p.g
                || pool.colorless < p.c
            {
                return Err(EngineError::Illegal("floated mana already spent"));
            }
            pool.white -= p.w;
            pool.blue -= p.u;
            pool.black -= p.b;
            pool.red -= p.r;
            pool.green -= p.g;
            pool.colorless -= p.c;
        }

        super::set_tapped(&mut self.state, entry.source, false);

        let card_name = self
            .state
            .objects
            .get(&entry.source)
            .and_then(|o| self.registry.get(&o.card_id))
            .map(|d| d.name.clone())
            .unwrap_or_else(|| "permanent".to_string());
        Ok(ev_log(format!(
            "P{player} undoes mana ability: {card_name}"
        )))
    }

    pub(super) fn play_land(
        &mut self,
        player: PlayerId,
        hand_idx: usize,
        face_index: usize,
    ) -> Result<RuledEventBatch, EngineError> {
        if self.state.priority_player_id() != player {
            return Err(EngineError::Illegal("not your priority"));
        }
        let max_lands = 1 + self.extra_land_plays_for(player);
        if self.state.lands_played_this_turn >= max_lands {
            return Err(EngineError::Illegal(
                "land play limit reached for this turn",
            ));
        }
        if !super::priority::sorcery_speed_available(&self.state, player) {
            return Err(EngineError::Illegal("play land only at sorcery speed"));
        }
        let idx = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;
        let oid = *self.state.players[idx]
            .hand
            .get(hand_idx)
            .ok_or(EngineError::Illegal("bad hand index"))?;
        let card_id = self.state.objects.get(&oid).unwrap().card_id.clone();
        let def = self
            .registry
            .get(&card_id)
            .ok_or_else(|| EngineError::MissingCard(card_id.clone()))?;
        // CR 712: for MDFC lands, check the specific face; for normal lands, check the flat flag.
        let face = def
            .face(face_index)
            .ok_or(EngineError::Illegal("bad face index"))?;
        if !def.face_available_from_hand(face_index) {
            return Err(EngineError::Illegal("face cannot be played from hand"));
        }
        if !face.is_land {
            return Err(EngineError::Illegal("not a land"));
        }
        let land_name = if def.is_multiface() {
            face.name.to_string()
        } else {
            def.name.clone()
        };
        self.state.lands_played_this_turn += 1;
        let mut batch = RuledEventBatch::default();
        let item = StackItem {
            id: oid,
            controller: player,
            card_id,
            targets: Vec::new(),
            ability_text: Some("land play".to_string()),
            source_permanent_id: None,
            source_zone_change: self
                .state
                .zone_change_generation
                .get(&oid)
                .copied()
                .unwrap_or(0),
            source_face_change: 0,
            ability_index: None,
            triggered_ability: None,
            is_triggered: false,
            is_copy: false,
            face_index,
            flashback: false,
            chosen_x: 0,
            chosen_modes: Vec::new(),
            trigger_player: None,
            trigger_object: None,
        };
        match self.begin_battlefield_entry(
            item,
            BattlefieldEntryEvent {
                object_id: oid,
                deciding_player: player,
                destination_controller: player,
                face_index,
                chosen_x: 0,
                tapped: false,
                entry_counters: BTreeMap::new(),
                applied_effects: Vec::new(),
            },
            BattlefieldEntryCompletion::LandPlay {
                player,
                land_name: land_name.clone(),
            },
            &mut batch.events,
        ) {
            super::replacement::BattlefieldEntryProgress::Parked => return Ok(batch),
            super::replacement::BattlefieldEntryProgress::Ready(entry) => {
                self.commit_battlefield_entry(entry, None)?;
            }
        }
        self.state.passes_since_stack_change = 0;
        batch
            .events
            .push(ev_log(format!("P{player} played {land_name}")));
        fill_legal(&mut batch, self);
        Ok(batch)
    }
}

#[cfg(test)]
mod mana_payment_tests {
    use super::*;

    /// Build a 2-player engine and hand priority to player 0 so `pay_mana`'s priority gate passes.
    fn engine_with_priority() -> GameEngine {
        let mut e = GameEngine::new_with_default_decks(1, &[0, 1], 20).expect("new");
        e.state.priority_idx = 0;
        e
    }

    #[test]
    fn paid_card_cost_log_formats_any_number_of_components() {
        assert_eq!(format_paid_card_costs_log(&[]), "");
        assert_eq!(
            format_paid_card_costs_log(&[PaidCardCost::Discard("Mountain".into())]),
            " discarding Mountain"
        );
        assert_eq!(
            format_paid_card_costs_log(&[
                PaidCardCost::Discard("Mountain".into()),
                PaidCardCost::Sacrifice("Grizzly Bears".into()),
            ]),
            " discarding Mountain and sacrificing Grizzly Bears"
        );
        assert_eq!(
            format_paid_card_costs_log(&[
                PaidCardCost::Discard("Mountain".into()),
                PaidCardCost::Sacrifice("Grizzly Bears".into()),
                PaidCardCost::Sacrifice("Hill Giant".into()),
            ]),
            " discarding Mountain, sacrificing Grizzly Bears, and sacrificing Hill Giant"
        );
    }

    #[test]
    fn one_permanent_cannot_pay_two_consuming_cost_components() {
        use rv1::cost_selection::Selection;

        let mut e = engine_with_priority();
        let oid = e.state.players[0]
            .library
            .pop_front()
            .expect("default deck card");
        e.state.players[0].battlefield.push(oid);
        let object = e.state.objects.get_mut(&oid).expect("object");
        object.zone = Zone::Battlefield;
        object.base_controller = 0;
        object.controller = 0;
        let filter = TargetFilter {
            kind: TargetKind::AnyPermanent,
            controller: TargetController::You,
            ..TargetFilter::default()
        };
        let costs = [
            AbilityCost::SacrificePermanent {
                filter: filter.clone(),
            },
            AbilityCost::SacrificePermanent { filter },
        ];
        let selections = [
            rv1::CostSelection {
                cost_index: 0,
                selection: Some(Selection::PermanentId(oid)),
            },
            rv1::CostSelection {
                cost_index: 1,
                selection: Some(Selection::PermanentId(oid)),
            },
        ];

        let err = e
            .pay_ability_costs(0, 0, oid, &costs, &[], &selections, &[])
            .err()
            .expect("one object cannot be sacrificed twice");
        assert!(format!("{err:?}").contains("one object cannot pay two costs"));
        assert_eq!(e.state.objects[&oid].zone, Zone::Battlefield);
    }

    #[test]
    fn multi_digit_generic_paid_from_pool() {
        // Regression for the old per-char parser: "{15}" must consume 15, not 6.
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.colorless = 15;
        let cost = ManaCost {
            pips: vec![ManaSymbol::Generic(15)],
        };
        assert!(pay_mana(&mut e.state, 0, &cost, 0, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.colorless, 0);
    }

    #[test]
    fn insufficient_generic_mana_rejected() {
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.colorless = 14;
        let cost = ManaCost {
            pips: vec![ManaSymbol::Generic(15)],
        };
        assert!(matches!(
            pay_mana(&mut e.state, 0, &cost, 0, 0, &[]),
            Err(EngineError::Illegal(_))
        ));
    }

    #[test]
    fn generic_reduction_applies_after_increases_and_floors_without_touching_colored_pips() {
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.blue = 1;
        e.state.players[0].mana_pool.colorless = 4;
        let cost = ManaCost::parse("{1}{U}").expect("cost");

        let plan = plan_mana_payment_with_reduction(&e.state, 0, &cost, 0, 2, 3, &[])
            .expect("the reduction applies after the increase");
        commit_mana_payment(&mut e.state, 0, plan);
        assert_eq!(e.state.players[0].mana_pool.blue, 0);
        assert_eq!(e.state.players[0].mana_pool.colorless, 4);

        e.state.players[0].mana_pool.blue = 1;
        let colored_only = ManaCost::parse("{U}").expect("cost");
        let plan =
            plan_mana_payment_with_reduction(&e.state, 0, &colored_only, 0, 0, u32::MAX, &[])
                .expect("an oversized generic reduction floors at zero");
        commit_mana_payment(&mut e.state, 0, plan);
        assert_eq!(e.state.players[0].mana_pool.blue, 0);
        assert_eq!(e.state.players[0].mana_pool.colorless, 4);
    }

    #[test]
    fn reduced_mana_and_discard_cost_are_planned_and_committed_together() {
        use rv1::cost_selection::Selection;

        let mut e = engine_with_priority();
        let source_oid = e.state.players[0].hand[0];
        let discarded_oid = e.state.players[0].hand[1];
        e.state.players[0].mana_pool.blue = 1;
        let costs = [AdditionalCost::DiscardCard];
        let selections = [rv1::CostSelection {
            cost_index: 0,
            selection: Some(Selection::HandIndex(1)),
        }];

        let plan = e
            .plan_spell_costs(
                0,
                0,
                source_oid,
                &ManaCost::parse("{1}{U}").expect("cost"),
                0,
                0,
                1,
                &[],
                &costs,
                &selections,
                &[],
                &[],
            )
            .expect("reduced mana and discard cost should validate together");
        let payment = e
            .commit_spell_costs(0, 0, plan)
            .expect("validated costs should commit");

        assert_eq!(e.state.players[0].mana_pool.blue, 0);
        assert_eq!(e.state.objects[&source_oid].zone, Zone::Hand);
        assert_eq!(e.state.objects[&discarded_oid].zone, Zone::Graveyard);
        assert_eq!(payment.paid_card_costs.len(), 1);
    }

    #[test]
    fn colorless_pip_requires_colorless_mana() {
        // {C} cannot be paid with colored mana (unlike generic).
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.red = 5;
        let cost = ManaCost {
            pips: vec![ManaSymbol::C],
        };
        assert!(matches!(
            pay_mana(&mut e.state, 0, &cost, 0, 0, &[]),
            Err(EngineError::Illegal(_))
        ));
        e.state.players[0].mana_pool.colorless = 1;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, 0, &[]).is_ok());
    }

    #[test]
    fn x_cost_pays_chosen_value_as_generic() {
        // CR 107.3b: {X}{R} with X=4 needs 4 generic + 1 red.
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.colorless = 4;
        e.state.players[0].mana_pool.red = 1;
        let cost = ManaCost {
            pips: vec![ManaSymbol::X, ManaSymbol::R],
        };
        assert!(pay_mana(&mut e.state, 0, &cost, 4, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.colorless, 0);
        assert_eq!(e.state.players[0].mana_pool.red, 0);
    }

    #[test]
    fn x_zero_pays_only_fixed_pips() {
        // CR 107.3: X=0 is a legal choice; only the {R} must be paid.
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.red = 1;
        let cost = ManaCost {
            pips: vec![ManaSymbol::X, ManaSymbol::R],
        };
        assert!(pay_mana(&mut e.state, 0, &cost, 0, 0, &[]).is_ok());
    }

    #[test]
    fn insufficient_mana_for_chosen_x_rejected() {
        // Choosing X larger than the pool can cover fails cleanly (no partial payment).
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.colorless = 3;
        e.state.players[0].mana_pool.red = 1;
        let cost = ManaCost {
            pips: vec![ManaSymbol::X, ManaSymbol::R],
        };
        assert!(matches!(
            pay_mana(&mut e.state, 0, &cost, 4, 0, &[]),
            Err(EngineError::Illegal(_))
        ));
    }

    #[test]
    fn hybrid_pip_paid_by_either_color() {
        // {G/U} payable by green alone or by blue alone (CR 107.4d).
        let cost = ManaCost {
            pips: vec![ManaSymbol::Hybrid(ColorPip::G, ColorPip::U)],
        };
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.green = 1;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.green, 0);

        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.blue = 1;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.blue, 0);
    }

    #[test]
    fn hybrid_solve_avoids_greedy_dead_end() {
        // {G/U}{G} with one G + one U: the {G/U} must take U so the {G} can take G (CR 107.4).
        let cost = ManaCost {
            pips: vec![ManaSymbol::Hybrid(ColorPip::G, ColorPip::U), ManaSymbol::G],
        };
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.green = 1;
        e.state.players[0].mana_pool.blue = 1;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.green, 0);
        assert_eq!(e.state.players[0].mana_pool.blue, 0);
    }

    #[test]
    fn mono_hybrid_paid_by_generic_when_color_absent() {
        // {2/W} payable by two generic when no white is available (CR 107.4e).
        let cost = ManaCost {
            pips: vec![ManaSymbol::MonoHybrid(2, ColorPip::W)],
        };
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.red = 2;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.red, 0);

        // ...or by one white (the cheaper alternative is preferred, leaving generic mana spare).
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.white = 1;
        e.state.players[0].mana_pool.red = 2;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.white, 0);
        assert_eq!(e.state.players[0].mana_pool.red, 2);
    }

    #[test]
    fn mono_hybrid_rejected_with_one_generic() {
        // One spare mana can't cover the {2/...} generic alternative.
        let cost = ManaCost {
            pips: vec![ManaSymbol::MonoHybrid(2, ColorPip::W)],
        };
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.red = 1;
        assert!(matches!(
            pay_mana(&mut e.state, 0, &cost, 0, 0, &[]),
            Err(EngineError::Illegal(_))
        ));
    }

    #[test]
    fn phyrexian_paid_by_mana() {
        // {B/P} with no life flag must be paid by black mana (CR 107.4f).
        let cost = ManaCost {
            pips: vec![ManaSymbol::Phyrexian(ColorPip::B)],
        };
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.black = 1;
        let life = e.state.players[0].life;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.black, 0);
        assert_eq!(e.state.players[0].life, life); // no life paid
    }

    #[test]
    fn phyrexian_paid_by_life() {
        // {B/P} with pay_life on pip 0 deducts 2 life and touches no mana (CR 107.4f).
        let cost = ManaCost {
            pips: vec![ManaSymbol::Phyrexian(ColorPip::B)],
        };
        let mut e = engine_with_priority();
        let life = e.state.players[0].life;
        let flex = [rv1::FlexPipPayment {
            pip_index: 0,
            pay_life: true,
        }];
        assert!(pay_mana(&mut e.state, 0, &cost, 0, 0, &flex).is_ok());
        assert_eq!(e.state.players[0].life, life - 2);
    }

    #[test]
    fn phyrexian_life_rejected_when_insufficient_life() {
        // CR 119.4: can't pay 2 life with only 1 (and no mana to fall back on).
        let cost = ManaCost {
            pips: vec![ManaSymbol::Phyrexian(ColorPip::B)],
        };
        let mut e = engine_with_priority();
        e.state.players[0].life = 1;
        let flex = [rv1::FlexPipPayment {
            pip_index: 0,
            pay_life: true,
        }];
        assert!(matches!(
            pay_mana(&mut e.state, 0, &cost, 0, 0, &flex),
            Err(EngineError::Illegal(_))
        ));
        assert_eq!(e.state.players[0].life, 1); // unchanged on failure
    }
}
