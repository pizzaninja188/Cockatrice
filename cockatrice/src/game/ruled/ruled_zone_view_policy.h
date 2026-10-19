#ifndef COCKATRICE_RULED_ZONE_VIEW_POLICY_H
#define COCKATRICE_RULED_ZONE_VIEW_POLICY_H

#include <array>
#include <limits>

#include <QPointF>
#include <QRectF>
#include <QSizeF>
#include <QString>
#include <libcockatrice/utility/zone_names.h>

inline bool ruledPlayerNeedsCustomZoneMenu(bool localOrJudge, bool ruledMode)
{
    return localOrJudge || ruledMode;
}

inline bool ruledCustomZoneViewAllowed(bool localOrJudge, bool ruledMode, const QString &zoneName)
{
    return localOrJudge || (ruledMode && zoneName == QLatin1String(ZoneNames::COMMAND));
}

inline bool ruledZoneViewHasAuthoritativeCardList(bool revealZone, bool writeableRevealZone)
{
    return revealZone && !writeableRevealZone;
}

inline QPointF ruledLibrarySearchPickerPosition(const QRectF &visibleStackSceneRect,
                                                const QSizeF &pickerSize,
                                                const QPointF &preferredPosition,
                                                const QRectF &viewportSceneRect)
{
    if (!viewportSceneRect.isValid() || viewportSceneRect.isEmpty() || pickerSize.width() <= 0.0 ||
        pickerSize.height() <= 0.0) {
        return preferredPosition;
    }

    const bool hasVisibleStack = visibleStackSceneRect.isValid() && !visibleStackSceneRect.isEmpty();
    const QRectF preferredRect(preferredPosition, pickerSize);
    if (viewportSceneRect.contains(preferredRect) &&
        (!hasVisibleStack || !preferredRect.intersects(visibleStackSceneRect))) {
        return preferredPosition;
    }

    const auto clampParallel = [](qreal value, qreal start, qreal extent, qreal pickerExtent) {
        const qreal maximum = start + extent - pickerExtent;
        return maximum < start ? start : qBound(start, value, maximum);
    };
    const auto clampToViewport = [&](const QPointF &position) {
        return QPointF(clampParallel(position.x(), viewportSceneRect.left(), viewportSceneRect.width(),
                                     pickerSize.width()),
                       clampParallel(position.y(), viewportSceneRect.top(), viewportSceneRect.height(),
                                     pickerSize.height()));
    };
    if (!hasVisibleStack) {
        return clampToViewport(preferredPosition);
    }

    // These placements are flush with the visible stack bounds. Only the axis running along the
    // selected edge is clamped, so the picker never slides back across the stack it is uncovering.
    const qreal leftAlignedY = clampParallel(preferredPosition.y(), viewportSceneRect.top(),
                                             viewportSceneRect.height(), pickerSize.height());
    const qreal topAlignedX = clampParallel(preferredPosition.x(), viewportSceneRect.left(),
                                            viewportSceneRect.width(), pickerSize.width());
    const std::array<QPointF, 4> edgePositions = {
        QPointF(visibleStackSceneRect.left() - pickerSize.width(), leftAlignedY),
        QPointF(visibleStackSceneRect.right(), leftAlignedY),
        QPointF(topAlignedX, visibleStackSceneRect.top() - pickerSize.height()),
        QPointF(topAlignedX, visibleStackSceneRect.bottom()),
    };
    const auto pickerRectAt = [&](const QPointF &position) { return QRectF(position, pickerSize); };
    const auto squaredDistance = [](const QPointF &left, const QPointF &right) {
        const qreal dx = left.x() - right.x();
        const qreal dy = left.y() - right.y();
        return dx * dx + dy * dy;
    };

    int bestFullyVisible = -1;
    qreal bestDistance = std::numeric_limits<qreal>::max();
    for (int i = 0; i < static_cast<int>(edgePositions.size()); ++i) {
        const QRectF candidate = pickerRectAt(edgePositions[static_cast<std::size_t>(i)]);
        if (!viewportSceneRect.contains(candidate) || candidate.intersects(visibleStackSceneRect)) {
            continue;
        }
        const qreal distance = squaredDistance(candidate.topLeft(), preferredPosition);
        if (distance < bestDistance) {
            bestDistance = distance;
            bestFullyVisible = i;
        }
    }
    if (bestFullyVisible >= 0) {
        return edgePositions[static_cast<std::size_t>(bestFullyVisible)];
    }

    // Under tighter viewport constraints, keep a non-overlapping candidate only when its title
    // region remains usable, preferring the one with the most visible area. Match the existing
    // ZoneViewWidget title bar (24 units) and minimum visible width (100 units), capped for smaller
    // picker dimensions.
    int bestReachable = -1;
    qreal bestVisibleArea = -1.0;
    bestDistance = std::numeric_limits<qreal>::max();
    const qreal minimumTitleWidth = qMin(100.0, pickerSize.width());
    const qreal minimumTitleHeight = qMin(24.0, pickerSize.height());
    for (int i = 0; i < static_cast<int>(edgePositions.size()); ++i) {
        const QPointF position = edgePositions[static_cast<std::size_t>(i)];
        const QRectF candidate = pickerRectAt(position);
        if (!viewportSceneRect.contains(candidate.topLeft()) || !viewportSceneRect.intersects(candidate) ||
            candidate.intersects(visibleStackSceneRect)) {
            continue;
        }
        const QRectF titleRegion(position, QSizeF(pickerSize.width(), minimumTitleHeight));
        const QRectF visibleTitleRegion = titleRegion.intersected(viewportSceneRect);
        if (visibleTitleRegion.width() < minimumTitleWidth || visibleTitleRegion.height() < minimumTitleHeight) {
            continue;
        }
        const QRectF visible = candidate.intersected(viewportSceneRect);
        const qreal visibleArea = visible.width() * visible.height();
        const qreal distance = squaredDistance(position, preferredPosition);
        if (visibleArea > bestVisibleArea || (visibleArea == bestVisibleArea && distance < bestDistance)) {
            bestVisibleArea = visibleArea;
            bestDistance = distance;
            bestReachable = i;
        }
    }
    if (bestReachable >= 0) {
        return edgePositions[static_cast<std::size_t>(bestReachable)];
    }

    // No edge placement leaves a usable title region (for example, when the stack covers the
    // viewport). Keep the picker operable and maximize its visible area; overlap is then unavoidable.
    return clampToViewport(preferredPosition);
}

#endif // COCKATRICE_RULED_ZONE_VIEW_POLICY_H
