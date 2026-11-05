#include "ruled_public_card_zone.h"

#include "../../game_graphics/board/card_item.h"
#include "../../game_graphics/card_dimensions.h"

#include <QFontMetrics>
#include <QPainter>
#include <libcockatrice/game/zones/card_zone_logic.h>

namespace
{
constexpr int cardGap = 4;
constexpr int labelHeight = 14;
}

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
                                        : QRectF(0, 0, cards.size() * (CardDimensions::WIDTH + cardGap) - cardGap,
                                                 labelHeight + CardDimensions::HEIGHT);
    const bool changed = next != bounds;
    prepareGeometryChange();
    bounds = next;
    for (int i = 0; i < cards.size(); ++i) {
        cards[i]->setVisible(true);
        cards[i]->setPos(i * (CardDimensions::WIDTH + cardGap), labelHeight);
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
    auto font = painter->font();
    font.setPixelSize(10);
    painter->setFont(font);
    painter->setPen(Qt::white);
    const auto label = painter->fontMetrics().elidedText(tr("Command zone"), Qt::ElideRight, bounds.width());
    painter->drawText(QRectF(0, 0, bounds.width(), labelHeight), Qt::AlignCenter, label);
}
