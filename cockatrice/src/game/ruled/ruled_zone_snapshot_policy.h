#ifndef COCKATRICE_RULED_ZONE_SNAPSHOT_POLICY_H
#define COCKATRICE_RULED_ZONE_SNAPSHOT_POLICY_H

#include <QString>
#include <libcockatrice/utility/zone_names.h>

inline bool ruledSnapshotPreservesEventAuthoritativeZone(const QString &zoneName)
{
    return zoneName == QLatin1String(ZoneNames::STACK) || zoneName == QLatin1String(ZoneNames::GRAVE) ||
           zoneName == QLatin1String(ZoneNames::EXILE);
}

#endif // COCKATRICE_RULED_ZONE_SNAPSHOT_POLICY_H
