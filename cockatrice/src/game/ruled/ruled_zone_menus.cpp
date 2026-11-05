#include "ruled_zone_menus.h"

#include "../../client/settings/cache_settings.h"
#include "../../client/settings/shortcuts_settings.h"
#include "../../game_graphics/player/menu/abstract_player_component.h"
#include "../../game_graphics/player/menu/custom_zone_menu.h"
#include "../../game_graphics/player/menu/grave_menu.h"
#include "../../game_graphics/player/menu/hand_menu.h"
#include "../../game_graphics/player/menu/library_menu.h"
#include "../../game_graphics/player/menu/player_menu.h"
#include "../../game_graphics/player/menu/rfg_menu.h"
#include "../../game_graphics/player/menu/sideboard_menu.h"
#include "../../game_graphics/player/player_graphics_item.h"
#include "../../game_graphics/zones/hand_zone.h"
#include "../../game_graphics/zones/pile_zone.h"
#include "../../game_graphics/zones/table_zone.h"
#include "ruled_actions.h"

#include <QAction>
#include <QList>
#include <QMenu>
#include <libcockatrice/game/player/player_logic.h>

namespace
{
void keepInspectionActions(QMenu *menu, const QList<QAction *> &allowed)
{
    if (!menu) {
        return;
    }
    // Do not delete actions: the retained upstream components still own and translate them.
    for (auto *action : menu->actions()) {
        menu->removeAction(action);
    }
    for (auto *action : allowed) {
        if (!action) {
            continue;
        }
        action->setProperty("ruledZoneInspection", true);
        menu->addAction(action);
    }
}
} // namespace

bool RuledZoneMenus::isRuled(const PlayerMenu &menus)
{
    return RuledActions::isRuledGame(menus.player->getLogic()->getGame());
}

void RuledZoneMenus::configure(PlayerMenu &menus)
{
    if (!isRuled(menus)) {
        return;
    }
    keepInspectionActions(menus.handMenu, {});
    keepInspectionActions(menus.libraryMenu, {});
    keepInspectionActions(menus.graveMenu, {menus.graveMenu->aViewGraveyard});
    keepInspectionActions(menus.rfgMenu, {menus.rfgMenu->aViewRfg});
    if (menus.sideboardMenu) {
        // Upstream SideboardMenu contains only View sideboard.
        keepInspectionActions(menus.sideboardMenu, menus.sideboardMenu->actions());
    }
    keepInspectionActions(menus.playerMenu, {menus.graveMenu->aViewGraveyard, menus.rfgMenu->aViewRfg,
                                             menus.sideboardMenu ? menus.sideboardMenu->menuAction() : nullptr,
                                             menus.customZonesMenu ? menus.customZonesMenu->menuAction() : nullptr});
}

bool RuledZoneMenus::bind(PlayerMenu &menus)
{
    if (!isRuled(menus)) {
        return false;
    }
    auto *player = menus.player;
    player->getTableZoneGraphicsItem()->setMenu(nullptr);
    player->getHandZoneGraphicsItem()->setMenu(nullptr);
    // Clear the default double-click draw action along with the library popup.
    player->getDeckZoneGraphicsItem()->setMenu(nullptr);
    player->getGraveyardZoneGraphicsItem()->setMenu(menus.graveMenu, menus.graveMenu->aViewGraveyard);
    player->getRfgZoneGraphicsItem()->setMenu(menus.rfgMenu, menus.rfgMenu->aViewRfg);
    auto *sideboard = menus.sideboardMenu;
    player->getSideboardZoneGraphicsItem()->setMenu(
        sideboard, sideboard && !sideboard->actions().isEmpty() ? sideboard->actions().first() : nullptr);
    return true;
}

bool RuledZoneMenus::activateShortcuts(PlayerMenu &menus)
{
    if (!isRuled(menus)) {
        return false;
    }
    for (auto *component : menus.managedComponents) {
        component->setShortcutsInactive();
    }
    // Preserve only read-only zone shortcuts; counter/mana shortcuts retain their existing signal path.
    menus.graveMenu->aViewGraveyard->setShortcuts(
        SettingsCache::instance().shortcuts().getShortcut("Player/aViewGraveyard"));
    if (menus.sideboardMenu) {
        menus.sideboardMenu->setShortcutsActive();
    }
    return true;
}
