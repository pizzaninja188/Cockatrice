#include "ruled_client_state.h"

#include "ruled_client_host.h"

#include <algorithm>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

RuledClientState::RuledClientState(RuledClientHost *_host, QObject *parent) : QObject(parent), host(_host)
{
}

// ---------------------------------------------------------------------------------------
// Identity
// ---------------------------------------------------------------------------------------

quint32 RuledClientState::graveyardEngineOidForServerCardId(int serverCardId) const
{
    for (auto it = graveyardEngineOidToServerCardId.constBegin(); it != graveyardEngineOidToServerCardId.constEnd();
         ++it) {
        if (it.value() == serverCardId) {
            return it.key();
        }
    }
    return 0;
}

// ---------------------------------------------------------------------------------------
// Legal hand actions
// ---------------------------------------------------------------------------------------

bool RuledClientState::isLandPlayLegalForHandIndex(int handIndex) const
{
    return legalLandPlayHandIndices.contains(handIndex);
}

int RuledClientState::landPlayHandIndexForCard(const QString &cardName, int preferredHandIndex) const
{
    const QList<int> matching = landPlayHandIndicesForCardName(cardName);
    if (matching.contains(preferredHandIndex)) {
        return preferredHandIndex;
    }
    if (matching.isEmpty()) {
        return -1;
    }
    return matching.first();
}

QList<int> RuledClientState::landPlayHandIndicesForCardName(const QString &cardName) const
{
    QList<int> matching = legalLandPlayIndicesByCardName.values(cardName);
    std::sort(matching.begin(), matching.end());
    return matching;
}

bool RuledClientState::isSpellCastLegalForHandIndex(int handIndex) const
{
    return legalSpellCastHandIndices.contains(handIndex);
}

bool RuledClientState::isSpellCastNeedsTargetForHandIndex(int handIndex) const
{
    return legalSpellCastNeedsTargetHandIndices.contains(handIndex);
}

int RuledClientState::spellCastHandIndexForCard(const QString &cardName, int preferredHandIndex) const
{
    const QList<int> matching = spellCastHandIndicesForCardName(cardName);
    if (matching.contains(preferredHandIndex)) {
        return preferredHandIndex;
    }
    if (matching.isEmpty()) {
        return -1;
    }
    return matching.first();
}

QList<int> RuledClientState::spellCastHandIndicesForCardName(const QString &cardName) const
{
    QList<int> matching = legalSpellCastIndicesByCardName.values(cardName);
    std::sort(matching.begin(), matching.end());
    return matching;
}

QVector<RuledLandFaceOption> RuledClientState::landPlayFaceOptionsForHandIndex(int handIndex) const
{
    QVector<RuledLandFaceOption> options = legalLandPlayFaceOptionsByHandIndex.value(handIndex);
    std::sort(options.begin(), options.end(),
              [](const RuledLandFaceOption &a, const RuledLandFaceOption &b) { return a.faceIndex < b.faceIndex; });
    return options;
}

bool RuledClientState::isCleanupDiscardLegalForHandIndex(int handIndex) const
{
    return legalCleanupDiscardHandIndices.contains(handIndex);
}

int RuledClientState::cleanupDiscardHandIndexForCard(const QString &cardName, int preferredHandIndex) const
{
    const QList<int> matching = cleanupDiscardHandIndicesForCardName(cardName);
    if (matching.contains(preferredHandIndex)) {
        return preferredHandIndex;
    }
    if (matching.isEmpty()) {
        return -1;
    }
    return matching.first();
}

QList<int> RuledClientState::cleanupDiscardHandIndicesForCardName(const QString &cardName) const
{
    QList<int> matching = legalCleanupDiscardIndicesByCardName.values(cardName);
    std::sort(matching.begin(), matching.end());
    return matching;
}

bool RuledClientState::localPlayerMustCleanupDiscard() const
{
    return !legalCleanupDiscardHandIndices.isEmpty();
}

int RuledClientState::cleanupDiscardRequiredCount() const
{
    const int n = legalCleanupDiscardHandIndices.size();
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
    if (!isCleanupDiscardLegalForHandIndex(ruledHandIndex)) {
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
    if (legalCleanupDiscardHandIndices.isEmpty()) {
        cleanupDiscardSelectedIndices.clear();
        emit cleanupDiscardUiChanged(0, 0);
        emit combatStateChanged();
        return;
    }
    for (auto it = cleanupDiscardSelectedIndices.begin(); it != cleanupDiscardSelectedIndices.end();) {
        if (!legalCleanupDiscardHandIndices.contains(*it)) {
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

bool RuledClientState::isOpeningBottomLegalForHandIndex(int handIndex) const
{
    return legalOpeningBottomHandIndices.contains(handIndex);
}

QList<int> RuledClientState::openingBottomLegalHandIndicesSorted() const
{
    QList<int> legal = legalOpeningBottomHandIndices.values();
    std::sort(legal.begin(), legal.end());
    return legal;
}

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
    if (!isOpeningBottomLegalForHandIndex(ruledHandIndex)) {
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
// Tier-3 resolution hand pick
// ---------------------------------------------------------------------------------------

bool RuledClientState::isResolutionHandPickCardSelectable(int serverCardId) const
{
    if (!resolutionHandPick.has_value()) {
        return false;
    }
    if (!resolutionHandPick->serverCardIdToOid.contains(serverCardId)) {
        return false;
    }
    // Already selected: always show its highlight/number.
    if (resolutionHandPick->selectedServerCardIds.contains(serverCardId)) {
        return true;
    }
    // When unique-names is on, exclude candidates whose name is already taken by a
    // different selected card — they lose the faint outline and become unclickable.
    if (resolutionHandPick->uniqueNames) {
        const QString &name = resolutionHandPick->serverCardIdToName.value(serverCardId);
        for (int selId : resolutionHandPick->selectedServerCardIds) {
            if (resolutionHandPick->serverCardIdToName.value(selId) == name) {
                return false;
            }
        }
    }
    return true;
}

int RuledClientState::resolutionHandPickClickOrderFor(int serverCardId) const
{
    if (!resolutionHandPick.has_value()) {
        return 0;
    }
    const int pos = resolutionHandPick->selectedServerCardIds.indexOf(serverCardId);
    return pos + 1;
}

QVector<int> RuledClientState::resolutionHandPickCandidateServerCardIds() const
{
    if (!resolutionHandPick.has_value()) {
        return {};
    }
    const QList<int> keys = resolutionHandPick->serverCardIdToOid.keys();
    return QVector<int>(keys.begin(), keys.end());
}

void RuledClientState::toggleResolutionHandPickCard(int serverCardId)
{
    if (!resolutionHandPick.has_value()) {
        return;
    }
    if (!resolutionHandPick->serverCardIdToOid.contains(serverCardId)) {
        return;
    }
    const int pos = resolutionHandPick->selectedServerCardIds.indexOf(serverCardId);
    if (pos >= 0) {
        resolutionHandPick->selectedServerCardIds.removeAt(pos);
    } else if (resolutionHandPick->selectedServerCardIds.size() < resolutionHandPick->max) {
        if (resolutionHandPick->uniqueNames) {
            const QString clickedName = resolutionHandPick->serverCardIdToName.value(serverCardId);
            bool nameTaken = false;
            for (int selId : resolutionHandPick->selectedServerCardIds) {
                if (resolutionHandPick->serverCardIdToName.value(selId) == clickedName) {
                    nameTaken = true;
                    break;
                }
            }
            if (nameTaken) {
                return;
            }
        }
        resolutionHandPick->selectedServerCardIds.append(serverCardId);
    }
    emit resolutionHandPickUiChanged(resolutionHandPick->min, resolutionHandPick->selectedServerCardIds.size());
    emit combatStateChanged();
}

void RuledClientState::submitResolutionHandPick()
{
    if (!resolutionHandPick.has_value()) {
        return;
    }
    const int n = resolutionHandPick->selectedServerCardIds.size();
    if (n < resolutionHandPick->min || n > resolutionHandPick->max) {
        return;
    }
    QVector<quint32> chosen;
    chosen.reserve(n);
    for (int scid : resolutionHandPick->selectedServerCardIds) {
        const quint32 oid = resolutionHandPick->serverCardIdToOid.value(scid, 0);
        if (oid != 0) {
            chosen.append(oid);
        }
    }
    const bool wasRevealed = resolutionHandPick->pickZone == PickZone::Revealed;
    resolutionHandPick.reset();
    emit resolutionHandPickUiChanged(-1, -1);
    if (wasRevealed) {
        emit revealedPickChanged(false, {}, {}, 0, 0);
    }
    emit combatStateChanged();

    ruled::v1::RuledCommand cmd;
    auto *sub = cmd.mutable_submit_resolution_choice();
    for (quint32 o : chosen) {
        sub->add_chosen_object_ids(o);
    }
    host->sendRuledCommand(cmd);
}

void RuledClientState::submitCopyTargetChoice(quint32 oid)
{
    pendingCopyTargetChoice = {};
    ruled::v1::RuledCommand cmd;
    cmd.mutable_submit_resolution_choice()->add_chosen_object_ids(oid);
    host->sendRuledCommand(cmd);
}

void RuledClientState::submitLegendKeepChoice(quint32 oid)
{
    // CR 704.5j: the chosen permanent is the legend to KEEP; the engine sacrifices the rest.
    pendingLegendKeepChoice = {};
    ruled::v1::RuledCommand cmd;
    cmd.mutable_submit_resolution_choice()->add_chosen_object_ids(oid);
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
    } else {
        pendingAttackerOids.insert(engineOid);
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
    } else {
        stagedBlockerOids.insert(blockerOid);
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
    // Send in click order. Each sent command removes a card from the engine hand Vec, shifting all
    // higher indices down by one. Adjust each index for prior removals.
    for (int k = 0; k < clickOrder.size(); ++k) {
        const int orig = clickOrder[k];
        int adjusted = orig;
        for (int j = 0; j < k; ++j) {
            if (clickOrder[j] < orig) {
                --adjusted;
            }
        }
        ruled::v1::RuledCommand ruledCommand;
        ruledCommand.mutable_put_opening_hand_on_bottom()->set_hand_card_index(static_cast<quint32>(adjusted));
        host->sendRuledCommand(ruledCommand);
    }
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

void RuledClientState::clearSessionState()
{
    // Pending trigger
    hasPendingTrigger = false;
    pendingTriggerSourceOid = 0;
    pendingTriggerAbilityIndex = 0;
    pendingTriggerAbilityText.clear();
    pendingTriggerControllerPlayerId = -1;
    pendingCopyTargetChoice = {};
    pendingLegendKeepChoice = {};

    // Stack tracking — the host removes the synthetic ability CardItems from their zones (which
    // calls back into unregisterSyntheticStackCard) before invoking this.
    stackOidOrder.clear();
    stackTargetsByStackOid.clear();
    stackAnnotationByOid.clear();
    stackSourceOidByStackOid.clear();
    syntheticAbilityControllerPid.clear();
    syntheticAbilityFakeIds.clear();

    // Engine oid -> Server_Card.id for graveyard cards: rebuilt per batch from the server's
    // GraveyardObjectMap, but that event is only sent when non-empty, so a stale map would
    // otherwise survive into the next game and offer phantom targets.
    graveyardEngineOidToServerCardId.clear();

    // Legal action sets
    legalLandPlayHandIndices.clear();
    legalLandPlayIndicesByCardName.clear();
    legalLandPlayFaceOptionsByHandIndex.clear();
    legalSpellCastHandIndices.clear();
    legalSpellCastIndicesByCardName.clear();
    legalSpellCastNeedsTargetHandIndices.clear();
    legalCleanupDiscardHandIndices.clear();
    legalCleanupDiscardIndicesByCardName.clear();
    legalOpeningBottomHandIndices.clear();

    // Opening sequence
    openingUiKind = RuledOpeningUiKind::None;
    openingMulliganCount = 0;
    openingPickSeatIds.clear();
    openingBottomSelectedIndices.clear();

    // Resolution hand-pick
    if (resolutionHandPick.has_value() && resolutionHandPick->pickZone == PickZone::Revealed) {
        emit revealedPickChanged(false, {}, {}, 0, 0);
    }
    resolutionHandPick.reset();
    emit resolutionHandPickUiChanged(-1, -1);

    emit sessionReset();
}
