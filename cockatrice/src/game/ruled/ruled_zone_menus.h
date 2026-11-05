#ifndef RULED_ZONE_MENUS_H
#define RULED_ZONE_MENUS_H

class PlayerMenu;

// Keep the upstream menu objects intact, but expose only inspection in ruled games.
struct RuledZoneMenus
{
    static void configure(PlayerMenu &menus);
    static bool bind(PlayerMenu &menus);
    static bool activateShortcuts(PlayerMenu &menus);

private:
    static bool isRuled(const PlayerMenu &menus);
};

#endif
