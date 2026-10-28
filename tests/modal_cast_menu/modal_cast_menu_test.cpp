#include "game/ruled/ruled_pending_cast.h"

#include <QApplication>
#include <QCheckBox>
#include <QLabel>
#include <QMenu>
#include <QPushButton>
#include <QTest>
#include <QTimer>
#include <gtest/gtest.h>

namespace
{
QMenu *activeMenu()
{
    for (QWidget *widget : QApplication::topLevelWidgets()) {
        if (auto *menu = qobject_cast<QMenu *>(widget); menu && menu->isVisible()) {
            return menu;
        }
    }
    return nullptr;
}

QPushButton *buttonWithText(QMenu *menu, const QString &text)
{
    for (auto *button : menu->findChildren<QPushButton *>()) {
        if (button->text() == text) {
            return button;
        }
    }
    return nullptr;
}

QVector<RuledModalSpellOption> threeModes()
{
    return {
        {0, QStringLiteral("First"), true, false, {}},
        {1, QStringLiteral("Second"), true, false, {}},
        {2, QStringLiteral("Unavailable"), false, false, {}},
    };
}
} // namespace

TEST(ModalCastMenuTest, AdventureFacePickerUsesEngineOptionsWhenPhysicalNameIsOnlyTheFront)
{
    QTimer::singleShot(0, []() {
        QMenu *menu = activeMenu();
        ASSERT_NE(menu, nullptr);
        ASSERT_EQ(menu->actions().size(), 2);
    EXPECT_EQ(menu->actions().at(0)->text(), QStringLiteral("Cast Bonecrusher Giant ({2}{R})"));
    EXPECT_EQ(menu->actions().at(1)->text(), QStringLiteral("Cast Stomp ({1}{R})"));
        const QRect actionRect = menu->actionGeometry(menu->actions().at(1));
        QTest::mouseClick(menu, Qt::LeftButton, Qt::NoModifier, actionRect.center());
    });
    const QVector<RuledFaceOption> faces = {
        {0, QStringLiteral("Bonecrusher Giant"), QStringLiteral("{2}{R}")},
        {1, QStringLiteral("Stomp"), QStringLiteral("{1}{R}")},
    };
    const auto result = RuledPendingCast::chooseFace(nullptr, QStringLiteral("Bonecrusher Giant"), faces);
    ASSERT_TRUE(result.has_value());
    EXPECT_EQ(result->faceIndex, 1);
    EXPECT_EQ(result->faceName, QStringLiteral("Stomp"));
    EXPECT_EQ(result->manaCost, QStringLiteral("{1}{R}"));
}

TEST(ModalCastMenuTest, ChooseOneUsesOrdinaryActionsAndDisablesUnavailableMode)
{
    QTimer::singleShot(0, []() {
        QMenu *menu = activeMenu();
        ASSERT_NE(menu, nullptr);
        ASSERT_EQ(menu->actions().size(), 3);
        EXPECT_EQ(menu->actions().at(0)->text(), QStringLiteral("Cast Boros Charm — First"));
        EXPECT_FALSE(menu->actions().at(2)->isEnabled());
        const QRect actionRect = menu->actionGeometry(menu->actions().at(1));
        QTest::mouseClick(menu, Qt::LeftButton, Qt::NoModifier, actionRect.center());
    });
    const auto result =
        RuledPendingCast::chooseModes(nullptr, QStringLiteral("Boros Charm"), threeModes(), 1, 1);
    ASSERT_TRUE(result.has_value());
    EXPECT_EQ(*result, QVector<int>({1}));
}

TEST(ModalCastMenuTest, ActivatedAbilityModeUsesActivationWording)
{
    bool menuWasInspected = false;
    bool actionWasTriggered = false;
    QTimer closePopup;
    closePopup.setSingleShot(true);
    QObject::connect(&closePopup, &QTimer::timeout, &closePopup, []() {
        for (QWidget *widget : QApplication::topLevelWidgets()) {
            if (auto *menu = qobject_cast<QMenu *>(widget)) {
                menu->close();
            }
        }
    });
    closePopup.start(1000);
    QTimer::singleShot(0, [&menuWasInspected, &actionWasTriggered]() {
        QMenu *menu = activeMenu();
        if (!menu) {
            for (QWidget *widget : QApplication::topLevelWidgets()) {
                if (auto *popup = qobject_cast<QMenu *>(widget)) {
                    popup->close();
                }
            }
            return;
        }
        menuWasInspected = menu->actions().size() == 1 &&
                           menu->actions().at(0)->text() ==
                               QStringLiteral("Activate Test Artifact — Untap this artifact");
        if (!menuWasInspected) {
            menu->close();
            return;
        }
        const QRect actionRect = menu->actionGeometry(menu->actions().at(0));
        QTest::mouseClick(menu, Qt::LeftButton, Qt::NoModifier, actionRect.center());
        actionWasTriggered = true;
    });
    const QVector<RuledModalSpellOption> modes = {
        {2, QStringLiteral("Untap this artifact"), true, false, {}},
    };
    const auto result = RuledPendingCast::chooseModes(nullptr, QStringLiteral("Test Artifact"), modes, 1, 1,
                                                       RuledPendingCast::ModeChoicePurpose::ActivatedAbility);
    closePopup.stop();
    EXPECT_TRUE(menuWasInspected);
    EXPECT_TRUE(actionWasTriggered);
    ASSERT_TRUE(result.has_value());
    EXPECT_EQ(*result, QVector<int>({2}));
}

TEST(ModalCastMenuTest, ChooseMultipleKeepsCheckboxesOpenAndEnforcesExactCount)
{
    QTimer::singleShot(0, []() {
        QMenu *menu = activeMenu();
        ASSERT_NE(menu, nullptr);
        const auto checks = menu->findChildren<QCheckBox *>();
        ASSERT_EQ(checks.size(), 3);
        auto *confirm = buttonWithText(menu, QStringLiteral("Cast with selected modes"));
        ASSERT_NE(confirm, nullptr);
        EXPECT_FALSE(confirm->isEnabled());
        EXPECT_FALSE(checks.at(2)->isEnabled());
        checks.at(0)->click();
        EXPECT_TRUE(menu->isVisible());
        EXPECT_FALSE(confirm->isEnabled());
        checks.at(1)->click();
        EXPECT_TRUE(menu->isVisible());
        EXPECT_TRUE(confirm->isEnabled());
        const auto labels = menu->findChildren<QLabel *>();
        ASSERT_FALSE(labels.isEmpty());
        EXPECT_EQ(labels.first()->text(), QStringLiteral("Choose exactly 2 — 2/2 selected."));
        confirm->click();
    });
    const auto result =
        RuledPendingCast::chooseModes(nullptr, QStringLiteral("Test Command"), threeModes(), 2, 2);
    ASSERT_TRUE(result.has_value());
    EXPECT_EQ(*result, QVector<int>({0, 1}));
}

TEST(ModalCastMenuTest, ChooseMultipleSupportsRangeAndCancel)
{
    QTimer::singleShot(0, []() {
        QMenu *menu = activeMenu();
        ASSERT_NE(menu, nullptr);
        const auto checks = menu->findChildren<QCheckBox *>();
        checks.at(0)->click();
        auto *confirm = buttonWithText(menu, QStringLiteral("Cast with selected modes"));
        ASSERT_NE(confirm, nullptr);
        EXPECT_TRUE(confirm->isEnabled());
        auto *cancel = buttonWithText(menu, QStringLiteral("Cancel"));
        ASSERT_NE(cancel, nullptr);
        cancel->click();
    });
    const auto result =
        RuledPendingCast::chooseModes(nullptr, QStringLiteral("Test Command"), threeModes(), 1, 2);
    EXPECT_FALSE(result.has_value());
}

int main(int argc, char **argv)
{
    QApplication app(argc, argv);
    ::testing::InitGoogleTest(&argc, argv);
    return RUN_ALL_TESTS();
}
