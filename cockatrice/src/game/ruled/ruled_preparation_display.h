#ifndef COCKATRICE_RULED_PREPARATION_DISPLAY_H
#define COCKATRICE_RULED_PREPARATION_DISPLAY_H

#include "ruled_client_host.h"
#include <QHash>
#include <QObject>
#include <QPointer>

class AbstractGame;
class CardItem;

/// Materializes only the noncard exile entries identified by the engine and relay.
/// Ordinary exile cards continue to follow physical move events.
class RuledPreparationDisplay : public QObject
{
public:
    RuledPreparationDisplay(AbstractGame *game, QObject *parent);
    void reconcile(const QVector<RuledClientHost::PreparationCopy> &copies);
    static void copyViewState(const CardItem *source, CardItem *view);

private:
    void remove(quint32 oid);
    AbstractGame *game;
    QHash<quint32, QPointer<CardItem>> displayed;
};

#endif
