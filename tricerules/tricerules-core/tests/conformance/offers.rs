//! Deterministic choices from engine-authored offers; unsupported shapes remain explicit.
use super::*;
use tricerules_cards::mana::{ManaCost, ManaSymbol};

pub(super) fn targets(
    e: &GameEngine,
    actor: i32,
    offer: Option<&SpellTargets>,
) -> Result<Vec<TargetRef>, String> {
    let Some(offer) = offer else {
        return Ok(vec![]);
    };
    let mut result: Vec<TargetRef> = vec![];
    for group in &offer.groups {
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
        // Fixtures use ordinary two-seat games, where the other seat is the opponent.
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
                object_id,
                kind,
                group_index: group.group_index,
                damage_amount: 0,
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

pub(super) fn modes(
    e: &GameEngine,
    actor: i32,
    min: u32,
    offers: &[LegalSpellMode],
) -> Result<Vec<SelectedSpellMode>, String> {
    let mut result = vec![];
    for mode in offers.iter().filter(|m| m.selectable) {
        if result.len() == min as usize {
            break;
        }
        if mode.linked_cast_cost.is_some() {
            continue;
        }
        if let Ok(targets) = targets(e, actor, mode.targets.as_ref()) {
            result.push(SelectedSpellMode {
                mode_index: mode.mode_index,
                targets,
            });
        }
    }
    if result.len() != min as usize {
        return Err("modal fixture cannot satisfy required selectable modes".into());
    }
    Ok(result)
}

pub(super) fn costs(offer: Option<&LegalCostChoices>) -> Result<Vec<CostSelection>, String> {
    let Some(offer) = offer else {
        return Ok(vec![]);
    };
    if !offer.non_mana_costs_payable {
        return Err("nonmana costs need additional fixture resources".into());
    }
    if offer.cast_cost_groups.iter().any(|g| g.min > 0) {
        return Err("required cast-cost group needs a specialized fixture".into());
    }
    let mut used = std::collections::BTreeSet::new();
    let mut result = vec![];
    for choice in &offer.choices {
        if choice.min == 0 {
            continue;
        }
        if choice.counter_removal.is_some() || choice.aggregate_minimum.is_some() {
            return Err("counter or aggregate cost needs a specialized fixture".into());
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

pub(super) fn pay(e: &GameEngine, actor: i32, command: &mut RuledCommand) -> Result<(), String> {
    fn query(command: &RuledCommand) -> PreviewPayment {
        let mut q = PreviewPayment {
            transaction_id: 1,
            revision: 1,
            ..Default::default()
        };
        match command.cmd.as_ref().unwrap() {
            Cmd::CastSpell(c) => q.cast_spell = Some(c.clone()),
            Cmd::ActivateAbility(c) => q.activate_ability = Some(c.clone()),
            Cmd::SubmitResolutionChoice(c) => q.resolution_choice = Some(c.clone()),
            _ => {}
        }
        q
    }
    let preview = e.preview_payment(actor, &query(command));
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
    for pip in cost.pips {
        match pip {
            ManaSymbol::W => mana.w += 1,
            ManaSymbol::U => mana.u += 1,
            ManaSymbol::B => mana.b += 1,
            ManaSymbol::R => mana.r += 1,
            ManaSymbol::G => mana.g += 1,
            ManaSymbol::C => mana.c += 1,
            ManaSymbol::Generic(n) | ManaSymbol::MonoHybrid(n, _) => mana.c += n,
            ManaSymbol::Hybrid(c, _) | ManaSymbol::Phyrexian(c) => match c {
                tricerules_cards::mana::ColorPip::W => mana.w += 1,
                tricerules_cards::mana::ColorPip::U => mana.u += 1,
                tricerules_cards::mana::ColorPip::B => mana.b += 1,
                tricerules_cards::mana::ColorPip::R => mana.r += 1,
                tricerules_cards::mana::ColorPip::G => mana.g += 1,
            },
            ManaSymbol::X => return Err("unresolved X payment".into()),
        }
    }
    selection.mana = Some(mana);
    match command.cmd.as_mut().unwrap() {
        Cmd::CastSpell(c) => c.payment = Some(selection),
        Cmd::ActivateAbility(c) => c.payment = Some(selection),
        Cmd::SubmitResolutionChoice(c) => c.payment = Some(selection),
        _ => return Err("unsupported payment action".into()),
    }
    let checked = e.preview_payment(actor, &query(command));
    if !checked.valid || !checked.complete {
        return Err(format!("payment not complete: {}", checked.error));
    }
    Ok(())
}
