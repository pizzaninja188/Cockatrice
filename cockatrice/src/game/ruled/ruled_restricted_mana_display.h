/**
 * @file ruled_restricted_mana_display.h
 * @ingroup GameGraphicsPlayers
 * @brief Displays CR 106.6 mana groups separately from the unrestricted mana pool.
 */

#ifndef COCKATRICE_RULED_RESTRICTED_MANA_DISPLAY_H
#define COCKATRICE_RULED_RESTRICTED_MANA_DISPLAY_H

#include <QGraphicsObject>

class Player;

class RuledRestrictedManaDisplay : public QGraphicsObject
{
    Q_OBJECT

public:
    explicit RuledRestrictedManaDisplay(Player *player, QGraphicsItem *parent = nullptr);

    [[nodiscard]] QRectF boundingRect() const override;
    void paint(QPainter *painter, const QStyleOptionGraphicsItem *option, QWidget *widget) override;
    [[nodiscard]] qreal displayWidth() const;

public slots:
    void refresh();

signals:
    void widthChanged();

private:
    Player *player;
    QRectF bounds;
};

#endif // COCKATRICE_RULED_RESTRICTED_MANA_DISPLAY_H
