#include "ruled_automation_ui.h"

#include "../../client/settings/cache_settings.h"
#include "../../client/settings/shortcuts_settings.h"
#include "../../game_graphics/board/card_item.h"
#include "../../game_graphics/phases_toolbar.h"
#include "../../interface/widgets/tabs/tab_game.h"
#include "../prompt/game_prompt_widget.h"
#include "ruled_actions.h"
#include "ruled_auto_pass_policy.h"
#include "ruled_automation_controls.h"
#include "ruled_client_state.h"
#include "ruled_control_bindings.h"
#include "ruled_diagnostic_viewer.h"

#include <QDockWidget>
#include <QMenu>
#include <QPointer>
#include <libcockatrice/game/game.h>
#include <libcockatrice/game/game_event_handler.h>
#include <libcockatrice/game/game_meta_info.h>
#include <libcockatrice/game/player/player_info.h>
#include <libcockatrice/game/player/player_logic.h>
#include <libcockatrice/game/player/player_manager.h>
#include <libcockatrice/game/zones/card_zone_logic.h>
#include <libcockatrice/utility/zone_names.h>
#include <memory>

namespace rv1 = ruled::v1;
namespace
{
const QList<QPair<int, const char *>> keys{
    {1, "Player/ruledPrimary"},  {2, "Player/ruledUntilResponse"},   {3, "Player/ruledYes"},
    {4, "Player/ruledNo"},       {5, "Player/ruledClearAutomation"}, {6, "Player/ruledPassTurn"},
    {8, "Player/ruledAutoPass"}, {0, "Player/ruledEndStep"}};
RuledAutomationControls *controlsFor(TabGame &tab)
{
    return tab.findChild<RuledAutomationControls *>(QStringLiteral("ruledAutomationControls"));
}
QDockWidget *dockFor(TabGame &tab)
{
    return tab.findChild<QDockWidget *>(QStringLiteral("ruledControlsDock"));
}
int toolbarIndex(rv1::PhaseId phase)
{
    google::protobuf::RepeatedField<int> phases;
    phases.Add(phase);
    const auto stops = RuledAutoPassPolicy::toToolbarStops(phases);
    for (int i = 0; i < static_cast<int>(stops.size()); ++i) {
        if (stops[i]) {
            return i;
        }
    }
    return -1;
}
} // namespace
void RuledAutomationUi::configure(TabGame &tab)
{
    if (!tab.gamePromptWidget || controlsFor(tab) || tab.game->getPlayerManager()->isSpectator()) {
        return;
    }
    auto *dock = new QDockWidget(&tab);
    dock->setObjectName(QStringLiteral("ruledControlsDock"));
    dock->setFeatures(QDockWidget::DockWidgetClosable | QDockWidget::DockWidgetFloatable |
                      QDockWidget::DockWidgetMovable);
    auto *controls = new RuledAutomationControls(dock);
    dock->setWidget(controls);
    tab.addDockWidget(Qt::RightDockWidgetArea, dock);
    QObject::connect(dock, &QDockWidget::topLevelChanged, &tab, [&tab] { retranslateUi(tab); });
    controls->setScope(&tab);
    tab.gamePromptWidget->controlBindings()->setKeys(QStringLiteral("primary"), {});
    QObject::connect(controls, &RuledAutomationControls::primaryRequested, tab.gamePromptWidget,
                     &GamePromptWidget::activatePrimaryAction);
    QObject::connect(tab.gamePromptWidget, &GamePromptWidget::primaryActionAvailabilityChanged, controls,
                     [&tab] { refresh(tab); });
    auto sequence = std::make_shared<quint64>(0);
    QObject::connect(controls, &RuledAutomationControls::commandRequested, controls,
                     [&tab, controls, sequence](const rv1::RuledCommand &command) {
                         if (!command.has_update_automation()) {
                             RuledActions::sendRuledCommand(tab.game, command);
                             return;
                         }
                         const auto request = ++*sequence;
                         controls->setUpdating(true);
                         QPointer<RuledAutomationControls> guard(controls);
                         RuledActions::sendRuledCommandExpectingAck(tab.game, command,
                                                                    [guard, sequence, request](bool accepted) {
                                                                        if (guard && request == *sequence) {
                                                                            guard->setUpdating(false, !accepted);
                                                                        }
                                                                    });
                     });
    const auto shortcuts = [controls] {
        for (const auto &entry : keys) {
            controls->setShortcut(entry.first, SettingsCache::instance().shortcuts().getShortcut(entry.second));
        }
    };
    QObject::connect(&SettingsCache::instance().shortcuts(), &ShortcutsSettings::shortCutChanged, controls, shortcuts);
    shortcuts();
    QObject::connect(tab.game->getGameMetaInfo(), &GameMetaInfo::startedChanged, controls, [&tab] { refresh(tab); });
    if (auto *state = RuledActions::stateFor(tab.game)) {
        QObject::connect(state, &RuledClientState::legalActionsChanged, controls, [&tab] { refresh(tab); });
        QObject::connect(state, &RuledClientState::sessionReset, controls, [&tab, controls, sequence] {
            ++*sequence;
            controls->setUpdating(false);
            refresh(tab);
        });
        QObject::connect(state, &RuledClientState::localActionAuthorityRevoked, controls, [&tab] { refresh(tab); });
    }
    QObject::connect(tab.phasesToolbar, &PhasesToolbar::automationContextMenuRequested, controls,
                     [&tab, controls](int index, const QPoint &position) {
                         std::array<bool, 11> selected{};
                         if (index < 1 || index >= static_cast<int>(selected.size())) {
                             return;
                         }
                         selected[index] = true;
                         const auto phases = RuledAutoPassPolicy::fromToolbarStops(selected, {}).stop_on_own_turn();
                         if (phases.empty()) {
                             return;
                         }
                         QMenu *menu = nullptr;
                         if (phases.size() == 1) {
                             menu = controls->phaseMenu(static_cast<rv1::PhaseId>(phases[0]));
                         } else {
                             menu = new QMenu(controls);
                             for (int phase : phases) {
                                 auto *child = controls->phaseMenu(static_cast<rv1::PhaseId>(phase));
                                 child->setParent(menu, child->windowFlags());
                                 child->setTitle(phase == rv1::PHASE_ID_FIRST_STRIKE_DAMAGE
                                                     ? QObject::tr("First strike damage")
                                                     : QObject::tr("Combat damage"));
                                 menu->addMenu(child);
                             }
                         }
                         menu->setAttribute(Qt::WA_DeleteOnClose);
                         menu->popup(position);
                     });
    refresh(tab);
}
void RuledAutomationUi::registerDock(TabGame &tab)
{
    if (auto *dock = dockFor(tab)) {
        tab.registerDockWidget(tab.viewMenu, dock, {350, 150});
    }
}
void RuledAutomationUi::retranslateUi(TabGame &tab)
{
    if (auto *dock = dockFor(tab)) {
        const auto title = QObject::tr("Game Controls");
        dock->setWindowTitle(title + (dock->isFloating()
                                         ? QObject::tr(" | Game #%1").arg(tab.game->getGameMetaInfo()->gameId())
                                         : QString()));
        if (tab.dockToActions.contains(dock)) {
            tab.dockToActions[dock].menu->setTitle(title);
        }
    }
}
void RuledAutomationUi::resetLayout(TabGame &tab)
{
    if (auto *dock = dockFor(tab)) {
        dock->setFloating(false);
        tab.addDockWidget(Qt::RightDockWidgetArea, dock);
        dock->show();
        dock->resize(350, 150);
    }
}
void RuledAutomationUi::refresh(TabGame &tab)
{
    auto *controls = controlsFor(tab);
    if (!controls) {
        return;
    }
    const auto *state = RuledActions::stateFor(tab.game);
    const bool live = state && qobject_cast<Game *>(tab.game) && tab.game->getGameMetaInfo()->started() &&
                      !tab.game->getPlayerManager()->isSpectator() && !RuledDiagnosticViewer::isPlayback(tab.game);
    const auto snapshot = state ? state->automation : rv1::AutomationState{};
    const quint64 prompt =
        state && state->isCurrentResolutionPrompt(state->resolutionPromptId) ? state->resolutionPromptId : 0;
    controls->setControlState(snapshot, prompt, live, state && state->isEngineCommandPending(),
                              tab.gamePromptWidget && tab.gamePromptWidget->primaryActionAvailable());
    controls->setEnabled(live);
    tab.phasesToolbar->setAutomationContextMenusEnabled(live);
    tab.phasesToolbar->setAutomationMarkers(
        live && snapshot.has_stop_once() ? toolbarIndex(snapshot.stop_once().phase()) : -1,
        live && snapshot.has_yield_phase() ? toolbarIndex(snapshot.yield_phase().phase()) : -1);
}
QMenu *RuledAutomationUi::stackMenu(CardItem *card)
{
    if (!card || !card->getOwner() || !card->getZone() || card->getZone()->getName() != ZoneNames::STACK) {
        return nullptr;
    }
    auto *game = card->getOwner()->getGame();
    auto *state = RuledActions::stateFor(game);
    auto *tab = RuledActions::tab(game);
    if (!state || !tab || RuledDiagnosticViewer::isPlayback(game)) {
        return nullptr;
    }
    auto *controls = controlsFor(*tab);
    if (!controls) {
        return nullptr;
    }
    const auto oid = state->engineOidForCardId(card->getOwner()->getPlayerInfo()->getId(), card->getId());
    return controls->stackMenu(oid);
}
bool RuledAutomationUi::tryStackMenu(CardItem *card, const QPoint &position)
{
    auto *menu = stackMenu(card);
    if (!menu) {
        return false;
    }
    menu->setAttribute(Qt::WA_DeleteOnClose);
    menu->popup(position);
    return true;
}
