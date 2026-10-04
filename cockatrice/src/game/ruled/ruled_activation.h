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
        if (pending.stage() != ruled::v1::ABILITY_ACTIVATION_STAGE_OPPONENT_TARGET)
            return std::nullopt;
        const auto offered = std::find_if(pending.target_candidates().begin(), pending.target_candidates().end(),
                                          [target](const auto &entry) { return entry.object_id() == *target; });
        if (offered == pending.target_candidates().end())
            return std::nullopt;
        *choice->mutable_target() = *offered;
    }
    return command;
}

inline void ruledApplyActivationView(PendingActivatedAbility &local,
                                     const ruled::v1::PendingAbilityActivation &engine)
{
    const bool same = local.engineTransactionId == engine.transaction_id();
    const bool submitting = same && (local.stage == PendingActivatedAbility::Stage::CommitPending ||
                                      local.stage == PendingActivatedAbility::Stage::CancelPending);
    const bool wasPaying = same && local.enginePaymentInitialized;
    local.valid = true;
    local.chosenOpponentTargets = true;
    local.engineTransactionId = engine.transaction_id();
    local.engineRevision = engine.revision();
    local.permanentOid = engine.source_object_id();
    local.expectedZoneChangeGeneration = engine.source_zone_change_generation();
    local.abilityIndex = static_cast<int>(engine.ability_index());
    local.sourceZone = ruled::v1::ABILITY_SOURCE_ZONE_BATTLEFIELD;
    local.abilityText = QString::fromStdString(engine.source_description());
    local.needsTarget = true;
    local.waitingForTarget = false;
    local.waitingForCost = false;
    local.selectedTargets.clear();
    for (const auto &target : engine.announced_targets()) {
        PendingActivatedAbility::Target selected;
        selected.ref.set_object_id(target.object_id());
        selected.ref.set_group_index(target.group_index());
        selected.ref.set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
        selected.zoneChangeGeneration = target.zone_change_generation();
        local.selectedTargets.append(selected);
    }
    const bool paying = engine.stage() == ruled::v1::ABILITY_ACTIVATION_STAGE_PAYMENT;
    if (!submitting)
        local.stage = paying ? PendingActivatedAbility::Stage::Paying : PendingActivatedAbility::Stage::Waiting;
    local.waitingForMana = paying;
    if (paying && !wasPaying) {
        local.enginePaymentInitialized = true;
        local.remainingCost = RuledPendingCast::parseSimpleManaCost(QString::fromStdString(engine.locked_total_cost()));
        local.flexPips = RuledPendingCast::parseFlexPips(QString::fromStdString(engine.locked_total_cost()));
        local.targetingCostApplied = true;
        local.costChoices.clear();
    }
}

#endif
