#ifndef RULED_CLIENT_UI_H
#define RULED_CLIENT_UI_H
#include <QList>
#include <QString>
class QColor;
struct ArrowData;
template <class T> class QSharedPointer;
class AbstractGame;
class TabGame;
class GameScene;
class MessageLogWidget;
class PlayerLogic;
class PlayerGraphicsItem;
class CardState;
class CardItem;
class CardList;
class CardZoneLogic;
class CardZone;
class ZoneViewZone;
class RuledPlayerActions;
class RuledGameController;
class RuledClientHost;
// View-only projections of upstream model identities; never infer engine legality here.
class RuledCardViews : public QList<CardItem *>
{
    bool known = true;

public:
    explicit RuledCardViews(bool contentsKnown = true) : known(contentsKnown)
    {
    }
    bool getContentsKnown() const
    {
        return known;
    }
    CardItem *findCard(int id) const;
};
namespace RuledActions
{
QSharedPointer<ArrowData> arrowData(PlayerLogic *creator, int id, const QColor &color);
RuledGameController *controller(const AbstractGame *game);
RuledClientHost *host(const AbstractGame *game);
TabGame *tab(const AbstractGame *game);
GameScene *scene(const AbstractGame *game);
GameScene *scene(const PlayerLogic *player);
MessageLogWidget *log(const AbstractGame *game);
PlayerGraphicsItem *graphics(const PlayerLogic *player);
RuledPlayerActions *actions(const PlayerLogic *player);
CardItem *cardView(CardState *state);
CardItem *card(const AbstractGame *game, int playerId, const QString &zone, int cardId);
RuledCardViews cardItems(const CardList &cards);
CardZone *zoneGraphics(CardZoneLogic *zone);
ZoneViewZone *zoneView(CardZoneLogic *zone);
} // namespace RuledActions
#endif
