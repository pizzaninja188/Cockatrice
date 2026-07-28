// Offscreen widget tests for the ruled dev console.
//
// The widget's whole contract is: take a line, remember it, hand it on, and show whatever status
// the caller pushes back. It deliberately does no parsing and sends nothing — that lives in
// RuledDevCommandParser and TabGame — so everything worth testing here is input handling.
//
// Follows tests/game_prompt/: offscreen, children found by objectName, visibility asserted with
// isHidden() because nothing is ever shown.

#include "game/ruled/ruled_dev_console.h"

#include <QApplication>
#include <QLabel>
#include <QLineEdit>
#include <QSignalSpy>
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
    void submit(const QString &line)
    {
        input->setText(line);
        QTest::keyClick(input, Qt::Key_Return);
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

} // namespace

int main(int argc, char **argv)
{
    QApplication app(argc, argv);
    ::testing::InitGoogleTest(&argc, argv);
    return RUN_ALL_TESTS();
}
