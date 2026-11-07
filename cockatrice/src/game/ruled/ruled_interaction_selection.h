#ifndef RULED_INTERACTION_SELECTION_H
#define RULED_INTERACTION_SELECTION_H

#include "ruled_pending_cast.h"

/// Payment candidates come from the preview; mana abilities additionally belong to the acting seat.
[[nodiscard]] inline RuledTargetClickEligibility ruledPaymentCandidateEligibility(const RuledClientState &state,
                                                                                 quint32 oid,
                                                                                 bool battlefield,
                                                                                 int sourcePlayerId,
                                                                                 int actingPlayerId)
{
    using E = RuledTargetClickEligibility;
    if (!state.payment.active)
        return E::NotTargeting;
    if (oid == 0 || actingPlayerId < 0)
        return E::Illegal;
    bool legal = false;
    if (oid != 0 && battlefield) {
        const auto *offered = state.payment.candidate(oid);
        legal = state.payment.selected(oid) ||
                (offered && offered->options_size() > 0 &&
                 offered->object().zone_change_generation() == state.abilitySourceGeneration(oid));
    }
    // Public ability metadata is evaluated for the source's controller, not its viewer.
    // The physical battlefield seat follows control changes, just as it does for clicks.
    if (sourcePlayerId == actingPlayerId) {
        const auto abilities = state.activatedAbilitiesForOid(oid);
        for (int index = 0; index < abilities.size(); ++index) {
            const auto &ability = abilities.at(index);
            legal |= ability && ability->isManaAbility() && state.abilityActivatable(oid, index);
        }
    }
    return legal ? E::Legal : E::Illegal;
}

/// Same candidate and competing-cost receipts used by the payment click path. Hand candidates
/// use engine slots, while already selected hand cards use stable physical IDs.
template <typename Pending>
[[nodiscard]] inline RuledTargetClickEligibility ruledPendingCostCandidateEligibility(const Pending &pending,
                                                                                      RuledCostChoiceZone zone,
                                                                                      quint32 candidateId,
                                                                                      quint32 stableId,
                                                                                      quint64 generation)
{
    if (!pending.valid || !pending.waitingForCost) {
        return RuledTargetClickEligibility::NotTargeting;
    }
    if (pending.nextCostChoice < 0 || pending.nextCostChoice >= pending.costChoices.size()) {
        return RuledTargetClickEligibility::Illegal;
    }
    const auto &choice = pending.costChoices.at(pending.nextCostChoice);
    if (choice.zone != zone || !choice.candidateIds.contains(candidateId) ||
        (choice.candidateGenerations.contains(candidateId) &&
         choice.candidateGenerations.value(candidateId) != generation)) {
        return RuledTargetClickEligibility::Illegal;
    }
    for (const auto &selected : pending.costSelections) {
        if (ruledCostSelectionConflicts(choice, pending.costChoices, selected, stableId)) {
            return RuledTargetClickEligibility::Illegal;
        }
    }
    return RuledTargetClickEligibility::Legal;
}

#endif
