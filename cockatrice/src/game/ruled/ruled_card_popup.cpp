#include "ruled_card_popup.h"

#include "../board/abstract_card_item.h"
#include "../board/card_item.h"
#include "../player/player.h"
#include "../player/player_info.h"
#include "ruled_actions.h"
#include "ruled_client_state.h"

#include <QObject>
#include <libcockatrice/utility/zone_names.h>
#include <memory>
#include <utility>

void connectRuledCardPopup(AbstractGame *game,
                           AbstractCardItem *card,
                           QObject *context,
                           std::function<void(const QPoint &, const CardRef &)> show,
                           std::function<void(const QString &)> dismiss)
{
    auto identity = std::make_shared<RuledCardPopupIdentity>();
    QObject::connect(card, &AbstractCardItem::showCardInfoPopup, context,
                     [game, card, identity, show = std::move(show)](const QPoint &pos, const CardRef &ref) {
                         auto *physical = dynamic_cast<CardItem *>(card);
                         auto *zone = physical ? physical->getZone() : nullptr;
                         const bool ruledFaceDownExile = physical && physical->getFaceDown() && zone &&
                                                         zone->getName() == ZoneNames::EXILE &&
                                                         RuledActions::isRuledGame(game);
                         const RuledClientState *state = ruledFaceDownExile ? RuledActions::stateFor(game) : nullptr;
                         Player *zonePlayer = zone ? zone->getPlayer() : nullptr;
                         const PlayerInfo *info = zonePlayer ? zonePlayer->getPlayerInfo() : nullptr;
                         const QString privateName = state && info
                                                         ? state->privateFaceDownNameForCard(info->getId(), physical->getId())
                                                         : QString{};
                         show(pos, identity->open(ref, ruledFaceDownExile, privateName));
                     });
    QObject::connect(card, static_cast<void (AbstractCardItem::*)(QString)>(&AbstractCardItem::deleteCardInfoPopup),
                     context, [identity, dismiss = std::move(dismiss)](const QString &name) {
                         dismiss(identity->close(name));
                     });
}
