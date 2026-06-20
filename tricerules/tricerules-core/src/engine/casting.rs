use super::*;
use super::combat::priority_locked_for_combat_declaration;
use super::events::{ev_log, ev_priority_changed, format_spell_targets_log};
use super::legal_actions::fill_legal;
use super::resolution::{permanent_moved_event, sacrifice_permanent};
use super::targeting::{validate_effect_targets, validate_spell_targets};

/// CR 702.8b: true if the card face is castable at instant speed (is an instant, or has flash).
pub(super) fn castable_at_instant_speed(face: &tricerules_cards::CardFace) -> bool {
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
pub(super) fn solve_flex(pool: PoolVec, flex: &[FlexPip], idx: usize, generic: u32) -> Option<PoolVec> {
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

/// Pays `cost` and returns the amount of life spent on Phyrexian pips (CR 107.4f).
pub(super) fn pay_mana(
    state: &mut GameState,
    player_idx: usize,
    cost: &ManaCost,
    x_value: u32,
    flex_payments: &[rv1::FlexPipPayment],
) -> Result<u32, EngineError> {
    if player_idx != state.priority_idx {
        return Err(EngineError::Illegal(
            "only priority player can pay mana for spells",
        ));
    }
    let life_pips: HashSet<usize> = flex_payments
        .iter()
        .filter(|fp| fp.pay_life)
        .map(|fp| fp.pip_index as usize)
        .collect();

    let mut need_color: PoolVec = [0; 6];
    let mut need_generic = 0u32;
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
            ManaSymbol::Generic(n) => need_generic += n,
            ManaSymbol::X => need_generic += x_value,
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

    // CR 119.4: a player can pay life only if they have at least that much.
    if life_cost > 0 && state.players[player_idx].life < life_cost as i32 {
        return Err(EngineError::Illegal(
            "not enough life to pay Phyrexian cost",
        ));
    }

    let pool = &state.players[player_idx].mana_pool;
    let mut working: PoolVec = [
        pool.white,
        pool.blue,
        pool.black,
        pool.red,
        pool.green,
        pool.colorless,
    ];
    for i in 0..6 {
        if working[i] < need_color[i] {
            return Err(EngineError::Illegal(
                "not enough mana in pool; tap your lands first",
            ));
        }
        working[i] -= need_color[i];
    }
    let Some(remaining) = solve_flex(working, &flex, 0, need_generic) else {
        return Err(EngineError::Illegal(
            "not enough mana in pool; tap your lands first",
        ));
    };

    let pool = &mut state.players[player_idx].mana_pool;
    pool.white = remaining[0];
    pool.blue = remaining[1];
    pool.black = remaining[2];
    pool.red = remaining[3];
    pool.green = remaining[4];
    pool.colorless = remaining[POOL_C];
    state.players[player_idx].life -= life_cost as i32;
    Ok(life_cost)
}

impl GameEngine {
    pub(super) fn cast_spell(
        &mut self,
        player: PlayerId,
        hand_idx: usize,
        targets: &[rv1::TargetRef],
        x_value: u32,
        flex_payments: &[rv1::FlexPipPayment],
        face_index: usize,
    ) -> Result<RuledEventBatch, EngineError> {
        if self.state.priority_player_id() != player {
            return Err(EngineError::Illegal("not your priority"));
        }
        if self.state.turn_step == TurnStep::Cleanup {
            return Err(EngineError::Illegal("no spells during cleanup"));
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
        let face = def
            .face(face_index)
            .ok_or(EngineError::Illegal("bad face index"))?;
        if face.is_land {
            return Err(EngineError::Illegal("use play land"));
        }
        let face_is_sorcery = face.is_sorcery;
        let face_instant_speed = castable_at_instant_speed(&face);
        let face_mana = face.mana_cost.clone();
        let face_name = face.name.to_string();
        let is_multiface = def.is_multiface();
        let face_effects: Vec<SpellEffectKind> = face.spell_effect.to_vec();
        let sorcery_ok = super::priority::sorcery_speed_available(&self.state, player);
        let instant_ok = super::priority::instant_timing_step_allowed(self.state.turn_step);
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
        if !self.state.pending_triggers.is_empty() {
            return Err(EngineError::Illegal(
                "must choose trigger target before casting",
            ));
        }
        validate_spell_targets(&self.state, self.registry, player, &face_effects, targets)?;
        let has_x = face_mana.has_x();
        if x_value != 0 && !has_x {
            return Err(EngineError::Illegal("x_value given but cost has no {X}"));
        }
        let chosen_x = if has_x { x_value } else { 0 };
        let life_paid = pay_mana(&mut self.state, idx, &face_mana, chosen_x, flex_payments)?;

        self.state.players[idx].hand.retain(|&x| x != oid);
        let trefs: Vec<ObjectId> = targets.iter().map(|t| t.object_id).collect();
        let tgt_line = format_spell_targets_log(&self.state, self.registry, &trefs);

        self.state.stack.push(StackItem {
            id: oid,
            controller: player,
            card_id: card_id.clone(),
            targets: trefs,
            ability_text: None,
            source_permanent_id: None,
            ability_index: None,
            is_triggered: false,
            is_copy: false,
            chosen_x,
            face_index,
        });
        if let Some(o) = self.state.objects.get_mut(&oid) {
            o.zone = Zone::Stack;
        }

        self.state.passes_since_stack_change = 0;
        self.state.priority_idx = idx;

        let cast_card_id = card_id.clone();
        let mut batch = RuledEventBatch::default();
        let x_line = if has_x {
            format!(" (X={chosen_x})")
        } else {
            String::new()
        };
        batch.events.push(ev_log(format!(
            "P{} casts {}{}{}",
            player, face_name, x_line, tgt_line
        )));
        let stack_annotation = match (is_multiface, has_x) {
            (true, true) => format!("{face_name} (X = {chosen_x})"),
            (true, false) => face_name.clone(),
            (false, true) => format!("X = {chosen_x}"),
            (false, false) => String::new(),
        };
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
        batch.events.push(rv1::RuledEvent {
            ev: Some(rv1::ruled_event::Ev::StackPushed(rv1::StackPushed {
                object_id: oid,
                description: face_name.clone(),
                targets: targets.to_vec(),
                ability_annotation: stack_annotation,
                card_id: cast_card_id.clone(),
                is_copy: false,
            })),
        });
        self.fire_triggers(
            GameEvent::SpellCast {
                caster: player,
                card_id: cast_card_id,
            },
            &mut batch.events,
        );
        batch.events.push(ev_priority_changed(self));
        fill_legal(&mut batch, self);
        Ok(batch)
    }

    pub(super) fn activate_ability(
        &mut self,
        player: PlayerId,
        permanent_id: u32,
        ability_index: usize,
        targets: &[rv1::TargetRef],
        flex_payments: &[rv1::FlexPipPayment],
        mana_option_index: u32,
    ) -> Result<RuledEventBatch, EngineError> {
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

        let card_id = self
            .state
            .objects
            .get(&permanent_id)
            .filter(|o| o.zone == Zone::Battlefield)
            .map(|o| o.card_id.clone())
            .ok_or(EngineError::Illegal("permanent not on battlefield"))?;
        if !self.state.players[idx].battlefield.contains(&permanent_id) {
            return Err(EngineError::Illegal("not your permanent"));
        }

        let def = self
            .registry
            .get(&card_id)
            .ok_or_else(|| EngineError::MissingCard(card_id.clone()))?;

        let ability = def
            .activated_abilities
            .get(ability_index)
            .ok_or(EngineError::Illegal("no such activated ability"))?
            .clone();

        if matches!(ability.effect, SpellEffectKind::ProduceMana { .. }) {
            return self.resolve_mana_ability(
                player,
                idx,
                permanent_id,
                &card_id,
                &ability,
                mana_option_index,
                targets,
                flex_payments,
            );
        }

        self.state.undoable_mana_abilities.clear();

        validate_effect_targets(&self.state, self.registry, player, &ability.effect, targets)?;

        let (sacrifice_ev, life_paid) = self.pay_ability_cost(
            player,
            idx,
            permanent_id,
            &card_id,
            &ability.cost,
            flex_payments,
        )?;

        let trefs: Vec<ObjectId> = targets.iter().map(|t| t.object_id).collect();
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
            targets: trefs.clone(),
            ability_text: Some(ability_text.clone()),
            source_permanent_id: Some(permanent_id),
            ability_index: Some(ability_index),
            is_triggered: false,
            is_copy: false,
            chosen_x: 0,
            face_index: 0,
        });
        self.state.passes_since_stack_change = 0;
        self.state.priority_idx = idx;

        let tgt_line = format_spell_targets_log(&self.state, self.registry, &trefs);
        let mut batch = RuledEventBatch::default();
        batch.events.push(ev_log(format!(
            "P{player} activates {card_name}: {ability_text}{tgt_line}"
        )));
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
        if let Some(ev) = sacrifice_ev {
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
            })),
        });
        batch.events.push(ev_priority_changed(self));
        fill_legal(&mut batch, self);
        Ok(batch)
    }

    pub(super) fn pay_ability_cost(
        &mut self,
        player: PlayerId,
        idx: usize,
        permanent_id: ObjectId,
        card_id: &str,
        cost: &AbilityCost,
        flex_payments: &[rv1::FlexPipPayment],
    ) -> Result<(Option<rv1::RuledEvent>, u32), EngineError> {
        match cost {
            AbilityCost::Tap => {
                self.tap_for_cost(permanent_id, card_id)?;
                Ok((None, 0))
            }
            AbilityCost::Mana(cost) => {
                let life_paid = pay_mana(&mut self.state, idx, cost, 0, flex_payments)?;
                Ok((None, life_paid))
            }
            AbilityCost::TapAndMana(cost) => {
                self.check_tappable(permanent_id, card_id)?;
                let life_paid = pay_mana(&mut self.state, idx, cost, 0, flex_payments)?;
                self.tap_for_cost(permanent_id, card_id)?;
                Ok((None, life_paid))
            }
            AbilityCost::Sacrifice => {
                let owner = self
                    .state
                    .objects
                    .get(&permanent_id)
                    .map(|o| o.owner)
                    .unwrap_or(player);
                sacrifice_permanent(&mut self.state, permanent_id)?;
                Ok((
                    Some(permanent_moved_event(
                        &self.state,
                        permanent_id,
                        owner,
                        rv1::permanent_moved::Destination::Graveyard,
                    )),
                    0,
                ))
            }
        }
    }

    pub(super) fn check_tappable(&self, permanent_id: ObjectId, card_id: &str) -> Result<(), EngineError> {
        let has_haste = self
            .registry
            .get(card_id)
            .map(|d| d.keywords.contains(&tricerules_cards::Keyword::Haste))
            .unwrap_or(false);
        let o = self
            .state
            .objects
            .get(&permanent_id)
            .ok_or(EngineError::Illegal("permanent missing"))?;
        if o.tapped {
            return Err(EngineError::Illegal("permanent is already tapped"));
        }
        if o.summoning_sick && !has_haste {
            return Err(EngineError::Illegal(
                "cannot use tap ability due to summoning sickness",
            ));
        }
        Ok(())
    }

    pub(super) fn tap_for_cost(&mut self, permanent_id: ObjectId, card_id: &str) -> Result<(), EngineError> {
        self.check_tappable(permanent_id, card_id)?;
        if let Some(o) = self.state.objects.get_mut(&permanent_id) {
            o.tapped = true;
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
    ) -> Result<RuledEventBatch, EngineError> {
        if !targets.is_empty() {
            return Err(EngineError::Illegal("mana ability takes no targets"));
        }
        let SpellEffectKind::ProduceMana { options } = &ability.effect else {
            return Err(EngineError::Illegal("not a mana ability"));
        };
        let amount = options
            .get(mana_option_index as usize)
            .ok_or(EngineError::Illegal("invalid mana option"))?;

        let (sacrifice_ev, life_paid) = self.pay_ability_cost(
            player,
            idx,
            permanent_id,
            card_id,
            &ability.cost,
            flex_payments,
        )?;

        let pool = &mut self.state.players[idx].mana_pool;
        pool.white += amount.w;
        pool.blue += amount.u;
        pool.black += amount.b;
        pool.red += amount.r;
        pool.green += amount.g;
        pool.colorless += amount.c;

        if matches!(ability.cost, AbilityCost::Tap) {
            self.state
                .undoable_mana_abilities
                .push(UndoableManaAbility {
                    player,
                    source: permanent_id,
                    produced: *amount,
                });
        }

        let card_name = self
            .registry
            .get(card_id)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| card_id.to_string());
        let ability_text = ability.text.clone();

        let mut batch = RuledEventBatch::default();
        batch.events.push(ev_log(format!(
            "P{player} activates {card_name}: {ability_text}"
        )));
        if let Some(ev) = sacrifice_ev {
            batch.events.push(ev);
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
        Ok(batch)
    }

    pub(super) fn undo_mana_ability(&mut self, player: PlayerId) -> Result<RuledEventBatch, EngineError> {
        let idx = self
            .state
            .player_idx(player)
            .ok_or(EngineError::UnknownPlayer(player))?;
        if self.state.priority_player_id() != player {
            return Err(EngineError::Illegal("not your priority"));
        }
        let pos = self
            .state
            .undoable_mana_abilities
            .iter()
            .rposition(|e| e.player == player)
            .ok_or(EngineError::Illegal("no mana ability to undo"))?;
        let entry = self.state.undoable_mana_abilities.remove(pos);

        let pool = &mut self.state.players[idx].mana_pool;
        let p = &entry.produced;
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

        if let Some(o) = self.state.objects.get_mut(&entry.source) {
            o.tapped = false;
        }

        let card_name = self
            .state
            .objects
            .get(&entry.source)
            .and_then(|o| self.registry.get(&o.card_id))
            .map(|d| d.name.clone())
            .unwrap_or_else(|| "permanent".to_string());
        let mut batch = RuledEventBatch::default();
        batch.events.push(ev_log(format!(
            "P{player} undoes mana ability: {card_name}"
        )));
        Ok(batch)
    }

    pub(super) fn play_land(
        &mut self,
        player: PlayerId,
        hand_idx: usize,
    ) -> Result<RuledEventBatch, EngineError> {
        if self.state.priority_player_id() != player {
            return Err(EngineError::Illegal("not your priority"));
        }
        if self.state.land_dropped_this_turn {
            return Err(EngineError::Illegal("one land per turn"));
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
        let def = self.registry.get(&card_id).unwrap();
        if !def.is_land {
            return Err(EngineError::Illegal("not a land"));
        }
        self.state.land_dropped_this_turn = true;
        self.state.players[idx].hand.retain(|&x| x != oid);
        self.state.players[idx].battlefield.push(oid);
        if let Some(o) = self.state.objects.get_mut(&oid) {
            o.zone = Zone::Battlefield;
        }
        self.state.passes_since_stack_change = 0;
        let mut batch = RuledEventBatch::default();
        let land_name = def.name.clone();
        batch
            .events
            .push(ev_log(format!("P{} played {}", player, land_name)));
        self.fire_triggers(
            GameEvent::EntersBattlefield { object_id: oid },
            &mut batch.events,
        );
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
    fn multi_digit_generic_paid_from_pool() {
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.colorless = 15;
        let cost = ManaCost {
            pips: vec![ManaSymbol::Generic(15)],
        };
        assert!(pay_mana(&mut e.state, 0, &cost, 0, &[]).is_ok());
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
            pay_mana(&mut e.state, 0, &cost, 0, &[]),
            Err(EngineError::Illegal(_))
        ));
    }

    #[test]
    fn colorless_pip_requires_colorless_mana() {
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.red = 5;
        let cost = ManaCost {
            pips: vec![ManaSymbol::C],
        };
        assert!(matches!(
            pay_mana(&mut e.state, 0, &cost, 0, &[]),
            Err(EngineError::Illegal(_))
        ));
        e.state.players[0].mana_pool.colorless = 1;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, &[]).is_ok());
    }

    #[test]
    fn x_cost_pays_chosen_value_as_generic() {
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.colorless = 4;
        e.state.players[0].mana_pool.red = 1;
        let cost = ManaCost {
            pips: vec![ManaSymbol::X, ManaSymbol::R],
        };
        assert!(pay_mana(&mut e.state, 0, &cost, 4, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.colorless, 0);
        assert_eq!(e.state.players[0].mana_pool.red, 0);
    }

    #[test]
    fn x_zero_pays_only_fixed_pips() {
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.red = 1;
        let cost = ManaCost {
            pips: vec![ManaSymbol::X, ManaSymbol::R],
        };
        assert!(pay_mana(&mut e.state, 0, &cost, 0, &[]).is_ok());
    }

    #[test]
    fn insufficient_mana_for_chosen_x_rejected() {
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.colorless = 3;
        e.state.players[0].mana_pool.red = 1;
        let cost = ManaCost {
            pips: vec![ManaSymbol::X, ManaSymbol::R],
        };
        assert!(matches!(
            pay_mana(&mut e.state, 0, &cost, 4, &[]),
            Err(EngineError::Illegal(_))
        ));
    }

    #[test]
    fn hybrid_pip_paid_by_either_color() {
        let cost = ManaCost {
            pips: vec![ManaSymbol::Hybrid(ColorPip::G, ColorPip::U)],
        };
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.green = 1;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.green, 0);

        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.blue = 1;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.blue, 0);
    }

    #[test]
    fn hybrid_solve_avoids_greedy_dead_end() {
        let cost = ManaCost {
            pips: vec![ManaSymbol::Hybrid(ColorPip::G, ColorPip::U), ManaSymbol::G],
        };
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.green = 1;
        e.state.players[0].mana_pool.blue = 1;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.green, 0);
        assert_eq!(e.state.players[0].mana_pool.blue, 0);
    }

    #[test]
    fn mono_hybrid_paid_by_generic_when_color_absent() {
        let cost = ManaCost {
            pips: vec![ManaSymbol::MonoHybrid(2, ColorPip::W)],
        };
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.red = 2;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.red, 0);

        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.white = 1;
        e.state.players[0].mana_pool.red = 2;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.white, 0);
        assert_eq!(e.state.players[0].mana_pool.red, 2);
    }

    #[test]
    fn mono_hybrid_rejected_with_one_generic() {
        let cost = ManaCost {
            pips: vec![ManaSymbol::MonoHybrid(2, ColorPip::W)],
        };
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.red = 1;
        assert!(matches!(
            pay_mana(&mut e.state, 0, &cost, 0, &[]),
            Err(EngineError::Illegal(_))
        ));
    }

    #[test]
    fn phyrexian_paid_by_mana() {
        let cost = ManaCost {
            pips: vec![ManaSymbol::Phyrexian(ColorPip::B)],
        };
        let mut e = engine_with_priority();
        e.state.players[0].mana_pool.black = 1;
        let life = e.state.players[0].life;
        assert!(pay_mana(&mut e.state, 0, &cost, 0, &[]).is_ok());
        assert_eq!(e.state.players[0].mana_pool.black, 0);
        assert_eq!(e.state.players[0].life, life);
    }

    #[test]
    fn phyrexian_paid_by_life() {
        let cost = ManaCost {
            pips: vec![ManaSymbol::Phyrexian(ColorPip::B)],
        };
        let mut e = engine_with_priority();
        let life = e.state.players[0].life;
        let flex = [rv1::FlexPipPayment {
            pip_index: 0,
            pay_life: true,
        }];
        assert!(pay_mana(&mut e.state, 0, &cost, 0, &flex).is_ok());
        assert_eq!(e.state.players[0].life, life - 2);
    }

    #[test]
    fn phyrexian_life_rejected_when_insufficient_life() {
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
            pay_mana(&mut e.state, 0, &cost, 0, &flex),
            Err(EngineError::Illegal(_))
        ));
        assert_eq!(e.state.players[0].life, 1);
    }
}
