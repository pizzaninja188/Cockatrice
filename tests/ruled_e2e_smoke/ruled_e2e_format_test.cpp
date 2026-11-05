#include "ruled_e2e_client.h"
#include "ruled_e2e_session.h"

#include <libcockatrice/protocol/pb/command_set_sideboard_lock.pb.h>
#include <libcockatrice/protocol/pb/command_set_sideboard_plan.pb.h>
#include <libcockatrice/protocol/pb/context_deck_select.pb.h>
#include <libcockatrice/protocol/pb/response_deck_download.pb.h>

namespace ruled_e2e
{
namespace
{
Response exchange(SmokeClient &client, CommandContainer &container)
{
    const auto id = client.nextCmdId;
    client.sendContainer(container);
    EXPECT_TRUE(client.pumpUntil([&] { return client.responses.count(id) > 0; }, 10000, "format response"));
    return client.responses.at(id);
}
QString commanderDeck(int islands = 99, int declarations = 1)
{
    QString commanders;
    for (int i = 0; i < declarations; ++i) {
        commanders += "<commander>Kami of the Crescent Moon</commander>";
    }
    return QStringLiteral("<cockatrice_deck version=\"1\"><commanders>%1</commanders><zone name=\"main\"><card "
                          "number=\"%2\" name=\"Island\"/></zone></cockatrice_deck>")
        .arg(commanders)
        .arg(islands);
}
class FormatClient : public SmokeClient
{
public:
    using SmokeClient::SmokeClient;
    std::set<int> commandOwners;
    std::map<int, int> startingLife;
    int deckSelectContexts = 0;
    bool sawLobbyDeckList = false;
    void onGameEventContext(const GameEventContext &context) override
    {
        if (context.HasExtension(Context_DeckSelect::ext)) {
            ++deckSelectContexts;
            sawLobbyDeckList |= context.GetExtension(Context_DeckSelect::ext).has_deck_list();
        }
    }
    void onPhysicalEvent(const GameEvent &event) override
    {
        if (event.HasExtension(Event_GameStateChanged::ext)) {
            const auto &state = event.GetExtension(Event_GameStateChanged::ext);
            if (!state.game_started()) {
                for (const auto &player : state.player_list()) {
                    sawLobbyDeckList |= player.properties().player_id() != myId && player.has_deck_list();
                }
            }
            if (state.game_started()) {
                for (const auto &player : state.player_list()) {
                    for (const auto &counter : player.counter_list()) {
                        if (counter.name() == "life") {
                            startingLife[player.properties().player_id()] = counter.count();
                        }
                    }
                }
            }
        }
    }
    void onRuledEvent(const ruled::v1::RuledEvent &event) override
    {
        if (event.has_command_zone_object_map()) {
            commandOwners.clear();
            for (const auto &entry : event.command_zone_object_map().entries()) {
                commandOwners.insert(entry.player_id());
            }
        }
    }
};
void expectEngineLife(const QString &root, int life)
{
    QDirIterator states(root, {"engine-latest.json"}, QDir::Files, QDirIterator::Subdirectories);
    ASSERT_TRUE(states.hasNext());
    QFile file(states.next());
    ASSERT_TRUE(file.open(QIODevice::ReadOnly));
    const auto players =
        QJsonDocument::fromJson(file.readAll()).object().value("state").toObject().value("players").toArray();
    ASSERT_EQ(players.size(), 2);
    for (const auto &player : players) {
        EXPECT_EQ(player.toObject().value("life").toInt(), life);
    }
}

} // namespace

TEST_F(RuledE2ESmokeTest, StandardFormatValidatesSelectionEditorAndSideboardingPrivately)
{
    const auto started = startServers();
    ASSERT_TRUE(started) << started.message();
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }
    FormatClient a("formatstandard", &transcript), b("formatobserver", &transcript);
    ASSERT_TRUE(a.loginAndJoinRoom());
    ASSERT_TRUE(b.loginAndJoinRoom());
    CommandContainer creation;
    creation.set_room_id(a.roomId);
    auto *create = creation.add_room_command()->MutableExtension(Command_CreateGame::ext);
    create->set_ruled_game(true);
    create->set_format(ruled::v1::GAME_FORMAT_STANDARD);
    create->set_max_players(2);
    create->set_share_decklists_on_load(true);
    ASSERT_EQ(exchange(a, creation).response_code(), Response::RespOk);
    ASSERT_TRUE(a.pumpUntil([&] { return a.gameId >= 0 && a.myId >= 0; }, 10000, "game created"));
    ASSERT_TRUE(a.selectDeck(deckXml({{60, "Forest"}})));
    ASSERT_TRUE(b.joinRuledGame(a.gameId));
    ASSERT_TRUE(b.pumpUntil([&] { return b.deckSelectContexts >= 1; }, 10000, "late-join deck selection context"));
    EXPECT_FALSE(b.sawLobbyDeckList);
    b.sawLobbyDeckList = false;
    ASSERT_TRUE(a.selectDeck(deckXml({{60, "Forest"}})));
    ASSERT_TRUE(b.pumpUntil([&] { return b.deckSelectContexts >= 2; }, 10000, "live deck selection context"));
    EXPECT_FALSE(b.sawLobbyDeckList);
    const auto invalid = deckXml({{59, "Forest"}, {1, "Black Lotus"}});
    CommandContainer selection;
    selection.set_game_id(a.gameId);
    selection.add_game_command()->MutableExtension(Command_DeckSelect::ext)->set_deck(invalid.toStdString());
    const auto rejected = exchange(a, selection);
    EXPECT_EQ(rejected.response_code(), Response::RespInvalidData);
    ASSERT_TRUE(rejected.HasExtension(Response_RuledDeckValidation::ext));
    const auto &report = rejected.GetExtension(Response_RuledDeckValidation::ext).validation();
    EXPECT_EQ(report.legality(), ruled::v1::DECK_LEGALITY_ILLEGAL);
    EXPECT_FALSE(report.engine_supported());
    EXPECT_FALSE(report.catalog_hash().empty());
    (void)b.pump(100);
    EXPECT_EQ(b.notifyCustomCount, 0);
    CommandContainer query;
    auto *request = query.add_session_command()->MutableExtension(Command_RuledValidateDeck::ext);
    request->set_deck(invalid.toStdString());
    request->set_format(ruled::v1::GAME_FORMAT_STANDARD);
    const auto checked = exchange(a, query);
    ASSERT_EQ(checked.response_code(), Response::RespOk);
    const auto &editorReport = checked.GetExtension(Response_RuledDeckValidation::ext).validation();
    EXPECT_EQ(editorReport.SerializeAsString(), report.SerializeAsString());
    CommandContainer unlock;
    unlock.set_game_id(a.gameId);
    unlock.add_game_command()->MutableExtension(Command_SetSideboardLock::ext)->set_locked(false);
    ASSERT_EQ(exchange(a, unlock).response_code(), Response::RespOk);
    CommandContainer sideboard;
    sideboard.set_game_id(a.gameId);
    auto *move = sideboard.add_game_command()->MutableExtension(Command_SetSideboardPlan::ext)->add_move_list();
    move->set_start_zone("main");
    move->set_target_zone("side");
    move->set_card_name("Forest");
    EXPECT_EQ(exchange(a, sideboard).response_code(), Response::RespInvalidData);
    ASSERT_TRUE(b.selectDeck(deckXml({{60, "Island"}})));
    a.sendReady();
    b.sendReady();
    ASSERT_TRUE(a.pumpUntil([&] { return a.gameStarted && a.stateVersion > 0; }, 20000,
                            "Standard start with retained accepted deck"));
    ASSERT_TRUE(b.pumpUntil([&] { return b.gameStarted && b.stateVersion > 0; }, 20000, "Standard observer start"));
    EXPECT_EQ(a.startingLife[a.myId], 20);
    EXPECT_EQ(a.startingLife[b.myId], 20);
    EXPECT_EQ(b.notifyCustomCount, 0);
    expectEngineLife(tempDir.filePath("captures"), 20);
}

TEST_F(RuledE2ESmokeTest, CommanderFormatCreatesFortyLifeAndPublicCommandBindings)
{
    const auto started = startServers();
    ASSERT_TRUE(started) << started.message();
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }
    FormatClient a("formatcommander", &transcript), b("formatpartner", &transcript);
    ASSERT_TRUE(a.loginAndJoinRoom());
    ASSERT_TRUE(b.loginAndJoinRoom());
    ASSERT_TRUE(a.createRuledGame(2, ruled::v1::GAME_FORMAT_COMMANDER));
    ASSERT_TRUE(b.joinRuledGame(a.gameId));
    ASSERT_TRUE(a.selectDeck(commanderDeck()));
    ASSERT_TRUE(b.selectDeck(commanderDeck()));
    CommandContainer invalid;
    invalid.set_game_id(a.gameId);
    invalid.add_game_command()->MutableExtension(Command_DeckSelect::ext)->set_deck(commanderDeck(98, 2).toStdString());
    const auto rejected = exchange(a, invalid);
    EXPECT_EQ(rejected.response_code(), Response::RespInvalidData);
    EXPECT_EQ(rejected.GetExtension(Response_RuledDeckValidation::ext).validation().legality(),
              ruled::v1::DECK_LEGALITY_ILLEGAL);
    a.sendReady();
    b.sendReady();
    ASSERT_TRUE(a.pumpUntil([&] { return a.gameStarted && a.stateVersion > 0; }, 20000, "Commander start"));
    ASSERT_TRUE(b.pumpUntil([&] { return b.gameStarted && b.stateVersion > 0; }, 20000, "Commander observer start"));
    EXPECT_EQ(a.startingLife[a.myId], 40);
    EXPECT_EQ(a.startingLife[b.myId], 40);
    EXPECT_EQ(a.commandOwners, (std::set<int>{a.myId, b.myId}));
    EXPECT_EQ(b.commandOwners, a.commandOwners);
    expectEngineLife(tempDir.filePath("captures"), 40);
}
} // namespace ruled_e2e
