#include "ruled_pending_cast.h"

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
        choice.kind != RuledCostChoiceKind::Unspecified || !choice.counterRemovalGroup ||
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
            if (requireOnlyGroups) {
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

} // namespace

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
    if (!pending.valid || pending.stage != PendingActivatedAbility::Stage::Announcing ||
        pending.engineTransactionId != 0 || pending.permanentAction || costIndex < 0 || objectId == 0) {
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
    if (!pending.valid || pending.stage != PendingActivatedAbility::Stage::Announcing ||
        pending.engineTransactionId != 0 || pending.permanentAction) {
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
    if (!pending.valid || pending.stage != PendingActivatedAbility::Stage::Announcing ||
        pending.engineTransactionId != 0 || pending.permanentAction ||
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
    if (!pending.valid || pending.stage != PendingActivatedAbility::Stage::Announcing ||
        pending.engineTransactionId != 0 || pending.permanentAction || !pending.counterGroupConfirmed ||
        pending.permanentOid == 0 || pending.abilityIndex < 0 || pending.selectedMode || !pending.selectedTargets.isEmpty() ||
        pending.needsTarget || pending.waitingForTarget || pending.chosenOpponentTargets ||
        pending.deferredReturnTappedCreature || pending.waitingForReturnTappedCreatureCandidate ||
        pending.xValue != 0 || pending.manaOptionIndex != 0 || pending.manaSplitFirstColorCount != 0 ||
        pending.castingPermissionId != 0 || pending.loyaltyXChoice || !pending.costSelections.isEmpty()) {
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
    if (!ability || !ability->activatable || ability->xCounterManaChoice || ability->loyaltyXChoice ||
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
    ability.valid = true;
    return ability;
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
