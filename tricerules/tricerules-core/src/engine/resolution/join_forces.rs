use super::*;

fn controller_first_turn_order(
    mut turn_order: Vec<PlayerId>,
    controller: PlayerId,
) -> Vec<PlayerId> {
    if let Some(start) = turn_order.iter().position(|player| *player == controller) {
        turn_order.rotate_left(start);
    }
    turn_order
}

/// Join Forces explicitly starts with the resolving spell or ability's controller, then continues
/// in turn order. The order is frozen when the instruction starts; players who leave before their
/// choice contribute zero (CR 800.4f).
pub(super) fn begin(cx: &mut EffectCx<'_>) -> Result<EffectOutcome, EngineError> {
    let mut turn_order = cx
        .engine
        .state
        .players
        .iter()
        .map(|player| player.id)
        .collect::<Vec<_>>();
    turn_order.sort_by_key(|player| cx.engine.state.apnap_rank(*player));
    let payer_order = controller_first_turn_order(turn_order, cx.controller)
        .into_iter()
        .filter(|player| {
            cx.engine
                .state
                .player_idx(*player)
                .is_some_and(|index| !cx.engine.state.players[index].has_lost)
        })
        .collect::<Vec<_>>();
    let Some(&first_payer) = payer_order.first() else {
        cx.effect_result.mana_paid = Some(0);
        return Ok(EffectOutcome::Continue);
    };

    let payment = PendingManaPayment {
        waterbend: false,
        variable_mana_contribution: true,
        target_spell_id: cx.top.id,
        generic_mana_cost: 0,
        mana_cost: ManaCost::default(),
        undo_history_start: cx.engine.state.undoable_mana_abilities.len(),
    };
    let prompt = format!("{}: Choose how much mana to contribute.", cx.spell_label);
    cx.engine.state.pending_resolution = Some(PendingResolution {
        deciding_player: first_payer,
        presentation: PendingResolutionPresentation {
            source_object_id: cx.top.id,
            candidates: Vec::new(),
            min: 0,
            max: u32::MAX,
            ordered: false,
            prompt,
            choice_kind: custom::ChoiceKind::ManaPayment,
            unique_names: false,
        },
        continuation: ResolutionContinuation::JoinForces {
            stack: ParkedStackResolution::new(cx.top.clone()),
            payment,
            payer_order,
            next_payer: 0,
            total_paid: 0,
        },
    });
    cx.events.push(
        cx.engine
            .resolution_payment_choice_event()
            .ok_or(EngineError::Illegal(
                "Join Forces contribution prompt missing",
            ))?,
    );
    Ok(EffectOutcome::Suspended)
}

#[cfg(test)]
mod tests {
    use super::controller_first_turn_order;

    #[test]
    fn starts_with_controller_then_wraps_through_turn_order() {
        assert_eq!(
            controller_first_turn_order(vec![4, 9, 27, 31], 27),
            vec![27, 31, 4, 9]
        );
    }
}
