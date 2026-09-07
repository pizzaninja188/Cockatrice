#include "ruled_server_diagnostics.h"

#include "version_string.h"

#include <QJsonDocument>
#include <libcockatrice/protocol/pb/response.pb.h>
#include <libcockatrice/protocol/ruled_diagnostics.h>

namespace
{
QString captureRoot(const QString &root)
{
    const auto path = root.isEmpty() ? RuledDiagnosticJournal::defaultRoot("server") : root;
    bool ok = false;
    const auto megabytes = qEnvironmentVariable("COCKATRICE_RULED_CAPTURE_QUOTA_MB").toLongLong(&ok);
    RuledDiagnosticJournal::prune(path, ok && megabytes > 0 ? megabytes * 1024 * 1024 : 2LL * 1024 * 1024 * 1024);
    return path;
}
} // namespace

RuledServerDiagnostics::RuledServerDiagnostics(quint64 gameId, const QString &root)
    : capture(captureRoot(root), "server", "server_only", {{"game_id", QString::number(gameId)}}, 512LL * 1024 * 1024)
{
    capture.metadata({{"servatrice_build", VERSION_STRING}, {"build", RuledDiagnostics::buildInfo()}});
}

void RuledServerDiagnostics::clientRequest(int playerId, const Command_RuledPayload &command)
{
    clientRequestId = QString::fromStdString(command.diagnostic_request_id()).left(128);
    capture.record("client_request", command, clientRequestId, {{"authenticated_player_id", playerId}});
}
void RuledServerDiagnostics::clientResult(int code)
{
    capture.note("client_result",
                 {{"response_code", QString::fromStdString(Response::ResponseCode_Name(code))},
                  {"command_index", QString::number(engineCommandIndex)}},
                 clientRequestId);
    clientRequestId.clear();
}
void RuledServerDiagnostics::engineRequest(const ruled::v1::IpcEnvelope &request)
{
    engineCorrelation = "engine-" + QString::number(capture.sequence() + 1);
    lastEngineRequest =
        capture.record("engine_request", request, engineCorrelation, {{"client_request_id", clientRequestId}});
}
void RuledServerDiagnostics::engineResponse(const ruled::v1::IpcResponse &response)
{
    capture.record("engine_response", response, engineCorrelation, {{"client_request_id", clientRequestId}});
    engineCommandIndex = response.diagnostic_command_index();
    if (!response.engine_build().empty()) {
        capture.metadata({{"engine_build", QString::fromStdString(response.engine_build())},
                          {"card_data_hash", QString::fromStdString(response.card_data_hash())},
                          {"effective_dev_commands_enabled", response.effective_dev_commands_enabled()}});
    }
    if (!response.diagnostic_state_json().empty()) {
        const auto state = QJsonDocument::fromJson(QByteArray::fromStdString(response.diagnostic_state_json()));
        if (state.isObject() && !state.object().contains("capture_error"))
            capture.state("engine", state.object(), engineCorrelation);
        else
            capture.incomplete("Invalid or unavailable engine snapshot: " +
                               state.object().value("capture_error").toString());
    }
}
void RuledServerDiagnostics::transportError(const QString &reason)
{
    capture.note("engine_transport_error", {{"reason", reason}}, engineCorrelation);
}
void RuledServerDiagnostics::projection(const ruled::v1::IpcResponse &response)
{
    capture.record("relay_projection", response, engineCorrelation);
}
void RuledServerDiagnostics::delivered(int recipient, const GameEventContainer &event)
{
    capture.record("recipient_event", event, engineCorrelation,
                   {{"recipient_player_id", recipient}, {"client_request_id", clientRequestId}});
}
void RuledServerDiagnostics::decorate(Event_RuledPayload &event) const
{
    auto *context = event.mutable_diagnostic_context();
    context->set_capture_id(capture.id().toStdString());
    context->set_request_sequence(lastEngineRequest);
    context->set_command_index(engineCommandIndex);
    context->set_client_request_id(clientRequestId.toStdString());
    context->set_capture_status(capture.isHealthy() ? "recording" : "incomplete");
}
