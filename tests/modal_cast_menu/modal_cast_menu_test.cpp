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
