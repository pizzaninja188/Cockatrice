#ifndef COCKATRICE_RULED_REPLACEMENT_PICKER_H
#define COCKATRICE_RULED_REPLACEMENT_PICKER_H

#include <QObject>
#include <QPointer>

class AbstractGame;
class GameScene;
class RuledClientState;
class ZoneViewWidget;

/// Owns the one public replacement-effect image window; choice state belongs to RuledClientState.
class RuledReplacementPicker : public QObject
{
    Q_OBJECT
public:
    RuledReplacementPicker(AbstractGame *game, GameScene *scene, RuledClientState *state, QObject *parent);
    ~RuledReplacementPicker() override;

private:
    void refresh();
    void close();
    QPointer<AbstractGame> game;
    QPointer<GameScene> scene;
    QPointer<RuledClientState> state;
    QPointer<ZoneViewWidget> view;
    quint64 renderedRevision = 0;
};

#endif
