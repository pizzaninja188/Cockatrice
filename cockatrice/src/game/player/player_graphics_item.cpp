#include "player_graphics_item.h"

#include "../../interface/widgets/tabs/tab_game.h"
#include "../board/abstract_card_item.h"
#include "../hand_counter.h"
#include "../ruled/ruled_actions.h"
#include "../ruled/ruled_restricted_mana_display.h"
#include "../zones/hand_zone.h"
#include "../zones/pile_zone.h"
#include "../zones/stack_zone.h"
#include "../zones/table_zone.h"

PlayerGraphicsItem::PlayerGraphicsItem(Player *_player) : player(_player)
{
    connect(&SettingsCache::instance(), &SettingsCache::horizontalHandChanged, this,
            &PlayerGraphicsItem::rearrangeZones);
    connect(&SettingsCache::instance(), &SettingsCache::handJustificationChanged, this,
            &PlayerGraphicsItem::rearrangeZones);
    connect(player, &Player::rearrangeCounters, this, &PlayerGraphicsItem::rearrangeCounters);

    playerArea = new PlayerArea(this);

    playerTarget = new PlayerTarget(player, playerArea);

    initializeZones();

    if (RuledActions::isRuledGame(player->getGame())) {
        restrictedManaDisplay = new RuledRestrictedManaDisplay(player, this);
        connect(restrictedManaDisplay, &RuledRestrictedManaDisplay::widthChanged, this,
                &PlayerGraphicsItem::rearrangeZones);
    }

    connect(tableZoneGraphicsItem, &TableZone::sizeChanged, this, &PlayerGraphicsItem::updateBoundingRect);

    updateBoundingRect();

    rearrangeZones();
    retranslateUi();
}

void PlayerGraphicsItem::retranslateUi()
{
    player->getPlayerMenu()->retranslateUi();

    QMapIterator<QString, CardZoneLogic *> zoneIterator(player->getZones());
    while (zoneIterator.hasNext()) {
        emit zoneIterator.next().value()->retranslateUi();
    }

    QMapIterator<int, AbstractCounter *> counterIterator(player->getCounters());
    while (counterIterator.hasNext()) {
        counterIterator.next().value()->retranslateUi();
    }
}

void PlayerGraphicsItem::onPlayerActiveChanged(bool _active)
{
    tableZoneGraphicsItem->setActive(_active);
}

void PlayerGraphicsItem::setPriorityHighlighted(bool highlighted)
{
    playerTarget->setPriorityHighlighted(highlighted);
}

void PlayerGraphicsItem::initializeZones()
{
    deckZoneGraphicsItem = new PileZone(player->getDeckZone(), this);

    sideboardGraphicsItem = new PileZone(player->getSideboardZone(), this);
    player->getSideboardZone()->setGraphicsVisibility(false);

    handCounter = new HandCounter(playerArea);

    graveyardZoneGraphicsItem = new PileZone(player->getGraveZone(), this);

    rfgZoneGraphicsItem = new PileZone(player->getRfgZone(), this);

    tableZoneGraphicsItem = new TableZone(player->getTableZone(), this);
    connect(tableZoneGraphicsItem, &TableZone::sizeChanged, this, &PlayerGraphicsItem::updateBoundingRect);

    stackZoneGraphicsItem =
        new StackZone(player->getStackZone(), static_cast<int>(tableZoneGraphicsItem->boundingRect().height()), this);

    handZoneGraphicsItem =
        new HandZone(player->getHandZone(), static_cast<int>(tableZoneGraphicsItem->boundingRect().height()), this);

    connect(handZoneGraphicsItem->getLogic(), &HandZoneLogic::cardCountChanged, handCounter,
            &HandCounter::updateNumber);
    connect(handCounter, &HandCounter::showContextMenu, handZoneGraphicsItem, &HandZone::showContextMenu);
}

QRectF PlayerGraphicsItem::boundingRect() const
{
    return bRect;
}

qreal PlayerGraphicsItem::getMinimumWidth() const
{
    qreal result = tableZoneGraphicsItem->getMinimumWidth() + CardDimensions::HEIGHT_F + 15 + counterAreaWidth +
                   restrictedManaExtraWidth();
    if (!SettingsCache::instance().getHorizontalHand()) {
        result += handZoneGraphicsItem->boundingRect().width();
    }
    return result;
}

void PlayerGraphicsItem::paint(QPainter * /*painter*/,
                               const QStyleOptionGraphicsItem * /*option*/,
                               QWidget * /*widget*/)
{
}

void PlayerGraphicsItem::processSceneSizeChange(int newPlayerWidth)
{
    // Extend table (and hand, if horizontal) to accommodate the new player width.
    qreal tableWidth = newPlayerWidth - CardDimensions::HEIGHT_F - 15 - counterAreaWidth - restrictedManaExtraWidth();
    if (!SettingsCache::instance().getHorizontalHand()) {
        tableWidth -= handZoneGraphicsItem->boundingRect().width();
    }

    tableZoneGraphicsItem->setWidth(tableWidth);
    handZoneGraphicsItem->setWidth(tableWidth);
}

void PlayerGraphicsItem::setMirrored(bool _mirrored)
{
    if (mirrored != _mirrored) {
        mirrored = _mirrored;
        rearrangeZones();
    }
}

void PlayerGraphicsItem::rearrangeCounters()
{
    qreal marginTop = 80;
    const qreal padding = 5;
    qreal ySize = boundingRect().y() + marginTop;

    // Place objects
    for (const auto &counter : player->getCounters()) {
        AbstractCounter *ctr = counter;

        if (!ctr->getShownInCounterArea()) {
            continue;
        }

        QRectF br = ctr->boundingRect();
        ctr->setPos((counterAreaWidth - br.width()) / 2, ySize);
        ySize += br.height() + padding;
    }
    if (restrictedManaDisplay) {
        restrictedManaDisplay->refresh();
    }
}

void PlayerGraphicsItem::rearrangeZones()
{
    rearrangeSidebar();
    auto base = QPointF(CardDimensions::HEIGHT_F + counterAreaWidth + restrictedManaExtraWidth() + 15, 0);
    stackZoneGraphicsItem->setVisible(false);
    if (SettingsCache::instance().getHorizontalHand()) {
        if (mirrored) {
            if (player->getHandZone()->contentsKnown()) {
                player->getPlayerInfo()->setHandVisible(true);
                handZoneGraphicsItem->setPos(base);
                base += QPointF(0, handZoneGraphicsItem->boundingRect().height());
            } else {
                player->getPlayerInfo()->setHandVisible(false);
            }

            tableZoneGraphicsItem->setPos(base);
        } else {
            tableZoneGraphicsItem->setPos(base.x(), 0);
            base += QPointF(0, tableZoneGraphicsItem->boundingRect().height());

            if (player->getHandZone()->contentsKnown()) {
                player->getPlayerInfo()->setHandVisible(true);
                handZoneGraphicsItem->setPos(base);
            } else {
                player->getPlayerInfo()->setHandVisible(false);
            }
        }
        handZoneGraphicsItem->setWidth(tableZoneGraphicsItem->getWidth());
    } else {
        player->getPlayerInfo()->setHandVisible(true);

        handZoneGraphicsItem->setPos(base);
        base += QPointF(handZoneGraphicsItem->boundingRect().width(), 0);

        tableZoneGraphicsItem->setPos(base);
    }
    handZoneGraphicsItem->setVisible(player->getPlayerInfo()->getHandVisible());
    handZoneGraphicsItem->updateOrientation();
    tableZoneGraphicsItem->reorganizeCards();
    updateBoundingRect();
    rearrangeCounters();
}

void PlayerGraphicsItem::updateBoundingRect()
{
    prepareGeometryChange();
    qreal width = CardDimensions::HEIGHT_F + 15 + counterAreaWidth + restrictedManaExtraWidth();
    if (SettingsCache::instance().getHorizontalHand()) {
        qreal handHeight =
            player->getPlayerInfo()->getHandVisible() ? handZoneGraphicsItem->boundingRect().height() : 0;
        bRect = QRectF(0, 0, width + tableZoneGraphicsItem->boundingRect().width(),
                       tableZoneGraphicsItem->boundingRect().height() + handHeight);
    } else {
        bRect = QRectF(
            0, 0, width + handZoneGraphicsItem->boundingRect().width() + tableZoneGraphicsItem->boundingRect().width(),
            tableZoneGraphicsItem->boundingRect().height());
    }
    playerArea->setSize(CardDimensions::HEIGHT_F + counterAreaWidth + restrictedManaExtraWidth() + 15, bRect.height());

    emit sizeChanged();
}

qreal PlayerGraphicsItem::restrictedManaExtraWidth() const
{
    return restrictedManaDisplay ? restrictedManaDisplay->displayWidth() : 0;
}

void PlayerGraphicsItem::rearrangeSidebar()
{
    const qreal counterColumnsWidth = counterAreaWidth + restrictedManaExtraWidth();
    const qreal avatarMarginX =
        (counterColumnsWidth + CardDimensions::HEIGHT_F + 15 - playerTarget->boundingRect().width()) / 2.0;
    const qreal avatarMarginY =
        (counterAreaWidth + CardDimensions::HEIGHT_F + 15 - playerTarget->boundingRect().width()) / 2.0;
    playerTarget->setPos(QPointF(avatarMarginX, avatarMarginY));

    const QPointF base(counterColumnsWidth + (CardDimensions::HEIGHT_F - CardDimensions::WIDTH_F + 15) / 2.0,
                       10 + playerTarget->boundingRect().height() + 5 -
                           (CardDimensions::HEIGHT_F - CardDimensions::WIDTH_F) / 2.0);
    const qreal pileStep = deckZoneGraphicsItem->boundingRect().width() + 5;
    const qreal handCounterHeight = handCounter->boundingRect().height();
    deckZoneGraphicsItem->setPos(base);
    handCounter->setPos(base + QPointF(0, pileStep + 10));
    graveyardZoneGraphicsItem->setPos(base + QPointF(0, pileStep + handCounterHeight + 10));
    rfgZoneGraphicsItem->setPos(base + QPointF(0, 2 * pileStep + handCounterHeight + 10));
    if (restrictedManaDisplay) {
        restrictedManaDisplay->setPos(counterAreaWidth, 0);
    }
}
