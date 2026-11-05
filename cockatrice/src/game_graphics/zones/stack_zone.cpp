#include "stack_zone.h"

#include "../../game/ruled/ruled_actions.h"
#include "../../client/settings/cache_settings.h"
#include "../../interface/theme_manager.h"
#include <libcockatrice/game/abstract_game.h>
#include "../board/arrow_item.h"
#include "../board/card_drag_item.h"
#include "../board/card_item.h"
#include <libcockatrice/game/game_event_handler.h>
#include "../../game/ruled/ruled_client_state.h"
#include <libcockatrice/game/player/player_logic.h>
#include "../../game/ruled/ruled_player_actions.h"
#include <libcockatrice/game/zones/stack_zone_logic.h>
#include "../../interface/theme_manager.h"
#include "../board/card_drag_item.h"
#include "../board/card_item.h"
#include "../card_dimensions.h"

#include <QBrush>
#include <QColor>
#include <QGraphicsItem>
#include <QList>
#include <QPainter>
#include <algorithm>
#include <climits>
#include <libcockatrice/protocol/pb/command_move_card.pb.h>
#include <QPoint>
#include <QRectF>
#include <QString>
#include <libcockatrice/game/board/card_list.h>
#include <libcockatrice/game/player/player_actions.h>
#include <libcockatrice/game/player/player_info.h>
#include <libcockatrice/game/player/player_logic.h>
#include <libcockatrice/game/zones/card_zone_logic.h>
#include <libcockatrice/game/zones/stack_zone_logic.h>
#include <qnumeric.h>
#include <string>

StackZone::StackZone(StackZoneLogic *_logic, int _zoneHeight, QGraphicsItem *parent)
    : SelectZone(_logic, parent), zoneHeight(_zoneHeight)
{
    connect(themeManager, &ThemeManager::themeChanged, this, &StackZone::updateBg);
    if (auto *game = getLogic()->getPlayer()->getGame()) {
        if (auto *handler = game->getGameEventHandler()) {
            connect(handler->ruled(), &RuledClientState::stackOrderChanged, this,
                    [this](const QList<quint32> &) { reorganizeCards(); });
        }
    }
    updateBg();
    setCacheMode(DeviceCoordinateCache);
}

void StackZone::updateBg()
{
    update();
}

QRectF StackZone::boundingRect() const
{
    return {0, 0, CardDimensions::WIDTH_F * 1.5, zoneHeight};
}

void StackZone::paint(QPainter *painter, const QStyleOptionGraphicsItem * /*option*/, QWidget * /*widget*/)
{
    if (playmatActive) {
        // Subtle overlay to distinguish stack zone from table zone (slightly darker)
        painter->fillRect(boundingRect(), QColor(0, 0, 0, 80));
    } else {
        QBrush brush = themeManager->getExtraBgBrush(ThemeManager::Stack, getLogic()->getPlayer()->getZoneId());
        painter->fillRect(boundingRect(), brush);
    }
}

void StackZone::onPlaymatChanged(bool active)
{
    playmatActive = active;
    // See TableZone::onPlaymatChanged for the rationale. Translucent overlay
    // over a dynamic playmat should not be held in the device cache.
    setCacheMode(active ? QGraphicsItem::NoCache : QGraphicsItem::DeviceCoordinateCache);
    update();
}

void StackZone::handleDropEvent(const QList<CardDragItem *> &dragItems,
                                CardZoneLogic *startZone,
                                const QPoint &dropPoint)
{
    if (startZone == nullptr || startZone->getPlayer() == nullptr || dragItems.isEmpty()) {
        return;
    }

    bool sameZone = startZone == getLogic();
    int index = calcDropIndexFromY(dropPoint.y(), !sameZone, MIN_CARD_VISIBLE);
    if (sameZone) {
        // Same-zone no-op: don't move a card onto itself
        const auto &cards = cardItems();
        if (!cards.isEmpty() && cards.at(index)->getId() == dragItems.at(0)->getId()) {
            return;
        }
    }

    QList<CardMoveRequest> cards;
    cards.reserve(dragItems.size());
    for (const CardDragItem *item : dragItems) {
        if (item) {
            cards.append(CardMoveRequest{item->getId(), item->isForceFaceDown()});
        }
    }

    getLogic()->getPlayer()->getPlayerActions()->moveCards(startZone, getLogic(), index, 0, cards);
}

void StackZone::setHeight(qreal newHeight)
{
    if (qFuzzyCompare(1.0 + zoneHeight, 1.0 + newHeight)) {
        return;
    }
    prepareGeometryChange();
    zoneHeight = newHeight;
    reorganizeCards();
    update();
}

void StackZone::reorganizeCards()
{
    const auto &rawCards = cardItems();
    if (!rawCards.isEmpty()) {
        // Build a display list and sort by engine push order when in a ruled game. A physical
        // Non-hand cast sources can be parked here before payment, but remain hidden until
        // CommitSpellCast produces StackPushed. Hand casts move here only on commit.
        // Index 0 = most recently pushed = resolves first. The underlying rawCards list
        // is not reordered so that takeCard(position, id) continues to work correctly.
        QList<CardItem *> display;
        display.reserve(rawCards.size());
        RuledClientState *ruledState = nullptr;
        int stackOwnerId = -1;
        if (auto *ag = getLogic()->getPlayer()->getGame()) {
            if (RuledActions::isRuledGame(ag)) {
                ruledState = ag->getGameEventHandler()->ruled();
                stackOwnerId = getLogic()->getPlayer()->getPlayerInfo()->getId();
            }
        }
        for (CardItem *card : rawCards) {
            const bool published = !ruledState || ruledState->isPublishedStackCard(stackOwnerId, card->getId());
            card->setVisible(published);
            if (published) {
                display.append(card);
            }
        }
        if (ruledState) {
            const QList<quint32> &oidOrder = ruledState->getStackOidOrder();
            std::sort(display.begin(), display.end(), [&](CardItem *a, CardItem *b) {
                int ia = static_cast<int>(oidOrder.indexOf(ruledState->engineOidForCardId(stackOwnerId, a->getId())));
                int ib = static_cast<int>(oidOrder.indexOf(ruledState->engineOidForCardId(stackOwnerId, b->getId())));
                if (ia < 0)
                    ia = INT_MAX;
                if (ib < 0)
                    ib = INT_MAX;
                return ia < ib;
            });
        }

        if (display.isEmpty()) {
            update();
            return;
        }

        const auto cardCount = static_cast<int>(display.size());
        qreal totalWidth = boundingRect().width();
        qreal cardWidth = display.at(0)->boundingRect().width();
        qreal xspace = 5;
        qreal x1 = xspace;
        qreal x2 = totalWidth - xspace - cardWidth;

        auto params = buildStackParams(MIN_CARD_VISIBLE);
        params.cardCount = cardCount;
        const auto layout = computeZoneLayout(params);
        for (int i = 0; i < cardCount; i++) {
            CardItem *card = display.at(i);
            qreal x = (i % 2) ? x2 : x1;
            qreal y = layout.start + i * layout.effectiveOffset;
            card->setPos(x, y);
            card->setRealZValue(i);
        }
    }
    update();
}
