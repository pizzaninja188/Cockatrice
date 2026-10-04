#include "custom_zone_menu.h"

#include "../player.h"
#include "../../ruled/ruled_actions.h"
#include "../../ruled/ruled_zone_view_policy.h"

CustomZoneMenu::CustomZoneMenu(Player *_player) : player(_player)
{
    menuAction()->setVisible(false);

    connect(player, &Player::clearCustomZonesMenu, this, &CustomZoneMenu::clearCustomZonesMenu);
    connect(player, &Player::addViewCustomZoneActionToCustomZoneMenu, this,
            &CustomZoneMenu::addViewCustomZoneActionToCustomZoneMenu);

    retranslateUi();
}

void CustomZoneMenu::retranslateUi()
{
    setTitle(tr("C&ustom Zones"));

    for (auto aViewZone : actions()) {
        aViewZone->setText(tr("View custom zone '%1'").arg(aViewZone->data().toString()));
    }
}

void CustomZoneMenu::clearCustomZonesMenu()
{
    clear();
    menuAction()->setVisible(false);
}

void CustomZoneMenu::addViewCustomZoneActionToCustomZoneMenu(QString zoneName)
{
    if (!ruledCustomZoneViewAllowed(player->getPlayerInfo()->getLocalOrJudge(),
                                   RuledActions::isRuledGame(player->getGame()), zoneName)) {
        return;
    }
    menuAction()->setVisible(true);
    QAction *aViewZone = addAction(tr("View custom zone '%1'").arg(zoneName));
    aViewZone->setData(zoneName);
    connect(aViewZone, &QAction::triggered, this,
            [zoneName, this]() { player->getGameScene()->toggleZoneView(player, zoneName, -1); });
}
