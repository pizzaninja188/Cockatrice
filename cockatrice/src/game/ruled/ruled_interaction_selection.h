#ifndef RULED_INTERACTION_SELECTION_H
#define RULED_INTERACTION_SELECTION_H

#include "ruled_pending_cast.h"

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
