#ifndef RULED_COST_SELECTION_H
#define RULED_COST_SELECTION_H

#include "ruled_client_state.h"

#include <QCoreApplication>

/// Wire shape and presentation only. Rust publishes candidates and decides payment legality.
inline bool ruledCostUsesObjectRefs(RuledCostChoiceKind kind)
{
    return kind == RuledCostChoiceKind::Tap || kind == RuledCostChoiceKind::Blight;
}

inline bool ruledCostUsesObjectRefs(const RuledCostChoice &choice)
{
    return choice.zone == RuledCostChoiceZone::Graveyard || ruledCostUsesObjectRefs(choice.kind);
}

inline bool ruledCostNeedsConfirmation(const RuledCostChoice &choice)
{
    return ruledCostUsesObjectRefs(choice);
}

inline bool ruledCastCostUsesPermanent(RuledCastCostOptionKind kind)
{
    return kind == RuledCastCostOptionKind::Behold ||
           kind == RuledCastCostOptionKind::TapPermanentForGenericReduction || kind == RuledCastCostOptionKind::Blight;
}

inline QString ruledCostSelectionPrompt(const RuledCostChoice &choice, const QString &name)
{
    const auto tr = [](const char *text) { return QCoreApplication::translate("RuledCostSelection", text); };
    if (choice.zone == RuledCostChoiceZone::Hand)
        return tr("Choose a card to discard for %1.").arg(name);
    if (choice.zone == RuledCostChoiceZone::Graveyard)
        if (choice.aggregateMinimum > 0)
            return tr("Choose graveyard cards with total mana value %1 or greater for %2.")
                .arg(choice.aggregateMinimum)
                .arg(name);
    if (choice.zone == RuledCostChoiceZone::Graveyard)
        return tr("Choose %1 card(s) from your graveyard for %2.").arg(choice.min).arg(name);
    if (choice.kind == RuledCostChoiceKind::Blight)
        return tr("Blight %1: choose one creature you control for %2.").arg(choice.blightCount).arg(name);
    if (choice.kind == RuledCostChoiceKind::Tap)
        if (choice.aggregateMinimum > 0)
            return tr("Choose untapped permanents with total power %1 or greater to tap for %2.")
                .arg(choice.aggregateMinimum)
                .arg(name);
    if (choice.kind == RuledCostChoiceKind::Tap)
        return tr("Choose %1 untapped permanent(s) to tap for %2.").arg(choice.max).arg(name);
    return tr("Choose a permanent to sacrifice for %1.").arg(name);
}

inline QString ruledCastCostSelectionPrompt(const RuledCastCostOption &option)
{
    const auto tr = [](const char *text) { return QCoreApplication::translate("RuledCostSelection", text); };
    if (option.kind == RuledCastCostOptionKind::Blight)
        return tr("%1: click one creature you control.").arg(option.label);
    if (option.kind == RuledCastCostOptionKind::TapPermanentForGenericReduction)
        return tr("%1: click an untapped creature you control.").arg(option.label);
    return tr("%1: click an authorized card in your hand or permanent you control.").arg(option.label);
}

#endif
