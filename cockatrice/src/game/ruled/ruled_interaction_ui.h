#ifndef RULED_INTERACTION_UI_H
#define RULED_INTERACTION_UI_H

#include "ruled_interaction_visual.h"

class CardItem;
class PlayerTarget;
class GameScene;
class ArrowItem;
class PileZone;
class QWidget;
class RuledClientState;

namespace RuledInteractionUi
{
void connectCard(CardItem *card);
void connectPlayer(PlayerTarget *player);
void connectPile(PileZone *pile);
void paintPile(PileZone *pile, QPainter *painter);
void refreshLibraryTop(QWidget *widget, const RuledClientState &state);
void paintCard(CardItem *card, QPainter *painter);
void paintPlayer(PlayerTarget *player, QPainter *painter, const QRectF &avatar);
void refreshScene(GameScene *scene);
[[nodiscard]] qreal arrowOpacity(const ArrowItem *arrow);
} // namespace RuledInteractionUi

#endif
