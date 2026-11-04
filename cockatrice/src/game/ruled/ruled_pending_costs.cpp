#include "ruled_pending_cast.h"

#include "ruled_activation.h"

#include <QCoreApplication>
#include <QRegularExpression>

#include <algorithm>
#include <limits>
#include <map>
#include <set>
#include <tuple>
#include <vector>

// Headless cost reconciliation and presentation. Separate from the menu implementation so
// menu-only consumers need not link the authoritative client-state query implementation.

namespace
{
using CounterContributionReceipt = std::tuple<quint32, quint32, quint64, quint32, quint32>;
using CounterContributionReceiptKey = std::tuple<quint32, quint32, quint64, quint32>;

template <typename CostSelections>
std::optional<std::vector<CounterContributionReceipt>> counterGroupReceipt(const CostSelections &selections)
{
    std::set<quint32> seenCostIndices;
    std::set<CounterContributionReceiptKey> seenRows;
    std::vector<CounterContributionReceipt> rows;
    for (const auto &selection : selections) {
        if (!selection.has_counter_removal_group() || !seenCostIndices.insert(selection.cost_index()).second) {
            return std::nullopt;
        }
        const auto &group = selection.counter_removal_group();
        if (group.contributions_size() == 0) {
            return std::nullopt;
        }
        for (const auto &contribution : group.contributions()) {
            if (!contribution.has_object() || contribution.object().object_id() == 0 || contribution.option_id() == 0 ||
                contribution.count() == 0) {
                return std::nullopt;
            }
            const CounterContributionReceiptKey key{selection.cost_index(), contribution.object().object_id(),
                                                     contribution.object().zone_change_generation(),
                                                     contribution.option_id()};
            if (!seenRows.insert(key).second) {
                return std::nullopt;
            }
            rows.emplace_back(selection.cost_index(), contribution.object().object_id(),
                              contribution.object().zone_change_generation(), contribution.option_id(),
                              contribution.count());
        }
    }
    if (rows.empty()) {
        return std::nullopt;
    }
    std::sort(rows.begin(), rows.end());
    return rows;
}

template <typename FlexPayments>
std::optional<std::map<quint32, std::pair<bool, int>>> flexPaymentReceipt(const FlexPayments &payments)
{
    std::map<quint32, std::pair<bool, int>> result;
    for (const auto &payment : payments) {
        if (!result.emplace(payment.pip_index(), std::make_pair(payment.pay_life(), static_cast<int>(payment.mana_color()))).second) {
            return std::nullopt;
        }
    }
    return result;
}

std::optional<std::vector<CounterContributionReceipt>> counterGroupLocalReceipt(
    const QVector<RuledPendingCounterContribution> &contributions)
{
    std::set<CounterContributionReceiptKey> seenRows;
    std::vector<CounterContributionReceipt> rows;
    for (const auto &row : contributions) {
        if (row.costIndex < 0 || row.objectId == 0 || row.optionId == 0 || row.count == 0) {
            return std::nullopt;
        }
        const auto costIndex = static_cast<quint32>(row.costIndex);
        const CounterContributionReceiptKey key{costIndex, row.objectId, row.zoneChangeGeneration, row.optionId};
        if (!seenRows.insert(key).second) {
            return std::nullopt;
        }
        rows.emplace_back(costIndex, row.objectId, row.zoneChangeGeneration, row.optionId, row.count);
    }
    if (rows.empty()) {
        return std::nullopt;
    }
    std::sort(rows.begin(), rows.end());
    return rows;
}

bool hasNoTargetOrOpponentShape(const ruled::v1::PendingAbilityActivation &pending)
{
    return !pending.has_chosen_opponent_id() && pending.announced_targets_size() == 0 &&
           pending.valid_opponent_ids_size() == 0 && !pending.has_target_group() && pending.target_candidates_size() == 0 &&
           pending.return_tapped_creature_candidates_size() == 0 && !pending.has_return_tapped_creature_cost_index();
}

bool matchesAcceptedCounterGroupPayment(const RuledClientState &state,
                                        const PendingActivatedAbility &local,
                                        int actorPlayerId,
                                        quint64 expectedTransactionId,
                                        quint64 expectedRevision)
{
    if (actorPlayerId < 0 || expectedTransactionId == 0 || !local.valid ||
        local.stage() != PendingActivatedAbility::Stage::Paying || !local.counterGroupConfirmed ||
        !local.hasInitializedPayment() || local.ownedBegin() || local.permanentAction || local.waitingForCost ||
        local.waitingForTarget || local.needsTarget || local.chosenOpponentTargets || local.selectedMode ||
        !local.selectedTargets.isEmpty() || local.deferredReturnTappedCreature ||
        local.waitingForReturnTappedCreatureCandidate || local.restartAfterTargetInvalidation ||
        !local.costChoices.isEmpty() || !local.costSelections.isEmpty() || local.permanentOid == 0 ||
        local.abilityIndex < 0 || local.transactionId() != expectedTransactionId ||
        local.revision() != expectedRevision || !state.pendingAbilityActivation) {
        return false;
    }

    const auto &engine = *state.pendingAbilityActivation;
    if (engine.transaction_id() != expectedTransactionId || engine.revision() != expectedRevision ||
        engine.stage() != ruled::v1::ABILITY_ACTIVATION_STAGE_PAYMENT || engine.actor_player_id() != actorPlayerId ||
        engine.deciding_player_id() != actorPlayerId || engine.source_object_id() != local.permanentOid ||
        engine.source_zone_change_generation() != local.expectedZoneChangeGeneration ||
        engine.source_zone() != local.sourceZone || engine.ability_index() != static_cast<quint32>(local.abilityIndex) ||
        !hasNoTargetOrOpponentShape(engine)) {
        return false;
    }

    const auto engineRows = counterGroupReceipt(engine.cost_selections());
    const auto localRows = counterGroupLocalReceipt(local.counterGroupContributions);
    if (!engineRows || !localRows || *engineRows != *localRows) {
        return false;
    }

    const auto engineFlex = flexPaymentReceipt(engine.flex_payments());
    const auto localFlex = flexPaymentReceipt(local.fixedFlexPayments);
    return engineFlex && localFlex && *engineFlex == *localFlex;
}

bool matchesOwnedCounterGroupBeginSnapshot(const RuledOwnedCounterGroupBegin &owned,
                                          const ruled::v1::PendingAbilityActivation &pending)
{
    const auto &announcement = owned.announcement;
    if (owned.draftId == 0 || owned.actorPlayerId < 0 || announcement.source_object_id() == 0 ||
        announcement.has_own_target() || announcement.has_opponent_player_id() || pending.transaction_id() == 0 ||
        pending.stage() != ruled::v1::ABILITY_ACTIVATION_STAGE_PAYMENT ||
        pending.actor_player_id() != owned.actorPlayerId || pending.deciding_player_id() != owned.actorPlayerId ||
        pending.source_object_id() != announcement.source_object_id() ||
        pending.source_zone_change_generation() != announcement.expected_zone_change_generation() ||
        pending.source_zone() != announcement.source_zone() || pending.ability_index() != announcement.ability_index() ||
        !hasNoTargetOrOpponentShape(pending)) {
        return false;
    }

    const auto expectedRows = counterGroupReceipt(announcement.cost_selections());
    const auto actualRows = counterGroupReceipt(pending.cost_selections());
    if (!expectedRows || !actualRows || *expectedRows != *actualRows) {
        return false;
    }

    const auto expectedFlex = flexPaymentReceipt(announcement.flex_payments());
    const auto actualFlex = flexPaymentReceipt(pending.flex_payments());
    return expectedFlex && actualFlex && *expectedFlex == *actualFlex;
}

bool currentOwnedCounterGroupBeginMatches(const PendingActivatedAbility &pending)
{
    const auto *ownedBegin = pending.ownedBegin();
    if (!ownedBegin || !pending.valid || pending.draftId == 0 || ownedBegin->draftId != pending.draftId ||
        pending.stage() != PendingActivatedAbility::Stage::BeginPending || pending.transactionId() != 0 ||
        pending.hasInitializedPayment() || pending.permanentAction || pending.waitingForCost ||
        pending.waitingForTarget || pending.waitingForMana) {
        return false;
    }

    const auto &owned = *ownedBegin;
    const auto &announcement = owned.announcement;
    return owned.actorPlayerId >= 0 && announcement.source_object_id() != 0 && !announcement.has_own_target() &&
           !announcement.has_opponent_player_id() && pending.permanentOid == announcement.source_object_id() &&
           pending.expectedZoneChangeGeneration == announcement.expected_zone_change_generation() &&
           pending.sourceZone == announcement.source_zone() && pending.abilityIndex >= 0 &&
           static_cast<quint32>(pending.abilityIndex) == announcement.ability_index() &&
           !pending.selectedMode && pending.selectedTargets.isEmpty() && !pending.needsTarget &&
           !pending.waitingForTarget && !pending.chosenOpponentTargets;
}

bool hasCounterGroupChoice(const QVector<RuledCostChoice> &choices)
{
    return std::any_of(choices.cbegin(), choices.cend(), [](const auto &choice) {
        return choice.kind == RuledCostChoiceKind::RemoveCountersFromPermanents ||
               choice.counterRemovalGroup.has_value();
    });
}

bool validCounterGroupOfferSet(const QVector<RuledCostChoice> &choices)
{
    if (choices.isEmpty()) {
        return false;
    }
    std::set<int> costIndices;
    for (const auto &choice : choices) {
        if (choice.costIndex < 0 || !costIndices.insert(choice.costIndex).second || !choice.counterRemovalGroup ||
            choice.kind != RuledCostChoiceKind::RemoveCountersFromPermanents ||
            choice.counterRemovalGroup->candidates.isEmpty() ||
            choice.counterRemovalGroup->candidates.front().options.isEmpty()) {
            return false;
        }
        PendingActivatedAbility probe;
        probe.valid = true;
        probe.costChoices = choices;
        const auto &candidate = choice.counterRemovalGroup->candidates.front();
        const auto &option = candidate.options.front();
        if (!ruledStageCounterContributions(
                probe,
                choice.costIndex,
                candidate.objectId,
                candidate.zoneChangeGeneration,
                {{choice.costIndex, candidate.objectId, candidate.zoneChangeGeneration, option.optionId, 1}})) {
            return false;
        }
    }
    return true;
}

void refreshCounterContributionLabels(PendingActivatedAbility &pending,
                                      const QVector<RuledCostChoice> &currentChoices)
{
    for (auto &row : pending.counterGroupContributions) {
        const auto choice = std::find_if(currentChoices.cbegin(), currentChoices.cend(), [&row](const auto &entry) {
            return entry.costIndex == row.costIndex &&
                   entry.kind == RuledCostChoiceKind::RemoveCountersFromPermanents &&
                   entry.counterRemovalGroup.has_value();
        });
        if (choice == currentChoices.cend()) {
            row.counterLabel.clear();
            continue;
        }
        const auto candidate = std::find_if(choice->counterRemovalGroup->candidates.cbegin(),
                                            choice->counterRemovalGroup->candidates.cend(),
                                            [&row](const auto &entry) {
                                                return entry.objectId == row.objectId &&
                                                       entry.zoneChangeGeneration == row.zoneChangeGeneration;
                                            });
        if (candidate == choice->counterRemovalGroup->candidates.cend()) {
            row.counterLabel.clear();
            continue;
        }
        const auto option = std::find_if(candidate->options.cbegin(), candidate->options.cend(), [&row](const auto &entry) {
            return entry.optionId == row.optionId;
        });
        row.counterLabel = option == candidate->options.cend() ? QString{} : option->label;
    }
}
} // namespace

bool ruledRetryCounterGroupPayment(RuledClientState &state,
                                   PendingActivatedAbility &local,
                                   int actorPlayerId,
                                   quint64 expectedTransactionId,
                                   quint64 expectedRevision)
{
    if (actorPlayerId < 0 || expectedTransactionId == 0 || state.isEngineCommandPending() ||
        ruledActivationNestedChoice(state) || !state.payment.active || state.payment.pending ||
        state.payment.submitting || !local.paymentFailed() ||
        !matchesAcceptedCounterGroupPayment(state, local, actorPlayerId, expectedTransactionId, expectedRevision)) {
        return false;
    }

    if (!local.retryPayment())
        return false;
    state.payment.invalidate();
    return true;
}

std::optional<ruled::v1::RuledCommand>
ruledCancelCounterGroupPayment(RuledClientState &state, PendingActivatedAbility &local, int actorPlayerId)
{
    const quint64 transactionId = local.transactionId();
    const quint64 revision = local.revision();
    if (actorPlayerId < 0 || state.isEngineCommandPending() || ruledActivationNestedChoice(state) ||
        !matchesAcceptedCounterGroupPayment(state, local, actorPlayerId, transactionId, revision) ||
        !local.beginCancel()) {
        return std::nullopt;
    }

    ruled::v1::RuledCommand command;
    auto *cancel = command.mutable_cancel_ability_activation();
    cancel->set_transaction_id(transactionId);
    cancel->set_expected_revision(revision);
    return command;
}

bool ruledRecordCounterGroupPreviewFailure(RuledClientState &state,
                                           PendingActivatedAbility &local,
                                           int actorPlayerId,
                                           const ruled::v1::PaymentPreview *rejectedReply)
{
    if (!matchesAcceptedCounterGroupPayment(state, local, actorPlayerId, local.transactionId(), local.revision())) {
        return false;
    }

    if (rejectedReply) {
        if (rejectedReply->valid() || !state.payment.apply(*rejectedReply)) {
            return false;
        }
    } else if (!state.payment.active || state.payment.pending || state.payment.submitting ||
               state.payment.view.transaction_id() != state.payment.transaction() || state.payment.view.valid()) {
        return false;
    }

    return local.markPaymentFailed();
}

bool ruledInvalidateCounterGroupPaymentForRefresh(RuledClientState &state,
                                                  const PendingActivatedAbility &local,
                                                  int actorPlayerId)
{
    if (!matchesAcceptedCounterGroupPayment(state, local, actorPlayerId, local.transactionId(), local.revision()) ||
        !state.payment.active || state.payment.submitting || local.paymentFailed()) {
        return false;
    }

    state.payment.invalidate();
    return true;
}

std::optional<RuledOwnedCounterGroupBegin> RuledPendingCast::prepareCounterGroupBegin(
    const RuledClientState &state,
    int actorPlayerId,
    const QVector<ruled::v1::FlexPipPayment> &flexPayments)
{
    auto &pending = ability;
    if (actorPlayerId < 0 || state.pendingAbilityActivation || pending.draftId == 0 || !pending.valid ||
        pending.stage() != PendingActivatedAbility::Stage::Announcing || pending.transactionId() != 0 ||
        pending.hasInitializedPayment() || pending.permanentAction || pending.ownedBegin()) {
        return std::nullopt;
    }

    const auto announcement = ruledBuildCounterGroupBegin(state, pending, flexPayments);
    if (!announcement) {
        return std::nullopt;
    }

    RuledOwnedCounterGroupBegin owned;
    owned.draftId = pending.draftId;
    owned.actorPlayerId = actorPlayerId;
    owned.announcement = *announcement;
    if (!pending.beginOwnedAnnouncement(owned))
        return std::nullopt;
    pending.fixedFlexPayments.clear();
    for (const auto &payment : announcement->flex_payments()) {
        pending.fixedFlexPayments.append(payment);
    }
    pending.waitingForCost = false;
    pending.waitingForTarget = false;
    pending.waitingForMana = false;
    return owned;
}

RuledOwnedCounterGroupBeginUpdate
RuledPendingCast::reconcileCounterGroupBegin(const ruled::v1::PendingAbilityActivation *currentPending)
{
    if (!ability.ownedBegin()) {
        return RuledOwnedCounterGroupBeginUpdate::Ignored;
    }
    if (!currentOwnedCounterGroupBeginMatches(ability)) {
        return RuledOwnedCounterGroupBeginUpdate::Ignored;
    }
    const auto *owned = ability.ownedBegin();
    const auto *matchingSnapshot =
        currentPending && matchesOwnedCounterGroupBeginSnapshot(*owned, *currentPending) ? currentPending : nullptr;
    const auto update = ability.observeOwnedSnapshot(matchingSnapshot);
    if (update != RuledOwnedCounterGroupBeginUpdate::Accepted)
        return update;

    ruledApplyActivationView(ability, *matchingSnapshot);
    return update;
}

RuledOwnedCounterGroupBeginUpdate RuledPendingCast::acknowledgeCounterGroupBegin(
    const RuledOwnedCounterGroupBegin &captured,
    bool accepted,
    const ruled::v1::PendingAbilityActivation *currentPending)
{
    const auto *owned = ability.ownedBegin();
    if (!owned || !currentOwnedCounterGroupBeginMatches(ability)) {
        return RuledOwnedCounterGroupBeginUpdate::Ignored;
    }
    QVector<ruled::v1::FlexPipPayment> retainedFlexPayments;
    for (const auto &payment : owned->announcement.flex_payments())
        retainedFlexPayments.append(payment);
    const auto update = ability.acknowledgeOwned(captured, accepted);
    if (update == RuledOwnedCounterGroupBeginUpdate::Rejected) {
        ability.fixedFlexPayments = retainedFlexPayments;
        if (++nextAbilityDraftId == 0)
            ++nextAbilityDraftId;
        ability.draftId = nextAbilityDraftId;
        ability.waitingForMana = false;
        return RuledOwnedCounterGroupBeginUpdate::Rejected;
    }
    if (update != RuledOwnedCounterGroupBeginUpdate::Waiting)
        return update;
    return reconcileCounterGroupBegin(currentPending);
}

QMap<QChar, int> RuledPendingCast::parseSimpleManaCost(const QString &manaCost)
{
    QMap<QChar, int> parsed;
    auto addSymbol = [&parsed](QChar c) {
        const QChar sym = c.toUpper();
        if (QStringLiteral("WUBRGCX").contains(sym)) {
            parsed[sym] += 1;
        }
    };

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
            // Numeric tokens are generic mana of any length ({1}, {4}, {10}); only fall back
            // to single-symbol parsing for non-numeric tokens ({G}, {C}, {X}). The previous
            // size()==1 check routed single digits to addSymbol, which silently dropped them.
            bool ok = false;
            const int generic = token.toInt(&ok);
            if (ok) {
                if (generic > 0) {
                    parsed['X'] += generic;
                }
            } else if (token.size() == 1) {
                addSymbol(token.at(0));
            }
            token.clear();
            continue;
        }
        if (inBraces) {
            token.append(c);
            continue;
        }
        if (c.isDigit()) {
            parsed['X'] += c.digitValue();
            continue;
        }
        addSymbol(c);
    }
    return parsed;
}

std::optional<QMap<QChar, int>> RuledPendingCast::parseRepresentableManaCost(const QString &manaCost)
{
    qint64 total = 0;
    const QRegularExpression tokens(QStringLiteral("\\{([^{}]+)\\}"));
    auto matches = tokens.globalMatch(manaCost);
    while (matches.hasNext()) {
        const QString token = matches.next().captured(1);
        if (std::all_of(token.cbegin(), token.cend(), [](QChar c) { return c.isDigit(); })) {
            bool valid = false;
            const quint64 value = token.toULongLong(&valid);
            if (!valid || value > static_cast<quint64>(std::numeric_limits<int>::max()))
                return std::nullopt;
            total += static_cast<qint64>(value);
        } else {
            for (QChar c : token)
                total += QStringLiteral("WUBRGCX").contains(c.toUpper());
        }
        if (total > std::numeric_limits<int>::max())
            return std::nullopt;
    }
    const auto parsed = parseSimpleManaCost(manaCost);
    total = 0;
    for (int value : parsed) {
        if (value < 0)
            return std::nullopt;
        total += value;
    }
    return total <= std::numeric_limits<int>::max() ? std::optional(parsed) : std::nullopt;
}

std::optional<int> RuledPendingCast::repeatedCastCostAmount(const RuledCastCostOption &option, quint32 count)
{
    if (option.kind != RuledCastCostOptionKind::Mana || !option.maximumRepetitions || count == 0 ||
        count > *option.maximumRepetitions)
        return std::nullopt;
    const auto unit = parseRepresentableManaCost(option.additionalManaCost);
    if (!unit || unit->size() != 1 || unit->value('X') <= 0 ||
        !QRegularExpression(QStringLiteral("^(\\{[0-9]+\\})+$")).match(option.additionalManaCost).hasMatch())
        return std::nullopt;
    const quint64 fee = static_cast<quint64>(unit->value('X')) * count;
    return fee <= static_cast<quint64>(std::numeric_limits<int>::max()) ? std::optional(static_cast<int>(fee))
                                                                        : std::nullopt;
}

bool RuledPendingCast::stageRepeatedCastCost(int optionIndex, quint32 count)
{
    if (!isAwaitingRuledCastCostOption() || spell.engineTransactionId != 0)
        return false;
    const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
    const auto option = std::find_if(group.options.cbegin(), group.options.cend(),
                                     [optionIndex](const auto &entry) { return entry.optionIndex == optionIndex; });
    if (group.min != 0 || group.max != 1 || group.options.size() != 1 || option == group.options.cend() ||
        !option->selectable || ruledCastCostGroupSelectionCount(spell, group.groupIndex) != 0)
        return false;
    const auto fee = repeatedCastCostAmount(*option, count);
    if (!fee)
        return false;
    qint64 total = *fee + static_cast<qint64>(spell.flexPips.size());
    for (int value : spell.remainingCost) {
        if (value < 0)
            return false;
        total += value;
    }
    if (total > std::numeric_limits<int>::max())
        return false;
    spell.remainingCost['X'] += *fee;
    RuledPendingCastCostSelection selection;
    selection.groupIndex = group.groupIndex;
    selection.optionIndex = optionIndex;
    selection.repetitions = count;
    spell.castCostSelections.append(selection);
    return true;
}

bool RuledPendingCast::repeatedCastCostPromptStillCurrent(const PendingRuledSpellCast &before, int optionIndex) const
{
    if (!before.valid || !isAwaitingRuledCastCostOption() || before.draftId != spell.draftId ||
        spell.stage != PendingRuledSpellCast::Stage::Announcing || spell.engineTransactionId != 0 ||
        spell.submissionPending || before.handIndex != spell.handIndex || before.source != spell.source ||
        before.sourceZoneChangeGeneration != spell.sourceZoneChangeGeneration || before.faceIndex != spell.faceIndex ||
        before.castMethod != spell.castMethod || before.castingPermissionId != spell.castingPermissionId ||
        before.nextCastCostGroup != spell.nextCastCostGroup || before.nextCastCostGroup < 0 ||
        before.nextCastCostGroup >= before.castCostGroups.size())
        return false;
    const auto &original = before.castCostGroups.at(before.nextCastCostGroup);
    const auto &current = spell.castCostGroups.at(spell.nextCastCostGroup);
    if (original.groupIndex != current.groupIndex || current.min != 0 || current.max != 1 ||
        current.options.size() != 1 || original.options.size() != 1 ||
        ruledCastCostGroupSelectionCount(spell, current.groupIndex) != 0)
        return false;
    const auto &oldOption = original.options.first();
    const auto &liveOption = current.options.first();
    return oldOption.optionIndex == optionIndex && liveOption.optionIndex == optionIndex && liveOption.selectable &&
           liveOption.kind == RuledCastCostOptionKind::Mana && oldOption.kind == liveOption.kind &&
           oldOption.maximumRepetitions == liveOption.maximumRepetitions && liveOption.maximumRepetitions &&
           oldOption.additionalManaCost == liveOption.additionalManaCost;
}

bool RuledPendingCast::expandRepeatedCastX(int chosenX)
{
    if (chosenX < 0 || spell.xPips < 0)
        return false;
    const qint64 generic = static_cast<qint64>(spell.remainingCost.value('X')) +
                           static_cast<qint64>(spell.xPips) * (static_cast<qint64>(chosenX) - 1);
    qint64 total = generic + static_cast<qint64>(spell.flexPips.size());
    if (generic < 0 || generic > std::numeric_limits<int>::max())
        return false;
    for (auto it = spell.remainingCost.cbegin(); it != spell.remainingCost.cend(); ++it) {
        if (it.value() < 0)
            return false;
        if (it.key() != QChar('X'))
            total += it.value();
    }
    if (total > std::numeric_limits<int>::max())
        return false;
    spell.xValue = chosenX;
    if (generic == 0)
        spell.remainingCost.remove('X');
    else
        spell.remainingCost['X'] = static_cast<int>(generic);
    spell.xPips = 0;
    return true;
}

bool RuledPendingCast::finalizeRepeatedCastManaCost(qint64 increase, qint64 reduction)
{
    qint64 total = increase + static_cast<qint64>(spell.flexPips.size());
    if (increase < 0 || reduction < 0)
        return false;
    for (int value : spell.remainingCost) {
        if (value < 0)
            return false;
        total += value;
    }
    if (total > std::numeric_limits<int>::max())
        return false;
    const qint64 generic = qMax<qint64>(0, spell.remainingCost.value('X') + increase - reduction);
    if (generic > std::numeric_limits<int>::max())
        return false;
    spell.remainingCost['X'] = static_cast<int>(generic);
    spell.manaCostFinalized = true;
    return true;
}

QString RuledPendingCast::formatSimpleManaCost(const QMap<QChar, int> &cost)
{
    // Render in canonical Scryfall brace form ({4}{G}{G}). The brackets double as a
    // placeholder for real mana symbols later, so they are kept rather than stripped.
    QString out;
    const int generic = cost.value('X', 0);
    if (generic > 0) {
        out += QStringLiteral("{%1}").arg(generic);
    }
    for (QChar c : QStringLiteral("WUBRGC")) {
        const int count = cost.value(c, 0);
        for (int i = 0; i < count; ++i) {
            out += QStringLiteral("{%1}").arg(c);
        }
    }
    return out;
}

bool RuledPendingCast::flexPipMatchesColor(const RuledFlexPip &pip, QChar color)
{
    const QChar c = color.toUpper();
    if (pip.colorA == c) {
        return true;
    }
    // Two-color hybrid and hybrid Phyrexian accept both component colors. A mono-hybrid
    // or mono-Phyrexian pip has no second color; its other branch is generic mana or life.
    return pip.generic == 0 && !pip.colorB.isNull() && pip.colorB == c;
}

bool RuledPendingCast::applyFlexChoicesToCost(QMap<QChar, int> &fixed,
                                              QVector<ruled::v1::FlexPipPayment> &payments,
                                              QVector<RuledFlexPip> &flex,
                                              const QVector<int> &choices)
{
    if (choices.size() != flex.size())
        return false;
    auto nextFixed = fixed;
    auto nextPayments = payments;
    QSet<quint32> seen;
    for (const auto &payment : payments) {
        if (seen.contains(payment.pip_index()))
            return false;
        seen.insert(payment.pip_index());
    }
    const auto add = [&nextFixed](QChar color, int count) {
        const int current = nextFixed.value(color, 0);
        if (current < 0 || count < 0 || current > std::numeric_limits<int>::max() - count)
            return false;
        nextFixed[color] = current + count;
        return true;
    };
    for (int i = 0; i < flex.size(); ++i) {
        const auto &pip = flex.at(i);
        if (seen.contains(pip.pipIndex))
            return false;
        seen.insert(pip.pipIndex);
        const int choice = choices.at(i);
        if (pip.phyrexian) {
            const auto payment = flexPipPaymentForChoice(pip, choice);
            if (!payment)
                return false;
            nextPayments.append(*payment);
            if (!payment->pay_life() && !add(choice == 0 ? pip.colorA.toUpper() : pip.colorB.toUpper(), 1))
                return false;
        } else {
            if (choice < 0 || choice > 1)
                return false;
            if (pip.generic > 0) {
                if (!add(choice == 0 ? pip.colorA.toUpper() : QChar('X'), choice == 0 ? 1 : pip.generic))
                    return false;
            } else if (!add((choice == 0 ? pip.colorA : pip.colorB).toUpper(), 1))
                return false;
        }
    }
    fixed = std::move(nextFixed);
    payments = std::move(nextPayments);
    flex.clear();
    return true;
}

bool RuledPendingCast::applyManaPipToFlexibleCost(QMap<QChar, int> &fixed,
                                                  QVector<RuledFlexPip> &flex,
                                                  bool colorlessMana,
                                                  QChar coloredMana)
{
    if (!colorlessMana) {
        const QChar sym = coloredMana.toUpper();
        // 1. A fixed colored demand of this exact color (CR 202.1).
        if (fixed.value(sym, 0) > 0) {
            fixed[sym] -= 1;
            return true;
        }
        // 2. CR 107.4d–f: pay an as-yet-untouched flexible pip's colored alternative. Preferring
        //    untouched pips means a correct-color tap claims a fresh pip rather than topping up a
        //    half-paid mono-hybrid generic — e.g. {2/R}{2/R} with one generic already down, a red
        //    completes the *other* pip and leaves the partial one alone.
        for (int i = 0; i < flex.size(); ++i) {
            if (flex[i].genericPaid == 0 && flexPipMatchesColor(flex[i], sym)) {
                flex.remove(i);
                return true;
            }
        }
    }
    // 3. Fixed generic {N}/{X}: payable by any mana.
    if (fixed.value('X', 0) > 0) {
        fixed['X'] -= 1;
        return true;
    }
    // 4. Fixed colorless {C}: only colorless mana qualifies (CR 107.4c).
    if (colorlessMana && fixed.value('C', 0) > 0) {
        fixed['C'] -= 1;
        return true;
    }
    // 5. CR 107.4e: a mono-hybrid generic alternative ({2/W}), payable by any mana. Top up a
    //    partially-paid pip first (so the mana already spent on it isn't stranded), otherwise
    //    open a fresh one.
    int partialIdx = -1;
    int freshIdx = -1;
    for (int i = 0; i < flex.size(); ++i) {
        if (flex[i].generic <= 0) {
            continue; // hybrid / Phyrexian have no generic alternative
        }
        if (flex[i].genericPaid > 0) {
            if (partialIdx < 0 || flex[i].genericPaid > flex[partialIdx].genericPaid) {
                partialIdx = i;
            }
        } else if (freshIdx < 0) {
            freshIdx = i;
        }
    }
    const int idx = (partialIdx >= 0) ? partialIdx : freshIdx;
    if (idx >= 0) {
        flex[idx].genericPaid += 1;
        if (flex[idx].genericPaid >= flex[idx].generic) {
            flex.remove(idx);
        }
        return true;
    }
    return false;
}

QString RuledPendingCast::formatRemainingCost(const QMap<QChar, int> &fixed, const QVector<RuledFlexPip> &flex)
{
    QString out = formatSimpleManaCost(fixed);
    for (const RuledFlexPip &pip : flex) {
        if (pip.phyrexian) {
            out += pip.colorB.isNull() ? QStringLiteral("{%1/P}").arg(pip.colorA)
                                      : QStringLiteral("{%1/%2/P}").arg(pip.colorA).arg(pip.colorB);
        } else if (pip.generic > 0) {
            out += QStringLiteral("{%1/%2}").arg(pip.generic - pip.genericPaid).arg(pip.colorA);
        } else {
            out += QStringLiteral("{%1/%2}").arg(pip.colorA).arg(pip.colorB);
        }
    }
    return out;
}

int RuledPendingCast::totalRemainingForCost(const QMap<QChar, int> &fixed, const QVector<RuledFlexPip> &flex)
{
    int total = 0;
    for (auto it = fixed.constBegin(); it != fixed.constEnd(); ++it) {
        total += it.value();
    }
    // Every unresolved flexible pip still owes at least one more mana (or, for Phyrexian, 2 life).
    total += flex.size();
    return total;
}

namespace
{
bool selectionStillLegal(const RuledClientState &state,
                         int localPlayerId,
                         const RuledPendingCostSelection &selection,
                         const QVector<RuledCostChoice> &choices)
{
    const auto choice = std::find_if(choices.cbegin(), choices.cend(), [&selection](const RuledCostChoice &entry) {
        return entry.costIndex == selection.costIndex && entry.zone == selection.zone;
    });
    if (choice == choices.cend()) {
        return false;
    }
    if (choice->kind == RuledCostChoiceKind::RemoveCounters) {
        return ruledCounterSelectionStillLegal(selection, *choice);
    }
    if (selection.selectedIds.isEmpty() || selection.selectedIds.size() > choice->max) {
        return false;
    }
    return std::all_of(selection.selectedIds.cbegin(), selection.selectedIds.cend(), [&](quint32 selectedId) {
        if (selection.zone == RuledCostChoiceZone::Hand) {
            const int slot = state.engineHandSlotForServerCard(localPlayerId, static_cast<int>(selectedId));
            return slot >= 0 && choice->candidateIds.contains(static_cast<quint32>(slot));
        }
        if (!choice->candidateIds.contains(selectedId)) {
            return false;
        }
        if (ruledCostUsesObjectRefs(*choice)) {
            const int selectedIndex = selection.selectedIds.indexOf(selectedId);
            return selectedIndex >= 0 &&
                   selection.selectedGenerations.value(selectedIndex) == choice->candidateGenerations.value(selectedId);
        }
        return true;
    });
}
} // namespace

bool RuledPendingCast::reconcileSpellCosts(const RuledClientState &state, int localPlayerId)
{
    // Once the engine has accepted this announcement, the source is expected to have left the
    // hand-action list. Keep the local payment staging bound to that matching engine transaction;
    // legal-action refreshes during mana payment must not cancel it or replace its locked costs.
    if (spell.valid && spell.engineTransactionId != 0 && state.pendingSpellCast &&
        state.pendingSpellCast->transaction_id() == spell.engineTransactionId) {
        return true;
    }

    if (spell.valid) {
        const bool sourceStillLegal =
            spell.source == RuledCastSource::Hand
                ? state.isHandCastActionLegal(spell.handIndex, spell.faceIndex, spell.castMethod, spell.prototyped,
                                               spell.asForetoldSource)
                : state.isZoneCastActionLegal(static_cast<quint32>(spell.handIndex), spell.faceIndex, spell.source,
                                              spell.castMethod, spell.castingPermissionId,
                                              spell.sourceZoneChangeGeneration, spell.prototyped, spell.asForetoldSource);
        const auto latest = state.spellCostData(spell.handIndex, spell.faceIndex, spell.source, spell.castMethod,
                                                spell.castingPermissionId, spell.prototyped);
        const bool castCostSelectionsStillLegal =
            std::all_of(spell.castCostSelections.cbegin(), spell.castCostSelections.cend(), [&](const auto &selection) {
                const auto group =
                    std::find_if(latest.castCostGroups.cbegin(), latest.castCostGroups.cend(),
                                 [&selection](const auto &entry) { return entry.groupIndex == selection.groupIndex; });
                if (group == latest.castCostGroups.cend()) {
                    return false;
                }
                const auto option =
                    std::find_if(group->options.cbegin(), group->options.cend(), [&selection](const auto &entry) {
                        return entry.optionIndex == selection.optionIndex;
                    });
                if (option == group->options.cend() || !option->selectable) {
                    return false;
                }
                if (option->maximumRepetitions) {
                    if (!selection.repetitions || !repeatedCastCostAmount(*option, *selection.repetitions))
                        return false;
                } else if (selection.repetitions && *selection.repetitions != 1) {
                    return false;
                }
                if (selection.objectKind == RuledPendingCastCostSelection::ObjectKind::Hand) {
                    const int slot = state.engineHandSlotForServerCard(localPlayerId, selection.selectedId);
                    return slot >= 0 && option->validHandIndices.contains(static_cast<quint32>(slot));
                }
                if (selection.objectKind == RuledPendingCastCostSelection::ObjectKind::Permanent) {
                    if (ruledCastCostUsesPermanentCohort(option->kind)) {
                        qint64 total = 0;
                        for (const quint32 oid : selection.selectedObjectIds) {
                            if (!option->validPermanentIds.contains(oid) ||
                                option->validPermanentGenerations.value(oid) !=
                                    selection.selectedObjectGenerations.value(oid) ||
                                option->candidateContributions.value(oid) !=
                                    selection.selectedObjectContributions.value(oid))
                                return false;
                            total += selection.selectedObjectContributions.value(oid);
                        }
                        const bool activeIncompleteSelection =
                            spell.waitingForCastCostObject && spell.nextCastCostGroup < latest.castCostGroups.size() &&
                            latest.castCostGroups.at(spell.nextCastCostGroup).groupIndex == selection.groupIndex &&
                            spell.activeCastCostOption == selection.optionIndex;
                        if (activeIncompleteSelection)
                            return selection.selectedObjectIds.size() <= option->objectMax;
                        return selection.selectedObjectIds.size() >= option->objectMin &&
                               selection.selectedObjectIds.size() <= option->objectMax &&
                               (option->aggregateMinimum <= 0 || total >= option->aggregateMinimum);
                    }
                    return option->validPermanentIds.contains(selection.selectedId) &&
                           option->validPermanentGenerations.value(selection.selectedId) ==
                               selection.expectedZoneChangeGeneration &&
                           option->validPermanentGenericReductions.value(selection.selectedId) ==
                               selection.genericCostReduction;
                }
                return option->kind == RuledCastCostOptionKind::Mana ||
                       option->kind == RuledCastCostOptionKind::PayLife;
            });
        const bool pendingCastCostObjectStillLegal =
            !spell.waitingForCastCostObject || (spell.nextCastCostGroup < latest.castCostGroups.size() && [&]() {
                const auto &group = latest.castCostGroups.at(spell.nextCastCostGroup);
                const auto option =
                    std::find_if(group.options.cbegin(), group.options.cend(),
                                 [this](const auto &entry) { return entry.optionIndex == spell.activeCastCostOption; });
                return option != group.options.cend() && option->selectable &&
                       ruledCastCostUsesObjectChoice(option->kind);
            }());
        if (!sourceStillLegal || !castCostSelectionsStillLegal || !pendingCastCostObjectStillLegal ||
            std::any_of(spell.costSelections.cbegin(), spell.costSelections.cend(),
                        [&state, localPlayerId, &latest](const auto &selection) {
                            return !selectionStillLegal(state, localPlayerId, selection, latest.choices);
                        })) {
            return false;
        } else {
            spell.costChoices = latest.choices;
            spell.castCostGroups = latest.castCostGroups;
        }
    }
    return true;
}

bool RuledPendingCast::reconcileAbilityCosts(const RuledClientState &state, int localPlayerId)
{
    if (ability.valid) {
        if (ability.ownedBegin()) {
            return true;
        }
        if (ability.stage() == PendingActivatedAbility::Stage::BeginPending || ability.transactionId() != 0)
            return ruledPendingAbilitySourceStillCurrent(state, ability);
        const bool sourceStillCurrent = ruledPendingAbilitySourceStillCurrent(state, ability);
        const auto latest = ability.permanentAction
                                ? QVector<RuledCostChoice>{}
                                : state.abilityCostChoices(ability.permanentOid, ability.abilityIndex);
        if (hasCounterGroupChoice(ability.costChoices) || hasCounterGroupChoice(latest)) {
            if (!sourceStillCurrent || ability.permanentAction || !ability.costSelections.isEmpty() ||
                !validCounterGroupOfferSet(latest)) {
                return false;
            }
            const std::optional<int> activeCostIndex =
                ability.nextCostChoice >= 0 && ability.nextCostChoice < ability.costChoices.size()
                    ? std::optional<int>(ability.costChoices.at(ability.nextCostChoice).costIndex)
                    : std::nullopt;
            const bool unchanged = ruledReconcileCounterContributions(ability, latest);
            if (!unchanged) {
                ability.counterGroupContributions.clear();
                ability.counterGroupConfirmed = false;
                ability.waitingForCost = true;
            } else {
                refreshCounterContributionLabels(ability, latest);
            }
            auto selectedGroup = latest.cend();
            if (activeCostIndex) {
                selectedGroup = std::find_if(latest.cbegin(), latest.cend(), [&](const auto &choice) {
                    return choice.costIndex == *activeCostIndex &&
                           choice.kind == RuledCostChoiceKind::RemoveCountersFromPermanents &&
                           choice.counterRemovalGroup.has_value();
                });
            }
            if (selectedGroup == latest.cend()) {
                selectedGroup = std::find_if(latest.cbegin(), latest.cend(), [](const auto &choice) {
                    return choice.kind == RuledCostChoiceKind::RemoveCountersFromPermanents &&
                           choice.counterRemovalGroup.has_value();
                });
            }
            if (selectedGroup == latest.cend()) {
                return false;
            }
            ability.nextCostChoice = static_cast<int>(std::distance(latest.cbegin(), selectedGroup));
            return true;
        }
        if (!sourceStillCurrent || std::any_of(ability.costSelections.cbegin(), ability.costSelections.cend(),
                                               [&state, localPlayerId, &latest](const auto &selection) {
                                                   return !selectionStillLegal(state, localPlayerId, selection, latest);
                                               })) {
            return false;
        } else {
            ability.costChoices = latest;
        }
    }
    return true;
}

bool RuledPendingCast::isAwaitingRuledCastCostOption() const
{
    return spell.valid && !spell.waitingForCastCostObject && spell.nextCastCostGroup < spell.castCostGroups.size();
}

bool RuledPendingCast::pendingRuledCastCostGroupIsOptional() const
{
    if (!spell.valid || spell.nextCastCostGroup >= spell.castCostGroups.size())
        return false;
    const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
    return group.min == 0 &&
           std::none_of(spell.selectedModeLinkedCastCosts.cbegin(), spell.selectedModeLinkedCastCosts.cend(),
                        [&group](const auto &coordinate) { return coordinate.first == group.groupIndex; });
}

QString RuledPendingCast::pendingRuledCastCostSkipLabel() const
{
    return pendingRuledCastCostGroupIsOptional() ? spell.castCostGroups.at(spell.nextCastCostGroup).skipLabel
                                                 : QString{};
}

QVector<RuledCastCostOption> RuledPendingCast::pendingRuledCastCostOptions() const
{
    if (!isAwaitingRuledCastCostOption()) {
        return {};
    }
    auto options = spell.castCostGroups.at(spell.nextCastCostGroup).options;
    const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
    const bool full = ruledCastCostGroupSelectionCount(spell, group.groupIndex) >= group.max;
    for (auto &option : options) {
        const QPair<int, int> coordinate{group.groupIndex, option.optionIndex};
        const bool selected = ruledCastCostOptionAlreadySelected(spell, group.groupIndex, option.optionIndex);
        option.selectable = option.selectable && (!full || selected) && !spell.modeLinkedCastCosts.contains(coordinate);
        if (selected)
            option.label = QCoreApplication::translate("PlayerActions", "Selected: %1").arg(option.label);
    }
    return options;
}

int RuledPendingCast::pendingRuledCastCostSelectedCount() const
{
    if (isAwaitingRuledCastCostObject()) {
        const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
        const auto selection =
            std::find_if(spell.castCostSelections.cbegin(), spell.castCostSelections.cend(), [&](const auto &entry) {
                return entry.groupIndex == group.groupIndex && entry.optionIndex == spell.activeCastCostOption;
            });
        return selection == spell.castCostSelections.cend() ? 0 : selection->selectedObjectIds.size();
    }
    if (!isAwaitingRuledCastCostOption())
        return 0;
    const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
    return ruledCastCostGroupSelectionCount(spell, group.groupIndex);
}

int RuledPendingCast::pendingRuledCastCostMinimum() const
{
    if (isAwaitingRuledCastCostObject()) {
        const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
        const auto option = std::find_if(group.options.cbegin(), group.options.cend(), [&](const auto &entry) {
            return entry.optionIndex == spell.activeCastCostOption;
        });
        return option == group.options.cend() ? 0 : option->objectMin;
    }
    return isAwaitingRuledCastCostOption() ? spell.castCostGroups.at(spell.nextCastCostGroup).min : 0;
}

int RuledPendingCast::pendingRuledCastCostMaximum() const
{
    if (isAwaitingRuledCastCostObject()) {
        const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
        const auto option = std::find_if(group.options.cbegin(), group.options.cend(), [&](const auto &entry) {
            return entry.optionIndex == spell.activeCastCostOption;
        });
        return option == group.options.cend() ? 0 : option->objectMax;
    }
    return isAwaitingRuledCastCostOption() ? spell.castCostGroups.at(spell.nextCastCostGroup).max : 0;
}

bool RuledPendingCast::pendingRuledCastCostObjectCanConfirm() const
{
    if (!isAwaitingRuledCastCostObject())
        return false;
    const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
    const auto option = std::find_if(group.options.cbegin(), group.options.cend(), [&](const auto &entry) {
        return entry.optionIndex == spell.activeCastCostOption;
    });
    const auto selection =
        std::find_if(spell.castCostSelections.cbegin(), spell.castCostSelections.cend(), [&](const auto &entry) {
            return entry.groupIndex == group.groupIndex && entry.optionIndex == spell.activeCastCostOption;
        });
    if (option == group.options.cend() || selection == spell.castCostSelections.cend())
        return false;
    const qint64 total = std::accumulate(
        selection->selectedObjectIds.cbegin(), selection->selectedObjectIds.cend(), qint64{0},
        [&](qint64 value, quint32 oid) { return value + selection->selectedObjectContributions.value(oid); });
    return selection->selectedObjectIds.size() >= option->objectMin &&
           selection->selectedObjectIds.size() <= option->objectMax &&
           (option->aggregateMinimum <= 0 || total >= option->aggregateMinimum);
}

bool RuledPendingCast::pendingRuledCastCostObjectUsesExplicitConfirmation() const
{
    if (!isAwaitingRuledCastCostObject())
        return false;
    const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
    const auto option = std::find_if(group.options.cbegin(), group.options.cend(), [&](const auto &entry) {
        return entry.optionIndex == spell.activeCastCostOption;
    });
    return option != group.options.cend() && ruledCastCostUsesPermanentCohort(option->kind) && option->objectMax > 1;
}

bool RuledPendingCast::isAwaitingRuledSpellCostSelection() const
{
    return spell.valid && spell.waitingForCost && spell.nextCostChoice < spell.costChoices.size();
}

bool RuledPendingCast::isAwaitingRuledCastCostObject() const
{
    return spell.valid && spell.waitingForCastCostObject && spell.nextCastCostGroup < spell.castCostGroups.size();
}

bool RuledPendingCast::isAwaitingRuledAbilityCostSelection() const
{
    return ability.valid && ability.waitingForCost && ability.nextCostChoice < ability.costChoices.size();
}

bool RuledPendingCast::isAwaitingRuledCounterGroupSelection() const
{
    return isAwaitingRuledAbilityCostSelection() && ability.stage() == PendingActivatedAbility::Stage::Announcing &&
           !ability.ownedBegin() && hasCounterGroupChoice(ability.costChoices);
}

bool RuledPendingCast::ruledCounterGroupSelectionConfirmable() const
{
    if (!isAwaitingRuledCounterGroupSelection()) {
        return false;
    }
    auto validated = ability;
    return ruledConfirmCounterContributions(validated);
}

bool RuledPendingCast::getRuledCounterGroupSelectionProgress(quint64 &required, quint64 &selected) const
{
    if (!isAwaitingRuledCounterGroupSelection() || !validCounterGroupOfferSet(ability.costChoices)) {
        required = 0;
        selected = 0;
        return false;
    }
    required = 0;
    selected = 0;
    for (const auto &choice : ability.costChoices) {
        const quint64 amount = choice.counterRemovalGroup->requiredCount;
        if (amount > std::numeric_limits<quint64>::max() - required) {
            return false;
        }
        required += amount;
    }
    for (const auto &row : ability.counterGroupContributions) {
        if (row.count > std::numeric_limits<quint64>::max() - selected) {
            return false;
        }
        selected += row.count;
    }
    return true;
}

QString RuledPendingCast::pendingRuledAbilityCostPromptText() const
{
    if (ability.valid && ability.deferredReturnTappedCreature && ability.waitingForReturnTappedCreatureCandidate)
        return QCoreApplication::translate("PlayerActions",
                                          "Activate a mana ability that taps a creature you control, then return it to its owner's hand to activate %1.")
            .arg(ability.cardName);
    if (!isAwaitingRuledAbilityCostSelection()) {
        return {};
    }
    const auto &choice = ability.costChoices.at(ability.nextCostChoice);
    if (hasCounterGroupChoice(ability.costChoices)) {
        const auto tr = [](const char *text) { return QCoreApplication::translate("PlayerActions", text); };
        QString prompt = tr("Remove the offered counter quantities from eligible permanents. Shared availability applies across all cost components.");
        for (int index = 0; index < ability.costChoices.size(); ++index) {
            const auto &groupChoice = ability.costChoices.at(index);
            if (!groupChoice.counterRemovalGroup) {
                continue;
            }
            quint64 count = 0;
            bool countValid = true;
            for (const auto &row : ability.counterGroupContributions) {
                if (row.costIndex != groupChoice.costIndex) {
                    continue;
                }
                if (row.count > std::numeric_limits<quint64>::max() - count) {
                    countValid = false;
                    break;
                }
                count += row.count;
            }
            const QString selected = countValid ? QString::number(static_cast<qulonglong>(count)) : tr("unavailable");
            const QString required =
                QString::number(static_cast<qulonglong>(groupChoice.counterRemovalGroup->requiredCount));
            prompt += tr("\nComponent %1: %2 / %3 counters")
                          .arg(index + 1)
                          .arg(selected, required);
        }
        prompt += ability.counterGroupConfirmed
                      ? tr("\nAll components are confirmed. Continue to payment.")
                      : tr("\nComplete each component, then confirm all costs.");
        return prompt;
    }
    const QString prompt = ruledCostSelectionPrompt(choice, ability.cardName);
    const auto progress = ruledPendingGraveyardCostSelectionProgress(ability);
    return choice.aggregateMinimum > 0 && progress ? QCoreApplication::translate("PlayerActions", "%1\nTotal: %2 / %3")
                                                         .arg(prompt)
                                                         .arg(progress->selected)
                                                         .arg(progress->required)
                                                   : prompt;
}

bool RuledPendingCast::isAwaitingRuledGraveyardCostSelection() const
{
    const bool abilityMulti = isAwaitingRuledAbilityCostSelection() &&
                              (ability.costChoices.at(ability.nextCostChoice).zone == RuledCostChoiceZone::Graveyard ||
                               ruledCostUsesObjectRefs(ability.costChoices.at(ability.nextCostChoice)));
    const bool spellTap =
        isAwaitingRuledSpellCostSelection() && ruledCostUsesObjectRefs(spell.costChoices.at(spell.nextCostChoice));
    return abilityMulti || spellTap;
}

bool RuledPendingCast::isRuledGraveyardCostObjectSelected(quint32 objectId) const
{
    return ruledPendingGraveyardCostSelectionContains(ability, objectId) ||
           ruledPendingGraveyardCostSelectionContains(spell, objectId);
}

bool RuledPendingCast::getRuledGraveyardCostSelectionProgress(int &required, int &selected) const
{
    auto progress = ruledPendingGraveyardCostSelectionProgress(ability);
    if (!progress.has_value()) {
        progress = ruledPendingGraveyardCostSelectionProgress(spell);
    }
    if (!progress.has_value()) {
        // Fail closed if an inconsistent local snapshot reaches the prompt renderer.
        required = 1;
        selected = 0;
        return false;
    }
    required = static_cast<int>(progress->required);
    selected = static_cast<int>(progress->selected);
    return true;
}

QString RuledPendingCast::pendingRuledSpellPromptText() const
{
    if (!spell.valid || spell.inDamageAllocationMode) {
        return {};
    }
    if (spell.waitingForCastCostObject && spell.nextCastCostGroup < spell.castCostGroups.size()) {
        const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
        const auto option = std::find_if(group.options.cbegin(), group.options.cend(), [this](const auto &entry) {
            return entry.optionIndex == spell.activeCastCostOption;
        });
        if (option == group.options.cend()) {
            return group.prompt;
        }
        const QString instruction = ruledCastCostSelectionPrompt(*option);
        QString progress;
        if (ruledCastCostUsesPermanentCohort(option->kind)) {
            const auto selection = std::find_if(
                spell.castCostSelections.cbegin(), spell.castCostSelections.cend(), [&](const auto &entry) {
                    return entry.groupIndex == group.groupIndex && entry.optionIndex == option->optionIndex;
                });
            if (selection != spell.castCostSelections.cend()) {
                const qint64 total =
                    std::accumulate(selection->selectedObjectIds.cbegin(), selection->selectedObjectIds.cend(),
                                    qint64{0}, [&](qint64 value, quint32 oid) {
                                        return value + selection->selectedObjectContributions.value(oid);
                                    });
                progress = option->aggregateMinimum > 0
                               ? QCoreApplication::translate("PlayerActions", "\nSelected: %1, total power: %2 / %3")
                                     .arg(selection->selectedObjectIds.size())
                                     .arg(total)
                                     .arg(option->aggregateMinimum)
                               : QCoreApplication::translate("PlayerActions", "\nSelected: %1 / %2")
                                     .arg(selection->selectedObjectIds.size())
                                     .arg(option->objectMax);
            }
        }
        const QString text = instruction + progress;
        return spell.castCostObjectError.isEmpty()
                   ? text
                   : QCoreApplication::translate("PlayerActions", "%1\n%2").arg(spell.castCostObjectError, text);
    }
    if (spell.nextCastCostGroup < spell.castCostGroups.size()) {
        const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
        return QCoreApplication::translate("PlayerActions", "%1\nSelected: %2 (%3-%4)")
            .arg(group.prompt)
            .arg(ruledCastCostGroupSelectionCount(spell, group.groupIndex))
            .arg(group.min)
            .arg(group.max);
    }
    if (spell.waitingForTarget) {
        return {};
    }
    if (isAwaitingRuledSpellCostSelection()) {
        const auto &choice = spell.costChoices.at(spell.nextCostChoice);
        const QString prompt = ruledCostSelectionPrompt(choice, spell.cardName);
        const auto progress = ruledPendingGraveyardCostSelectionProgress(spell);
        return choice.aggregateMinimum > 0 && progress
                   ? QCoreApplication::translate("PlayerActions", "%1\nTotal: %2 / %3")
                         .arg(prompt)
                         .arg(progress->selected)
                         .arg(progress->required)
                   : prompt;
    }
    if (totalRemainingForCost(spell.remainingCost, spell.flexPips) == 0) {
        return {};
    }
    return QCoreApplication::translate("PlayerActions", "Pay mana for %1: %2 remaining (click mana counters).")
        .arg(spell.cardName, formatRemainingCost(spell.remainingCost, spell.flexPips));
}

QString RuledPendingCast::pendingRuledAbilityPromptText() const
{
    if (!ability.valid || !ability.waitingForMana) {
        return {};
    }
    if (totalRemainingForCost(ability.remainingCost, ability.flexPips) == 0) {
        return {};
    }
    return QCoreApplication::translate("PlayerActions", "Pay mana for %1: %2 remaining (click mana counters).")
        .arg(ability.cardName, formatRemainingCost(ability.remainingCost, ability.flexPips));
}

bool RuledPendingCast::declineCastCostGroup()
{
    if (!pendingRuledCastCostGroupIsOptional())
        return false;
    const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
    for (auto it = spell.castCostSelections.begin(); it != spell.castCostSelections.end();) {
        if (it->groupIndex != group.groupIndex) {
            ++it;
            continue;
        }
        const auto option = std::find_if(group.options.cbegin(), group.options.cend(),
                                         [&it](const auto &entry) { return entry.optionIndex == it->optionIndex; });
        if (option != group.options.cend() && option->kind == RuledCastCostOptionKind::Mana) {
            const auto extra =
                it->repetitions ? QMap<QChar, int>{{'X', repeatedCastCostAmount(*option, *it->repetitions).value_or(0)}}
                                : parseSimpleManaCost(option->additionalManaCost);
            for (auto mana = extra.constBegin(); mana != extra.constEnd(); ++mana)
                spell.remainingCost[mana.key()] -= mana.value();
        }
        spell.castCostGenericReduction -= it->genericCostReduction;
        it = spell.castCostSelections.erase(it);
    }
    spell.waitingForCastCostObject = false;
    spell.activeCastCostOption = -1;
    spell.castCostObjectError.clear();
    ++spell.nextCastCostGroup;
    return true;
}
