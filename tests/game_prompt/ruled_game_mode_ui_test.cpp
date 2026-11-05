#include "game/ruled/ruled_game_mode_ui.h"

#include <QAction>
#include <QCheckBox>
#include <QSignalSpy>
#include <QWidget>
#include <gtest/gtest.h>

TEST(RuledGameModeUiTest, ModeControlsCannotOfferFreeformOrShuffle)
{
    QWidget parent;
    QCheckBox ruled(&parent);
    QCheckBox shuffle(&parent);
    shuffle.setChecked(true); // Simulate a saved upstream preference.
    parent.show();
    ASSERT_FALSE(ruled.isHidden());
    ASSERT_FALSE(shuffle.isHidden());
    RuledGameModeUi::configureCreationControls(&ruled, &shuffle);
    QSignalSpy modeChanges(&ruled, &QCheckBox::toggled);
    QSignalSpy shuffleChanges(&shuffle, &QCheckBox::toggled);
    ruled.click();
    shuffle.click();
    EXPECT_TRUE(ruled.isChecked());
    EXPECT_TRUE(ruled.isHidden());
    EXPECT_FALSE(ruled.isEnabled());
    EXPECT_FALSE(shuffle.isChecked());
    EXPECT_TRUE(shuffle.isHidden());
    EXPECT_FALSE(shuffle.isEnabled());
    EXPECT_EQ(modeChanges.count(), 0);
    EXPECT_EQ(shuffleChanges.count(), 0);
}

TEST(RuledGameModeUiTest, ConnectionStateCannotRestoreLocalGameAction)
{
    QAction localGame;
    ASSERT_TRUE(localGame.isEnabled());
    ASSERT_TRUE(localGame.isVisible());
    QSignalSpy triggered(&localGame, &QAction::triggered);
    RuledGameModeUi::disableLocalGameAction(&localGame);
    // Upstream connection-status and game-end callbacks try to enable this action again.
    localGame.setEnabled(true);
    localGame.setVisible(true);
    localGame.trigger();
    EXPECT_FALSE(localGame.isEnabled());
    EXPECT_FALSE(localGame.isVisible());
    EXPECT_EQ(triggered.count(), 0);
}
