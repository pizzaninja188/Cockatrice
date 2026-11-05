#ifndef RULED_GAME_MODE_UI_H
#define RULED_GAME_MODE_UI_H

class QAction;
class QCheckBox;

// Product entry-point policy. Retain upstream implementations for periodic merges.
namespace RuledGameModeUi
{
bool allowsLocalFreeformGames();
void configureCreationControls(QCheckBox *ruled, QCheckBox *shuffle);
void disableLocalGameAction(QAction *action);
} // namespace RuledGameModeUi

#endif
