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

void RuledAuxiliaryArea::anchorTo(const QRectF &rect, bool local)
{
    battlefield = rect;
    localPlayer = local;
    setPos(rect.right() - bounds.width() - 8, local ? rect.bottom() - bounds.height() - 8 : rect.top() + 8);
}

void RuledAuxiliaryArea::relayout()
{
    qreal width = 0;
    qreal height = 0;
    for (auto *section : sections) {
        if (!section->isVisibleTo(this) || section->boundingRect().isEmpty()) {
            continue;
        }
        section->setPos(8 - section->boundingRect().left(), height + 8 - section->boundingRect().top());
        width = qMax(width, section->boundingRect().width());
        height += section->boundingRect().height() + 8;
    }
    const QRectF next = height > 0 ? QRectF(0, 0, width + 16, height + 8) : QRectF();
    const bool changed = next != bounds;
    prepareGeometryChange();
    bounds = next;
    anchorTo(battlefield, localPlayer);
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
    painter->setBrush(QColor(25, 25, 25, 210));
    painter->drawRoundedRect(bounds.adjusted(1, 1, -1, -1), 6, 6);
}
