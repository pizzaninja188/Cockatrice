#include "ruled_auxiliary_area.h"

#include <QPainter>

RuledAuxiliaryArea::RuledAuxiliaryArea(QGraphicsItem *parent) : QGraphicsObject(parent)
{
    setZValue(5);
    setAcceptedMouseButtons(Qt::NoButton);
}

void RuledAuxiliaryArea::addSection(QGraphicsItem *section)
{
    if (!section || sections.contains(section)) {
        return;
    }
    section->setParentItem(this);
    sections.append(section);
    relayout();
}

void RuledAuxiliaryArea::anchorTo(const QRectF &landRow)
{
    row = landRow;
    setPos(row.right() - bounds.width() - edgeInset, row.center().y() - bounds.height() / 2);
}

void RuledAuxiliaryArea::relayout()
{
    constexpr qreal padding = 4;
    constexpr qreal gap = 8;
    qreal x = padding;
    qreal height = 0;
    for (auto *section : sections) {
        const auto rect = section->boundingRect();
        if (!section->isVisibleTo(this) || rect.isEmpty()) {
            continue;
        }
        section->setPos(x - rect.left(), padding - rect.top());
        x += rect.width() + gap;
        height = qMax(height, rect.height());
    }
    const QRectF next = height > 0 ? QRectF(0, 0, x - gap + padding, height + 2 * padding) : QRectF();
    const bool changed = next != bounds;
    prepareGeometryChange();
    bounds = next;
    anchorTo(row);
    update();
    if (changed) {
        emit extentChanged();
    }
}

void RuledAuxiliaryArea::paint(QPainter *painter, const QStyleOptionGraphicsItem *, QWidget *)
{
    if (bounds.isEmpty()) {
        return;
    }
    painter->setPen(QColor(160, 160, 160, 180));
    painter->setBrush(QColor(25, 25, 25, 170));
    painter->drawRoundedRect(bounds.adjusted(1, 1, -1, -1), 4, 4);
}
