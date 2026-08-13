#include "ruled_client_state.h"

#include "ruled_client_host.h"

#include <algorithm>
#include <QTimer>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

RuledClientState::RuledClientState(RuledClientHost *_host, QObject *parent) : QObject(parent), host(_host)
{
}

bool RuledClientState::beginEngineCommand()
{
    if (engineCommandPending) {
        return false;
    }
    engineCommandPending = true;
    engineCommandIndicatorVisible = false;
    const quint64 generation = ++engineCommandGeneration;
    emit engineCommandPendingUiChanged();
    QTimer::singleShot(150, this, [this, generation] {
        if (engineCommandPending && engineCommandGeneration == generation) {
            showEngineCommandIndicator();
        }
    });
    return true;
}

void RuledClientState::showEngineCommandIndicator()
{
    if (!engineCommandPending || engineCommandIndicatorVisible) {
        return;
    }
    engineCommandIndicatorVisible = true;
    emit engineCommandPendingUiChanged();
}

void RuledClientState::finishEngineCommand()
{
    ++engineCommandGeneration;
    if (!engineCommandPending && !engineCommandIndicatorVisible) {
        return;
    }
    engineCommandPending = false;
    engineCommandIndicatorVisible = false;
    emit engineCommandPendingUiChanged();
}

// ---------------------------------------------------------------------------------------
// Identity
// ---------------------------------------------------------------------------------------

void RuledClientState::setPendingCastGraveyardTargets(const QSet<quint32> &oids)
{
    if (pendingCastGraveyardOids == oids) {
        return;
    }
    pendingCastGraveyardOids = oids;
    emitGraveyardTargetsNeeded();
}

void RuledClientState::emitGraveyardTargetsNeeded()
{
    // Union of the three things that can want a graveyard open. All go through here so the "which
    // views are open" decision has a single owner — the alternative is several signals racing to
    // open and close the same widget.
    //
    // 1. the pending cast's legal targets (choosing a target for Raise Dead / Reanimate);
    QSet<quint32> oids = pendingCastGraveyardOids;
    // 2. a pending trigger's legal targets (Gravedigger ETB);
    if (hasPendingTriggerTarget()) {
        const quint64 abilityKey = abilityTargetKey(lastTriggerSourceOid, static_cast<int>(lastTriggerAbilityIndex));
        const auto &triggerTargets = validTargetsByAbility.value(abilityKey).validGraveyardIds;
        for (quint32 oid : triggerTargets) {
            oids.insert(oid);
        }
    }
    // 3. the graveyard targets of anything still *on the stack*. A spell that has been cast keeps
    //    its graveyard open until it resolves or is countered, so the targeting arrow stays
    //    anchored to the card rather than to a pile the player can no longer see. Falls out of the
    //    stack order automatically the moment the spell leaves the stack.
    //
    //    Read from the latch, never from the live graveyard map: a permanent that dies while an
    //    ability targeting it is on the stack lands in that map, and testing it would pop both
    //    players' graveyard views open for an ability that never targeted a graveyard at all.
    //    Only a target chosen *in* a graveyard (Reanimate) is latched Graveyard — see
    //    `RuledEventDispatcher::applyStackPushed`.
    const QList<quint32> stackOrder = getStackOidOrder();
    for (auto it = stackTargetsByStackOid.constBegin(); it != stackTargetsByStackOid.constEnd(); ++it) {
        if (!stackOrder.contains(it.key())) {
            continue;
        }
        for (quint32 target : it.value()) {
            if (latchedTargetKind(it.key(), target) == RuledTargetItemKind::Graveyard) {
                oids.insert(target);
            }
        }
    }

    QList<int> playerIds;
    for (quint32 oid : oids) {
        const int pid = graveyardOidToPlayerId.value(oid, -1);
        if (pid >= 0 && !playerIds.contains(pid)) {
            playerIds.append(pid);
        }
    }
    // Deterministic order so the views open the same way every time (QSet iteration is not).
    std::sort(playerIds.begin(), playerIds.end());
    emit graveyardTargetsNeeded(playerIds);
}

// ---------------------------------------------------------------------------------------
// Legal hand actions
// ---------------------------------------------------------------------------------------

const RuledHandActionSet &RuledClientState::handActionSet(RuledHandActionKind kind) const
{
    static const RuledHandActionSet empty;
    const auto it = handActions.constFind(kind);
    return it == handActions.constEnd() ? empty : *it;
}

bool RuledClientState::isHandActionLegal(RuledHandActionKind kind, int handIndex) const
{
    return handActionSet(kind).handIndices.contains(handIndex);
}

QList<int> RuledClientState::handActionLegalIndicesSorted(RuledHandActionKind kind) const
{
    QList<int> legal = handActionSet(kind).handIndices.values();
    std::sort(legal.begin(), legal.end());
    return legal;
}

QList<int> RuledClientState::handActionIndicesForCardName(RuledHandActionKind kind, const QString &cardName) const
{
    QList<int> matching = handActionSet(kind).indicesByCardName.values(cardName);
    std::sort(matching.begin(), matching.end());
    return matching;
}

QList<int> RuledClientState::handActionClickCandidates(RuledHandActionKind kind, const QString &cardName) const
{
    if (kind == ruled::v1::HAND_ACTION_CLEANUP_DISCARD || kind == ruled::v1::HAND_ACTION_OPENING_BOTTOM) {
        return handActionLegalIndicesSorted(kind);
    }
    return handActionIndicesForCardName(kind, cardName);
}

int RuledClientState::handActionIndexForCard(RuledHandActionKind kind,
                                             const QString &cardName,
                                             int preferredHandIndex) const
{
    const QList<int> matching = handActionIndicesForCardName(kind, cardName);
    if (matching.contains(preferredHandIndex)) {
        return preferredHandIndex;
    }
    if (matching.isEmpty()) {
        return -1;
    }
    return matching.first();
}

QVector<RuledFaceOption> RuledClientState::handActionFaceOptions(RuledHandActionKind kind, int handIndex) const
{
    QVector<RuledFaceOption> options = handActionSet(kind).faceOptionsByIndex.value(handIndex);
    std::sort(options.begin(), options.end(),
              [](const RuledFaceOption &a, const RuledFaceOption &b) { return a.faceIndex < b.faceIndex; });
    return options;
}

bool RuledClientState::handActionNeedsTarget(RuledHandActionKind kind, int handIndex, int faceIndex) const
{
    return handActionSet(kind).needsTargetCastKeys.contains(spellTargetKey(handIndex, faceIndex));
}

void RuledClientState::clearHandActions()
{
    handActions.clear();
}

bool RuledClientState::localPlayerMustCleanupDiscard() const
{
    return !handActionSet(ruled::v1::HAND_ACTION_CLEANUP_DISCARD).handIndices.isEmpty();
}

int RuledClientState::cleanupDiscardRequiredCount() const
{
    const int n = handActionSet(ruled::v1::HAND_ACTION_CLEANUP_DISCARD).handIndices.size();
    if (n <= 7) {
        return 0;
    }
    return n - 7;
}

int RuledClientState::cleanupDiscardSelectedCount() const
{
    return cleanupDiscardSelectedIndices.size();
}

bool RuledClientState::isCleanupDiscardHandIndexSelected(int handIndex) const
{
    return cleanupDiscardSelectedIndices.contains(handIndex);
}

QList<int> RuledClientState::cleanupDiscardSelectedIndicesSorted() const
{
    QList<int> out;
    out.reserve(cleanupDiscardSelectedIndices.size());
    for (int x : cleanupDiscardSelectedIndices) {
        out.append(x);
    }
    std::sort(out.begin(), out.end());
    return out;
}

void RuledClientState::toggleCleanupDiscardHandIndex(int ruledHandIndex)
{
    if (!isHandActionLegal(ruled::v1::HAND_ACTION_CLEANUP_DISCARD, ruledHandIndex)) {
        return;
    }
    const int need = cleanupDiscardRequiredCount();
    if (need <= 0) {
        return;
    }
    if (cleanupDiscardSelectedIndices.contains(ruledHandIndex)) {
        cleanupDiscardSelectedIndices.remove(ruledHandIndex);
    } else if (cleanupDiscardSelectedIndices.size() < need) {
        cleanupDiscardSelectedIndices.insert(ruledHandIndex);
    }
    emit cleanupDiscardUiChanged(need, cleanupDiscardSelectedIndices.size());
    emit combatStateChanged();
}

void RuledClientState::clearCleanupDiscardSelection(bool emitUiChange)
{
    if (cleanupDiscardSelectedIndices.isEmpty()) {
        return;
    }
    cleanupDiscardSelectedIndices.clear();
    if (emitUiChange) {
        emit cleanupDiscardUiChanged(cleanupDiscardRequiredCount(), 0);
        emit combatStateChanged();
    }
}

void RuledClientState::pruneCleanupDiscardSelectionAndEmitUi()
{
    const QSet<int> &legal = handActionSet(ruled::v1::HAND_ACTION_CLEANUP_DISCARD).handIndices;
    if (legal.isEmpty()) {
        cleanupDiscardSelectedIndices.clear();
        emit cleanupDiscardUiChanged(0, 0);
        emit combatStateChanged();
        return;
    }
    for (auto it = cleanupDiscardSelectedIndices.begin(); it != cleanupDiscardSelectedIndices.end();) {
        if (!legal.contains(*it)) {
            it = cleanupDiscardSelectedIndices.erase(it);
        } else {
            ++it;
        }
    }
    emit cleanupDiscardUiChanged(cleanupDiscardRequiredCount(), cleanupDiscardSelectedIndices.size());
    emit combatStateChanged();
}

// ---------------------------------------------------------------------------------------
// Opening sequence
// ---------------------------------------------------------------------------------------

int RuledClientState::openingBottomRequiredCount() const
{
    return openingMulliganCount;
}

int RuledClientState::openingBottomSelectedCount() const
{
    return openingBottomSelectedIndices.size();
}

bool RuledClientState::isOpeningBottomHandIndexSelected(int handIndex) const
{
    return openingBottomSelectedIndices.contains(handIndex);
}

int RuledClientState::openingBottomClickOrderFor(int handIndex) const
{
    const int pos = openingBottomSelectedIndices.indexOf(handIndex);
    return pos + 1; // 1-based; returns 0 if not selected
}

void RuledClientState::toggleOpeningBottomHandIndex(int ruledHandIndex)
{
    if (!isHandActionLegal(ruled::v1::HAND_ACTION_OPENING_BOTTOM, ruledHandIndex)) {
        return;
    }
    const int need = openingBottomRequiredCount();
    if (need <= 0) {
        return;
    }
    if (openingBottomSelectedIndices.contains(ruledHandIndex)) {
        openingBottomSelectedIndices.removeOne(ruledHandIndex);
    } else if (openingBottomSelectedIndices.size() < need) {
        openingBottomSelectedIndices.append(ruledHandIndex);
    }
    emit openingBottomUiChanged(need, openingBottomSelectedIndices.size());
    emit combatStateChanged();
}

void RuledClientState::clearOpeningBottomSelection(bool emitUiChange)
{
    if (openingBottomSelectedIndices.isEmpty()) {
        return;
    }
    openingBottomSelectedIndices.clear();
    if (emitUiChange) {
        emit openingBottomUiChanged(openingBottomRequiredCount(), 0);
        emit combatStateChanged();
    }
}

// ---------------------------------------------------------------------------------------
// Pending choices — one holder, one teardown path, one submission path
// ---------------------------------------------------------------------------------------

void RuledClientState::teardownPendingChoice()
{
    if (!pendingChoice.has_value()) {
        return;
    }
    // A Revealed pick owns a popup window; tell the tab to close it before the state goes away.
    if (pendingChoice->kind == ChoiceKind::ResolutionPick && pendingChoice->pickZone == PickZone::Revealed) {
        emit revealedPickChanged(false, {}, {}, 0, 0);
    }
    pendingChoice.reset();
}

void RuledClientState::setPendingChoice(RuledPendingChoice choice)
{
    teardownPendingChoice();
    pendingChoice = std::move(choice);
}

void RuledClientState::clearPendingChoice()
{
    teardownPendingChoice();
}

void RuledClientState::clearPendingChoiceOfKind(ChoiceKind kind)
{
    if (hasPendingChoiceOfKind(kind)) {
        teardownPendingChoice();
    }
}

void RuledClientState::sendResolutionChoice(const QVector<quint32> &chosenOids,
                                            ruled::v1::ResolutionChoiceDecision decision)
{
    ruled::v1::RuledCommand cmd;
    auto *sub = cmd.mutable_submit_resolution_choice();
    for (const quint32 oid : chosenOids) {
        sub->add_chosen_object_ids(oid);
    }
    sub->set_decision(decision);
    host->sendRuledCommand(cmd);
}

void RuledClientState::payResolutionMana()
{
    if (resolutionPaymentCurrentlyLegal()) {
        submitResolutionPayment(ruled::v1::RESOLUTION_CHOICE_DECISION_PAY_MANA);
    }
}

void RuledClientState::declineResolutionMana()
{
    if (isResolutionPaymentActive()) {
        submitResolutionPayment(ruled::v1::RESOLUTION_CHOICE_DECISION_DECLINE);
    }
}

void RuledClientState::submitResolutionPayment(ruled::v1::ResolutionChoiceDecision decision)
{
    if (!isResolutionPaymentActive()) {
        return;
    }
    const RuledPendingChoice restore = *pendingChoice;
    clearPendingChoiceOfKind(ChoiceKind::ResolutionPayment);
    emit resolutionPaymentUiChanged(false);
    emit combatStateChanged();

    ruled::v1::RuledCommand command;
    command.mutable_submit_resolution_choice()->set_decision(decision);
    host->sendRuledCommandExpectingAck(command, [this, restore](bool accepted) {
        emit resolutionPaymentSubmissionFinished(accepted);
        if (!accepted && !pendingChoice.has_value()) {
            setPendingChoice(restore);
            emit resolutionPaymentUiChanged(true);
            emit combatStateChanged();
        }
    });
}

void RuledClientState::submitPendingChoiceObject(quint32 oid)
{
    clearPendingChoice();
    sendResolutionChoice({oid});
}

void RuledClientState::declinePendingClickChoice()
{
    if (!pendingClickChoiceMayDecline()) {
        return;
    }
    const ChoiceKind kind = pendingChoice->kind;
    if (kind == ChoiceKind::CopySource) {
        clearPendingChoiceOfKind(kind);
        sendResolutionChoice({});
        return;
    }
    if (kind != ChoiceKind::TriggerTarget) {
        return;
    }
    ruled::v1::RuledCommand command;
    command.mutable_choose_trigger_target()->set_decline(true);
    clearPendingChoiceOfKind(kind);
    host->sendRuledCommand(command);
}

// ---------------------------------------------------------------------------------------
// Tier-3 resolution pick
// ---------------------------------------------------------------------------------------

bool RuledClientState::isResolutionHandPickCardSelectable(int serverCardId) const
{
    if (!isResolutionHandPickActive()) {
        return false;
    }
    if (!pendingChoice->serverCardIdToOid.contains(serverCardId)) {
        return false;
    }
    // Already selected: always show its highlight/number.
    if (pendingChoice->selectedServerCardIds.contains(serverCardId)) {
        return true;
    }
    // When unique-names is on, exclude candidates whose name is already taken by a
    // different selected card — they lose the faint outline and become unclickable.
    if (pendingChoice->uniqueNames) {
        const QString &name = pendingChoice->serverCardIdToName.value(serverCardId);
        for (int selId : pendingChoice->selectedServerCardIds) {
            if (pendingChoice->serverCardIdToName.value(selId) == name) {
                return false;
            }
        }
    }
    return true;
}

int RuledClientState::resolutionHandPickClickOrderFor(int serverCardId) const
{
    if (!isResolutionHandPickActive()) {
        return 0;
    }
    const int pos = pendingChoice->selectedServerCardIds.indexOf(serverCardId);
    return pos + 1;
}

QVector<int> RuledClientState::resolutionHandPickCandidateServerCardIds() const
{
    if (!isResolutionHandPickActive()) {
        return {};
    }
    const QList<int> keys = pendingChoice->serverCardIdToOid.keys();
    return QVector<int>(keys.begin(), keys.end());
}

void RuledClientState::toggleResolutionHandPickCard(int serverCardId)
{
    if (!isResolutionHandPickActive()) {
        return;
    }
    if (!pendingChoice->serverCardIdToOid.contains(serverCardId)) {
        return;
    }
    const int pos = pendingChoice->selectedServerCardIds.indexOf(serverCardId);
    if (pos >= 0) {
        pendingChoice->selectedServerCardIds.removeAt(pos);
    } else if (pendingChoice->selectedServerCardIds.size() < pendingChoice->max) {
        if (pendingChoice->uniqueNames) {
            const QString clickedName = pendingChoice->serverCardIdToName.value(serverCardId);
            bool nameTaken = false;
            for (int selId : pendingChoice->selectedServerCardIds) {
                if (pendingChoice->serverCardIdToName.value(selId) == clickedName) {
                    nameTaken = true;
                    break;
                }
            }
            if (nameTaken) {
                return;
            }
        }
        pendingChoice->selectedServerCardIds.append(serverCardId);
    }
    emit resolutionHandPickUiChanged(pendingChoice->min, pendingChoice->selectedServerCardIds.size());
    emit combatStateChanged();
}

void RuledClientState::submitResolutionHandPick()
{
    if (!isResolutionHandPickActive()) {
        return;
    }
    const int n = pendingChoice->selectedServerCardIds.size();
    if (n < pendingChoice->min || n > pendingChoice->max) {
        return;
    }
    QVector<quint32> chosen;
    chosen.reserve(n);
    for (int scid : pendingChoice->selectedServerCardIds) {
        const quint32 oid = pendingChoice->serverCardIdToOid.value(scid, 0);
        if (oid != 0) {
            chosen.append(oid);
        }
    }
    clearPendingChoice();
    emit resolutionHandPickUiChanged(-1, -1);
    emit resolutionPaymentUiChanged(false);
    emit combatStateChanged();
    sendResolutionChoice(chosen);
}

// ---------------------------------------------------------------------------------------
// Simultaneous trigger ordering (CR 603.3b)
// ---------------------------------------------------------------------------------------

void RuledClientState::pickTriggerOrderCard(int serverCardId)
{
    if (!isTriggerOrderPickCard(serverCardId)) {
        return;
    }
    submitTriggerOrder(pendingChoice->orderCardIdToOid.value(serverCardId));
}

void RuledClientState::submitTriggerOrder(quint32 triggerOid)
{
    if (!hasPendingTriggerOrder()) {
        return;
    }
    ruled::v1::RuledCommand cmd;
    cmd.mutable_submit_trigger_order()->set_trigger_object_id(triggerOid);
    // The choice is cleared so a second click during the round trip sends nothing the engine is
    // about to refuse — but deliberately *without* announcing a UI change. The popup stays up,
    // showing the old cards for the moment it takes the reply to arrive, and the batch's single
    // triggerOrderUiChanged then either refreshes it with what remains or closes it.
    clearPendingChoice();
    host->sendRuledCommand(cmd);
}

// ---------------------------------------------------------------------------------------
// Turn / phase roles
// ---------------------------------------------------------------------------------------

bool RuledClientState::localPlayerIsActive() const
{
    const int localId = host->localPlayerId();
    if (localId < 0 || currentActivePlayerId < 0) {
        return false;
    }
    if (currentCombatPhase == RuledCombatPhase::DeclareAttackers) {
        return localId == currentActivePlayerId && !attackersSubmittedThisStep;
    }
    return localId == currentActivePlayerId;
}

bool RuledClientState::localPlayerIsDefender() const
{
    const int localId = host->localPlayerId();
    if (localId < 0 || currentActivePlayerId < 0) {
        return false;
    }
    if (currentCombatPhase == RuledCombatPhase::DeclareBlockers) {
        return localId != currentActivePlayerId && !blockersSubmittedThisStep;
    }
    return localId != currentActivePlayerId;
}

bool RuledClientState::combatDeclarationSatisfied() const
{
    // CR 508.1d: every must-attack creature must be among the staged attackers.
    if (currentCombatPhase == RuledCombatPhase::DeclareAttackers) {
        for (const quint32 oid : requiredAttackerOids) {
            if (!pendingAttackerOids.contains(oid)) {
                return false;
            }
        }
        return true;
    }
    // CR 509.1c: every must-block creature must be staged as a blocker of some attacker. The engine
    // still validates the specific pairing (evasion); here we only require it be blocking something.
    if (currentCombatPhase == RuledCombatPhase::DeclareBlockers) {
        for (const quint32 oid : requiredBlockerOids) {
            if (!pendingBlocks.contains(oid) && !committedBlocks.contains(oid)) {
                return false;
            }
        }
        return true;
    }
    return true;
}

// ---------------------------------------------------------------------------------------
// Combat staging
// ---------------------------------------------------------------------------------------

void RuledClientState::togglePendingAttacker(quint32 engineOid)
{
    if (engineOid == 0) {
        return;
    }
    if (pendingAttackerOids.contains(engineOid)) {
        pendingAttackerOids.remove(engineOid);
    } else if (selectableAttackerOids.contains(engineOid)) {
        pendingAttackerOids.insert(engineOid);
    } else {
        return;
    }
    syncAttackersPreviewToServer();
    emit combatStateChanged();
}

void RuledClientState::clearPendingAttackers()
{
    if (pendingAttackerOids.isEmpty()) {
        return;
    }
    pendingAttackerOids.clear();
    syncAttackersPreviewToServer();
    emit combatStateChanged();
}

void RuledClientState::toggleStagedBlocker(quint32 blockerOid)
{
    if (stagedBlockerOids.contains(blockerOid)) {
        stagedBlockerOids.remove(blockerOid);
    } else if (isSelectableBlocker(blockerOid)) {
        stagedBlockerOids.insert(blockerOid);
    } else {
        return;
    }
    emit combatStateChanged();
}

void RuledClientState::clearStagedBlockers()
{
    if (stagedBlockerOids.isEmpty()) {
        return;
    }
    stagedBlockerOids.clear();
    emit combatStateChanged();
}

void RuledClientState::pairStagedBlockerToAttacker(quint32 attackerOid)
{
    if (stagedBlockerOids.isEmpty() || attackerOid == 0) {
        return;
    }
    if (!currentAttackerOids.contains(attackerOid)) {
        return;
    }
    // Pairing is all-or-nothing: do not silently move only the legal subset of a multi-blocker
    // staging selection. The engine publishes this exact relation from the same predicate used by
    // DeclareBlockers, including attacker-side restrictions such as "can't be blocked."
    for (quint32 blockerOid : std::as_const(stagedBlockerOids)) {
        if (!isLegalBlockPair(blockerOid, attackerOid)) {
            return;
        }
    }
    for (quint32 blockerOid : std::as_const(stagedBlockerOids)) {
        pendingBlocks.insert(blockerOid, attackerOid);
    }
    stagedBlockerOids.clear();
    syncBlockersPreviewToServer();
    emit combatStateChanged();
}

void RuledClientState::clearPendingBlocks()
{
    if (pendingBlocks.isEmpty() && stagedBlockerOids.isEmpty() && committedBlocks.isEmpty()) {
        return;
    }
    pendingBlocks.clear();
    committedBlocks.clear();
    stagedBlockerOids.clear();
    syncBlockersPreviewToServer();
    emit combatStateChanged();
}

void RuledClientState::syncAttackersPreviewToServer()
{
    if (currentCombatPhase != RuledCombatPhase::DeclareAttackers) {
        return;
    }
    if (attackersSubmittedThisStep) {
        return;
    }
    const int localId = host->localPlayerId();
    if (localId < 0 || currentActivePlayerId < 0) {
        return;
    }
    if (localId != currentActivePlayerId) {
        return;
    }

    ruled::v1::RuledCommand ruledCommand;
    auto *preview = ruledCommand.mutable_preview_declare_attackers();
    for (const quint32 oid : pendingAttackerOids) {
        preview->add_creature_ids(oid);
    }
    host->sendRuledCommand(ruledCommand);
}

void RuledClientState::syncBlockersPreviewToServer()
{
    if (currentCombatPhase != RuledCombatPhase::DeclareBlockers) {
        return;
    }
    if (blockersSubmittedThisStep) {
        return;
    }
    const int localId = host->localPlayerId();
    if (localId < 0 || currentActivePlayerId < 0) {
        return;
    }
    if (localId == currentActivePlayerId) {
        return;
    }

    ruled::v1::RuledCommand ruledCommand;
    auto *preview = ruledCommand.mutable_preview_declare_blockers();
    for (auto it = pendingBlocks.constBegin(); it != pendingBlocks.constEnd(); ++it) {
        auto *pair = preview->add_block_pairs();
        pair->set_blocker_id(it.key());
        pair->set_attacker_id(it.value());
    }
    host->sendRuledCommand(ruledCommand);
}

void RuledClientState::confirmAttackers()
{
    ruled::v1::RuledCommand ruledCommand;
    auto *declare = ruledCommand.mutable_declare_attackers();
    for (const quint32 oid : pendingAttackerOids) {
        declare->add_creature_ids(oid);
    }
    host->sendRuledCommand(ruledCommand);
    attackersSubmittedThisStep = true;
    pendingAttackerOids.clear();
    emit combatStateChanged();
}

void RuledClientState::skipAttackers()
{
    ruled::v1::RuledCommand ruledCommand;
    ruledCommand.mutable_declare_attackers();
    host->sendRuledCommand(ruledCommand);
    attackersSubmittedThisStep = true;
    pendingAttackerOids.clear();
    emit combatStateChanged();
}

void RuledClientState::confirmBlockers()
{
    ruled::v1::RuledCommand ruledCommand;
    auto *declare = ruledCommand.mutable_declare_blockers();
    for (auto it = pendingBlocks.constBegin(); it != pendingBlocks.constEnd(); ++it) {
        auto *pair = declare->add_block_pairs();
        pair->set_blocker_id(it.key());
        pair->set_attacker_id(it.value());
    }
    // On rejection (e.g. menace requires 2+ blockers), roll back the eagerly-set guard flags so
    // the defender can fix and re-submit. On success the engine's BlockersDeclared event will
    // update committedBlocks and set blockersSubmittedThisStep via the normal event path.
    host->sendRuledCommandExpectingAck(ruledCommand, [this](bool accepted) {
        if (accepted) {
            return;
        }
        pendingBlocks = committedBlocks;
        committedBlocks.clear();
        blockersSubmittedThisStep = false;
        // Fire blockerRejected first so the prompt widget sets its sticky label before
        // combatStateChanged triggers refreshPromptLabel().
        emit blockerRejected();
        emit combatStateChanged();
    });
    // Eagerly flip the guard to prevent a double-submit while the round-trip is in flight;
    // the callback above resets it if the engine rejects.
    blockersSubmittedThisStep = true;
    committedBlocks = pendingBlocks;
    pendingBlocks.clear();
    stagedBlockerOids.clear();
    emit combatStateChanged();
}

void RuledClientState::skipBlockers()
{
    ruled::v1::RuledCommand ruledCommand;
    ruledCommand.mutable_declare_blockers();
    host->sendRuledCommand(ruledCommand);
    blockersSubmittedThisStep = true;
    pendingBlocks.clear();
    committedBlocks.clear();
    stagedBlockerOids.clear();
    emit combatStateChanged();
}

// ---------------------------------------------------------------------------------------
// Opening sequence commands
// ---------------------------------------------------------------------------------------

void RuledClientState::openingPickFirstSeat(int seatId)
{
    ruled::v1::RuledCommand ruledCommand;
    ruledCommand.mutable_choose_starting_player()->set_starting_player_id(seatId);
    host->sendRuledCommand(ruledCommand);
}

void RuledClientState::openingMulliganKeep()
{
    ruled::v1::RuledCommand ruledCommand;
    ruledCommand.mutable_mulligan()->set_keep(true);
    host->sendRuledCommand(ruledCommand);
}

void RuledClientState::openingMulliganRedraw()
{
    ++openingMulliganCount;
    ruled::v1::RuledCommand ruledCommand;
    ruledCommand.mutable_mulligan()->set_keep(false);
    host->sendRuledCommand(ruledCommand);
}

void RuledClientState::openingBottomCancel()
{
    clearOpeningBottomSelection(true);
}

void RuledClientState::openingBottomDone()
{
    const int need = openingBottomRequiredCount();
    if (need <= 0 || openingBottomSelectedIndices.size() != need) {
        return;
    }
    const QList<int> clickOrder = openingBottomSelectedIndices;
    clearOpeningBottomSelection(false);
    notifyHandUiChanged();
    // Each accepted command removes a card from the engine hand Vec, shifting all higher indices
    // down by one. Compute the adjusted sequence up front, then wait for each acknowledgement so
    // the global one-gameplay-command lock never drops or reorders a bottoming choice.
    QList<int> adjustedIndices;
    for (int k = 0; k < clickOrder.size(); ++k) {
        const int orig = clickOrder[k];
        int adjusted = orig;
        for (int j = 0; j < k; ++j) {
            if (clickOrder[j] < orig) {
                --adjusted;
            }
        }
        adjustedIndices.append(adjusted);
    }
    sendOpeningBottomCommandSequence(adjustedIndices, 0);
}

void RuledClientState::sendOpeningBottomCommandSequence(const QList<int> &adjustedIndices, int position)
{
    if (position < 0 || position >= adjustedIndices.size()) {
        return;
    }
    ruled::v1::RuledCommand ruledCommand;
    ruledCommand.mutable_put_opening_hand_on_bottom()->set_hand_card_index(
        static_cast<quint32>(adjustedIndices[position]));
    host->sendRuledCommandExpectingAck(ruledCommand, [this, adjustedIndices, position](bool accepted) {
        if (accepted) {
            sendOpeningBottomCommandSequence(adjustedIndices, position + 1);
        }
    });
}

// ---------------------------------------------------------------------------------------
// Combat damage assignment
// ---------------------------------------------------------------------------------------

quint32 RuledClientState::currentCombatDamageAttackerOid() const
{
    if (currentCombatDamageAttackerIdx < 0 || currentCombatDamageAttackerIdx >= combatDamagePendingAttackers.size()) {
        return 0;
    }
    return combatDamagePendingAttackers.at(currentCombatDamageAttackerIdx);
}

int RuledClientState::combatPowerForCreatureOid(quint32 engineOid) const
{
    const int fromEngine = engineOidBattlefieldPower.value(engineOid, 0);
    if (fromEngine > 0) {
        return fromEngine;
    }
    if (engineOid == 0) {
        return 0;
    }
    int pow = 0;
    int tough = 0;
    if (host->fallbackCreaturePt(engineOid, &pow, &tough)) {
        return pow;
    }
    return 0;
}

int RuledClientState::combatToughnessForCreatureOid(quint32 engineOid) const
{
    const int fromEngine = engineOidBattlefieldToughness.value(engineOid, 0);
    if (fromEngine > 0) {
        return fromEngine;
    }
    if (engineOid == 0) {
        return 1;
    }
    int pow = 0;
    int tough = 0;
    if (host->fallbackCreaturePt(engineOid, &pow, &tough) && tough > 0) {
        return tough;
    }
    return 1;
}

void RuledClientState::seedDefaultCombatDamageForCurrentAttacker()
{
    if (!localPlayerIsActive()) {
        return;
    }
    const quint32 curAtt = currentCombatDamageAttackerOid();
    if (curAtt == 0) {
        return;
    }
    const QList<quint32> blockers = committedBlockerGroups.value(curAtt);
    const bool hasTramp = engineOidTrample.value(curAtt, false);
    // Single-blocker without trample: no explicit assignment needed; skip.
    if (blockers.size() < 2 && !hasTramp) {
        return;
    }
    const int power = combatPowerForCreatureOid(curAtt);
    if (power <= 0) {
        return;
    }
    for (quint32 blk : blockers) {
        pendingCombatDamageByBlocker.remove(blk);
    }
    int remaining = power;
    for (int i = 0; i < blockers.size(); ++i) {
        const quint32 blk = blockers.at(i);
        const int lethal = qMax(1, combatToughnessForCreatureOid(blk) - engineOidMarkedDamage.value(blk, 0));
        if (hasTramp) {
            // CR 702.19: trample — assign exactly lethal to each blocker; remainder goes to
            // the defending player (not to the last blocker).
            const int assign = qMin(remaining, lethal);
            remaining -= assign;
            if (assign > 0) {
                pendingCombatDamageByBlocker.insert(blk, static_cast<quint32>(assign));
            }
        } else if (i == blockers.size() - 1) {
            // Non-trample last blocker: receives all remaining damage.
            if (remaining > 0) {
                pendingCombatDamageByBlocker.insert(blk, static_cast<quint32>(remaining));
            }
            break;
        } else {
            // Non-trample middle blocker: assign up to lethal.
            const int assign = qMin(remaining, lethal);
            remaining -= assign;
            if (assign > 0) {
                pendingCombatDamageByBlocker.insert(blk, static_cast<quint32>(assign));
            }
        }
    }
    emit combatDamageUiChanged();
    emit combatStateChanged();
}

void RuledClientState::bumpBlockerCombatDamage(quint32 blockerOid, int delta)
{
    const quint32 curAtt = currentCombatDamageAttackerOid();
    if (curAtt == 0 || delta == 0) {
        return;
    }
    const QList<quint32> &blockers = committedBlockerGroups.value(curAtt);
    if (!blockers.contains(blockerOid)) {
        return;
    }
    const int power = combatPowerForCreatureOid(curAtt);
    if (power <= 0) {
        return;
    }
    const quint32 cur = pendingCombatDamageByBlocker.value(blockerOid, 0);
    qint64 next = static_cast<qint64>(cur) + delta;
    if (next < 0) {
        next = 0;
    }
    if (next > power) {
        next = power;
    }
    if (next == 0) {
        pendingCombatDamageByBlocker.remove(blockerOid);
    } else {
        pendingCombatDamageByBlocker.insert(blockerOid, static_cast<quint32>(next));
    }
    emit combatDamageUiChanged();
    emit combatStateChanged();
}

void RuledClientState::clearCombatDamageAssignmentState()
{
    combatDamagePendingAttackers.clear();
    currentCombatDamageAttackerIdx = -1;
    pendingCombatDamageByBlocker.clear();
}

QString RuledClientState::currentCombatDamageAttackerDisplayName() const
{
    const quint32 att = currentCombatDamageAttackerOid();
    if (att == 0) {
        return {};
    }
    const QString name = host->battlefieldCardName(att);
    if (!name.isEmpty()) {
        return name;
    }
    return tr("creature");
}

int RuledClientState::currentCombatDamageAttackerPower() const
{
    const quint32 att = currentCombatDamageAttackerOid();
    if (att == 0) {
        return 0;
    }
    return combatPowerForCreatureOid(att);
}

int RuledClientState::localCombatDamageAssignedTotal() const
{
    int sum = 0;
    for (auto it = pendingCombatDamageByBlocker.constBegin(); it != pendingCombatDamageByBlocker.constEnd(); ++it) {
        sum += static_cast<int>(it.value());
    }
    return sum;
}

int RuledClientState::localCombatDamagePlayerDamage() const
{
    const quint32 curAtt = currentCombatDamageAttackerOid();
    if (curAtt == 0 || !engineOidTrample.value(curAtt, false)) {
        return 0;
    }
    const int power = currentCombatDamageAttackerPower();
    const QList<quint32> blockers = committedBlockerGroups.value(curAtt);
    int sum = 0;
    for (quint32 blk : blockers) {
        sum += static_cast<int>(pendingCombatDamageByBlocker.value(blk, 0));
    }
    return qMax(0, power - sum);
}

bool RuledClientState::localCombatDamageAssignmentLegal() const
{
    const quint32 curAtt = currentCombatDamageAttackerOid();
    if (curAtt == 0) {
        return false;
    }
    const int power = currentCombatDamageAttackerPower();
    if (power <= 0) {
        return false;
    }
    const QList<quint32> blockers = committedBlockerGroups.value(curAtt);
    int sum = 0;
    for (quint32 blk : blockers) {
        sum += static_cast<int>(pendingCombatDamageByBlocker.value(blk, 0));
    }
    if (engineOidTrample.value(curAtt, false)) {
        // CR 702.19: each blocker must receive >= lethal; total assigned to blockers must be <= power
        // (remainder goes to the defending player).
        if (sum > power) {
            return false;
        }
        for (quint32 blk : blockers) {
            const int lethal = qMax(1, combatToughnessForCreatureOid(blk) - engineOidMarkedDamage.value(blk, 0));
            if (static_cast<int>(pendingCombatDamageByBlocker.value(blk, 0)) < lethal) {
                return false;
            }
        }
        return true;
    }
    return sum == power;
}

void RuledClientState::confirmCombatDamageForCurrentAttacker()
{
    const quint32 curAtt = currentCombatDamageAttackerOid();
    if (curAtt == 0) {
        return;
    }
    if (!localCombatDamageAssignmentLegal()) {
        return;
    }
    ruled::v1::RuledCommand ruledCommand;
    auto *acd = ruledCommand.mutable_assign_combat_damage();
    acd->set_attacker_id(curAtt);
    const QList<quint32> blockers = committedBlockerGroups.value(curAtt);
    const int power = currentCombatDamageAttackerPower();
    int blockerSum = 0;
    for (quint32 blk : blockers) {
        const auto dmg = pendingCombatDamageByBlocker.value(blk, 0);
        auto *pair = acd->add_assignments();
        pair->set_blocker_id(blk);
        pair->set_damage(dmg);
        blockerSum += static_cast<int>(dmg);
    }
    // CR 702.19: for trample, set the defending player's share of the damage.
    const int playerDmg = qMax(0, power - blockerSum);
    if (engineOidTrample.value(curAtt, false) && playerDmg > 0) {
        acd->set_defending_player_damage(static_cast<uint32_t>(playerDmg));
    }
    host->sendRuledCommand(ruledCommand);
    emit combatStateChanged();
}

// ---------------------------------------------------------------------------------------
// Stack
// ---------------------------------------------------------------------------------------

void RuledClientState::registerSyntheticStackCard(quint32 virtualOid, int fakeCardId, int zonePlayerId)
{
    syntheticAbilityFakeIds.insert(virtualOid, fakeCardId);
    syntheticAbilityControllerPid.insert(virtualOid, zonePlayerId);
    ownerCardIdToEngineOid.insert(makeOwnedCardKey(zonePlayerId, fakeCardId), virtualOid);
}

void RuledClientState::unregisterSyntheticStackCard(quint32 virtualOid, int fakeCardId)
{
    syntheticAbilityFakeIds.remove(virtualOid);
    const int zonePid = syntheticAbilityControllerPid.take(virtualOid);
    ownerCardIdToEngineOid.remove(makeOwnedCardKey(zonePid, fakeCardId));
}

// ---------------------------------------------------------------------------------------
// Session lifecycle
// ---------------------------------------------------------------------------------------

void RuledClientState::clearSessionState(RuledSessionResetScope scope)
{
    finishEngineCommand();
    // Pending choice + the trigger stack bookkeeping that outlives it.
    clearPendingChoice();
    lastTriggerSourceOid = 0;
    lastTriggerAbilityIndex = 0;
    lastTriggerControllerPlayerId = -1;

    // Stack tracking — the host removes the synthetic ability CardItems from their zones (which
    // calls back into unregisterSyntheticStackCard) before invoking this.
    stackOidOrder.clear();
    triggerOrderCandidateOids.clear();
    stackTargetsByStackOid.clear();
    stackTargetKindByStackAndTargetOid.clear();
    stackAnnotationByOid.clear();
    stackSourceOidByStackOid.clear();
    syntheticAbilityControllerPid.clear();
    syntheticAbilityFakeIds.clear();

    // Graveyard identity maps: rebuilt per batch from the server's GraveyardObjectMap, but that
    // event is only sent when non-empty, so a stale map would otherwise survive into the next
    // game and offer phantom targets.
    ownedGraveyardCardToEngineOid.clear();
    graveyardOidToPlayerId.clear();
    ownedExileCardToEngineOid.clear();
    exileOidToPlayerId.clear();
    exileOidToServerCardId.clear();
    pendingCastGraveyardOids.clear();

    // Legal actions + opening sequence. Skipped on the game-start transition: the incoming
    // session's first batch has already populated these (see SessionResetScope), and clearing
    // them here would drop the choose-first prompt on the floor with the engine blocked waiting
    // for the answer. Not a leak risk either way — resetPerBatchLegalActions() rebuilds all of
    // it at the head of every payload.
    if (scope == RuledSessionResetScope::All) {
        clearHandActions();
        zoneCastActions = {};
        zoneCastSourceByOid.clear();
        zoneCastCostsByCastKey.clear();
        validTargetsByHandSlot.clear();
        validTargetsByZoneObject.clear();
        openingUiKind = RuledOpeningUiKind::None;
        openingMulliganCount = 0;
        openingPickSeatIds.clear();
        openingBottomSelectedIndices.clear();
    }

    // A pick may have been live when the session ended; the holder is already cleared above, but
    // the prompt panel still needs telling.
    emit resolutionHandPickUiChanged(-1, -1);

    emit sessionReset();
}
