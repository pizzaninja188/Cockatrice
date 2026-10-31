#ifndef RULED_CARD_PHASING_PAINTER_H
#define RULED_CARD_PHASING_PAINTER_H

#include <QBrush>
#include <QColor>
#include <QFont>
#include <QPainter>
#include <QPainterPath>
#include <QPen>
#include <QRectF>
#include <QSizeF>
#include <QString>
#include <QtGlobal>

namespace RuledCardPhasingPainter
{
/// Draw the neutral public phasing veil and banner over the existing physical card.
template <typename ApplyCardTransform>
void paintPhasedOut(QPainter *painter,
                    const QPainterPath &cardShape,
                    const QSizeF &translatedSize,
                    qreal scaleFactor,
                    const QString &label,
                    ApplyCardTransform applyCardTransform)
{
    // Phasing is public status, while this remains the same physical card (including any
    // face-down presentation). The neutral veil and label expose no Oracle/card identity.
    painter->save();
    painter->fillPath(cardShape, QBrush(QColor(12, 28, 44, 105)));
    painter->setRenderHint(QPainter::Antialiasing, true);
    QPen phasedBorder(QColor(105, 195, 245));
    phasedBorder.setWidthF(qMax(2.0, 3.0 * scaleFactor));
    painter->setPen(phasedBorder);
    painter->setBrush(Qt::NoBrush);
    painter->drawPath(cardShape);
    painter->restore();

    painter->save();
    applyCardTransform(painter);
    const qreal bannerInset = 7.0 * scaleFactor;
    const QRectF banner(bannerInset, translatedSize.height() * 0.42,
                        translatedSize.width() - 2.0 * bannerInset, translatedSize.height() * 0.16);
    painter->setPen(Qt::NoPen);
    painter->setBrush(QColor(7, 24, 40, 235));
    painter->drawRoundedRect(banner, 4.0 * scaleFactor, 4.0 * scaleFactor);
    painter->setPen(QColor(195, 230, 250));
    QFont phasedFont = painter->font();
    phasedFont.setBold(true);
    painter->setFont(phasedFont);
    painter->drawText(banner.adjusted(3.0 * scaleFactor, 0.0, -3.0 * scaleFactor, 0.0), Qt::AlignCenter, label);
    painter->restore();
}
} // namespace RuledCardPhasingPainter

#endif // RULED_CARD_PHASING_PAINTER_H
