#include "card_item.h"

#include "../../client/settings/cache_settings.h"
#include "../../interface/widgets/tabs/tab_game.h"
#include "../abstract_game.h"
#include "../game_event_handler.h"
#include "../game_scene.h"
#include "../phase.h"
#include "../player/player.h"
#include "../player/player_actions.h"
#include "../player/player_manager.h"
#include "../zones/logic/view_zone_logic.h"
#include "../zones/table_zone.h"
#include "../zones/view_zone.h"
#include "arrow_item.h"
#include "card_drag_item.h"

#include <../../client/settings/card_counter_settings.h>
#include <QApplication>
#include <QGraphicsSceneMouseEvent>
#include <QMenu>
#include <QPainter>
#include <QPen>
#include <libcockatrice/card/card_info.h>
#include <libcockatrice/protocol/pb/serverinfo_card.pb.h>
#include <libcockatrice/utility/zone_names.h>

CardItem::CardItem(Player *_owner, QGraphicsItem *parent, const CardRef &cardRef, int _cardid, CardZoneLogic *_zone)
    : AbstractCardItem(parent, cardRef, _owner, _cardid), zone(_zone), attacking(false), destroyOnZoneChange(false),
      doesntUntap(false), dragItem(nullptr), attachedTo(nullptr)
{
    owner->addCard(this);

    connect(&SettingsCache::instance().cardCounters(), &CardCounterSettings::colorChanged, this, [this](int counterId) {
        if (counters.contains(counterId))
            update();
    });

    if (auto *game = owner ? owner->getGame() : nullptr) {
        if (auto *handler = game->getGameEventHandler()) {
            connect(handler, &GameEventHandler::ruledCombatStateChanged, this, [this]() { update(); });
            connect(handler, &GameEventHandler::ruledBattlefieldMapUpdated, this, [this]() { update(); });
        }
    }
}

void CardItem::prepareDelete()
{
    if (owner != nullptr) {
        if (owner->getGame()->getActiveCard() == this) {
            owner->getPlayerMenu()->updateCardMenu(nullptr);
            owner->getGame()->setActiveCard(nullptr);
        }
        owner = nullptr;
    }

    while (!attachedCards.isEmpty()) {
        attachedCards.first()->setZone(nullptr); // so that it won't try to call reorganizeCards()
        attachedCards.first()->setAttachedTo(nullptr);
    }

    if (attachedTo != nullptr) {
        attachedTo->removeAttachedCard(this);
        attachedTo = nullptr;
    }
}

void CardItem::deleteLater()
{
    prepareDelete();
    if (scene())
        static_cast<GameScene *>(scene())->unregisterAnimationItem(this);
    AbstractCardItem::deleteLater();
}

void CardItem::setZone(CardZoneLogic *_zone)
{
    zone = _zone;
}

void CardItem::retranslateUi()
{
}

void CardItem::paint(QPainter *painter, const QStyleOptionGraphicsItem *option, QWidget *widget)
{
    auto &cardCounterSettings = SettingsCache::instance().cardCounters();

    painter->save();
    AbstractCardItem::paint(painter, option, widget);

    int i = 0;
    QMapIterator<int, int> counterIterator(counters);
    while (counterIterator.hasNext()) {
        counterIterator.next();
        QColor _color = cardCounterSettings.color(counterIterator.key());

        paintNumberEllipse(counterIterator.value(), 14, _color, i, counters.size(), painter);
        ++i;
    }

    QSizeF translatedSize = getTranslatedSize(painter);
    qreal scaleFactor = translatedSize.width() / boundingRect().width();
    GameEventHandler *ruledHandler = nullptr;
    quint32 ruledOid = 0;
    if (auto *game = owner ? owner->getGame() : nullptr) {
        if (game->getGameMetaInfo()->proto().ruled_game()) {
            ruledHandler = game->getGameEventHandler();
            if (ruledHandler) {
                const int ownerPlayerId = owner ? owner->getPlayerInfo()->getId() : -1;
                ruledOid = ruledHandler->engineOidForCardId(ownerPlayerId, id);
            }
        }
    }

    if (!pt.isEmpty()) {
        painter->save();
        transformPainter(painter, translatedSize, tapAngle);

        if (!getFaceDown() && pt == exactCard.getInfo().getPowTough()) {
            painter->setPen(Qt::white);
        } else {
            painter->setPen(QColor(255, 150, 0)); // dark orange
        }

        painter->setBackground(Qt::black);
        painter->setBackgroundMode(Qt::OpaqueMode);

        painter->drawText(QRectF(4 * scaleFactor, 4 * scaleFactor, translatedSize.width() - 10 * scaleFactor,
                                 translatedSize.height() - 8 * scaleFactor),
                          Qt::AlignRight | Qt::AlignBottom, pt);
        painter->restore();
    }

    if (ruledHandler && ruledOid != 0) {
        const int markedDamage = ruledHandler->markedDamageForEngineOid(ruledOid);
        if (markedDamage > 0) {
            painter->save();
            transformPainter(painter, translatedSize, tapAngle);
            painter->setPen(QColor(220, 20, 60)); // crimson
            painter->setBackground(Qt::black);
            painter->setBackgroundMode(Qt::OpaqueMode);
            painter->drawText(QRectF(4 * scaleFactor, 4 * scaleFactor, translatedSize.width() - 10 * scaleFactor,
                                     translatedSize.height() - 28 * scaleFactor),
                              Qt::AlignRight | Qt::AlignBottom, QString::number(markedDamage));
            painter->restore();
        }
    }

    QString renderedAnnotation = annotation;
    if (ruledHandler && ruledOid != 0 && ruledHandler->isEngineOidSummoningSick(ruledOid)) {
        if (!renderedAnnotation.contains(QStringLiteral("summoning sick"), Qt::CaseInsensitive)) {
            if (!renderedAnnotation.isEmpty()) {
                renderedAnnotation += QLatin1Char('\n');
            }
            renderedAnnotation += QStringLiteral("summoning sick");
        }
    }

    if (!renderedAnnotation.isEmpty()) {
        painter->save();

        transformPainter(painter, translatedSize, tapAngle);
        painter->setBackground(Qt::black);
        painter->setBackgroundMode(Qt::OpaqueMode);
        painter->setPen(Qt::white);

        painter->drawText(QRectF(4 * scaleFactor, 4 * scaleFactor, translatedSize.width() - 8 * scaleFactor,
                                 translatedSize.height() - 8 * scaleFactor),
                          Qt::AlignCenter | Qt::TextWrapAnywhere, renderedAnnotation);
        painter->restore();
    }

    if (getBeingPointedAt()) {
        painter->fillPath(shape(), QBrush(QColor(255, 0, 0, 100)));
    }

    if (doesntUntap) {
        painter->save();

        painter->setRenderHint(QPainter::Antialiasing, false);

        QPen pen;
        pen.setColor(Qt::magenta);
        pen.setWidth(0); // Cosmetic pen
        painter->setPen(pen);
        painter->drawPath(shape());

        painter->restore();
    }

    if (ruledHandler) {
        if (ruledOid != 0) {
            QColor outlineColor;
            if (ruledHandler->isPendingAttacker(ruledOid)) {
                outlineColor = QColor(255, 215, 0); // gold for pending attackers
            } else if (ruledHandler->stagedBlocker() == ruledOid) {
                outlineColor = QColor(0, 255, 128); // green for staged blocker
            } else if (ruledHandler->pendingBlockTargetForBlocker(ruledOid) != 0) {
                outlineColor = QColor(80, 160, 255); // blue for paired blocker
            } else if (ruledHandler->isCurrentAttacker(ruledOid) && !attacking) {
                // Engine has confirmed this attacker but the AttrAttacking event
                // may not have arrived yet — draw a faint marker.
                outlineColor = QColor(255, 80, 80, 200); // red-ish
            }
            if (outlineColor.isValid()) {
                painter->save();
                painter->setRenderHint(QPainter::Antialiasing, true);
                QPen pen;
                pen.setColor(outlineColor);
                pen.setWidth(3);
                painter->setPen(pen);
                painter->drawPath(shape());
                painter->restore();
            }
        }
        if (zone && zone->getName() == ZoneNames::HAND && owner && owner->getPlayerInfo()->getLocal() &&
            ruledHandler->localPlayerMustCleanupDiscard()) {
            if (zone->getCards().indexOf(const_cast<CardItem *>(this)) >= 0) {
                const int ri = ruledHandler->resolveRuledCleanupDiscardHandIndexForClickedCard(this);
                if (ri >= 0 && ruledHandler->isRuledCleanupDiscardLegalForHandIndex(ri) &&
                    ruledHandler->isRuledCleanupDiscardHandIndexSelected(ri)) {
                    painter->save();
                    painter->setRenderHint(QPainter::Antialiasing, true);
                    QPen pen;
                    pen.setColor(QColor(255, 165, 0)); // orange for cleanup discard selection
                    pen.setWidth(4);
                    painter->setPen(pen);
                    painter->drawPath(shape());
                    painter->restore();
                }
            }
        }
    }

    painter->restore();
}

void CardItem::setAttacking(bool _attacking)
{
    attacking = _attacking;
    update();
}

void CardItem::setCounter(int _id, int _value)
{
    if (_value)
        counters.insert(_id, _value);
    else
        counters.remove(_id);
    update();
}

void CardItem::setAnnotation(const QString &_annotation)
{
    annotation = _annotation;
    update();
}

void CardItem::setDoesntUntap(bool _doesntUntap)
{
    doesntUntap = _doesntUntap;
    update();
}

void CardItem::setPT(const QString &_pt)
{
    pt = _pt;
    update();
}

void CardItem::setAttachedTo(CardItem *_attachedTo)
{
    if (attachedTo != nullptr) {
        attachedTo->removeAttachedCard(this);
    }

    gridPoint.setX(-1);
    attachedTo = _attachedTo;
    if (attachedTo != nullptr) {
        // If the zone is being torn down, it might already be null by the time a card tries to un-attach all its
        // attached cards
        if (attachedTo->zone == nullptr) {
            deleteLater();
        } else {
            emit attachedTo->zone->cardAdded(this);
            attachedTo->addAttachedCard(this);
            if (zone != attachedTo->getZone()) {
                attachedTo->getZone()->reorganizeCards();
            }
        }
    } else {
        // If the zone is being torn down, it might already be null by the time a card tries to un-attach all its
        // attached cards
        if (zone == nullptr) {
            deleteLater();
        } else {
            emit zone->cardAdded(this);
        }
    }

    if (zone != nullptr) {
        zone->reorganizeCards();
    }
}

/**
 * @brief Resets the fields that should be reset after a zone transition
 */
void CardItem::resetState(bool keepAnnotations)
{
    attacking = false;
    counters.clear();
    pt.clear();
    if (!keepAnnotations) {
        annotation.clear();
    }
    attachedTo = 0;
    attachedCards.clear();
    setTapped(false, false);
    setDoesntUntap(false);
    if (scene())
        static_cast<GameScene *>(scene())->unregisterAnimationItem(this);
    update();
}

void CardItem::processCardInfo(const ServerInfo_Card &_info)
{
    counters.clear();
    const int counterListSize = _info.counter_list_size();
    for (int i = 0; i < counterListSize; ++i) {
        const ServerInfo_CardCounter &counterInfo = _info.counter_list(i);
        counters.insert(counterInfo.id(), counterInfo.value());
    }

    setId(_info.id());
    setCardRef({QString::fromStdString(_info.name()), QString::fromStdString(_info.provider_id())});
    setAttacking(_info.attacking());
    setFaceDown(_info.face_down());
    setPT(QString::fromStdString(_info.pt()));
    setAnnotation(QString::fromStdString(_info.annotation()));
    setColor(QString::fromStdString(_info.color()));
    setTapped(_info.tapped());
    setDestroyOnZoneChange(_info.destroy_on_zone_change());
    setDoesntUntap(_info.doesnt_untap());
}

CardDragItem *CardItem::createDragItem(int _id, const QPointF &_pos, const QPointF &_scenePos, bool forceFaceDown)
{
    deleteDragItem();
    dragItem = new CardDragItem(this, _id, _pos, forceFaceDown);
    dragItem->setVisible(false);
    scene()->addItem(dragItem);
    dragItem->updatePosition(_scenePos);
    dragItem->setVisible(true);

    return dragItem;
}

void CardItem::deleteDragItem()
{
    if (dragItem) {
        dragItem->deleteLater();
    }
    dragItem = nullptr;
}

void CardItem::drawArrow(const QColor &arrowColor)
{
    if (owner->getGame()->getPlayerManager()->isSpectator())
        return;

    auto *game = owner->getGame();
    Player *arrowOwner = game->getPlayerManager()->getActiveLocalPlayer(game->getGameState()->getActivePlayer());
    int phase = 0; // 0 means to not set the phase
    if (SettingsCache::instance().getDoNotDeleteArrowsInSubPhases()) {
        int currentPhase = game->getGameState()->getCurrentPhase();
        phase = Phases::getLastSubphase(currentPhase) + 1;
    }
    ArrowDragItem *arrow = new ArrowDragItem(arrowOwner, this, arrowColor, phase);
    scene()->addItem(arrow);
    arrow->grabMouse();

    for (const auto &item : scene()->selectedItems()) {
        CardItem *card = qgraphicsitem_cast<CardItem *>(item);
        if (card == nullptr || card == this)
            continue;
        if (card->getZone() != zone)
            continue;

        ArrowDragItem *childArrow = new ArrowDragItem(arrowOwner, card, arrowColor, phase);
        scene()->addItem(childArrow);
        arrow->addChildArrow(childArrow);
    }
}

void CardItem::drawAttachArrow()
{
    if (owner->getGame()->getPlayerManager()->isSpectator())
        return;

    auto *arrow = new ArrowAttachItem(this);
    scene()->addItem(arrow);
    arrow->grabMouse();

    for (const auto &item : scene()->selectedItems()) {
        CardItem *card = qgraphicsitem_cast<CardItem *>(item);
        if (card == nullptr)
            continue;
        if (card->getZone() != zone)
            continue;

        ArrowAttachItem *childArrow = new ArrowAttachItem(card);
        scene()->addItem(childArrow);
        arrow->addChildArrow(childArrow);
    }
}

void CardItem::mouseMoveEvent(QGraphicsSceneMouseEvent *event)
{
    if (event->buttons().testFlag(Qt::RightButton)) {
        if ((event->screenPos() - event->buttonDownScreenPos(Qt::RightButton)).manhattanLength() <
            2 * QApplication::startDragDistance())
            return;

        QColor arrowColor = Qt::red;
        if (event->modifiers().testFlag(Qt::ControlModifier))
            arrowColor = Qt::yellow;
        else if (event->modifiers().testFlag(Qt::AltModifier))
            arrowColor = Qt::blue;
        else if (event->modifiers().testFlag(Qt::ShiftModifier))
            arrowColor = Qt::green;

        drawArrow(arrowColor);
    } else if (event->buttons().testFlag(Qt::LeftButton)) {
        if ((event->screenPos() - event->buttonDownScreenPos(Qt::LeftButton)).manhattanLength() <
            2 * QApplication::startDragDistance())
            return;
        if (const ZoneViewZoneLogic *view = qobject_cast<const ZoneViewZoneLogic *>(zone)) {
            if (view->getRevealZone() && !view->getWriteableRevealZone())
                return;
        } else if (!owner->getPlayerInfo()->getLocalOrJudge())
            return;

        if (auto *game = owner->getGame();
            game && game->getGameMetaInfo() && game->getGameMetaInfo()->proto().ruled_game()) {
            setCursor(Qt::OpenHandCursor);
            return;
        }

        bool forceFaceDown = event->modifiers().testFlag(Qt::ShiftModifier);

        // Use the buttonDownPos to align the hot spot with the position when
        // the user originally clicked
        createDragItem(id, event->buttonDownPos(Qt::LeftButton), event->scenePos(), forceFaceDown);
        dragItem->grabMouse();

        int childIndex = 0;
        for (const auto &item : scene()->selectedItems()) {
            CardItem *card = static_cast<CardItem *>(item);
            if ((card == this) || (card->getZone() != zone))
                continue;
            ++childIndex;
            QPointF childPos;
            if (zone->getHasCardAttr())
                childPos = card->pos() - pos();
            else
                childPos = QPointF(childIndex * CardDimensions::WIDTH_HALF_F, 0);
            CardDragItem *drag =
                new CardDragItem(card, card->getId(), childPos, card->getFaceDown() || forceFaceDown, dragItem);
            drag->setPos(dragItem->pos() + childPos);
            scene()->addItem(drag);
        }
    }
    setCursor(Qt::OpenHandCursor);
}

static bool isTableLandSingleClickLegal(const CardItem *card);

void CardItem::playCard(bool faceDown)
{
    // Do nothing if the card belongs to another player
    if (!owner->getPlayerInfo()->getLocalOrJudge())
        return;

    TableZoneLogic *tz = qobject_cast<TableZoneLogic *>(zone);
    if (tz) {
        if (auto *game = owner->getGame();
            game && game->getGameMetaInfo() && game->getGameMetaInfo()->proto().ruled_game()) {
            // Non-lands: no freeform click-to-tap. Face-up lands: still use table tap for local mana shortcut.
            if (!isTableLandSingleClickLegal(this) || faceDown) {
                return;
            }
        }
        emit tz->toggleTapped();
    } else {
        if (SettingsCache::instance().getClickPlaysAllSelected()) {
            faceDown ? zone->getPlayer()->getPlayerActions()->actPlayFacedown()
                     : zone->getPlayer()->getPlayerActions()->actPlay();
        } else {
            zone->getPlayer()->getPlayerActions()->playCard(this, faceDown);
        }
    }
}

/**
 * @brief returns true if the zone is a unwritable reveal zone view (eg a card reveal window). Will return false if zone
 * is nullptr.
 */
static bool isUnwritableRevealZone(CardZoneLogic *zone)
{
    if (auto *view = qobject_cast<ZoneViewZoneLogic *>(zone)) {
        return view->getRevealZone() && !view->getWriteableRevealZone();
    }
    return false;
}

/** True if the left press/release pair is a click, not a drag-away (matches drag-start threshold in mouseMoveEvent). */
static bool isStationaryLeftRelease(const QGraphicsSceneMouseEvent *event)
{
    return (event->screenPos() - event->buttonDownScreenPos(Qt::LeftButton)).manhattanLength() <
           QApplication::startDragDistance();
}

static bool isRuledLandSingleClickLegal(const CardItem *card)
{
    if (!card || !card->getOwner() || !card->getZone()) {
        return false;
    }
    if (card->getZone()->getName() != ZoneNames::HAND) {
        return false;
    }
    if (!card->getCardInfo().getCardType().contains("Land", Qt::CaseInsensitive)) {
        return false;
    }

    auto *game = card->getOwner()->getGame();
    if (!game || !game->getGameMetaInfo()->proto().ruled_game()) {
        return false;
    }

    const int handIndex = card->getZone()->getCards().indexOf(const_cast<CardItem *>(card));
    if (handIndex < 0) {
        return false;
    }
    const int resolved = game->getGameEventHandler()->resolveRuledLandPlayHandIndexForClickedCard(card);
    return resolved >= 0;
}

static bool isRuledSpellSingleClickLegal(const CardItem *card)
{
    if (!card || !card->getOwner() || !card->getZone()) {
        return false;
    }
    if (card->getZone()->getName() != ZoneNames::HAND) {
        return false;
    }
    if (card->getCardInfo().getCardType().contains("Land", Qt::CaseInsensitive)) {
        return false;
    }

    auto *game = card->getOwner()->getGame();
    if (!game || !game->getGameMetaInfo()->proto().ruled_game()) {
        return false;
    }

    const int handIndex = card->getZone()->getCards().indexOf(const_cast<CardItem *>(card));
    if (handIndex < 0) {
        return false;
    }
    const int resolved = game->getGameEventHandler()->resolveRuledSpellCastHandIndexForClickedCard(card);
    return resolved >= 0;
}

static bool isTableLandSingleClickLegal(const CardItem *card)
{
    if (!card || !card->getZone() || card->getFaceDown()) {
        return false;
    }
    if (card->getZone()->getName() != ZoneNames::TABLE) {
        return false;
    }
    return card->getCardInfo().getCardType().contains("Land", Qt::CaseInsensitive);
}

namespace {
GameEventHandler *ruledHandlerForCard(const CardItem *card)
{
    if (!card || !card->getOwner() || !card->getOwner()->getGame()) {
        return nullptr;
    }
    auto *game = card->getOwner()->getGame();
    if (!game->getGameMetaInfo()->proto().ruled_game()) {
        return nullptr;
    }
    return game->getGameEventHandler();
}

bool isCombatEligibleCreature(const CardItem *card)
{
    if (!card || !card->getZone()) {
        return false;
    }
    if (card->getZone()->getName() != ZoneNames::TABLE) {
        return false;
    }
    if (card->getFaceDown()) {
        return false;
    }
    return card->getCardInfo().getCardType().contains("Creature", Qt::CaseInsensitive);
}

// Try to handle a left-click as a ruled-mode combat input.
// Returns true if the click was consumed by combat handling.
bool handleRuledCombatClick(CardItem *card)
{
    GameEventHandler *handler = ruledHandlerForCard(card);
    if (!handler) {
        return false;
    }
    if (!isCombatEligibleCreature(card)) {
        return false;
    }
    Player *owner = card->getOwner();
    const int ownerPlayerId = owner ? owner->getPlayerInfo()->getId() : -1;
    const quint32 oid = handler->engineOidForCardId(ownerPlayerId, card->getId());
    if (oid == 0) {
        return false;
    }
    const auto phase = handler->getRuledCombatPhase();
    using Phase = GameEventHandler::RuledCombatPhase;
    const bool ownCreature = owner && owner->getPlayerInfo()->getLocal();

    if (phase == Phase::DeclareAttackers && handler->localPlayerIsRuledActive() && ownCreature) {
        if (card->getTapped()) {
            return false;
        }
        if (handler->isEngineOidSummoningSick(oid)) {
            return false;
        }
        handler->togglePendingAttacker(oid);
        return true;
    }

    if (phase == Phase::DeclareBlockers && handler->localPlayerIsRuledDefender()) {
        if (ownCreature) {
            if (card->getTapped()) {
                return false;
            }
            // Toggle: clicking the staged blocker again clears the staging.
            if (handler->stagedBlocker() == oid) {
                handler->clearStagedBlocker();
            } else {
                handler->selectStagedBlocker(oid);
            }
            return true;
        }
        // Clicked an enemy creature — pair with the staged blocker if it's an attacker.
        if (handler->hasStagedBlocker() && handler->isCurrentAttacker(oid)) {
            handler->pairStagedBlockerToAttacker(oid);
            return true;
        }
    }

    return false;
}
} // namespace

/**
 * This method is called when a "click to play" is done on the card.
 * This is either triggered by a single click or double click, depending on the settings.
 *
 * @param shiftHeld if the shift key was held during the click
 */
void CardItem::handleClickedToPlay(bool shiftHeld)
{
    if (isUnwritableRevealZone(zone)) {
        if (SettingsCache::instance().getClickPlaysAllSelected()) {
            zone->getPlayer()->getPlayerActions()->actHide();
        } else {
            zone->removeCard(this);
        }
    } else {
        playCard(shiftHeld);
    }
}

void CardItem::mouseReleaseEvent(QGraphicsSceneMouseEvent *event)
{
    if (event->button() == Qt::RightButton) {

        if (owner != nullptr) {
            owner->getGame()->setActiveCard(this);
            if (QMenu *cardMenu = owner->getPlayerMenu()->updateCardMenu(this)) {
                cardMenu->popup(event->screenPos());
                return;
            }
        }
    } else if ((event->modifiers() != Qt::AltModifier) && (event->button() == Qt::LeftButton)) {
        const bool stationaryLeft = isStationaryLeftRelease(event);
        if (owner != nullptr) {
            auto *game = owner->getGame();
            auto *playerManager = game ? game->getPlayerManager() : nullptr;
            auto *localPlayer = playerManager ? playerManager->getPlayers().value(playerManager->getLocalPlayerId()) : nullptr;
            auto *actions = localPlayer ? localPlayer->getPlayerActions() : nullptr;
            if (stationaryLeft && owner->getPlayerInfo()->getLocal() && actions && zone &&
                zone->getName() == ZoneNames::HAND && actions->tryRuledOpeningBottomCard(this)) {
                update();
                AbstractCardItem::mouseReleaseEvent(event);
                return;
            }
            if (stationaryLeft && owner->getPlayerInfo()->getLocal() && actions && zone &&
                zone->getName() == ZoneNames::HAND && actions->tryToggleRuledCleanupDiscard(this)) {
                update();
                AbstractCardItem::mouseReleaseEvent(event);
                return;
            }
            if (stationaryLeft && actions && actions->tryHandleRuledSpellTargetClick(this)) {
                setCursor(Qt::OpenHandCursor);
                AbstractCardItem::mouseReleaseEvent(event);
                return;
            }
        }
        // Ruled-mode combat clicks take priority over normal play handling on the table.
        if (stationaryLeft && handleRuledCombatClick(this)) {
            update();
            if (owner != nullptr) {
                setCursor(Qt::OpenHandCursor);
            }
            AbstractCardItem::mouseReleaseEvent(event);
            return;
        }
        if (stationaryLeft &&
            (!SettingsCache::instance().getDoubleClickToPlay() || isRuledLandSingleClickLegal(this) ||
             isRuledSpellSingleClickLegal(this) || isTableLandSingleClickLegal(this))) {
            handleClickedToPlay(event->modifiers().testFlag(Qt::ShiftModifier));
        }
    }

    if (owner != nullptr) { // cards without owner will be deleted
        setCursor(Qt::OpenHandCursor);
    }
    AbstractCardItem::mouseReleaseEvent(event);
}

void CardItem::mouseDoubleClickEvent(QGraphicsSceneMouseEvent *event)
{
    if ((event->modifiers() != Qt::AltModifier) && (event->buttons() == Qt::LeftButton) &&
        (SettingsCache::instance().getDoubleClickToPlay())) {
        handleClickedToPlay(event->modifiers().testFlag(Qt::ShiftModifier));
    }
    event->accept();
}

bool CardItem::animationEvent()
{
    int rotation = ROTATION_DEGREES_PER_FRAME;
    bool animationIncomplete = true;
    if (!tapped)
        rotation *= -1;

    tapAngle += rotation;
    if (tapped && (tapAngle > 90)) {
        tapAngle = 90;
        animationIncomplete = false;
    }
    if (!tapped && (tapAngle < 0)) {
        tapAngle = 0;
        animationIncomplete = false;
    }

    setTransform(QTransform()
                     .translate(CardDimensions::WIDTH_HALF_F, CardDimensions::HEIGHT_HALF_F)
                     .rotate(tapAngle)
                     .translate(-CardDimensions::WIDTH_HALF_F, -CardDimensions::HEIGHT_HALF_F));
    setHovered(false);
    update();

    return animationIncomplete;
}

QVariant CardItem::itemChange(GraphicsItemChange change, const QVariant &value)
{
    if ((change == ItemSelectedHasChanged) && owner != nullptr) {
        if (value == true) {
            owner->getGame()->setActiveCard(this);
            owner->getPlayerMenu()->updateCardMenu(this);
        } else if (owner->getGameScene()->selectedItems().isEmpty()) {

            owner->getGame()->setActiveCard(nullptr);
            owner->getPlayerMenu()->updateCardMenu(nullptr);
        }
    }
    return AbstractCardItem::itemChange(change, value);
}
