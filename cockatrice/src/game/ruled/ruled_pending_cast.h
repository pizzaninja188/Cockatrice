/**
 * @file ruled_pending_cast.h
 * @ingroup GameLogic
 * @brief Local-player state for an in-progress ruled spell cast or ability activation.
 *
 * This is UI state, not authoritative rules state. The engine supplies every legal action and
 * target set, and validates the completed command. PlayerActions owns one instance and keeps the
 * existing click/payment orchestration as thin access to this fork-owned state holder.
 */

#ifndef COCKATRICE_RULED_PENDING_CAST_H
#define COCKATRICE_RULED_PENDING_CAST_H

#include "ruled_client_state.h"

#include <QChar>
#include <QHash>
#include <QList>
#include <QMap>
#include <QString>
#include <QStringList>
#include <QVector>
#include <QtGlobal>
#include <algorithm>
#include <optional>

class QWidget;
class CardItem;
class Player;
class PlayerActions;

struct RuledCardActionMenuOption
{
    enum class Kind
    {
        CastFace,
        ActivateAbility,
    };

    Kind kind = Kind::CastFace;
    int index = -1;
    QString label;
    bool enabled = true;
};

struct RuledFlexPip
{
    quint32 pipIndex = 0;
    QChar colorA;
    QChar colorB;
    int generic = 0;
    bool phyrexian = false;
    int genericPaid = 0;
};

struct RuledPendingCostSelection
{
    int costIndex = -1;
    RuledCostChoiceZone zone = RuledCostChoiceZone::Battlefield;
    /// Stable Server_Card.id for hand choices; engine ObjectId for battlefield/graveyard choices.
    /// Single-card legacy costs carry one value; bounded graveyard costs carry the complete set.
    QVector<quint32> selectedIds;
};

struct RuledPendingCastCostSelection
{
    enum class ObjectKind
    {
        None,
        Hand,
        Permanent,
    };
    int groupIndex = -1;
    int optionIndex = -1;
    ObjectKind objectKind = ObjectKind::None;
    /// Stable Server_Card.id for hand choices; engine ObjectId for battlefield choices.
    quint32 selectedId = 0;
    quint64 expectedZoneChangeGeneration = 0;
    int genericCostReduction = 0;
};

struct PendingActivatedAbility
{
    bool valid = false;
    bool permanentAction = false;
    ruled::v1::PermanentActionKind permanentActionKind = ruled::v1::PERMANENT_ACTION_KIND_UNSPECIFIED;
    std::optional<quint32> permanentActionFaceIndex;
    ruled::v1::AbilitySourceZone sourceZone = ruled::v1::ABILITY_SOURCE_ZONE_BATTLEFIELD;
    quint64 expectedZoneChangeGeneration = 0;
    quint32 permanentOid = 0;
    int abilityIndex = -1;
    QString abilityText;
    QString cardName;
    bool needsTarget = false;
    bool waitingForTarget = false;
    quint32 selectedTargetOid = 0;
    bool waitingForCost = false;
    QVector<RuledCostChoice> costChoices;
    int nextCostChoice = 0;
    QVector<RuledPendingCostSelection> costSelections;
    bool waitingForMana = false;
    QMap<QChar, int> remainingCost;
    QVector<RuledFlexPip> flexPips;
    QVector<quint32> lifePipIndices;
    bool targetingCostApplied = false;
};

struct RuledGraveyardCostSelectionProgress
{
    int required = 0;
    int selected = 0;
    bool confirmable = false;
};

/// Reconstruct the visible graveyard-cost transaction from the current engine-authored choice.
/// Generic prompt refreshes use this instead of defaulting to 0/0, and stale, duplicate, or
/// non-candidate object ids never contribute to the visible selected count.
[[nodiscard]] inline std::optional<RuledGraveyardCostSelectionProgress>
ruledPendingGraveyardCostSelectionProgress(const PendingActivatedAbility &pending)
{
    if (!pending.valid || !pending.waitingForCost || pending.nextCostChoice < 0 ||
        pending.nextCostChoice >= pending.costChoices.size()) {
        return std::nullopt;
    }
    const auto &choice = pending.costChoices.at(pending.nextCostChoice);
    if (choice.zone != RuledCostChoiceZone::Graveyard) {
        return std::nullopt;
    }

    QVector<quint32> validSelectedIds;
    const auto selection =
        std::find_if(pending.costSelections.cbegin(), pending.costSelections.cend(), [&choice](const auto &entry) {
            return entry.costIndex == choice.costIndex && entry.zone == choice.zone;
        });
    if (selection != pending.costSelections.cend()) {
        for (const quint32 objectId : selection->selectedIds) {
            if (choice.candidateIds.contains(objectId) && !validSelectedIds.contains(objectId)) {
                validSelectedIds.append(objectId);
            }
        }
    }

    const int selected = validSelectedIds.size();
    return RuledGraveyardCostSelectionProgress{
        choice.min,
        selected,
        selected >= choice.min && selected <= choice.max,
    };
}

[[nodiscard]] inline bool ruledPendingGraveyardCostSelectionContains(const PendingActivatedAbility &pending,
                                                                      quint32 objectId)
{
    if (objectId == 0 || !ruledPendingGraveyardCostSelectionProgress(pending).has_value()) {
        return false;
    }
    const auto &choice = pending.costChoices.at(pending.nextCostChoice);
    if (!choice.candidateIds.contains(objectId)) {
        return false;
    }
    return std::any_of(
        pending.costSelections.cbegin(), pending.costSelections.cend(), [&choice, objectId](const auto &selection) {
            return selection.costIndex == choice.costIndex && selection.zone == choice.zone &&
                   selection.selectedIds.contains(objectId);
        });
}

/// Revalidate the source identity of a pending activated-ability-shaped UI transaction. Generic
/// permanent actions deliberately carry no activated-ability index, so they must be matched
/// against the engine's typed action list instead.
[[nodiscard]] inline bool ruledPendingAbilitySourceStillCurrent(const RuledClientState &state,
                                                                 const PendingActivatedAbility &pending)
{
    if (pending.permanentAction) {
        return state
            .permanentActionFor(pending.permanentOid, pending.expectedZoneChangeGeneration,
                                pending.permanentActionKind, pending.permanentActionFaceIndex)
            .has_value();
    }
    return state.abilitySourceGeneration(pending.permanentOid) == pending.expectedZoneChangeGeneration &&
           state.activatedAbilityIndicesForOid(pending.permanentOid).contains(pending.abilityIndex);
}

struct PendingRuledSpellCast
{
    struct SelectedMode
    {
        int modeIndex = -1;
        QString label;
        bool needsTarget = false;
        RuledSpellTargetData targets;
        QVector<quint32> selectedTargetOids;
        QVector<quint32> selectedTargetDamages;
        QVector<QVector<quint32>> selectedTargetOidsByGroup;
        QVector<QVector<quint32>> selectedTargetDamagesByGroup;
    };

    int handIndex = -1;
    RuledCastSource source = RuledCastSource::Hand;
    ruled::v1::CastMethod castMethod = ruled::v1::CAST_METHOD_NORMAL;
    /// Fireball's "divided evenly, rounded down": the engine splits on resolution, so there is no
    /// allocation to collect, no one-damage-per-target cap, and zero targets is a legal cast.
    bool damageDividedEvenly = false;
    int faceIndex = 0;
    QString cardName;
    QMap<QChar, int> remainingCost;
    QVector<quint32> selectedTargetOids;
    QVector<quint32> selectedTargetDamages;
    QVector<QVector<quint32>> selectedTargetOidsByGroup;
    QVector<QVector<quint32>> selectedTargetDamagesByGroup;
    int activeTargetGroupPosition = -1;
    bool waitingForTarget = false;
    bool valid = false;
    int minTargets = 1;
    int maxTargets = 0;
    int fixedDamage = 0;
    bool isDamageTargets = false;
    int extraManaPerTarget = 0;
    bool inDamageAllocationMode = false;
    int damageAllocationTotal = 0;
    QVector<int> targetDamageAllocations;
    int xPips = 0;
    int xValue = 0;
    int genericCostReduction = 0;
    int castCostGenericReduction = 0;
    bool manaCostFinalized = false;
    QVector<RuledFlexPip> flexPips;
    QVector<quint32> lifePipIndices;
    bool waitingForCost = false;
    QVector<RuledCostChoice> costChoices;
    int nextCostChoice = 0;
    QVector<RuledPendingCostSelection> costSelections;
    QVector<RuledCastCostGroup> castCostGroups;
    int nextCastCostGroup = 0;
    bool waitingForCastCostObject = false;
    int activeCastCostOption = -1;
    QString castCostObjectError;
    QVector<RuledPendingCastCostSelection> castCostSelections;
    QVector<SelectedMode> selectedModes;
    int activeModePosition = -1;
};

/// A required exactly-one target group completes on the target click. Every other legal range
/// needs an explicit confirmation surface, including optional 0-1 groups where confirming zero
/// targets is semantically different from cancelling the entire cast.
[[nodiscard]] inline bool ruledTargetRangeUsesExplicitConfirmation(int minTargets, int maxTargets)
{
    return minTargets == 0 || maxTargets != 1;
}

[[nodiscard]] inline bool ruledTargetGroupUsesExplicitConfirmation(const RuledTargetGroupData &group)
{
    return ruledTargetRangeUsesExplicitConfirmation(group.minTargets, group.maxTargets);
}

[[nodiscard]] inline bool ruledPendingTargetSelectionCanConfirm(const PendingRuledSpellCast &spell)
{
    const int selected = spell.selectedTargetOids.size();
    return spell.valid && spell.waitingForTarget &&
           ruledTargetRangeUsesExplicitConfirmation(spell.minTargets, spell.maxTargets) &&
           selected >= spell.minTargets && selected <= spell.maxTargets;
}

enum class RuledPendingPaymentAction
{
    None,
    CastSpell,
    ActivateAbility,
};

/// Physical surface the user clicked while a CR 115 target choice is pending.
enum class RuledTargetCandidateKind
{
    Battlefield,
    Stack,
    Graveyard,
    Player,
};

/// Tri-state result lets normal/freeform handling continue when no target choice exists, while an
/// illegal candidate consumes the click before CardItem/PlayerTarget can perform another action.
enum class RuledTargetClickEligibility
{
    NotTargeting,
    Legal,
    Illegal,
};

enum class RuledCastCostCandidateKind
{
    Hand,
    Permanent,
};

/// Engine-authored click affordance for the object stage of a cast-cost option. This stays
/// separate from CR 115 targeting: behold is a nontargeted cost choice, but the card surface still
/// needs the same legal/illegal cursor contract while the local transaction is staged.
[[nodiscard]] inline RuledTargetClickEligibility
ruledCastCostObjectEligibility(const PendingRuledSpellCast &spell, RuledCastCostCandidateKind kind, quint32 id)
{
    if (!spell.valid || !spell.waitingForCastCostObject) {
        return RuledTargetClickEligibility::NotTargeting;
    }
    if (spell.nextCastCostGroup < 0 || spell.nextCastCostGroup >= spell.castCostGroups.size()) {
        return RuledTargetClickEligibility::Illegal;
    }
    const auto &group = spell.castCostGroups.at(spell.nextCastCostGroup);
    const auto option = std::find_if(group.options.cbegin(), group.options.cend(), [&spell](const auto &entry) {
        return entry.optionIndex == spell.activeCastCostOption;
    });
    if (option == group.options.cend() || !option->selectable ||
        (option->kind != RuledCastCostOptionKind::Behold &&
         option->kind != RuledCastCostOptionKind::TapPermanentForGenericReduction)) {
        return RuledTargetClickEligibility::Illegal;
    }
    const bool legal = kind == RuledCastCostCandidateKind::Hand
                           ? option->kind == RuledCastCostOptionKind::Behold && option->validHandIndices.contains(id)
                           : option->validPermanentIds.contains(id);
    return legal ? RuledTargetClickEligibility::Legal : RuledTargetClickEligibility::Illegal;
}

/// Cast-cost groups are declarations made before targeting and mana payment. Merely displaying
/// the current group's option buttons does not complete that declaration: finalizing mana there
/// would freeze the unreduced cost before Harmonize can record the selected creature's power.
[[nodiscard]] inline bool ruledCastCostGroupsComplete(const PendingRuledSpellCast &spell)
{
    return spell.valid && !spell.waitingForCastCostObject &&
           spell.nextCastCostGroup >= spell.castCostGroups.size();
}

[[nodiscard]] inline bool ruledTargetDataContains(const RuledTargetGroupData &data,
                                                  RuledTargetCandidateKind kind,
                                                  quint32 oid,
                                                  int localPlayerId)
{
    switch (kind) {
        case RuledTargetCandidateKind::Battlefield:
            return data.validPermanentIds.contains(oid);
        case RuledTargetCandidateKind::Stack:
            return data.validStackIds.contains(oid);
        case RuledTargetCandidateKind::Graveyard:
            return data.validGraveyardIds.contains(oid);
        case RuledTargetCandidateKind::Player:
            return oid == static_cast<quint32>(localPlayerId) ? data.canTargetSelf : data.canTargetOpponent;
    }
    return false;
}

/// Encode the physical surface represented by an engine ObjectId. The candidate group is
/// authoritative; this exists because player ids and object ids intentionally share integers.
[[nodiscard]] inline ruled::v1::TargetRefKind ruledTargetRefKind(const RuledTargetGroupData &data,
                                                                 quint32 oid,
                                                                 int localPlayerId)
{
    if (data.validGraveyardIds.contains(oid)) {
        return ruled::v1::TARGET_REF_KIND_GRAVEYARD;
    }
    if (data.validStackIds.contains(oid)) {
        return ruled::v1::TARGET_REF_KIND_STACK;
    }
    if (data.validPermanentIds.contains(oid)) {
        return ruled::v1::TARGET_REF_KIND_PERMANENT;
    }
    if (oid == static_cast<quint32>(localPlayerId) ? data.canTargetSelf : data.canTargetOpponent) {
        return ruled::v1::TARGET_REF_KIND_PLAYER;
    }
    return ruled::v1::TARGET_REF_KIND_UNSPECIFIED;
}

inline void ruledAccumulateTargetingCosts(const RuledSpellTargetData &data,
                                          const QVector<QVector<quint32>> &selectedByGroup,
                                          const QVector<quint32> &fallbackSelected,
                                          int localPlayerId,
                                          QHash<quint64, int> &activeApplications)
{
    for (int groupPosition = 0; groupPosition < data.groups.size(); ++groupPosition) {
        const auto &group = data.groups.at(groupPosition);
        const QVector<quint32> selected = groupPosition < selectedByGroup.size()
                                                ? selectedByGroup.at(groupPosition)
                                                : (data.groups.size() == 1 ? fallbackSelected : QVector<quint32>{});
        for (const quint32 oid : selected) {
            const auto kind = ruledTargetRefKind(group, oid, localPlayerId);
            for (const auto &application : data.targetingCostApplications) {
                const bool affected = std::any_of(
                    application.affectedTargets.cbegin(), application.affectedTargets.cend(),
                    [kind, oid](const auto &candidate) { return candidate.kind == kind && candidate.oid == oid; });
                if (affected) {
                    activeApplications.insert(application.applicationId, application.genericMana);
                }
            }
        }
    }
}

[[nodiscard]] inline int ruledModalSpellTargetingCost(const PendingRuledSpellCast &spell, int localPlayerId)
{
    QHash<quint64, int> active;
    for (const auto &mode : spell.selectedModes) {
        ruledAccumulateTargetingCosts(mode.targets, mode.selectedTargetOidsByGroup, mode.selectedTargetOids,
                                      localPlayerId, active);
    }
    int total = 0;
    for (auto it = active.cbegin(); it != active.cend(); ++it) {
        total += it.value();
    }
    return total;
}

[[nodiscard]] inline int ruledTargetingCostForSelection(const RuledSpellTargetData &data,
                                                        const QVector<QVector<quint32>> &selectedByGroup,
                                                        const QVector<quint32> &fallbackSelected,
                                                        int localPlayerId)
{
    QHash<quint64, int> active;
    ruledAccumulateTargetingCosts(data, selectedByGroup, fallbackSelected, localPlayerId, active);
    int total = 0;
    for (auto it = active.cbegin(); it != active.cend(); ++it) {
        total += it.value();
    }
    return total;
}

inline void ruledAccumulateTargetedCostReductions(const RuledSpellTargetData &data,
                                                  const QVector<QVector<quint32>> &selectedByGroup,
                                                  const QVector<quint32> &fallbackSelected,
                                                  int localPlayerId,
                                                  QHash<quint64, int> &activeApplications)
{
    for (int groupPosition = 0; groupPosition < data.groups.size(); ++groupPosition) {
        const auto &group = data.groups.at(groupPosition);
        const QVector<quint32> selected = groupPosition < selectedByGroup.size()
                                                ? selectedByGroup.at(groupPosition)
                                                : (data.groups.size() == 1 ? fallbackSelected : QVector<quint32>{});
        for (const quint32 oid : selected) {
            const auto kind = ruledTargetRefKind(group, oid, localPlayerId);
            for (const auto &application : data.targetedCostReductionApplications) {
                const bool qualifies = std::any_of(
                    application.qualifyingTargets.cbegin(), application.qualifyingTargets.cend(),
                    [kind, oid](const auto &candidate) { return candidate.kind == kind && candidate.oid == oid; });
                if (qualifies) {
                    activeApplications.insert(application.applicationId, application.genericMana);
                }
            }
        }
    }
}

[[nodiscard]] inline int ruledModalSpellTargetedCostReduction(const PendingRuledSpellCast &spell,
                                                               int localPlayerId)
{
    QHash<quint64, int> active;
    for (const auto &mode : spell.selectedModes) {
        ruledAccumulateTargetedCostReductions(mode.targets, mode.selectedTargetOidsByGroup,
                                              mode.selectedTargetOids, localPlayerId, active);
    }
    int total = 0;
    for (auto it = active.cbegin(); it != active.cend(); ++it) {
        total += it.value();
    }
    return total;
}

[[nodiscard]] inline int ruledTargetedCostReductionForSelection(
    const RuledSpellTargetData &data,
    const QVector<QVector<quint32>> &selectedByGroup,
    const QVector<quint32> &fallbackSelected,
    int localPlayerId)
{
    QHash<quint64, int> active;
    ruledAccumulateTargetedCostReductions(data, selectedByGroup, fallbackSelected, localPlayerId, active);
    int total = 0;
    for (auto it = active.cbegin(); it != active.cend(); ++it) {
        total += it.value();
    }
    return total;
}

/// CR 601.2f quote arithmetic: the caller's base generic already includes chosen X; all generic
/// increases are added before reductions, and reductions cannot make the generic component negative.
[[nodiscard]] inline int ruledFinalGenericCost(int baseGeneric, int genericIncreases, int genericReduction)
{
    return qMax(0, baseGeneric + genericIncreases - genericReduction);
}

[[nodiscard]] inline std::optional<RuledSpellTargetData>
currentRuledSpellTargetData(const PendingRuledSpellCast &spell, const RuledClientState &state)
{
    if (!spell.valid) {
        return std::nullopt;
    }
    if (spell.activeModePosition >= 0 && spell.activeModePosition < spell.selectedModes.size()) {
        return state.modalSpellTargetData(spell.handIndex, spell.faceIndex,
                                          spell.selectedModes.at(spell.activeModePosition).modeIndex, spell.source,
                                          spell.castMethod);
    }
    return state.spellTargetData(spell.handIndex, spell.faceIndex, spell.source);
}

[[nodiscard]] inline std::optional<RuledTargetGroupData>
currentRuledSpellTargetGroup(const PendingRuledSpellCast &spell, const RuledClientState &state)
{
    const auto data = currentRuledSpellTargetData(spell, state);
    if (!data.has_value() || spell.activeTargetGroupPosition < 0 ||
        spell.activeTargetGroupPosition >= data->groups.size()) {
        return std::nullopt;
    }
    return data->groups.at(spell.activeTargetGroupPosition);
}

/// One authoritative click predicate for every true target-selection flow. Untargeted resolution
/// and cost choices deliberately stay out of this function.
[[nodiscard]] inline RuledTargetClickEligibility
ruledTargetClickEligibility(const PendingRuledSpellCast &spell,
                            const PendingActivatedAbility &ability,
                            const RuledClientState &state,
                            RuledTargetCandidateKind kind,
                            quint32 oid,
                            int localPlayerId)
{
    if (state.hasPendingChoiceOfKind(RuledClientState::ChoiceKind::CopyTarget)) {
        const bool supportedSurface = kind == RuledTargetCandidateKind::Battlefield ||
                                      kind == RuledTargetCandidateKind::Stack ||
                                      kind == RuledTargetCandidateKind::Player;
        return supportedSurface && state.isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopyTarget, oid)
                   ? RuledTargetClickEligibility::Legal
                   : RuledTargetClickEligibility::Illegal;
    }
    if (state.hasPendingChoiceOfKind(RuledClientState::ChoiceKind::AuraPermanent)) {
        return kind == RuledTargetCandidateKind::Battlefield &&
                       state.isPendingChoiceCandidate(RuledClientState::ChoiceKind::AuraPermanent, oid)
                   ? RuledTargetClickEligibility::Legal
                   : RuledTargetClickEligibility::Illegal;
    }
    if (state.hasPendingChoiceOfKind(RuledClientState::ChoiceKind::AuraPlayer)) {
        return kind == RuledTargetCandidateKind::Player &&
                       state.isPendingChoiceCandidate(RuledClientState::ChoiceKind::AuraPlayer, oid)
                   ? RuledTargetClickEligibility::Legal
                   : RuledTargetClickEligibility::Illegal;
    }
    if (state.hasPendingTriggerTarget()) {
        const auto refKind = kind == RuledTargetCandidateKind::Graveyard
                                 ? ruled::v1::TARGET_REF_KIND_GRAVEYARD
                             : kind == RuledTargetCandidateKind::Stack ? ruled::v1::TARGET_REF_KIND_STACK
                             : kind == RuledTargetCandidateKind::Player ? ruled::v1::TARGET_REF_KIND_PLAYER
                                                                        : ruled::v1::TARGET_REF_KIND_PERMANENT;
        const int targetPlayerId = kind == RuledTargetCandidateKind::Player ? static_cast<int>(oid) : localPlayerId;
        return state.isPendingTriggerTargetCandidate(refKind, oid, targetPlayerId)
                   ? RuledTargetClickEligibility::Legal
                   : RuledTargetClickEligibility::Illegal;
    }
    if (ability.valid && ability.waitingForTarget) {
        const auto data = state.abilityTargetData(ability.permanentOid, ability.abilityIndex);
        return ruledTargetDataContains(data, kind, oid, localPlayerId) ? RuledTargetClickEligibility::Legal
                                                                       : RuledTargetClickEligibility::Illegal;
    }
    if (spell.valid && spell.waitingForTarget) {
        const auto data = currentRuledSpellTargetGroup(spell, state);
        return data.has_value() && ruledTargetDataContains(*data, kind, oid, localPlayerId)
                   ? RuledTargetClickEligibility::Legal
                   : RuledTargetClickEligibility::Illegal;
    }
    return RuledTargetClickEligibility::NotTargeting;
}

[[nodiscard]] inline bool ruledTargetDataContainsOid(const RuledTargetGroupData &data,
                                                     quint32 oid,
                                                     int localPlayerId)
{
    return data.validPermanentIds.contains(oid) || data.validStackIds.contains(oid) ||
           data.validGraveyardIds.contains(oid) ||
           (oid == static_cast<quint32>(localPlayerId) ? data.canTargetSelf : data.canTargetOpponent);
}

/// Remove locally staged targets that disappeared from the newest LegalActions snapshot. Parallel
/// damage vectors are pruned at the same indices so a later command cannot pair damage with the
/// wrong object.
[[nodiscard]] inline bool reconcileRuledPendingTargets(PendingRuledSpellCast &spell,
                                                       PendingActivatedAbility &ability,
                                                       const RuledClientState &state,
                                                       int localPlayerId)
{
    bool changed = false;
    const auto prune = [&](QVector<quint32> &oids,
                           QVector<quint32> &damages,
                           QVector<int> *allocations,
                           const RuledTargetGroupData &data) {
        for (int i = oids.size() - 1; i >= 0; --i) {
            if (ruledTargetDataContainsOid(data, oids.at(i), localPlayerId)) {
                continue;
            }
            oids.remove(i);
            if (i < damages.size()) {
                damages.remove(i);
            }
            if (allocations && i < allocations->size()) {
                allocations->remove(i);
            }
            changed = true;
        }
    };

    if (spell.valid) {
        if (spell.selectedModes.isEmpty()) {
            const auto data = state.spellTargetData(spell.handIndex, spell.faceIndex, spell.source);
            while (spell.selectedTargetOidsByGroup.size() < data.groups.size()) {
                spell.selectedTargetOidsByGroup.append(QVector<quint32>{});
            }
            while (spell.selectedTargetDamagesByGroup.size() < data.groups.size()) {
                spell.selectedTargetDamagesByGroup.append(QVector<quint32>{});
            }
            if (spell.activeTargetGroupPosition >= 0 &&
                spell.activeTargetGroupPosition < spell.selectedTargetOidsByGroup.size()) {
                spell.selectedTargetOidsByGroup[spell.activeTargetGroupPosition] = spell.selectedTargetOids;
                spell.selectedTargetDamagesByGroup[spell.activeTargetGroupPosition] = spell.selectedTargetDamages;
            }
            for (int groupIndex = 0; groupIndex < data.groups.size(); ++groupIndex) {
                QVector<int> *const allocations = groupIndex == spell.activeTargetGroupPosition
                                                      ? &spell.targetDamageAllocations
                                                      : nullptr;
                prune(spell.selectedTargetOidsByGroup[groupIndex], spell.selectedTargetDamagesByGroup[groupIndex],
                      allocations, data.groups.at(groupIndex));
            }
            if (spell.activeTargetGroupPosition >= 0 &&
                spell.activeTargetGroupPosition < spell.selectedTargetOidsByGroup.size()) {
                spell.selectedTargetOids = spell.selectedTargetOidsByGroup.at(spell.activeTargetGroupPosition);
                spell.selectedTargetDamages =
                    spell.selectedTargetDamagesByGroup.at(spell.activeTargetGroupPosition);
            }
            for (int groupIndex = 0; groupIndex < data.groups.size(); ++groupIndex) {
                const auto &group = data.groups.at(groupIndex);
                if (spell.selectedTargetOidsByGroup.at(groupIndex).size() < group.minTargets) {
                    spell.activeTargetGroupPosition = groupIndex;
                    spell.selectedTargetOids = spell.selectedTargetOidsByGroup.at(groupIndex);
                    spell.selectedTargetDamages = spell.selectedTargetDamagesByGroup.at(groupIndex);
                    spell.minTargets = group.minTargets;
                    spell.maxTargets = group.maxTargets;
                    spell.waitingForTarget = true;
                    break;
                }
            }
        } else {
            for (int modePosition = 0; modePosition < spell.selectedModes.size(); ++modePosition) {
                auto &mode = spell.selectedModes[modePosition];
                const auto data = state.modalSpellTargetData(spell.handIndex, spell.faceIndex, mode.modeIndex,
                                                             spell.source, spell.castMethod);
                if (!data.has_value()) {
                    continue;
                }
                while (mode.selectedTargetOidsByGroup.size() < data->groups.size()) {
                    mode.selectedTargetOidsByGroup.append(QVector<quint32>{});
                }
                while (mode.selectedTargetDamagesByGroup.size() < data->groups.size()) {
                    mode.selectedTargetDamagesByGroup.append(QVector<quint32>{});
                }
                if (modePosition == spell.activeModePosition && spell.activeTargetGroupPosition >= 0 &&
                    spell.activeTargetGroupPosition < mode.selectedTargetOidsByGroup.size()) {
                    mode.selectedTargetOidsByGroup[spell.activeTargetGroupPosition] = spell.selectedTargetOids;
                    mode.selectedTargetDamagesByGroup[spell.activeTargetGroupPosition] =
                        spell.selectedTargetDamages;
                }
                for (int groupIndex = 0; groupIndex < data->groups.size(); ++groupIndex) {
                    prune(mode.selectedTargetOidsByGroup[groupIndex], mode.selectedTargetDamagesByGroup[groupIndex],
                          nullptr, data->groups.at(groupIndex));
                    if (modePosition == spell.activeModePosition &&
                        groupIndex == spell.activeTargetGroupPosition) {
                        spell.selectedTargetOids = mode.selectedTargetOidsByGroup.at(groupIndex);
                        spell.selectedTargetDamages = mode.selectedTargetDamagesByGroup.at(groupIndex);
                    }
                    const auto &group = data->groups.at(groupIndex);
                    if (mode.selectedTargetOidsByGroup.at(groupIndex).size() < group.minTargets) {
                        spell.activeModePosition = modePosition;
                        spell.activeTargetGroupPosition = groupIndex;
                        spell.selectedTargetOidsByGroup = mode.selectedTargetOidsByGroup;
                        spell.selectedTargetDamagesByGroup = mode.selectedTargetDamagesByGroup;
                        spell.selectedTargetOids = mode.selectedTargetOidsByGroup.at(groupIndex);
                        spell.selectedTargetDamages = mode.selectedTargetDamagesByGroup.at(groupIndex);
                        spell.minTargets = group.minTargets;
                        spell.maxTargets = group.maxTargets;
                        spell.waitingForTarget = true;
                        break;
                    }
                }
            }
        }
    }
    if (ability.valid && ability.selectedTargetOid != 0) {
        const auto data = state.abilityTargetData(ability.permanentOid, ability.abilityIndex);
        if (!ruledTargetDataContainsOid(data, ability.selectedTargetOid, localPlayerId)) {
            ability.selectedTargetOid = 0;
            changed = true;
        }
    }
    return changed;
}

/// Identify a locally staged spell or ability whose last mana pip was consumed while another
/// engine command (normally that mana ability) was still in flight. The caller invokes this after
/// the command lock clears and submits the returned action.
[[nodiscard]] inline RuledPendingPaymentAction readyRuledPendingPaymentAction(const PendingRuledSpellCast &spell,
                                                                              const PendingActivatedAbility &ability)
{
    const auto costIsPaid = [](const QMap<QChar, int> &fixed, const QVector<RuledFlexPip> &flex) {
        for (auto it = fixed.constBegin(); it != fixed.constEnd(); ++it) {
            if (it.value() > 0) {
                return false;
            }
        }
        return flex.isEmpty();
    };
    if (spell.valid && !spell.waitingForTarget && !spell.waitingForCost && !spell.inDamageAllocationMode &&
        costIsPaid(spell.remainingCost, spell.flexPips)) {
        return RuledPendingPaymentAction::CastSpell;
    }
    if (ability.valid && !ability.waitingForTarget && !ability.waitingForCost &&
        costIsPaid(ability.remainingCost, ability.flexPips)) {
        return RuledPendingPaymentAction::ActivateAbility;
    }
    return RuledPendingPaymentAction::None;
}

class RuledPendingCast
{
public:
    enum class InteractionKind
    {
        None,
        Spell,
        Ability,
    };

    RuledPendingCast();

    /// Shared left/right-click modal picker. Choose-one spells use ordinary menu actions;
    /// choose-N spells use persistent checkboxes plus explicit confirm/cancel controls.
    static std::optional<QVector<int>> chooseModes(QWidget *parent,
                                                   const QString &cardName,
                                                   const QVector<RuledModalSpellOption> &modes,
                                                   int minModes,
                                                   int maxModes);

    /// Pick one engine-authoritative castable face. The physical CardItem may expose only its
    /// front display name (Adventure), so menu entries come exclusively from `faces`.
    static std::optional<RuledFaceOption>
    chooseFace(QWidget *parent, const QString &cardName, const QVector<RuledFaceOption> &faces);

    /// Build one engine-authoritative menu model for alternate actions on a physical card.
    /// Castable faces precede zone abilities so a cycler in hand exposes both Cast and Cycle.
    static QVector<RuledCardActionMenuOption>
    cardActionMenuOptions(const QVector<RuledFaceOption> &castFaces,
                          const QList<int> &abilityIndices,
                          const QStringList &abilityLabels,
                          const QHash<int, bool> &abilityEnabled);

    /// Spell casts and activated abilities are mutually exclusive local UI transactions.
    PendingRuledSpellCast &beginSpell();
    PendingActivatedAbility &beginAbility();
    void clearSpell();
    void clearAbility();
    [[nodiscard]] InteractionKind activeInteraction() const;

    PendingRuledSpellCast spell;
    PendingActivatedAbility ability;
};

/// Fork-owned bridge between PlayerActions' pending state and concrete CardItem/Player target
/// surfaces. PlayerActions exposes only thin wrappers and one friend declaration.
class RuledTargetUi
{
public:
    static void ensureRefreshConnection(PlayerActions *actions);
    static void reconcile(PlayerActions *actions);
    [[nodiscard]] static RuledTargetClickEligibility cardEligibility(const PlayerActions *actions, CardItem *card);
    [[nodiscard]] static RuledTargetClickEligibility playerEligibility(const PlayerActions *actions, Player *target);
};

#endif // COCKATRICE_RULED_PENDING_CAST_H
