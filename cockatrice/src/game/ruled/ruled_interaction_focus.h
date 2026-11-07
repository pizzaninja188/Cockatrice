#ifndef RULED_INTERACTION_FOCUS_H
#define RULED_INTERACTION_FOCUS_H

#include "ruled_client_state.h"
#include "ruled_pending_cast.h"

/// One-hop, recipient-visible relationships. Domains stay separate even for identical IDs.
struct RuledInteractionFocus
{
    bool active = false;
    QSet<quint32> battlefield;
    QSet<quint32> stack;
    QSet<quint32> graveyard;
    QSet<int> players;
};

[[nodiscard]] inline RuledTargetClickEligibility
ruledCombatSelection(const RuledClientState &state, int localPlayerId, quint32 oid)
{
    using E = RuledTargetClickEligibility;
    using Phase = RuledClientState::RuledCombatPhase;
    if (state.currentCombatPhase == Phase::DeclareAttackers && state.currentActivePlayerId == localPlayerId &&
        !state.attackersSubmittedThisStep) {
        return state.selectableAttackerOids.contains(oid) ? E::Legal : E::Illegal;
    }
    if (state.currentCombatPhase == Phase::DeclareBlockers && state.currentPriorityPlayerId == localPlayerId &&
        !state.blockersSubmittedThisStep) {
        return !state.legalBlockAttackerOidsByBlocker.value(oid).isEmpty() ? E::Legal : E::Illegal;
    }
    return E::NotTargeting;
}

[[nodiscard]] inline bool ruledFocusPermanentCurrent(const RuledClientState &state, quint32 oid, quint64 generation)
{
    return state.battlefieldGenerationByOid.contains(oid) &&
           state.battlefieldGenerationByOid.value(oid) == generation && !state.isEngineOidPhasedOut(oid);
}

[[nodiscard]] inline RuledInteractionFocus ruledStackFocus(const RuledClientState &state, quint32 stackOid)
{
    RuledInteractionFocus focus;
    if (!state.stackOidOrder.contains(stackOid)) {
        return focus;
    }
    focus.active = true;
    focus.stack.insert(stackOid);
    const auto source = state.stackSourceCapturedOidByStackOid.constFind(stackOid);
    if (source != state.stackSourceCapturedOidByStackOid.cend() &&
        ruledFocusPermanentCurrent(state, source.value(), state.stackSourceGenerationByStackOid.value(stackOid))) {
        focus.battlefield.insert(source.value());
    }
    const auto original = state.stackCopySourceByStackOid.value(stackOid);
    if (original != 0 && state.stackOidOrder.contains(original)) {
        focus.stack.insert(original);
    }
    for (const auto &target : state.stackTargetReceiptsByStackOid.value(stackOid)) {
        const auto oid = target.object_id();
        switch (target.kind()) {
            case ruled::v1::TARGET_REF_KIND_PLAYER:
                focus.players.insert(static_cast<int>(oid));
                break;
            case ruled::v1::TARGET_REF_KIND_PERMANENT:
                if (target.has_expected_zone_change_generation() &&
                    ruledFocusPermanentCurrent(state, oid, target.expected_zone_change_generation())) {
                    focus.battlefield.insert(oid);
                }
                break;
            case ruled::v1::TARGET_REF_KIND_STACK:
                if (state.stackOidOrder.contains(oid)) {
                    focus.stack.insert(oid);
                }
                break;
            case ruled::v1::TARGET_REF_KIND_GRAVEYARD:
                // Public-zone handles are retired by the relay on every zone change.
                if (state.graveyardOidToServerCardId.contains(oid)) {
                    focus.graveyard.insert(oid);
                }
                break;
            default:
                break;
        }
    }
    return focus;
}

inline void
ruledFocusDefender(RuledInteractionFocus &focus, const RuledClientState &state, const ruled::v1::AttackAssignment &edge)
{
    if (!ruledFocusPermanentCurrent(state, edge.attacker_object_id(), edge.attacker_zone_change_generation())) {
        return;
    }
    const auto &target = edge.defender();
    if (target.kind() == ruled::v1::TARGET_REF_KIND_PLAYER) {
        focus.players.insert(static_cast<int>(target.object_id()));
    } else if (target.kind() == ruled::v1::TARGET_REF_KIND_PERMANENT &&
               ruledFocusPermanentCurrent(state, target.object_id(), edge.defender_zone_change_generation())) {
        focus.battlefield.insert(target.object_id());
    }
}

[[nodiscard]] inline RuledInteractionFocus
ruledCombatFocus(const RuledClientState &state, quint32 hoveredOid, int localPlayerId)
{
    RuledInteractionFocus focus;
    if (hoveredOid == 0 || state.isEngineOidPhasedOut(hoveredOid)) {
        return focus;
    }
    using Phase = RuledClientState::RuledCombatPhase;
    const bool declaringAttacks = state.currentCombatPhase == Phase::DeclareAttackers &&
                                  state.currentActivePlayerId == localPlayerId && !state.attackersSubmittedThisStep;
    const bool declaringBlocks = state.currentCombatPhase == Phase::DeclareBlockers &&
                                 state.currentPriorityPlayerId == localPlayerId && !state.blockersSubmittedThisStep;
    if (declaringAttacks && state.selectableAttackerOids.contains(hoveredOid)) {
        focus.active = true;
        focus.battlefield.insert(hoveredOid);
        for (const auto &edge : state.legalAttackAssignmentsByAttacker.value(hoveredOid)) {
            ruledFocusDefender(focus, state, edge);
        }
        return focus;
    }
    if (declaringBlocks) {
        focus.active = state.currentAttackerOids.contains(hoveredOid) ||
                       state.legalBlockAttackerOidsByBlocker.contains(hoveredOid);
        for (auto it = state.legalBlockAttackerOidsByBlocker.cbegin();
             it != state.legalBlockAttackerOidsByBlocker.cend(); ++it) {
            if (it.key() == hoveredOid) {
                focus.battlefield.unite(it.value());
            }
            if (it.value().contains(hoveredOid)) {
                focus.battlefield.insert(it.key());
            }
        }
    } else {
        focus.active = state.currentAttackerOids.contains(hoveredOid) ||
                       state.pendingAttackerOids.contains(hoveredOid) ||
                       state.remoteAttackerPreviewOids.contains(hoveredOid);
        for (const auto *pairs : {&state.committedBlocks, &state.pendingBlocks, &state.remoteBlockPreviewPairs}) {
            for (auto it = pairs->cbegin(); it != pairs->cend(); ++it) {
                if (it.key() == hoveredOid || it.value() == hoveredOid) {
                    focus.active = true;
                    focus.battlefield.insert(it.key());
                    focus.battlefield.insert(it.value());
                }
            }
        }
    }
    if (!focus.active) {
        return focus;
    }
    focus.battlefield.insert(hoveredOid);
    const auto combatants = focus.battlefield;
    for (const auto *assignments :
         {&state.currentAttackAssignments, &state.pendingAttackAssignments, &state.remoteAttackPreviewAssignments}) {
        for (auto it = assignments->cbegin(); it != assignments->cend(); ++it) {
            if (combatants.contains(it.key())) {
                ruledFocusDefender(focus, state, it.value());
            }
        }
    }
    return focus;
}

#endif
