#include "view_zone.h"
#include "../../game/ruled/ruled_preparation_display.h"

#include "../../game/ruled/ruled_actions.h"
#include "../../game/ruled/ruled_zone_view_policy.h"
#include "../board/card_drag_item.h"
#include "../board/card_item.h"
#include <libcockatrice/game/abstract_game.h>
#include <libcockatrice/game/game_event_handler.h>
#include "../../game/ruled/ruled_client_state.h"

#include <algorithm>
#include <climits>
#include <libcockatrice/game/player/player_logic.h>
#include "../../game/ruled/ruled_player_actions.h"
#include <libcockatrice/game/zones/view_zone_logic.h>
#include "../../client/settings/cache_settings.h"
#include "../board/card_drag_item.h"
#include "../board/card_item.h"
#include "../card_dimensions.h"
#include "libcockatrice/card/printing/exact_card.h"
#include "libcockatrice/filters/filter_string.h"
#include "libcockatrice/protocol/pb/response.pb.h"
#include "libcockatrice/protocol/pb/serverinfo_zone.pb.h"
#include "libcockatrice/utility/card_ref.h"

#include "../card_dimensions.h"
#include <QBrush>
#include <QColor>
#include <QDebug>
#include <QString>
#include <QGraphicsSceneWheelEvent>
#include <QMessageLogger>
#include <QObject>
#include <QPainter>
#include <QSharedPointer>
#include <QtGlobal>
#include <QtMath>
#include <functional>
#include <libcockatrice/card/card_localization.h>
#include <libcockatrice/game/player/player_actions.h>
#include <libcockatrice/game/player/player_info.h>
#include <libcockatrice/game/player/player_logic.h>
#include <libcockatrice/game/zones/card_zone_logic.h>
#include <libcockatrice/game/zones/view_zone_logic.h>
#include <libcockatrice/protocol/pb/response_dump_zone.pb.h>
#include <libcockatrice/protocol/pb/serverinfo_card.pb.h>
#include <libcockatrice/protocol/pending_command.h>
#include <libcockatrice/settings/cards_display_settings.h>
#include <string>

class QGraphicsItem;

namespace
{
/** Horizontal offset between stack objects in the stack window (fan layout). */
constexpr qreal kStackFanStepX = 20.0;
} // namespace

/**
 * @param parent the parent QGraphicsWidget containing the reveal zone
 */
ZoneViewZone::ZoneViewZone(ZoneViewZoneLogic *_logic, QGraphicsItem *parent)
    : SelectZone(_logic, parent), bRect(QRectF()), minRows(0), groupBy(CardList::NoSort), sortBy(CardList::NoSort),
      pileView(false)
{
    auto *viewLogic = qobject_cast<ZoneViewZoneLogic *>(getLogic());
    if (!(viewLogic->getRevealZone() && !viewLogic->getWriteableRevealZone())) {
        viewLogic->getOriginalZone()->getViews().append(viewLogic);
    }
    connect(_logic, &ZoneViewZoneLogic::closeView, this, &ZoneViewZone::close);
}

void ZoneViewZone::addToViews()
{
    auto *viewLogic = qobject_cast<ZoneViewZoneLogic *>(getLogic());
    viewLogic->getOriginalZone()->getViews().append(viewLogic);
}

void ZoneViewZone::removeFromViews()
{
    auto *viewLogic = qobject_cast<ZoneViewZoneLogic *>(getLogic());
    viewLogic->getOriginalZone()->getViews().removeOne(viewLogic);
}

/**
 * Deletes this ZoneView and removes it from the origZone's views.
 * You should normally call this method instead of deleteLater()
 */
void ZoneViewZone::close()
{
    emit closed();
    auto *viewLogic = qobject_cast<ZoneViewZoneLogic *>(getLogic());
    if (!(viewLogic->getRevealZone() && !viewLogic->getWriteableRevealZone())) {
        viewLogic->getOriginalZone()->getViews().removeOne(viewLogic);
    }
    deleteLater();
}

QRectF ZoneViewZone::boundingRect() const
{
    return bRect;
}

void ZoneViewZone::paint(QPainter *painter, const QStyleOptionGraphicsItem * /*option*/, QWidget * /*widget*/)
{
    QBrush windowBrush(QColor(240, 240, 240));
    windowBrush.setColor(windowBrush.color().darker(150));
    painter->fillRect(boundingRect(), windowBrush);
}

void ZoneViewZone::initializeCards(const QList<const ServerInfo_Card *> &cardList)
{
    auto *logic = qobject_cast<ZoneViewZoneLogic *>(getLogic());
    int numberCards = logic->getNumberCards();
    if (!cardList.isEmpty()) {
        for (int i = 0; i < cardList.size(); ++i) {
            auto card = cardList[i];
            CardRef cardRef = {QString::fromStdString(card->name()), QString::fromStdString(card->provider_id())};
            auto copy = new CardItem(getLogic()->getPlayer(), this, cardRef, card->id());
            copy->setFaceDown(card->face_down());
            // The caller-supplied ServerInfo_Card carries an annotation, and dropping it here was
            // silently discarding text the popup was asked to show (ruled trigger ordering labels
            // each candidate with its ability text). Empty for every other caller, so a no-op there.
            copy->setAnnotation(QString::fromStdString(card->annotation()));

            getLogic()->addCard(copy->getState(), false, i);
        }
        // addCard triggers updateCardIds on every call, which rewrites all card IDs to sequential
        // position indices for unknown zones. Re-apply the server-supplied IDs so that pick-selectable
        // checks (which key on server card ID) work correctly for library search / revealed-card popups.
        const CardList &zoneCards = getLogic()->getCards();
        for (int i = 0; i < cardList.size() && i < zoneCards.size(); ++i) {
            zoneCards[i]->setId(cardList[i]->id());
        }
        reorganizeCards();
    } else if (ruledZoneViewHasAuthoritativeCardList(logic->getRevealZone(), logic->getWriteableRevealZone())) {
        reorganizeCards();
    } else if (!qobject_cast<ZoneViewZoneLogic *>(getLogic())->getOriginalZone()->contentsKnown()) {
        auto *viewLogic = qobject_cast<ZoneViewZoneLogic *>(getLogic());
        PendingCommand *pend = viewLogic->prepareZoneDump();
        if (pend != nullptr) {
            // A local game answers synchronously, so the handler has to be connected before the
            // command is sent or the response would be delivered to nobody.
            connect(pend, &PendingCommand::finished, this, &ZoneViewZone::zoneDumpReceived);
            viewLogic->sendZoneDump(pend);
        }
    } else {
        const CardList &c = logic->getOriginalZone()->getCards();
        int number = numberCards == -1 ? c.size() : (numberCards < c.size() ? numberCards : c.size());
        for (int i = 0; i < number; i++) {
            CardState *card = c.at(i);
            auto copy = new CardItem(getLogic()->getPlayer(), this, card->getCardRef(), card->getId());
            copy->setFaceDown(card->getFaceDown());
            RuledPreparationDisplay::copyViewState(RuledActions::cardView(card), copy);

            getLogic()->addCard(copy->getState(), false, i);
        }
        reorganizeCards();
    }
}

void ZoneViewZone::zoneDumpReceived(const Response &r)
{
    const Response_DumpZone &resp = r.GetExtension(Response_DumpZone::ext);
    const int respCardListSize = resp.zone_info().card_list_size();
    for (int i = 0; i < respCardListSize; ++i) {
        const ServerInfo_Card &cardInfo = resp.zone_info().card_list(i);
        auto cardName = QString::fromStdString(cardInfo.name());
        auto cardProviderId = QString::fromStdString(cardInfo.provider_id());
        auto card = new CardItem(getLogic()->getPlayer(), this, {cardName, cardProviderId}, cardInfo.id(), getLogic());
        card->setFaceDown(cardInfo.face_down());
        getLogic()->rawInsertCard(card->getState(), i);
    }

    qobject_cast<ZoneViewZoneLogic *>(getLogic())->updateCardIds(ZoneViewZoneLogic::INITIALIZE);
    reorganizeCards();
    // clang-format off
    emit getLogic()->cardCountChanged(); // emit keyword causes spurious spacing around ->
    // clang-format on
}

// Because of boundingRect(), this function must not be called before the zone was added to a scene.
void ZoneViewZone::reorganizeCards()
{
    const auto *zvLogic = qobject_cast<ZoneViewZoneLogic *>(getLogic());
    // A synthetic fan view (trigger ordering, replacement-effect picking) reuses the stack fan
    // layout over a deck-zone scaffold, but it is not the stack: its cards carry popup-local index
    // ids, so the engine's published-stack filter and LIFO sort must not touch them. Applying the
    // filter hid every candidate and left those popups empty.
    const bool isRealStackZone =
        zvLogic && zvLogic->getOriginalZone()->getName().compare(QStringLiteral("stack"), Qt::CaseInsensitive) == 0;
    const bool isStackZoneView = forceStackFanLayout || isRealStackZone;
    RuledClientState *ruledState = nullptr;
    int stackOwnerId = -1;
    if (isRealStackZone) {
        if (auto *player = getLogic()->getPlayer()) {
            if (auto *ag = player->getGame(); ag && RuledActions::isRuledGame(ag)) {
                ruledState = ag->getGameEventHandler()->ruled();
                stackOwnerId = player->getPlayerInfo()->getId();
            }
        }
    }

    // filter cards
    CardList cardsToDisplay(getLogic()->contentsKnown());
    for (auto card : cardItems()) {
        if ((!ruledState || ruledState->isPublishedStackCard(stackOwnerId, card->getId())) && filterString.check(card->getCard().getCardPtr())) {
            card->show();
            cardsToDisplay.append(card->getState());
        } else {
            card->hide();
        }
    }

    // sort cards
    QList<CardList::SortOption> sortOptions;
    if (groupBy != CardList::NoSort) {
        sortOptions << groupBy;
    }

    if (sortBy != CardList::NoSort) {
        sortOptions << sortBy;

        // implicitly sort by name at the end so that cards with the same name appear together
        if (sortBy != CardList::SortByName) {
            sortOptions << CardList::SortByName;
        }

        // group printings together
        sortOptions << CardList::SortByPrinting;
    }

    cardsToDisplay.sortBy(sortOptions);

    const int cardCount = cardsToDisplay.size();
    if (isStackZoneView) {
        // For the real ruled stack view, sort cardsToDisplay by engine push order so the fan
        // layout mirrors the engine's authoritative LIFO order. Index 0 → rightmost → resolves
        // first visually. This corrects the mismatch between physical spells (appended on
        // Event_MoveCard) and synthetic ability cards (prepended on createSyntheticAbilityStackCard).
        // `ruledState` is set only for the real stack zone, so popup fan views keep click order.
        if (ruledState) {
            const QList<quint32> &oidOrder = ruledState->getStackOidOrder();
            const int pid = stackOwnerId;
            std::sort(cardsToDisplay.begin(), cardsToDisplay.end(),
                      [&](CardState *a, CardState *b) {
                          int ia = static_cast<int>(
                              oidOrder.indexOf(ruledState->engineOidForCardId(pid, a->getId())));
                          int ib = static_cast<int>(
                              oidOrder.indexOf(ruledState->engineOidForCardId(pid, b->getId())));
                          if (ia < 0) ia = INT_MAX;
                          if (ib < 0) ib = INT_MAX;
                          return ia < ib;
                      });
        }
        // Horizontal fan: list index 0 on the right (stack top / LIFO front); older objects fan left; z rises to the
        // right so the foremost spell stays visually on top. Ruled stack window uses a wider step so faces stay
        // readable (default 20px overlaps most of a 72px card).
        const qreal fanStepX =
            forceStackFanLayout ? qMax(kStackFanStepX, CardDimensions::WIDTH_F * 0.42) : kStackFanStepX;
        for (int i = 0; i < cardCount; ++i) {
            CardItem *c = RuledActions::cardView(cardsToDisplay.at(i));
            const int slotFromLeft = cardCount - 1 - i;
            const qreal px = HORIZONTAL_PADDING + slotFromLeft * fanStepX;
            const qreal py = VERTICAL_PADDING;
            c->setPos(px, py);
            c->setRealZValue(slotFromLeft);
        }
        const qreal awidth = HORIZONTAL_PADDING * 2 + CardDimensions::WIDTH_F +
                             qMax(0, cardCount - 1) * fanStepX;
        const qreal aheight = VERTICAL_PADDING * 2 + CardDimensions::HEIGHT_F;
        optimumRect = QRectF(0, 0, awidth, aheight);
        updateGeometry();
        emit optimumRectChanged();
        return;
    }

    // position cards
    GridSize gridSize;
    if (pileView) {
        gridSize = positionCardsForDisplay(cardsToDisplay, groupBy);
    } else {
        gridSize = positionCardsForDisplay(cardsToDisplay);
    }

    // determine bounding rect
    qreal aleft = 0;
    qreal atop = 0;
    qreal awidth = gridSize.cols * CardDimensions::WIDTH_F + CardDimensions::WIDTH_HALF_F + HORIZONTAL_PADDING;
    qreal aheight = (gridSize.rows * CardDimensions::HEIGHT_F) / 3 + CardDimensions::HEIGHT_F * 1.3;
    optimumRect = QRectF(aleft, atop, awidth, aheight);

    updateGeometry();
    emit optimumRectChanged();
}

/**
 * @brief Sets the position of each card to the proper position for the view
 *
 * @param cards The cards to reposition. Will modify the cards in the list.
 * @param pileOption Property used to group cards for the piles. Expects `cards` to be sorted by that property. Pass in
 * NoSort to not make piles.
 *
 * @returns The number of rows and columns to display
 */
ZoneViewZone::GridSize ZoneViewZone::positionCardsForDisplay(CardList &cards, CardList::SortOption pileOption)
{
    int cardCount = cards.size();

    if (pileOption != CardList::NoSort) {
        int row = 0;
        int col = 0;
        int longestRow = 0;

        QString lastColumnProp;

        const auto extractor = CardList::getExtractorFor(pileOption);

        for (int i = 0; i < cardCount; i++) {
            CardState *c = cards.at(i);
            CardItem *cItem = qobject_cast<CardItem *>(c->parent());
            QString columnProp = extractor(c);

            if (i) { // if not the first card
                if (columnProp == lastColumnProp) {
                    row++; // add below current card
                } else {   // if no match then move card to next column
                    col++;
                    row = 0;
                }
            }

            lastColumnProp = columnProp;
            qreal x = col * CardDimensions::WIDTH_F;
            qreal y = row * CardDimensions::HEIGHT_F / 3;
            cItem->setPos(HORIZONTAL_PADDING + x, VERTICAL_PADDING + y);
            cItem->setRealZValue(i);
            longestRow = qMax(row, longestRow);
        }

        // +1 because the row/col variables used in the calculations are 0-indexed but
        // GridSize expects the actual row/col count
        return GridSize{longestRow + 1, qMax(col + 1, 3)};

    } else {
        int cols = qBound(1, qFloor(qSqrt((double)cardCount / 2)), 7);
        int rows = qMax(qCeil((double)cardCount / cols), 1);
        if (minRows == 0) {
            minRows = rows;
        } else if (rows < minRows) {
            rows = minRows;
            cols = qCeil((double)cardCount / minRows);
        }

        if (cols < 2) {
            cols = 2;
        }

        qCDebug(ViewZoneLog) << "reorganizeCards: rows=" << rows << "cols=" << cols;

        for (int i = 0; i < cardCount; i++) {
            auto *cItem = qobject_cast<CardItem *>(cards.at(i)->parent());
            qreal x = (i / rows) * CardDimensions::WIDTH_F;
            qreal y = (i % rows) * CardDimensions::HEIGHT_F / 3;
            cItem->setPos(HORIZONTAL_PADDING + x, VERTICAL_PADDING + y);
            cItem->setRealZValue(i);
        }

        return GridSize{rows, qMax(cols, 1)};
    }
}

void ZoneViewZone::setFilterString(const QString &_filterString)
{
    const CardsDisplaySettings &cardsDisplay = SettingsCache::instance().cardsDisplay();
    filterString = FilterString(
        _filterString, CardSearchLanguage{cardsDisplay.getCardLang(),
                                          static_cast<SearchLanguageMode>(cardsDisplay.getCardSearchLanguage())});
    reorganizeCards();
}

void ZoneViewZone::setGroupBy(CardList::SortOption _groupBy)
{
    groupBy = _groupBy;
    reorganizeCards();
}

void ZoneViewZone::setSortBy(CardList::SortOption _sortBy)
{
    sortBy = _sortBy;
    reorganizeCards();
}

void ZoneViewZone::setPileView(int _pileView)
{
    pileView = _pileView;
    reorganizeCards();
}

void ZoneViewZone::handleDropEvent(const QList<CardDragItem *> &dragItems,
                                   CardZoneLogic *startZone,
                                   const QPoint & /*dropPoint*/)
{
    QList<CardMoveRequest> cards;
    cards.reserve(dragItems.size());
    for (int i = 0; i < dragItems.size(); ++i) {
        cards.append(CardMoveRequest{dragItems[i]->getId(), dragItems[i]->isForceFaceDown()});
    }

    getLogic()->getPlayer()->getPlayerActions()->moveCards(startZone, getLogic(), 0, 0, cards);
}

void ZoneViewZone::setGeometry(const QRectF &rect)
{
    prepareGeometryChange();
    setPos(rect.x(), rect.y());
    bRect = QRectF(0, 0, rect.width(), rect.height());
}

QSizeF ZoneViewZone::sizeHint(Qt::SizeHint /*which*/, const QSizeF & /*constraint*/) const
{
    return optimumRect.size();
}

void ZoneViewZone::wheelEvent(QGraphicsSceneWheelEvent *event)
{
    emit wheelEventReceived(event);
}
