#include "ruled_client_ui.h"

#include "../../game_graphics/board/card_item.h"
#include "../../game_graphics/game_scene.h"
#include "../../game_graphics/player/player_graphics_item.h"
#include "../../game_graphics/zones/card_zone.h"
#include "../../game_graphics/zones/view_zone.h"
#include "../../interface/widgets/tabs/tab_game.h"
#include "ruled_game_controller.h"
#include "ruled_player_actions.h"

#include <libcockatrice/game/abstract_game.h>
#include <libcockatrice/game/board/arrow_data.h>
#include <libcockatrice/game/board/card_list.h>
#include <libcockatrice/game/board/card_state.h>
#include <libcockatrice/game/game_event_handler.h>
#include <libcockatrice/game/player/player_info.h>
#include <libcockatrice/game/player/player_logic.h>
#include <libcockatrice/game/zones/card_zone_logic.h>
CardItem *RuledCardViews::findCard(int id) const
{
    for (auto *card : *this) {
        if (card && card->getId() == id) {
            return card;
        }
    }
    return nullptr;
}
namespace RuledActions
{
RuledGameController *controller(const AbstractGame *game)
{
    return game ? dynamic_cast<RuledGameController *>(game->getGameEventHandler()->getRuledHooks()) : nullptr;
}
RuledClientHost *host(const AbstractGame *game)
{
    return controller(game);
}

QSharedPointer<ArrowData> arrowData(PlayerLogic *creator, int id, const QColor &color)
{
    auto data = QSharedPointer<ArrowData>::create();
    data->id = id;
    data->color = color;
    if (creator) {
        data->creatorId = creator->getPlayerInfo()->getId();
        data->isLocalCreator = creator->getPlayerInfo()->getLocal();
    }
    return data;
}

TabGame *tab(const AbstractGame *game)
{
    return game ? qobject_cast<TabGame *>(game->parent()) : nullptr;
}
GameScene *scene(const AbstractGame *game)
{
    auto *view = tab(game);
    return view ? view->getGameScene() : nullptr;
}
GameScene *scene(const PlayerLogic *player)
{
    return player ? scene(player->getGame()) : nullptr;
}
MessageLogWidget *log(const AbstractGame *game)
{
    auto *view = tab(game);
    return view ? view->getMessageLog() : nullptr;
}
PlayerGraphicsItem *graphics(const PlayerLogic *player)
{
    auto *view = player ? scene(player->getGame()) : nullptr;
    return view ? view->viewForPlayer(player->getPlayerInfo()->getId()) : nullptr;
}
RuledPlayerActions *actions(const PlayerLogic *player)
{
    return player ? qobject_cast<RuledPlayerActions *>(player->getPlayerActions()) : nullptr;
}
CardItem *cardView(CardState *state)
{
    return state ? qobject_cast<CardItem *>(state->parent()) : nullptr;
}
CardItem *card(const AbstractGame *game, int playerId, const QString &zone, int cardId)
{
    return game ? cardView(game->getCard(playerId, zone, cardId)) : nullptr;
}
RuledCardViews cardItems(const CardList &cards)
{
    RuledCardViews result(cards.getContentsKnown());
    for (auto *state : cards) {
        if (auto *view = cardView(state)) {
            result.append(view);
        }
    }
    return result;
}
CardZone *zoneGraphics(CardZoneLogic *zone)
{
    auto *view = zone && zone->getPlayer() ? scene(zone->getPlayer()->getGame()) : nullptr;
    if (!view) {
        return nullptr;
    }
    for (auto *item : view->items()) {
        auto *candidate = qgraphicsitem_cast<CardZone *>(item);
        if (candidate && candidate->getLogic() == zone) {
            return candidate;
        }
    }
    return nullptr;
}
ZoneViewZone *zoneView(CardZoneLogic *zone)
{
    return qobject_cast<ZoneViewZone *>(zoneGraphics(zone));
}
} // namespace RuledActions
