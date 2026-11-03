#include "ruled_pending_cast.h"

#include "ruled_client_host.h"

#include <QCheckBox>
#include <QCursor>
#include <QHBoxLayout>
#include <QLabel>
#include <QMenu>
#include <QPushButton>
#include <QVBoxLayout>
#include <QWidgetAction>

#include <map>
#include <tuple>

namespace
{

using CounterInventoryKey = std::tuple<quint32, quint64, quint32>;
using CounterContributionKey = std::tuple<int, quint32, quint64, quint32>;

bool isKnownAbilitySourceZone(ruled::v1::AbilitySourceZone sourceZone)
{
    switch (sourceZone) {
        case ruled::v1::ABILITY_SOURCE_ZONE_BATTLEFIELD:
        case ruled::v1::ABILITY_SOURCE_ZONE_HAND:
        case ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD:
            return true;
        default:
            return false;
    }
}

bool addCounterCount(quint64 count, quint64 &total)
{
    if (count > std::numeric_limits<quint64>::max() - total) {
        return false;
    }
    total += count;
    return true;
}

const RuledCounterRemovalGroupCandidate *findCounterCandidate(const RuledCounterRemovalGroup &group, quint32 objectId)
{
    const RuledCounterRemovalGroupCandidate *found = nullptr;
    for (const auto &candidate : group.candidates) {
        if (candidate.objectId != objectId) {
            continue;
        }
        if (found) {
            return nullptr;
        }
        found = &candidate;
    }
    return found;
}

const RuledCounterRemovalOption *findCounterOption(const RuledCounterRemovalGroupCandidate &candidate,
                                                   quint32 optionId)
{
    const RuledCounterRemovalOption *found = nullptr;
    for (const auto &option : candidate.options) {
        if (option.optionId != optionId) {
            continue;
        }
        if (found) {
            return nullptr;
        }
        found = &option;
    }
    return found;
}

bool validCounterGroupChoice(const RuledCostChoice &choice)
{
    if (choice.costIndex < 0 || choice.zone != RuledCostChoiceZone::Battlefield ||
        choice.kind != RuledCostChoiceKind::RemoveCountersFromPermanents || !choice.counterRemovalGroup ||
        choice.counterRemovalGroup->requiredCount == 0 || choice.counterRemovalGroup->candidates.isEmpty()) {
        return false;
    }
    QSet<quint32> candidateObjectIds;
    for (const auto &candidate : choice.counterRemovalGroup->candidates) {
        if (candidate.objectId == 0 || candidateObjectIds.contains(candidate.objectId) || candidate.options.isEmpty()) {
            return false;
        }
        candidateObjectIds.insert(candidate.objectId);
        QSet<quint32> optionIds;
        for (const auto &option : candidate.options) {
            if (option.optionId == 0 || option.availableCount == 0 || optionIds.contains(option.optionId)) {
                return false;
            }
            optionIds.insert(option.optionId);
        }
    }
    return true;
}

bool collectCounterGroupChoices(const QVector<RuledCostChoice> &choices,
                                bool requireOnlyGroups,
                                QMap<int, const RuledCostChoice *> &groups)
{
    groups.clear();
    QSet<int> costIndices;
    for (const auto &choice : choices) {
        if (choice.costIndex < 0 || costIndices.contains(choice.costIndex)) {
            return false;
        }
        costIndices.insert(choice.costIndex);
        if (!choice.counterRemovalGroup) {
            if (requireOnlyGroups || choice.kind == RuledCostChoiceKind::RemoveCountersFromPermanents) {
                return false;
            }
            continue;
        }
        if (!validCounterGroupChoice(choice)) {
            return false;
        }
        groups.insert(choice.costIndex, &choice);
    }
    return true;
}

bool sameCounterGroupMaterial(const RuledCostChoice &left, const RuledCostChoice &right)
{
    if (!validCounterGroupChoice(left) || !validCounterGroupChoice(right) || left.costIndex != right.costIndex ||
        left.zone != right.zone || left.kind != right.kind ||
        left.counterRemovalGroup->requiredCount != right.counterRemovalGroup->requiredCount ||
        left.counterRemovalGroup->candidates.size() != right.counterRemovalGroup->candidates.size()) {
        return false;
    }
    for (const auto &candidate : left.counterRemovalGroup->candidates) {
        const auto *current = findCounterCandidate(*right.counterRemovalGroup, candidate.objectId);
        if (!current || candidate.zoneChangeGeneration != current->zoneChangeGeneration ||
            candidate.options.size() != current->options.size()) {
            return false;
        }
        for (const auto &option : candidate.options) {
            const auto *currentOption = findCounterOption(*current, option.optionId);
            if (!currentOption || option.availableCount != currentOption->availableCount) {
                return false;
            }
        }
    }
    return true;
}

bool sameCounterGroupOffers(const QVector<RuledCostChoice> &left, const QVector<RuledCostChoice> &right)
{
    QMap<int, const RuledCostChoice *> leftGroups;
    QMap<int, const RuledCostChoice *> rightGroups;
    if (!collectCounterGroupChoices(left, true, leftGroups) || !collectCounterGroupChoices(right, true, rightGroups) ||
        leftGroups.isEmpty() || leftGroups.size() != rightGroups.size()) {
        return false;
    }
    for (auto it = leftGroups.cbegin(); it != leftGroups.cend(); ++it) {
        const auto current = rightGroups.constFind(it.key());
        if (current == rightGroups.cend() || !sameCounterGroupMaterial(*it.value(), *current.value())) {
            return false;
        }
    }
    return true;
}

bool sameTypedCounterGroupOffers(const QVector<RuledCostChoice> &left, const QVector<RuledCostChoice> &right)
{
    QMap<int, const RuledCostChoice *> leftGroups;
    QMap<int, const RuledCostChoice *> rightGroups;
    if (!collectCounterGroupChoices(left, false, leftGroups) || !collectCounterGroupChoices(right, false, rightGroups) ||
        leftGroups.isEmpty() || leftGroups.size() != rightGroups.size()) {
        return false;
    }
    for (auto it = leftGroups.cbegin(); it != leftGroups.cend(); ++it) {
        const auto current = rightGroups.constFind(it.key());
        if (current == rightGroups.cend() || !sameCounterGroupMaterial(*it.value(), *current.value())) {
            return false;
        }
    }
    return true;
}

bool sameCounterContributionRows(const QVector<RuledPendingCounterContribution> &left,
                                 const QVector<RuledPendingCounterContribution> &right)
{
    if (left.size() != right.size()) {
        return false;
    }
    auto sortedLeft = left;
    auto sortedRight = right;
    const auto less = [](const auto &a, const auto &b) {
        return std::tie(a.costIndex, a.objectId, a.zoneChangeGeneration, a.optionId, a.count) <
               std::tie(b.costIndex, b.objectId, b.zoneChangeGeneration, b.optionId, b.count);
    };
    std::sort(sortedLeft.begin(), sortedLeft.end(), less);
    std::sort(sortedRight.begin(), sortedRight.end(), less);
    for (int i = 0; i < sortedLeft.size(); ++i) {
        const auto &a = sortedLeft.at(i);
        const auto &b = sortedRight.at(i);
        if (a.costIndex != b.costIndex || a.objectId != b.objectId ||
            a.zoneChangeGeneration != b.zoneChangeGeneration || a.optionId != b.optionId || a.count != b.count) {
            return false;
        }
    }
    return true;
}

bool validCounterContributionRows(const QVector<RuledCostChoice> &choices,
                                  const QVector<RuledPendingCounterContribution> &rows,
                                  bool requireComplete,
                                  bool requireOnlyGroups);

using CounterGroupFlexPipReceipt = std::map<quint32, std::tuple<QChar, QChar, int, bool, int>>;
using CounterGroupFlexPaymentReceipt = std::map<quint32, bool>;

std::optional<CounterGroupFlexPipReceipt> counterGroupFlexPipReceipt(const QVector<RuledFlexPip> &pips)
{
    CounterGroupFlexPipReceipt receipt;
    for (const auto &pip : pips) {
        if (!receipt.emplace(pip.pipIndex,
                             std::make_tuple(pip.colorA, pip.colorB, pip.generic, pip.phyrexian, pip.genericPaid))
                 .second) {
            return std::nullopt;
        }
    }
    return receipt;
}

std::optional<CounterGroupFlexPaymentReceipt>
counterGroupFlexPaymentReceipt(const QVector<ruled::v1::FlexPipPayment> &payments)
{
    CounterGroupFlexPaymentReceipt receipt;
    for (const auto &payment : payments) {
        if (!receipt.emplace(payment.pip_index(), payment.pay_life()).second) {
            return std::nullopt;
        }
    }
    return receipt;
}

bool sameCounterGroupFlexPips(const QVector<RuledFlexPip> &left, const QVector<RuledFlexPip> &right)
{
    const auto leftReceipt = counterGroupFlexPipReceipt(left);
    const auto rightReceipt = counterGroupFlexPipReceipt(right);
    return leftReceipt && rightReceipt && *leftReceipt == *rightReceipt;
}

bool sameCounterGroupFlexPayments(const QVector<ruled::v1::FlexPipPayment> &left,
                                  const QVector<ruled::v1::FlexPipPayment> &right)
{
    const auto leftReceipt = counterGroupFlexPaymentReceipt(left);
    const auto rightReceipt = counterGroupFlexPaymentReceipt(right);
    return leftReceipt && rightReceipt && *leftReceipt == *rightReceipt;
}

bool sameCounterGroupPipIndices(const QVector<quint32> &left, const QVector<quint32> &right)
{
    if (left.size() != right.size()) {
        return false;
    }
    auto sortedLeft = left;
    auto sortedRight = right;
    std::sort(sortedLeft.begin(), sortedLeft.end());
    std::sort(sortedRight.begin(), sortedRight.end());
    return std::adjacent_find(sortedLeft.cbegin(), sortedLeft.cend()) == sortedLeft.cend() &&
           std::adjacent_find(sortedRight.cbegin(), sortedRight.cend()) == sortedRight.cend() &&
           sortedLeft == sortedRight;
}

bool sameCounterGroupPromptContext(const RuledClientState &state,
                                   const PendingActivatedAbility &current,
                                   const PendingActivatedAbility &captured)
{
    if (captured.draftId == 0 || captured.draftId != current.draftId || !captured.valid || !current.valid ||
        captured.stage() != PendingActivatedAbility::Stage::Announcing ||
        current.stage() != PendingActivatedAbility::Stage::Announcing || captured.transactionId() != 0 ||
        current.transactionId() != 0 || captured.hasInitializedPayment() || current.hasInitializedPayment() ||
        captured.ownedBegin() || current.ownedBegin() || !captured.counterGroupConfirmed ||
        !current.counterGroupConfirmed || captured.permanentAction || current.permanentAction ||
        captured.permanentOid == 0 || captured.permanentOid != current.permanentOid ||
        captured.expectedZoneChangeGeneration != current.expectedZoneChangeGeneration ||
        !isKnownAbilitySourceZone(captured.sourceZone) || captured.sourceZone != current.sourceZone ||
        captured.abilityIndex < 0 || captured.abilityIndex != current.abilityIndex ||
        captured.revision() != current.revision() || captured.manaOptionIndex != current.manaOptionIndex ||
        captured.xValue != current.xValue || captured.manaSplitFirstColorCount != current.manaSplitFirstColorCount ||
        captured.castingPermissionId != current.castingPermissionId || captured.selectedMode || current.selectedMode ||
        !captured.selectedTargets.isEmpty() || !current.selectedTargets.isEmpty() || captured.needsTarget ||
        current.needsTarget || captured.waitingForTarget || current.waitingForTarget ||
        captured.chosenOpponentTargets || current.chosenOpponentTargets || captured.deferredReturnTappedCreature ||
        current.deferredReturnTappedCreature || captured.waitingForReturnTappedCreatureCandidate ||
        current.waitingForReturnTappedCreatureCandidate || captured.loyaltyXChoice || current.loyaltyXChoice ||
        !captured.costSelections.isEmpty() || !current.costSelections.isEmpty() ||
        captured.nextCostChoice != current.nextCostChoice || captured.waitingForCost != current.waitingForCost ||
        captured.waitingForMana != current.waitingForMana ||
        captured.targetingCostApplied != current.targetingCostApplied ||
        captured.restartAfterTargetInvalidation != current.restartAfterTargetInvalidation ||
        captured.remainingCost != current.remainingCost ||
        !sameCounterContributionRows(captured.counterGroupContributions, current.counterGroupContributions) ||
        !sameCounterGroupOffers(captured.costChoices, current.costChoices) ||
        !sameCounterGroupFlexPayments(captured.fixedFlexPayments, current.fixedFlexPayments) ||
        !sameCounterGroupFlexPips(captured.flexPips, current.flexPips) ||
        !sameCounterGroupPipIndices(captured.lifePipIndices, current.lifePipIndices) ||
        !validCounterContributionRows(captured.costChoices, captured.counterGroupContributions, true, true) ||
        !validCounterContributionRows(current.costChoices, current.counterGroupContributions, true, true)) {
        return false;
    }

    const auto freshSourceZone = state.abilitySourceZone(current.permanentOid);
    const auto freshSourceGeneration = state.abilitySourceGeneration(current.permanentOid);
    const auto freshAbilityIndices = state.activatedAbilityIndicesForOid(current.permanentOid);
    if (freshSourceZone != captured.sourceZone || freshSourceZone != current.sourceZone ||
        freshSourceGeneration != captured.expectedZoneChangeGeneration ||
        freshSourceGeneration != current.expectedZoneChangeGeneration ||
        !freshAbilityIndices.contains(captured.abilityIndex) || !freshAbilityIndices.contains(current.abilityIndex)) {
        return false;
    }

    const auto freshChoices = state.abilityCostChoices(current.permanentOid, current.abilityIndex);
    if (!sameCounterGroupOffers(captured.costChoices, freshChoices) ||
        !sameCounterGroupOffers(current.costChoices, freshChoices)) {
        return false;
    }

    const auto ability = state.activatedAbilityForOid(current.permanentOid, current.abilityIndex);
    return ability && sameCounterGroupFlexPips(captured.flexPips, RuledPendingCast::parseFlexPips(ability->manaCost)) &&
           sameCounterGroupFlexPips(current.flexPips, RuledPendingCast::parseFlexPips(ability->manaCost));
}

bool validCounterContributionRows(const QVector<RuledCostChoice> &choices,
                                  const QVector<RuledPendingCounterContribution> &rows,
                                  bool requireComplete,
                                  bool requireOnlyGroups)
{
    QMap<int, const RuledCostChoice *> groups;
    if (!collectCounterGroupChoices(choices, requireOnlyGroups, groups) || groups.isEmpty()) {
        return false;
    }

    std::map<quint32, quint64> objectGenerations;
    std::map<CounterInventoryKey, quint32> availabilityCeilings;
    for (auto it = groups.cbegin(); it != groups.cend(); ++it) {
        const auto &group = *it.value()->counterRemovalGroup;
        for (const auto &candidate : group.candidates) {
            const auto [generation, inserted] =
                objectGenerations.emplace(candidate.objectId, candidate.zoneChangeGeneration);
            if (!inserted && generation->second != candidate.zoneChangeGeneration) {
                return false;
            }
            for (const auto &option : candidate.options) {
                const CounterInventoryKey key{candidate.objectId, candidate.zoneChangeGeneration, option.optionId};
                const auto [ceiling, newKey] = availabilityCeilings.emplace(key, option.availableCount);
                if (!newKey) {
                    ceiling->second = std::min(ceiling->second, option.availableCount);
                }
            }
        }
    }

    QMap<int, quint64> totalsByCost;
    std::map<CounterInventoryKey, quint64> usedByObjectAndKind;
    std::map<CounterContributionKey, bool> seenRows;
    for (const auto &row : rows) {
        const auto groupIt = groups.constFind(row.costIndex);
        if (groupIt == groups.cend() || row.objectId == 0 || row.optionId == 0 || row.count == 0) {
            return false;
        }
        const auto &group = *groupIt.value()->counterRemovalGroup;
        const auto *candidate = findCounterCandidate(group, row.objectId);
        if (!candidate || candidate->zoneChangeGeneration != row.zoneChangeGeneration) {
            return false;
        }
        const auto *option = findCounterOption(*candidate, row.optionId);
        if (!option || row.count > option->availableCount) {
            return false;
        }

        const CounterContributionKey rowKey{row.costIndex, row.objectId, row.zoneChangeGeneration, row.optionId};
        if (!seenRows.emplace(rowKey, true).second) {
            return false;
        }
        auto total = totalsByCost.value(row.costIndex, 0);
        if (!addCounterCount(row.count, total) || total > group.requiredCount) {
            return false;
        }
        totalsByCost.insert(row.costIndex, total);

        const CounterInventoryKey inventoryKey{row.objectId, row.zoneChangeGeneration, row.optionId};
        auto used = usedByObjectAndKind[inventoryKey];
        if (!addCounterCount(row.count, used)) {
            return false;
        }
        usedByObjectAndKind[inventoryKey] = used;
    }

    for (const auto &usage : usedByObjectAndKind) {
        const auto ceiling = availabilityCeilings.find(usage.first);
        if (ceiling == availabilityCeilings.end() || usage.second > ceiling->second) {
            return false;
        }
    }
    if (requireComplete) {
        for (auto it = groups.cbegin(); it != groups.cend(); ++it) {
            if (totalsByCost.value(it.key(), 0) != it.value()->counterRemovalGroup->requiredCount) {
                return false;
            }
        }
    }
    return true;
}

bool validCurrentCounterGroupDraftSource(const RuledClientState &state,
                                         const PendingActivatedAbility &pending)
{
    if (!pending.valid || pending.stage() != PendingActivatedAbility::Stage::Announcing ||
        pending.transactionId() != 0 || pending.hasInitializedPayment() || pending.permanentAction ||
        pending.permanentOid == 0 || pending.abilityIndex < 0 || !isKnownAbilitySourceZone(pending.sourceZone) ||
        state.abilitySourceZone(pending.permanentOid) != pending.sourceZone ||
        state.abilitySourceGeneration(pending.permanentOid) != pending.expectedZoneChangeGeneration ||
        !state.activatedAbilityIndicesForOid(pending.permanentOid).contains(pending.abilityIndex)) {
        return false;
    }
    const auto ability = state.activatedAbilityForOid(pending.permanentOid, pending.abilityIndex);
    const auto key = RuledClientState::abilityTargetKey(pending.permanentOid, pending.abilityIndex);
    return ability && ability->activatable && state.abilityActivatable(pending.permanentOid, pending.abilityIndex) &&
           state.abilityCostData.contains(key);
}

bool validCurrentCounterGroupDraft(const RuledClientState &state,
                                   const PendingActivatedAbility &pending,
                                   const QVector<RuledCostChoice> &currentChoices)
{
    if (!validCurrentCounterGroupDraftSource(state, pending) ||
        !sameTypedCounterGroupOffers(pending.costChoices, currentChoices) ||
        !validCounterContributionRows(currentChoices, pending.counterGroupContributions, false, false)) {
        return false;
    }
    return !pending.counterGroupConfirmed ||
           validCounterContributionRows(currentChoices, pending.counterGroupContributions, true, true);
}

} // namespace

std::optional<RuledCounterQuantityDialog::Context>
ruledCounterQuantityContext(const RuledClientState &state,
                            const PendingActivatedAbility &pending,
                            int costIndex,
                            quint32 objectId,
                            quint64 zoneChangeGeneration,
                            const QString &displayName)
{
    if (costIndex < 0 || objectId == 0 || !validCurrentCounterGroupDraftSource(state, pending)) {
        return std::nullopt;
    }

    const auto currentChoices = state.abilityCostChoices(pending.permanentOid, pending.abilityIndex);
    if (!validCurrentCounterGroupDraft(state, pending, currentChoices)) {
        return std::nullopt;
    }

    QMap<int, const RuledCostChoice *> groups;
    if (!collectCounterGroupChoices(currentChoices, false, groups)) {
        return std::nullopt;
    }
    const auto selectedGroup = groups.constFind(costIndex);
    if (selectedGroup == groups.cend()) {
        return std::nullopt;
    }
    const auto &group = *selectedGroup.value()->counterRemovalGroup;
    const auto *candidate = findCounterCandidate(group, objectId);
    if (!candidate || candidate->zoneChangeGeneration != zoneChangeGeneration) {
        return std::nullopt;
    }

    std::map<CounterInventoryKey, quint64> sharedAvailability;
    for (auto it = groups.cbegin(); it != groups.cend(); ++it) {
        for (const auto &offeredCandidate : it.value()->counterRemovalGroup->candidates) {
            for (const auto &option : offeredCandidate.options) {
                const CounterInventoryKey key{
                    offeredCandidate.objectId, offeredCandidate.zoneChangeGeneration, option.optionId};
                const auto [ceiling, inserted] = sharedAvailability.emplace(key, option.availableCount);
                if (!inserted) {
                    ceiling->second = std::min<quint64>(ceiling->second, option.availableCount);
                }
            }
        }
    }

    RuledCounterQuantityDialog::Context context;
    context.costIndex = static_cast<quint32>(costIndex);
    context.objectId = objectId;
    context.zoneChangeGeneration = zoneChangeGeneration;
    context.displayName = displayName;
    quint64 assignedToOtherObjects = 0;
    for (const auto &row : pending.counterGroupContributions) {
        if (row.costIndex == costIndex && row.objectId != objectId &&
            !addCounterCount(row.count, assignedToOtherObjects)) {
            return std::nullopt;
        }
    }
    if (assignedToOtherObjects > group.requiredCount) {
        return std::nullopt;
    }
    context.remainingForThisObject =
        static_cast<quint32>(static_cast<quint64>(group.requiredCount) - assignedToOtherObjects);

    std::map<quint32, quint64> initialQuantities;
    for (const auto &row : pending.counterGroupContributions) {
        if (row.costIndex == costIndex && row.objectId == objectId &&
            row.zoneChangeGeneration == zoneChangeGeneration) {
            auto &quantity = initialQuantities[row.optionId];
            if (!addCounterCount(row.count, quantity) || quantity > std::numeric_limits<quint32>::max()) {
                return std::nullopt;
            }
        }
    }

    for (const auto &option : candidate->options) {
        const CounterInventoryKey key{objectId, zoneChangeGeneration, option.optionId};
        const auto ceiling = sharedAvailability.find(key);
        if (ceiling == sharedAvailability.end()) {
            return std::nullopt;
        }
        quint64 reservedElsewhere = 0;
        for (const auto &row : pending.counterGroupContributions) {
            const bool replacedReceipt = row.costIndex == costIndex && row.objectId == objectId &&
                                         row.zoneChangeGeneration == zoneChangeGeneration;
            if (!replacedReceipt && row.objectId == objectId && row.zoneChangeGeneration == zoneChangeGeneration &&
                row.optionId == option.optionId && !addCounterCount(row.count, reservedElsewhere)) {
                return std::nullopt;
            }
        }
        if (reservedElsewhere > ceiling->second) {
            return std::nullopt;
        }
        const quint64 available = ceiling->second - reservedElsewhere;
        if (available > std::numeric_limits<quint32>::max()) {
            return std::nullopt;
        }
        if (available == 0) {
            continue;
        }
        context.offeredOptions.append({option.optionId, option.label, static_cast<quint32>(available)});
    }
    if (context.offeredOptions.isEmpty()) {
        return std::nullopt;
    }

    quint64 initialTotal = 0;
    for (const auto &entry : initialQuantities) {
        const auto offered = std::find_if(context.offeredOptions.cbegin(),
                                          context.offeredOptions.cend(),
                                          [&entry](const auto &option) { return option.optionId == entry.first; });
        if (offered == context.offeredOptions.cend() || entry.second > offered->availableCount ||
            !addCounterCount(entry.second, initialTotal) || initialTotal > context.remainingForThisObject) {
            return std::nullopt;
        }
        context.initialQuantities.insert(entry.first, static_cast<quint32>(entry.second));
    }
    return context;
}

std::optional<QVector<RuledPendingCounterContribution>>
ruledCounterQuantityRows(const RuledCounterQuantityDialog::Context &context,
                         const RuledCounterQuantityDialog::Result &result)
{
    if (context.costIndex > static_cast<quint32>(std::numeric_limits<int>::max()) || context.objectId == 0 ||
        context.offeredOptions.isEmpty() || result.costIndex != context.costIndex || result.objectId != context.objectId ||
        result.zoneChangeGeneration != context.zoneChangeGeneration) {
        return std::nullopt;
    }

    QHash<quint32, quint32> availableByOption;
    for (const auto &option : context.offeredOptions) {
        if (option.optionId == 0 || option.availableCount == 0 || availableByOption.contains(option.optionId)) {
            return std::nullopt;
        }
        availableByOption.insert(option.optionId, option.availableCount);
    }

    quint64 initialTotal = 0;
    for (auto it = context.initialQuantities.cbegin(); it != context.initialQuantities.cend(); ++it) {
        const auto offered = availableByOption.constFind(it.key());
        if (offered == availableByOption.cend() || it.value() > offered.value() ||
            !addCounterCount(it.value(), initialTotal) || initialTotal > context.remainingForThisObject) {
            return std::nullopt;
        }
    }

    QVector<RuledPendingCounterContribution> rows;
    QSet<quint32> seenOptions;
    quint64 total = 0;
    for (const auto &selection : result.selections) {
        if (selection.optionId == 0 || seenOptions.contains(selection.optionId) ||
            !availableByOption.contains(selection.optionId) ||
            selection.count > availableByOption.value(selection.optionId) || !addCounterCount(selection.count, total) ||
            total > context.remainingForThisObject) {
            return std::nullopt;
        }
        seenOptions.insert(selection.optionId);
        if (selection.count == 0) {
            continue;
        }
        const auto option = std::find_if(context.offeredOptions.cbegin(),
                                         context.offeredOptions.cend(),
                                         [&selection](const auto &entry) { return entry.optionId == selection.optionId; });
        rows.append({static_cast<int>(result.costIndex),
                     result.objectId,
                     result.zoneChangeGeneration,
                     selection.optionId,
                     selection.count,
                     option->label});
    }
    return rows;
}

QVector<RuledFlexPip> RuledPendingCast::parseFlexPips(const QString &manaCost)
{
    // Walk the Scryfall brace groups in order so each pip's index matches the engine's
    // ManaCost pip order. Flexible pips (CR 107.4d–f) contain a slash: {G/U} hybrid,
    // {2/W} mono-hybrid, {C/P} Phyrexian. Everything else just advances the index.
    QVector<RuledFlexPip> out;
    const QString validColors = QStringLiteral("WUBRG");
    quint32 index = 0;
    bool inBraces = false;
    QString token;
    for (QChar c : manaCost) {
        if (c == '{') {
            inBraces = true;
            token.clear();
            continue;
        }
        if (c == '}') {
            inBraces = false;
            const int slash = token.indexOf('/');
            if (slash > 0) {
                const QString left = token.left(slash).toUpper();
                const QString right = token.mid(slash + 1).toUpper();
                RuledFlexPip pip;
                pip.pipIndex = index;
                bool numeric = false;
                const int leftNum = left.toInt(&numeric);
                if (right == QLatin1String("P") && left.size() == 1 && validColors.contains(left)) {
                    pip.phyrexian = true;
                    pip.colorA = left.at(0);
                    out.append(pip);
                } else if (numeric && right.size() == 1 && validColors.contains(right)) {
                    pip.generic = leftNum;
                    pip.colorA = right.at(0);
                    out.append(pip);
                } else if (left.size() == 1 && right.size() == 1 && validColors.contains(left) &&
                           validColors.contains(right)) {
                    pip.colorA = left.at(0);
                    pip.colorB = right.at(0);
                    out.append(pip);
                }
                // Unrecognized slash forms (e.g. {G/U/P}, {S}) are left for the engine to reject.
            }
            ++index;
            token.clear();
            continue;
        }
        if (inBraces) {
            token.append(c);
        }
    }
    return out;
}

RuledPendingCast::RuledPendingCast() = default;

bool RuledPendingCast::chooseCounterCosts(QWidget *parent, PendingActivatedAbility &pending)
{
    while (pending.valid && pending.nextCostChoice < pending.costChoices.size()) {
        const auto choice = pending.costChoices.at(pending.nextCostChoice);
        if (choice.kind != RuledCostChoiceKind::RemoveCounters)
            break;
        if (choice.counterOptions.isEmpty() || choice.counterCount == 0)
            return false;
        if (choice.counterSourceId == 0)
            break;
        if (choice.counterSourceId != pending.permanentOid ||
            choice.counterSourceGeneration != pending.expectedZoneChangeGeneration)
            return false;
        quint32 optionId = choice.counterOptions.front().optionId;
        const int position = pending.nextCostChoice;
        const int abilityIndex = pending.abilityIndex;
        if (choice.counterOptions.size() > 1) {
            QMenu menu(parent);
            menu.addSection(QObject::tr("Remove %1 counter(s) from %2").arg(choice.counterCount).arg(pending.cardName));
            for (const auto &option : choice.counterOptions) {
                auto *action =
                    menu.addAction(QObject::tr("%1 (%2 available)").arg(option.label).arg(option.availableCount));
                action->setData(option.optionId);
            }
            QAction *chosen = menu.exec(QCursor::pos());
            if (!chosen)
                return false;
            optionId = chosen->data().toUInt();
        }
        // The nested menu loop may receive a new engine batch or cancel the transaction.
        RuledPendingCostSelection selection{
            choice.costIndex, choice.zone, {choice.counterSourceId}, {choice.counterSourceGeneration}, optionId};
        if (!pending.valid || pending.abilityIndex != abilityIndex || pending.nextCostChoice != position ||
            pending.permanentOid != choice.counterSourceId ||
            pending.expectedZoneChangeGeneration != choice.counterSourceGeneration ||
            position >= pending.costChoices.size() ||
            !ruledCounterSelectionStillLegal(selection, pending.costChoices.at(position)))
            return false;
        pending.costSelections.append(selection);
        ++pending.nextCostChoice;
    }
    return pending.valid;
}

bool ruledStageCounterContributions(PendingActivatedAbility &pending,
                                    int costIndex,
                                    quint32 objectId,
                                    quint64 zoneChangeGeneration,
                                    const QVector<RuledPendingCounterContribution> &replacementRows)
{
    if (!pending.valid || pending.stage() != PendingActivatedAbility::Stage::Announcing ||
        pending.transactionId() != 0 || pending.permanentAction || costIndex < 0 || objectId == 0) {
        return false;
    }
    if (replacementRows.isEmpty()) {
        pending.counterGroupContributions.erase(
            std::remove_if(pending.counterGroupContributions.begin(), pending.counterGroupContributions.end(),
                           [=](const auto &row) {
                               return row.costIndex == costIndex && row.objectId == objectId &&
                                      row.zoneChangeGeneration == zoneChangeGeneration;
                           }),
            pending.counterGroupContributions.end());
        pending.counterGroupConfirmed = false;
        return true;
    }

    QMap<int, const RuledCostChoice *> groups;
    if (!collectCounterGroupChoices(pending.costChoices, false, groups)) {
        return false;
    }
    const auto groupIt = groups.constFind(costIndex);
    if (groupIt == groups.cend() ||
        !findCounterCandidate(*groupIt.value()->counterRemovalGroup, objectId)) {
        return false;
    }
    const auto *candidate = findCounterCandidate(*groupIt.value()->counterRemovalGroup, objectId);
    if (!candidate || candidate->zoneChangeGeneration != zoneChangeGeneration) {
        return false;
    }

    QVector<RuledPendingCounterContribution> updated;
    for (const auto &row : pending.counterGroupContributions) {
        if (row.costIndex == costIndex && row.objectId == objectId && row.zoneChangeGeneration == zoneChangeGeneration) {
            continue;
        }
        updated.append(row);
    }
    QSet<quint32> optionIds;
    QVector<RuledPendingCounterContribution> normalizedRows;
    for (const auto &row : replacementRows) {
        if (row.costIndex != costIndex || row.objectId != objectId ||
            row.zoneChangeGeneration != zoneChangeGeneration || row.optionId == 0 || optionIds.contains(row.optionId)) {
            return false;
        }
        optionIds.insert(row.optionId);
        if (!findCounterOption(*candidate, row.optionId)) {
            return false;
        }
        if (row.count != 0) {
            normalizedRows.append(row);
        }
    }
    updated += normalizedRows;
    if (!validCounterContributionRows(pending.costChoices, updated, false, false)) {
        return false;
    }
    pending.counterGroupContributions = updated;
    pending.counterGroupConfirmed = false;
    return true;
}

bool ruledReconcileCounterContributions(PendingActivatedAbility &pending,
                                        const QVector<RuledCostChoice> &currentChoices)
{
    if (!pending.valid || pending.stage() != PendingActivatedAbility::Stage::Announcing ||
        pending.transactionId() != 0 || pending.permanentAction) {
        return false;
    }

    QMap<int, const RuledCostChoice *> previousGroups;
    QMap<int, const RuledCostChoice *> currentGroups;
    if (!collectCounterGroupChoices(pending.costChoices, false, previousGroups) ||
        !collectCounterGroupChoices(currentChoices, false, currentGroups)) {
        pending.counterGroupContributions.clear();
        pending.counterGroupConfirmed = false;
        pending.costChoices = currentChoices;
        return false;
    }

    QSet<int> changedCostIndices;
    for (auto it = previousGroups.cbegin(); it != previousGroups.cend(); ++it) {
        const auto current = currentGroups.constFind(it.key());
        if (current == currentGroups.cend() || !sameCounterGroupMaterial(*it.value(), *current.value())) {
            changedCostIndices.insert(it.key());
        }
    }
    for (auto it = currentGroups.cbegin(); it != currentGroups.cend(); ++it) {
        if (!previousGroups.contains(it.key())) {
            changedCostIndices.insert(it.key());
        }
    }

    const bool materialChanged = !changedCostIndices.isEmpty();
    if (materialChanged) {
        pending.counterGroupContributions.erase(
            std::remove_if(pending.counterGroupContributions.begin(), pending.counterGroupContributions.end(),
                           [&changedCostIndices](const auto &row) { return changedCostIndices.contains(row.costIndex); }),
            pending.counterGroupContributions.end());
        pending.counterGroupConfirmed = false;
    }

    const bool rowsValid = validCounterContributionRows(currentChoices, pending.counterGroupContributions, false, false);
    if (!rowsValid) {
        pending.counterGroupContributions.clear();
        pending.counterGroupConfirmed = false;
    } else if (pending.counterGroupConfirmed &&
               !validCounterContributionRows(currentChoices, pending.counterGroupContributions, true, true)) {
        pending.counterGroupConfirmed = false;
    }

    const bool confirmationStillValid = !pending.counterGroupConfirmed ||
                                        validCounterContributionRows(
                                            currentChoices, pending.counterGroupContributions, true, true);
    pending.costChoices = currentChoices;
    return !materialChanged && rowsValid && confirmationStillValid;
}

bool ruledConfirmCounterContributions(PendingActivatedAbility &pending)
{
    if (!pending.valid || pending.stage() != PendingActivatedAbility::Stage::Announcing ||
        pending.transactionId() != 0 || pending.permanentAction ||
        !validCounterContributionRows(pending.costChoices, pending.counterGroupContributions, true, true)) {
        pending.counterGroupConfirmed = false;
        return false;
    }
    pending.counterGroupConfirmed = true;
    return true;
}

std::optional<ruled::v1::BeginAbilityActivation> ruledBuildCounterGroupBegin(
    const RuledClientState &state,
    const PendingActivatedAbility &pending,
    const QVector<ruled::v1::FlexPipPayment> &flexPayments)
{
    if (!pending.valid || pending.stage() != PendingActivatedAbility::Stage::Announcing ||
        pending.transactionId() != 0 || pending.permanentAction || !pending.counterGroupConfirmed ||
        pending.permanentOid == 0 || pending.abilityIndex < 0 || pending.selectedMode ||
        !pending.selectedTargets.isEmpty() || pending.needsTarget || pending.waitingForTarget ||
        pending.chosenOpponentTargets || pending.deferredReturnTappedCreature ||
        pending.waitingForReturnTappedCreatureCandidate || pending.xValue != 0 || pending.manaOptionIndex != 0 ||
        pending.manaSplitFirstColorCount != 0 || pending.castingPermissionId != 0 || pending.loyaltyXChoice ||
        !pending.costSelections.isEmpty()) {
        return std::nullopt;
    }

    const auto currentSourceZone = state.abilitySourceZone(pending.permanentOid);
    if (!isKnownAbilitySourceZone(pending.sourceZone) || !isKnownAbilitySourceZone(currentSourceZone) ||
        state.abilitySourceGeneration(pending.permanentOid) != pending.expectedZoneChangeGeneration ||
        currentSourceZone != pending.sourceZone ||
        !state.activatedAbilityIndicesForOid(pending.permanentOid).contains(pending.abilityIndex)) {
        return std::nullopt;
    }
    const auto ability = state.activatedAbilityForOid(pending.permanentOid, pending.abilityIndex);
    if (!ability || !ability->activatable || !state.abilityActivatable(pending.permanentOid, pending.abilityIndex) ||
        ability->xCounterManaChoice || ability->loyaltyXChoice ||
        ability->manaCost.contains(QLatin1Char('X'), Qt::CaseInsensitive)) {
        return std::nullopt;
    }
    const auto abilityKey = RuledClientState::abilityTargetKey(pending.permanentOid, pending.abilityIndex);
    if (!state.abilityCostData.contains(abilityKey) ||
        state.abilityNeedsTarget(pending.permanentOid, pending.abilityIndex) ||
        state.activatedAbilityModesByAbility.contains(abilityKey)) {
        return std::nullopt;
    }
    const auto currentChoices = state.abilityCostChoices(pending.permanentOid, pending.abilityIndex);
    if (!sameCounterGroupOffers(pending.costChoices, currentChoices) ||
        !validCounterContributionRows(currentChoices, pending.counterGroupContributions, true, true)) {
        return std::nullopt;
    }

    QSet<quint32> expectedPhiIndices;
    QSet<quint32> allPipIndices;
    const auto currentFlexPips = RuledPendingCast::parseFlexPips(ability->manaCost);
    if (currentFlexPips.size() != pending.flexPips.size()) {
        return std::nullopt;
    }
    for (int pipPosition = 0; pipPosition < currentFlexPips.size(); ++pipPosition) {
        const auto &pip = currentFlexPips.at(pipPosition);
        const auto &cachedPip = pending.flexPips.at(pipPosition);
        if (pip.pipIndex != cachedPip.pipIndex || pip.colorA != cachedPip.colorA ||
            pip.colorB != cachedPip.colorB || pip.generic != cachedPip.generic ||
            pip.phyrexian != cachedPip.phyrexian || pip.genericPaid != cachedPip.genericPaid ||
            allPipIndices.contains(pip.pipIndex)) {
            return std::nullopt;
        }
        allPipIndices.insert(pip.pipIndex);
        if (pip.phyrexian) {
            expectedPhiIndices.insert(pip.pipIndex);
        }
    }
    QSet<quint32> suppliedPhiIndices;
    for (const auto &payment : flexPayments) {
        if (!expectedPhiIndices.contains(payment.pip_index()) || suppliedPhiIndices.contains(payment.pip_index())) {
            return std::nullopt;
        }
        suppliedPhiIndices.insert(payment.pip_index());
    }
    if (suppliedPhiIndices.size() != expectedPhiIndices.size()) {
        return std::nullopt;
    }

    ruled::v1::BeginAbilityActivation begin;
    begin.set_source_object_id(pending.permanentOid);
    begin.set_expected_zone_change_generation(pending.expectedZoneChangeGeneration);
    begin.set_source_zone(pending.sourceZone);
    begin.set_ability_index(static_cast<quint32>(pending.abilityIndex));

    auto sortedRows = pending.counterGroupContributions;
    std::sort(sortedRows.begin(), sortedRows.end(), [](const auto &left, const auto &right) {
        return std::tie(left.costIndex, left.objectId, left.zoneChangeGeneration, left.optionId) <
               std::tie(right.costIndex, right.objectId, right.zoneChangeGeneration, right.optionId);
    });
    ruled::v1::CostSelection *selection = nullptr;
    int activeCostIndex = -1;
    for (const auto &row : sortedRows) {
        if (!selection || activeCostIndex != row.costIndex) {
            selection = begin.add_cost_selections();
            selection->set_cost_index(static_cast<quint32>(row.costIndex));
            activeCostIndex = row.costIndex;
        }
        auto *contribution = selection->mutable_counter_removal_group()->add_contributions();
        contribution->mutable_object()->set_object_id(row.objectId);
        contribution->mutable_object()->set_zone_change_generation(row.zoneChangeGeneration);
        contribution->set_option_id(row.optionId);
        contribution->set_count(row.count);
    }

    auto sortedFlexPayments = flexPayments;
    std::sort(sortedFlexPayments.begin(), sortedFlexPayments.end(), [](const auto &left, const auto &right) {
        return left.pip_index() < right.pip_index();
    });
    for (const auto &payment : sortedFlexPayments) {
        begin.add_flex_payments()->CopyFrom(payment);
    }
    return begin;
}

PendingRuledSpellCast &RuledPendingCast::beginSpell()
{
    ability = {};
    spell = {};
    spell.draftId = ++nextSpellDraftId;
    spell.valid = true;
    return spell;
}

PendingActivatedAbility &RuledPendingCast::beginAbility()
{
    spell = {};
    ability = {};
    if (++nextAbilityDraftId == 0) {
        ++nextAbilityDraftId;
    }
    ability.draftId = nextAbilityDraftId;
    ability.valid = true;
    return ability;
}

bool RuledPendingCast::rejectOrdinaryAbilityBegin(quint64 capturedDraftId)
{
    if (!ability.valid || capturedDraftId == 0 || ability.draftId != capturedDraftId || ability.ownedBegin() ||
        ability.stage() != PendingActivatedAbility::Stage::BeginPending) {
        return false;
    }
    ability.rejectBegin();
    return true;
}

bool RuledPendingCast::stageCounterContributionsAfterPrompt(
    const RuledClientState &state,
    const PendingActivatedAbility &capturedPending,
    int costIndex,
    quint32 objectId,
    quint64 zoneChangeGeneration,
    const QVector<RuledPendingCounterContribution> &replacementRows)
{
    const auto &pending = ability;
    if (capturedPending.draftId == 0 || pending.draftId == 0 || capturedPending.draftId != pending.draftId ||
        !capturedPending.valid || !pending.valid ||
        capturedPending.stage() != PendingActivatedAbility::Stage::Announcing ||
        pending.stage() != PendingActivatedAbility::Stage::Announcing || capturedPending.transactionId() != 0 ||
        pending.transactionId() != 0 || capturedPending.hasInitializedPayment() || pending.hasInitializedPayment() ||
        capturedPending.permanentAction || pending.permanentAction || capturedPending.permanentOid == 0 ||
        pending.permanentOid != capturedPending.permanentOid || pending.sourceZone != capturedPending.sourceZone ||
        pending.expectedZoneChangeGeneration != capturedPending.expectedZoneChangeGeneration ||
        pending.abilityIndex < 0 || pending.abilityIndex != capturedPending.abilityIndex ||
        !capturedPending.waitingForCost || !pending.waitingForCost ||
        pending.nextCostChoice != capturedPending.nextCostChoice || pending.nextCostChoice < 0 ||
        pending.nextCostChoice >= pending.costChoices.size() || capturedPending.nextCostChoice < 0 ||
        capturedPending.nextCostChoice >= capturedPending.costChoices.size() ||
        !sameCounterContributionRows(capturedPending.counterGroupContributions, pending.counterGroupContributions) ||
        pending.counterGroupConfirmed != capturedPending.counterGroupConfirmed) {
        return false;
    }

    const auto &capturedActiveChoice = capturedPending.costChoices.at(capturedPending.nextCostChoice);
    const auto &activeChoice = pending.costChoices.at(pending.nextCostChoice);
    if (capturedActiveChoice.costIndex != costIndex || activeChoice.costIndex != costIndex ||
        !validCounterGroupChoice(capturedActiveChoice) || !validCounterGroupChoice(activeChoice) ||
        !validCounterContributionRows(pending.costChoices,
                                      pending.counterGroupContributions,
                                      pending.counterGroupConfirmed,
                                      true) ||
        !sameCounterGroupOffers(capturedPending.costChoices, pending.costChoices)) {
        return false;
    }

    if (!isKnownAbilitySourceZone(pending.sourceZone) ||
        state.abilitySourceZone(pending.permanentOid) != pending.sourceZone ||
        state.abilitySourceGeneration(pending.permanentOid) != pending.expectedZoneChangeGeneration ||
        !state.activatedAbilityIndicesForOid(pending.permanentOid).contains(pending.abilityIndex)) {
        return false;
    }
    const auto offeredAbility = state.activatedAbilityForOid(pending.permanentOid, pending.abilityIndex);
    if (!offeredAbility || !offeredAbility->activatable) {
        return false;
    }

    const auto currentChoices = state.abilityCostChoices(pending.permanentOid, pending.abilityIndex);
    if (pending.nextCostChoice >= currentChoices.size() || !sameCounterGroupOffers(pending.costChoices, currentChoices)) {
        return false;
    }
    const auto &currentActiveChoice = currentChoices.at(pending.nextCostChoice);
    if (currentActiveChoice.costIndex != costIndex || !validCounterGroupChoice(currentActiveChoice)) {
        return false;
    }
    QMap<int, const RuledCostChoice *> capturedGroups;
    QMap<int, const RuledCostChoice *> cachedGroups;
    QMap<int, const RuledCostChoice *> currentGroups;
    if (!collectCounterGroupChoices(capturedPending.costChoices, true, capturedGroups) ||
        !collectCounterGroupChoices(pending.costChoices, true, cachedGroups) ||
        !collectCounterGroupChoices(currentChoices, true, currentGroups)) {
        return false;
    }
    const auto capturedGroup = capturedGroups.constFind(costIndex);
    const auto cachedGroup = cachedGroups.constFind(costIndex);
    const auto currentGroup = currentGroups.constFind(costIndex);
    if (capturedGroup == capturedGroups.cend() || cachedGroup == cachedGroups.cend() ||
        currentGroup == currentGroups.cend()) {
        return false;
    }
    const auto *capturedCandidate = findCounterCandidate(*capturedGroup.value()->counterRemovalGroup, objectId);
    const auto *cachedCandidate = findCounterCandidate(*cachedGroup.value()->counterRemovalGroup, objectId);
    const auto *currentCandidate = findCounterCandidate(*currentGroup.value()->counterRemovalGroup, objectId);
    if (!capturedCandidate || !cachedCandidate || !currentCandidate ||
        capturedCandidate->zoneChangeGeneration != zoneChangeGeneration ||
        cachedCandidate->zoneChangeGeneration != zoneChangeGeneration ||
        currentCandidate->zoneChangeGeneration != zoneChangeGeneration) {
        return false;
    }

    return ruledStageCounterContributions(ability, costIndex, objectId, zoneChangeGeneration, replacementRows);
}

bool RuledPendingCast::selectCounterGroupAfterPrompt(const RuledClientState &state,
                                                     const PendingActivatedAbility &capturedPending,
                                                     int costIndex,
                                                     quint32 objectId,
                                                     quint64 zoneChangeGeneration)
{
    auto &pending = ability;
    if (costIndex < 0 || objectId == 0 || capturedPending.draftId == 0 || pending.draftId == 0 ||
        capturedPending.draftId != pending.draftId || !capturedPending.valid || !pending.valid ||
        capturedPending.stage() != PendingActivatedAbility::Stage::Announcing ||
        pending.stage() != PendingActivatedAbility::Stage::Announcing || capturedPending.transactionId() != 0 ||
        pending.transactionId() != 0 || capturedPending.hasInitializedPayment() || pending.hasInitializedPayment() ||
        capturedPending.permanentAction || pending.permanentAction || !capturedPending.waitingForCost ||
        !pending.waitingForCost || capturedPending.permanentOid != pending.permanentOid ||
        capturedPending.sourceZone != pending.sourceZone ||
        capturedPending.expectedZoneChangeGeneration != pending.expectedZoneChangeGeneration ||
        capturedPending.abilityIndex != pending.abilityIndex || capturedPending.nextCostChoice < 0 ||
        capturedPending.nextCostChoice >= capturedPending.costChoices.size() || pending.nextCostChoice < 0 ||
        pending.nextCostChoice >= pending.costChoices.size() ||
        !sameCounterContributionRows(capturedPending.counterGroupContributions, pending.counterGroupContributions) ||
        capturedPending.counterGroupConfirmed != pending.counterGroupConfirmed) {
        return false;
    }

    const auto &capturedActiveChoice = capturedPending.costChoices.at(capturedPending.nextCostChoice);
    const auto &currentActiveChoice = pending.costChoices.at(pending.nextCostChoice);
    if (capturedActiveChoice.costIndex != currentActiveChoice.costIndex ||
        !sameTypedCounterGroupOffers(capturedPending.costChoices, pending.costChoices)) {
        return false;
    }

    const auto currentChoices = state.abilityCostChoices(pending.permanentOid, pending.abilityIndex);
    if (!validCurrentCounterGroupDraft(state, capturedPending, currentChoices) ||
        !validCurrentCounterGroupDraft(state, pending, currentChoices)) {
        return false;
    }

    const auto findGroup = [costIndex](const QVector<RuledCostChoice> &choices) {
        return std::find_if(choices.cbegin(), choices.cend(), [costIndex](const auto &entry) {
            return entry.costIndex == costIndex &&
                   entry.kind == RuledCostChoiceKind::RemoveCountersFromPermanents &&
                   entry.counterRemovalGroup.has_value();
        });
    };
    const auto capturedChoice = findGroup(capturedPending.costChoices);
    const auto currentChoice = findGroup(pending.costChoices);
    const auto rawChoice = findGroup(currentChoices);
    if (capturedChoice == capturedPending.costChoices.cend() || currentChoice == pending.costChoices.cend() ||
        rawChoice == currentChoices.cend() || !validCounterGroupChoice(*capturedChoice) ||
        !validCounterGroupChoice(*currentChoice) || !validCounterGroupChoice(*rawChoice)) {
        return false;
    }
    const auto *capturedCandidate = findCounterCandidate(*capturedChoice->counterRemovalGroup, objectId);
    const auto *currentCandidate = findCounterCandidate(*currentChoice->counterRemovalGroup, objectId);
    const auto *rawCandidate = findCounterCandidate(*rawChoice->counterRemovalGroup, objectId);
    if (!capturedCandidate || !currentCandidate || !rawCandidate ||
        capturedCandidate->zoneChangeGeneration != zoneChangeGeneration ||
        currentCandidate->zoneChangeGeneration != zoneChangeGeneration ||
        rawCandidate->zoneChangeGeneration != zoneChangeGeneration) {
        return false;
    }

    pending.nextCostChoice = static_cast<int>(std::distance(pending.costChoices.cbegin(), currentChoice));
    return true;
}

bool RuledPendingCast::applyCounterGroupFlexibleChoiceAfterPrompt(
    const RuledClientState &state,
    const PendingActivatedAbility &capturedPending,
    const std::optional<QVector<ruled::v1::FlexPipPayment>> &result)
{
    auto &pending = ability;
    if (!sameCounterGroupPromptContext(state, pending, capturedPending)) {
        return false;
    }
    if (!result) {
        return true;
    }

    auto validatedPending = pending;
    std::sort(validatedPending.flexPips.begin(), validatedPending.flexPips.end(), [](const auto &left, const auto &right) {
        return left.pipIndex < right.pipIndex;
    });
    const auto validatedBegin = ruledBuildCounterGroupBegin(state, validatedPending, *result);
    if (!validatedBegin) {
        return false;
    }

    pending.fixedFlexPayments.clear();
    for (const auto &payment : validatedBegin->flex_payments()) {
        pending.fixedFlexPayments.append(payment);
    }
    return true;
}

void RuledPendingCast::clearSpell()
{
    spell = {};
}

void RuledPendingCast::clearAbility()
{
    ability = {};
}

RuledPendingCast::InteractionKind RuledPendingCast::activeInteraction() const
{
    if (spell.valid) {
        return InteractionKind::Spell;
    }
    if (ability.valid) {
        return InteractionKind::Ability;
    }
    return InteractionKind::None;
}

static QString ruledCastOptionLabel(const RuledFaceOption &face)
{
    QString label;
    if (face.castMethod == ruled::v1::CAST_METHOD_WARP) {
        label = QObject::tr("Warp %1").arg(face.faceName);
    } else if (face.castMethod == ruled::v1::CAST_METHOD_SNEAK) {
        label = QObject::tr("Sneak %1").arg(face.faceName);
    } else if (face.castMethod == ruled::v1::CAST_METHOD_WITHOUT_MANA_COST) {
        label = QObject::tr("Cast %1 without paying its mana cost").arg(face.faceName);
    } else {
        label = QObject::tr("Cast %1").arg(face.faceName);
    }
    if (face.prototyped) {
        label = QObject::tr("Prototype %1").arg(label);
    }
    if (!face.permissionSourceLabel.isEmpty()) {
        label += QObject::tr(" — %1").arg(face.permissionSourceLabel);
    }
    return face.manaCost.isEmpty() ? label : QStringLiteral("%1 (%2)").arg(label, face.manaCost);
}

QVector<RuledCardActionMenuOption>
RuledPendingCast::cardActionMenuOptions(const QVector<RuledFaceOption> &castFaces,
                                        const RuledClientState &state,
                                        quint32 sourceOid,
                                        bool manaAbilitiesOnly,
                                        const QVector<QPair<int, QString>> &paymentContributions)
{
    const auto abilityIndices = state.activatedAbilityIndicesForOid(sourceOid);
    QVector<RuledCardActionMenuOption> options;
    options.reserve(paymentContributions.size() + castFaces.size() + abilityIndices.size());
    for (const auto &[kind, label] : paymentContributions)
        options.append({RuledCardActionMenuOption::Kind::PaymentContribution, kind, label, true});
    for (const auto &face : castFaces) {
        if (manaAbilitiesOnly)
            break;
        options.append({RuledCardActionMenuOption::Kind::CastFace, face.faceIndex, ruledCastOptionLabel(face), true, 0,
                        face.castMethod, face.prototyped, face.castingPermissionId});
    }
    for (const int abilityIndex : abilityIndices) {
        const auto ability = state.activatedAbilityForOid(sourceOid, abilityIndex);
        if (!ability || (manaAbilitiesOnly && !ability->isManaAbility()))
            continue;
        const auto targets = state.abilityTargetData(sourceOid, abilityIndex);
        const bool hasXTargetChoices = std::any_of(targets.groups.cbegin(), targets.groups.cend(),
                                                   [](const auto &group) { return group.hasXTargetChoices; });
        const bool hasLegalXTargetChoice =
            !hasXTargetChoices ||
            std::all_of(targets.groups.cbegin(), targets.groups.cend(), [](const auto &group) {
                return !group.hasXTargetChoices || !ruledAvailableXTargetValues(group).isEmpty();
            });
        const QStringList manaOptions = ability->manaOptionsForSelection();
        const QString abilityLabel = ability->menuLabel();
        for (int optionIndex = 0; optionIndex < manaOptions.size(); ++optionIndex) {
            const QString label =
                manaOptions.size() > 1
                    ? (ability->manaProduced.isEmpty()
                           ? QObject::tr("%1 — Choose {%2}").arg(abilityLabel, manaOptions.at(optionIndex))
                           : QObject::tr("%1 — Add {%2}").arg(abilityLabel, manaOptions.at(optionIndex)))
                    : abilityLabel;
            options.append({RuledCardActionMenuOption::Kind::ActivateAbility, abilityIndex, label,
                            state.abilityActivatable(sourceOid, abilityIndex) && hasLegalXTargetChoice, optionIndex});
        }
    }
    return options;
}

std::optional<RuledFaceOption> RuledPendingCast::chooseFace(QWidget *parent,
                                                            const QString &cardName,
                                                            const QVector<RuledFaceOption> &faces)
{
    if (faces.isEmpty()) {
        return std::nullopt;
    }

    QMenu menu(parent);
    menu.setTitle(cardName);
    QVector<QAction *> actions;
    actions.reserve(faces.size());
    for (const auto &face : faces) {
        actions.append(menu.addAction(ruledCastOptionLabel(face)));
    }
    QAction *chosen = menu.exec(QCursor::pos());
    const int position = actions.indexOf(chosen);
    if (position < 0) {
        return std::nullopt;
    }
    return faces.at(position);
}

std::optional<QVector<int>> RuledPendingCast::chooseModes(QWidget *parent,
                                                          const QString &cardName,
                                                          const QVector<RuledModalSpellOption> &modes,
                                                          int minModes,
                                                          int maxModes,
                                                          ModeChoicePurpose purpose)
{
    QMenu menu(parent);
    const bool activatingAbility = purpose == ModeChoicePurpose::ActivatedAbility;
    menu.setTitle(activatingAbility ? QObject::tr("Activate %1").arg(cardName) : cardName);

    if (minModes == 1 && maxModes == 1) {
        QVector<QAction *> actions;
        actions.reserve(modes.size());
        for (const auto &mode : modes) {
            const QString label = activatingAbility ? QObject::tr("Activate %1 — %2").arg(cardName, mode.label)
                                                    : QObject::tr("Cast %1 — %2").arg(cardName, mode.label);
            auto *action = menu.addAction(label);
            action->setEnabled(mode.selectable);
            actions.append(action);
        }
        QAction *chosen = menu.exec(QCursor::pos());
        const int position = actions.indexOf(chosen);
        if (position < 0) {
            return std::nullopt;
        }
        return QVector<int>{modes.at(position).modeIndex};
    }

    auto *panel = new QWidget(&menu);
    auto *layout = new QVBoxLayout(panel);
    layout->setContentsMargins(8, 6, 8, 6);
    auto *status = new QLabel(panel);
    layout->addWidget(status);

    QVector<QCheckBox *> checks;
    checks.reserve(modes.size());
    for (const auto &mode : modes) {
        auto *check = new QCheckBox(mode.label, panel);
        check->setEnabled(mode.selectable);
        layout->addWidget(check);
        checks.append(check);
    }

    auto *buttons = new QHBoxLayout;
    auto *confirm = new QPushButton(activatingAbility ? QObject::tr("Activate with selected modes")
                                                      : QObject::tr("Cast with selected modes"),
                                    panel);
    auto *cancel = new QPushButton(QObject::tr("Cancel"), panel);
    buttons->addWidget(confirm);
    buttons->addWidget(cancel);
    layout->addLayout(buttons);

    auto updateState = [=]() {
        int selected = 0;
        for (const auto *check : checks) {
            selected += check->isChecked() ? 1 : 0;
        }
        const QString requirement = minModes == maxModes
            ? QObject::tr("Choose exactly %1 — %2/%1 selected.").arg(minModes).arg(selected)
            : QObject::tr("Choose %1–%2 — %3 selected.").arg(minModes).arg(maxModes).arg(selected);
        status->setText(requirement);
        confirm->setEnabled(selected >= minModes && selected <= maxModes);
        for (auto *check : checks) {
            if (!check->isChecked()) {
                check->setEnabled(check->property("modeSelectable").toBool() && selected < maxModes);
            }
        }
    };
    for (int i = 0; i < checks.size(); ++i) {
        checks[i]->setProperty("modeSelectable", modes[i].selectable);
        QObject::connect(checks[i], &QCheckBox::toggled, panel, updateState);
    }

    bool confirmed = false;
    QObject::connect(confirm, &QPushButton::clicked, &menu, [&]() {
        confirmed = true;
        menu.close();
    });
    QObject::connect(cancel, &QPushButton::clicked, &menu, &QMenu::close);

    auto *widgetAction = new QWidgetAction(&menu);
    widgetAction->setDefaultWidget(panel);
    menu.addAction(widgetAction);
    updateState();
    menu.exec(QCursor::pos());
    if (!confirmed) {
        return std::nullopt;
    }

    QVector<int> selectedModes;
    for (int i = 0; i < checks.size(); ++i) {
        if (checks[i]->isChecked()) {
            selectedModes.append(modes[i].modeIndex);
        }
    }
    return selectedModes;
}

std::optional<ruled::v1::RuledCommand>
RuledPendingCast::submissionCommand(const ruled::v1::RuledCommand &command,
                                    const PendingRuledSpellCast *pendingSpell,
                                    const RuledClientState *state)
{
    if (!command.has_begin_spell_cast() || !command.begin_spell_cast().has_announcement())
        return command;
    const auto method = command.begin_spell_cast().announcement().cast_method();
    const bool specialMethod = method == ruled::v1::CAST_METHOD_MADNESS ||
                               method == ruled::v1::CAST_METHOD_SIEGE_DEFEAT;
    const bool permissionMethod = method == ruled::v1::CAST_METHOD_PERMISSION;
    if (!specialMethod && !permissionMethod)
        return command;

    const bool permissionTargetsCurrentSpecialCast =
        permissionMethod && pendingSpell && state && pendingSpell->source == RuledCastSource::Exile &&
        state->hasPendingChoiceOfKind(RuledClientState::ChoiceKind::SpecialCast) &&
        state->isPendingChoiceCandidate(RuledClientState::ChoiceKind::SpecialCast,
                                        static_cast<quint32>(pendingSpell->handIndex));
    if (permissionMethod && state && state->hasActiveLocalResolutionPrompt() &&
        !permissionTargetsCurrentSpecialCast)
        return std::nullopt;
    if (permissionMethod && (!pendingSpell || pendingSpell->resolutionPromptId == 0) &&
        !permissionTargetsCurrentSpecialCast)
        return command; // Ordinary Permission casts still announce with BeginSpellCast.

    if (!pendingSpell || !state || !matchesSpecialCastOffer(*pendingSpell, *state))
        return std::nullopt;
    ruled::v1::RuledCommand result;
    auto *choice = result.mutable_submit_resolution_choice();
    choice->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_CAST_SPELL);
    choice->set_resolution_prompt_id(pendingSpell->resolutionPromptId);
    *choice->mutable_spell_cast_announcement() = command.begin_spell_cast().announcement();
    return result;
}

bool RuledPendingCast::matchesSpecialCastOffer(const PendingRuledSpellCast &spell, const RuledClientState &state)
{
    return spell.valid && spell.resolutionPromptId != 0 && state.isCurrentResolutionPrompt(spell.resolutionPromptId) &&
           state.pendingChoice.has_value() &&
           state.pendingChoice->resolutionPromptId == spell.resolutionPromptId &&
           spell.source == RuledCastSource::Exile &&
           (spell.castMethod == ruled::v1::CAST_METHOD_MADNESS ||
            spell.castMethod == ruled::v1::CAST_METHOD_SIEGE_DEFEAT ||
            spell.castMethod == ruled::v1::CAST_METHOD_PERMISSION) &&
           state.isPendingChoiceCandidate(RuledClientState::ChoiceKind::SpecialCast,
                                          static_cast<quint32>(spell.handIndex)) &&
           state.isZoneCastActionLegal(static_cast<quint32>(spell.handIndex), spell.faceIndex, spell.source,
                                       spell.castMethod, spell.castingPermissionId, spell.sourceZoneChangeGeneration,
                                       spell.prototyped);
}

bool RuledPendingCast::resolutionChoiceBlocksSpell(const RuledClientState &state) const
{
    if (spell.engineTransactionId != 0 && state.pendingSpellCast &&
        state.pendingSpellCast->transaction_id() == spell.engineTransactionId)
        return false;
    if (matchesSpecialCastOffer(spell, state))
        return false;
    return state.hasActiveLocalResolutionPrompt() || state.choiceWaitingPlayerId >= 0 || state.pendingChoice.has_value();
}
