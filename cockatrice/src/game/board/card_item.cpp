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
#include "../ruled/ruled_actions.h"
#include "../ruled/ruled_client_state.h"
#include "../ruled/ruled_token_display.h"
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
#include <libcockatrice/card/database/card_database_manager.h>
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
            RuledClientState *ruled = handler->ruled();
            connect(ruled, &RuledClientState::combatStateChanged, this, [this]() { update(); });
            connect(ruled, &RuledClientState::battlefieldMapUpdated, this, [this]() { update(); });
            connect(ruled, &RuledClientState::combatDamageUiChanged, this, [this]() { update(); });
            connect(ruled, &RuledClientState::spellTargetSelectionChanged, this, [this]() { update(); });
            connect(ruled, &RuledClientState::spellDamageAllocationUiChanged, this, [this]() { update(); });
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
    AbstractGame *ruledGame = owner ? owner->getGame() : nullptr;
    RuledClientState *ruledHandler = RuledActions::stateFor(ruledGame);
    quint32 ruledOid = 0;
    quint32 ruledTargetSelectionOid = 0;
    if (ruledHandler) {
        const int ownerPlayerId = owner ? owner->getPlayerInfo()->getId() : -1;
        ruledOid = ruledHandler->engineOidForCardId(ownerPlayerId, id);
        ruledTargetSelectionOid =
            zone && zone->getName() == ZoneNames::GRAVE
                ? ruledHandler->graveyardEngineOidForOwnedCard(ownerPlayerId, id)
                : ruledOid;
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
    // CR 702.10b: a creature with Haste is unaffected by summoning sickness — don't
    // show the "summoning sick" tag if the creature has Haste. Only show it for
    // battlefield cards (TABLE zone); a lingering spell/ability card in the stack zone
    // may share an OID with a summoning-sick permanent but must never show the label there.
    const bool inTableZone = zone && zone->getName() == QLatin1String(ZoneNames::TABLE);
    if (ruledHandler && ruledOid != 0 && inTableZone && ruledHandler->isEngineOidSummoningSick(ruledOid)
        && !ruledHandler->isEngineOidHaste(ruledOid)) {
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

    // Ability annotation: draw italic text at the bottom of the card for abilities on the stack.
    if (ruledHandler && ruledOid != 0) {
        const QString abilityAnnotation = ruledHandler->stackAnnotation(ruledOid);
        if (!abilityAnnotation.isEmpty()) {
            painter->save();
            transformPainter(painter, translatedSize, tapAngle);
            painter->setBackground(QColor(0, 0, 0, 160));
            painter->setBackgroundMode(Qt::OpaqueMode);
            painter->setPen(QColor(220, 220, 255));
            QFont abilityFont = painter->font();
            abilityFont.setItalic(true);
            abilityFont.setPointSizeF(abilityFont.pointSizeF() * 0.75);
            painter->setFont(abilityFont);
            painter->drawText(
                QRectF(4 * scaleFactor, translatedSize.height() * 0.65,
                       translatedSize.width() - 8 * scaleFactor, translatedSize.height() * 0.33),
                Qt::AlignCenter | Qt::TextWrapAnywhere,
                abilityAnnotation);
            painter->restore();
        }
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
            const auto ruledPhase = ruledHandler->getCombatPhase();
            using RuledPhase = RuledClientState::RuledCombatPhase;
            if (ruledPhase == RuledPhase::AssignCombatDamage) {
                const quint32 curAtt = ruledHandler->currentCombatDamageAttackerOid();
                const int dmg = static_cast<int>(ruledHandler->assignedCombatDamageForBlocker(ruledOid));
                if (dmg > 0) {
                    outlineColor = QColor(200, 40, 40); // red tint for blockers with assigned damage
                } else if (curAtt != 0 && ruledHandler->getCommittedBlocks().value(ruledOid, 0) == curAtt) {
                    outlineColor = QColor(255, 200, 0); // yellow for blockers of current attacker
                }
            } else if (ruledHandler->isPendingAttacker(ruledOid)) {
                outlineColor = QColor(255, 215, 0); // gold for pending attackers
            } else if (ruledHandler->isStagedBlocker(ruledOid)) {
                outlineColor = QColor(0, 255, 128); // green for staged blockers
            } else if (ruledHandler->pendingBlockTargetForBlocker(ruledOid) != 0) {
                outlineColor = QColor(80, 160, 255); // blue for paired blocker
            } else if (ruledHandler->isCurrentAttacker(ruledOid) && !attacking) {
                // Engine has confirmed this attacker but the AttrAttacking event
                // may not have arrived yet — draw a faint marker.
                outlineColor = QColor(255, 80, 80, 200); // red-ish
            }
            if (ruledHandler->isPendingChoiceCandidate(RuledClientState::ChoiceKind::CopySource, ruledOid)) {
                outlineColor = QColor(80, 200, 255); // cyan for an eligible untargeted copy source
            }
            if (ruledHandler->isPendingChoiceCandidate(RuledClientState::ChoiceKind::AuraPermanent, ruledOid)) {
                outlineColor = QColor(180, 100, 255); // violet for a legal returning-Aura recipient
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
            if (RuledActions::isSpellDamageAllocationDisplayActive(ruledGame)) {
                const int alloc = RuledActions::spellDamageAllocationForOid(ruledGame, ruledOid);
                if (alloc > 0) {
                    paintNumberEllipse(alloc, 14, QColor(255, 120, 0), 0, 1, painter);
                }
            }
            const int loyalty = ruledHandler->loyaltyForPermanentOid(ruledOid);
            if (loyalty >= 0) {
                paintNumberEllipse(loyalty, 14, QColor(70, 110, 220), 0, 1, painter);
            }
            const int defense = ruledHandler->defenseForPermanentOid(ruledOid);
            if (defense >= 0) {
                paintNumberEllipse(defense, 14, QColor(205, 125, 35), 0, 1, painter);
            }
            if (ruledPhase == RuledPhase::AssignCombatDamage) {
                const quint32 curAtt = ruledHandler->currentCombatDamageAttackerOid();
                if (curAtt != 0 && ruledHandler->getCommittedBlocks().value(ruledOid, 0) == curAtt) {
                    const int dmg = static_cast<int>(ruledHandler->assignedCombatDamageForBlocker(ruledOid));
                    paintNumberEllipse(dmg, 14, dmg > 0 ? QColor(220, 60, 60) : QColor(140, 140, 140), 0, 1, painter);
                }
            }
        }
        if (ruledTargetSelectionOid != 0 &&
            RuledActions::isSelectedSpellTarget(ruledGame, ruledTargetSelectionOid)) {
            painter->save();
            painter->setRenderHint(QPainter::Antialiasing, true);
            QPen pen(QColor(220, 40, 40));
            pen.setWidth(4);
            painter->setPen(pen);
            painter->drawPath(shape());
            painter->restore();
        }
        if (zone && zone->getName() == ZoneNames::TABLE && ruledTargetSelectionOid != 0 &&
            RuledActions::isSelectedCastCostPermanent(ruledGame, ruledTargetSelectionOid)) {
            painter->save();
            painter->setRenderHint(QPainter::Antialiasing, true);
            QPen pen(QColor(255, 165, 0));
            pen.setWidth(4);
            painter->setPen(pen);
            painter->drawPath(shape());
            painter->restore();
        }
        if (zone && (zone->getName() == ZoneNames::GRAVE || zone->getName() == ZoneNames::TABLE) &&
            owner && owner->getPlayerInfo()->getLocal() &&
            ruledTargetSelectionOid != 0 &&
            RuledActions::isSelectedGraveyardCostObject(ruledGame, ruledTargetSelectionOid)) {
            painter->save();
            painter->setRenderHint(QPainter::Antialiasing, true);
            QPen pen(QColor(255, 165, 0));
            pen.setWidth(4);
            painter->setPen(pen);
            painter->drawPath(shape());
            painter->restore();
        }
        if (zone && zone->getName() == ZoneNames::HAND && owner && owner->getPlayerInfo()->getLocal() &&
            ruledHandler->localPlayerMustCleanupDiscard()) {
            if (zone->getCards().indexOf(const_cast<CardItem *>(this)) >= 0) {
                const int ri =
                    RuledActions::resolveHandActionIndex(ruledHandler, ruled::v1::HAND_ACTION_CLEANUP_DISCARD, this);
                if (ri >= 0 &&
                    ruledHandler->isHandActionLegal(ruled::v1::HAND_ACTION_CLEANUP_DISCARD, ri) &&
                    ruledHandler->isCleanupDiscardHandIndexSelected(ri)) {
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
        if (zone && zone->getName() == ZoneNames::HAND && owner && owner->getPlayerInfo()->getLocal() &&
            ruledHandler->getOpeningUiKind() == RuledClientState::RuledOpeningUiKind::BottomLibrary) {
            if (zone->getCards().indexOf(const_cast<CardItem *>(this)) >= 0) {
                const int ri =
                    RuledActions::resolveHandActionIndex(ruledHandler, ruled::v1::HAND_ACTION_OPENING_BOTTOM, this);
                if (ri >= 0) {
                    const int clickOrder = ruledHandler->openingBottomClickOrderFor(ri);
                    if (clickOrder > 0) {
                        painter->save();
                        painter->setRenderHint(QPainter::Antialiasing, true);
                        QPen pen;
                        pen.setColor(QColor(128, 0, 255));
                        pen.setWidth(4);
                        painter->setPen(pen);
                        painter->drawPath(shape());
                        painter->restore();
                        paintNumberEllipse(clickOrder, 14, QColor(128, 0, 255), -1, 1, painter);
                    }
                }
            }
        }
        // Resolution pick (Brainstorm hand, Gifts Ungiven deck search, Gifts Ungiven opponent popup):
        // number selected cards in click order using cyan.
        // Gate on the pick's own zone before touching the id: candidate ids are only unique
        // within that zone, so an ungated lookup lights up unrelated cards that happen to share
        // an id with a candidate.
        if (RuledActions::isResolutionPickZoneCard(ruledHandler, this)) {
            const int scid = getId();
            if (ruledHandler->isResolutionHandPickCardSelectable(scid)) {
                const int clickOrder = ruledHandler->resolutionHandPickClickOrderFor(scid);
                if (clickOrder > 0) {
                    painter->save();
                    painter->setRenderHint(QPainter::Antialiasing, true);
                    QPen pen;
                    pen.setColor(QColor(0, 200, 200)); // cyan: distinct from mulligan bottom (purple)
                    pen.setWidth(4);
                    painter->setPen(pen);
                    painter->drawPath(shape());
                    painter->restore();
                    paintNumberEllipse(clickOrder, 14, QColor(0, 200, 200), -1, 1, painter);
                } else {
                    // Selectable but not yet selected: draw subtle outline
                    painter->save();
                    painter->setRenderHint(QPainter::Antialiasing, true);
                    QPen pen;
                    pen.setColor(QColor(0, 200, 200, 90));
                    pen.setWidth(2);
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

namespace
{
// Reapply the ruled-owned token display resolution during a full-state resync.
void retargetRuledTokenCardRef(const CardItem *card,
                               CardRef &ref,
                               const QString &pt,
                               const QString &color,
                               const QStringList &keywords,
                               const QStringList &triggeredAbilityTexts)
{
    if (ref.name.isEmpty() || !RuledActions::isRuledGameForCard(card)) {
        return;
    }
    CardRef resolved =
        RuledTokenDisplay::resolve(CardDatabaseManager::query(), ref.name, pt, color, keywords, triggeredAbilityTexts);
    if (!resolved.name.isEmpty()) {
        ref = resolved;
    }
}
} // namespace

void CardItem::processCardInfo(const ServerInfo_Card &_info)
{
    counters.clear();
    const int counterListSize = _info.counter_list_size();
    for (int i = 0; i < counterListSize; ++i) {
        const ServerInfo_CardCounter &counterInfo = _info.counter_list(i);
        counters.insert(counterInfo.id(), counterInfo.value());
    }

    setId(_info.id());
    // A full-state resync (e.g. after a ruled battlefield reorder) carries the engine's bare token
    // subtype as the name; remap it to the matching Oracle "<Subtype> Token" art so it doesn't revert
    // the resolution done at token creation.
    CardRef ref{QString::fromStdString(_info.name()), QString::fromStdString(_info.provider_id())};
    if (!_info.face_down() && _info.has_token_base_pt() && !_info.token_base_pt().empty()) {
        QStringList keywords;
        keywords.reserve(_info.ability_keywords_size());
        for (const auto &kw : _info.ability_keywords()) {
            keywords.append(QString::fromStdString(kw));
        }
        QStringList triggeredAbilityTexts;
        triggeredAbilityTexts.reserve(_info.triggered_ability_texts_size());
        for (const auto &text : _info.triggered_ability_texts()) {
            triggeredAbilityTexts.append(QString::fromStdString(text));
        }
        // `_info.pt()` is the EFFECTIVE P/T (anthem-boosted) from the zone-view sync.
        // Using it for Oracle art lookup would match a higher-P/T token variant
        // (e.g. "Soldier Token        " 2/2) instead of the token's actual 1/1 base.
        // Use `token_base_pt` — the immutable printed stats — when the server provides it.
        const QString lookupPt = _info.has_token_base_pt() && !_info.token_base_pt().empty()
                                     ? QString::fromStdString(_info.token_base_pt())
                                     : QString::fromStdString(_info.pt());
        retargetRuledTokenCardRef(this, ref, lookupPt, QString::fromStdString(_info.color()), keywords,
                                  triggeredAbilityTexts);
    }
    setCardRef(ref);
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
            RuledActions::isRuledGame(game)) {
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

void CardItem::mousePressEvent(QGraphicsSceneMouseEvent *event)
{
    ruledTargetPressConsumed = false;
    if (event->button() == Qt::LeftButton && owner && owner->getGame()) {
        auto *manager = owner->getGame()->getPlayerManager();
        Player *local = manager ? manager->getPlayers().value(manager->getLocalPlayerId(), nullptr) : nullptr;
        PlayerActions *actions = local ? local->getPlayerActions() : nullptr;
        if (actions) {
            const auto eligibility = actions->ruledCardTargetEligibility(this);
            if (eligibility == RuledTargetClickEligibility::Illegal) {
                ruledTargetPressConsumed = true;
                setCursor(Qt::ArrowCursor);
                event->accept();
                return;
            }
            if (eligibility == RuledTargetClickEligibility::Legal) {
                ruledTargetPressConsumed = true;
                setCursor(Qt::CrossCursor);
                event->accept();
                return;
            }
        }
    }
    AbstractCardItem::mousePressEvent(event);
}

void CardItem::playCard(bool faceDown)
{
    // Do nothing if the card belongs to another player
    if (!owner->getPlayerInfo()->getLocalOrJudge())
        return;

    TableZoneLogic *tz = qobject_cast<TableZoneLogic *>(zone);
    if (tz) {
        if (auto *game = owner->getGame();
            RuledActions::isRuledGame(game)) {
            if (RuledActions::gameplayInputLocked(game)) {
                return;
            }
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

/**
 * This method is called when a "click to play" is done on the card.
 * This is either triggered by a single click or double click, depending on the settings.
 *
 * @param shiftHeld if the shift key was held during the click
 */
void CardItem::handleClickedToPlay(bool shiftHeld)
{
    if (isUnwritableRevealZone(zone)) {
        // In ruled mode a reveal-zone popup is an engine-driven pick UI (tutor search, Thoughtseize,
        // Gifts Ungiven), not a freeform reveal window: clicking a candidate selects it in
        // mouseReleaseEvent, and the popup closes when the engine ends the pick. The freeform
        // "hide this card from the window" action would silently drop a candidate from the picker.
        if (RuledActions::isRuledGameForCard(this)) {
            return;
        }
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
        if (RuledActions::tryHandleCombatRightClick(this)) {
            update();
            AbstractCardItem::mouseReleaseEvent(event);
            return;
        }
        // Engine-authored card-action menu on right-click: battlefield/graveyard abilities and,
        // for cyclers in hand, Cast plus Cycle in the same menu. Multi-face cast-only cards keep
        // the dedicated face picker below.
        if (owner != nullptr) {
            auto *game = owner->getGame();
            auto *playerManager = game ? game->getPlayerManager() : nullptr;
            auto *localPlayer = playerManager ? playerManager->getPlayers().value(playerManager->getLocalPlayerId()) : nullptr;
            auto *actions = localPlayer ? localPlayer->getPlayerActions() : nullptr;
            // Spell damage allocation: right-click decrements this target's allocation.
            if (actions && zone && zone->getName() == ZoneNames::TABLE &&
                actions->tryBumpSpellDamageAllocationForCard(this, -1)) {
                update();
                AbstractCardItem::mouseReleaseEvent(event);
                return;
            }
            if (owner->getPlayerInfo()->getLocal() && actions && actions->tryRuledActivateAbilityMenu(this, false)) {
                AbstractCardItem::mouseReleaseEvent(event);
                return;
            }
            if (owner->getPlayerInfo()->getLocal() && actions && actions->tryRuledSpellCastFaceMenu(this)) {
                AbstractCardItem::mouseReleaseEvent(event);
                return;
            }
            // CR 712: MDFC land side-picker ("Play Cragcrown Pathway" / "Play Timbercrown Pathway").
            if (owner->getPlayerInfo()->getLocal() && actions && actions->tryRuledLandPlayFaceMenu(this)) {
                AbstractCardItem::mouseReleaseEvent(event);
                return;
            }
        }
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
            if (stationaryLeft && RuledActions::gameplayInputLocked(game)) {
                // Keep the scene responsive and the last settled board visible, but do not let a
                // second gameplay click mutate local staging while its predecessor is in flight.
                setCursor(Qt::OpenHandCursor);
                AbstractCardItem::mouseReleaseEvent(event);
                return;
            }
            auto *playerManager = game ? game->getPlayerManager() : nullptr;
            auto *localPlayer = playerManager ? playerManager->getPlayers().value(playerManager->getLocalPlayerId()) : nullptr;
            auto *actions = localPlayer ? localPlayer->getPlayerActions() : nullptr;
            if (stationaryLeft && actions) {
                const auto eligibility = actions->ruledCardTargetEligibility(this);
                if (eligibility == RuledTargetClickEligibility::Illegal) {
                    setCursor(Qt::ArrowCursor);
                    event->accept();
                    return;
                }
                if (eligibility == RuledTargetClickEligibility::Legal) {
                    setCursor(Qt::CrossCursor);
                }
            }
            // Tier-3 resolution pick: hand cards (Brainstorm), deck zone-view cards (Gifts Ungiven
            // search), and revealed popup cards (Gifts Ungiven opponent pick).
            if (stationaryLeft && actions && zone &&
                (zone->getName() == ZoneNames::HAND || zone->getName() == ZoneNames::DECK) &&
                actions->tryRuledResolutionHandPickCard(this)) {
                update();
                AbstractCardItem::mouseReleaseEvent(event);
                return;
            }
            // CR 603.3b: clicking a card in the trigger-ordering popup puts that ability on the
            // stack. The popup is built on the deck zone as a scaffold, same as the picks above.
            if (stationaryLeft && owner->getPlayerInfo()->getLocal() && actions && zone &&
                zone->getName() == ZoneNames::DECK && actions->tryRuledTriggerOrderCard(this)) {
                update();
                AbstractCardItem::mouseReleaseEvent(event);
                return;
            }
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
            // Ability target click (handles pending activation or trigger target selection).
            if (stationaryLeft && actions && actions->tryHandleRuledAbilityTargetClick(this)) {
                setCursor(actions->ruledCardTargetEligibility(this) == RuledTargetClickEligibility::Legal
                              ? Qt::CrossCursor
                              : Qt::OpenHandCursor);
                AbstractCardItem::mouseReleaseEvent(event);
                return;
            }
            if (stationaryLeft && actions && actions->tryHandleRuledSpellTargetClick(this)) {
                setCursor(actions->ruledCardTargetEligibility(this) == RuledTargetClickEligibility::Legal
                              ? Qt::CrossCursor
                              : Qt::OpenHandCursor);
                AbstractCardItem::mouseReleaseEvent(event);
                return;
            }
            // Left-click uses the same engine-authored action menu for hand/graveyard abilities;
            // battlefield mana abilities retain their direct CR 605 fast path.
            if (stationaryLeft && owner->getPlayerInfo()->getLocal() && actions && zone &&
                actions->tryRuledActivateAbilityMenu(this, true)) {
                setCursor(Qt::OpenHandCursor);
                AbstractCardItem::mouseReleaseEvent(event);
                return;
            }
            // Spell damage allocation: left-click increments this target's allocation.
            if (stationaryLeft && actions && zone && zone->getName() == ZoneNames::TABLE &&
                actions->tryBumpSpellDamageAllocationForCard(this, +1)) {
                update();
                setCursor(Qt::OpenHandCursor);
                AbstractCardItem::mouseReleaseEvent(event);
                return;
            }
        }
        // Ruled-mode combat clicks take priority over normal play handling on the table.
        if (stationaryLeft && RuledActions::tryHandleCombatClick(this)) {
            update();
            if (owner != nullptr) {
                setCursor(Qt::OpenHandCursor);
            }
            AbstractCardItem::mouseReleaseEvent(event);
            return;
        }
        if (stationaryLeft &&
            (!SettingsCache::instance().getDoubleClickToPlay() || RuledActions::isSingleClickPlayLegal(this) ||
             isTableLandSingleClickLegal(this))) {
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
    if (event->button() == Qt::LeftButton && ruledTargetPressConsumed) {
        ruledTargetPressConsumed = false;
        event->accept();
        return;
    }
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

void CardItem::triggerTapAnimationFrom(int startAngle)
{
    if (!SettingsCache::instance().getTapAnimation() || !scene()) {
        return;
    }
    tapAngle = startAngle;
    static_cast<GameScene *>(scene())->registerAnimationItem(this);
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
