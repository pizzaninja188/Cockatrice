/**
 * @file ruled_pending_cast.h
 * @ingroup GameLogic
 * @brief Local-player state for an in-progress ruled spell cast or ability activation.
 *
 * This is UI state, not authoritative rules state. The engine supplies every legal action and
 * target set, and validates the completed command. PlayerActions owns one instance and keeps the
 * entry points as thin access to this state holder and the RuledPaymentUi progression bridge.
 */

#ifndef COCKATRICE_RULED_PENDING_CAST_H
#define COCKATRICE_RULED_PENDING_CAST_H

#include "ruled_client_state.h"
#include "ruled_cost_selection.h"

#include <QChar>
#include <QHash>
#include <QList>
#include <QMap>
#include <QString>
#include <QStringList>
#include <QVector>
#include <QtGlobal>
#include <algorithm>
#include <limits>
#include <numeric>
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
        PaymentContribution,
        ActivateAbility,
    };

    Kind kind = Kind::CastFace;
    int index = -1;
    QString label;
    bool enabled = true;
    int manaOptionIndex = 0;
    ruled::v1::CastMethod castMethod = ruled::v1::CAST_METHOD_NORMAL;
    quint64 castingPermissionId = 0;
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
    /// Parallel generation snapshot for typed public-zone choices; zero/empty for concealed slots.
    QVector<quint64> selectedGenerations;
    /// Opaque engine option; zero denotes an ordinary object-selection cost.
    quint32 counterOptionId = 0;
};

inline bool ruledCounterSelectionStillLegal(const RuledPendingCostSelection &selection, const RuledCostChoice &choice)
{
    if (choice.kind != RuledCostChoiceKind::RemoveCounters || choice.counterCount == 0 ||
        selection.selectedIds.size() != 1 || selection.selectedGenerations.size() != 1) {
        return false;
    }
    const quint32 objectId = selection.selectedIds.front();
    const quint64 generation = selection.selectedGenerations.front();
    const bool sourceCurrent = choice.counterSourceId != 0
                                   ? objectId == choice.counterSourceId && generation == choice.counterSourceGeneration
                                   : choice.candidateIds.contains(objectId) &&
                                         choice.candidateGenerations.contains(objectId) &&
                                         generation == choice.candidateGenerations.value(objectId);
    return sourceCurrent &&
           std::any_of(choice.counterOptions.cbegin(), choice.counterOptions.cend(), [&](const auto &option) {
               return option.optionId == selection.counterOptionId && option.availableCount >= choice.counterCount;
           });
}

inline void ruledWriteCounterRemoval(const RuledPendingCostSelection &selection, ruled::v1::CostSelection &command)
{
    auto *removal = command.mutable_counter_removal();
    removal->set_option_id(selection.counterOptionId);
    removal->mutable_source()->set_object_id(selection.selectedIds.value(0));
    removal->mutable_source()->set_zone_change_generation(selection.selectedGenerations.value(0));
}

inline void ruledWriteCostObjectRefs(const RuledPendingCostSelection &selection, ruled::v1::CostSelection &command)
{
    auto *objects = selection.zone == RuledCostChoiceZone::Graveyard ? command.mutable_graveyard_objects()
                                                                     : command.mutable_battlefield_objects();
    for (int i = 0; i < selection.selectedIds.size(); ++i) {
        auto *object = objects->add_objects();
        object->set_object_id(selection.selectedIds.at(i));
        object->set_zone_change_generation(selection.selectedGenerations.value(i));
    }
}

/// Local duplicate prevention must not rule out paying non-consuming costs on a creature that
/// is also sacrificed. The engine validates the full transaction, including changed generations.
inline bool ruledCostSelectionConflicts(const RuledCostChoice &choice,
                                        const QVector<RuledCostChoice> &choices,
                                        const RuledPendingCostSelection &already,
                                        quint32 id)
{
    if (already.costIndex == choice.costIndex || already.zone != choice.zone || !already.selectedIds.contains(id))
        return false;
    const auto previous = std::find_if(choices.cbegin(), choices.cend(),
                                       [&already](const auto &entry) { return entry.costIndex == already.costIndex; });
    if (previous == choices.cend())
        return true;
    // Returning a Sneak attacker changes zones and must be exclusive with every other use of
    // that physical permanent. The engine remains authoritative and revalidates generation.
    if (choice.kind == RuledCostChoiceKind::ReturnUnblockedAttacker ||
        previous->kind == RuledCostChoiceKind::ReturnUnblockedAttacker ||
        choice.kind == RuledCostChoiceKind::ReturnTappedCreature ||
        previous->kind == RuledCostChoiceKind::ReturnTappedCreature)
        return true;
    if (choice.kind == RuledCostChoiceKind::Blight || previous->kind == RuledCostChoiceKind::Blight ||
        choice.kind == RuledCostChoiceKind::RemoveCounters || previous->kind == RuledCostChoiceKind::RemoveCounters)
        return false;
    if ((choice.kind == RuledCostChoiceKind::Tap && previous->kind == RuledCostChoiceKind::Sacrifice) ||
        (choice.kind == RuledCostChoiceKind::Sacrifice && previous->kind == RuledCostChoiceKind::Tap))
        return false;
    return true;
}

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
    QVector<quint32> selectedObjectIds;
    QHash<quint32, quint64> selectedObjectGenerations;
    QHash<quint32, qint64> selectedObjectContributions;
    std::optional<quint32> repetitions;
};

struct PendingActivatedAbility
{
    enum class Stage
    {
        Announcing,
        BeginPending,
        Waiting,
        Paying,
        CommitPending,
        CancelPending
    };
    Stage stage = Stage::Announcing;
    quint64 engineTransactionId = 0;
    quint64 engineRevision = 0;
    bool chosenOpponentTargets = false;
    bool enginePaymentInitialized = false;
    bool valid = false;
    bool permanentAction = false;
    ruled::v1::PermanentActionKind permanentActionKind = ruled::v1::PERMANENT_ACTION_KIND_UNSPECIFIED;
    std::optional<quint32> permanentActionFaceIndex;
    ruled::v1::AbilitySourceZone sourceZone = ruled::v1::ABILITY_SOURCE_ZONE_BATTLEFIELD;
    quint64 expectedZoneChangeGeneration = 0;
    quint32 permanentOid = 0;
    int abilityIndex = -1;
    int manaOptionIndex = 0;
    quint32 xValue = 0;
    quint32 manaSplitFirstColorCount = 0;
    quint64 castingPermissionId = 0;
    QString abilityText;
    QString cardName;
    bool needsTarget = false;
    bool waitingForTarget = false;
    struct Target
    {
        ruled::v1::TargetRef ref;
        quint64 zoneChangeGeneration = 0;
    };
    QVector<Target> selectedTargets;
    int activeTargetGroupPosition = 0;
    bool waitingForCost = false;
    bool deferredReturnTappedCreature = false;
    bool waitingForReturnTappedCreatureCandidate = false;
    QVector<RuledCostChoice> costChoices;
    int nextCostChoice = 0;
    QVector<RuledPendingCostSelection> costSelections;
    bool waitingForMana = false;
    QMap<QChar, int> remainingCost;
    QVector<RuledFlexPip> flexPips;
    QVector<quint32> lifePipIndices;
    bool targetingCostApplied = false;
    bool restartAfterTargetInvalidation = false;

    /// Shared activation identity and choice header; target and payment cohorts are appended by the host.
    void writeActivationHeader(ruled::v1::ActivateAbility &command) const
    {
        command.set_source_object_id(permanentOid);
        command.set_source_zone(sourceZone);
        command.set_expected_zone_change_generation(expectedZoneChangeGeneration);
        command.set_ability_index(static_cast<quint32>(abilityIndex));
        command.set_mana_option_index(static_cast<quint32>(manaOptionIndex));
        command.set_x_value(xValue);
        command.set_mana_split_first_color_count(manaSplitFirstColorCount);
    }
};

[[nodiscard]] inline int ruledActivatedManaXPipCount(const QString &manaCost, bool hasCounterManaSplit)
{
    return hasCounterManaSplit ? 0 : manaCost.count(QLatin1Char('X'), Qt::CaseInsensitive);
}

/// The UI stores unpaid generic mana in an int; the bound is representational, not an
/// affordability guess, because players may produce mana during an activation.
[[nodiscard]] inline quint32 ruledActivatedXChoiceMaximum(const PendingActivatedAbility &pending, int xPips)
{
    if (xPips <= 0 || pending.remainingCost.value(QChar('X'), 0) < xPips) {
        return 0;
    }
    const int fixedGeneric = pending.remainingCost.value(QChar('X'), 0) - xPips;
    return static_cast<quint32>((std::numeric_limits<int>::max() - fixedGeneric) / xPips);
}

/// Convert each printed {X} mana pip from the parser's one-generic placeholder to chosen X.
/// The caller cancels the whole pending activation when the prompt returns no value.
[[nodiscard]] inline bool
ruledApplyActivatedXChoice(PendingActivatedAbility &pending, int xPips, std::optional<quint32> chosenX)
{
    if (!chosenX || xPips <= 0 || pending.remainingCost.value(QChar('X'), 0) < xPips ||
        *chosenX > ruledActivatedXChoiceMaximum(pending, xPips)) {
        return false;
    }
    pending.xValue = *chosenX;
    const int generic = pending.remainingCost.value(QChar('X'), 0) - xPips + xPips * static_cast<int>(*chosenX);
    if (generic > 0) {
        pending.remainingCost[QChar('X')] = generic;
    } else {
        pending.remainingCost.remove(QChar('X'));
    }
    return true;
}

enum class RuledXCounterManaPromptStep
{
    ChooseX,
    ChooseFirstColorCount,
};

struct RuledXCounterManaSelection
{
    quint32 xValue = 0;
    quint32 firstColorCount = 0;
};

/// Run the two bounded choices for a storage-counter mana ability. Keeping the prompt progression
/// here lets the UI and headless tests share the same zero-X, range, and cancellation behavior.
template <typename Prompt>
[[nodiscard]] inline std::optional<RuledXCounterManaSelection> ruledPromptXCounterManaSplit(quint32 maximumX,
                                                                                            Prompt &&prompt)
{
    const auto xValue = prompt(RuledXCounterManaPromptStep::ChooseX, maximumX);
    if (!xValue || *xValue > maximumX) {
        return std::nullopt;
    }
    if (*xValue == 0) {
        return RuledXCounterManaSelection{0, 0};
    }
    const auto firstColorCount = prompt(RuledXCounterManaPromptStep::ChooseFirstColorCount, *xValue);
    if (!firstColorCount || *firstColorCount > *xValue) {
        return std::nullopt;
    }
    return RuledXCounterManaSelection{*xValue, *firstColorCount};
}

struct RuledGraveyardCostSelectionProgress
{
    qint64 required = 0;
    qint64 selected = 0;
    bool confirmable = false;
    RuledCostChoiceZone zone = RuledCostChoiceZone::Graveyard;
};

/// Reconstruct the visible graveyard-cost transaction from the current engine-authored choice.
/// Generic prompt refreshes use this instead of defaulting to 0/0, and stale, duplicate, or
/// non-candidate object ids never contribute to the visible selected count.
template <typename PendingPayment>
[[nodiscard]] inline std::optional<RuledGraveyardCostSelectionProgress>
ruledPendingGraveyardCostSelectionProgress(const PendingPayment &pending)
{
    if (!pending.valid || !pending.waitingForCost || pending.nextCostChoice < 0 ||
        pending.nextCostChoice >= pending.costChoices.size()) {
        return std::nullopt;
    }
    const auto &choice = pending.costChoices.at(pending.nextCostChoice);
    if (!ruledCostNeedsConfirmation(choice)) {
        return std::nullopt;
    }

    QVector<quint32> validSelectedIds;
    const auto selection =
        std::find_if(pending.costSelections.cbegin(), pending.costSelections.cend(), [&choice](const auto &entry) {
            return entry.costIndex == choice.costIndex && entry.zone == choice.zone;
        });
    if (selection != pending.costSelections.cend()) {
        for (const quint32 objectId : selection->selectedIds) {
            if (ruledCostUsesObjectRefs(choice)) {
                const int index = selection->selectedIds.indexOf(objectId);
                if (!choice.candidateGenerations.contains(objectId) || index >= selection->selectedGenerations.size() ||
                    selection->selectedGenerations.at(index) != choice.candidateGenerations.value(objectId))
                    continue;
            }
            if (choice.candidateIds.contains(objectId) && !validSelectedIds.contains(objectId)) {
                validSelectedIds.append(objectId);
            }
        }
    }

    const qint64 selected = choice.aggregateMinimum > 0
                                ? std::accumulate(validSelectedIds.cbegin(), validSelectedIds.cend(), qint64{0},
                                                  [&choice](qint64 total, quint32 objectId) {
                                                      return total + choice.candidateContributions.value(objectId);
                                                  })
                                : validSelectedIds.size();
    const qint64 required = choice.aggregateMinimum > 0 ? choice.aggregateMinimum : choice.min;
    return RuledGraveyardCostSelectionProgress{
        required,
        selected,
        selected >= required && (choice.aggregateMinimum > 0 || selected <= choice.max),
        choice.zone,
    };
}

template <typename PendingPayment>
[[nodiscard]] inline bool ruledPendingGraveyardCostSelectionContains(const PendingPayment &pending, quint32 objectId)
{
    if (objectId == 0 || !ruledPendingGraveyardCostSelectionProgress(pending).has_value()) {
        return false;
    }
    const auto &choice = pending.costChoices.at(pending.nextCostChoice);
    if (!choice.candidateIds.contains(objectId)) {
        return false;
    }
    return std::any_of(pending.costSelections.cbegin(), pending.costSelections.cend(),
                       [&choice, objectId](const auto &selection) {
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
    // The engine owns an accepted announcement even when its live ability/grant is no longer
    // published. Begin also survives the short gap before its authoritative reply arrives.
    if (pending.stage == PendingActivatedAbility::Stage::BeginPending)
        return true;
    if (pending.engineTransactionId != 0) {
        return state.pendingAbilityActivation &&
               state.pendingAbilityActivation->transaction_id() == pending.engineTransactionId &&
               state.pendingAbilityActivation->source_object_id() == pending.permanentOid &&
               state.pendingAbilityActivation->source_zone_change_generation() ==
                   pending.expectedZoneChangeGeneration &&
               state.pendingAbilityActivation->ability_index() == static_cast<quint32>(pending.abilityIndex);
    }
    if (pending.permanentAction) {
        return state
            .permanentActionFor(pending.permanentOid, pending.expectedZoneChangeGeneration, pending.permanentActionKind,
                                pending.permanentActionFaceIndex)
            .has_value();
    }
    return state.abilitySourceGeneration(pending.permanentOid) == pending.expectedZoneChangeGeneration &&
           state.activatedAbilityIndicesForOid(pending.permanentOid).contains(pending.abilityIndex);
}

struct PendingRuledSpellCast
{
    enum class Stage
    {
        Announcing,
        BeginPending,
        Paying,
        CommitPending,
        CancelPending,
    };

    bool hasConvoke = false;
    // Local announcement identity, preserved by nested-payment snapshots.
    quint64 draftId = 0;
    Stage stage = Stage::Announcing;
    quint64 engineTransactionId = 0;
    quint32 reservedObjectId = 0;
    bool resolutionTimeOffer = false;

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
        int linkedCastCostGroupIndex = -1;
        int linkedCastCostOptionIndex = -1;
    };

    int handIndex = -1;
    RuledCastSource source = RuledCastSource::Hand;
    ruled::v1::CastMethod castMethod = ruled::v1::CAST_METHOD_NORMAL;
    quint64 sourceZoneChangeGeneration = 0;
    quint64 castingPermissionId = 0;
    /// Mirrors the current spell or modal option; only ChooseAtCast collects allocations.
    ruled::v1::DamageDivision damageDivision = ruled::v1::DAMAGE_DIVISION_CHOOSE_AT_CAST;
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
    /// Every option owned by a mode is chosen only through that mode. This prevents the generic
    /// additional-cost picker from independently toggling a Spree cost.
    QSet<QPair<int, int>> modeLinkedCastCosts;
    QVector<QPair<int, int>> selectedModeLinkedCastCosts;
    bool submissionPending = false;
    QVector<SelectedMode> selectedModes;
    int activeModePosition = -1;
};

[[nodiscard]] inline int ruledCastCostGroupSelectionCount(const PendingRuledSpellCast &spell, int groupIndex)
{
    return static_cast<int>(
        std::count_if(spell.castCostSelections.cbegin(), spell.castCostSelections.cend(),
                      [groupIndex](const auto &selection) { return selection.groupIndex == groupIndex; }));
}

[[nodiscard]] inline bool
ruledCastCostOptionAlreadySelected(const PendingRuledSpellCast &spell, int groupIndex, int optionIndex)
{
    return std::any_of(spell.castCostSelections.cbegin(), spell.castCostSelections.cend(),
                       [groupIndex, optionIndex](const auto &selection) {
                           return selection.groupIndex == groupIndex && selection.optionIndex == optionIndex;
                       });
}

[[nodiscard]] inline bool ruledCastCostGroupCanConfirm(const PendingRuledSpellCast &spell,
                                                       const RuledCastCostGroup &group)
{
    const int selected = ruledCastCostGroupSelectionCount(spell, group.groupIndex);
    return selected >= group.min && selected <= group.max;
}

/// Choosing the sole allowed group entry is the declaration. Wider groups stay open so the player
/// can build the intended subset before confirming it.
[[nodiscard]] inline bool ruledCastCostGroupSelectionCompletesImmediately(const PendingRuledSpellCast &spell,
                                                                          const RuledCastCostGroup &group)
{
    return group.max == 1 && ruledCastCostGroupSelectionCount(spell, group.groupIndex) == 1 &&
           ruledCastCostGroupCanConfirm(spell, group);
}

/// A required exactly-one permanent cost object completes on the object click. Wider cohorts stay
/// open for explicit confirmation so the player can select the intended set.
[[nodiscard]] inline bool ruledCastCostObjectSelectionCompletesImmediately(const RuledCastCostOption &option,
                                                                           int selectedObjectCount,
                                                                           bool selectionCanConfirm)
{
    return option.objectMax == 1 && selectedObjectCount == 1 && selectionCanConfirm;
}

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
    if (option == group.options.cend() || !option->selectable || !ruledCastCostUsesObjectChoice(option->kind)) {
        return RuledTargetClickEligibility::Illegal;
    }
    const bool legal = kind == RuledCastCostCandidateKind::Hand
                           ? ruledCastCostUsesHandChoice(option->kind) && option->validHandIndices.contains(id)
                           : ruledCastCostUsesPermanentChoice(option->kind) && option->validPermanentIds.contains(id);
    return legal ? RuledTargetClickEligibility::Legal : RuledTargetClickEligibility::Illegal;
}

/// Cast-cost groups are declarations made before targeting and mana payment. Merely displaying
/// the current group's option buttons does not complete that declaration: finalizing mana there
/// would freeze the unreduced cost before Harmonize can record the selected creature's power.
[[nodiscard]] inline bool ruledCastCostGroupsComplete(const PendingRuledSpellCast &spell)
{
    return spell.valid && !spell.waitingForCastCostObject && spell.nextCastCostGroup >= spell.castCostGroups.size();
}

[[nodiscard]] inline bool
ruledTargetDataContains(const RuledTargetGroupData &data, RuledTargetCandidateKind kind, quint32 oid, int localPlayerId)
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
[[nodiscard]] inline ruled::v1::TargetRefKind
ruledTargetRefKind(const RuledTargetGroupData &data, quint32 oid, int localPlayerId)
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

[[nodiscard]] inline QVector<ruled::v1::TargetRef>
ruledSelectedTargetRefs(const RuledSpellTargetData &data,
                        const QVector<QVector<quint32>> &selectedByGroup,
                        int localPlayerId)
{
    QVector<ruled::v1::TargetRef> result;
    for (int position = 0; position < data.groups.size(); ++position) {
        const auto &group = data.groups.at(position);
        for (const auto oid : selectedByGroup.value(position)) {
            auto &target = result.emplaceBack();
            target.set_group_index(static_cast<quint32>(group.groupIndex));
            target.set_object_id(oid);
            target.set_kind(ruledTargetRefKind(group, oid, localPlayerId));
        }
    }
    return result;
}

[[nodiscard]] inline QVector<ruled::v1::TargetRef> ruledSelectedTargetRefs(const PendingActivatedAbility &ability)
{
    QVector<ruled::v1::TargetRef> result;
    for (const auto &target : ability.selectedTargets) {
        result.append(target.ref);
    }
    return result;
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

[[nodiscard]] inline int ruledModalSpellTargetedCostReduction(const PendingRuledSpellCast &spell, int localPlayerId)
{
    QHash<quint64, int> active;
    for (const auto &mode : spell.selectedModes) {
        ruledAccumulateTargetedCostReductions(mode.targets, mode.selectedTargetOidsByGroup, mode.selectedTargetOids,
                                              localPlayerId, active);
    }
    int total = 0;
    for (auto it = active.cbegin(); it != active.cend(); ++it) {
        total += it.value();
    }
    return total;
}

[[nodiscard]] inline int ruledTargetedCostReductionForSelection(const RuledSpellTargetData &data,
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

[[nodiscard]] inline std::optional<RuledSpellTargetData> currentRuledSpellTargetData(const PendingRuledSpellCast &spell,
                                                                                     const RuledClientState &state)
{
    if (!spell.valid) {
        return std::nullopt;
    }
    if (spell.activeModePosition >= 0 && spell.activeModePosition < spell.selectedModes.size()) {
        return state.modalSpellTargetData(spell.handIndex, spell.faceIndex,
                                          spell.selectedModes.at(spell.activeModePosition).modeIndex, spell.source,
                                          spell.castMethod, spell.castingPermissionId);
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

[[nodiscard]] inline QString ruledPendingSpellTargetPrompt(const PendingRuledSpellCast &spell,
                                                           const RuledClientState &state)
{
    const auto data = currentRuledSpellTargetData(spell, state);
    const auto group = currentRuledSpellTargetGroup(spell, state);
    QString sourceContext = spell.cardName;
    if (spell.activeModePosition >= 0 && spell.activeModePosition < spell.selectedModes.size()) {
        sourceContext = spell.selectedModes.at(spell.activeModePosition).label;
    }
    const RuledTargetGroupData fallbackGroup;
    return formatRuledTargetPrompt(sourceContext, group.value_or(fallbackGroup), spell.activeTargetGroupPosition,
                                   data.has_value() ? data->groups.size() : 0);
}

[[nodiscard]] inline QString ruledPendingAbilityTargetPrompt(const PendingActivatedAbility &ability,
                                                             const RuledClientState &state)
{
    const RuledSpellTargetData data = state.abilityTargetData(ability.permanentOid, ability.abilityIndex);
    const RuledTargetGroupData group = data.groups.isEmpty() ? static_cast<const RuledTargetGroupData &>(data)
                                                             : data.groups.value(ability.activeTargetGroupPosition);
    return formatRuledTargetPrompt(ability.abilityText, group, ability.activeTargetGroupPosition, data.groups.size());
}

[[nodiscard]] inline int ruledTargetSelectionDisplayMaximum(const RuledTargetGroupData &group)
{
    return ruledTargetGroupUsesExplicitConfirmation(group) ? group.maxTargets : -1;
}

/// One authoritative click predicate for every true target-selection flow. Untargeted resolution
/// and cost choices deliberately stay out of this function.
[[nodiscard]] inline RuledTargetClickEligibility ruledTargetClickEligibility(const PendingRuledSpellCast &spell,
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
    if (state.hasPendingChoiceOfKind(RuledClientState::ChoiceKind::PermanentChoice)) {
        return kind == RuledTargetCandidateKind::Battlefield &&
                       state.isPendingChoiceCandidate(RuledClientState::ChoiceKind::PermanentChoice, oid)
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
    if (state.hasPendingChoiceOfKind(RuledClientState::ChoiceKind::BattleProtector)) {
        return kind == RuledTargetCandidateKind::Player &&
                       state.isPendingChoiceCandidate(RuledClientState::ChoiceKind::BattleProtector, oid)
                   ? RuledTargetClickEligibility::Legal
                   : RuledTargetClickEligibility::Illegal;
    }
    if (state.hasPendingChoiceOfKind(RuledClientState::ChoiceKind::AttackingTokenDefender)) {
        const bool legal = kind == RuledTargetCandidateKind::Player
                               ? state.isLegalAttackPlayerDefender(static_cast<int>(oid))
                           : kind == RuledTargetCandidateKind::Battlefield ? state.isLegalAttackPermanentDefender(oid)
                                                                           : false;
        return legal ? RuledTargetClickEligibility::Legal : RuledTargetClickEligibility::Illegal;
    }
    if (state.hasPendingTriggerTarget()) {
        const auto refKind = kind == RuledTargetCandidateKind::Graveyard ? ruled::v1::TARGET_REF_KIND_GRAVEYARD
                             : kind == RuledTargetCandidateKind::Stack   ? ruled::v1::TARGET_REF_KIND_STACK
                             : kind == RuledTargetCandidateKind::Player  ? ruled::v1::TARGET_REF_KIND_PLAYER
                                                                         : ruled::v1::TARGET_REF_KIND_PERMANENT;
        const int targetPlayerId = kind == RuledTargetCandidateKind::Player ? static_cast<int>(oid) : localPlayerId;
        return state.isPendingTriggerTargetCandidate(refKind, oid, targetPlayerId)
                   ? RuledTargetClickEligibility::Legal
                   : RuledTargetClickEligibility::Illegal;
    }
    if (state.pendingAbilityActivation && !state.pendingChoice && !state.isWaitingForChoice()) {
        const auto &pending = *state.pendingAbilityActivation;
        if (pending.stage() == ruled::v1::ABILITY_ACTIVATION_STAGE_OPPONENT_TARGET &&
            pending.deciding_player_id() == localPlayerId) {
            const bool legal = kind == RuledTargetCandidateKind::Battlefield &&
                               std::any_of(pending.target_candidates().begin(), pending.target_candidates().end(),
                                           [oid](const auto &target) { return target.object_id() == oid; });
            return legal ? RuledTargetClickEligibility::Legal : RuledTargetClickEligibility::Illegal;
        }
    }
    if (ability.valid && ability.waitingForTarget) {
        const auto data = state.abilityTargetData(ability.permanentOid, ability.abilityIndex);
        const auto group = data.groups.isEmpty() ? static_cast<const RuledTargetGroupData &>(data)
                                                 : data.groups.value(ability.activeTargetGroupPosition);
        if (group.hasXTargetChoices) {
            const auto candidateGeneration = ruledXTargetCandidateGeneration(group, ability.xValue, oid);
            if (kind != RuledTargetCandidateKind::Battlefield || !candidateGeneration ||
                *candidateGeneration != state.battlefieldGenerationByOid.value(oid, 0)) {
                return RuledTargetClickEligibility::Illegal;
            }
        }
        return !group.chosenByOpponent && ruledTargetDataContains(group, kind, oid, localPlayerId) &&
                       ruledTargetPairCompatible(group, ruledTargetRefKind(group, oid, localPlayerId), oid,
                                                   ruledSelectedTargetRefs(ability))
                   ? RuledTargetClickEligibility::Legal
                   : RuledTargetClickEligibility::Illegal;
    }
    if (spell.valid && spell.waitingForTarget) {
        const auto data = currentRuledSpellTargetGroup(spell, state);
        const auto allGroups = currentRuledSpellTargetData(spell, state);
        return data.has_value() && allGroups.has_value() && ruledTargetDataContains(*data, kind, oid, localPlayerId) &&
                       ruledTargetPairCompatible(*data, ruledTargetRefKind(*data, oid, localPlayerId), oid,
                                                  ruledSelectedTargetRefs(*allGroups, spell.selectedTargetOidsByGroup,
                                                                           localPlayerId))
                   ? RuledTargetClickEligibility::Legal
                   : RuledTargetClickEligibility::Illegal;
    }
    return RuledTargetClickEligibility::NotTargeting;
}

[[nodiscard]] inline bool ruledTargetDataContainsOid(const RuledTargetGroupData &data, quint32 oid, int localPlayerId)
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
    const auto prune = [&](QVector<quint32> &oids, QVector<quint32> &damages, QVector<int> *allocations,
                           const RuledTargetGroupData &data, const QVector<ruled::v1::TargetRef> &selected) {
        for (int i = oids.size() - 1; i >= 0; --i) {
            if (ruledTargetDataContainsOid(data, oids.at(i), localPlayerId) &&
                ruledTargetPairCompatible(data, ruledTargetRefKind(data, oids.at(i), localPlayerId), oids.at(i), selected)) {
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
                QVector<int> *const allocations =
                    groupIndex == spell.activeTargetGroupPosition ? &spell.targetDamageAllocations : nullptr;
                prune(spell.selectedTargetOidsByGroup[groupIndex], spell.selectedTargetDamagesByGroup[groupIndex],
                      allocations, data.groups.at(groupIndex),
                      ruledSelectedTargetRefs(data, spell.selectedTargetOidsByGroup, localPlayerId));
            }
            if (spell.activeTargetGroupPosition >= 0 &&
                spell.activeTargetGroupPosition < spell.selectedTargetOidsByGroup.size()) {
                spell.selectedTargetOids = spell.selectedTargetOidsByGroup.at(spell.activeTargetGroupPosition);
                spell.selectedTargetDamages = spell.selectedTargetDamagesByGroup.at(spell.activeTargetGroupPosition);
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
                                                             spell.source, spell.castMethod, spell.castingPermissionId);
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
                    mode.selectedTargetDamagesByGroup[spell.activeTargetGroupPosition] = spell.selectedTargetDamages;
                }
                for (int groupIndex = 0; groupIndex < data->groups.size(); ++groupIndex) {
                    prune(mode.selectedTargetOidsByGroup[groupIndex], mode.selectedTargetDamagesByGroup[groupIndex],
                          nullptr, data->groups.at(groupIndex),
                          ruledSelectedTargetRefs(*data, mode.selectedTargetOidsByGroup, localPlayerId));
                    if (modePosition == spell.activeModePosition && groupIndex == spell.activeTargetGroupPosition) {
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
    if (ability.valid && ability.engineTransactionId == 0 &&
        ability.stage == PendingActivatedAbility::Stage::Announcing && !ability.selectedTargets.isEmpty()) {
        const auto data = state.abilityTargetData(ability.permanentOid, ability.abilityIndex);
        for (int index = ability.selectedTargets.size() - 1; index >= 0; --index) {
            const auto &target = ability.selectedTargets.at(index);
            const auto group = std::find_if(data.groups.cbegin(), data.groups.cend(), [&target](const auto &entry) {
                return entry.groupIndex == static_cast<int>(target.ref.group_index());
            });
            const auto &candidates =
                group == data.groups.cend() ? static_cast<const RuledTargetGroupData &>(data) : *group;
            const auto xTargetGeneration =
                candidates.hasXTargetChoices
                    ? ruledXTargetCandidateGeneration(candidates, ability.xValue, target.ref.object_id())
                    : std::optional<quint64>{};
            const bool xTargetIncarnationMatches =
                !candidates.hasXTargetChoices ||
                (xTargetGeneration && target.ref.has_expected_zone_change_generation() &&
                 target.ref.expected_zone_change_generation() == *xTargetGeneration &&
                 target.zoneChangeGeneration == *xTargetGeneration &&
                 state.battlefieldGenerationByOid.value(target.ref.object_id(), 0) == *xTargetGeneration);
            if (candidates.hasXTargetChoices && !xTargetIncarnationMatches && ability.targetingCostApplied) {
                ability.restartAfterTargetInvalidation = true;
            }
            if (!ruledTargetDataContainsOid(candidates, target.ref.object_id(), localPlayerId) ||
                !xTargetIncarnationMatches ||
                !ruledTargetPairCompatible(candidates, target.ref.kind(), target.ref.object_id(),
                                           ruledSelectedTargetRefs(ability))) {
                const int position =
                    group == data.groups.cend() ? 0 : static_cast<int>(std::distance(data.groups.cbegin(), group));
                ability.selectedTargets.resize(index);
                ability.activeTargetGroupPosition = position;
                ability.waitingForTarget = true;
                ability.waitingForMana = false;
                changed = true;
            }
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
        (!ability.chosenOpponentTargets || ability.stage == PendingActivatedAbility::Stage::Paying) &&
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
    bool isAwaitingRuledCastCostOption() const;
    bool pendingRuledCastCostGroupIsOptional() const;
    bool declineCastCostGroup();
    QString pendingRuledCastCostSkipLabel() const;
    QVector<RuledCastCostOption> pendingRuledCastCostOptions() const;
    int pendingRuledCastCostSelectedCount() const;
    int pendingRuledCastCostMinimum() const;
    int pendingRuledCastCostMaximum() const;
    bool pendingRuledCastCostObjectCanConfirm() const;
    bool pendingRuledCastCostObjectUsesExplicitConfirmation() const;
    bool isAwaitingRuledSpellCostSelection() const;
    bool isAwaitingRuledCastCostObject() const;
    bool isAwaitingRuledAbilityCostSelection() const;
    QString pendingRuledAbilityCostPromptText() const;
    bool isAwaitingRuledGraveyardCostSelection() const;
    bool isRuledGraveyardCostObjectSelected(quint32 objectId) const;
    bool getRuledGraveyardCostSelectionProgress(int &required, int &selected) const;
    QString pendingRuledSpellPromptText() const;
    QString pendingRuledAbilityPromptText() const;

    // Headless cost staging and display helpers; engine choices remain authoritative.
    static QMap<QChar, int> parseSimpleManaCost(const QString &manaCost);
    static std::optional<QMap<QChar, int>> parseRepresentableManaCost(const QString &manaCost);
    static std::optional<int> repeatedCastCostAmount(const RuledCastCostOption &option, quint32 count);
    bool stageRepeatedCastCost(int optionIndex, quint32 count);
    bool repeatedCastCostPromptStillCurrent(const PendingRuledSpellCast &before, int optionIndex) const;
    bool expandRepeatedCastX(int chosenX);
    bool finalizeRepeatedCastManaCost(qint64 increase, qint64 reduction);
    static QString formatSimpleManaCost(const QMap<QChar, int> &cost);
    static QVector<RuledFlexPip> parseFlexPips(const QString &manaCost);
    static bool flexPipMatchesColor(const RuledFlexPip &pip, QChar color);
    static void applyFlexChoicesToCost(QMap<QChar, int> &fixed,
                                       QVector<quint32> &lifePipIndices,
                                       QVector<RuledFlexPip> &flex,
                                       const QVector<bool> &choiceIsAlternative);
    static bool applyManaPipToFlexibleCost(QMap<QChar, int> &fixed,
                                           QVector<RuledFlexPip> &flex,
                                           bool colorlessMana,
                                           QChar coloredMana);
    static QString formatRemainingCost(const QMap<QChar, int> &fixed, const QVector<RuledFlexPip> &flex);
    static int totalRemainingForCost(const QMap<QChar, int> &fixed, const QVector<RuledFlexPip> &flex);
    bool reconcileSpellCosts(const RuledClientState &state, int localPlayerId);
    bool reconcileAbilityCosts(const RuledClientState &state, int localPlayerId);

    /// Collect only engine-authored counter choices. Cancellation never submits payment.
    static bool chooseCounterCosts(QWidget *parent, PendingActivatedAbility &pending);

    /// Shared left/right-click modal picker. Choose-one spells use ordinary menu actions;
    /// choose-N spells use persistent checkboxes plus explicit confirm/cancel controls.
    static std::optional<QVector<int>> chooseModes(QWidget *parent,
                                                   const QString &cardName,
                                                   const QVector<RuledModalSpellOption> &modes,
                                                   int minModes,
                                                   int maxModes);

    /// Pick one engine-authoritative castable face. The physical CardItem may expose only its
    /// front display name (Adventure), so menu entries come exclusively from `faces`.
    /// Keep previews as cast proposals; bind only final submissions to the resolving offer.
    static ruled::v1::RuledCommand submissionCommand(const ruled::v1::RuledCommand &command);

    static std::optional<RuledFaceOption>
    chooseFace(QWidget *parent, const QString &cardName, const QVector<RuledFaceOption> &faces);

    /// Build one engine-authoritative menu model for alternate actions on a physical card.
    /// Castable faces precede zone abilities so a cycler in hand exposes both Cast and Cycle.
    static QVector<RuledCardActionMenuOption>
    cardActionMenuOptions(const QVector<RuledFaceOption> &castFaces,
                          const RuledClientState &state,
                          quint32 sourceOid,
                          bool manaAbilitiesOnly = false,
                          const QVector<QPair<int, QString>> &paymentContributions = {});

    /// Spell casts and activated abilities are mutually exclusive local UI transactions.
    /// A parked cast offer owns the exact source, generation, face, method, and permission.
    [[nodiscard]] static bool matchesSpecialCastOffer(const PendingRuledSpellCast &spell,
                                                      const RuledClientState &state);
    [[nodiscard]] bool resolutionChoiceBlocksSpell(const RuledClientState &state) const;
    PendingRuledSpellCast &beginSpell();
    PendingActivatedAbility &beginAbility();
    void clearSpell();
    void clearAbility();
    [[nodiscard]] InteractionKind activeInteraction() const;

    // Local target staging only; authoritative candidates remain in RuledClientState.
    enum class DamageAllocationStep
    {
        Ready,
        Invalid,
        Allocating
    };
    enum class DamageAllocationChange
    {
        Unavailable,
        Unchanged,
        Changed
    };
    DamageAllocationStep prepareSpellDamageAllocation();
    DamageAllocationChange bumpSpellDamageAllocation(quint32 oid, int delta);
    bool confirmSpellDamageAllocation();
    int pendingDamageTargetsTotal() const;
    int effectiveDamageTargetsMax() const;
    bool isInSpellDamageAllocationMode() const;
    bool isSpellDamageAllocationDisplayActive() const;
    int spellDamageAllocationForOid(quint32 oid) const;
    int spellDamageAllocationAssignedTotal() const;
    int spellDamageAllocationMaxTotal() const;
    bool spellDamageAllocationIsLegal() const;
    bool isTargetSelectedForPendingSpell(quint32 oid) const;
    bool isCastCostPermanentSelected(quint32 oid) const;
    void loadCurrentTargetGroup(const RuledClientState &state);
    bool storeCurrentTargetGroupAndAdvance(const RuledClientState &state);
    bool storeCurrentModalTargetsAndAdvance(const RuledClientState &state);

    PendingRuledSpellCast spell;
    PendingActivatedAbility ability;

private:
    quint64 nextSpellDraftId = 0;
};

/// Fork-owned bridge between PlayerActions' pending state and concrete CardItem/Player target
/// surfaces. PlayerActions exposes only thin wrappers and one friend declaration.
class RuledTargetUi
{
public:
    static bool tryHandleRuledSpellTargetClick(PlayerActions *actions, CardItem *card);
    static bool isPlayerSelectedAsPendingSpellTarget(const PlayerActions *actions, int playerId);
    static void confirmMultiTargetSelection(PlayerActions *actions);
    static bool isAwaitingRuledPlayerTargetSelection(const PlayerActions *actions);
    static bool isAwaitingRuledAbilityOrTriggerPlayerTarget(const PlayerActions *actions);
    static bool tryHandleRuledSpellTargetPlayerClick(PlayerActions *actions, Player *targetPlayer);
    static void loadCurrentTargetGroup(PlayerActions *actions);
    static bool storeCurrentTargetGroupAndAdvance(PlayerActions *actions);
    static bool storeCurrentModalTargetsAndAdvance(PlayerActions *actions);
    static bool finalizeTargetSelectionAndContinue(PlayerActions *actions);
    static int spellDamageAllocationForPlayerId(const PlayerActions *actions, int playerId);
    static bool tryBumpSpellDamageAllocationForOid(PlayerActions *actions, quint32 oid, int delta);
    static bool tryBumpSpellDamageAllocationForCard(PlayerActions *actions, CardItem *card, int delta);
    static bool tryBumpSpellDamageAllocationForPlayer(PlayerActions *actions, Player *targetPlayer, int delta);
    static void confirmSpellDamageAllocation(PlayerActions *actions);
    static bool tryHandleRuledAbilityTargetClick(PlayerActions *actions, CardItem *card);
    static bool tryHandleRuledAbilityTargetPlayerClick(PlayerActions *actions, Player *targetPlayer);
    static void ensureRefreshConnection(PlayerActions *actions);
    static void reconcile(PlayerActions *actions);
    [[nodiscard]] static RuledTargetClickEligibility cardEligibility(const PlayerActions *actions, CardItem *card);
    [[nodiscard]] static RuledTargetClickEligibility playerEligibility(const PlayerActions *actions, Player *target);
};

#endif // COCKATRICE_RULED_PENDING_CAST_H
