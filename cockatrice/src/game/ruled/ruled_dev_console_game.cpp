#include "ruled_dev_console_game.h"

#include "../abstract_game.h"
#include "ruled_actions.h"
#include "ruled_client_state.h"
#include "ruled_dev_console.h"
#include "ruled_dev_console_runner.h"

#include <libcockatrice/network/client/abstract/abstract_client.h>

namespace
{
QVector<int> seatIds(AbstractGame *game)
{
    // QMap keys are ordered. Materialize the list before taking iterators.
    const QList<int> keys = game->getPlayerManager()->getPlayers().keys();
    return QVector<int>(keys.cbegin(), keys.cend());
}
} // namespace

void connectRuledDevConsole(RuledDevConsoleWidget *console, AbstractGame *game)
{
    auto *runner = new RuledDevConsoleRunner(
        [game](const ruled::v1::RuledCommand &command, std::function<void(bool)> onFinished) {
            RuledActions::sendRuledCommandExpectingAck(game, command, std::move(onFinished));
        },
        console);
    QObject::connect(runner, &RuledDevConsoleRunner::busyChanged, console, &RuledDevConsoleWidget::setBusy);
    QObject::connect(runner, &RuledDevConsoleRunner::statusChanged, console, &RuledDevConsoleWidget::setStatus);
    QObject::connect(console, &RuledDevConsoleWidget::commandSubmitted, runner, [runner, game](const QString &line) {
        runner->submit(line, game->getPlayerManager()->getLocalPlayerId(), seatIds(game));
    });
    QObject::connect(console, &RuledDevConsoleWidget::presetSubmitted, runner,
                     [runner, game](const QString &name, const QString &text) {
                         runner->runPreset(name, text, game->getPlayerManager()->getLocalPlayerId(), seatIds(game));
                     });
    QObject::connect(console, &RuledDevConsoleWidget::cancelRequested, runner, &RuledDevConsoleRunner::cancel);
    if (auto *state = RuledActions::stateFor(game)) {
        QObject::connect(state, &RuledClientState::sessionReset, runner,
                         [runner] { runner->reset(QObject::tr("game session changed")); });
    }
    QObject::connect(game->getGameEventHandler(), &GameEventHandler::gameClosed, runner,
                     [runner] { runner->reset(QObject::tr("game closed")); });
    if (auto *client = game->getClientForPlayer(-1)) {
        console->setEnabled(client->getStatus() == StatusLoggedIn);
        QObject::connect(client, &AbstractClient::statusChanged, runner, [runner, console](ClientStatus status) {
            if (status != StatusLoggedIn)
                runner->reset(QObject::tr("connection changed"));
            console->setEnabled(status == StatusLoggedIn);
        });
        QObject::connect(client, &QObject::destroyed, runner, [runner, console] {
            runner->reset(QObject::tr("connection closed"));
            console->setEnabled(false);
        });
    } else {
        console->setEnabled(false);
    }
}
