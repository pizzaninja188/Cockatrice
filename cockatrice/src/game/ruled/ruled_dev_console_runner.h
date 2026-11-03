#ifndef RULED_DEV_CONSOLE_RUNNER_H
#define RULED_DEV_CONSOLE_RUNNER_H

#include <QObject>
#include <QStringList>
#include <QVector>
#include <functional>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

/// Client-only sequencing of existing dev commands. The sender must report engine acceptance.
/// Presets prepare test states; neither parsing nor an accepted setup proves gameplay reachability.
class RuledDevConsoleRunner : public QObject
{
    Q_OBJECT
public:
    using Sender = std::function<void(const ruled::v1::RuledCommand &, std::function<void(bool)>)>;
    explicit RuledDevConsoleRunner(Sender sender, QObject *parent = nullptr);

    void submit(const QString &line, int localPlayerId, const QVector<int> &seatIds);
    void runPreset(const QString &name, const QString &text, int localPlayerId, const QVector<int> &seatIds);
    void cancel();
    /// Disconnect / session replacement: invalidate every outstanding callback, without resuming.
    void reset(const QString &reason);

signals:
    void busyChanged(bool busy);
    void statusChanged(const QString &message, bool isError);

private:
    void dispatchNext();
    void finish(const QString &message, bool isError);

    Sender sender;
    QVector<ruled::v1::RuledCommand> commands;
    QStringList lines;
    QString presetName;
    int nextCommand = 0;
    quint64 generation = 0;
    quint64 submission = 0;
    bool busy = false;
    bool pending = false;
    bool cancelled = false;
};

#endif
