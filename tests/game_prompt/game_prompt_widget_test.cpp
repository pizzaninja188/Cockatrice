#include "game_prompt_widget.h"

#include <QApplication>
#include <QPushButton>
#include <QSignalSpy>
#include <gtest/gtest.h>

class GamePromptWidgetTest : public ::testing::Test
{
protected:
    void SetUp() override { widget = std::make_unique<GamePromptWidget>(); }
    std::unique_ptr<GamePromptWidget> widget;

    QPushButton *btn(const char *objectName)
    {
        return widget->findChild<QPushButton *>(objectName);
    }
};

// --- Pass priority ---

TEST_F(GamePromptWidgetTest, PassPriorityButtonEnabledWhenSet)
{
    widget->setPassPriorityEnabled(true);
    EXPECT_TRUE(btn("passPriorityButton")->isEnabled());
}

TEST_F(GamePromptWidgetTest, PassPriorityButtonDisabledWhenNotEnabled)
{
    widget->setPassPriorityEnabled(false);
    EXPECT_FALSE(btn("passPriorityButton")->isEnabled());
}

TEST_F(GamePromptWidgetTest, PassPrioritySignalEmitted)
{
    widget->setLocalPlayerHasPriority(true);
    widget->setPassPriorityEnabled(true);
    QSignalSpy spy(widget.get(), &GamePromptWidget::passPriorityRequested);
    btn("passPriorityButton")->click();
    EXPECT_EQ(spy.count(), 1);
}

// --- Combat mode ---

TEST_F(GamePromptWidgetTest, DeclareAttackersShowsConfirmButton)
{
    widget->setLocalPlayerHasPriority(true);
    widget->setCombatMode(GamePromptWidget::CombatMode::DeclareAttackers, true);
    EXPECT_FALSE(btn("confirmAttackersButton")->isHidden());
    EXPECT_TRUE(btn("passPriorityButton")->isHidden());
}

TEST_F(GamePromptWidgetTest, DeclareAttackersWithoutLocalButtonsHidesCombatButton)
{
    widget->setLocalPlayerHasPriority(true);
    widget->setCombatMode(GamePromptWidget::CombatMode::DeclareAttackers, false);
    EXPECT_TRUE(btn("confirmAttackersButton")->isHidden());
    EXPECT_FALSE(btn("passPriorityButton")->isHidden());
}

TEST_F(GamePromptWidgetTest, LocalPlayerMustDeclareCombatTrueWhenAttacking)
{
    widget->setCombatMode(GamePromptWidget::CombatMode::DeclareAttackers, true);
    EXPECT_TRUE(widget->localPlayerMustDeclareCombat());
}

TEST_F(GamePromptWidgetTest, LocalPlayerMustDeclareCombatFalseWithoutButtons)
{
    widget->setCombatMode(GamePromptWidget::CombatMode::DeclareAttackers, false);
    EXPECT_FALSE(widget->localPlayerMustDeclareCombat());
}

TEST_F(GamePromptWidgetTest, LocalPlayerMustDeclareCombatFalseInNoneMode)
{
    widget->setCombatMode(GamePromptWidget::CombatMode::None, true);
    EXPECT_FALSE(widget->localPlayerMustDeclareCombat());
}

TEST_F(GamePromptWidgetTest, ConfirmAttackersSignalEmitted)
{
    widget->setLocalPlayerHasPriority(true);
    widget->setCombatMode(GamePromptWidget::CombatMode::DeclareAttackers, true);
    QSignalSpy spy(widget.get(), &GamePromptWidget::confirmAttackersRequested);
    btn("confirmAttackersButton")->click();
    EXPECT_EQ(spy.count(), 1);
}

TEST_F(GamePromptWidgetTest, DeclareBlockersShowsConfirmAndResetButtons)
{
    widget->setLocalPlayerHasPriority(true);
    widget->setCombatMode(GamePromptWidget::CombatMode::DeclareBlockers, true);
    EXPECT_FALSE(btn("confirmBlockersButton")->isHidden());
    EXPECT_FALSE(btn("resetBlockersButton")->isHidden());
}

TEST_F(GamePromptWidgetTest, ConfirmBlockersSignalEmitted)
{
    widget->setLocalPlayerHasPriority(true);
    widget->setCombatMode(GamePromptWidget::CombatMode::DeclareBlockers, true);
    QSignalSpy spy(widget.get(), &GamePromptWidget::confirmBlockersRequested);
    btn("confirmBlockersButton")->click();
    EXPECT_EQ(spy.count(), 1);
}

TEST_F(GamePromptWidgetTest, ResetBlockersSignalEmitted)
{
    widget->setLocalPlayerHasPriority(true);
    widget->setCombatMode(GamePromptWidget::CombatMode::DeclareBlockers, true);
    QSignalSpy spy(widget.get(), &GamePromptWidget::resetBlockersRequested);
    btn("resetBlockersButton")->click();
    EXPECT_EQ(spy.count(), 1);
}

// --- Targeting mode ---

TEST_F(GamePromptWidgetTest, TargetingModeShowsCancelButton)
{
    widget->setTargetingMode(true, "Lightning Bolt");
    EXPECT_FALSE(btn("cancelTargetingButton")->isHidden());
    EXPECT_TRUE(btn("passPriorityButton")->isHidden());
}

TEST_F(GamePromptWidgetTest, TargetingModeCancelSignalEmitted)
{
    widget->setTargetingMode(true, "Lightning Bolt");
    QSignalSpy spy(widget.get(), &GamePromptWidget::cancelTargetingRequested);
    btn("cancelTargetingButton")->click();
    EXPECT_EQ(spy.count(), 1);
}

TEST_F(GamePromptWidgetTest, TargetingModeClearedRestoresPassPriority)
{
    widget->setLocalPlayerHasPriority(true);
    widget->setTargetingMode(true, "Bolt");
    widget->setTargetingMode(false);
    EXPECT_TRUE(btn("cancelTargetingButton")->isHidden());
    EXPECT_FALSE(btn("passPriorityButton")->isHidden());
}

// --- Player names ---

TEST_F(GamePromptWidgetTest, ActivePlayerNameRoundTrips)
{
    widget->setActivePlayerName("Alice");
    EXPECT_EQ(widget->getActivePlayerName(), "Alice");
}

// --- First-strike substep button labels ---

TEST_F(GamePromptWidgetTest, FirstStrikeStepPendingChangesButtonText)
{
    widget->setCombatMode(GamePromptWidget::CombatMode::DeclareBlockers, false);
    widget->setLocalPlayerHasPriority(true);
    widget->setFirstStrikeStepPending(true);
    EXPECT_EQ(btn("passPriorityButton")->text(), "First Strike Damage");
}

TEST_F(GamePromptWidgetTest, FirstStrikeDamageStepActiveChangesButtonText)
{
    widget->setLocalPlayerHasPriority(true);
    widget->setFirstStrikeDamageStepActive(true);
    EXPECT_EQ(btn("passPriorityButton")->text(), "Combat Damage");
}

TEST_F(GamePromptWidgetTest, StackItemsChangesButtonTextToNoResponse)
{
    widget->setLocalPlayerHasPriority(true);
    widget->setRuledStackHasItems(true);
    EXPECT_EQ(btn("passPriorityButton")->text(), "No Response");
}

// --- Undo land tap ---

TEST_F(GamePromptWidgetTest, UndoLandTapVisibleWhenAvailableAndHasPriority)
{
    widget->setLocalPlayerHasPriority(true);
    widget->setLandTapUndoAvailable(true);
    EXPECT_FALSE(btn("undoLandTapButton")->isHidden());
}

TEST_F(GamePromptWidgetTest, UndoLandTapHiddenWhenNotAvailable)
{
    widget->setLocalPlayerHasPriority(true);
    widget->setLandTapUndoAvailable(false);
    EXPECT_FALSE(btn("undoLandTapButton")->isVisible());
}

TEST_F(GamePromptWidgetTest, UndoLandTapSignalEmitted)
{
    widget->setLocalPlayerHasPriority(true);
    widget->setLandTapUndoAvailable(true);
    QSignalSpy spy(widget.get(), &GamePromptWidget::undoLandTapRequested);
    btn("undoLandTapButton")->click();
    EXPECT_EQ(spy.count(), 1);
}

int main(int argc, char **argv)
{
    QApplication app(argc, argv);
    ::testing::InitGoogleTest(&argc, argv);
    return RUN_ALL_TESTS();
}
