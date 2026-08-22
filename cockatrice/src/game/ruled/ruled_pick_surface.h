#ifndef COCKATRICE_RULED_PICK_SURFACE_H
#define COCKATRICE_RULED_PICK_SURFACE_H

/// Pure description of where a tier-3 card choice is rendered. Kept independent of CardItem and
/// Player so the click-routing contract can be covered by the headless ruled-client tests.
enum class RuledPickZone
{
    Hand,
    Deck,
    Revealed
};

enum class RuledPickScaffoldZone
{
    Hand,
    Deck,
    Other
};

/// Whether a card surface belongs to the active pick. Public hand reveals deliberately accept a
/// nonlocal HAND zone view: the revealed hand owner is the widget scaffold, while chooser
/// authority remains in RuledClientState and the engine-authored selectable mask.
[[nodiscard]] constexpr bool isRuledPickSurfaceCard(RuledPickZone pickZone,
                                                     RuledPickScaffoldZone scaffoldZone,
                                                     bool isZoneView,
                                                     bool zoneIsLocal)
{
    switch (pickZone) {
        case RuledPickZone::Hand:
            return scaffoldZone == RuledPickScaffoldZone::Hand && zoneIsLocal;
        case RuledPickZone::Deck:
            return isZoneView && scaffoldZone == RuledPickScaffoldZone::Deck && zoneIsLocal;
        case RuledPickZone::Revealed:
            return isZoneView &&
                   (scaffoldZone == RuledPickScaffoldZone::Deck || scaffoldZone == RuledPickScaffoldZone::Hand);
    }
    return false;
}

#endif // COCKATRICE_RULED_PICK_SURFACE_H
