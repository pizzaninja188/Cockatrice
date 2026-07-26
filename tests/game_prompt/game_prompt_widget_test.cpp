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

// CR 508.1d: OK is disabled (but still shown) while a required attacker is unstaged.
TEST_F(GamePromptWidgetTest, DeclareAttackersDisablesConfirmWhenRequirementUnmet)
{
    widget->setLocalPlayerHasPriority(true);
    widget->setCombatMode(GamePromptWidget::CombatMode::DeclareAttackers, true, /*declarationSatisfied=*/false);
    EXPECT_FALSE(btn("confirmAttackersButton")->isHidden());
    EXPECT_FALSE(btn("confirmAttackersButton")->isEnabled());
    // Staging the required attacker satisfies the requirement and re-enables OK.
    widget->setCombatMode(GamePromptWidget::CombatMode::DeclareAttackers, true, /*declarationSatisfied=*/true);
    EXPECT_TRUE(btn("confirmAttackersButton")->isEnabled());
}

// CR 509.1c: OK is disabled (but still shown) while a required blocker is unstaged.
TEST_F(GamePromptWidgetTest, DeclareBlockersDisablesConfirmWhenRequirementUnmet)
{
    widget->setLocalPlayerHasPriority(true);
    widget->setCombatMode(GamePromptWidget::CombatMode::DeclareBlockers, true, /*declarationSatisfied=*/false);
    EXPECT_FALSE(btn("confirmBlockersButton")->isHidden());
    EXPECT_FALSE(btn("confirmBlockersButton")->isEnabled());
    widget->setCombatMode(GamePromptWidget::CombatMode::DeclareBlockers, true, /*declarationSatisfied=*/true);
    EXPECT_TRUE(btn("confirmBlockersButton")->isEnabled());
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

// --- Prompt modes ---

using PromptMode = GamePromptWidget::PromptMode;

TEST_F(GamePromptWidgetTest, DefaultModeIsNormal)
{
    EXPECT_EQ(widget->effectiveMode(), PromptMode::Normal);
}

TEST_F(GamePromptWidgetTest, TargetingSourcesOrIntoTheTargetingMode)
{
    widget->setSpellCastPending(true);
    EXPECT_EQ(widget->effectiveMode(), PromptMode::Targeting);
    // Two sources up: dropping one must NOT drop the mode — they are independent inputs.
    widget->setTargetingMode(true, "Bolt");
    widget->setSpellCastPending(false);
    EXPECT_EQ(widget->effectiveMode(), PromptMode::Targeting);
    widget->setTargetingMode(false);
    EXPECT_EQ(widget->effectiveMode(), PromptMode::Normal);
}

TEST_F(GamePromptWidgetTest, TakeOverModesOutrankTargeting)
{
    widget->setSpellCastPending(true);
    widget->setRuledPromptState({PromptMode::CleanupDiscard, 2, 0, {}, {}});
    EXPECT_EQ(widget->effectiveMode(), PromptMode::CleanupDiscard);
    EXPECT_TRUE(btn("passPriorityButton")->isHidden());
    EXPECT_TRUE(btn("cancelTargetingButton")->isHidden());
}

TEST_F(GamePromptWidgetTest, TargetingOutranksAParkedClickChoice)
{
    widget->setRuledPromptState({PromptMode::ClickChoice, 0, 0, "Choose a target for: Draw a card.", {}});
    EXPECT_EQ(widget->effectiveMode(), PromptMode::ClickChoice);
    widget->setSpellCastPending(true);
    EXPECT_EQ(widget->effectiveMode(), PromptMode::Targeting);
    widget->setSpellCastPending(false);
    EXPECT_EQ(widget->effectiveMode(), PromptMode::ClickChoice);
}

TEST_F(GamePromptWidgetTest, ResolutionPickShowsConfirmEnabledOnlyWhenSatisfied)
{
    widget->setLocalPlayerHasPriority(true);
    widget->setRuledPromptState({PromptMode::ResolutionPick, 2, 1, "Put two cards back.", {}});
    EXPECT_FALSE(btn("resolutionHandPickConfirmButton")->isHidden());
    EXPECT_FALSE(btn("resolutionHandPickConfirmButton")->isEnabled());
    EXPECT_TRUE(btn("passPriorityButton")->isHidden());

    widget->setRuledPromptState({PromptMode::ResolutionPick, 2, 2, "Put two cards back.", {}});
    EXPECT_TRUE(btn("resolutionHandPickConfirmButton")->isEnabled());

    widget->setRuledPromptState({});
    EXPECT_TRUE(btn("resolutionHandPickConfirmButton")->isHidden());
    EXPECT_FALSE(btn("passPriorityButton")->isHidden());
}

TEST_F(GamePromptWidgetTest, OpeningBottomDoneAppearsOnlyOnAnExactSelection)
{
    widget->setLocalPlayerHasPriority(true);
    widget->setRuledPromptState({PromptMode::OpeningBottom, 2, 0, {}, {}});
    EXPECT_TRUE(btn("passPriorityButton")->isHidden());
    EXPECT_TRUE(btn("openingBottomDoneButton")->isHidden());
    EXPECT_TRUE(btn("openingBottomCancelButton")->isHidden());

    widget->setRuledPromptState({PromptMode::OpeningBottom, 2, 1, {}, {}});
    EXPECT_TRUE(btn("openingBottomDoneButton")->isHidden());
    EXPECT_FALSE(btn("openingBottomCancelButton")->isHidden());

    widget->setRuledPromptState({PromptMode::OpeningBottom, 2, 2, {}, {}});
    EXPECT_FALSE(btn("openingBottomDoneButton")->isHidden());
}

TEST_F(GamePromptWidgetTest, OpeningChooseFirstEmitsTheSeatIdItWasGiven)
{
    widget->setRuledPromptState({PromptMode::OpeningChooseFirst, 0, 0, {}, QVector<int>({3, 7})});
    ASSERT_FALSE(btn("openingPickSeatButton2")->isHidden());
    QSignalSpy spy(widget.get(), &GamePromptWidget::ruledOpeningPickSeatRequested);
    btn("openingPickSeatButton2")->click();
    ASSERT_EQ(spy.count(), 1);
    EXPECT_EQ(spy.at(0).at(0).toInt(), 7);

    // Re-entering with different seats rewires the buttons instead of stacking connections.
    widget->setRuledPromptState({PromptMode::OpeningChooseFirst, 0, 0, {}, QVector<int>({9, 4})});
    btn("openingPickSeatButton2")->click();
    ASSERT_EQ(spy.count(), 2);
    EXPECT_EQ(spy.at(1).at(0).toInt(), 4);
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
