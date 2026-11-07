#ifndef RULED_INTERACTION_VISUAL_H
#define RULED_INTERACTION_VISUAL_H

#include <QPainter>
#include <QPainterPath>

/// Presentation only. The caller obtains eligibility from the same engine offers as clicks.
struct RuledInteractionVisual
{
    bool dimmed = false;
    bool candidate = false;
    bool actionable = false;
    bool related = false;
};

namespace RuledInteractionPainter
{
/// Paint after card art, before counters, annotations and existing selection markers.
inline void
paint(QPainter *painter, const QPainterPath &shape, const RuledInteractionVisual &visual, int dimPercent = 45)
{
    painter->save();
    if (visual.dimmed) {
        painter->fillPath(shape, QColor(0, 0, 0, qRound(255.0 * qBound(0, dimPercent, 100) / 100.0)));
    } else if (visual.candidate || visual.actionable || visual.related) {
        // Draw inside the card's bounds, with a dark backing for pale artwork.
        painter->setClipPath(shape, Qt::IntersectClip);
        painter->setRenderHint(QPainter::Antialiasing, true);
        painter->setBrush(Qt::NoBrush);
        QPen backing(QColor(0, 0, 0, 220));
        backing.setWidthF(8);
        painter->setPen(backing);
        painter->drawPath(shape);
        QPen outline(visual.candidate ? QColor(80, 200, 255)
                     : visual.related ? QColor(255, 204, 64)
                                      : QColor(Qt::white));
        outline.setWidthF(5);
        painter->setPen(outline);
        painter->drawPath(shape);
    }
    painter->restore();
}
} // namespace RuledInteractionPainter

#endif
