#ifndef RULED_E2E_COMMON_H
#define RULED_E2E_COMMON_H
#include <QByteArray>
#include <QCoreApplication>
#include <QDir>
#include <QDirIterator>
#include <QElapsedTimer>
#include <QFile>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QProcess>
#include <QProcessEnvironment>
#include <QRegularExpression>
#include <QString>
#include <QStringList>
#include <QTcpSocket>
#include <QTemporaryDir>
#include <algorithm>
#include <array>
#include <gtest/gtest.h>
#include <libcockatrice/protocol/pb/command_deck_select.pb.h>
#include <libcockatrice/protocol/pb/command_ready_start.pb.h>
#include <libcockatrice/protocol/pb/command_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/commands.pb.h>
#include <libcockatrice/protocol/pb/event_create_token.pb.h>
#include <libcockatrice/protocol/pb/event_flip_card.pb.h>
#include <libcockatrice/protocol/pb/event_game_joined.pb.h>
#include <libcockatrice/protocol/pb/event_game_state_changed.pb.h>
#include <libcockatrice/protocol/pb/event_list_games.pb.h>
#include <libcockatrice/protocol/pb/event_list_rooms.pb.h>
#include <libcockatrice/protocol/pb/event_move_card.pb.h>
#include <libcockatrice/protocol/pb/event_notify_user.pb.h>
#include <libcockatrice/protocol/pb/event_reveal_cards.pb.h>
#include <libcockatrice/protocol/pb/event_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/event_set_card_attr.pb.h>
#include <libcockatrice/protocol/pb/game_commands.pb.h>
#include <libcockatrice/protocol/pb/game_event.pb.h>
#include <libcockatrice/protocol/pb/game_event_container.pb.h>
#include <libcockatrice/protocol/pb/response.pb.h>
#include <libcockatrice/protocol/pb/room_commands.pb.h>
#include <libcockatrice/protocol/pb/room_event.pb.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <libcockatrice/protocol/pb/server_message.pb.h>
#include <libcockatrice/protocol/pb/session_commands.pb.h>
#include <libcockatrice/protocol/pb/session_event.pb.h>
#include <libcockatrice/protocol/ruled_diagnostic_reader.h>
#include <libcockatrice/utility/zone_names.h>
#include <map>
#include <optional>
#include <set>
#include <string>
#include <vector>

namespace ruled_e2e
{
constexpr quint64 kForcedSeed = 421700421700ULL;
constexpr int kServatricePort = 47997;
constexpr int kTriceRulesPort = 17391;
constexpr int kOverallDeadlineMs = 120 * 1000;

QString envOr(const char *name, const QString &fallback);
QString servatriceExePath();
QString triceRulesExePath();
QString deckXml(const std::vector<std::pair<int, QString>> &mainboard);
bool waitForPortOpen(int port, int timeoutMs);
} // namespace ruled_e2e
#endif
