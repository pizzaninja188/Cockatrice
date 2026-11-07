#ifndef RULED_INTERACTION_UI_H
#define RULED_INTERACTION_UI_H

#include "ruled_interaction_visual.h"

class CardItem;
class PlayerTarget;

namespace RuledInteractionUi
{
void connectCard(CardItem *card);
void connectPlayer(PlayerTarget *player);
void paintCard(CardItem *card, QPainter *painter);
void paintPlayer(PlayerTarget *player, QPainter *painter, const QRectF &avatar);
} // namespace RuledInteractionUi

#endif
