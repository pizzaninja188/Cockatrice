#include "ruled_restricted_mana_display.h"

#include "../../interface/pixel_map_generator.h"
#include "../abstract_game.h"
#include "../board/abstract_counter.h"
#include "../game_event_handler.h"
#include "../player/player.h"
#include "../player/player_actions.h"
#include "ruled_client_state.h"
#include "ruled_restricted_mana_model.h"

#include <QGraphicsSceneMouseEvent>
#include <QPainter>
#include <algorithm>

namespace
{
constexpr qreal COLUMN_WIDTH = 55.0;
constexpr qreal DEFAULT_COUNTER_SIZE = 40.0;
constexpr qreal DEFAULT_FIRST_ROW_Y = 80.0;
constexpr qreal DEFAULT_ROW_STEP = 45.0;

class RestrictedManaPip final : public QGraphicsObject
{
public:
    RestrictedManaPip(Player *player,
                      quint32 groupId,
                      QChar symbol,
                      int count,
                      const QString &label,
                      qreal size,
                      QGraphicsItem *parent)
        : QGraphicsObject(parent), player(player), groupId(groupId), symbol(symbol), count(count), size(size)
    {
        setAcceptHoverEvents(true);
        setCursor(player->getPlayerInfo()->getLocal() ? Qt::PointingHandCursor : Qt::ArrowCursor);
        setToolTip(label);
    }

    [[nodiscard]] QRectF boundingRect() const override
    {
        return QRectF(0, 0, size, size);
    }

    void paint(QPainter *painter, const QStyleOptionGraphicsItem *, QWidget *) override
    {
        const auto *actions = player->getPlayerActions();
        const bool eligible =
            !actions->ruledRestrictedManaPaymentPending() || actions->ruledRestrictedManaGroupEligible(groupId);
        painter->save();
        painter->setOpacity(eligible ? 1.0 : 0.38);
        const QString iconName = symbol == QLatin1Char('C') ? QStringLiteral("x") : QString(symbol.toLower());
        painter->drawPixmap(boundingRect().toRect(),
                            CounterPixmapGenerator::generatePixmap(static_cast<int>(size), iconName, hovered));

        QPen restrictedPen(QColor(45, 45, 45, 220), 2, Qt::DashLine);
        painter->setPen(restrictedPen);
        painter->setBrush(Qt::NoBrush);
        painter->drawEllipse(boundingRect().adjusted(2, 2, -2, -2));

        if (count > 0) {
            QFont countFont(QStringLiteral("Serif"));
            countFont.setPixelSize(qMax(static_cast<int>(size / 2), 10));
            countFont.setWeight(QFont::Bold);
            painter->setFont(countFont);
            painter->setPen(Qt::black);
            painter->drawText(boundingRect(), Qt::AlignCenter, QString::number(count));
        }

        QFont lockFont;
        lockFont.setPixelSize(qMax(static_cast<int>(size / 4), 8));
        painter->setFont(lockFont);
        painter->setPen(QColor(25, 25, 25));
        painter->drawText(boundingRect().adjusted(2, 0, -2, -2), Qt::AlignRight | Qt::AlignBottom,
                          QString::fromUtf8("\xF0\x9F\x94\x92"));
        painter->restore();
    }

protected:
    void hoverEnterEvent(QGraphicsSceneHoverEvent *event) override
    {
        hovered = true;
        update();
        QGraphicsObject::hoverEnterEvent(event);
    }

    void hoverLeaveEvent(QGraphicsSceneHoverEvent *event) override
    {
        hovered = false;
        update();
        QGraphicsObject::hoverLeaveEvent(event);
    }

    void mousePressEvent(QGraphicsSceneMouseEvent *event) override
    {
        if (event->button() == Qt::LeftButton && player->getPlayerInfo()->getLocal() &&
            player->getPlayerActions()->tryPayRuledRestrictedMana(groupId, symbol)) {
            event->accept();
            return;
        }
        QGraphicsObject::mousePressEvent(event);
    }

private:
    Player *player;
    quint32 groupId;
    QChar symbol;
    int count;
    qreal size;
    bool hovered = false;
};

struct CounterRow
{
    qreal y = DEFAULT_FIRST_ROW_Y;
    qreal size = DEFAULT_COUNTER_SIZE;
};

CounterRow rowForSymbol(Player *player, QChar symbol)
{
    const QChar wanted = symbol == QLatin1Char('C') ? QLatin1Char('X') : symbol;
    for (const auto *counter : player->getCounters()) {
        if (counter && counter->getShownInCounterArea() && counter->getName().trimmed().toUpper() == wanted) {
            return {counter->pos().y(), counter->boundingRect().width()};
        }
    }
    const int index = QStringLiteral("WUBRGC").indexOf(symbol);
    return {DEFAULT_FIRST_ROW_Y + qMax(index, 0) * DEFAULT_ROW_STEP, DEFAULT_COUNTER_SIZE};
}
} // namespace

RuledRestrictedManaDisplay::RuledRestrictedManaDisplay(Player *_player, QGraphicsItem *parent)
    : QGraphicsObject(parent), player(_player)
{
    auto *state = player->getGame()->getGameEventHandler()->ruled();
    connect(state, &RuledClientState::restrictedManaChanged, this, [this](int playerId) {
        if (playerId == player->getPlayerInfo()->getId()) {
            refresh();
        }
    });
    connect(state, &RuledClientState::legalActionsChanged, this, &RuledRestrictedManaDisplay::refresh);
    connect(state, &RuledClientState::sessionReset, this, &RuledRestrictedManaDisplay::refresh);
    connect(player->getPlayerActions(), &PlayerActions::ruledRestrictedManaStagingChanged, this,
            &RuledRestrictedManaDisplay::refresh, Qt::QueuedConnection);
    connect(player->getPlayerActions(), &PlayerActions::ruledSpellManaPromptChanged, this,
            &RuledRestrictedManaDisplay::refresh, Qt::QueuedConnection);
    connect(player->getPlayerActions(), &PlayerActions::ruledAbilityManaPromptChanged, this,
            &RuledRestrictedManaDisplay::refresh, Qt::QueuedConnection);
    refresh();
}

QRectF RuledRestrictedManaDisplay::boundingRect() const
{
    return bounds;
}

qreal RuledRestrictedManaDisplay::displayWidth() const
{
    return bounds.width();
}

void RuledRestrictedManaDisplay::paint(QPainter *painter, const QStyleOptionGraphicsItem *, QWidget *)
{
    Q_UNUSED(painter);
}

void RuledRestrictedManaDisplay::refresh()
{
    const qreal oldWidth = bounds.width();
    const auto children = childItems();
    for (QGraphicsItem *child : children) {
        delete child;
    }

    auto *state = player->getGame()->getGameEventHandler()->ruled();
    auto groups =
        state ? state->restrictedManaForPlayer(player->getPlayerInfo()->getId()) : QVector<RuledRestrictedManaGroup>{};
    std::sort(groups.begin(), groups.end(),
              [](const auto &left, const auto &right) { return left.groupId < right.groupId; });

    RuledRestrictedManaSelections stagedSelections;
    for (const auto &group : groups) {
        for (const QChar symbol : QStringLiteral("WUBRGC")) {
            stagedSelections[group.groupId][symbol] =
                player->getPlayerActions()->ruledRestrictedManaOptimisticSpendCount(group.groupId, symbol);
        }
    }

    qreal bottom = 0;
    int column = 0;
    for (const auto &group : groups) {
        bool groupVisible = false;
        for (const QChar symbol : QStringLiteral("WUBRGC")) {
            const int authoritative = group.countForSymbol(symbol);
            const int staged = stagedSelections.value(group.groupId).value(symbol);
            const int visible = qMax(0, authoritative - staged);
            if (visible == 0) {
                continue;
            }
            groupVisible = true;
            const CounterRow row = rowForSymbol(player, symbol);
            auto *pip =
                new RestrictedManaPip(player, group.groupId, symbol, visible, group.displayLabel, row.size, this);
            pip->setPos(column * COLUMN_WIDTH + (COLUMN_WIDTH - row.size) / 2.0, row.y);
            bottom = qMax(bottom, row.y + row.size);
        }
        column += groupVisible ? 1 : 0;
    }

    prepareGeometryChange();
    bounds = QRectF(0, 0, ruledVisibleRestrictedManaColumnCount(groups, stagedSelections) * COLUMN_WIDTH,
                    qMax(bottom + 5, DEFAULT_FIRST_ROW_Y));
    update();
    if (!qFuzzyCompare(oldWidth + 1.0, bounds.width() + 1.0)) {
        emit widthChanged();
    }
}
