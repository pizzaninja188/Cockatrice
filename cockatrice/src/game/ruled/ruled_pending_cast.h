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
    bool valid = false;
    quint32 permanentOid = 0;
    int abilityIndex = -1;
    QString abilityText;
    QString cardName;
    bool needsTarget = false;
    bool waitingForTarget = false;
    quint32 selectedTargetOid = 0;
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

/// Identify a locally staged spell or ability whose last mana pip was consumed while another
/// engine command (normally that mana ability) was still in flight. The caller invokes this after
/// the command lock clears and submits the returned action.
[[nodiscard]] inline RuledPendingPaymentAction
readyRuledPendingPaymentAction(const PendingRuledSpellCast &spell, const PendingActivatedAbility &ability)
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
    if (ability.valid && !ability.waitingForTarget && costIsPaid(ability.remainingCost, ability.flexPips)) {
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

#endif // COCKATRICE_RULED_PENDING_CAST_H
