#include "ruled_game_menu.h"

#include "../../interface/widgets/menus/tearoff_menu.h"
#include "../../interface/widgets/tabs/tab_game.h"
#include "../prompt/game_prompt_widget.h"
#include "ruled_actions.h"
#include "ruled_game_mode_ui.h"

#include <QAction>
#include <QPushButton>
#include <libcockatrice/game/game.h>
#include <libcockatrice/game/game_event_handler.h>

namespace
{
void suppressAction(QAction *action)
{
    const auto suppress = [action] {
        if (action->isVisible()) {
            action->setVisible(false);
        }
        if (action->isEnabled()) {
            action->setEnabled(false);
        }
        if (!action->shortcuts().isEmpty()) {
            action->setShortcuts({});
        }
    };
    // Retain upstream actions, but keep role, translation and shortcut refreshes from restoring them.
    QObject::connect(action, &QAction::changed, action, suppress);
    suppress();
}
} // namespace

void RuledGameMenu::configure(TabGame &tab)
{
    if (!RuledActions::isRuledGame(tab.game)) {
        return;
    }
    for (auto *action : {tab.aReverseTurn, tab.aTurnOrder, tab.phasesMenu->menuAction(), tab.aNextPhase,
                         tab.aNextPhaseAction, tab.aCardMenu}) {
        suppressAction(action);
    }
    for (auto *action : tab.phaseActions) {
        suppressAction(action);
    }

    QObject::disconnect(tab.aNextTurn, &QAction::triggered, tab.game->getGameEventHandler(),
                        &GameEventHandler::handleNextTurn);
    RuledGameModeUi::bindPriorityMenuAction(
        tab.aNextTurn, tab.gamePromptWidget->findChild<QPushButton *>(QStringLiteral("passPriorityButton")));
}
