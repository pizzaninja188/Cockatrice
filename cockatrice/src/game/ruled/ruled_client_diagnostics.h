#ifndef RULED_CLIENT_DIAGNOSTICS_H
#define RULED_CLIENT_DIAGNOSTICS_H
#include <QJsonObject>
#include <QObject>
#include <memory>
class AbstractGame;
class RuledDiagnosticJournal;
class PendingCommand;
class GameEventContainer;
namespace ruled::v1
{
class RuledCommand;
}
namespace google::protobuf
{
class Message;
}

/// Recipient-only evidence, scoped to one game tab. Never queries server hidden state.
class RuledClientDiagnostics : public QObject
{
public:
    explicit RuledClientDiagnostics(AbstractGame *game, QObject *parent);
    ~RuledClientDiagnostics() override;
    bool ensure();
    void received(const GameEventContainer &events);
    void prepared(PendingCommand *command);
    void blocked(const google::protobuf::Message &command, const QString &reason);
    void snapshot(const QString &correlation = {});
    QJsonObject currentState() const;
    QString markReport(const QString &reportId);
    QString directory() const;
    QString error() const;

protected:
    bool eventFilter(QObject *watched, QEvent *event) override;

private:
    AbstractGame *game;
    std::unique_ptr<RuledDiagnosticJournal> journal;
    QString lastServerCapture;
    quint64 submitted = 0;
};
#endif
