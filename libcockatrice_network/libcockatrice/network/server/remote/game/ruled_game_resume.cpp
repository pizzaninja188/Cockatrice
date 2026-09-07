#include "ruled_game_resume.h"

#include <QCryptographicHash>
#include <QFile>
#include <QUuid>
#include <limits>

namespace
{
bool fail(QString *error, const QString &why)
{
    if (error)
        *error = why;
    return false;
}
} // namespace
RuledGameResume::RuledGameResume(const QString &path) : requested(!path.isEmpty())
{
    if (!requested)
        return;
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly) || file.size() > 64LL * 1024 * 1024) {
        errorText = "Cannot read bounded local resume plan";
        return;
    }
    const auto bytes = file.readAll();
    if (!data.ParseFromArray(bytes.constData(), int(bytes.size()))) {
        errorText = "Invalid local resume plan protobuf";
        return;
    }
    validate(data, &errorText);
}
bool RuledGameResume::validate(const ruled::diagnostics::ResumePlan &plan, QString *error)
{
    if (plan.format_version() != 1 || !plan.has_session_start() || plan.engine_build().empty() ||
        plan.card_data_hash().empty() || QUuid(QString::fromStdString(plan.parent_capture_id())).isNull() ||
        plan.initial_state_sha256().size() != 32 || plan.stop_after() != static_cast<quint64>(plan.steps_size()) ||
        !plan.session_start().diagnostic_capture_enabled())
        return fail(error, "Unsupported or incomplete resume plan");
    QSet<int> players;
    for (const auto id : plan.session_start().player_ids()) {
        if (id < 0 || id == std::numeric_limits<int>::max() || players.contains(id))
            return fail(error, "Invalid or duplicate recorded player ID");
        players.insert(id);
    }
    if (players.size() < 2)
        return fail(error, "Resume plan has fewer than two seats");
    QSet<int> decks;
    for (const auto &deck : plan.display_decks()) {
        if (!players.contains(deck.player_id()) || decks.contains(deck.player_id()) ||
            deck.mainboard_card_name().empty())
            return fail(error, "Invalid display deck roster");
        decks.insert(deck.player_id());
    }
    if (players != decks)
        return fail(error, "Display decks do not cover every recorded seat");
    quint64 previous = 0;
    for (const auto &step : plan.steps()) {
        ruled::v1::RuledCommand command;
        if (!step.has_command() || !players.contains(step.command().player_id()) ||
            step.source_sequence() <= previous || step.expected_state_sha256().size() != 32 ||
            !command.ParseFromString(step.command().ruled_command()) ||
            command.cmd_case() == ruled::v1::RuledCommand::CMD_NOT_SET)
            return fail(error, "Invalid accepted resume step");
        previous = step.source_sequence();
    }
    QSet<int> policies;
    for (const auto &policy : plan.restored_auto_pass_policies()) {
        if (!players.contains(policy.player_id()) || policies.contains(policy.player_id()))
            return fail(error, "Invalid policy roster");
        policies.insert(policy.player_id());
    }
    return true;
}
int RuledGameResume::participantId(int fallback, bool spectator, const QSet<int> &occupied) const
{
    if (!valid())
        return fallback;
    int next = fallback;
    for (const auto id : data.session_start().player_ids()) {
        if (!spectator && !occupied.contains(id))
            return id;
        next = qMax(next, id + 1);
    }
    while (occupied.contains(next) && next < std::numeric_limits<int>::max())
        ++next;
    return next;
}
bool RuledGameResume::checkState(const ruled::v1::IpcResponse &response, const std::string &expected, QString *error)
{
    if (!response.ok())
        return fail(error, "Engine refused replayed command: " + QString::fromStdString(response.error()));
    const auto actual = QCryptographicHash::hash(QByteArray::fromStdString(response.diagnostic_state_json()),
                                                 QCryptographicHash::Sha256);
    if (actual != QByteArray::fromStdString(expected))
        return fail(error, "Engine state differs from the verified resume plan");
    return true;
}
bool RuledGameResume::checkStartup(const ruled::v1::IpcResponse &response, QString *error) const
{
    if (!valid())
        return fail(error, errorText);
    if (response.engine_build() != data.engine_build() || response.card_data_hash() != data.card_data_hash() ||
        response.effective_dev_commands_enabled() != data.effective_dev_commands_enabled())
        return fail(error, "Resume engine build, card data, or effective dev gate differs from the verified plan");
    return checkState(response, data.initial_state_sha256(), error);
}
