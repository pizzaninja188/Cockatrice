//! Deterministic choices from engine-authored offers; unsupported shapes remain explicit.
use super::ScenarioCommandExt;
use super::{Cmd, Ev, GameEngine};
use tricerules_cards::mana::{ManaCost, ManaSymbol};
use tricerules_proto::ruled::v1::*;

/// Build a positive fixture activation from current engine offers, including zone,
/// generation, targets, nonmana choices and a complete payment preview. Expectations
/// still belong in the calling test. Unsupported choices/resources return an error.
/// Keep raw constructors for deliberately illegal/stale commands in negative tests.
pub(crate) fn activation(
    e: &mut GameEngine,
    actor: i32,
    object: u32,
    index: u32,
) -> Result<RuledCommand, String> {
    let batch = e.initial_response_batch();
    let legal = batch
        .legal_by_player
        .get(&actor)
        .ok_or("unknown fixture actor")?;
    let key = (u64::from(object) << 32) | u64::from(index);
    let choices = legal
        .cost_choices_by_ability
        .get(&key)
        .ok_or("ability not offered by generic battlefield fixture")?;
    let (zone, generation, activatable, mana_cost) = if let Some(action) = legal
        .zone_ability_actions
        .iter()
        .find(|a| a.object_id == object && a.ability_index == index)
    {
        let ability = action
            .ability
            .as_ref()
            .ok_or("zone offer omitted ability")?;
        (
            action.source_zone,
            action.zone_change_generation,
            ability.activatable,
            ability.mana_cost.clone(),
        )
    } else {
        let source = batch
            .events
            .iter()
            .find_map(|event| match &event.ev {
                Some(Ev::ZoneView(view)) => view
                    .per_player
                    .iter()
                    .find(|p| p.player_id == actor)
                    .and_then(|p| p.battlefield_objects.iter().find(|o| o.object_id == object)),
                _ => None,
            })
            .ok_or("source not offered to fixture actor")?;
        let ability = source
            .activated_abilities
            .iter()
            .find(|a| a.ability_index == index)
            .ok_or("ability not offered by generic battlefield fixture")?;
        (
            AbilitySourceZone::Battlefield as i32,
            source.zone_change_generation,
            ability.activatable,
            ability.mana_cost.clone(),
        )
    };
    let returns_tapped_creature = zone == AbilitySourceZone::Hand as i32
        && choices
            .choices
            .iter()
            .any(|choice| choice.kind() == CostChoiceKind::ReturnTappedCreature);
    let aggregate_counters = choices
        .choices
        .iter()
        .any(|choice| choice.counter_removal_group.is_some());
    if returns_tapped_creature || aggregate_counters {
        // These costs are bound in the engine's staged payment window: a returned creature after
        // mana abilities may tap new candidates, and aggregate counter contributions against the
        // window's trusted inventory. The drain commits the window with its published candidate.
        if !choices.non_mana_costs_payable || !activatable {
            return Err("staged activation is not currently payable".into());
        }
        let cost_selections = if aggregate_counters {
            costs(Some(choices))?
        } else {
            vec![]
        };
        return Ok(RuledCommand {
            cmd: Some(Cmd::BeginAbilityActivation(BeginAbilityActivation {
                source_object_id: object,
                expected_zone_change_generation: generation,
                ability_index: index,
                source_zone: zone,
                cost_selections,
                flex_payments: if aggregate_counters {
                    phyrexian_mana_choices(&mana_cost)?
                } else {
                    vec![]
                },
                ..Default::default()
            })),
        });
    }
    let costs = costs(Some(choices))?;
    let mut x_value = 0;
    let mut activation_targets = Vec::new();
    if let Some(offer) = legal.valid_targets_by_ability.get(&key) {
        if offer.groups.iter().any(|group| group.chosen_by_opponent) {
            let group = offer
                .groups
                .first()
                .ok_or("missing controller target group")?;
            if group.chosen_by_opponent || group.min != 1 || group.max != 1 || !activatable {
                return Err("unsupported staged controller target group".into());
            }
            let &target = group
                .valid_permanent_ids
                .first()
                .ok_or("no controller creature target")?;
            return Ok(RuledCommand {
                cmd: Some(Cmd::BeginAbilityActivation(BeginAbilityActivation {
                    source_object_id: object,
                    expected_zone_change_generation: generation,
                    ability_index: index,
                    source_zone: AbilitySourceZone::Battlefield as i32,
                    own_target: Some(AbilityActivationTarget {
                        kind: tricerules_proto::TargetRefKind::Permanent as i32,
                        object_id: target,
                        zone_change_generation: e
                            .state
                            .zone_change_generation
                            .get(&target)
                            .copied()
                            .unwrap_or(0),
                        group_index: group.group_index,
                    }),
                    ..Default::default()
                })),
            });
        }
        if let Some(group) = offer
            .groups
            .iter()
            .find(|group| group.x_target_choices.is_some())
        {
            if group.min != 1 || group.max != 1 {
                return Err("unsupported X-bound target group shape".into());
            }
            let choices = group
                .x_target_choices
                .as_ref()
                .ok_or("missing X-target offer")?;
            let choice = choices
                .choices
                .iter()
                .find(|choice| !choice.candidates.is_empty())
                .ok_or("no X-target pair in engine offer")?;
            let candidate = choice
                .candidates
                .first()
                .ok_or("selected X choice has no target candidate")?;
            x_value = choice.x_value;
            activation_targets.push(TargetRef {
                object_id: candidate.object_id,
                group_index: group.group_index,
                kind: TargetRefKind::Permanent as i32,
                expected_zone_change_generation: Some(candidate.zone_change_generation),
                ..Default::default()
            });
        } else {
            activation_targets = targets(e, actor, Some(offer))?;
        }
    }
    let (activation_targets, selected_modes) =
        if let Some(mode_offer) = legal.activated_ability_modes.get(&key) {
            (
                Vec::new(),
                modes(e, actor, mode_offer.min_modes, &mode_offer.modes)?,
            )
        } else {
            (activation_targets, Vec::new())
        };
    let mut command = RuledCommand {
        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
            source_object_id: object,
            source_zone: zone,
            expected_zone_change_generation: generation,
            ability_index: index,
            targets: activation_targets,
            selected_modes,
            x_value,
            cost_selections: costs,
            ..Default::default()
        })),
    };
    // Preserve specific payment diagnostics for unsupported conformance fixtures.
    pay(e, actor, &mut command)?;
    if !activatable {
        return Err("fixture ability is not currently activatable".into());
    }
    Ok(command)
}

pub(crate) fn targets(
    e: &GameEngine,
    actor: i32,
    offer: Option<&SpellTargets>,
) -> Result<Vec<TargetRef>, String> {
    let Some(offer) = offer else {
        return Ok(vec![]);
    };
    let mut result: Vec<TargetRef> = vec![];
    if let Some(allocation) = &offer.counter_allocation {
        if allocation.fixed_total == 0
            || offer
                .groups
                .iter()
                .filter(|group| group.group_index == allocation.group_index)
                .count()
                != 1
        {
            return Err("invalid fixed counter-allocation offer".into());
        }
    }
    for group in &offer.groups {
        if let Some(allocation) = offer
            .counter_allocation
            .as_ref()
            .filter(|allocation| allocation.group_index == group.group_index)
        {
            if group.min > 1 || group.max < 1 || group.chosen_by_opponent {
                return Err("counter allocation needs a specialized group fixture".into());
            }
            let candidate = group
                .valid_permanent_targets
                .iter()
                .find(|candidate| {
                    group.valid_permanent_ids.contains(&candidate.object_id)
                        && !result.iter().any(|target| {
                            target.object_id == candidate.object_id
                                && group
                                    .distinct_from_group_indices
                                    .contains(&target.group_index)
                        })
                })
                .ok_or("counter allocation needs an offered generation-bound permanent")?;
            result.push(TargetRef {
                object_id: candidate.object_id,
                group_index: group.group_index,
                kind: TargetRefKind::Permanent as i32,
                counter_amount: allocation.fixed_total,
                expected_zone_change_generation: Some(candidate.zone_change_generation),
                ..Default::default()
            });
            continue;
        }
        let mut candidates = vec![];
        for (kind, ids) in [
            (2, &group.valid_permanent_ids),
            (3, &group.valid_stack_ids),
            (4, &group.valid_graveyard_ids),
        ] {
            candidates.extend(ids.iter().map(|&id| (kind, id)));
        }
        if group.can_target_self {
            candidates.push((1, actor as u32));
        }
        // Select only other actual player IDs; fixtures may have more than two seats.
        if group.can_target_opponent {
            candidates.extend(
                e.state
                    .players
                    .iter()
                    .filter(|p| p.id != actor)
                    .map(|p| (1, p.id as u32)),
            );
        }
        let count = group
            .min
            .max(u32::from(group.max > 0 && !candidates.is_empty()));
        let mut picked = 0;
        for (kind, object_id) in candidates {
            if result.iter().any(|t| {
                t.kind == kind
                    && t.object_id == object_id
                    && (t.group_index == group.group_index
                        || group.distinct_from_group_indices.contains(&t.group_index))
            }) {
                continue;
            }
            if picked == count {
                break;
            }
            result.push(TargetRef {
                counter_amount: 0,
                object_id,
                kind,
                group_index: group.group_index,
                damage_amount: 0,
                expected_zone_change_generation: (kind == TargetRefKind::Permanent as i32).then(
                    || {
                        e.state
                            .zone_change_generation
                            .get(&object_id)
                            .copied()
                            .unwrap_or(0)
                    },
                ),
            });
            picked += 1;
        }
        if picked < group.min {
            return Err(format!(
                "target group {} needs a richer fixture",
                group.group_index
            ));
        }
    }
    if offer.is_damage_targets && offer.damage_division == 0 {
        let total = offer.fixed_damage;
        if total < result.len() as u32 {
            return Err("damage allocation needs a nonzero X fixture".into());
        }
        let count = result.len() as u32;
        for (i, target) in result.iter_mut().enumerate() {
            target.damage_amount = if i == 0 { total - count + 1 } else { 1 };
        }
    }
    Ok(result)
}

/// Announce mana for every Phyrexian or hybrid Phyrexian pip of an engine-offered ability cost.
/// Staged aggregate activations require each such pip to be chosen explicitly.
fn phyrexian_mana_choices(cost: &str) -> Result<Vec<FlexPipPayment>, String> {
    if cost.is_empty() {
        return Ok(vec![]);
    }
    let cost = ManaCost::parse(cost).map_err(|error| format!("offered mana cost: {error}"))?;
    Ok(cost
        .pips
        .iter()
        .enumerate()
        .filter_map(|(index, pip)| {
            // A single-color Phyrexian pip's mana branch is implied; only hybrid Phyrexian pips
            // announce which color pays.
            let mana_color = match pip {
                ManaSymbol::Phyrexian(_) => 0,
                ManaSymbol::HybridPhyrexian(color, _) => mana_color(*color) as i32,
                _ => return None,
            };
            Some(FlexPipPayment {
                pip_index: index as u32,
                pay_life: false,
                mana_color,
            })
        })
        .collect())
}

fn mana_color(color: tricerules_cards::mana::ColorPip) -> ManaColor {
    match color {
        tricerules_cards::mana::ColorPip::W => ManaColor::White,
        tricerules_cards::mana::ColorPip::U => ManaColor::Blue,
        tricerules_cards::mana::ColorPip::B => ManaColor::Black,
        tricerules_cards::mana::ColorPip::R => ManaColor::Red,
        tricerules_cards::mana::ColorPip::G => ManaColor::Green,
    }
}

/// Choose one announced full-mana branch from the engine's offered cost, never Oracle display data.
pub(crate) fn hybrid_phyrexian_choices(cost: &str) -> Result<Vec<FlexPipPayment>, String> {
    let cost = ManaCost::parse(cost).map_err(|error| format!("offered mana cost: {error}"))?;
    Ok(cost
        .pips
        .iter()
        .enumerate()
        .filter_map(|(index, pip)| {
            let ManaSymbol::HybridPhyrexian(first, _) = pip else {
                return None;
            };
            let color = match first {
                tricerules_cards::mana::ColorPip::W => ManaColor::White,
                tricerules_cards::mana::ColorPip::U => ManaColor::Blue,
                tricerules_cards::mana::ColorPip::B => ManaColor::Black,
                tricerules_cards::mana::ColorPip::R => ManaColor::Red,
                tricerules_cards::mana::ColorPip::G => ManaColor::Green,
            };
            Some(FlexPipPayment {
                pip_index: index as u32,
                pay_life: false,
                mana_color: color as i32,
            })
        })
        .collect())
}

/// Modes for triggers and activated abilities, which never carry linked cast costs.
pub(crate) fn modes(
    e: &GameEngine,
    actor: i32,
    min: u32,
    offers: &[LegalSpellMode],
) -> Result<Vec<SelectedSpellMode>, String> {
    let (modes, linked) = spell_modes(e, actor, min, offers)?;
    if !linked.is_empty() {
        return Err("linked cast cost offered outside a spell cast".into());
    }
    Ok(modes)
}

/// Spell modes plus the Spree/Tiered cast-cost options linked to the chosen modes. The linked
/// coordinates are copied verbatim into `cast_cost_groups`.
pub(crate) fn spell_modes(
    e: &GameEngine,
    actor: i32,
    min: u32,
    offers: &[LegalSpellMode],
) -> Result<(Vec<SelectedSpellMode>, Vec<LinkedCastCostOption>), String> {
    let mut result = vec![];
    let mut linked = vec![];
    for mode in offers.iter().filter(|m| m.selectable) {
        if result.len() == min as usize {
            break;
        }
        if let Ok(targets) = targets(e, actor, mode.targets.as_ref()) {
            result.push(SelectedSpellMode {
                mode_index: mode.mode_index,
                targets,
            });
            linked.extend(mode.linked_cast_cost);
        }
    }
    if result.len() != min as usize {
        return Err("modal fixture cannot satisfy required selectable modes".into());
    }
    Ok((result, linked))
}

/// Required cast-cost groups and mode-linked options, each filled from that option's own
/// engine-published candidates. Additional mana is funded later by `pay`'s preview.
pub(crate) fn cast_cost_groups(
    offer: Option<&LegalCostChoices>,
    linked: &[LinkedCastCostOption],
) -> Result<Vec<CastCostGroupSelection>, String> {
    let Some(offer) = offer else {
        if linked.is_empty() {
            return Ok(vec![]);
        }
        return Err("linked cast cost without published cast-cost groups".into());
    };
    let mut result = vec![];
    for group in &offer.cast_cost_groups {
        let chosen: Vec<_> = linked
            .iter()
            .filter(|option| option.group_index == group.group_index)
            .collect();
        if !chosen.is_empty() {
            for link in chosen {
                let option = group
                    .options
                    .iter()
                    .find(|option| option.option_index == link.option_index)
                    .ok_or("linked cast cost names an unpublished option")?;
                result.push(cast_cost_option_selection(group.group_index, option)?);
            }
            continue;
        }
        let mut picked = 0;
        for option in group.options.iter().filter(|option| option.selectable) {
            if picked == group.min {
                break;
            }
            if let Ok(selection) = cast_cost_option_selection(group.group_index, option) {
                result.push(selection);
                picked += 1;
            }
        }
        if picked < group.min {
            return Err("required cast-cost group has no satisfiable option".into());
        }
    }
    if linked.iter().any(|link| {
        !offer
            .cast_cost_groups
            .iter()
            .any(|g| g.group_index == link.group_index)
    }) {
        return Err("linked cast cost names an unpublished group".into());
    }
    Ok(result)
}

fn cast_cost_option_selection(
    group_index: u32,
    option: &LegalCastCostOption,
) -> Result<CastCostGroupSelection, String> {
    use cast_cost_group_selection::SelectedObject;
    let mut selection = CastCostGroupSelection {
        group_index,
        option_index: option.option_index,
        ..Default::default()
    };
    if !option.candidate_objects.is_empty() || option.object_min > 0 {
        let minimum = option.aggregate_minimum.map_or(0, |a| i64::from(a.minimum));
        let mut objects = vec![];
        let mut contribution = 0i64;
        for candidate in &option.candidate_objects {
            if contribution >= minimum && objects.len() >= option.object_min as usize {
                break;
            }
            if option.object_max > 0 && objects.len() == option.object_max as usize {
                break;
            }
            let Some(object) = candidate.object else {
                continue;
            };
            if minimum > 0 && candidate.contribution <= 0 {
                continue;
            }
            objects.push(object);
            contribution += candidate.contribution;
        }
        if contribution < minimum || objects.len() < option.object_min as usize {
            return Err("cast-cost cohort lacks offered candidates".into());
        }
        selection.battlefield_objects = Some(CostObjectRefs { objects });
    } else if let Some(&id) = option.valid_permanent_ids.first() {
        selection.selected_object = Some(SelectedObject::PermanentId(id));
        selection.expected_zone_change_generation = option
            .valid_permanent_generations
            .first()
            .copied()
            .ok_or("cast-cost permanent omitted its generation")?;
    } else if let Some(&slot) = option.valid_hand_indices.first() {
        selection.selected_object = Some(SelectedObject::HandIndex(slot));
    } else if !matches!(
        option.kind(),
        CastCostOptionKind::Mana | CastCostOptionKind::PayLife
    ) {
        return Err("cast-cost option offered no object candidate".into());
    }
    Ok(selection)
}

pub(crate) fn costs(offer: Option<&LegalCostChoices>) -> Result<Vec<CostSelection>, String> {
    let Some(offer) = offer else {
        return Ok(vec![]);
    };
    if !offer.non_mana_costs_payable {
        return Err("nonmana costs need additional fixture resources".into());
    }
    let mut used = std::collections::BTreeSet::new();
    let mut result = vec![];
    for choice in &offer.choices {
        if let Some(selection) = counter_or_aggregate_selection(choice, &mut used)? {
            result.push(CostSelection {
                cost_index: choice.cost_index,
                selection: Some(selection),
            });
            continue;
        }
        if choice.min == 0 {
            continue;
        }
        let selection = if choice.zone == CostChoiceZone::Hand as i32 {
            let id = choice
                .candidate_ids
                .iter()
                .find(|id| !used.contains(&(choice.zone, **id)))
                .ok_or("distinct hand cost assignment unavailable")?;
            used.insert((choice.zone, *id));
            if choice.min != 1 {
                return Err("multi-card hand cost needs a specialized fixture".into());
            }
            cost_selection::Selection::HandIndex(*id)
        } else {
            let objects: Vec<_> = choice
                .candidate_objects
                .iter()
                .filter_map(|c| c.object)
                .filter(|o| !used.contains(&(choice.zone, o.object_id)))
                .take(choice.min as usize)
                .collect();
            if objects.len() != choice.min as usize {
                return Err("generation-bound cost candidates unavailable".into());
            }
            for o in &objects {
                used.insert((choice.zone, o.object_id));
            }
            let refs = CostObjectRefs { objects };
            match CostChoiceZone::try_from(choice.zone) {
                Ok(CostChoiceZone::Battlefield)
                    if choice.min == 1
                        && matches!(
                            CostChoiceKind::try_from(choice.kind),
                            Ok(CostChoiceKind::Sacrifice)
                        ) =>
                {
                    cost_selection::Selection::PermanentId(refs.objects[0].object_id)
                }
                Ok(CostChoiceZone::Battlefield) => {
                    cost_selection::Selection::BattlefieldObjects(refs)
                }
                Ok(CostChoiceZone::Graveyard) => cost_selection::Selection::GraveyardObjects(refs),
                _ => return Err("unsupported cost zone".into()),
            }
        };
        result.push(CostSelection {
            cost_index: choice.cost_index,
            selection: Some(selection),
        });
    }
    Ok(result)
}

/// Select engine-offered counter-removal sources and aggregate-minimum objects (Crew, total
/// power). Returns `None` for choices with neither shape so the caller's plain path handles them.
fn counter_or_aggregate_selection(
    choice: &LegalCostChoice,
    used: &mut std::collections::BTreeSet<(i32, u32)>,
) -> Result<Option<cost_selection::Selection>, String> {
    if let Some(group) = &choice.counter_removal_group {
        let mut contributions = vec![];
        let mut remaining = group.required_count;
        for candidate in &group.candidates {
            if remaining == 0 {
                break;
            }
            let Some(object) = candidate.object else {
                continue;
            };
            for option in &candidate.options {
                if remaining == 0 {
                    break;
                }
                let count = option.available_count.min(remaining);
                if count == 0 {
                    continue;
                }
                contributions.push(CounterRemovalContribution {
                    object: Some(object),
                    option_id: option.option_id,
                    count,
                });
                remaining -= count;
            }
        }
        if remaining > 0 {
            return Err("counter-removal group lacks enough offered counters".into());
        }
        return Ok(Some(cost_selection::Selection::CounterRemovalGroup(
            CounterRemovalGroupSelection { contributions },
        )));
    }
    if let Some(removal) = &choice.counter_removal {
        let option = removal
            .options
            .iter()
            .find(|option| option.available_count >= removal.count)
            .ok_or("counter-removal cost lacks enough offered counters")?;
        let source = match removal.source {
            Some(source) => source,
            None => choice
                .candidate_objects
                .iter()
                .find_map(|candidate| candidate.object)
                .ok_or("counter-removal cost offered no source candidate")?,
        };
        return Ok(Some(cost_selection::Selection::CounterRemoval(
            CounterRemovalSelection {
                source: Some(source),
                option_id: option.option_id,
            },
        )));
    }
    let Some(aggregate) = &choice.aggregate_minimum else {
        return Ok(None);
    };
    let mut objects = vec![];
    let mut contribution = 0i64;
    for candidate in &choice.candidate_objects {
        if contribution >= i64::from(aggregate.minimum) && objects.len() >= choice.min as usize {
            break;
        }
        let Some(object) = candidate.object else {
            continue;
        };
        if candidate.contribution <= 0 || used.contains(&(choice.zone, object.object_id)) {
            continue;
        }
        if choice.max > 0 && objects.len() == choice.max as usize {
            break;
        }
        used.insert((choice.zone, object.object_id));
        objects.push(object);
        contribution += candidate.contribution;
    }
    if contribution < i64::from(aggregate.minimum) || objects.len() < choice.min as usize {
        return Err("aggregate cost needs more offered contribution".into());
    }
    let refs = CostObjectRefs { objects };
    Ok(Some(match CostChoiceZone::try_from(choice.zone) {
        Ok(CostChoiceZone::Graveyard) => cost_selection::Selection::GraveyardObjects(refs),
        _ => cost_selection::Selection::BattlefieldObjects(refs),
    }))
}

pub(crate) fn pay(e: &GameEngine, actor: i32, command: &mut RuledCommand) -> Result<(), String> {
    fn query(command: &RuledCommand) -> PreviewPayment {
        let mut q = PreviewPayment {
            transaction_id: 1,
            revision: 1,
            ..Default::default()
        };
        match command.cmd.as_ref().unwrap() {
            Cmd::CastSpell(c) => q.cast_spell = Some(c.clone()),
            Cmd::ActivateAbility(c) => q.activate_ability = Some(c.clone()),
            Cmd::CommitAbilityActivation(c) => q.commit_ability_activation = Some(c.clone()),
            Cmd::SubmitResolutionChoice(c) => q.resolution_choice = Some(c.clone()),
            _ => {}
        }
        q
    }
    let preview = e.preview_payment_scenario(actor, &query(command));
    if !preview.valid {
        return Err(format!("payment preview rejected: {}", preview.error));
    }
    let mut selection = preview
        .selection
        .ok_or("payment preview omitted selection")?;
    let cost = ManaCost::parse(
        preview
            .remaining_cost
            .split(" or ")
            .next()
            .unwrap_or_default(),
    )
    .map_err(|err| format!("payment cost: {err:?}"))?;
    let mut mana = selection.mana.unwrap_or_default();
    let mut generic = 0;
    for pip in cost.pips {
        match pip {
            ManaSymbol::W => mana.w += 1,
            ManaSymbol::U => mana.u += 1,
            ManaSymbol::B => mana.b += 1,
            ManaSymbol::R => mana.r += 1,
            ManaSymbol::G => mana.g += 1,
            ManaSymbol::C => mana.c += 1,
            ManaSymbol::Generic(n) | ManaSymbol::MonoHybrid(n, _) => generic += n,
            ManaSymbol::Hybrid(c, _) | ManaSymbol::Phyrexian(c) => match c {
                tricerules_cards::mana::ColorPip::W => mana.w += 1,
                tricerules_cards::mana::ColorPip::U => mana.u += 1,
                tricerules_cards::mana::ColorPip::B => mana.b += 1,
                tricerules_cards::mana::ColorPip::R => mana.r += 1,
                tricerules_cards::mana::ColorPip::G => mana.g += 1,
            },
            ManaSymbol::HybridPhyrexian(_, _) => {
                return Err("unannounced hybrid Phyrexian payment".into())
            }
            ManaSymbol::X => return Err("unresolved X payment".into()),
        }
    }
    // Generic mana comes from colorless first, then from whatever colored mana remains in the
    // payer's pool after the colored pips above.
    let pool = e
        .state
        .player_idx(actor)
        .map(|index| e.state.players[index].mana_pool)
        .unwrap_or_default();
    for (spent, available) in [
        (&mut mana.c, pool.colorless),
        (&mut mana.w, pool.white),
        (&mut mana.u, pool.blue),
        (&mut mana.b, pool.black),
        (&mut mana.r, pool.red),
        (&mut mana.g, pool.green),
    ] {
        let take = generic.min(available.saturating_sub(*spent));
        *spent += take;
        generic -= take;
    }
    mana.c += generic;
    selection.mana = Some(mana);
    match command.cmd.as_mut().unwrap() {
        Cmd::CastSpell(c) => c.payment = Some(selection),
        Cmd::ActivateAbility(c) => c.payment = Some(selection),
        Cmd::CommitAbilityActivation(c) => c.payment = Some(selection),
        Cmd::SubmitResolutionChoice(c) => c.payment = Some(selection),
        _ => return Err("unsupported payment action".into()),
    }
    let checked = e.preview_payment_scenario(actor, &query(command));
    if !checked.valid || !checked.complete {
        return Err(format!("payment not complete: {}", checked.error));
    }
    Ok(())
}
