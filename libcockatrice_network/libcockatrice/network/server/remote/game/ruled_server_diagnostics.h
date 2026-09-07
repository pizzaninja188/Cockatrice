#ifndef RULED_SERVER_DIAGNOSTICS_H
#define RULED_SERVER_DIAGNOSTICS_H

#include <libcockatrice/protocol/pb/command_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/event_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/game_event_container.pb.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <libcockatrice/protocol/ruled_diagnostic_journal.h>

/// Server-only capture coordinator. Correlation is observational; it never supplies an actor
/// to the engine or changes accepted-command indexing.
class RuledServerDiagnostics
{
public:
    explicit RuledServerDiagnostics(quint64 gameId, const QString &root = {});
    RuledDiagnosticJournal &journal()
    {
        return capture;
    }
    void clientRequest(int authenticatedPlayerId, const Command_RuledPayload &command);
    void clientResult(int responseCode);
    void engineRequest(const ruled::v1::IpcEnvelope &request);
    void engineResponse(const ruled::v1::IpcResponse &response);
    void transportError(const QString &reason);
    void projection(const ruled::v1::IpcResponse &response);
    void delivered(int recipientPlayerId, const GameEventContainer &event);
    void decorate(Event_RuledPayload &event) const;
    quint64 commandIndex() const
    {
        return engineCommandIndex;
    }

private:
    RuledDiagnosticJournal capture;
    QString clientRequestId, engineCorrelation;
    quint64 lastEngineRequest = 0, engineCommandIndex = 0;
};

#endif
