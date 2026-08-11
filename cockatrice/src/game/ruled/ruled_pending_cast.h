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
#include <QMap>
#include <QString>
#include <QVector>
#include <QtGlobal>
#include <optional>

class QWidget;
class CardItem;
class Player;
class PlayerActions;

struct RuledFlexPip
{
    quint32 pipIndex = 0;
    QChar colorA;
    QChar colorB;
    int generic = 0;
    bool phyrexian = false;
    int genericPaid = 0;
};

struct PendingActivatedAbility
{
    struct CostSelection
    {
        int costIndex = -1;
        RuledAbilityCostChoiceZone zone = RuledAbilityCostChoiceZone::Battlefield;
        quint32 selectedId = 0;
    };

    bool valid = false;
    quint32 permanentOid = 0;
    int abilityIndex = -1;
    QString abilityText;
    QString cardName;
    bool needsTarget = false;
    bool waitingForTarget = false;
    quint32 selectedTargetOid = 0;
    bool waitingForCost = false;
    QVector<RuledAbilityCostChoice> costChoices;
    int nextCostChoice = 0;
    QVector<CostSelection> costSelections;
    bool waitingForMana = false;
    QMap<QChar, int> remainingCost;
    QVector<RuledFlexPip> flexPips;
    QVector<quint32> lifePipIndices;
};

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
    };

    int handIndex = -1;
    RuledCastSource source = RuledCastSource::Hand;
    /// Fireball's "divided evenly, rounded down": the engine splits on resolution, so there is no
    /// allocation to collect, no one-damage-per-target cap, and zero targets is a legal cast.
    bool damageDividedEvenly = false;
    int faceIndex = 0;
    QString cardName;
    QMap<QChar, int> remainingCost;
    QVector<quint32> selectedTargetOids;
    QVector<quint32> selectedTargetDamages;
    bool waitingForTarget = false;
    bool valid = false;
    int maxTargets = 0;
    int fixedDamage = 0;
    bool isDamageTargets = false;
    int extraManaPerTarget = 0;
    bool inDamageAllocationMode = false;
    int damageAllocationTotal = 0;
    QVector<int> targetDamageAllocations;
    int xPips = 0;
    int xValue = 0;
    QVector<RuledFlexPip> flexPips;
    QVector<quint32> lifePipIndices;
    QVector<SelectedMode> selectedModes;
    int activeModePosition = -1;
};

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

[[nodiscard]] inline bool ruledTargetDataContains(const RuledSpellTargetData &data,
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

[[nodiscard]] inline std::optional<RuledSpellTargetData>
currentRuledSpellTargetData(const PendingRuledSpellCast &spell, const RuledClientState &state)
{
    if (!spell.valid) {
        return std::nullopt;
    }
    if (spell.activeModePosition >= 0 && spell.activeModePosition < spell.selectedModes.size()) {
        return state.modalSpellTargetData(spell.handIndex, spell.faceIndex,
                                          spell.selectedModes.at(spell.activeModePosition).modeIndex, spell.source);
    }
    return state.spellTargetData(spell.handIndex, spell.faceIndex, spell.source);
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
    if (state.hasPendingTriggerTarget()) {
        const auto data = state.abilityTargetData(state.lastTriggerSourceOid,
                                                  static_cast<int>(state.lastTriggerAbilityIndex));
        return ruledTargetDataContains(data, kind, oid, localPlayerId) ? RuledTargetClickEligibility::Legal
                                                                       : RuledTargetClickEligibility::Illegal;
    }
    if (ability.valid && ability.waitingForTarget) {
        const auto data = state.abilityTargetData(ability.permanentOid, ability.abilityIndex);
        return ruledTargetDataContains(data, kind, oid, localPlayerId) ? RuledTargetClickEligibility::Legal
                                                                       : RuledTargetClickEligibility::Illegal;
    }
    if (spell.valid && spell.waitingForTarget) {
        const auto data = currentRuledSpellTargetData(spell, state);
        return data.has_value() && ruledTargetDataContains(*data, kind, oid, localPlayerId)
                   ? RuledTargetClickEligibility::Legal
                   : RuledTargetClickEligibility::Illegal;
    }
    return RuledTargetClickEligibility::NotTargeting;
}

[[nodiscard]] inline bool ruledTargetDataContainsOid(const RuledSpellTargetData &data,
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
                           const RuledSpellTargetData &data) {
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
            prune(spell.selectedTargetOids, spell.selectedTargetDamages, &spell.targetDamageAllocations, data);
        } else {
            for (int modePosition = 0; modePosition < spell.selectedModes.size(); ++modePosition) {
                auto &mode = spell.selectedModes[modePosition];
                const auto data = state.modalSpellTargetData(spell.handIndex, spell.faceIndex, mode.modeIndex,
                                                             spell.source);
                const RuledSpellTargetData empty;
                prune(mode.selectedTargetOids, mode.selectedTargetDamages, nullptr,
                      data.has_value() ? *data : empty);
                if (modePosition == spell.activeModePosition) {
                    prune(spell.selectedTargetOids, spell.selectedTargetDamages,
                          &spell.targetDamageAllocations, data.has_value() ? *data : empty);
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
    if (spell.valid && !spell.waitingForTarget && !spell.inDamageAllocationMode &&
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
