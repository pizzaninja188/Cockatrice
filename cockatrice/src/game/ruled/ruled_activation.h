#ifndef RULED_ACTIVATION_H
#define RULED_ACTIVATION_H

#include "ruled_pending_cast.h"

/// Pure transaction helpers shared by the image-click, payment and reconnect bridges.
/// Offered generations are receipts: never replace one with a later physical incarnation.
inline bool ruledActivationNestedChoice(const RuledClientState &state)
{
    return state.pendingChoice.has_value() || state.isWaitingForChoice();
}

inline bool ruledActivationCanPay(const RuledClientState &state, int localPlayerId)
{
    return state.pendingAbilityActivation && !ruledActivationNestedChoice(state) &&
           state.pendingAbilityActivation->actor_player_id() == localPlayerId &&
           state.pendingAbilityActivation->stage() == ruled::v1::ABILITY_ACTIVATION_STAGE_PAYMENT;
}

/// Reopen the exact accepted aggregate-counter activation for an explicit payment retry. A retry
/// only invalidates the local preview after the client request, local draft receipt, and current
/// engine payment projection all describe the same accepted transaction.
[[nodiscard]] bool ruledRetryCounterGroupPayment(RuledClientState &state,
                                                 PendingActivatedAbility &local,
                                                 int actorPlayerId,
                                                 quint64 expectedTransactionId,
                                                 quint64 expectedRevision);

[[nodiscard]] std::optional<ruled::v1::RuledCommand>
ruledCancelCounterGroupPayment(RuledClientState &state, PendingActivatedAbility &local, int actorPlayerId);

[[nodiscard]] bool ruledRecordCounterGroupPreviewFailure(
    RuledClientState &state,
    PendingActivatedAbility &local,
    int actorPlayerId,
    const ruled::v1::PaymentPreview *rejectedReply = nullptr);

[[nodiscard]] bool ruledInvalidateCounterGroupPaymentForRefresh(RuledClientState &state,
                                                                 const PendingActivatedAbility &local,
                                                                 int actorPlayerId);

inline std::optional<ruled::v1::RuledCommand>
ruledActivationChoice(const RuledClientState &state, int localPlayerId, quint64 transaction, quint64 revision,
                      std::optional<int> opponent, std::optional<quint32> target)
{
    if (!state.pendingAbilityActivation || state.isEngineCommandPending() || ruledActivationNestedChoice(state))
        return std::nullopt;
    const auto &pending = *state.pendingAbilityActivation;
    if (pending.transaction_id() != transaction || pending.revision() != revision ||
        pending.deciding_player_id() != localPlayerId || opponent.has_value() == target.has_value())
        return std::nullopt;
    ruled::v1::RuledCommand command;
    auto *choice = command.mutable_submit_ability_activation_choice();
    choice->set_transaction_id(transaction);
    choice->set_expected_revision(revision);
    if (opponent) {
        if (pending.stage() != ruled::v1::ABILITY_ACTIVATION_STAGE_CHOOSE_OPPONENT ||
            std::find(pending.valid_opponent_ids().begin(), pending.valid_opponent_ids().end(), *opponent) ==
                pending.valid_opponent_ids().end())
            return std::nullopt;
        choice->set_opponent_player_id(*opponent);
    } else {
        if (pending.stage() != ruled::v1::ABILITY_ACTIVATION_STAGE_TARGET_SELECTION)
            return std::nullopt;
        const auto offered = std::find_if(pending.target_candidates().begin(), pending.target_candidates().end(),
                                          [target](const auto &entry) { return entry.object_id() == *target &&
                                              entry.kind() == ruled::v1::TARGET_REF_KIND_PERMANENT; });
        if (offered == pending.target_candidates().end())
            return std::nullopt;
        *choice->mutable_target() = *offered;
    }
    return command;
}

inline void ruledApplyActivationView(PendingActivatedAbility &local,
                                     const ruled::v1::PendingAbilityActivation &engine)
{
    const bool same = local.transactionId() == engine.transaction_id();
    const bool wasPaying = same && local.hasInitializedPayment();
    const bool wasWaitingForCost = same && local.waitingForCost;
    const bool wasWaitingForReturnCandidate = same && local.waitingForReturnTappedCreatureCandidate;
    const auto previousCostSelections = same ? local.costSelections : QVector<RuledPendingCostSelection>{};
    const auto previousCounterGroupContributions = local.counterGroupContributions;
    const auto previousCostChoices = local.costChoices;
    const bool initializePayment = local.applyEngineSnapshot(engine);
    const bool previousSourceMatches = local.valid && local.permanentOid == engine.source_object_id() &&
                                       local.expectedZoneChangeGeneration == engine.source_zone_change_generation() &&
                                       local.sourceZone == engine.source_zone() &&
                                       local.abilityIndex == static_cast<int>(engine.ability_index());
    local.valid = true;
    local.chosenOpponentTargets = true;
    local.permanentOid = engine.source_object_id();
    local.expectedZoneChangeGeneration = engine.source_zone_change_generation();
    local.abilityIndex = static_cast<int>(engine.ability_index());
    local.sourceZone = engine.source_zone();
    local.abilityText = QString::fromStdString(engine.source_description());
    local.deferredReturnTappedCreature = engine.source_zone() == ruled::v1::ABILITY_SOURCE_ZONE_HAND &&
                                         engine.has_return_tapped_creature_cost_index();
    local.needsTarget = !local.deferredReturnTappedCreature;
    local.waitingForTarget = false;
    local.waitingForCost = false;
    local.selectedTargets.clear();
    for (const auto &target : engine.announced_targets()) {
        PendingActivatedAbility::Target selected;
        selected.ref.set_object_id(target.object_id());
        selected.ref.set_group_index(target.group_index());
        selected.ref.set_kind(target.kind());
        selected.zoneChangeGeneration = target.zone_change_generation();
        local.selectedTargets.append(selected);
    }
    const bool paying = engine.stage() == ruled::v1::ABILITY_ACTIVATION_STAGE_PAYMENT;
    const bool acceptedCounterGroup =
        paying && std::any_of(engine.cost_selections().begin(), engine.cost_selections().end(), [](const auto &selection) {
            return selection.has_counter_removal_group();
        });
    local.waitingForMana = paying;
    local.waitingForReturnTappedCreatureCandidate = wasWaitingForReturnCandidate;
    if (acceptedCounterGroup) {
        QVector<RuledPendingCounterContribution> acceptedRows;
        for (const auto &selection : engine.cost_selections()) {
            if (!selection.has_counter_removal_group())
                continue;
            const int costIndex = static_cast<int>(selection.cost_index());
            for (const auto &contribution : selection.counter_removal_group().contributions()) {
                RuledPendingCounterContribution row;
                row.costIndex = costIndex;
                row.objectId = contribution.object().object_id();
                row.zoneChangeGeneration = contribution.object().zone_change_generation();
                row.optionId = contribution.option_id();
                row.count = contribution.count();

                if (previousSourceMatches && wasPaying) {
                    const auto previous = std::find_if(
                        previousCounterGroupContributions.cbegin(),
                        previousCounterGroupContributions.cend(),
                        [&row](const auto &candidate) {
                            return candidate.costIndex == row.costIndex && candidate.objectId == row.objectId &&
                                   candidate.zoneChangeGeneration == row.zoneChangeGeneration &&
                                   candidate.optionId == row.optionId;
                        });
                    if (previous != previousCounterGroupContributions.cend())
                        row.counterLabel = previous->counterLabel;
                } else if (previousSourceMatches) {
                    const auto previousChoice = std::find_if(
                        previousCostChoices.cbegin(), previousCostChoices.cend(), [&row](const auto &choice) {
                            return choice.costIndex == row.costIndex &&
                                   choice.kind == RuledCostChoiceKind::RemoveCountersFromPermanents &&
                                   choice.counterRemovalGroup.has_value();
                        });
                    if (previousChoice != previousCostChoices.cend()) {
                        const auto previousCandidate = std::find_if(
                            previousChoice->counterRemovalGroup->candidates.cbegin(),
                            previousChoice->counterRemovalGroup->candidates.cend(),
                            [&row](const auto &candidate) {
                                return candidate.objectId == row.objectId &&
                                       candidate.zoneChangeGeneration == row.zoneChangeGeneration;
                            });
                        if (previousCandidate != previousChoice->counterRemovalGroup->candidates.cend()) {
                            const auto previousOption = std::find_if(
                                previousCandidate->options.cbegin(), previousCandidate->options.cend(),
                                [&row](const auto &option) { return option.optionId == row.optionId; });
                            if (previousOption != previousCandidate->options.cend())
                                row.counterLabel = previousOption->label;
                        }
                    }
                }
                acceptedRows.append(row);
            }
        }

        local.counterGroupContributions = acceptedRows;
        local.counterGroupConfirmed = true;
        local.fixedFlexPayments.clear();
        for (const auto &payment : engine.flex_payments())
            local.fixedFlexPayments.append(payment);
        local.engineLockedCostLabel = QString::fromStdString(engine.locked_total_cost());

        local.costChoices.clear();
        local.nextCostChoice = 0;
        local.costSelections.clear();
        local.waitingForCost = false;
        local.waitingForTarget = false;
        local.waitingForReturnTappedCreatureCandidate = false;
        local.chosenOpponentTargets = engine.has_chosen_opponent_id();
        local.needsTarget = false;
        local.deferredReturnTappedCreature = false;
        local.flexPips.clear();
        local.flexPayments.clear();
        if (initializePayment) {
            local.remainingCost.clear();
            local.targetingCostApplied = true;
        }
        return;
    }
    if (local.deferredReturnTappedCreature) {
        RuledCostChoice choice;
        choice.costIndex = static_cast<int>(engine.return_tapped_creature_cost_index());
        choice.zone = RuledCostChoiceZone::Battlefield;
        choice.min = 1;
        choice.max = 1;
        choice.kind = RuledCostChoiceKind::ReturnTappedCreature;
        for (const auto &candidate : engine.return_tapped_creature_candidates()) {
            if (candidate.object_id() == 0)
                continue;
            choice.candidateIds.insert(candidate.object_id());
            choice.candidateGenerations.insert(candidate.object_id(), candidate.zone_change_generation());
        }
        local.costChoices = {choice};
        local.costSelections.clear();
        for (const auto &selection : previousCostSelections) {
            if (selection.costIndex == choice.costIndex && selection.zone == choice.zone &&
                selection.selectedIds.size() == 1 && selection.selectedGenerations.size() == 1 &&
                choice.candidateGenerations.value(selection.selectedIds.first(), 0) ==
                    selection.selectedGenerations.first()) {
                local.costSelections.append(selection);
                break;
            }
        }
        const bool hasSelection = !local.costSelections.isEmpty();
        local.nextCostChoice = wasWaitingForCost ? 0 : (hasSelection ? 1 : 0);
        local.waitingForCost = wasWaitingForCost && !choice.candidateIds.isEmpty();
        if (wasWaitingForCost && !hasSelection && choice.candidateIds.isEmpty())
            local.waitingForReturnTappedCreatureCandidate = true;
        local.waitingForMana = paying && !local.waitingForCost;
    } else if (paying && initializePayment) {
        local.costChoices.clear();
        local.costSelections.clear();
        local.nextCostChoice = 0;
    }
    if (paying && initializePayment) {
        local.fixedFlexPayments.clear();
        local.flexPayments.clear();
        for (const auto &payment : engine.flex_payments())
            local.flexPayments.append(payment);
        local.remainingCost = RuledPendingCast::parseSimpleManaCost(QString::fromStdString(engine.locked_total_cost()));
        local.flexPips = RuledPendingCast::parseFlexPips(QString::fromStdString(engine.locked_total_cost()));
        local.targetingCostApplied = true;
    }
}

[[nodiscard]] inline std::optional<ruled::v1::CostObjectRef>
ruledReturnTappedCreatureSelection(const PendingActivatedAbility &local)
{
    if (!local.deferredReturnTappedCreature || local.costChoices.size() != 1 || local.costSelections.size() != 1)
        return std::nullopt;
    const auto &choice = local.costChoices.first();
    const auto &selection = local.costSelections.first();
    if (choice.kind != RuledCostChoiceKind::ReturnTappedCreature || selection.costIndex != choice.costIndex ||
        selection.zone != RuledCostChoiceZone::Battlefield || selection.selectedIds.size() != 1 ||
        selection.selectedGenerations.size() != 1)
        return std::nullopt;
    const quint32 objectId = selection.selectedIds.first();
    const quint64 generation = selection.selectedGenerations.first();
    if (!choice.candidateIds.contains(objectId) || choice.candidateGenerations.value(objectId, 0) != generation)
        return std::nullopt;
    ruled::v1::CostObjectRef result;
    result.set_object_id(objectId);
    result.set_zone_change_generation(generation);
    return result;
}

#endif
