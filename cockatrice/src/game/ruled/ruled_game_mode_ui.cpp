#include "ruled_game_mode_ui.h"

#include <QAction>
#include <QCheckBox>
#include <QObject>

namespace RuledGameModeUi
{
bool allowsLocalFreeformGames()
{
    return false;
}

void configureCreationControls(QCheckBox *ruled, QCheckBox *shuffle)
{
    ruled->setChecked(true);
    ruled->setEnabled(false);
    ruled->hide();
    shuffle->setChecked(false);
    shuffle->setEnabled(false);
    shuffle->hide();
}

void disableLocalGameAction(QAction *action)
{
    const auto disable = [action] {
        if (action->isEnabled()) {
            action->setEnabled(false);
        }
        if (action->isVisible()) {
            action->setVisible(false);
        }
    };
    // Upstream reconnect and game-end callbacks may try to enable the action again.
    QObject::connect(action, &QAction::changed, action, disable);
    disable();
}
} // namespace RuledGameModeUi
