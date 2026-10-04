#ifndef COCKATRICE_RULED_CARD_DISPLAY_IDENTITY_H
#define COCKATRICE_RULED_CARD_DISPLAY_IDENTITY_H

#include "ruled_client_state.h"
#include <QObject>
#include <QVariant>

/// Popup IDs are local to their view, even after its pending choice has finished.
inline void markRuledChoiceLocalIds(QObject &zone)
{
    zone.setProperty("ruledChoiceLocalIds", true);
}

[[nodiscard]] inline bool ruledCardSurfaceHasPhysicalIdentity(const QObject *zone)
{
    return !zone || (!zone->property("ruledChoiceLocalIds").toBool() &&
                     !zone->property("ruledReplacementPicker").toBool() &&
                     !zone->property("ruledRevealSnapshot").toBool());
}

[[nodiscard]] inline quint32 ruledPhysicalDisplayOid(const RuledClientState &state,
                                                    int ownerPlayerId,
                                                    int cardId,
                                                    const QObject *zone)
{
    return ruledCardSurfaceHasPhysicalIdentity(zone) ? state.engineOidForCardId(ownerPlayerId, cardId) : 0;
}

#endif // COCKATRICE_RULED_CARD_DISPLAY_IDENTITY_H
