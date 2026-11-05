#include "ruled_public_card_zone.h"

#include "../../game_graphics/board/card_item.h"
#include "../../game_graphics/card_dimensions.h"

#include <QPainter>
#include <libcockatrice/game/zones/card_zone_logic.h>

RuledPublicCardZone::RuledPublicCardZone(CardZoneLogic *logic, QGraphicsItem *parent) : CardZone(logic, parent)
{
    connect(logic, &CardZoneLogic::cardCountChanged, this, &RuledPublicCardZone::reorganizeCards);
    for (auto *card : logic->getCards()) {
        CardZone::onCardAdded(card);
    }
    reorganizeCards();
}

void RuledPublicCardZone::onCardAdded(CardState *card)
{
    CardZone::onCardAdded(card);
    reorganizeCards();
}

void RuledPublicCardZone::reorganizeCards()
{
    const auto &cards = cardItems();
    const QRectF next = cards.isEmpty() ? QRectF()
                                        : QRectF(0, 0, 2 * (CardDimensions::WIDTH + 8),
                                                 22 + ((cards.size() + 1) / 2) * (CardDimensions::HEIGHT + 8));
    const bool changed = next != bounds;
    prepareGeometryChange();
    bounds = next;
    for (int i = 0; i < cards.size(); ++i) {
        cards[i]->setVisible(true);
        cards[i]->setPos((i % 2) * (CardDimensions::WIDTH + 8), 22 + (i / 2) * (CardDimensions::HEIGHT + 8));
    }
    update();
    if (changed) {
        emit extentChanged();
    }
}

void RuledPublicCardZone::paint(QPainter *painter, const QStyleOptionGraphicsItem *, QWidget *)
{
    if (bounds.isEmpty()) {
        return;
    }
    painter->setPen(Qt::white);
    painter->drawText(QRectF(0, 0, bounds.width(), 20), Qt::AlignLeft | Qt::AlignVCenter, tr("Command zone"));
}
