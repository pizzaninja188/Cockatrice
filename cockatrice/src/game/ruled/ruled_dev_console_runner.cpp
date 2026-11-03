#include "ruled_dev_console_runner.h"

#include "ruled_dev_command_parser.h"

#include <QPointer>
#include <QTimer>

RuledDevConsoleRunner::RuledDevConsoleRunner(Sender sender, QObject *parent)
    : QObject(parent), sender(std::move(sender))
{
}

void RuledDevConsoleRunner::submit(const QString &line, int localPlayerId, const QVector<int> &seatIds)
{
    if (busy)
        return;
    const auto parsed = RuledDevCommandParser::parse(line, localPlayerId, seatIds);
    if (!parsed.ok) {
        finish(parsed.handledLocally ? parsed.message : parsed.error, !parsed.handledLocally);
        return;
    }
    commands = {parsed.command};
    lines = {line};
    presetName.clear();
    nextCommand = 0;
    cancelled = false;
    busy = true;
    ++generation;
    emit busyChanged(true);
    dispatchNext();
}

void RuledDevConsoleRunner::runPreset(const QString &name,
                                      const QString &text,
                                      int localPlayerId,
                                      const QVector<int> &seatIds)
{
    if (busy)
        return;
    if (text.size() > 65536 || text.contains(QChar::Null)) {
        finish(tr("Preset must be a text file of at most 64 KiB."), true);
        return;
    }
    QVector<ruled::v1::RuledCommand> prepared;
    QStringList preparedLines;
    const auto sourceLines = text.split(QLatin1Char('\n'));
    for (int i = 0; i < sourceLines.size(); ++i) {
        QString line = sourceLines.at(i).trimmed();
        if (i == 0 && line.startsWith(QChar(0xfeff)))
            line.remove(0, 1);
        if (line.isEmpty() || line.startsWith(QLatin1Char('#')))
            continue;
        const auto parsed = RuledDevCommandParser::parse(line, localPlayerId, seatIds);
        if (!parsed.ok || prepared.size() >= 128) {
            const QString error = prepared.size() >= 128  ? tr("At most 128 commands are allowed.")
                                  : parsed.handledLocally ? tr("Help is not a setup command.")
                                                          : parsed.error;
            finish(tr("Preset line %1: %2 Nothing sent.").arg(i + 1).arg(error), true);
            return;
        }
        prepared.append(parsed.command);
        preparedLines.append(line);
    }
    if (prepared.isEmpty()) {
        finish(tr("Preset contains no commands. Nothing sent."), true);
        return;
    }
    // Resolve ordinals once, against this session's actual seats, before the first send.
    commands = std::move(prepared);
    lines = std::move(preparedLines);
    presetName = name;
    nextCommand = 0;
    cancelled = false;
    busy = true;
    ++generation;
    emit busyChanged(true);
    dispatchNext();
}

void RuledDevConsoleRunner::dispatchNext()
{
    if (!busy || pending)
        return;
    if (cancelled) {
        finish(tr("Cancelled. Accepted commands remain; no rollback."), false);
        return;
    }
    if (nextCommand == commands.size()) {
        finish(presetName.isEmpty()
                   ? tr("Accepted by the engine.")
                   : tr("%1: all %2 setup commands accepted. Prepared test state; no reachability proof.")
                         .arg(presetName)
                         .arg(commands.size()),
               false);
        return;
    }
    const int index = nextCommand++;
    pending = true;
    const quint64 run = generation;
    const quint64 request = ++submission;
    emit statusChanged(
        tr("Waiting for engine acceptance (%1/%2): %3").arg(index + 1).arg(commands.size()).arg(lines.at(index)),
        false);
    // A sender may synchronously reject and finish (which clears commands).
    const auto command = commands.at(index);
    sender(command, [self = QPointer<RuledDevConsoleRunner>(this), run, request](bool accepted) {
        if (!self || self->generation != run || self->submission != request || !self->pending)
            return;
        self->pending = false;
        if (!accepted) {
            self->finish(self->tr("Stopped at command %1: rejected or unavailable. See the game log. "
                                  "Earlier accepted commands remain; no rollback.")
                             .arg(self->nextCommand),
                         true);
            return;
        }
        // Leave response processing before sending again; cancel/reset may intervene in this gap.
        QTimer::singleShot(0, self, [self, run] {
            if (self && self->generation == run)
                self->dispatchNext();
        });
    });
}

void RuledDevConsoleRunner::finish(const QString &message, bool isError)
{
    ++generation;
    commands.clear();
    lines.clear();
    pending = false;
    busy = false;
    emit busyChanged(false);
    emit statusChanged(message, isError);
}

void RuledDevConsoleRunner::cancel()
{
    if (!busy)
        return;
    cancelled = true;
    if (pending) {
        emit statusChanged(tr("Cancelling: waiting for the command already sent. Its effects cannot be undone."),
                           false);
    } else {
        dispatchNext();
    }
}

void RuledDevConsoleRunner::reset(const QString &reason)
{
    const bool wasBusy = busy;
    finish(wasBusy ? tr("Stopped: %1. Accepted commands remain; no automatic resume.").arg(reason) : QString(),
           wasBusy);
}
