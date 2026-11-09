//! Recipient-private payment witnesses and conservative no-action proofs for automation.
//! Highlighting still requires a positive witness; uncertainty never authorizes a pass.
use super::payment::candidates::{
    combinations, cost_selections, phyrexian_choices, product, AbilityInventory,
};
use super::*;
pub(super) mod proofs;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Availability {
    Available,
    Unavailable,
    Unknown,
}

impl Availability {
    fn available(self) -> bool {
        self == Self::Available
    }
}

pub(super) struct Report {
    pub objects: BTreeSet<ObjectId>,
    pub stop: rv1::AutomationStopReason,
}

impl Report {
    fn record(&mut self, oid: ObjectId, result: Availability, reason: rv1::AutomationStopReason) {
        if result.available() {
            self.objects.insert(oid);
            if matches!(
                self.stop,
                rv1::AutomationStopReason::None | rv1::AutomationStopReason::Uncertain
            ) {
                self.stop = reason;
            }
        } else if result == Availability::Unknown && self.stop == rv1::AutomationStopReason::None {
            self.stop = rv1::AutomationStopReason::Uncertain;
        }
    }
}

pub(super) fn affordable_objects(
    engine: &GameEngine,
    player: PlayerId,
    actions: &LegalActions,
    battlefield_abilities: &AbilityInventory,
) -> Report {
    let mut ready = Report {
        objects: BTreeSet::new(),
        stop: rv1::AutomationStopReason::None,
    };
    let Some(idx) = engine.state.player_idx(player) else {
        return ready;
    };
    if engine.state.is_terminal() || engine.state.players[idx].has_lost {
        return ready;
    }
    for action in &actions.hand_actions {
        let Some(&oid) = engine.state.players[idx]
            .hand
            .get(action.hand_index as usize)
        else {
            continue;
        };
        if action.kind == rv1::HandActionKind::HandActionPlayLand as i32 {
            ready.record(
                oid,
                Availability::Available,
                rv1::AutomationStopReason::Land,
            );
        } else if action.kind == rv1::HandActionKind::HandActionCastSpell as i32 {
            let command = rv1::CastSpell {
                source: Some(rv1::CastSource {
                    location: Some(rv1::cast_source::Location::HandIndex(action.hand_index)),
                    ..Default::default()
                }),
                face_index: action.face_index,
                cast_method: action.cast_method,
                prototyped: action.prototyped,
                ..Default::default()
            };
            let result = spell_payable(
                engine,
                player,
                oid,
                command,
                &action.cost,
                action.cost_choices.as_ref(),
                actions
                    .valid_targets_by_hand_slot
                    .get(&((action.hand_index << 8) | action.face_index)),
                &action.modes,
                action.min_modes,
                action.max_modes,
                &action.as_foretold_costs,
                action.base_cost_unpayable,
                battlefield_abilities,
            );
            ready.record(oid, result, rv1::AutomationStopReason::Spell);
        } else {
            ready.record(
                oid,
                Availability::Unknown,
                rv1::AutomationStopReason::SpecialAction,
            );
        }
    }
    for action in &actions.zone_land_actions {
        ready.record(
            action.object_id,
            Availability::Available,
            rv1::AutomationStopReason::Land,
        );
    }
    for action in &actions.permanent_actions {
        let purpose = match rv1::PermanentActionKind::try_from(action.kind) {
            Ok(rv1::PermanentActionKind::TurnFaceUp) => SpecialActionManaPurpose::TurnFaceUp,
            Ok(rv1::PermanentActionKind::UnlockRoomDoor) => {
                SpecialActionManaPurpose::UnlockRoomDoor
            }
            _ => {
                ready.record(
                    action.object_id,
                    Availability::Unknown,
                    rv1::AutomationStopReason::SpecialAction,
                );
                continue;
            }
        };
        let Ok(mana) = ManaCost::parse(&action.mana_cost) else {
            ready.record(
                action.object_id,
                Availability::Unknown,
                rv1::AutomationStopReason::SpecialAction,
            );
            continue;
        };
        let mut result = if proofs::announcements_complete(
            engine,
            &mana,
            &rv1::LegalCostChoices::default(),
            None,
            &[],
        ) {
            Availability::Unavailable
        } else {
            Availability::Unknown
        };
        for flex in phyrexian_choices(&mana) {
            let Ok(cost) =
                engine.prepare_special_action_payment_costs(player, &mana, &flex, &[], purpose)
            else {
                continue;
            };
            if engine.payment_with_available_sources(
                &cost,
                engine.payment_object_ref(action.object_id),
                false,
                &|e| e.eligible_restricted_mana_for_special_action(idx, purpose),
                &mut 512,
                Some(battlefield_abilities),
            ) {
                result = Availability::Available;
                break;
            } else if !proofs::payment_impossible(engine, &cost, false, battlefield_abilities) {
                result = Availability::Unknown;
            }
        }
        ready.record(
            action.object_id,
            result,
            rv1::AutomationStopReason::SpecialAction,
        );
    }
    for action in &actions.zone_cast_actions {
        use rv1::cast_source::Location;
        let location = match rv1::CastSourceZone::try_from(action.source_zone) {
            Ok(rv1::CastSourceZone::Graveyard) => Location::GraveyardObjectId(action.object_id),
            Ok(rv1::CastSourceZone::Exile) => Location::ExileObjectId(action.object_id),
            Ok(rv1::CastSourceZone::Library) => Location::LibraryObjectId(action.object_id),
            Ok(rv1::CastSourceZone::Command) => Location::CommandZoneObjectId(action.object_id),
            _ => continue,
        };
        let command = rv1::CastSpell {
            source: Some(rv1::CastSource {
                location: Some(location),
                expected_zone_change_generation: Some(action.zone_change_generation),
            }),
            face_index: action.face_index,
            cast_method: action.cast_method,
            prototyped: action.prototyped,
            casting_permission_id: action.casting_permission_id,
            ..Default::default()
        };
        let result = spell_payable(
            engine,
            player,
            action.object_id,
            command,
            &action.cost,
            action.cost_choices.as_ref(),
            actions
                .valid_targets_by_zone_object
                .get(&((u64::from(action.object_id) << 8) | u64::from(action.face_index))),
            &action.modes,
            action.min_modes,
            action.max_modes,
            &action.as_foretold_costs,
            action.base_cost_unpayable,
            battlefield_abilities,
        );
        ready.record(action.object_id, result, rv1::AutomationStopReason::Spell);
    }
    let payment_window = engine.resolving_mana_payment_for(player)
        || engine
            .state
            .pending_spell_cast
            .as_ref()
            .is_some_and(|p| p.caster == player)
        || engine
            .state
            .pending_attack_declaration
            .as_ref()
            .is_some_and(|p| p.attacking_player_id == player)
        || engine.paying_ability_activation(player);
    let priority_window = engine.state.priority_player_id() == player
        && engine.state.opening.is_none()
        && engine.state.pending_spell_cast.is_none()
        && engine.state.blocking_choice().is_none()
        && !engine.state.combat.as_ref().is_some_and(|combat| {
            combat.blockers_declared
                && combat.damage_assignment_needed
                && combat.assign_combat_damage_phase
        })
        && super::priority::instant_timing_step_allowed(&engine.state)
        && !super::combat::priority_locked_for_combat_declaration(&engine.state);
    if priority_window || payment_window {
        for (oid, effective_abilities) in battlefield_abilities {
            let oid = *oid;
            for effective in effective_abilities {
                if priority_window || effective.definition.is_mana_ability() {
                    let result = ability_payable(
                        engine,
                        player,
                        oid,
                        effective,
                        actions,
                        battlefield_abilities,
                    );
                    if result.available() {
                        ready.objects.insert(oid);
                    }
                    if effective.definition.is_mana_ability() {
                        match proofs::mana_only(engine, player, oid, &effective.definition) {
                            Some(true) => continue,
                            Some(false) => {}
                            None => {
                                if result != Availability::Unavailable {
                                    ready.record(
                                        oid,
                                        Availability::Unknown,
                                        rv1::AutomationStopReason::Ability,
                                    );
                                }
                                continue;
                            }
                        }
                    }
                    ready.record(oid, result, rv1::AutomationStopReason::Ability);
                }
            }
        }
        for action in &actions.zone_ability_actions {
            if !priority_window {
                continue;
            }
            let zone = if action.source_zone == rv1::AbilitySourceZone::Hand as i32 {
                AbilitySourceZone::Hand
            } else {
                AbilitySourceZone::Graveyard
            };
            for (index, ability, face) in
                engine.authored_zone_activated_abilities(action.object_id, zone)
            {
                if index as u32 != action.ability_index {
                    continue;
                }
                let definition = engine.ability_definition(
                    action.object_id,
                    face,
                    vec![ability.ability_id.clone()],
                );
                let effective =
                    EffectiveActivatedAbility::authored(action.ability_index, ability, definition);
                let result = ability_payable(
                    engine,
                    player,
                    action.object_id,
                    &effective,
                    actions,
                    battlefield_abilities,
                );
                ready.record(action.object_id, result, rv1::AutomationStopReason::Ability);
            }
        }
    }
    ready
}

fn target_variants(
    engine: &GameEngine,
    player: PlayerId,
    targets: Option<&rv1::SpellTargets>,
    x: u32,
) -> Vec<Vec<rv1::TargetRef>> {
    let Some(targets) = targets else {
        return vec![vec![]];
    };
    let groups = targets
        .groups
        .iter()
        .map(|group| {
            let mut candidates = Vec::new();
            for (kind, ids) in [
                (rv1::TargetRefKind::Permanent, &group.valid_permanent_ids),
                (rv1::TargetRefKind::Stack, &group.valid_stack_ids),
                (rv1::TargetRefKind::Graveyard, &group.valid_graveyard_ids),
            ] {
                candidates.extend(ids.iter().map(|&object_id| rv1::TargetRef {
                    object_id,
                    kind: kind as i32,
                    group_index: group.group_index,
                    expected_zone_change_generation: Some(
                        engine.payment_object_ref(object_id).zone_change_generation,
                    ),
                    ..Default::default()
                }));
            }
            candidates.extend(
                engine
                    .state
                    .players
                    .iter()
                    .filter(|p| {
                        !p.has_lost
                            && if p.id == player {
                                group.can_target_self
                            } else {
                                group.can_target_opponent
                            }
                    })
                    .map(|p| rv1::TargetRef {
                        object_id: p.id as u32,
                        kind: rv1::TargetRefKind::Player as i32,
                        group_index: group.group_index,
                        ..Default::default()
                    }),
            );
            combinations(&candidates, group.min as usize, group.max as usize)
        })
        .collect();
    product(groups)
        .into_iter()
        .filter_map(|groups| {
            let mut selected: Vec<_> = groups.into_iter().flatten().collect();
            if targets.is_damage_targets
                && targets.damage_division == rv1::DamageDivision::ChooseAtCast as i32
                && !selected.is_empty()
            {
                let total = if targets.fixed_damage > 0 {
                    targets.fixed_damage
                } else {
                    x
                };
                if total < selected.len() as u32 {
                    return None;
                }
                let extra = total - selected.len() as u32;
                for target in &mut selected {
                    target.damage_amount = 1;
                }
                selected[0].damage_amount += extra;
            }
            if let Some(allocation) = &targets.counter_allocation {
                let mut group: Vec<_> = selected
                    .iter_mut()
                    .filter(|t| t.group_index == allocation.group_index)
                    .collect();
                if group.is_empty() || allocation.fixed_total < group.len() as u32 {
                    return None;
                }
                let extra = allocation.fixed_total - group.len() as u32;
                for target in &mut group {
                    target.counter_amount = 1;
                }
                group[0].counter_amount += extra;
            }
            Some(selected)
        })
        .collect()
}

fn mode_variants(
    engine: &GameEngine,
    player: PlayerId,
    modes: &[rv1::LegalSpellMode],
    min: u32,
    max: u32,
    x: u32,
) -> Vec<Vec<rv1::SelectedSpellMode>> {
    if modes.is_empty() {
        return vec![vec![]];
    }
    let selectable: Vec<_> = modes.iter().filter(|m| m.selectable).collect();
    combinations(&selectable, min as usize, max as usize)
        .into_iter()
        .flat_map(|chosen| {
            product(
                chosen
                    .iter()
                    .map(|m| {
                        target_variants(engine, player, m.targets.as_ref(), x)
                            .into_iter()
                            .map(|targets| rv1::SelectedSpellMode {
                                mode_index: m.mode_index,
                                targets,
                            })
                            .collect()
                    })
                    .collect(),
            )
        })
        .take(super::payment::candidates::PROPOSAL_LIMIT)
        .collect()
}

fn cast_group_variants(costs: &rv1::LegalCostChoices) -> Vec<Vec<rv1::CastCostGroupSelection>> {
    use rv1::cast_cost_group_selection::SelectedObject;
    let groups = costs
        .cast_cost_groups
        .iter()
        .map(|group| {
            let options: Vec<_> = group.options.iter().filter(|o| o.selectable).collect();
            combinations(&options, group.min as usize, group.max as usize)
                .into_iter()
                .flat_map(|chosen| {
                    product(
                        chosen
                            .into_iter()
                            .map(|option| {
                                let base = rv1::CastCostGroupSelection {
                                    group_index: group.group_index,
                                    option_index: option.option_index,
                                    ..Default::default()
                                };
                                let mut variants = Vec::new();
                                for &hand_index in &option.valid_hand_indices {
                                    variants.push(rv1::CastCostGroupSelection {
                                        selected_object: Some(SelectedObject::HandIndex(
                                            hand_index,
                                        )),
                                        ..base.clone()
                                    });
                                }
                                for (index, &permanent_id) in
                                    option.valid_permanent_ids.iter().enumerate()
                                {
                                    variants.push(rv1::CastCostGroupSelection {
                                        selected_object: Some(SelectedObject::PermanentId(
                                            permanent_id,
                                        )),
                                        expected_zone_change_generation: option
                                            .valid_permanent_generations
                                            .get(index)
                                            .copied()
                                            .unwrap_or(0),
                                        ..base.clone()
                                    });
                                }
                                if option.object_max > 0 {
                                    for objects in combinations(
                                        &option.candidate_objects,
                                        option.object_min as usize,
                                        option.object_max as usize,
                                    ) {
                                        if option.aggregate_minimum.as_ref().is_some_and(|a| {
                                            objects
                                                .iter()
                                                .map(|o| i128::from(o.contribution))
                                                .sum::<i128>()
                                                < i128::from(a.minimum)
                                        }) {
                                            continue;
                                        }
                                        variants.push(rv1::CastCostGroupSelection {
                                            battlefield_objects: Some(rv1::CostObjectRefs {
                                                objects: objects
                                                    .into_iter()
                                                    .filter_map(|o| o.object)
                                                    .collect(),
                                            }),
                                            ..base.clone()
                                        });
                                    }
                                }
                                if variants.is_empty()
                                    && matches!(
                                        rv1::CastCostOptionKind::try_from(option.kind),
                                        Ok(rv1::CastCostOptionKind::Mana
                                            | rv1::CastCostOptionKind::PayLife)
                                    )
                                {
                                    variants.push(base);
                                }
                                variants
                            })
                            .collect(),
                    )
                })
                .take(super::payment::candidates::PROPOSAL_LIMIT)
                .collect()
        })
        .collect();
    product(groups)
        .into_iter()
        .map(|groups| groups.into_iter().flatten().collect())
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn spell_payable(
    engine: &GameEngine,
    player: PlayerId,
    oid: ObjectId,
    base: rv1::CastSpell,
    mana: &str,
    costs: Option<&rv1::LegalCostChoices>,
    targets: Option<&rv1::SpellTargets>,
    modes: &[rv1::LegalSpellMode],
    min: u32,
    max: u32,
    alternatives: &[rv1::AsForetoldCostOption],
    base_unpayable: bool,
    battlefield_abilities: &AbilityInventory,
) -> Availability {
    let Some(idx) = engine.state.player_idx(player) else {
        return Availability::Unknown;
    };
    let Some(object) = engine.state.objects.get(&oid) else {
        return Availability::Unknown;
    };
    let Some(def) = engine.registry.get(&object.card_id) else {
        return Availability::Unknown;
    };
    let Some(face) = def.face(base.face_index as usize) else {
        return Availability::Unknown;
    };
    let characteristic_face = if base.prototyped {
        face.prototyped_face()
    } else {
        Some(face.clone())
    };
    let Some(characteristic_face) = characteristic_face else {
        return Availability::Unknown;
    };
    let Ok(mana) = ManaCost::parse(mana) else {
        return Availability::Unknown;
    };
    let costs = costs.cloned().unwrap_or_default();
    if base.cast_method == rv1::CastMethod::Normal as i32
        && base
            .source
            .as_ref()
            .and_then(|source| source.location.as_ref())
            .is_some_and(|location| matches!(location, rv1::cast_source::Location::HandIndex(_)))
        && !base.prototyped
        && alternatives.is_empty()
        && costs.cast_cost_groups.is_empty()
        && engine.state.blocking_choice().is_none()
        && !face.keywords.contains(&Keyword::Convoke)
        && !engine.minimum_printed_mana_possible(player, &mana, battlefield_abilities)
    {
        return Availability::Unavailable;
    }
    let mut uncertain = base.cast_method != rv1::CastMethod::Normal as i32
        || !proofs::announcements_complete(engine, &mana, &costs, targets, modes);
    let selections = cost_selections(&costs);
    let cast_groups = cast_group_variants(&costs);
    let mut remaining = 512;
    for x in [0, 1] {
        for targets in target_variants(engine, player, targets, x) {
            for selected_modes in mode_variants(engine, player, modes, min, max, x) {
                for selection in &selections {
                    for cast_group_selections in &cast_groups {
                        for flex in phyrexian_choices(&mana) {
                            // Every remaining witness would fail immediately once the shared
                            // mana search is exhausted. Do not enumerate more announcements.
                            if remaining == 0 {
                                return Availability::Unknown;
                            }
                            let mut command = base.clone();
                            command.x_value = if mana.has_x() { x } else { 0 };
                            command.targets = targets.clone();
                            command.selected_modes = selected_modes.clone();
                            command.cost_selections = selection.clone();
                            command.cast_cost_group_selections = cast_group_selections.clone();
                            command.flex_payments = flex;
                            let options = (!base_unpayable)
                                .then_some(None)
                                .into_iter()
                                .chain(alternatives.iter().map(|a| a.source))
                                .collect::<Vec<_>>();
                            for alternative in options {
                                command.as_foretold_source = alternative;
                                let Ok(prepared) = engine.prepare_spell_cast(player, &command)
                                else {
                                    continue;
                                };
                                uncertain |= !proofs::announcements_complete(
                                    engine,
                                    &prepared.payment.mana,
                                    &costs,
                                    None,
                                    &[],
                                );
                                // Alternative and additional costs can replace or extend the pip
                                // vector. Announce Phyrexian branches against that final vector.
                                for final_flex in phyrexian_choices(&prepared.payment.mana) {
                                    command.flex_payments = final_flex;
                                    let Ok(prepared) = engine.prepare_spell_cast(player, &command)
                                    else {
                                        continue;
                                    };
                                    if engine.payment_with_available_sources(
                                        &prepared.payment,
                                        engine.payment_object_ref(oid),
                                        prepared.convoke,
                                        &|e| {
                                            e.eligible_restricted_mana_for_spell(
                                                idx,
                                                &characteristic_face,
                                            )
                                        },
                                        &mut remaining,
                                        Some(battlefield_abilities),
                                    ) {
                                        return Availability::Available;
                                    } else if !proofs::payment_impossible(
                                        engine,
                                        &prepared.payment,
                                        prepared.convoke,
                                        battlefield_abilities,
                                    ) {
                                        uncertain = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    if uncertain {
        Availability::Unknown
    } else {
        Availability::Unavailable
    }
}

fn ability_payable(
    engine: &GameEngine,
    player: PlayerId,
    oid: ObjectId,
    effective: &EffectiveActivatedAbility,
    actions: &LegalActions,
    battlefield_abilities: &AbilityInventory,
) -> Availability {
    let ability = &effective.definition;
    if !engine.ability_activatable(oid, effective.slot as usize, ability) {
        return Availability::Unavailable;
    }
    if ability.is_mana_ability()
        && ability.storage_counter_split_mana().is_none()
        && engine
            .active_mana_options(oid, ability)
            .is_none_or(|options| options.is_empty())
    {
        return Availability::Unavailable;
    }
    let idx = engine.state.player_idx(player).unwrap();
    let key = (u64::from(oid) << 32) | u64::from(effective.slot);
    let choices = actions
        .cost_choices_by_ability
        .get(&key)
        .cloned()
        .unwrap_or_else(|| {
            super::legal_actions::legal_ability_cost_choices(
                engine,
                player,
                oid,
                effective.slot as usize,
                ability,
            )
        });
    let source = super::targeting::TargetSourceIdentity::captured_for_ability(
        oid,
        engine.payment_object_ref(oid).zone_change_generation,
        ability,
    );
    let program = engine.program_for_activation_occurrence(&effective.occurrence);
    let targets = actions.valid_targets_by_ability.get(&key);
    let modal = actions.activated_ability_modes.get(&key);
    let mut uncertain = modal.is_some()
        || ability.requires_opponent_target_choice()
        || !proofs::announcements_complete(engine, &ManaCost::default(), &choices, targets, &[])
        || ability.costs.iter().any(|cost| match cost {
            AbilityCost::Mana(mana) => mana.has_x(),
            AbilityCost::Tap | AbilityCost::SacrificeSelf | AbilityCost::Loyalty(_) => false,
            _ => true,
        });
    let mut xs = BTreeSet::from([0]);
    if ability.costs.iter().any(
        |c| matches!(c, AbilityCost::Mana(mana) | AbilityCost::Waterbend(mana) if mana.has_x()),
    ) {
        xs.insert(1);
    }
    if ability.costs.contains(&AbilityCost::LoyaltyX) {
        if let Some(range) = engine.loyalty_x_range(player, oid) {
            xs.insert(range.min_x);
        }
    }
    if let Some(targets) = targets {
        for group in &targets.groups {
            if let Some(choices) = &group.x_target_choices {
                xs.extend(choices.choices.iter().map(|c| c.x_value));
            }
        }
    }
    let mut remaining = 512;
    for x in xs {
        for selections in cost_selections(&choices) {
            for targets in target_variants(engine, player, targets, x) {
                let modes = modal.map_or_else(
                    || vec![vec![]],
                    |m| mode_variants(engine, player, &m.modes, m.min_modes, m.max_modes, x),
                );
                for selected_modes in modes {
                    if remaining == 0 {
                        return Availability::Unknown;
                    }
                    let activation_targets = if let Some(modal) = &ability.modal {
                        let command = rv1::ActivateAbility {
                            targets: targets.clone(),
                            selected_modes,
                            x_value: x,
                            ..Default::default()
                        };
                        let Ok((targets, _)) = engine
                            .validate_and_capture_modal_activation(player, source, modal, &command)
                        else {
                            continue;
                        };
                        targets
                    } else {
                        if super::targeting::validate_ability_targets_with_context_and_program(
                            engine,
                            player,
                            source,
                            &targets,
                            super::targeting::TargetResolutionContext {
                                effects: &ability.effect,
                                targeting: ability.targeting.as_ref(),
                                trigger_context: TriggerContext::default(),
                                program,
                            },
                        )
                        .is_err()
                        {
                            continue;
                        }
                        if super::targeting::validate_x_target_mana_value(
                            engine,
                            &ability.effect,
                            ability.targeting.as_ref(),
                            &targets,
                            x,
                            program,
                        )
                        .is_err()
                        {
                            continue;
                        }
                        targets.clone()
                    };
                    let Ok(reduction) = engine.activated_mana_reduction(player, oid, ability)
                    else {
                        continue;
                    };
                    let extra = engine.targeting_cost_increase(
                        player,
                        TargetingCostAction::ActivatedAbilities,
                        &activation_targets,
                    );
                    let prepare = |flex: &[rv1::FlexPipPayment]| {
                        engine.prepare_availability_ability_costs(
                            player,
                            idx,
                            oid,
                            effective,
                            flex,
                            &selections,
                            x,
                            extra,
                            reduction,
                        )
                    };
                    let Ok(preliminary) = prepare(&[]) else {
                        continue;
                    };
                    uncertain |= !proofs::announcements_complete(
                        engine,
                        &preliminary.mana,
                        &choices,
                        None,
                        &[],
                    );
                    for flex in phyrexian_choices(&preliminary.mana) {
                        let Ok(cost) = prepare(&flex) else {
                            continue;
                        };
                        if engine.payment_with_available_sources(
                            &cost,
                            engine.payment_object_ref(oid),
                            false,
                            &|e| e.eligible_restricted_mana_for_ability(idx, oid),
                            &mut remaining,
                            Some(battlefield_abilities),
                        ) {
                            return Availability::Available;
                        } else if !proofs::payment_impossible(
                            engine,
                            &cost,
                            false,
                            battlefield_abilities,
                        ) {
                            uncertain = true;
                        }
                    }
                }
            }
        }
    }
    if uncertain {
        Availability::Unknown
    } else {
        Availability::Unavailable
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> GameEngine {
        let mut e = GameEngine::new(
            tricerules_cards::registry::global(),
            13_110_602,
            &[0, 1],
            20,
            None,
            true,
        )
        .unwrap();
        e.state.opening = None;
        e.state.turn_step = TurnStep::Main1;
        e.state.priority_idx = 0;
        e
    }

    fn put(e: &mut GameEngine, name: &str, zone: rv1::DevZone) -> ObjectId {
        e.apply_dev_command(
            0,
            &rv1::DevCommand {
                target_player_id: 0,
                dev: Some(rv1::dev_command::Dev::PutCardInZone(
                    rv1::DevPutCardInZone {
                        card_name: name.into(),
                        zone: zone as i32,
                        ready: true,
                    },
                )),
            },
        )
        .unwrap();
        e.reconcile_activated_ability_slots();
        e.state.next_object_id - 1
    }

    fn hints(e: &GameEngine, player: PlayerId) -> BTreeSet<ObjectId> {
        let mut batch = RuledEventBatch::default();
        super::super::legal_actions::fill_legal(&mut batch, e);
        let actions = &batch.legal_by_player[&player];
        let idx = e.state.player_idx(player).unwrap();
        actions
            .affordable_objects
            .iter()
            .map(|source| source.object_id)
            .chain(
                actions
                    .affordable_hand_indices
                    .iter()
                    .map(|&slot| e.state.players[idx].hand[slot as usize]),
            )
            .collect()
    }

    #[test]
    fn mana_only_probe_preserves_tap_triggers_and_does_not_mutate_state() {
        let mut e = engine();
        let forest = put(&mut e, "Forest", rv1::DevZone::Battlefield);
        let creature = put(&mut e, "Volatile Wanderglyph", rv1::DevZone::Battlefield);
        let mana = &e.effective_activated_abilities(forest)[0].definition;
        let before = serde_json::to_vec(&e.state).unwrap();
        assert_eq!(proofs::mana_only(&e, 0, forest, mana), Some(true));
        // The same pure mana effect on this source would also cause its loot trigger.
        assert_eq!(proofs::mana_only(&e, 0, creature, mana), Some(false));
        assert_eq!(serde_json::to_vec(&e.state).unwrap(), before);
    }

    #[test]
    fn spell_hints_require_payable_mana_and_priority_without_mutation() {
        let mut e = engine();
        let bear = put(&mut e, "Grizzly Bears", rv1::DevZone::Hand);
        assert!(!hints(&e, 0).contains(&bear));
        put(&mut e, "Forest", rv1::DevZone::Battlefield);
        assert!(!hints(&e, 0).contains(&bear));
        put(&mut e, "Island", rv1::DevZone::Battlefield);
        let before = serde_json::to_vec(&e.state).unwrap();
        assert!(hints(&e, 0).contains(&bear));
        assert_eq!(serde_json::to_vec(&e.state).unwrap(), before);
        e.state.priority_idx = 1;
        assert!(!hints(&e, 0).contains(&bear));
        assert!(!hints(&e, 1).contains(&bear));
    }

    #[test]
    fn mana_ability_and_land_play_are_actionable_only_in_their_windows() {
        let mut e = engine();
        let land = put(&mut e, "Island", rv1::DevZone::Hand);
        let stone = put(&mut e, "Mind Stone", rv1::DevZone::Battlefield);
        assert!(hints(&e, 0).contains(&land));
        assert!(hints(&e, 0).contains(&stone));
        e.state.objects.get_mut(&stone).unwrap().tapped = true;
        assert!(!hints(&e, 0).contains(&stone));
        e.state.priority_idx = 1;
        assert!(!hints(&e, 0).contains(&land));
    }

    #[test]
    fn spell_payment_hints_exclude_nonmana_abilities_until_cancel() {
        let mut e = engine();
        let bear = put(&mut e, "Grizzly Bears", rv1::DevZone::Hand);
        let gnomes = put(&mut e, "Bottle Gnomes", rv1::DevZone::Battlefield);
        let forest = put(&mut e, "Forest", rv1::DevZone::Battlefield);
        assert!(hints(&e, 0).contains(&gnomes));
        let slot = e.state.players[0]
            .hand
            .iter()
            .position(|&id| id == bear)
            .unwrap();
        e.begin_spell_cast(
            0,
            &rv1::BeginSpellCast {
                announcement: Some(rv1::SpellCastAnnouncement {
                    cast_method: rv1::CastMethod::Normal as i32,
                    source: Some(rv1::CastSource {
                        location: Some(rv1::cast_source::Location::HandIndex(slot as u32)),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
            },
        )
        .unwrap();
        let offered = hints(&e, 0);
        assert!(offered.contains(&forest));
        assert!(!offered.contains(&gnomes));
        let transaction_id = e.state.pending_spell_cast.as_ref().unwrap().transaction_id;
        e.cancel_spell_cast(0, &rv1::CancelSpellCast { transaction_id })
            .unwrap();
        assert!(hints(&e, 0).contains(&gnomes));
    }

    #[test]
    fn damage_assignment_is_not_a_priority_window() {
        let mut e = engine();
        let gnomes = put(&mut e, "Bottle Gnomes", rv1::DevZone::Battlefield);
        let forest = put(&mut e, "Forest", rv1::DevZone::Battlefield);
        e.state.turn_step = TurnStep::DeclareBlockers;
        e.state.combat = Some(CombatState {
            attacking: vec![],
            attack_assignments: HashMap::new(),
            blockers: HashMap::new(),
            damage_assignments: HashMap::new(),
            trample_player_damage: HashMap::new(),
            damage_assignment_needed: true,
            attackers_declared: true,
            blockers_declared_by: vec![1],
            blockers_declared: true,
            assign_combat_damage_phase: false,
            first_strike_attackers: vec![],
            first_strike_blockers: HashMap::new(),
            first_strike_damage_done: false,
        });
        assert!(hints(&e, 0).contains(&gnomes));
        assert!(hints(&e, 0).contains(&forest));
        e.state.combat.as_mut().unwrap().assign_combat_damage_phase = true;
        let offered = hints(&e, 0);
        assert!(!offered.contains(&gnomes));
        assert!(!offered.contains(&forest));
    }

    #[test]
    fn additional_costs_modes_and_graveyard_casts_use_real_announcements() {
        let mut e = engine();
        e.state.players[0].hand.clear();
        let triumph = put(&mut e, "Bitter Triumph", rv1::DevZone::Hand);
        e.state.players[0].mana_pool.black = 1;
        e.state.players[0].mana_pool.colorless = 1;
        e.state.players[0].life = 2;
        put(&mut e, "Grizzly Bears", rv1::DevZone::Battlefield);
        assert!(!hints(&e, 0).contains(&triumph));
        e.state.players[0].life = 3;
        assert!(hints(&e, 0).contains(&triumph));
        let charm = put(&mut e, "Boros Charm", rv1::DevZone::Hand);
        e.state.players[0].mana_pool.red = 1;
        assert!(!hints(&e, 0).contains(&charm));
        e.state.players[0].mana_pool.white = 1;
        assert!(hints(&e, 0).contains(&charm));
        let looting = put(&mut e, "Faithless Looting", rv1::DevZone::Hand);
        e.state.players[0].hand.retain(|&oid| oid != looting);
        e.state.players[0].graveyard.push(looting);
        e.state.objects.get_mut(&looting).unwrap().zone = Zone::Graveyard;
        e.state.players[0].mana_pool = Default::default();
        e.state.players[0].mana_pool.red = 1;
        assert!(!hints(&e, 0).contains(&looting));
        e.state.players[0].mana_pool.colorless = 2;
        assert!(hints(&e, 0).contains(&looting));
    }

    #[test]
    fn aggregate_counters_and_phyrexian_life_are_competing_resources() {
        let mut e = engine();
        let tekuthal = put(
            &mut e,
            "Tekuthal, Inquiry Dominus",
            rv1::DevZone::Battlefield,
        );
        let bear = put(&mut e, "Grizzly Bears", rv1::DevZone::Battlefield);
        e.state
            .objects
            .get_mut(&bear)
            .unwrap()
            .set_counter(CounterKind::PlusOnePlusOne, 3);
        e.state.players[0].mana_pool.colorless = 1;
        e.state.players[0].life = 3;
        assert!(!hints(&e, 0).contains(&tekuthal));
        e.state.players[0].life = 4;
        assert!(hints(&e, 0).contains(&tekuthal));
        e.state
            .objects
            .get_mut(&bear)
            .unwrap()
            .set_counter(CounterKind::PlusOnePlusOne, 2);
        assert!(!hints(&e, 0).contains(&tekuthal));
    }
}
