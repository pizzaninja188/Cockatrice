// Offscreen widget tests for the ruled dev console.
//
// The widget previews files, remembers submitted lines, and delegates parsing/sequencing to the
// runner. An injected acknowledged sender exercises async failure and lifecycle behavior.
//
// Follows tests/game_prompt/: offscreen, children found by objectName, visibility asserted with
// isHidden() because nothing is ever shown.

#include "game/ruled/ruled_dev_console.h"
#include "game/ruled/ruled_dev_console_runner.h"

#include <QApplication>
#include <QLabel>
#include <QLineEdit>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QSignalSpy>
#include <QTemporaryFile>
#include <QTest>
#include <gtest/gtest.h>

namespace
{

class RuledDevConsoleTest : public ::testing::Test
{
protected:
    void SetUp() override
    {
        widget = std::make_unique<RuledDevConsoleWidget>();
        input = widget->findChild<QLineEdit *>(QStringLiteral("devConsoleInput"));
        status = widget->findChild<QLabel *>(QStringLiteral("devConsoleStatus"));
        ASSERT_NE(input, nullptr);
        ASSERT_NE(status, nullptr);
    }

    /// Type a line and press return, the way a user would.
    void submit(const QString &line, bool acknowledge = true)
    {
        input->setText(line);
        QTest::keyClick(input, Qt::Key_Return);
        if (acknowledge)
            widget->setBusy(false);
    }

    std::unique_ptr<RuledDevConsoleWidget> widget;
    QLineEdit *input = nullptr;
    QLabel *status = nullptr;
};

TEST_F(RuledDevConsoleTest, SubmittingEmitsTheLineAndClearsTheInput)
{
    QSignalSpy spy(widget.get(), &RuledDevConsoleWidget::commandSubmitted);
    submit(QStringLiteral("put hand Serra Angel"));

    ASSERT_EQ(spy.count(), 1);
    EXPECT_EQ(spy.at(0).at(0).toString(), QStringLiteral("put hand Serra Angel"));
    EXPECT_TRUE(input->text().isEmpty());
}

TEST_F(RuledDevConsoleTest, BlankLinesAreIgnored)
{
    QSignalSpy spy(widget.get(), &RuledDevConsoleWidget::commandSubmitted);
    submit(QStringLiteral("   "));
    EXPECT_EQ(spy.count(), 0);
}

TEST_F(RuledDevConsoleTest, DuplicateSubmissionWaitsForAcknowledgment)
{
    QSignalSpy spy(widget.get(), &RuledDevConsoleWidget::commandSubmitted);
    submit(QStringLiteral("mana 3RR"), false);
    submit(QStringLiteral("mana 3RR"), false);
    EXPECT_EQ(spy.count(), 1) << "a pending command must not be submitted twice";
}

TEST_F(RuledDevConsoleTest, UpAndDownWalkTheHistory)
{
    submit(QStringLiteral("mana 3RR"));
    submit(QStringLiteral("put bf Grizzly Bears"));

    QTest::keyClick(input, Qt::Key_Up);
    EXPECT_EQ(input->text(), QStringLiteral("put bf Grizzly Bears"));
    QTest::keyClick(input, Qt::Key_Up);
    EXPECT_EQ(input->text(), QStringLiteral("mana 3RR"));
    // Already at the oldest entry: staying put beats wrapping around to the newest.
    QTest::keyClick(input, Qt::Key_Up);
    EXPECT_EQ(input->text(), QStringLiteral("mana 3RR"));

    QTest::keyClick(input, Qt::Key_Down);
    EXPECT_EQ(input->text(), QStringLiteral("put bf Grizzly Bears"));
    // Stepping past the newest returns to a blank line, the way a shell does.
    QTest::keyClick(input, Qt::Key_Down);
    EXPECT_TRUE(input->text().isEmpty());
}

TEST_F(RuledDevConsoleTest, RepeatedCommandsAreRecordedOnce)
{
    submit(QStringLiteral("mana 3RR"));
    submit(QStringLiteral("mana 3RR"));

    QTest::keyClick(input, Qt::Key_Up);
    EXPECT_EQ(input->text(), QStringLiteral("mana 3RR"));
    // Only one entry, so going further back cannot find a second copy.
    QTest::keyClick(input, Qt::Key_Up);
    EXPECT_EQ(input->text(), QStringLiteral("mana 3RR"));
    QTest::keyClick(input, Qt::Key_Down);
    EXPECT_TRUE(input->text().isEmpty());
}

TEST_F(RuledDevConsoleTest, StatusShowsAndClears)
{
    EXPECT_TRUE(status->isHidden()) << "no status until something is pushed";

    widget->setStatus(QStringLiteral("Unknown zone 'sideboard'."), true);
    EXPECT_FALSE(status->isHidden());
    EXPECT_EQ(status->text(), QStringLiteral("Unknown zone 'sideboard'."));

    widget->setStatus(QString(), false);
    EXPECT_TRUE(status->isHidden());
}

TEST_F(RuledDevConsoleTest, SubmittingClearsAStaleStatus)
{
    widget->setStatus(QStringLiteral("Unknown zone 'sideboard'."), true);
    submit(QStringLiteral("put hand Serra Angel"));
    EXPECT_TRUE(status->isHidden()) << "an error from the previous line must not linger";
}

TEST_F(RuledDevConsoleTest, EnabledFlagIsOffUnlessSetOnTheCommandLine)
{
    EXPECT_FALSE(RuledDevConsoleWidget::isEnabled());
    RuledDevConsoleWidget::setEnabledFromCommandLine(true);
    EXPECT_TRUE(RuledDevConsoleWidget::isEnabled());
    RuledDevConsoleWidget::setEnabledFromCommandLine(false);
}

TEST_F(RuledDevConsoleTest, LoadingPreviewsWithoutSendingAndRunLocksDuplicateClicks)
{
    QTemporaryFile file;
    ASSERT_TRUE(file.open());
    file.write("# test\r\nmana 3RR\r\nput hand Serra Angel\r\n");
    file.flush();
    QSignalSpy spy(widget.get(), &RuledDevConsoleWidget::presetSubmitted);
    ASSERT_TRUE(widget->loadPreset(file.fileName()));
    EXPECT_EQ(spy.count(), 0);
    auto *preview = widget->findChild<QPlainTextEdit *>(QStringLiteral("devPresetPreview"));
    ASSERT_NE(preview, nullptr);
    EXPECT_FALSE(preview->isHidden());
    EXPECT_TRUE(preview->isReadOnly());
    EXPECT_TRUE(preview->toPlainText().contains(QStringLiteral("mana 3RR")));
    auto *run = widget->findChild<QPushButton *>(QStringLiteral("devPresetRun"));
    ASSERT_NE(run, nullptr);
    run->click();
    run->click();
    EXPECT_EQ(spy.count(), 1);
    EXPECT_FALSE(input->isEnabled());
    EXPECT_FALSE(widget->loadPreset(file.fileName()));
    widget->setBusy(false);
    run->click();
    EXPECT_EQ(spy.count(), 2) << "rerunning requires another explicit Run action";
}

TEST_F(RuledDevConsoleTest, InvalidOrOversizedFileDoesNotReplaceLoadedPreset)
{
    QTemporaryFile file;
    ASSERT_TRUE(file.open());
    file.write("mana R");
    file.flush();
    ASSERT_TRUE(widget->loadPreset(file.fileName()));
    auto *preview = widget->findChild<QPlainTextEdit *>(QStringLiteral("devPresetPreview"));
    file.resize(0);
    file.write(QByteArray(65537, 'x'));
    file.flush();
    EXPECT_FALSE(widget->loadPreset(file.fileName()));
    EXPECT_EQ(preview->toPlainText(), QStringLiteral("mana R"));
    file.resize(0);
    file.write(QByteArray("\xff\xfe", 2));
    file.flush();
    EXPECT_FALSE(widget->loadPreset(file.fileName()));
    EXPECT_EQ(preview->toPlainText(), QStringLiteral("mana R"));
}

TEST_F(RuledDevConsoleTest, Utf8BomLoadsAndRunsThroughTheRealRunner)
{
    QTemporaryFile file;
    ASSERT_TRUE(file.open());
    file.write(QByteArray::fromHex("efbbbf") + "# prepared state\r\nmana U\r\n");
    file.flush();
    int sent = 0;
    RuledDevConsoleRunner runner([&sent](const ruled::v1::RuledCommand &command, std::function<void(bool)> ack) {
        ++sent;
        EXPECT_EQ(command.dev_command().add_mana().u(), 1);
        ack(true);
    });
    QObject::connect(&runner, &RuledDevConsoleRunner::busyChanged, widget.get(), &RuledDevConsoleWidget::setBusy);
    QObject::connect(widget.get(), &RuledDevConsoleWidget::presetSubmitted, &runner,
                     [&runner](const QString &name, const QString &text) { runner.runPreset(name, text, 0, {0, 1}); });
    ASSERT_TRUE(widget->loadPreset(file.fileName()));
    widget->findChild<QPushButton *>(QStringLiteral("devPresetRun"))->click();
    EXPECT_EQ(sent, 1);
    QApplication::processEvents();
    EXPECT_TRUE(input->isEnabled());
}

TEST_F(RuledDevConsoleTest, RealRunnerUnlocksAfterHelpParseFailureAndEngineRejection)
{
    std::function<void(bool)> pending;
    RuledDevConsoleRunner runner(
        [&pending](const ruled::v1::RuledCommand &, std::function<void(bool)> ack) { pending = std::move(ack); });
    QObject::connect(&runner, &RuledDevConsoleRunner::busyChanged, widget.get(), &RuledDevConsoleWidget::setBusy);
    QObject::connect(&runner, &RuledDevConsoleRunner::statusChanged, widget.get(), &RuledDevConsoleWidget::setStatus);
    QObject::connect(widget.get(), &RuledDevConsoleWidget::commandSubmitted, &runner,
                     [&runner](const QString &line) { runner.submit(line, 0, {0, 1}); });
    submit(QStringLiteral("help"), false);
    EXPECT_TRUE(input->isEnabled());
    EXPECT_FALSE(status->isHidden());
    submit(QStringLiteral("invalid"), false);
    EXPECT_TRUE(input->isEnabled());
    EXPECT_TRUE(status->text().contains(QStringLiteral("Unknown command")));
    submit(QStringLiteral("mana R"), false);
    EXPECT_FALSE(input->isEnabled());
    ASSERT_TRUE(pending);
    pending(false);
    EXPECT_TRUE(input->isEnabled());
    EXPECT_TRUE(status->text().contains(QStringLiteral("rejected")));
}

class RuledDevConsoleRunnerTest : public ::testing::Test
{
protected:
    void SetUp() override
    {
        runner = std::make_unique<RuledDevConsoleRunner>(
            [this](const ruled::v1::RuledCommand &command, std::function<void(bool)> ack) {
                sent.append(command);
                callbacks.append(std::move(ack));
            });
        busySpy = std::make_unique<QSignalSpy>(runner.get(), &RuledDevConsoleRunner::busyChanged);
        statusSpy = std::make_unique<QSignalSpy>(runner.get(), &RuledDevConsoleRunner::statusChanged);
    }
    void run(const QString &text = QStringLiteral("mana R\nput 2 bf Grizzly Bears ready"))
    {
        runner->runPreset(QStringLiteral("test"), text, 10, {10, 20});
    }
    void ack(int index, bool accepted)
    {
        const auto callback = callbacks.at(index);
        callback(accepted);
        QApplication::processEvents();
    }
    bool busy() const
    {
        return busySpy->last().at(0).toBool();
    }
    QString status() const
    {
        return statusSpy->last().at(0).toString();
    }

    std::unique_ptr<RuledDevConsoleRunner> runner;
    std::unique_ptr<QSignalSpy> busySpy;
    std::unique_ptr<QSignalSpy> statusSpy;
    QVector<ruled::v1::RuledCommand> sent;
    QVector<std::function<void(bool)>> callbacks;
};

TEST_F(RuledDevConsoleRunnerTest, WaitsForEachEngineAckAndReportsCompletionOnlyAfterLast)
{
    run();
    ASSERT_EQ(sent.size(), 1);
    EXPECT_TRUE(busy());
    EXPECT_TRUE(sent[0].dev_command().has_add_mana());
    ack(0, true);
    ASSERT_EQ(sent.size(), 2);
    EXPECT_TRUE(busy());
    EXPECT_EQ(sent[1].dev_command().target_player_id(), 20);
    ack(1, true);
    EXPECT_FALSE(busy());
    EXPECT_TRUE(status().contains(QStringLiteral("all 2 setup commands accepted")));
}

TEST_F(RuledDevConsoleRunnerTest, RejectionStopsWithoutSendingTheRest)
{
    run();
    ack(0, false);
    EXPECT_EQ(sent.size(), 1);
    EXPECT_FALSE(busy());
    EXPECT_TRUE(status().contains(QStringLiteral("Stopped at command 1")));
    EXPECT_TRUE(statusSpy->last().at(1).toBool());
}

TEST_F(RuledDevConsoleRunnerTest, PreflightsAllLinesAndSeatsBeforeAnySend)
{
    run(QStringLiteral("mana R\nnot-a-command"));
    EXPECT_TRUE(sent.isEmpty());
    EXPECT_TRUE(status().contains(QStringLiteral("line 2")));
    run(QStringLiteral("mana R\nput 3 bf Hill Giant"));
    EXPECT_TRUE(sent.isEmpty());
    run(QStringLiteral("mana R\nhelp"));
    EXPECT_TRUE(sent.isEmpty());
    run(QStringLiteral("# comment only\n\n"));
    EXPECT_TRUE(sent.isEmpty());
    EXPECT_FALSE(busy());
}

TEST_F(RuledDevConsoleRunnerTest, CommentsBomCrLfAndNonContiguousSeatIdsUseExistingParser)
{
    const QString text = QChar(0xfeff) + QStringLiteral("# prepared state\r\n\r\n/put 4 bf Hill Giant ready\r\n");
    runner->runPreset(QStringLiteral("four seats"), text, 20, {10, 20, 40, 90});
    ASSERT_EQ(sent.size(), 1);
    EXPECT_EQ(sent[0].dev_command().target_player_id(), 90);
    EXPECT_TRUE(sent[0].dev_command().put_card_in_zone().ready());
    ack(0, true);
    EXPECT_FALSE(busy());
}

TEST_F(RuledDevConsoleRunnerTest, BoundsPresetSizeAndCommandCount)
{
    run(QString(65537, QLatin1Char('x')));
    EXPECT_TRUE(sent.isEmpty());
    run(QStringLiteral("mana R\n").repeated(129));
    EXPECT_TRUE(sent.isEmpty());
    EXPECT_TRUE(status().contains(QStringLiteral("128")));
}

TEST_F(RuledDevConsoleRunnerTest, CancelWaitsForInFlightAckAndBlocksRerunsUntilItArrives)
{
    run();
    runner->cancel();
    EXPECT_TRUE(busy());
    run();
    runner->submit(QStringLiteral("mana W"), 10, {10, 20});
    EXPECT_EQ(sent.size(), 1);
    ack(0, true);
    EXPECT_EQ(sent.size(), 1);
    EXPECT_FALSE(busy());
    EXPECT_TRUE(status().contains(QStringLiteral("Cancelled")));
    run();
    EXPECT_EQ(sent.size(), 2);
}

TEST_F(RuledDevConsoleRunnerTest, CancelBetweenAckAndDeferredSendPreventsTheNextCommand)
{
    run();
    callbacks.at(0)(true);
    runner->cancel();
    QApplication::processEvents();
    EXPECT_EQ(sent.size(), 1);
    EXPECT_FALSE(busy());
}

TEST_F(RuledDevConsoleRunnerTest, SessionResetIgnoresOldAckAndDoesNotResumeOldQueue)
{
    run();
    runner->reset(QStringLiteral("session changed"));
    EXPECT_FALSE(busy());
    run(QStringLiteral("mana U"));
    EXPECT_EQ(sent.size(), 2);
    ack(0, true);
    EXPECT_TRUE(busy());
    EXPECT_EQ(sent.size(), 2);
    ack(1, true);
    EXPECT_FALSE(busy());
}

TEST_F(RuledDevConsoleRunnerTest, DuplicateRepliesCannotAdvanceAnotherSubmission)
{
    run();
    ack(0, true);
    ASSERT_EQ(sent.size(), 2);
    ack(0, false);
    EXPECT_TRUE(busy());
    ack(1, true);
    EXPECT_FALSE(busy());
}

TEST_F(RuledDevConsoleRunnerTest, ResetBetweenAckAndDeferredSendPreventsTheNextCommand)
{
    run();
    callbacks.at(0)(true);
    runner->reset(QStringLiteral("disconnected"));
    QApplication::processEvents();
    EXPECT_EQ(sent.size(), 1);
    EXPECT_FALSE(busy());
}

TEST_F(RuledDevConsoleRunnerTest, ClosingTheConsoleMakesPendingRepliesHarmless)
{
    run();
    runner.reset();
    ack(0, true);
    EXPECT_EQ(sent.size(), 1);
}

TEST_F(RuledDevConsoleRunnerTest, ManualHelpAndParseFailureUnlockTheInputWithoutSending)
{
    runner->submit(QStringLiteral("help"), 10, {10, 20});
    EXPECT_FALSE(busy());
    EXPECT_FALSE(statusSpy->last().at(1).toBool());
    runner->submit(QStringLiteral("invalid"), 10, {10, 20});
    EXPECT_FALSE(busy());
    EXPECT_TRUE(statusSpy->last().at(1).toBool());
    EXPECT_TRUE(sent.isEmpty());
}

} // namespace

int main(int argc, char **argv)
{
    QApplication app(argc, argv);
    ::testing::InitGoogleTest(&argc, argv);
    return RUN_ALL_TESTS();
}
