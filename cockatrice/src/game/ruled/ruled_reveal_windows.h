#ifndef COCKATRICE_RULED_REVEAL_WINDOWS_H
#define COCKATRICE_RULED_REVEAL_WINDOWS_H

#include <QMap>
#include <QObject>
#include <QPointer>

class AbstractGame;
class GameScene;
class RuledClientState;
class ZoneViewWidget;

// The only public-reveal window owner. Private look/search windows keep their private lifecycle.
class RuledRevealWindows : public QObject
{
    Q_OBJECT
public:
    RuledRevealWindows(AbstractGame *game, GameScene *scene, RuledClientState *state, QObject *parent);
    ~RuledRevealWindows() override;

private:
    void refresh();
    QPointer<AbstractGame> game;
    QPointer<GameScene> scene;
    QPointer<RuledClientState> state;
    QMap<QString, QPointer<ZoneViewWidget>> windows;
};

#endif
