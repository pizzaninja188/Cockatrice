#ifndef RULED_GAME_MODE_UI_H
#define RULED_GAME_MODE_UI_H

class QAction;
class QCheckBox;
class QPushButton;
class QComboBox;
class QSpinBox;
class ServerInfo_Game;

// Product entry-point policy. Retain upstream implementations for periodic merges.
namespace RuledGameModeUi
{
bool allowsLocalFreeformGames();
void configureCreationControls(QCheckBox *ruled, QCheckBox *shuffle);
void configureFormatControls(QComboBox *format, QSpinBox *players, QSpinBox *life);
void configureFormatInfo(QComboBox *format, QSpinBox *players, const ServerInfo_Game &info);
void disableLocalGameAction(QAction *action);
void bindPriorityMenuAction(QAction *action, QPushButton *button);
} // namespace RuledGameModeUi

#endif
