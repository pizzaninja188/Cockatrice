#ifndef COCKATRICE_RULED_ZONE_VIEW_POLICY_H
#define COCKATRICE_RULED_ZONE_VIEW_POLICY_H

#include <QString>
#include <libcockatrice/utility/zone_names.h>

inline bool ruledPlayerNeedsCustomZoneMenu(bool localOrJudge, bool ruledMode)
{
    return localOrJudge || ruledMode;
}

inline bool ruledCustomZoneViewAllowed(bool localOrJudge, bool ruledMode, const QString &zoneName)
{
    return localOrJudge || (ruledMode && zoneName == QLatin1String(ZoneNames::COMMAND));
}

inline bool ruledZoneViewHasAuthoritativeCardList(bool revealZone, bool writeableRevealZone)
{
    return revealZone && !writeableRevealZone;
}

#endif // COCKATRICE_RULED_ZONE_VIEW_POLICY_H
