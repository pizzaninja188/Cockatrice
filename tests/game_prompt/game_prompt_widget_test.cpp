#include "game_prompt_widget.h"

#include <QApplication>
#include <QCheckBox>
#include <QLabel>
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

    QLabel *label(const char *objectName)
    {
        return widget->findChild<QLabel *>(objectName);
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
    widget->setTargetingMode(true, "Counter target spell");
    EXPECT_FALSE(btn("cancelTargetingButton")->isHidden());
    EXPECT_TRUE(btn("passPriorityButton")->isHidden());
    EXPECT_EQ(label("promptLabel")->text(), QStringLiteral("Counter target spell"));
}

TEST_F(GamePromptWidgetTest, TargetingModeUpdatesForTheNextSelectedMode)
{
    widget->setTargetingMode(true, "Counter target spell");
    widget->setTargetingMode(true, "Return target permanent to its owner's hand");
    EXPECT_EQ(label("promptLabel")->text(), QStringLiteral("Return target permanent to its owner's hand"));
}

TEST_F(GamePromptWidgetTest, ActivatedAbilityTargetingUsesAbilityText)
{
    widget->setActivatedAbilityTargetPending(true, "{T}: Tap target permanent");
    EXPECT_EQ(label("promptLabel")->text(),
              QString::fromUtf8("Choose a target for “{T}: Tap target permanent”, or press Cancel."));
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

TEST_F(GamePromptWidgetTest, OptionalMultiTargetShowsConfirmAtZeroSelections)
{
    widget->setTargetingMode(true, "Ghostform");
    widget->setMultiTargetSelectionCount(0, 0, 2);
    EXPECT_FALSE(btn("confirmTargetsButton")->isHidden());
    EXPECT_TRUE(btn("confirmTargetsButton")->isEnabled());
}

TEST_F(GamePromptWidgetTest, RequiredVariableTargetGroupDisablesConfirmBelowMinimum)
{
    widget->setTargetingMode(true, "Choose creatures");
    widget->setMultiTargetSelectionCount(0, 1, 2);
    EXPECT_FALSE(btn("confirmTargetsButton")->isHidden());
    EXPECT_FALSE(btn("confirmTargetsButton")->isEnabled());
    widget->setMultiTargetSelectionCount(1, 1, 2);
    EXPECT_TRUE(btn("confirmTargetsButton")->isEnabled());
}

TEST_F(GamePromptWidgetTest, CommandPendingHidesActionsWithoutReplacingCurrentPrompt)
{
    widget->setLocalPlayerHasPriority(true);
    widget->setPromptText("Choose attackers.");
    widget->setRuledPromptState({PromptMode::CommandPending});
    EXPECT_EQ(widget->effectiveMode(), PromptMode::CommandPending);
    EXPECT_EQ(label("promptLabel")->text(), "Choose attackers.");
    EXPECT_TRUE(btn("passPriorityButton")->isHidden());
    EXPECT_TRUE(btn("confirmAttackersButton")->isHidden());
}

TEST_F(GamePromptWidgetTest, UpdatingGameShowsDelayedStatusAndNoActions)
{
    widget->setLocalPlayerHasPriority(true);
    widget->setRuledPromptState({PromptMode::UpdatingGame});
    EXPECT_EQ(widget->effectiveMode(), PromptMode::UpdatingGame);
    EXPECT_EQ(label("promptLabel")->text(), QStringLiteral("Updating game") + QChar(0x2026));
    EXPECT_TRUE(btn("passPriorityButton")->isHidden());
    EXPECT_TRUE(btn("cancelTargetingButton")->isHidden());
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

TEST_F(GamePromptWidgetTest, OptionalClickChoiceUsesTheGenericDeclineControl)
{
    QSignalSpy spy(widget.get(), &GamePromptWidget::declineClickChoiceRequested);
    widget->setRuledPromptState(
        {PromptMode::ClickChoice, 0, 0, "Choose a creature for Clone to copy, or Decline to enter as Clone.", {}, true});

    auto *decline = btn("declineClickChoiceButton");
    ASSERT_NE(decline, nullptr);
    EXPECT_FALSE(decline->isHidden());
    decline->click();
    EXPECT_EQ(spy.count(), 1);
}

TEST_F(GamePromptWidgetTest, VariableClickChoiceUsesConfirmTargetsIncludingZeroSelections)
{
    QSignalSpy spy(widget.get(), &GamePromptWidget::confirmTargetsRequested);
    GamePromptWidget::RuledPromptState state;
    state.mode = PromptMode::ClickChoice;
    state.required = 0;
    state.selected = 0;
    state.text = "Choose up to two target cards from a single graveyard.";
    state.max = 2;
    widget->setRuledPromptState(state);

    auto *confirm = btn("confirmTargetsButton");
    ASSERT_NE(confirm, nullptr);
    EXPECT_FALSE(confirm->isHidden());
    EXPECT_TRUE(confirm->isEnabled());
    confirm->click();
    EXPECT_EQ(spy.count(), 1);
    EXPECT_TRUE(btn("declineClickChoiceButton")->isHidden());
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

TEST_F(GamePromptWidgetTest, ResolutionPaymentUsesNormalCostControlsAndSuppressesPriority)
{
    QSignalSpy declineSpy(widget.get(), &GamePromptWidget::ruledResolutionPaymentDeclineRequested);
    widget->setLocalPlayerHasPriority(true);
    widget->setLandTapUndoAvailable(true);
    widget->setRuledPromptState(
        {PromptMode::ResolutionPayment, 0, 0, "Pay {4} or decline.", {}, false, 4, false});

    EXPECT_EQ(widget->effectiveMode(), PromptMode::ResolutionPayment);
    EXPECT_FALSE(btn("resolutionPaymentDeclineButton")->isHidden());
    EXPECT_TRUE(btn("passPriorityButton")->isHidden());
    EXPECT_FALSE(btn("undoLandTapButton")->isHidden());

    widget->setLandTapUndoAvailable(false);
    EXPECT_TRUE(btn("undoLandTapButton")->isHidden());
    btn("resolutionPaymentDeclineButton")->click();
    EXPECT_EQ(declineSpy.count(), 1);
}

TEST_F(GamePromptWidgetTest, GraveyardCostSelectionRequiresExactCountAndCanCancelLocally)
{
    QSignalSpy confirmSpy(widget.get(), &GamePromptWidget::ruledCostSelectionConfirmRequested);
    QSignalSpy cancelSpy(widget.get(), &GamePromptWidget::ruledCostSelectionCancelRequested);
    widget->setLocalPlayerHasPriority(true);
    GamePromptWidget::RuledPromptState state;
    state.mode = PromptMode::CostSelection;
    state.required = 2;
    state.selected = 1;
    state.text = "Choose two graveyard cards.";
    state.canDecline = true;
    widget->setRuledPromptState(state);

    EXPECT_EQ(widget->effectiveMode(), PromptMode::CostSelection);
    EXPECT_FALSE(btn("resolutionHandPickConfirmButton")->isHidden());
    EXPECT_FALSE(btn("resolutionHandPickConfirmButton")->isEnabled());
    EXPECT_FALSE(btn("openingBottomCancelButton")->isHidden());
    EXPECT_TRUE(btn("passPriorityButton")->isHidden());

    state.selected = 2;
    widget->setRuledPromptState(state);
    EXPECT_TRUE(btn("resolutionHandPickConfirmButton")->isEnabled());
    btn("resolutionHandPickConfirmButton")->click();
    btn("openingBottomCancelButton")->click();
    EXPECT_EQ(confirmSpy.count(), 1);
    EXPECT_EQ(cancelSpy.count(), 1);
}

TEST_F(GamePromptWidgetTest, MandatoryResolutionCostSelectionDoesNotOfferCancel)
{
    GamePromptWidget::RuledPromptState state;
    state.mode = PromptMode::CostSelection;
    state.required = 1;
    state.text = "Choose one untapped permanent.";
    state.canDecline = false;
    widget->setRuledPromptState(state);
    EXPECT_TRUE(btn("openingBottomCancelButton")->isHidden());
    EXPECT_FALSE(btn("resolutionHandPickConfirmButton")->isEnabled());

    state.selected = 1;
    widget->setRuledPromptState(state);
    EXPECT_TRUE(btn("resolutionHandPickConfirmButton")->isEnabled());
    state.canDecline = true;
    widget->setRuledPromptState(state);
    EXPECT_FALSE(btn("openingBottomCancelButton")->isHidden());
}

TEST_F(GamePromptWidgetTest, ChoiceOptionsRenderAsOrdinaryLabeledButtons)
{
    QSignalSpy optionSpy(widget.get(), &GamePromptWidget::ruledChoiceOptionRequested);
    QSignalSpy declineSpy(widget.get(), &GamePromptWidget::declineClickChoiceRequested);
    GamePromptWidget::RuledPromptState state;
    state.mode = PromptMode::ChoiceOptions;
    state.text = "Choose one.";
    state.canDecline = true;
    state.choiceOptions = {{0, "Gain 4 life", true}, {1, "Put a +1/+1 counter on it", false}};
    widget->setRuledPromptState(state);

    auto *life = btn("ruledChoiceOptionButton_0");
    auto *counter = btn("ruledChoiceOptionButton_1");
    ASSERT_NE(life, nullptr);
    ASSERT_NE(counter, nullptr);
    EXPECT_FALSE(life->isHidden());
    EXPECT_TRUE(life->isEnabled());
    EXPECT_FALSE(counter->isEnabled());
    EXPECT_TRUE(btn("passPriorityButton")->isHidden());
    EXPECT_FALSE(btn("declineClickChoiceButton")->isHidden());

    life->click();
    ASSERT_EQ(optionSpy.count(), 1);
    EXPECT_EQ(optionSpy.takeFirst().at(0).toInt(), 0);
    btn("declineClickChoiceButton")->click();
    EXPECT_EQ(declineSpy.count(), 1);
}

TEST_F(GamePromptWidgetTest, CastCostOptionsUseTheirOwnButtonRouteAndSuppressPriorityControls)
{
    QSignalSpy optionSpy(widget.get(), &GamePromptWidget::ruledCastCostOptionRequested);
    QSignalSpy resolutionSpy(widget.get(), &GamePromptWidget::ruledChoiceOptionRequested);
    QSignalSpy cancelSpy(widget.get(), &GamePromptWidget::cancelTargetingRequested);
    QSignalSpy backSpy(widget.get(), &GamePromptWidget::ruledCastCostBackRequested);
    GamePromptWidget::RuledPromptState state;
    state.mode = PromptMode::CastCostOptions;
    state.text = "Behold a Dragon or pay {1}.";
    state.choiceOptions = {{-1, "Cast normally", true}, {0, "Behold a Dragon", true}, {1, "Pay {1}", true}};
    widget->setRuledPromptState(state);

    EXPECT_EQ(widget->effectiveMode(), PromptMode::CastCostOptions);
    EXPECT_TRUE(btn("passPriorityButton")->isHidden());
    EXPECT_FALSE(btn("cancelTargetingButton")->isHidden());
    auto *behold = btn("ruledChoiceOptionButton_0");
    ASSERT_NE(behold, nullptr);
    behold->click();
    ASSERT_EQ(optionSpy.count(), 1);
    EXPECT_EQ(optionSpy.takeFirst().at(0).toInt(), 0);
    EXPECT_EQ(resolutionSpy.count(), 0);
    btn("cancelTargetingButton")->click();
    EXPECT_EQ(cancelSpy.count(), 1);

    state.mode = PromptMode::CastCostObject;
    state.choiceOptions.clear();
    widget->setRuledPromptState(state);
    EXPECT_EQ(widget->effectiveMode(), PromptMode::CastCostObject);
    EXPECT_FALSE(btn("cancelTargetingButton")->isHidden());
    EXPECT_FALSE(btn("declineClickChoiceButton")->isHidden());
    EXPECT_EQ(btn("declineClickChoiceButton")->text(), "Back");
    btn("declineClickChoiceButton")->click();
    EXPECT_EQ(backSpy.count(), 1);
    EXPECT_EQ(resolutionSpy.count(), 0);
}

TEST_F(GamePromptWidgetTest, CastCostOptionControllerCanReplaceStaleButtonsWithObjectPicker)
{
    GamePromptWidget::RuledPromptState options;
    options.mode = PromptMode::CastCostOptions;
    options.text = "Behold a Dragon or pay {1}.";
    options.choiceOptions = {{0, "Behold a Dragon", true}, {1, "Pay {1}", true}};
    widget->setRuledPromptState(options);

    QObject::connect(widget.get(), &GamePromptWidget::ruledCastCostOptionRequested, widget.get(),
                     [this](int optionIndex) {
        ASSERT_EQ(optionIndex, 0);
        GamePromptWidget::RuledPromptState objectPicker;
        objectPicker.mode = PromptMode::CastCostObject;
        objectPicker.text = "Choose a Dragon you control or a Dragon card in your hand.";
        widget->setRuledPromptState(objectPicker);
                     });

    btn("ruledChoiceOptionButton_0")->click();
    EXPECT_EQ(widget->effectiveMode(), PromptMode::CastCostObject);
    EXPECT_EQ(btn("ruledChoiceOptionButton_0"), nullptr);
    EXPECT_EQ(btn("ruledChoiceOptionButton_1"), nullptr);
    EXPECT_EQ(label("promptLabel")->text(), "Choose a Dragon you control or a Dragon card in your hand.");
    EXPECT_FALSE(btn("cancelTargetingButton")->isHidden());
}

TEST_F(GamePromptWidgetTest, HarmonizeShowsFullCostAndCreatureBranchesWithBackAndCancel)
{
    QSignalSpy optionSpy(widget.get(), &GamePromptWidget::ruledCastCostOptionRequested);
    QSignalSpy cancelSpy(widget.get(), &GamePromptWidget::cancelTargetingRequested);
    QSignalSpy backSpy(widget.get(), &GamePromptWidget::ruledCastCostBackRequested);
    GamePromptWidget::RuledPromptState state;
    state.mode = PromptMode::CastCostOptions;
    state.text = "Harmonize: you may tap an untapped creature you control.";
    state.choiceOptions = {{-1, "Pay full Harmonize cost", true}, {0, "Tap a creature", true}};
    widget->setRuledPromptState(state);

    ASSERT_NE(btn("ruledChoiceOptionButton_-1"), nullptr);
    ASSERT_NE(btn("ruledChoiceOptionButton_0"), nullptr);
    EXPECT_EQ(btn("ruledChoiceOptionButton_-1")->text(), "Pay full Harmonize cost");
    EXPECT_EQ(btn("ruledChoiceOptionButton_0")->text(), "Tap a creature");
    btn("ruledChoiceOptionButton_0")->click();
    ASSERT_EQ(optionSpy.count(), 1);
    EXPECT_EQ(optionSpy.takeFirst().at(0).toInt(), 0);

    state.mode = PromptMode::CastCostObject;
    state.choiceOptions.clear();
    widget->setRuledPromptState(state);
    EXPECT_EQ(btn("declineClickChoiceButton")->text(), "Back");
    btn("declineClickChoiceButton")->click();
    EXPECT_EQ(backSpy.count(), 1);
    btn("cancelTargetingButton")->click();
    EXPECT_EQ(cancelSpy.count(), 1);
}

TEST_F(GamePromptWidgetTest, ZoneSelectionUsesCheckboxesAndConfirmsTheMatchingAuthoredBranch)
{
    QSignalSpy optionSpy(widget.get(), &GamePromptWidget::ruledChoiceOptionRequested);
    GamePromptWidget::RuledPromptState state;
    state.mode = PromptMode::ZoneSelection;
    state.text = "Choose which zones to search.";
    state.choiceOptions = {
        {0, "Hand", true, {1}},
        {1, "Graveyard", true, {2}},
        {2, "Library", true, {3}},
        {3, "Hand + Graveyard", true, {1, 2}},
        {4, "Hand + Library", true, {1, 3}},
        {5, "Graveyard + Library", true, {2, 3}},
        {6, "Hand + Graveyard + Library", true, {1, 2, 3}},
    };
    widget->setRuledPromptState(state);

    auto *hand = widget->findChild<QCheckBox *>("zoneSelectionHandCheckBox");
    auto *graveyard = widget->findChild<QCheckBox *>("zoneSelectionGraveyardCheckBox");
    auto *library = widget->findChild<QCheckBox *>("zoneSelectionLibraryCheckBox");
    auto *confirm = btn("zoneSelectionConfirmButton");
    ASSERT_NE(hand, nullptr);
    ASSERT_NE(graveyard, nullptr);
    ASSERT_NE(library, nullptr);
    ASSERT_NE(confirm, nullptr);
    EXPECT_FALSE(hand->isHidden());
    EXPECT_FALSE(graveyard->isHidden());
    EXPECT_FALSE(library->isHidden());
    EXPECT_FALSE(confirm->isEnabled());
    EXPECT_EQ(btn("ruledChoiceOptionButton_0"), nullptr);

    hand->click();
    graveyard->click();
    EXPECT_TRUE(confirm->isEnabled());
    confirm->click();
    ASSERT_EQ(optionSpy.count(), 1);
    EXPECT_EQ(optionSpy.takeFirst().at(0).toInt(), 3);
}

TEST_F(GamePromptWidgetTest, ResolutionPaymentAutoCompletesAndHasNoPayButton)
{
    GamePromptWidget::RuledPromptState state;
    state.mode = PromptMode::ResolutionPayment;
    state.text = "Pay {2}{R}.";
    state.paymentCurrentlyLegal = false;
    widget->setRuledPromptState(state);
    EXPECT_EQ(btn("resolutionPaymentPayButton"), nullptr);
    EXPECT_FALSE(btn("resolutionPaymentDeclineButton")->isHidden());
}

TEST_F(GamePromptWidgetTest, WaitingForResolutionChoiceSuppressesEveryActionControl)
{
    widget->setLocalPlayerHasPriority(true);
    widget->setRuledPromptState({PromptMode::WaitingForChoice, 0, 0, "Waiting for p1...", {}});

    EXPECT_EQ(widget->effectiveMode(), PromptMode::WaitingForChoice);
    EXPECT_EQ(label("promptLabel")->text(), QString("Waiting for p1..."));
    EXPECT_TRUE(btn("passPriorityButton")->isHidden());
    EXPECT_TRUE(btn("resolutionPaymentDeclineButton")->isHidden());
    EXPECT_TRUE(btn("undoLandTapButton")->isHidden());
}

TEST_F(GamePromptWidgetTest, TriggerOrderTakesOverFromTargetingAndShowsTheCallerText)
{
    widget->setLocalPlayerHasPriority(true);
    // CR 603.3b: the engine is hard-blocked on the answer, so a stale mid-cast targeting state
    // must not outrank the ordering prompt.
    widget->setSpellCastPending(true);
    widget->setRuledPromptState(
        {PromptMode::TriggerOrder, 2, 0, "Click the trigger to put on the stack next (2 left).", {}});
    EXPECT_EQ(widget->effectiveMode(), PromptMode::TriggerOrder);
    EXPECT_TRUE(label("promptLabel")->text().contains(QStringLiteral("put on the stack next")));

    widget->setSpellCastPending(false);
    EXPECT_EQ(widget->effectiveMode(), PromptMode::TriggerOrder);
}

TEST_F(GamePromptWidgetTest, TriggerOrderHidesPriorityAndCombatButtons)
{
    widget->setLocalPlayerHasPriority(true);
    widget->setRuledPromptState({PromptMode::TriggerOrder, 2, 0, "Click the trigger to put on the stack next.", {}});
    // Picking happens by clicking a card in the ordering popup — this mode owns no button.
    EXPECT_TRUE(btn("passPriorityButton")->isHidden());
    EXPECT_TRUE(btn("resolutionHandPickConfirmButton")->isHidden());
    EXPECT_TRUE(btn("declineClickChoiceButton")->isHidden());

    widget->setRuledPromptState({});
    EXPECT_EQ(widget->effectiveMode(), PromptMode::Normal);
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
