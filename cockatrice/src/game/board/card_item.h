/**
 * @file card_item.h
 * @ingroup GameGraphicsCards
 * @brief TODO: Document this.
 */

#ifndef CARDITEM_H
#define CARDITEM_H

#include "../zones/logic/card_zone_logic.h"
#include "abstract_card_item.h"

#include <QStringList>
#include <libcockatrice/network/server/remote/game/server_card.h>

class CardDatabase;
class CardDragItem;
class CardZone;
class ServerInfo_Card;
class Player;
class QAction;
class QColor;

const int MAX_COUNTERS_ON_CARD = 999;
const int ROTATION_DEGREES_PER_FRAME = 10;

class CardItem : public AbstractCardItem
{
    Q_OBJECT
private:
    CardZoneLogic *zone;
    bool attacking;
    QMap<int, int> counters;
    QString annotation;
    QString pt;
    bool destroyOnZoneChange;
    bool doesntUntap;
    QPoint gridPoint;
    CardDragItem *dragItem;
    CardItem *attachedTo;
    QList<CardItem *> attachedCards;

    void prepareDelete();
    void handleClickedToPlay(bool shiftHeld);
public slots:
    void deleteLater();

public:
    enum
    {
        Type = typeCard
    };
    [[nodiscard]] int type() const override
    {
        return Type;
    }
    explicit CardItem(Player *_owner,
                      QGraphicsItem *parent = nullptr,
                      const CardRef &cardRef = {},
                      int _cardid = -1,
                      CardZoneLogic *_zone = nullptr);

    void retranslateUi();
    [[nodiscard]] CardZoneLogic *getZone() const
    {
        return zone;
    }
    void setZone(CardZoneLogic *_zone);
    void paint(QPainter *painter, const QStyleOptionGraphicsItem *option, QWidget *widget) override;
    [[nodiscard]] QPoint getGridPoint() const
    {
        return gridPoint;
    }
    void setGridPoint(const QPoint &_gridPoint)
    {
        gridPoint = _gridPoint;
    }
    [[nodiscard]] QPoint getGridPos() const
    {
        return gridPoint;
    }
    [[nodiscard]] Player *getOwner() const
    {
        return owner;
    }
    void setOwner(Player *_owner)
    {
        owner = _owner;
    }
    [[nodiscard]] bool getAttacking() const
    {
        return attacking;
    }
    void setAttacking(bool _attacking);
    [[nodiscard]] const QMap<int, int> &getCounters() const
    {
        return counters;
    }
    void setCounter(int _id, int _value);
    [[nodiscard]] QString getAnnotation() const
    {
        return annotation;
    }
    void setAnnotation(const QString &_annotation);
    [[nodiscard]] bool getDoesntUntap() const
    {
        return doesntUntap;
    }
    void setDoesntUntap(bool _doesntUntap);
    [[nodiscard]] QString getPT() const
    {
        return pt;
    }
    void setPT(const QString &_pt);
    [[nodiscard]] bool getDestroyOnZoneChange() const
    {
        return destroyOnZoneChange;
    }
    void setDestroyOnZoneChange(bool _destroy)
    {
        destroyOnZoneChange = _destroy;
    }
    [[nodiscard]] CardItem *getAttachedTo() const
    {
        return attachedTo;
    }
    void setAttachedTo(CardItem *_attachedTo);
    void addAttachedCard(CardItem *card)
    {
        attachedCards.append(card);
    }
    void removeAttachedCard(CardItem *card)
    {
        attachedCards.removeOne(card);
    }
    [[nodiscard]] const QList<CardItem *> &getAttachedCards() const
    {
        return attachedCards;
    }
    void resetState(bool keepAnnotations = false);
    void processCardInfo(const ServerInfo_Card &_info);

    // Ruled engine tokens are named by their MTG subtype ("Knight"); the Oracle display DB stores
    // generic tokens as "<Subtype> Token", with same-name variants kept apart by trailing spaces.
    // Given the engine token's characteristics, return the CardRef of the best-matching Oracle token
    // (by P/T, color, keyword abilities) so it shows the right art/details, or an empty CardRef if
    // the family has no entry. Display-only; the engine stays authoritative for rules. Shared by the
    // token-creation event and full-state resync so the resolved name survives a battlefield resync.
    static CardRef resolveRuledTokenDisplayCard(const QString &subtype,
                                                const QString &enginePt,
                                                const QString &engineColor,
                                                const QStringList &engineKeywords);

    bool animationEvent();
    /// Restart the tap animation from `startAngle` degrees, sweeping toward whatever `tapped`
    /// currently says (0 or 90). Used to resume an animation that a zone rebuild interrupted.
    void triggerTapAnimationFrom(int startAngle);
    CardDragItem *createDragItem(int _id, const QPointF &_pos, const QPointF &_scenePos, bool forceFaceDown);
    void deleteDragItem();
    void drawArrow(const QColor &arrowColor);
    void drawAttachArrow();
    void playCard(bool faceDown);

protected:
    void mouseMoveEvent(QGraphicsSceneMouseEvent *event) override;
    void mouseReleaseEvent(QGraphicsSceneMouseEvent *event) override;
    void mouseDoubleClickEvent(QGraphicsSceneMouseEvent *event) override;
    QVariant itemChange(GraphicsItemChange change, const QVariant &value) override;
};

#endif
