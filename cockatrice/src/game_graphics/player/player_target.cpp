#include <libcockatrice/settings/interface_settings.h>
#include "player_target.h"

#include "../../client/settings/cache_settings.h"
#include "../../interface/pixel_map_generator.h"
#include <libcockatrice/game/abstract_game.h>
#include <libcockatrice/game/game_event_handler.h>
#include "../../game/ruled/ruled_player_actions.h"
#include <libcockatrice/game/player/player_manager.h>
#include "../../game/ruled/ruled_actions.h"
#include "../../game/ruled/ruled_client_state.h"
#include "../../game/ruled/ruled_interaction_ui.h"
#include <libcockatrice/game/player/player_logic.h>

#include <QDebug>
#include <QGraphicsSceneMouseEvent>
#include "../../interface/theme_manager.h"
#include "../board/abstract_counter.h"
#include "../board/abstract_graphics_item.h"
#include "../board/arrow_target.h"
#include "../game_scene.h"
#include "libcockatrice/settings/interface_settings.h"
#include "user_level.h"

#include <QBrush>
#include <QColor>
#include <QFlag>
#include <QFlags>
#include <QFont>
#include <QObject>
#include <QPainter>
#include <QPainterPath>
#include <QPen>
#include <QPixmapCache>
#include <QPointF>
#include <QRadialGradient>
#include <QRectF>
#include <QSize>
#include <QSizeF>
#include <QString>
#include <QTransform>
#include <QtGlobal>
#include <QtMath>
#include <libcockatrice/game/player/player_info.h>
#include <libcockatrice/game/player/player_logic.h>
#include <libcockatrice/protocol/pb/serverinfo_user.pb.h>
#include <qnamespace.h>
#include <qnumeric.h>
#include <string>
#include <sys/types.h>

class CounterState;
class QGraphicsItem;

PlayerCounter::PlayerCounter(CounterState *state, PlayerLogic *player, QGraphicsItem *parent)
    : AbstractCounter(state, player, false, false, parent)
{
}

QRectF PlayerCounter::boundingRect() const
{
    return {0, 0, 50, 30};
}

void PlayerCounter::paint(QPainter *painter, const QStyleOptionGraphicsItem * /*option*/, QWidget * /*widget*/)
{
    const int radius = 15;
    const qreal border = 1.5;
    // The box is drawn with a border-wide stroke straddling the path, so the
    // visible outline spans [inset, inset + border]. Fills that must not cover
    // the outline (e.g. the life-change flash) use a path inset by `border`.
    const auto makePath = [](qreal inset) {
        QPainterPath path(QPointF(50 - inset, inset));
        path.lineTo(radius, inset);
        path.arcTo(inset, inset, 2 * radius, 2 * radius, 90, 90);
        path.lineTo(inset, 30 - inset);
        path.lineTo(50 - inset, 30 - inset);
        path.closeSubpath();
        return path;
    };
    QPainterPath path = makePath(border / 2);

    QPen pen(QColor(100, 100, 100));
    pen.setWidthF(border);
    painter->setPen(pen);
    painter->setBrush(hovered ? QColor(50, 50, 50, 160) : QColor(0, 0, 0, 160));

    painter->drawPath(path);

    QRectF translatedRect = path.controlPointRect();
    QSize translatedSize = translatedRect.size().toSize();
    QFont font = themeManager->cardTextFont();
    font.setWeight(QFont::Bold);
    font.setPixelSize(qMax(qRound(translatedSize.height() / 1.3), 9));
    painter->setFont(font);
    painter->setPen(Qt::white);
    painter->drawText(translatedRect, Qt::AlignCenter, QString::number(value));

    // Life-change flash: emerald on gain, red on loss, decaying over a few ticks.
    if (flashAlpha > 0) {
        painter->save();
        QColor flashColor = flashDelta > 0 ? QColor(52, 224, 122) : QColor(239, 68, 68);
        flashColor.setAlphaF(0.45 * flashAlpha);
        painter->setPen(Qt::NoPen);
        painter->setBrush(flashColor);
        painter->setOpacity(0.85);
        painter->drawPath(makePath(border));
        painter->restore();
    }
}

void PlayerCounter::onValueChanged(int oldValue, int newValue, bool skipDamageAnimation)
{
    flashDelta = newValue - oldValue;
    if (flashDelta == 0) {
        return;
    }

    if (!SettingsCache::instance().userInterface().getLifeCounterAnimationsEnabled()) {
        flashAlpha = 0.0;
        return;
    }

    if (skipDamageAnimation) {
        flashAlpha = 0.0;
        return;
    }

    flashAlpha = 1.0;
    flashClock.start();
    if (scene()) {
        static_cast<GameScene *>(scene())->registerAnimationItem(this);
    }
}

bool PlayerCounter::animationEvent()
{
    flashAlpha = 1.0 - flashClock.elapsed() / flashDurationMs;
    if (flashAlpha <= 0.0) {
        flashAlpha = 0.0;
        return false;
    }
    update();
    return true;
}

PlayerTarget::PlayerTarget(PlayerLogic *_owner, QGraphicsItem *parentItem)
    : ArrowTarget(_owner, parentItem), playerCounter(nullptr)
{
    RuledInteractionUi::connectPlayer(this);
    setCacheMode(DeviceCoordinateCache);

    if (auto *game = _owner ? _owner->getGame() : nullptr) {
        if (auto *handler = game->getGameEventHandler()) {
            RuledClientState *ruled = handler->ruled();
            connect(ruled, &RuledClientState::spellTargetSelectionChanged, this, [this]() { update(); });
            connect(ruled, &RuledClientState::spellDamageAllocationUiChanged, this, [this]() { update(); });
            connect(ruled, &RuledClientState::resolutionCostSelectionChanged, this, [this]() { update(); });
        }
    }

    const std::string &bmp = _owner->getPlayerInfo()->getUserInfo()->avatar_bmp();
    if (!fullPixmap.loadFromData((const uchar *)bmp.data(), static_cast<uint>(bmp.size()))) {
        fullPixmap = QPixmap();
    }
}

PlayerTarget::~PlayerTarget()
{
    // Explicit deletion is necessary in spite of parent/child relationship
    // as we need this object to be alive to receive the destroyed() signal.
    delete playerCounter;
}

QRectF PlayerTarget::boundingRect() const
{
    return {0, 0, 160, 64};
}

void PlayerTarget::paint(QPainter *painter, const QStyleOptionGraphicsItem * /*option*/, QWidget * /*widget*/)
{
    const ServerInfo_User *const info = owner->getPlayerInfo()->getUserInfo();

    const qreal border = 2;

    QRectF avatarBoundingRect = boundingRect().adjusted(border, border, -border, -border);
    QRectF translatedRect = painter->combinedTransform().mapRect(avatarBoundingRect);
    QSize translatedSize = translatedRect.size().toSize();
    QPixmap cachedPixmap;
    // The key must cover everything the generated pawn depends on: the rendered
    // size, the user level, and the pixmap being drawn. fullPixmap.cacheKey() is
    // 0 for every null pixmap, so the default-pawn branch additionally needs the
    // pawn's privlevel (lowercased, matching UserLevelPixmapGenerator) and colors
    // in the key — otherwise two players without a custom avatar (and the same
    // user level) would share one cached pawn.
    const QString cacheKey = "avatar" + QString::number(translatedSize.width()) + "_" +
                             QString::number(translatedSize.height()) + "_" + QString::number(info->user_level()) +
                             "_" + QString::number(fullPixmap.cacheKey()) + "_" +
                             QString::fromStdString(info->privlevel()).toLower() + "_" +
                             QString::fromStdString(info->pawn_colors().left_side()) + "_" +
                             QString::fromStdString(info->pawn_colors().right_side());
    if (!QPixmapCache::find(cacheKey, &cachedPixmap)) {
        cachedPixmap = QPixmap(translatedSize.width(), translatedSize.height());

        QPainter tempPainter(&cachedPixmap);
        // pow(foo, 0.5) equals to sqrt(foo), but using sqrt(foo) in this context will produce a compile error with
        // MSVC++
        QRadialGradient grad(translatedRect.center(), qPow(translatedSize.width() * translatedSize.width() +
                                                               translatedSize.height() * translatedSize.height(),
                                                           0.5) /
                                                          2);
        grad.setColorAt(1, Qt::black);
        grad.setColorAt(0, QColor(180, 180, 180));
        tempPainter.fillRect(QRectF(0, 0, translatedSize.width(), translatedSize.height()), grad);

        if (fullPixmap.isNull()) {
            int sideLength = translatedSize.height();
            QPixmap tempPixmap = UserLevelPixmapGenerator::generatePixmap(
                sideLength, UserLevelFlags(info->user_level()), info->pawn_colors(), false,
                QString::fromStdString(info->privlevel()));
            int x = (translatedSize.width() - sideLength) / 2;
            int y = 0;
            tempPainter.drawPixmap(x, y, tempPixmap);
        } else {
            QPixmap tempPixmap = fullPixmap.scaled(translatedSize, Qt::KeepAspectRatio, Qt::SmoothTransformation);
            int x = (translatedSize.width() - tempPixmap.width()) / 2;
            int y = (translatedSize.height() - tempPixmap.height()) / 2;
            tempPainter.drawPixmap(x, y, tempPixmap);
        }

        QPixmapCache::insert(cacheKey, cachedPixmap);
    }

    painter->save();
    resetPainterTransform(painter);
    painter->translate((translatedSize.width() - cachedPixmap.width()) / 2.0, 0);
    painter->drawPixmap(translatedRect, cachedPixmap, cachedPixmap.rect());
    painter->restore();

    RuledInteractionUi::paintPlayer(this, painter, avatarBoundingRect);
    QRectF nameRect = QRectF(0, boundingRect().height() - 20, 110, 20);
    painter->fillRect(nameRect, QColor(0, 0, 0, 160));
    QRectF translatedNameRect = painter->combinedTransform().mapRect(nameRect);

    painter->save();
    resetPainterTransform(painter);

    QString name = QString::fromStdString(info->name());
    if (name.size() > 13) {
        name = name.mid(0, 10) + "...";
    }

    QFont font = themeManager->appFont(AppFont::CardText);
    font.setPixelSize(qMax(qRound(translatedNameRect.height() / 1.5), 9));
    painter->setFont(font);
    painter->setPen(Qt::white);
    painter->drawText(translatedNameRect, Qt::AlignVCenter | Qt::AlignLeft, "  " + name);
    painter->restore();

    QPen pen(QColor(100, 100, 100));
    pen.setWidth(border);
    pen.setJoinStyle(Qt::RoundJoin);
    painter->setPen(pen);
    painter->drawRect(boundingRect().adjusted(border / 2, border / 2, -border / 2, -border / 2));

    if (priorityHighlighted) {
        const qreal outerWidth = 4.0;
        const qreal inset = border + 1.0;
        QRectF highlightRect = avatarBoundingRect.adjusted(inset, inset, -inset, -inset);
        QPen priorityPen(QColor(255, 204, 64));
        priorityPen.setWidthF(outerWidth);
        priorityPen.setJoinStyle(Qt::RoundJoin);
        painter->setPen(priorityPen);
        painter->setBrush(Qt::NoBrush);
        painter->drawRoundedRect(highlightRect, 4.0, 4.0);
    }

    if (owner && owner->getGame()) {
        AbstractGame *ruledGame = owner->getGame();
        const int ownerId = owner->getPlayerInfo()->getId();
        if (RuledActions::isPlayerSelectedAsSpellTarget(ruledGame, ownerId)) {
            const qreal selWidth = 3.0;
            const qreal selInset = border + 3.0;
            QRectF selRect = avatarBoundingRect.adjusted(selInset, selInset, -selInset, -selInset);
            QPen selPen(QColor(220, 40, 40));
            selPen.setWidthF(selWidth);
            selPen.setJoinStyle(Qt::RoundJoin);
            painter->setPen(selPen);
            painter->setBrush(Qt::NoBrush);
            painter->drawRoundedRect(selRect, 4.0, 4.0);
        }
        if (GameEventHandler *handler = ruledGame->getGameEventHandler()) {
            RuledClientState *const ruled = handler->ruled();
            const quint32 playerId = static_cast<quint32>(ownerId);
            if (ruled->isProliferatePlayerCandidate(playerId)) {
                const qreal inset = border + 2.0;
                QRectF candidateRect = avatarBoundingRect.adjusted(inset, inset, -inset, -inset);
                QPen candidatePen(QColor(80, 200, 255));
                candidatePen.setWidthF(3.0);
                candidatePen.setJoinStyle(Qt::RoundJoin);
                painter->setPen(candidatePen);
                painter->setBrush(Qt::NoBrush);
                painter->drawRoundedRect(candidateRect, 4.0, 4.0);
            }
            if (ruled->isProliferatePlayerSelected(playerId)) {
                const qreal inset = border + 6.0;
                QRectF selectedRect = avatarBoundingRect.adjusted(inset, inset, -inset, -inset);
                QPen selectedPen(QColor(80, 230, 130));
                selectedPen.setWidthF(4.0);
                selectedPen.setJoinStyle(Qt::RoundJoin);
                painter->setPen(selectedPen);
                painter->setBrush(Qt::NoBrush);
                painter->drawRoundedRect(selectedRect, 4.0, 4.0);
            }
        }
        if (RuledActions::isSpellDamageAllocationDisplayActive(ruledGame)) {
            const int alloc = RuledActions::spellDamageAllocationForPlayerId(ruledGame, ownerId);
            if (alloc > 0) {
                paintNumberEllipse(alloc, 14, QColor(255, 120, 0), 0, 1, painter);
            }
        }
    }

    if (getBeingPointedAt()) {
        painter->fillRect(boundingRect(), QBrush(QColor(255, 0, 0, 100)));
    }
}

void PlayerTarget::setPriorityHighlighted(bool highlighted)
{
    if (priorityHighlighted == highlighted) {
        return;
    }
    priorityHighlighted = highlighted;
    update();
}

AbstractCounter *PlayerTarget::addCounter(CounterState *state)
{
    if (playerCounter) {
        disconnect(playerCounter, nullptr, this, nullptr);
        playerCounter->delCounter();
    }
    playerCounter = new PlayerCounter(state, owner, this);
    playerCounter->setPos(boundingRect().width() - playerCounter->boundingRect().width(),
                          boundingRect().height() - playerCounter->boundingRect().height());
    connect(playerCounter, &PlayerCounter::destroyed, this, &PlayerTarget::counterDeleted);
    return playerCounter;
}

void PlayerTarget::counterDeleted()
{
    playerCounter = nullptr;
}

void PlayerTarget::mousePressEvent(QGraphicsSceneMouseEvent *event)
{
    if (owner && owner->getGame()) {
        auto *playerManager = owner->getGame()->getPlayerManager();
        if (playerManager) {
            PlayerLogic *localPlayer = playerManager->getPlayers().value(playerManager->getLocalPlayerId(), nullptr);
            if (localPlayer && RuledActions::actions(localPlayer)) {
                RuledPlayerActions *actions = RuledActions::actions(localPlayer);
                // Right-click reduces this player's damage allocation. Qt only routes the release
                // to the item that accepted the matching press, so the right press must be accepted
                // here or the decrement release in mouseReleaseEvent would never arrive (a context
                // menu would grab it instead — the reason creatures decremented but players didn't).
                if (event->button() == Qt::RightButton && actions->isInSpellDamageAllocationMode()) {
                    event->accept();
                    return;
                }
                if (event->button() == Qt::LeftButton) {
                    if (RuledActions::isCombatDefenderPlayerCandidate(owner)) {
                        setCursor(Qt::CrossCursor);
                        event->accept();
                        return;
                    }
                    const auto eligibility = actions->ruledPlayerTargetEligibility(owner);
                    if (eligibility != RuledTargetClickEligibility::NotTargeting) {
                        setCursor(eligibility == RuledTargetClickEligibility::Legal ? Qt::CrossCursor
                                                                                  : Qt::ArrowCursor);
                        event->accept();
                        return;
                    }
                    if (actions->isInSpellDamageAllocationMode()) {
                        event->accept();
                        return;
                    }
                }
            }
        }
    }
    ArrowTarget::mousePressEvent(event);
}

void PlayerTarget::mouseReleaseEvent(QGraphicsSceneMouseEvent *event)
{
    if (owner && owner->getGame()) {
        auto *playerManager = owner->getGame()->getPlayerManager();
        if (playerManager) {
            PlayerLogic *localPlayer = playerManager->getPlayers().value(playerManager->getLocalPlayerId(), nullptr);
            RuledPlayerActions *actions = localPlayer ? RuledActions::actions(localPlayer) : nullptr;
            if (event->button() == Qt::LeftButton && actions) {
                if (RuledActions::tryHandleCombatDefenderPlayerClick(owner)) {
                    event->accept();
                    return;
                }
                const auto eligibility = actions->ruledPlayerTargetEligibility(owner);
                if (eligibility == RuledTargetClickEligibility::Illegal) {
                    setCursor(Qt::ArrowCursor);
                    event->accept();
                    return;
                }
                // Spell damage allocation takes priority: left-click increments this player's allocation.
                if (actions->tryBumpSpellDamageAllocationForPlayer(owner, +1)) {
                    event->accept();
                    return;
                }
                if (actions->tryHandleRuledAbilityTargetPlayerClick(owner)) {
                    event->accept();
                    return;
                }
                if (actions->tryHandleRuledSpellTargetPlayerClick(owner)) {
                    event->accept();
                    return;
                }
            }
            if (event->button() == Qt::RightButton && actions) {
                // Spell damage allocation: right-click decrements this player's allocation.
                if (actions->tryBumpSpellDamageAllocationForPlayer(owner, -1)) {
                    event->accept();
                    return;
                }
            }
        }
    }
    QGraphicsItem::mouseReleaseEvent(event);
}
