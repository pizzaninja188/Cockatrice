#include "game_prompt_widget.h"

#include <QPushButton>
#include <gtest/gtest.h>
#include <libcockatrice/game/game.h>
#include <libcockatrice/game/game_event_handler.h>
#include <libcockatrice/network/client/abstract/abstract_client.h>
#include <libcockatrice/protocol/pb/command_next_turn.pb.h>
#include <libcockatrice/protocol/pb/command_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/commands.pb.h>
#include <libcockatrice/protocol/pb/event_game_joined.pb.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>

namespace
{
class PriorityRecordingClient : public AbstractClient
{
public:
    QList<CommandContainer> sent;

protected:
    void sendCommandContainer(const CommandContainer &command) override { sent.append(command); }
};

class GamePromptPriorityTest : public testing::Test
{
protected:
    void clickPassAndExpectEngineCommand(bool stackHasItems)
    {
        PriorityRecordingClient client;
        QList<AbstractClient *> clients{&client};
        Event_GameJoined joined;
        joined.set_player_id(0);
        joined.mutable_game_info()->set_game_id(42);
        joined.mutable_game_info()->set_ruled_game(true);
        Game game(nullptr, false, clients, joined, {});
        GamePromptWidget prompt;
        QObject::connect(&prompt, &GamePromptWidget::passPriorityRequested, game.getGameEventHandler(),
                         &GameEventHandler::handleNextTurn);
        prompt.setLocalPlayerHasPriority(true);
        prompt.setPassPriorityEnabled(true);
        prompt.setRuledStackHasItems(stackHasItems);
        auto *button = prompt.findChild<QPushButton *>(QStringLiteral("passPriorityButton"));
        ASSERT_NE(button, nullptr);
        ASSERT_FALSE(button->isHidden());
        ASSERT_TRUE(button->isEnabled());
        if (stackHasItems) {
            EXPECT_EQ(button->text(), QStringLiteral("No Response"));
        }
        button->click();

        ASSERT_EQ(client.sent.size(), 1);
        const auto &container = client.sent.first();
        EXPECT_EQ(container.game_id(), 42);
        ASSERT_EQ(container.game_command_size(), 1);
        const auto &command = container.game_command(0);
        ASSERT_TRUE(command.HasExtension(Command_RuledPayload::ext));
        EXPECT_FALSE(command.HasExtension(Command_NextTurn::ext));
        ruled::v1::RuledCommand engineCommand;
        ASSERT_TRUE(engineCommand.ParseFromString(command.GetExtension(Command_RuledPayload::ext).payload()));
        EXPECT_TRUE(engineCommand.has_pass_priority());
    }
};

TEST_F(GamePromptPriorityTest, NoResponseClickSendsEnginePassPriority)
{
    clickPassAndExpectEngineCommand(true);
}

TEST_F(GamePromptPriorityTest, EmptyStackAdvanceStillPassesEnginePriority)
{
    clickPassAndExpectEngineCommand(false);
}

TEST_F(GamePromptPriorityTest, FreeformNextTurnRetainsLegacyCommand)
{
    PriorityRecordingClient client;
    QList<AbstractClient *> clients{&client};
    Event_GameJoined joined;
    joined.set_player_id(0);
    joined.mutable_game_info()->set_game_id(42);
    Game game(nullptr, false, clients, joined, {});

    game.getGameEventHandler()->handleNextTurn();

    ASSERT_EQ(client.sent.size(), 1);
    ASSERT_EQ(client.sent.first().game_command_size(), 1);
    const auto &command = client.sent.first().game_command(0);
    EXPECT_TRUE(command.HasExtension(Command_NextTurn::ext));
    EXPECT_FALSE(command.HasExtension(Command_RuledPayload::ext));
}
} // namespace
