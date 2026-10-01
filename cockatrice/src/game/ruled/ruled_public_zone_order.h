#ifndef COCKATRICE_RULED_PUBLIC_ZONE_ORDER_H
#define COCKATRICE_RULED_PUBLIC_ZONE_ORDER_H

class CardZoneLogic;
class ServerInfo_Zone;

class RuledPublicZoneOrder
{
public:
    // Called only from the ruled snapshot-preservation branch.
    static void apply(CardZoneLogic *zone, const ServerInfo_Zone &info);
};

#endif
