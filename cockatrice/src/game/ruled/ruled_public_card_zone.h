#pragma once

#include "../../game_graphics/zones/card_zone.h"

// A visible ordered row of public cards, backed by the existing real zone and CardItems.
class RuledPublicCardZone : public CardZone
{
    Q_OBJECT
public:
    explicit RuledPublicCardZone(CardZoneLogic *logic, QGraphicsItem *parent = nullptr);
    QRectF boundingRect() const override
    {
        return bounds;
    }
    void paint(QPainter *, const QStyleOptionGraphicsItem *, QWidget *) override;
    void onCardAdded(CardState *card) override;
    void handleDropEvent(const QList<CardDragItem *> &, CardZoneLogic *, const QPoint &) override
    {
    }
    void reorganizeCards() override;
signals:
    void extentChanged();

private:
    QRectF bounds;
};
