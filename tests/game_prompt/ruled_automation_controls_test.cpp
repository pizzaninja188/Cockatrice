#include "game/ruled/ruled_automation_controls.h"

#include <QAction>
#include <QApplication>
#include <QDialog>
#include <QDockWidget>
#include <QKeyEvent>
#include <QLabel>
#include <QLineEdit>
#include <QMainWindow>
#include <QMenu>
#include <QPushButton>
#include <QSignalSpy>
#include <QTest>
#include <QVBoxLayout>
#include <gtest/gtest.h>
#include <memory>

namespace rv1 = ruled::v1;
namespace
{
QPushButton *button(RuledAutomationControls &controls, int key)
{
    return controls.findChild<QPushButton *>(QStringLiteral("ruledControl%1").arg(key));
}
rv1::AutomationState ready()
{
    rv1::AutomationState state;
    state.set_can_start_yield(true);
    return state;
}
} // namespace

TEST(RuledAutomationControlsTest, AllEightControlsShareEnabledActionsAndCurrentPromptIdentity)
{
    RuledAutomationControls controls;
    std::vector<rv1::RuledCommand> sent;
    QObject::connect(&controls, &RuledAutomationControls::commandRequested, &controls,
                     [&](const auto &command) { sent.push_back(command); });
    QSignalSpy primary(&controls, &RuledAutomationControls::primaryRequested);
    auto state = ready();
    controls.setControlState(state, 0, true, false, true);
    for (int key : {1, 2, 3, 4, 5, 6, 8, 0}) {
        ASSERT_NE(button(controls, key), nullptr);
    }
    button(controls, 1)->click();
    EXPECT_EQ(primary.size(), 1);
    EXPECT_FALSE(button(controls, 3)->isEnabled());
    EXPECT_FALSE(button(controls, 4)->isEnabled());
    for (const auto &[key, mode] :
         std::vector<std::pair<int, rv1::AutomationYieldMode>>{{2, rv1::AUTOMATION_YIELD_MODE_UNTIL_RESPONSE},
                                                               {6, rv1::AUTOMATION_YIELD_MODE_TURN},
                                                               {0, rv1::AUTOMATION_YIELD_MODE_END_STEP}}) {
        button(controls, key)->click();
        ASSERT_FALSE(sent.empty());
        EXPECT_EQ(sent.back().update_automation().yield_to().mode(), mode);
    }
    state.set_yes_branch_index(0);
    state.set_can_answer_no(true);
    controls.setControlState(state, 913, true, false, true);
    button(controls, 3)->click();
    EXPECT_EQ(sent.back().submit_resolution_choice().resolution_prompt_id(), 913u);
    EXPECT_EQ(sent.back().submit_resolution_choice().decision(), rv1::RESOLUTION_CHOICE_DECISION_SELECT_BRANCH);
    button(controls, 4)->click();
    EXPECT_EQ(sent.back().submit_resolution_choice().decision(), rv1::RESOLUTION_CHOICE_DECISION_DECLINE);
    controls.setControlState(state, 0, true, false, false);
    const auto before = sent.size();
    button(controls, 3)->click();
    button(controls, 4)->click();
    button(controls, 1)->click();
    EXPECT_EQ(sent.size(), before);
    EXPECT_EQ(primary.size(), 1);
    button(controls, 8)->click();
    EXPECT_TRUE(sent.back().update_automation().auto_pass());
    controls.setControlState(state, 0, true, true, true);
    EXPECT_FALSE(button(controls, 6)->isEnabled());
    EXPECT_FALSE(button(controls, 1)->isEnabled());
    button(controls, 5)->click();
    EXPECT_TRUE(sent.back().update_automation().clear_all()) << "Clear remains available during an in-flight command";
}

TEST(RuledAutomationControlsTest, FullControlAndAutoPassDisplayOnlyAcknowledgedState)
{
    RuledAutomationControls controls;
    auto state = ready();
    controls.setControlState(state, 0, true, false, true);
    auto *full = controls.findChild<QPushButton *>("ruledFullControl");
    auto *status = controls.findChild<QLabel *>("ruledAutomationStatus");
    ASSERT_NE(full, nullptr);
    ASSERT_NE(status, nullptr);
    std::vector<rv1::RuledCommand> sent;
    QObject::connect(&controls, &RuledAutomationControls::commandRequested, &controls,
                     [&](const auto &command) { sent.push_back(command); });
    full->click();
    ASSERT_EQ(sent.size(), 1u);
    EXPECT_TRUE(sent.back().update_automation().full_control());
    EXPECT_FALSE(full->isChecked());
    controls.setUpdating(true);
    EXPECT_TRUE(status->text().contains("Updating"));
    EXPECT_TRUE(button(controls, 5)->isEnabled());
    EXPECT_TRUE(full->isEnabled());
    EXPECT_FALSE(button(controls, 8)->isEnabled());
    state.set_full_control(true);
    state.set_auto_pass(true);
    controls.setControlState(state, 0, true, false, true);
    controls.setUpdating(false);
    EXPECT_TRUE(full->isChecked());
    EXPECT_TRUE(status->text().contains("auto-pass enabled"));
    controls.setControlState(state, 0, false, false, true);
    const auto before = sent.size();
    button(controls, 5)->click();
    full->click();
    EXPECT_EQ(sent.size(), before) << "spectators, replay and inactive games have no command authority";
}

TEST(RuledAutomationControlsTest, StackMenuUsesExactOfferedIdentityAndSeparatesYieldFromAnswers)
{
    RuledAutomationControls controls;
    auto state = ready();
    auto *offer = state.add_stack_offers();
    offer->mutable_stack()->set_object_id(701);
    offer->mutable_stack()->set_zone_change_generation(19);
    offer->set_can_yield_trigger(true);
    offer->set_can_source_rule(true);
    offer->set_can_answer_optional(true);
    controls.setControlState(state, 0, true, false, true);
    std::vector<rv1::RuledCommand> sent;
    QObject::connect(&controls, &RuledAutomationControls::commandRequested, &controls,
                     [&](const auto &command) { sent.push_back(command); });
    std::unique_ptr<QMenu> menu(controls.stackMenu(701));
    ASSERT_NE(menu, nullptr);
    auto *item = menu->findChild<QAction *>("yieldStackItem");
    auto *entire = menu->findChild<QAction *>("yieldEntireStack");
    auto *source = menu->findChild<QAction *>("yieldThisSource");
    auto *broad = menu->findChild<QAction *>("yieldMatchingSources");
    ASSERT_NE(item, nullptr);
    ASSERT_NE(entire, nullptr);
    ASSERT_NE(source, nullptr);
    ASSERT_NE(broad, nullptr);
    item->trigger();
    ASSERT_EQ(sent.size(), 1u);
    EXPECT_EQ(sent.back().update_automation().yield_to().stack().SerializeAsString(),
              offer->stack().SerializeAsString());
    entire->trigger();
    EXPECT_EQ(sent.back().update_automation().yield_to().mode(), rv1::AUTOMATION_YIELD_MODE_ENTIRE_STACK);
    EXPECT_FALSE(sent.back().update_automation().yield_to().has_stack());
    source->trigger();
    EXPECT_EQ(sent.back().update_automation().trigger_rule().scope(), rv1::AUTOMATION_TRIGGER_SCOPE_SOURCE);
    EXPECT_TRUE(sent.back().update_automation().trigger_rule().yield_priority());
    broad->trigger();
    EXPECT_EQ(sent.back().update_automation().trigger_rule().scope(), rv1::AUTOMATION_TRIGGER_SCOPE_MATCHING_ABILITY);
    auto *no = menu->findChild<QAction *>("triggerAnswer2");
    ASSERT_NE(no, nullptr);
    no->trigger();
    EXPECT_EQ(sent.back().update_automation().trigger_rule().answer(), rv1::AUTOMATION_ANSWER_NO);
    EXPECT_FALSE(sent.back().update_automation().trigger_rule().yield_priority());
    const auto before = sent.size();
    offer->mutable_stack()->set_zone_change_generation(20);
    controls.setControlState(state, 0, true, false, true);
    item->trigger();
    source->trigger();
    no->trigger();
    EXPECT_EQ(sent.size(), before) << "a menu from an earlier stack incarnation is inert";
    EXPECT_EQ(controls.stackMenu(999), nullptr);
}

TEST(RuledAutomationControlsTest, StackPopupKeepsManagerHiddenUntilItsSubmenuIsOpened)
{
    RuledAutomationControls controls;
    auto state = ready();
    state.add_stack_offers()->mutable_stack()->set_object_id(701);
    controls.setControlState(state, 0, true, false, true);
    std::unique_ptr<QMenu> menu(controls.stackMenu(701));
    ASSERT_NE(menu, nullptr);
    auto *manager = menu->findChild<QMenu *>("ruledAutomationManager");
    ASSERT_NE(manager, nullptr);
    EXPECT_EQ(manager->windowType(), Qt::Popup);

    menu->popup(QPoint(100, 100));
    QApplication::processEvents();
    EXPECT_TRUE(menu->isVisible());
    EXPECT_FALSE(manager->isVisible()) << "Opening the stack menu must not also display its manager";
    EXPECT_EQ(QApplication::activePopupWidget(), menu.get());

    menu->setActiveAction(manager->menuAction());
    QTest::keyClick(menu.get(), Qt::Key_Right);
    QApplication::processEvents();
    EXPECT_TRUE(manager->isVisible());
    EXPECT_EQ(QApplication::activePopupWidget(), manager);
    QTest::keyClick(manager, Qt::Key_Escape);
    QApplication::processEvents();
    EXPECT_FALSE(manager->isVisible());
    EXPECT_TRUE(menu->isVisible());
    menu->close();
}

TEST(RuledAutomationControlsTest, PhaseMenuKeepsTurnScopeAndOneTimeStopSeparateFromYield)
{
    RuledAutomationControls controls;
    controls.setControlState(ready(), 0, true, false, true);
    std::vector<rv1::RuledCommand> sent;
    QObject::connect(&controls, &RuledAutomationControls::commandRequested, &controls,
                     [&](const auto &command) { sent.push_back(command); });
    std::unique_ptr<QMenu> menu(controls.phaseMenu(rv1::PHASE_ID_END_STEP));
    ASSERT_NE(menu, nullptr);
    auto *stop = menu->findChild<QAction *>("stopOnceOpponentTurn");
    auto *yield = menu->findChild<QAction *>("yieldPhaseOwnTurn");
    ASSERT_NE(stop, nullptr);
    ASSERT_NE(yield, nullptr);
    stop->trigger();
    ASSERT_EQ(sent.size(), 1u);
    EXPECT_EQ(sent.back().update_automation().stop_once().phase(), rv1::PHASE_ID_END_STEP);
    EXPECT_FALSE(sent.back().update_automation().stop_once().on_own_turn());
    yield->trigger();
    EXPECT_EQ(sent.back().update_automation().yield_to().mode(), rv1::AUTOMATION_YIELD_MODE_PHASE);
    EXPECT_TRUE(sent.back().update_automation().yield_to().phase().on_own_turn());
}

TEST(RuledAutomationControlsTest, DigitsNumpadAndRebindingsDoNotFireInTextEntryOrOnRepeat)
{
    QWidget scope;
    scope.setFocusPolicy(Qt::StrongFocus);
    auto *layout = new QVBoxLayout(&scope);
    auto *controls = new RuledAutomationControls(&scope);
    auto *chat = new QLineEdit(&scope);
    layout->addWidget(controls);
    layout->addWidget(chat);
    controls->setScope(&scope);
    controls->setControlState(ready(), 0, true, false, true);
    std::vector<rv1::RuledCommand> sent;
    QObject::connect(controls, &RuledAutomationControls::commandRequested, controls,
                     [&](const auto &command) { sent.push_back(command); });
    scope.show();
    scope.activateWindow();
    scope.setFocus();
    QApplication::processEvents();
    QTest::keyClick(&scope, Qt::Key_2);
    ASSERT_EQ(sent.size(), 1u);
    QTest::keyClick(&scope, Qt::Key_2, Qt::KeypadModifier);
    EXPECT_EQ(sent.size(), 2u);
    QKeyEvent repeat(QEvent::KeyPress, Qt::Key_2, Qt::NoModifier, QStringLiteral("2"), true, 1);
    QApplication::sendEvent(&scope, &repeat);
    EXPECT_EQ(sent.size(), 2u);
    chat->setFocus();
    QApplication::processEvents();
    QTest::keyClick(chat, Qt::Key_2);
    EXPECT_EQ(chat->text(), QStringLiteral("2"));
    EXPECT_EQ(sent.size(), 2u);
    controls->setShortcut(2, {QKeySequence(Qt::CTRL | Qt::Key_R)});
    scope.setFocus();
    QApplication::processEvents();
    QTest::keyClick(&scope, Qt::Key_2);
    EXPECT_EQ(sent.size(), 2u);
    QTest::keyClick(&scope, Qt::Key_R, Qt::ControlModifier);
    EXPECT_EQ(sent.size(), 3u);
}

TEST(RuledAutomationControlsTest, ShortcutsWorkFromBoardAndFloatingDockButNotTextEntryOrOtherWindows)
{
    QMainWindow game;
    auto *board = new QWidget(&game);
    board->setFocusPolicy(Qt::StrongFocus);
    game.setCentralWidget(board);
    auto *dock = new QDockWidget(&game);
    auto *controls = new RuledAutomationControls(dock);
    dock->setWidget(controls);
    game.addDockWidget(Qt::RightDockWidgetArea, dock);
    controls->setScope(&game);
    controls->setControlState(ready(), 0, true, false, true);
    std::vector<rv1::RuledCommand> sent;
    QObject::connect(controls, &RuledAutomationControls::commandRequested, controls,
                     [&](const auto &command) { sent.push_back(command); });
    game.show();
    game.activateWindow();
    board->setFocus();
    QApplication::processEvents();
    QTest::keyClick(board, Qt::Key_2);
    ASSERT_EQ(sent.size(), 1u);

    dock->setFloating(true);
    dock->show();
    dock->activateWindow();
    auto *clear = button(*controls, 5);
    clear->setFocus();
    QApplication::processEvents();
    QTest::keyClick(clear, Qt::Key_2);
    EXPECT_EQ(sent.size(), 2u);
    QTest::keyClick(clear, Qt::Key_2, Qt::KeypadModifier);
    EXPECT_EQ(sent.size(), 3u);

    game.activateWindow();
    board->setFocus();
    QApplication::processEvents();
    QTest::keyClick(board, Qt::Key_2);
    EXPECT_EQ(sent.size(), 4u) << "Floating the controls must preserve battlefield shortcuts";
    auto *chat = new QLineEdit(board);
    chat->show();
    chat->setFocus();
    QApplication::processEvents();
    QTest::keyClick(chat, Qt::Key_2);
    EXPECT_EQ(chat->text(), QStringLiteral("2"));
    EXPECT_EQ(sent.size(), 4u);

    board->setFocus();
    QMenu popup(&game);
    popup.addAction(QStringLiteral("Inspect"));
    popup.popup(QPoint(100, 100));
    QApplication::processEvents();
    QTest::keyClick(&popup, Qt::Key_2);
    EXPECT_EQ(sent.size(), 4u);
    popup.close();
    QDialog choice(&game);
    choice.setModal(true);
    choice.show();
    choice.activateWindow();
    choice.setFocus();
    QApplication::processEvents();
    QTest::keyClick(&choice, Qt::Key_2);
    EXPECT_EQ(sent.size(), 4u);
    choice.close();

    QWidget other;
    auto *otherLayout = new QVBoxLayout(&other);
    auto *otherControls = new RuledAutomationControls(&other);
    otherLayout->addWidget(otherControls);
    otherControls->setScope(&other);
    otherControls->setControlState(ready(), 0, true, false, true);
    int otherCommands = 0;
    QObject::connect(otherControls, &RuledAutomationControls::commandRequested, otherControls,
                     [&](const auto &) { ++otherCommands; });
    other.setFocusPolicy(Qt::StrongFocus);
    other.show();
    other.activateWindow();
    other.setFocus();
    QApplication::processEvents();
    QTest::keyClick(&other, Qt::Key_2);
    EXPECT_EQ(sent.size(), 4u);
    EXPECT_EQ(otherCommands, 1) << "Only the focused game may receive a shortcut shared by both games";
}
