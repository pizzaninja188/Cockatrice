#include "../abstract_game.h"
#include "../board/card_item.h"
#include "../game_event_handler.h"
#include "../player/player.h"
#include "../player/player_actions.h"
#include "../player/player_info.h"
#include "../zones/logic/card_zone_logic.h"
#include "ruled_actions.h"
#include "ruled_payment_ui.h"
#include "ruled_pending_cast.h"

#include <libcockatrice/utility/zone_names.h>

void RuledTargetUi::ensureRefreshConnection(PlayerActions *actions)
{
    if (!actions || !actions->player || !actions->player->getGame()) {
        return;
    }
    RuledClientState *state = actions->player->getGame()->getGameEventHandler()->ruled();
    if (!state) {
        return;
    }
    QObject::connect(state, &RuledClientState::legalActionsChanged, actions,
                     &PlayerActions::reconcilePendingRuledTargetSelections, Qt::UniqueConnection);
}

void RuledTargetUi::reconcile(PlayerActions *actions)
{
    if (!actions || !actions->player || !actions->player->getGame()) {
        return;
    }
    RuledClientState *state = actions->player->getGame()->getGameEventHandler()->ruled();
    if (!state) {
        return;
    }
    const int localPlayerId = actions->player->getPlayerInfo()->getId();
    const bool spellHadTargets = !actions->pendingRuledSpellCast.selectedTargetOids.isEmpty();
    const bool abilityHadTarget = actions->pendingActivatedAbility.selectedTargetOid != 0;
    if (!reconcileRuledPendingTargets(actions->pendingRuledSpellCast, actions->pendingActivatedAbility, *state,
                                      localPlayerId)) {
        return;
    }

    auto &spell = actions->pendingRuledSpellCast;
    if (spell.valid && spell.waitingForTarget) {
        spell.inDamageAllocationMode = false;
        const auto group = currentRuledSpellTargetGroup(spell, *state);
        const QString prompt = ruledPendingSpellTargetPrompt(spell, *state);
        emit actions->ruledSpellTargetingChanged(true, prompt);
        if (group.has_value()) {
            emit actions->ruledMultiTargetSelectionUpdated(spell.selectedTargetOids.size(), group->minTargets,
                                                           ruledTargetSelectionDisplayMaximum(*group));
        }
        if (spellHadTargets) {
            state->emitLocalLog(actions->tr("A selected target is no longer legal. %1").arg(prompt));
        }
    }
    auto &ability = actions->pendingActivatedAbility;
    if (abilityHadTarget && ability.valid && ability.selectedTargetOid == 0) {
        ability.waitingForTarget = true;
        ability.waitingForMana = false;
        const QString prompt = ruledPendingAbilityTargetPrompt(ability, *state);
        emit actions->ruledActivatedAbilityTargetPendingChanged(true, prompt);
        state->emitLocalLog(actions->tr("The selected target is no longer legal. %1").arg(prompt));
    }
    state->emitSpellTargetSelectionChanged();
    state->emitSpellDamageAllocationUiChanged();
}

RuledTargetClickEligibility RuledTargetUi::cardEligibility(const PlayerActions *actions, CardItem *card)
{
    if (!actions || !actions->player || !actions->player->getGame()) {
        return RuledTargetClickEligibility::NotTargeting;
    }
    RuledClientState *state = actions->player->getGame()->getGameEventHandler()->ruled();
    if (!state || !RuledActions::isRuledGame(actions->player->getGame())) {
        return RuledTargetClickEligibility::NotTargeting;
    }
    if (actions->pendingRuledSpellCast.valid && actions->pendingRuledSpellCast.waitingForCastCostObject) {
        if (!card || !card->getZone()) {
            return RuledTargetClickEligibility::Illegal;
        }
        const QString zoneName = card->getZone()->getName();
        if (zoneName == ZoneNames::HAND) {
            Player *const handPlayer = card->getZone()->getPlayer();
            if (handPlayer != actions->player || !handPlayer->getPlayerInfo()) {
                return RuledTargetClickEligibility::Illegal;
            }
            const int handSlot =
                state->engineHandSlotForServerCard(handPlayer->getPlayerInfo()->getId(), card->getId());
            return handSlot < 0
                       ? RuledTargetClickEligibility::Illegal
                       : ruledCastCostObjectEligibility(actions->pendingRuledSpellCast,
                                                        RuledCastCostCandidateKind::Hand,
                                                        static_cast<quint32>(handSlot));
        }
        if (zoneName == ZoneNames::TABLE) {
            const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
            const quint32 oid = state->engineOidForCardId(ownerPlayerId, card->getId());
            return oid == 0 ? RuledTargetClickEligibility::Illegal
                            : ruledCastCostObjectEligibility(actions->pendingRuledSpellCast,
                                                             RuledCastCostCandidateKind::Permanent, oid);
        }
        return RuledTargetClickEligibility::Illegal;
    }
    RuledTargetCandidateKind kind = RuledTargetCandidateKind::Battlefield;
    quint32 oid = 0;
    if (card && card->getZone()) {
        const QString zoneName = card->getZone()->getName();
        const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
        if (zoneName == ZoneNames::TABLE) {
            kind = RuledTargetCandidateKind::Battlefield;
            oid = state->engineOidForCardId(ownerPlayerId, card->getId());
        } else if (zoneName == ZoneNames::STACK) {
            kind = RuledTargetCandidateKind::Stack;
            oid = state->engineOidForCardId(ownerPlayerId, card->getId());
        } else if (zoneName == ZoneNames::GRAVE) {
            kind = RuledTargetCandidateKind::Graveyard;
            oid = state->graveyardEngineOidForOwnedCard(ownerPlayerId, card->getId());
        }
    }
    const auto eligibility = ::ruledTargetClickEligibility(
        actions->pendingRuledSpellCast, actions->pendingActivatedAbility, *state, kind, oid,
        actions->player->getPlayerInfo()->getId());
    return oid == 0 && eligibility != RuledTargetClickEligibility::NotTargeting
               ? RuledTargetClickEligibility::Illegal
               : eligibility;
}

RuledTargetClickEligibility RuledTargetUi::playerEligibility(const PlayerActions *actions, Player *target)
{
    if (!actions || !actions->player || !actions->player->getGame()) {
        return RuledTargetClickEligibility::NotTargeting;
    }
    RuledClientState *state = actions->player->getGame()->getGameEventHandler()->ruled();
    if (!state || !RuledActions::isRuledGame(actions->player->getGame())) {
        return RuledTargetClickEligibility::NotTargeting;
    }
    const int targetPlayerId = target ? target->getPlayerInfo()->getId() : -1;
    const auto eligibility = ::ruledTargetClickEligibility(
        actions->pendingRuledSpellCast, actions->pendingActivatedAbility, *state, RuledTargetCandidateKind::Player,
        targetPlayerId >= 0 ? static_cast<quint32>(targetPlayerId) : 0,
        actions->player->getPlayerInfo()->getId());
    return targetPlayerId < 0 && eligibility != RuledTargetClickEligibility::NotTargeting
               ? RuledTargetClickEligibility::Illegal
               : eligibility;
}

bool RuledTargetUi::tryHandleRuledSpellTargetClick(PlayerActions *actions, CardItem *card)
{
    if (!actions->pendingRuledSpellCast.valid || !actions->pendingRuledSpellCast.waitingForTarget) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(actions->player->getGame())) {
        return true;
    }
    if (!card || !card->getZone()) {
        return true;
    }
    if (!RuledActions::isRuledGame(actions->player->getGame())) {
        actions->ruledPayment->clearPendingRuledSpellCast();
        return false;
    }

    const QString zoneName = card->getZone()->getName();
    const bool isOnBattlefield = (zoneName == ZoneNames::TABLE);
    const bool isOnStack = (zoneName == ZoneNames::STACK);
    const bool isOnGraveyard = (zoneName == ZoneNames::GRAVE);
    if (!isOnBattlefield && !isOnStack && !isOnGraveyard) {
        actions->player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            PlayerActions::tr("Select a target on the battlefield, stack, or a graveyard, or press Cancel."));
        return true;
    }

    RuledClientState *handler = actions->player->getGame()->getGameEventHandler()->ruled();

    const int ownerPlayerId = card && card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
    quint32 targetOid = 0;
    if (isOnGraveyard) {
        // Graveyard cards are tracked via the GraveyardObjectMap (not the battlefield OID map),
        // and that map is keyed by owner: Server_Card ids repeat across players' zones, so a
        // spell that can read any graveyard (Reanimate) needs the owner to disambiguate.
        targetOid = handler ? handler->graveyardEngineOidForOwnedCard(ownerPlayerId, card->getId()) : 0;
    } else {
        targetOid = handler ? handler->engineOidForCardId(ownerPlayerId, card->getId()) : 0;
    }
    if (targetOid == 0) {
        actions->player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            PlayerActions::tr("That target is not selectable yet. Select another target or cancel %1.")
                .arg(actions->pendingRuledSpellCast.cardName));
        return true;
    }
    const auto activeGroup = currentRuledSpellTargetGroup(actions->pendingRuledSpellCast, *handler);
    const bool valid =
        activeGroup.has_value() && ruledTargetDataContains(*activeGroup,
                                                           isOnBattlefield ? RuledTargetCandidateKind::Battlefield
                                                           : isOnGraveyard ? RuledTargetCandidateKind::Graveyard
                                                                           : RuledTargetCandidateKind::Stack,
                                                           targetOid, actions->player->getPlayerInfo()->getId());
    if (!valid) {
        actions->player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            PlayerActions::tr("That is not a legal target for %1.").arg(actions->pendingRuledSpellCast.cardName));
        return true;
    }
    for (const int otherGroup : activeGroup->distinctFromGroupIndices) {
        if (actions->pendingRuledSpellCast.selectedTargetOidsByGroup.value(otherGroup).contains(targetOid)) {
            handler->emitLocalLog(PlayerActions::tr("That object is already selected in a distinct target group."));
            return true;
        }
    }

    if (actions->ruledPayment->tryRequireSpellTargetCost(isOnBattlefield ? ruled::v1::TARGET_REF_KIND_PERMANENT
                                                         : isOnGraveyard ? ruled::v1::TARGET_REF_KIND_GRAVEYARD
                                                                         : ruled::v1::TARGET_REF_KIND_STACK,
                                                         targetOid, activeGroup->groupIndex))
        return true;

    if (actions->pendingRuledSpellCast.selectedTargetOids.contains(targetOid)) {
        actions->pendingRuledSpellCast.selectedTargetOids.removeOne(targetOid);
        const int chosen = actions->pendingRuledSpellCast.selectedTargetOids.size();
        actions->player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            PlayerActions::tr("Target deselected. %1 target(s) chosen for %2.")
                .arg(chosen)
                .arg(actions->pendingRuledSpellCast.cardName));
        emit actions->ruledMultiTargetSelectionUpdated(chosen, actions->pendingRuledSpellCast.minTargets,
                                                       actions->ruledPendingCast->effectiveDamageTargetsMax());
        actions->player->getGame()->getGameEventHandler()->ruled()->emitSpellTargetSelectionChanged();
        return true;
    }

    actions->pendingRuledSpellCast.selectedTargetOids.append(targetOid);

    // For DamageTargets with room for more targets, stay in targeting mode.
    // CR 601.2d: each target must receive >= 1 damage, so the true cap is the total damage (or
    // the engine's max_targets, whichever is smaller). Reaching it auto-advances to damage
    // allocation — matching Fire's fixed 2-target cap, so Fireball no longer needs a re-click.
    const int effMax = actions->ruledPendingCast->effectiveDamageTargetsMax();
    const int chosen = actions->pendingRuledSpellCast.selectedTargetOids.size();
    // effMax == 0 means "no cap" — reachable only for evenly-divided damage, where no per-target
    // minimum bounds the count. There is nothing to auto-advance on, so the player confirms
    // explicitly (click the spell again, or the Confirm Targets button).
    if (actions->pendingRuledSpellCast.maxTargets != 1 && (effMax <= 0 || chosen < effMax)) {
        actions->player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            PlayerActions::tr("Target %1%2 chosen for %3. Click another target, or click %3 again to confirm.")
                .arg(chosen)
                .arg(effMax > 0 ? QStringLiteral("/%1").arg(effMax) : QString())
                .arg(actions->pendingRuledSpellCast.cardName));
        emit actions->ruledMultiTargetSelectionUpdated(chosen, actions->pendingRuledSpellCast.minTargets, effMax);
        actions->player->getGame()->getGameEventHandler()->ruled()->emitSpellTargetSelectionChanged();
        return true;
    }

    return RuledTargetUi::finalizeTargetSelectionAndContinue(actions);
}

bool RuledTargetUi::isPlayerSelectedAsPendingSpellTarget(const PlayerActions *actions, int playerId)
{
    return actions->ruledPendingCast->isTargetSelectedForPendingSpell(static_cast<quint32>(playerId));
}

void RuledTargetUi::confirmMultiTargetSelection(PlayerActions *actions)
{
    if (!ruledPendingTargetSelectionCanConfirm(actions->pendingRuledSpellCast)) {
        return;
    }
    RuledTargetUi::finalizeTargetSelectionAndContinue(actions);
}

bool RuledTargetUi::isAwaitingRuledPlayerTargetSelection(const PlayerActions *actions)
{
    if (!actions->pendingRuledSpellCast.valid || !actions->pendingRuledSpellCast.waitingForTarget) {
        return false;
    }
    RuledClientState *handler = actions->player->getGame()->getGameEventHandler()->ruled();
    if (!handler) {
        return false;
    }
    const auto group = currentRuledSpellTargetGroup(actions->pendingRuledSpellCast, *handler);
    return group.has_value() && (group->canTargetSelf || group->canTargetOpponent);
}

bool RuledTargetUi::isAwaitingRuledAbilityOrTriggerPlayerTarget(const PlayerActions *actions)
{
    if (actions->pendingActivatedAbility.valid && actions->pendingActivatedAbility.waitingForTarget) {
        return true;
    }
    RuledClientState *handler = actions->player->getGame()->getGameEventHandler()->ruled();
    return handler && (handler->hasPendingTriggerTarget() ||
                       handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget));
}

bool RuledTargetUi::tryHandleRuledSpellTargetPlayerClick(PlayerActions *actions, Player *targetPlayer)
{
    if (!actions->pendingRuledSpellCast.valid || !actions->pendingRuledSpellCast.waitingForTarget) {
        return false;
    }
    if (RuledActions::gameplayInputLocked(actions->player->getGame())) {
        return true;
    }
    if (!targetPlayer || !RuledActions::isRuledGame(actions->player->getGame())) {
        actions->ruledPayment->clearPendingRuledSpellCast();
        return false;
    }

    if (!RuledTargetUi::isAwaitingRuledPlayerTargetSelection(actions)) {
        actions->player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            PlayerActions::tr("%1 does not target players.").arg(actions->pendingRuledSpellCast.cardName));
        return true;
    }

    const int targetPlayerId = targetPlayer->getPlayerInfo()->getId();
    if (targetPlayerId < 0) {
        return true;
    }

    RuledClientState *handler = actions->player->getGame()->getGameEventHandler()->ruled();
    const bool isSelf = (targetPlayerId == actions->player->getPlayerInfo()->getId());
    const auto activeGroup = currentRuledSpellTargetGroup(actions->pendingRuledSpellCast, *handler);
    const bool canTargetSelf = activeGroup.has_value() && activeGroup->canTargetSelf;
    const bool canTargetOpponent = activeGroup.has_value() && activeGroup->canTargetOpponent;
    if (isSelf && !canTargetSelf) {
        actions->player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            PlayerActions::tr("%1 must target an opponent.").arg(actions->pendingRuledSpellCast.cardName));
        return true;
    }
    if (!isSelf && !canTargetOpponent) {
        actions->player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            PlayerActions::tr("%1 cannot target opponents.").arg(actions->pendingRuledSpellCast.cardName));
        return true;
    }

    const quint32 targetOid = static_cast<quint32>(targetPlayerId);
    for (const int otherGroup : activeGroup->distinctFromGroupIndices) {
        if (actions->pendingRuledSpellCast.selectedTargetOidsByGroup.value(otherGroup).contains(targetOid)) {
            handler->emitLocalLog(PlayerActions::tr("That player is already selected in a distinct target group."));
            return true;
        }
    }
    if (actions->pendingRuledSpellCast.selectedTargetOids.contains(targetOid)) {
        actions->pendingRuledSpellCast.selectedTargetOids.removeOne(targetOid);
        const int chosen = actions->pendingRuledSpellCast.selectedTargetOids.size();
        actions->player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            PlayerActions::tr("Target deselected. %1 target(s) chosen for %2.")
                .arg(chosen)
                .arg(actions->pendingRuledSpellCast.cardName));
        emit actions->ruledMultiTargetSelectionUpdated(chosen, actions->pendingRuledSpellCast.minTargets,
                                                       actions->ruledPendingCast->effectiveDamageTargetsMax());
        actions->player->getGame()->getGameEventHandler()->ruled()->emitSpellTargetSelectionChanged();
        return true;
    }

    actions->pendingRuledSpellCast.selectedTargetOids.append(targetOid);

    // CR 601.2d: each target must receive >= 1 damage, so the true cap is the total damage (or
    // the engine's max_targets, whichever is smaller). Reaching it auto-advances to damage
    // allocation — matching Fire's fixed 2-target cap, so Fireball no longer needs a re-click.
    const int effMax = actions->ruledPendingCast->effectiveDamageTargetsMax();
    const int chosen = actions->pendingRuledSpellCast.selectedTargetOids.size();
    // effMax == 0 means "no cap" — reachable only for evenly-divided damage, where no per-target
    // minimum bounds the count. There is nothing to auto-advance on, so the player confirms
    // explicitly (click the spell again, or the Confirm Targets button).
    if (actions->pendingRuledSpellCast.maxTargets != 1 && (effMax <= 0 || chosen < effMax)) {
        actions->player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
            PlayerActions::tr("Target %1%2 chosen for %3. Click another target, or click %3 again to confirm.")
                .arg(chosen)
                .arg(effMax > 0 ? QStringLiteral("/%1").arg(effMax) : QString())
                .arg(actions->pendingRuledSpellCast.cardName));
        emit actions->ruledMultiTargetSelectionUpdated(chosen, actions->pendingRuledSpellCast.minTargets, effMax);
        actions->player->getGame()->getGameEventHandler()->ruled()->emitSpellTargetSelectionChanged();
        return true;
    }

    return RuledTargetUi::finalizeTargetSelectionAndContinue(actions);
}

void RuledTargetUi::loadCurrentTargetGroup(PlayerActions *actions)
{
    const auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    if (state)
        actions->ruledPendingCast->loadCurrentTargetGroup(*state);
}

bool RuledTargetUi::storeCurrentTargetGroupAndAdvance(PlayerActions *actions)
{
    auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    if (!state || !actions->ruledPendingCast->storeCurrentTargetGroupAndAdvance(*state))
        return false;
    const auto data = currentRuledSpellTargetData(actions->pendingRuledSpellCast, *state);
    const auto &next = data->groups.at(actions->pendingRuledSpellCast.activeTargetGroupPosition);
    const QString prompt = ruledPendingSpellTargetPrompt(actions->pendingRuledSpellCast, *state);
    emit actions->ruledSpellTargetingChanged(true, prompt);
    emit actions->ruledMultiTargetSelectionUpdated(actions->pendingRuledSpellCast.selectedTargetOids.size(),
                                                   next.minTargets, ruledTargetSelectionDisplayMaximum(next));
    RuledActions::updateGraveyardTargetHint(actions->player, actions->pendingRuledSpellCast.handIndex,
                                            actions->pendingRuledSpellCast.faceIndex);
    state->emitLocalLog(prompt);
    state->emitSpellTargetSelectionChanged();
    actions->player->getGameScene()->update();
    return true;
}

bool RuledTargetUi::storeCurrentModalTargetsAndAdvance(PlayerActions *actions)
{
    auto *state = actions->player->getGame()->getGameEventHandler()->ruled();
    if (!state || !actions->ruledPendingCast->storeCurrentModalTargetsAndAdvance(*state))
        return false;
    const auto &nextMode =
        actions->pendingRuledSpellCast.selectedModes.at(actions->pendingRuledSpellCast.activeModePosition);
    const QString prompt = ruledPendingSpellTargetPrompt(actions->pendingRuledSpellCast,
                                                         *actions->player->getGame()->getGameEventHandler()->ruled());
    emit actions->ruledSpellTargetingChanged(true, prompt);
    if (!nextMode.targets.groups.isEmpty()) {
        const auto &group = nextMode.targets.groups.first();
        emit actions->ruledMultiTargetSelectionUpdated(actions->pendingRuledSpellCast.selectedTargetOids.size(),
                                                       group.minTargets, ruledTargetSelectionDisplayMaximum(group));
    }
    RuledActions::updateGraveyardTargetHint(actions->player, actions->pendingRuledSpellCast.handIndex,
                                            actions->pendingRuledSpellCast.faceIndex);
    actions->player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(prompt);
    actions->player->getGame()->getGameEventHandler()->ruled()->emitSpellTargetSelectionChanged();
    actions->player->getGameScene()->update();
    return true;
}

bool RuledTargetUi::finalizeTargetSelectionAndContinue(PlayerActions *actions)
{
    if (!actions->pendingRuledSpellCast.isDamageTargets && RuledTargetUi::storeCurrentTargetGroupAndAdvance(actions)) {
        return true;
    }

    actions->pendingRuledSpellCast.waitingForTarget = false;
    emit actions->ruledSpellTargetingChanged(false, {});
    emit actions->ruledMultiTargetSelectionUpdated(0, 0, -1);
    actions->player->getGameScene()->update();

    // CR 601.2f cost increases are finalized after every group and selected mode is stored below.
    // DamageTargets: allocate damage among chosen targets interactively.
    if (actions->pendingRuledSpellCast.isDamageTargets) {
        const int total = actions->pendingRuledSpellCast.fixedDamage > 0 ? actions->pendingRuledSpellCast.fixedDamage
                                                                         : actions->pendingRuledSpellCast.xValue;
        const int numTargets = actions->pendingRuledSpellCast.selectedTargetOids.size();
        const auto allocation = actions->ruledPendingCast->prepareSpellDamageAllocation();
        if (allocation == RuledPendingCast::DamageAllocationStep::Invalid) {
            actions->player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
                PlayerActions::tr(
                    "Cannot assign at least 1 damage to each target (%1 targets, %2 total). Cast cancelled.")
                    .arg(numTargets)
                    .arg(total));
            actions->ruledPayment->clearPendingRuledSpellCast();
            return true;
        }
        if (allocation == RuledPendingCast::DamageAllocationStep::Allocating) {
            actions->player->getGame()->getGameEventHandler()->ruled()->emitSpellDamageAllocationUiChanged();
            actions->player->getGame()->getGameEventHandler()->ruled()->emitLocalLog(
                PlayerActions::tr("Assign %1 damage among %2 targets (min 1 each). "
                                  "Click to add, right-click to reduce. Confirm when done.")
                    .arg(total)
                    .arg(numTargets));
            return true; // wait for the player to confirm via the prompt button
        }
        if (RuledTargetUi::storeCurrentTargetGroupAndAdvance(actions)) {
            return true;
        }
    }

    actions->pendingRuledSpellCast.activeTargetGroupPosition = -1;

    if (RuledTargetUi::storeCurrentModalTargetsAndAdvance(actions)) {
        return true;
    }

    // CR 107.4d–f: front-load hybrid/Phyrexian choices.
    actions->ruledPayment->finalizePendingSpellManaCost();
    actions->ruledPayment->continuePendingSpellAfterChoice();
    return true;
}

int RuledTargetUi::spellDamageAllocationForPlayerId(const PlayerActions *actions, int playerId)
{
    return actions->ruledPendingCast->spellDamageAllocationForOid(static_cast<quint32>(playerId));
}

bool RuledTargetUi::tryBumpSpellDamageAllocationForOid(PlayerActions *actions, quint32 oid, int delta)
{
    const auto change = actions->ruledPendingCast->bumpSpellDamageAllocation(oid, delta);
    if (change == RuledPendingCast::DamageAllocationChange::Changed)
        actions->player->getGame()->getGameEventHandler()->ruled()->emitSpellDamageAllocationUiChanged();
    return change != RuledPendingCast::DamageAllocationChange::Unavailable;
}

bool RuledTargetUi::tryBumpSpellDamageAllocationForCard(PlayerActions *actions, CardItem *card, int delta)
{
    if (!actions->ruledPendingCast->isInSpellDamageAllocationMode() || !card)
        return false;
    RuledClientState *handler = actions->player->getGame()->getGameEventHandler()->ruled();
    if (!handler)
        return false;
    const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
    const quint32 oid = handler->engineOidForCardId(ownerPlayerId, card->getId());
    if (oid == 0)
        return false;
    return RuledTargetUi::tryBumpSpellDamageAllocationForOid(actions, oid, delta);
}

bool RuledTargetUi::tryBumpSpellDamageAllocationForPlayer(PlayerActions *actions, Player *targetPlayer, int delta)
{
    if (!actions->ruledPendingCast->isInSpellDamageAllocationMode() || !targetPlayer)
        return false;
    return RuledTargetUi::tryBumpSpellDamageAllocationForOid(
        actions, static_cast<quint32>(targetPlayer->getPlayerInfo()->getId()), delta);
}

void RuledTargetUi::confirmSpellDamageAllocation(PlayerActions *actions)
{
    if (!actions->ruledPendingCast->confirmSpellDamageAllocation())
        return;
    actions->player->getGame()->getGameEventHandler()->ruled()->emitSpellDamageAllocationUiChanged();

    if (RuledTargetUi::storeCurrentTargetGroupAndAdvance(actions))
        return;
    actions->pendingRuledSpellCast.activeTargetGroupPosition = -1;
    if (RuledTargetUi::storeCurrentModalTargetsAndAdvance(actions))
        return;
    actions->ruledPayment->finalizePendingSpellManaCost();
    actions->ruledPayment->continuePendingSpellAfterChoice();
}

bool RuledTargetUi::tryHandleRuledAbilityTargetClick(PlayerActions *actions, CardItem *card)
{
    RuledClientState *handler = actions->player->getGame()->getGameEventHandler()->ruled();
    if (handler && handler->isEngineCommandPending()) {
        return true;
    }

    if (actions->ruledPayment->tryHandlePriorityCostClick(card))
        return true;

    // CR 614.12 / 707.5: Clone's entering-as-copy choice is untargeted but uses the existing
    // engine-authoritative board click path.
    if (handler && handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::PermanentChoice)) {
        if (!card || !card->getZone()) {
            return false;
        }
        if (card->getZone()->getName() != ZoneNames::TABLE) {
            handler->emitLocalLog(PlayerActions::tr("Choose a permanent on the battlefield."));
            return true;
        }
        const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
        const quint32 chosenOid = handler->engineOidForCardId(ownerPlayerId, card->getId());
        if (chosenOid == 0 ||
            !handler->isPendingChoiceCandidate(RuledClientState::ChoiceKind::PermanentChoice, chosenOid)) {
            handler->emitLocalLog(PlayerActions::tr("That permanent is not a legal choice."));
            return true;
        }
        handler->submitPendingChoiceObject(chosenOid);
        return true;
    }

    if (handler && handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopySource)) {
        if (!card || !card->getZone()) {
            return false;
        }
        if (card->getZone()->getName() != ZoneNames::TABLE) {
            handler->emitLocalLog(PlayerActions::tr("Choose a creature on the battlefield for Clone to copy."));
            return true;
        }
        const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
        const quint32 sourceOid = handler->engineOidForCardId(ownerPlayerId, card->getId());
        if (sourceOid == 0 || !handler->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopySource, sourceOid)) {
            handler->emitLocalLog(PlayerActions::tr("That is not a creature Clone can copy."));
            return true;
        }
        handler->submitPendingChoiceObject(sourceOid);
        return true;
    }

    if (handler && handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::AuraPermanent)) {
        if (!card || !card->getZone()) {
            return false;
        }
        if (card->getZone()->getName() != ZoneNames::TABLE) {
            handler->emitLocalLog(PlayerActions::tr("Choose a permanent on the battlefield for the Aura to enchant."));
            return true;
        }
        const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
        const quint32 recipientOid = handler->engineOidForCardId(ownerPlayerId, card->getId());
        if (recipientOid == 0 ||
            !handler->isPendingChoiceCandidate(RuledClientState::ChoiceKind::AuraPermanent, recipientOid)) {
            handler->emitLocalLog(PlayerActions::tr("That permanent cannot be enchanted by the returning Aura."));
            return true;
        }
        handler->submitPendingChoiceObject(recipientOid);
        return true;
    }

    // Check pending copy target choice first (CR 707.10c: redirect targets for a spell copy).
    if (handler && handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget)) {
        if (!card || !card->getZone()) {
            return false;
        }
        const QString zoneName = card->getZone()->getName();
        if (zoneName != ZoneNames::TABLE && zoneName != ZoneNames::STACK) {
            handler->emitLocalLog(PlayerActions::tr("Select a target on the battlefield or stack for the copy."));
            return true;
        }
        const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
        const quint32 targetOid = handler->engineOidForCardId(ownerPlayerId, card->getId());
        if (targetOid == 0 || !handler->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopyTarget, targetOid)) {
            handler->emitLocalLog(PlayerActions::tr("That is not a valid target for the copy."));
            return true;
        }
        handler->submitPendingChoiceObject(targetOid);
        return true;
    }

    // Check pending legend-rule keep choice (CR 704.5j: click the legend to keep on the battlefield).
    if (handler && handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::LegendKeep)) {
        if (!card || !card->getZone()) {
            return false;
        }
        if (card->getZone()->getName() != ZoneNames::TABLE) {
            handler->emitLocalLog(PlayerActions::tr("Click the legendary permanent to keep on the battlefield."));
            return true;
        }
        const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
        const quint32 keepOid = handler->engineOidForCardId(ownerPlayerId, card->getId());
        if (keepOid == 0 || !handler->isPendingChoiceCandidate(RuledClientState::ChoiceKind::LegendKeep, keepOid)) {
            handler->emitLocalLog(PlayerActions::tr("That is not one of the legends you must choose between."));
            return true;
        }
        handler->submitPendingChoiceObject(keepOid);
        return true;
    }

    // Check pending trigger first (higher priority).
    if (handler && handler->hasPendingTriggerTarget()) {
        if (!card || !card->getZone()) {
            return false;
        }
        const QString zoneName = card->getZone()->getName();
        const bool triggerIsGraveyard = (zoneName == ZoneNames::GRAVE);
        if (zoneName != ZoneNames::TABLE && zoneName != ZoneNames::STACK && !triggerIsGraveyard) {
            return false;
        }
        const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
        quint32 targetOid = 0;
        if (triggerIsGraveyard) {
            targetOid = handler->graveyardEngineOidForOwnedCard(ownerPlayerId, card->getId());
        } else {
            targetOid = handler->engineOidForCardId(ownerPlayerId, card->getId());
        }
        if (targetOid == 0) {
            return false;
        }
        const auto kind = triggerIsGraveyard             ? ruled::v1::TARGET_REF_KIND_GRAVEYARD
                          : zoneName == ZoneNames::STACK ? ruled::v1::TARGET_REF_KIND_STACK
                                                         : ruled::v1::TARGET_REF_KIND_PERMANENT;
        if (!handler->stagePendingTriggerTarget(kind, targetOid, ownerPlayerId)) {
            handler->emitLocalLog(PlayerActions::tr("That is not a legal target for the current target group."));
        }
        return true;
    }

    if (actions->ruledPayment->tryHandleAdditionalCostClick(card))
        return true;

    // Check pending activated ability target.
    if (!actions->pendingActivatedAbility.valid || !actions->pendingActivatedAbility.waitingForTarget) {
        return false;
    }
    if (!card || !card->getZone()) {
        return false;
    }
    const QString zoneName = card->getZone()->getName();
    const bool isGraveyard = zoneName == ZoneNames::GRAVE;
    if (zoneName != ZoneNames::TABLE && zoneName != ZoneNames::STACK && !isGraveyard) {
        return true;
    }
    const int ownerPlayerId = card->getOwner() ? card->getOwner()->getPlayerInfo()->getId() : -1;
    const quint32 targetOid = isGraveyard ? handler->graveyardEngineOidForOwnedCard(ownerPlayerId, card->getId())
                                          : handler->engineOidForCardId(ownerPlayerId, card->getId());
    if (targetOid == 0) {
        return true;
    }
    const RuledTargetCandidateKind kind = isGraveyard                    ? RuledTargetCandidateKind::Graveyard
                                          : zoneName == ZoneNames::STACK ? RuledTargetCandidateKind::Stack
                                                                         : RuledTargetCandidateKind::Battlefield;
    const auto targetData = handler->abilityTargetData(actions->pendingActivatedAbility.permanentOid,
                                                       actions->pendingActivatedAbility.abilityIndex);
    if (!ruledTargetDataContains(targetData, kind, targetOid, actions->player->getPlayerInfo()->getId())) {
        return true;
    }

    actions->pendingActivatedAbility.selectedTargetOid = targetOid;
    actions->pendingActivatedAbility.waitingForTarget = false;
    emit actions->ruledActivatedAbilityTargetPendingChanged(false, {});
    actions->ruledPayment->continuePendingActivatedAbilityAfterChoice();
    return true;
}

bool RuledTargetUi::tryHandleRuledAbilityTargetPlayerClick(PlayerActions *actions, Player *targetPlayer)
{
    RuledClientState *handler = actions->player->getGame()->getGameEventHandler()->ruled();
    if (handler && handler->isEngineCommandPending()) {
        return true;
    }

    if (handler && handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::AttackingTokenDefender)) {
        if (!targetPlayer) {
            return false;
        }
        const int playerId = targetPlayer->getPlayerInfo()->getId();
        if (!handler->isLegalAttackPlayerDefender(playerId)) {
            handler->emitLocalLog(PlayerActions::tr("That player is not a legal defender for the entering token."));
            return true;
        }
        handler->chooseAttackPlayerDefender(playerId);
        return true;
    }

    // Check pending copy target choice first (CR 707.10c: redirect targets for a spell copy).
    if (handler && handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget)) {
        if (!targetPlayer) {
            return false;
        }
        const quint32 targetOid = static_cast<quint32>(targetPlayer->getPlayerInfo()->getId());
        if (!handler->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopyTarget, targetOid)) {
            handler->emitLocalLog(PlayerActions::tr("That player is not a valid target for the copy."));
            return true;
        }
        handler->submitPendingChoiceObject(targetOid);
        return true;
    }

    if (handler && handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::AuraPlayer)) {
        if (!targetPlayer) {
            return false;
        }
        const quint32 playerId = static_cast<quint32>(targetPlayer->getPlayerInfo()->getId());
        if (!handler->isPendingChoiceCandidate(RuledClientState::ChoiceKind::AuraPlayer, playerId)) {
            handler->emitLocalLog(PlayerActions::tr("That player cannot be enchanted by the returning Aura."));
            return true;
        }
        handler->submitPendingChoiceObject(playerId);
        return true;
    }

    if (handler && handler->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::BattleProtector)) {
        if (!targetPlayer) {
            return false;
        }
        const quint32 playerId = static_cast<quint32>(targetPlayer->getPlayerInfo()->getId());
        if (!handler->isPendingChoiceCandidate(RuledClientState::ChoiceKind::BattleProtector, playerId)) {
            handler->emitLocalLog(PlayerActions::tr("That player cannot protect this Battle."));
            return true;
        }
        handler->submitPendingChoiceObject(playerId);
        return true;
    }

    if (handler && handler->hasPendingTriggerTarget()) {
        if (!targetPlayer) {
            return false;
        }
        const quint32 targetOid = static_cast<quint32>(targetPlayer->getPlayerInfo()->getId());
        if (!handler->stagePendingTriggerTarget(ruled::v1::TARGET_REF_KIND_PLAYER, targetOid,
                                                targetPlayer->getPlayerInfo()->getId())) {
            handler->emitLocalLog(PlayerActions::tr("That player is not a legal target for the current target group."));
        }
        return true;
    }

    if (!actions->pendingActivatedAbility.valid || !actions->pendingActivatedAbility.waitingForTarget) {
        return false;
    }
    if (!targetPlayer) {
        return false;
    }
    const quint32 targetOid = static_cast<quint32>(targetPlayer->getPlayerInfo()->getId());
    const quint32 permOid = actions->pendingActivatedAbility.permanentOid;
    const int abilityIdx = actions->pendingActivatedAbility.abilityIndex;
    if (!ruledTargetDataContains(handler->abilityTargetData(permOid, abilityIdx), RuledTargetCandidateKind::Player,
                                 targetOid, actions->player->getPlayerInfo()->getId())) {
        return true;
    }
    actions->pendingActivatedAbility.selectedTargetOid = targetOid;
    actions->pendingActivatedAbility.waitingForTarget = false;
    emit actions->ruledActivatedAbilityTargetPendingChanged(false, {});
    actions->ruledPayment->continuePendingActivatedAbilityAfterChoice();
    return true;
}
