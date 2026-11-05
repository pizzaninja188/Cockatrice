#include <libcockatrice/game/game_state.h>
#include <libcockatrice/game/game_event_handler.h>
#include <libcockatrice/game/player/player_manager.h>
#include <libcockatrice/game/board/counter_state.h>
#include "table_zone.h"

#include "../../game/ruled/ruled_actions.h"
#include "../../client/settings/cache_settings.h"
#include "../../interface/theme_manager.h"
#include <libcockatrice/game/abstract_game.h>
#include "../board/arrow_item.h"
#include "../board/card_drag_item.h"
#include "../board/card_item.h"
#include <libcockatrice/game/player/player_logic.h>
#include "../../game/ruled/ruled_player_actions.h"
#include "../../game/ruled/ruled_client_state.h"
#include "../board/abstract_counter.h"
#include "../board/abstract_card_item.h"
#include "../board/card_drag_item.h"
#include "../board/card_item.h"
#include "../game_scene.h"
#include "../z_values.h"
#include "libcockatrice/card/printing/exact_card.h"
#include "libcockatrice/protocol/pb/card_attributes.pb.h"

#include <QBrush>
#include <QColor>
#include <QGradient>
#include <QGraphicsItem>
#include <QGraphicsScene>
#include <QLinearGradient>
#include <QList>
#include <QListIterator>
#include <QPainter>
#include <QChar>
#include <QRegularExpression>
#include <QRegularExpressionMatch>
#include <QString>
#include <libcockatrice/card/card_info.h>
#include <libcockatrice/protocol/pb/command_inc_counter.pb.h>
#include <libcockatrice/protocol/pb/command_move_card.pb.h>
#include <libcockatrice/protocol/pb/command_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/command_set_card_attr.pb.h>
#include <QRectF>
#include <QSharedPointer>
#include <QString>
#include <QtGlobal>
#include <algorithm>
#include <iterator>
#include <libcockatrice/card/card_info.h>
#include <libcockatrice/game/board/card_list.h>
#include <libcockatrice/game/player/player_actions.h>
#include <libcockatrice/game/player/player_info.h>
#include <libcockatrice/game/player/player_logic.h>
#include <libcockatrice/game/zones/card_zone_logic.h>
#include <libcockatrice/game/zones/table_zone_logic.h>
#include <libcockatrice/settings/interface_settings.h>
#include <libcockatrice/utility/zone_names.h>
#include <string>

namespace google
{
namespace protobuf
{
class Message;
} // namespace protobuf
} // namespace google

const QColor TableZone::BACKGROUND_COLOR = QColor(100, 100, 100);
const QColor TableZone::FADE_MASK = QColor(0, 0, 0, 80);
const QColor TableZone::GRADIENT_COLOR = QColor(255, 255, 255, 150);
const QColor TableZone::GRADIENT_COLORLESS = QColor(255, 255, 255, 0);

static QString manaCounterNameFromSymbol(const QChar symbol)
{
    switch (symbol.toUpper().toLatin1()) {
        case 'W':
            return "w";
        case 'U':
            return "u";
        case 'B':
            return "b";
        case 'R':
            return "r";
        case 'G':
            return "g";
        case 'C':
            return "x";
        case 'X':
            return "x";
        default:
            return {};
    }
}

static QString inferLandManaCounterName(const CardItem *card)
{
    if (!card || card->getFaceDown()) {
        return {};
    }
    const QString cardType = card->getCardInfo().getCardType();
    if (!cardType.contains("Land", Qt::CaseInsensitive)) {
        return {};
    }

    // Prefer explicit mana symbols from tap abilities like "{T}: Add {G}.".
    static const QRegularExpression tapManaSymbolRegex(
        R"(\{T\}[^\n]*?\badd\b[^\n]*?\{([WUBRGCX])\})", QRegularExpression::CaseInsensitiveOption);
    const QRegularExpressionMatch match = tapManaSymbolRegex.match(card->getCardInfo().getText());
    if (match.hasMatch()) {
        return manaCounterNameFromSymbol(match.captured(1).at(0));
    }

    // Fall back to basic land subtypes when rules text does not expose a simple symbol.
    if (cardType.contains("Plains", Qt::CaseInsensitive)) {
        return "w";
    }
    if (cardType.contains("Island", Qt::CaseInsensitive)) {
        return "u";
    }
    if (cardType.contains("Swamp", Qt::CaseInsensitive)) {
        return "b";
    }
    if (cardType.contains("Mountain", Qt::CaseInsensitive)) {
        return "r";
    }
    if (cardType.contains("Forest", Qt::CaseInsensitive)) {
        return "g";
    }
    return {};
}

static int findCounterIdByName(PlayerLogic *player, const QString &counterName)
{
    if (!player || counterName.isEmpty()) {
        return -1;
    }
    const auto counters = player->getCounters();
    for (auto it = counters.constBegin(); it != counters.constEnd(); ++it) {
        if (it.value() != nullptr && it.value()->getName().compare(counterName, Qt::CaseInsensitive) == 0) {
            return it.key();
        }
    }
    return -1;
}

TableZone::TableZone(TableZoneLogic *_logic, bool _mirrored, QGraphicsItem *parent)
    : SelectZone(_logic, parent), active(false), mirrored(_mirrored)
{
    connect(_logic, &TableZoneLogic::contentSizeChanged, this, &TableZone::resizeToContents);
    connect(_logic, &TableZoneLogic::toggleTapped, this, &TableZone::toggleTapped);
    connect(themeManager, &ThemeManager::themeChanged, this, &TableZone::updateBg);
    connect(&SettingsCache::instance().userInterface(), &InterfaceSettings::invertVerticalCoordinateChanged, this,
            &TableZone::reorganizeCards);

    updateBg();

    height = MARGIN_TOP + MARGIN_BOTTOM + TableZoneLogic::ROW_COUNT * CardDimensions::HEIGHT +
             (TableZoneLogic::ROW_COUNT - 1) * PADDING_Y;
    width = MIN_WIDTH;
    currentMinimumWidth = width;

    setCacheMode(DeviceCoordinateCache);
    setAcceptHoverEvents(true);
}

void TableZone::updateBg()
{
    update();
}

void TableZone::triggerDamageShimmer()
{
    if (!SettingsCache::instance().userInterface().getBattlefieldFlashEnabled()) {
        damageShimmerAlpha = 0.0;
        return;
    }

    damageShimmerAlpha = 1.0;
    shimmerClock.start();
    if (scene()) {
        static_cast<GameScene *>(scene())->registerAnimationItem(this);
    }
}

bool TableZone::animationEvent()
{
    damageShimmerAlpha = 1.0 - shimmerClock.elapsed() / shimmerDurationMs;
    if (damageShimmerAlpha <= 0.0) {
        damageShimmerAlpha = 0.0;
        return false;
    }
    update();
    return true;
}

QRectF TableZone::boundingRect() const
{
    return QRectF(0, 0, width, height);
}

void TableZone::setMirrored(bool isMirrored)
{
    mirrored = isMirrored;
    update();
}

bool TableZone::isInverted() const
{
    return ((mirrored && !SettingsCache::instance().userInterface().getInvertVerticalCoordinate()) ||
            (!mirrored && SettingsCache::instance().userInterface().getInvertVerticalCoordinate()));
}

void TableZone::paint(QPainter *painter, const QStyleOptionGraphicsItem * /*option*/, QWidget * /*widget*/)
{
    if (playmatActive) {
        // Subtle overlay to distinguish table zone from stack zone
        painter->fillRect(boundingRect(), QColor(0, 0, 0, 60));
    } else {
        QBrush brush = themeManager->getExtraBgBrush(ThemeManager::Table, getLogic()->getPlayer()->getZoneId());
        painter->fillRect(boundingRect(), brush);
    }

    if (active) {
        paintZoneOutline(painter);
    } else {
        // inactive player gets a darker table zone with a semi transparent black mask
        // this means if the user provides a custom background or playmat it will fade
        painter->fillRect(boundingRect(), FADE_MASK);
    }

    // Decaying crimson wash from taking damage.
    if (damageShimmerAlpha > 0.0) {
        QColor shimmerColor(239, 68, 68);
        shimmerColor.setAlphaF(0.22 * damageShimmerAlpha);
        painter->fillRect(boundingRect(), shimmerColor);
    }

    paintLandDivider(painter);
}

void TableZone::onPlaymatChanged(bool active)
{
    playmatActive = active;
    // While a playmat is shown the zone paints a translucent overlay over the
    // dynamic playmat behind it. Keep it out of the device cache so the cached
    // pixels are never stale relative to the playmat (and to avoid compositing
    // artifacts of cached translucent content on some platforms).
    setCacheMode(active ? QGraphicsItem::NoCache : QGraphicsItem::DeviceCoordinateCache);
    update();
}

/**
   Render a soft outline around the edge of the TableZone.

   @param painter QPainter object
 */
void TableZone::paintZoneOutline(QPainter *painter)
{
    QLinearGradient grad1(0, 0, 0, 1);
    grad1.setCoordinateMode(QGradient::ObjectBoundingMode);
    grad1.setColorAt(0, GRADIENT_COLOR);
    grad1.setColorAt(1, GRADIENT_COLORLESS);
    painter->fillRect(QRectF(0, 0, width, BOX_LINE_WIDTH), QBrush(grad1));

    grad1.setFinalStop(1, 0);
    painter->fillRect(QRectF(0, 0, BOX_LINE_WIDTH, height), QBrush(grad1));

    grad1.setStart(0, 1);
    grad1.setFinalStop(0, 0);
    painter->fillRect(QRectF(0, height - BOX_LINE_WIDTH, width, BOX_LINE_WIDTH), QBrush(grad1));

    grad1.setStart(1, 0);
    painter->fillRect(QRectF(width - BOX_LINE_WIDTH, 0, BOX_LINE_WIDTH, height), QBrush(grad1));
}

/**
   Render a division line for land placement

   @painter QPainter object
 */
void TableZone::paintLandDivider(QPainter *painter)
{
    // Place the line 2 grid heights down then back it off just enough to allow
    // some space between a 3-card stack and the land area.
    qreal separatorY = MARGIN_TOP + 2 * (CardDimensions::HEIGHT + PADDING_Y) - STACKED_CARD_OFFSET_Y / 2;
    if (isInverted()) {
        separatorY = height - separatorY;
    }
    painter->setPen(QColor(255, 255, 255, 40));
    painter->drawLine(QPointF(0, separatorY), QPointF(width, separatorY));
}

void TableZone::handleDropEvent(const QList<CardDragItem *> &dragItems,
                                CardZoneLogic *startZone,
                                const QPoint &dropPoint)
{
    handleDropEventByGrid(dragItems, startZone, mapToGrid(dropPoint));
}

void TableZone::handleDropEventByGrid(const QList<CardDragItem *> &dragItems,
                                      CardZoneLogic *startZone,
                                      const QPoint &gridPoint)
{
    QList<CardMoveRequest> cards;
    cards.reserve(dragItems.size());
    for (const CardDragItem *item : dragItems) {
        CardMoveRequest card{item->getId(), item->isForceFaceDown()};
        if (startZone->getName() != getLogic()->getName() && !item->isForceFaceDown()) {
            const auto &cardInfo = item->getItem()->getCard();
            if (cardInfo) {
                card.pt = cardInfo.getInfo().getPowTough();
            }
        }
        cards.append(card);
    }

    startZone->getPlayer()->getPlayerActions()->moveCards(startZone, getLogic(), gridPoint.x(), gridPoint.y(), cards);
}

void TableZone::reorganizeCards()
{
    // Calculate card stack widths so mapping functions work properly
    computeCardStackWidths();

    for (int i = 0; i < cardItems().size(); ++i) {
        QPoint gridPoint = cardItems()[i]->getGridPos();
        if (gridPoint.x() == -1) {
            continue;
        }

        QPointF mapPoint = mapFromGrid(gridPoint);
        qreal x = mapPoint.x();
        qreal y = mapPoint.y();

        int numberAttachedCards = cardItems()[i]->getAttachedCards().size();
        qreal actualX = x + numberAttachedCards * STACKED_CARD_OFFSET_X;
        qreal actualY = y;
        if (numberAttachedCards) {
            actualY += 15;
        }

        cardItems()[i]->setPos(actualX, actualY);
        cardItems()[i]->setRealZValue(ZValues::tableCardZValue(actualX, actualY));

        QListIterator<CardState *> attachedCardIterator(cardItems()[i]->getAttachedCards());
        int j = 0;
        while (attachedCardIterator.hasNext()) {
            ++j;
            CardItem *attachedCard = qobject_cast<CardItem *>(attachedCardIterator.next()->parent());
            if (attachedCard == nullptr) {
                continue;
            }
            qreal childX = actualX - j * STACKED_CARD_OFFSET_X;
            qreal childY = y + 5;
            attachedCard->setPos(childX, childY);
            attachedCard->setRealZValue(ZValues::tableCardZValue(childX, childY));
        }
    }

    resizeToContents();
    update();
}

void TableZone::toggleTapped()
{
    QList<QGraphicsItem *> selectedItemsRaw = scene()->selectedItems();
    QList<QGraphicsItem *> selectedItems;

    auto isCardOnTable = [](const QGraphicsItem *item) {
        if (auto card = qgraphicsitem_cast<const CardItem *>(item)) {
            return card->getZone()->getName() == ZoneNames::TABLE;
        }
        return false;
    };

    std::copy_if(selectedItemsRaw.begin(), selectedItemsRaw.end(), std::back_inserter(selectedItems), isCardOnTable);

    bool tapAll = std::any_of(selectedItems.begin(), selectedItems.end(), [](const QGraphicsItem *item) {
        return !qgraphicsitem_cast<const CardItem *>(item)->getTapped();
    });
    if (!tapAll) {
        return;
    }

    QList<const ::google::protobuf::Message *> cmdList;
    const bool ruledGame = RuledActions::isRuledGame(getLogic()->getPlayer()->getGame());
    const int localPlayerId = getLogic()->getPlayer()->getGame()->getPlayerManager()->getLocalPlayerId();
    const int priorityPlayer = getLogic()->getPlayer()->getGame()->getGameState()->getPriorityPlayer();
    if (ruledGame && localPlayerId != priorityPlayer) {
        return;
    }
    if (ruledGame && RuledActions::gameplayInputLocked(getLogic()->getPlayer()->getGame())) {
        return;
    }
    if (ruledGame) {
        const RuledClientState *handler = getLogic()->getPlayer()->getGame()->getGameEventHandler()->ruled();
        const auto combatPhase = handler->getCombatPhase();
        const bool mustDeclareAttackers =
            combatPhase == RuledClientState::RuledCombatPhase::DeclareAttackers && handler->localPlayerIsActive();
        const bool mustDeclareBlockers =
            combatPhase == RuledClientState::RuledCombatPhase::DeclareBlockers && handler->localPlayerIsDefender();
        if (mustDeclareAttackers || mustDeclareBlockers) {
            return;
        }
    }
    RuledPlayerActions *const playerActions = RuledActions::actions(getLogic()->getPlayer());
    for (const auto &selectedItem : selectedItems) {
        CardItem *temp = qgraphicsitem_cast<CardItem *>(selectedItem);
        const bool wasTapped = temp->getTapped();
        if (wasTapped == tapAll) {
            continue;
        }

        if (ruledGame) {
            // CR 605/106: the engine owns tap state and the mana pool. Tapping a land here means
            // activating its mana ability — the engine taps the source and floats the mana, which
            // come back via zone_view (battlefield_tapped) and ManaPoolUpdated. The client never
            // taps locally, mints mana, or pokes counters. Tapping only ever *produces* mana; paying
            // a pending spell/ability is a separate step (click the pool counters), so the produced
            // mana shows on the pool counter and can be undone via the engine's UndoManaAbility.
            if (!(tapAll && !wasTapped)) {
                continue;
            }
            const QString manaCounterName = inferLandManaCounterName(temp);
            const QChar desiredColor = manaCounterName.isEmpty() ? QChar() : manaCounterName.at(0);
            if (Command_RuledPayload *activate =
                    playerActions->newRuledPayloadActivateManaAbilityForLand(temp, desiredColor)) {
                cmdList.append(activate);
            }
            continue;
        }

        // Freeform (legacy): the client taps the card locally and bumps its own mana counter.
        Command_SetCardAttr *cmd = new Command_SetCardAttr;
        cmd->set_zone(getLogic()->getName().toStdString());
        cmd->set_card_id(temp->getId());
        cmd->set_attribute(AttrTapped);
        cmd->set_attr_value(tapAll ? "1" : "0");
        cmdList.append(cmd);
        temp->setTapped(tapAll, true);

        if (tapAll && !wasTapped) {
            const QString manaCounterName = inferLandManaCounterName(temp);
            const int manaCounterId = findCounterIdByName(getLogic()->getPlayer(), manaCounterName);
            if (manaCounterId >= 0) {
                auto *counterCmd = new Command_IncCounter;
                counterCmd->set_counter_id(manaCounterId);
                counterCmd->set_delta(1);
                cmdList.append(counterCmd);

                if (auto *counter = getLogic()->getPlayer()->getCounters().value(manaCounterId, nullptr)) {
                    counter->setValue(counter->getValue() + 1);
                }

                playerActions->recordLandTapUndo(temp->getId(), manaCounterName, manaCounterId);
            }
        }
    }
    if (!cmdList.isEmpty()) {
        playerActions->sendGameCommand(playerActions->prepareGameCommand(cmdList));
    }
}

void TableZone::resizeToContents()
{
    int xMax = 0;

    // Find rightmost card position, which includes the left margin amount.
    for (int i = 0; i < cardItems().size(); ++i) {
        if (cardItems()[i]->pos().x() > xMax) {
            xMax = (int)cardItems()[i]->pos().x();
        }
    }

    // Minimum width is the rightmost card position plus enough room for
    // another card with padding, then margin.
    currentMinimumWidth = xMax + (2 * CardDimensions::WIDTH) + PADDING_X + MARGIN_RIGHT;

    if (currentMinimumWidth < MIN_WIDTH) {
        currentMinimumWidth = MIN_WIDTH;
    }

    if (currentMinimumWidth != width) {
        prepareGeometryChange();
        width = currentMinimumWidth;
        emit sizeChanged();
    }
}

CardItem *TableZone::getCardFromGrid(const QPoint &gridPoint) const
{
    for (int i = 0; i < cardItems().size(); i++) {
        if (cardItems().at(i)->getGridPoint() == gridPoint) {
            return cardItems().at(i);
        }
    }
    return 0;
}

CardItem *TableZone::getCardFromCoords(const QPointF &point) const
{
    QPoint gridPoint = mapToGrid(point);
    return getCardFromGrid(gridPoint);
}

void TableZone::computeCardStackWidths()
{
    // Each card stack is three grid points worth of card locations.
    // First pass: compute the number of cards at each card stack.
    QMap<int, int> cardStackCount;
    for (int i = 0; i < cardItems().size(); ++i) {
        const QPoint &gridPoint = cardItems()[i]->getGridPos();
        if (gridPoint.x() == -1) {
            continue;
        }

        const int key = getCardStackMapKey(gridPoint.x() / 3, gridPoint.y());
        cardStackCount.insert(key, cardStackCount.value(key, 0) + 1);
    }

    // Second pass: compute the width at each card stack.
    cardStackWidth.clear();
    for (int i = 0; i < cardItems().size(); ++i) {
        const QPoint &gridPoint = cardItems()[i]->getGridPos();
        if (gridPoint.x() == -1) {
            continue;
        }

        const int key = getCardStackMapKey(gridPoint.x() / 3, gridPoint.y());
        const int stackCount = cardStackCount.value(key, 0);
        if (stackCount == 1) {
            cardStackWidth.insert(key, CardDimensions::WIDTH +
                                           cardItems()[i]->getAttachedCards().size() * STACKED_CARD_OFFSET_X);
        } else {
            cardStackWidth.insert(key, CardDimensions::WIDTH + (stackCount - 1) * STACKED_CARD_OFFSET_X);
        }
    }
}

QPointF TableZone::mapFromGrid(QPoint gridPoint) const
{
    qreal x, y;

    // Start with margin plus stacked card offset
    x = MARGIN_LEFT + (gridPoint.x() % 3) * STACKED_CARD_OFFSET_X;

    // Add in width of card stack plus padding for each column
    for (int i = 0; i < gridPoint.x() / 3; ++i) {
        const int key = getCardStackMapKey(i, gridPoint.y());
        x += cardStackWidth.value(key, CardDimensions::WIDTH) + PADDING_X;
    }

    if (isInverted()) {
        gridPoint.setY(TableZoneLogic::ROW_COUNT - 1 - gridPoint.y());
    }

    // Start with margin plus stacked card offset
    y = MARGIN_TOP + (gridPoint.x() % 3) * STACKED_CARD_OFFSET_Y;

    // Add in card size and padding for each row
    for (int i = 0; i < gridPoint.y(); ++i) {
        y += CardDimensions::HEIGHT + PADDING_Y;
    }

    return QPointF(x, y);
}

QPoint TableZone::mapToGrid(const QPointF &mapPoint) const
{
    // Begin by calculating the y-coordinate of the grid space, which will be
    // used for the x-coordinate.

    // Offset point by the margin amount to reference point within grid area.
    int y = mapPoint.y() - MARGIN_TOP;

    // Below calculation effectively rounds to the nearest grid point.
    const int gridPointHeight = CardDimensions::HEIGHT + PADDING_Y;
    int gridPointY = (y + PADDING_Y / 2) / gridPointHeight;

    gridPointY = TableZoneLogic::clampValidTableRow(gridPointY);

    if (isInverted()) {
        gridPointY = TableZoneLogic::ROW_COUNT - 1 - gridPointY;
    }

    // Calculating the x-coordinate of the grid space requires adding up the
    // widths of each card stack along the row.

    // Offset point by the margin amount to reference point within grid area.
    int x = mapPoint.x() - MARGIN_LEFT + PADDING_X / 2;

    // Maximum value is a card width from the right margin, referenced to the
    // grid area.
    const int xMax = width - MARGIN_LEFT - MARGIN_RIGHT - CardDimensions::WIDTH;

    int xStack = 0;
    int xNextStack = 0;
    int nextStackCol = 0;
    while ((xNextStack <= x) && (xNextStack <= xMax)) {
        xStack = xNextStack;
        const int key = getCardStackMapKey(nextStackCol, gridPointY);
        xNextStack += cardStackWidth.value(key, CardDimensions::WIDTH) + PADDING_X;
        nextStackCol++;
    }
    int stackCol = qMax(nextStackCol - 1, 0);

    // Have the stack column, need to refine to the grid column.  Take the
    // difference between the point and the stack point and divide by stacked
    // card offsets.
    int xDiff = x - xStack;
    int gridPointX = stackCol * 3 + qMin(xDiff / STACKED_CARD_OFFSET_X, 2);

    return QPoint(gridPointX, gridPointY);
}

QPointF TableZone::closestGridPoint(const QPointF &point)
{
    QPoint gridPoint = mapToGrid(point);
    gridPoint.setX((gridPoint.x() / 3) * 3);
    if (getCardFromGrid(gridPoint)) {
        gridPoint.setX(gridPoint.x() + 1);
    }
    if (getCardFromGrid(gridPoint)) {
        gridPoint.setX(gridPoint.x() + 1);
    }
    return mapFromGrid(gridPoint);
}
