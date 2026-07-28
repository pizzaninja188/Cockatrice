#include "tab_game.h"

#include "../../../client/settings/cache_settings.h"
#include "../game/board/arrow_item.h"
#include "../game/board/card_item.h"
#include "../game/deckview/deck_view_container.h"
#include "../game/deckview/tabbed_deck_view_container.h"
#include "../game/game.h"
#include "../game/game_scene.h"
#include "../game/game_view.h"
#include "../game/log/message_log_widget.h"
#include "../game/phases_toolbar.h"
#include "../game/player/player.h"
#include "../game/player/player_actions.h"
#include "../game/player/player_list_widget.h"
#include "../game/prompt/game_prompt_widget.h"
#include "../game/replay.h"
#include "../game/ruled/ruled_actions.h"
#include "../game/ruled/ruled_client_state.h"
#include "../game/ruled/ruled_dev_command_parser.h"
#include "../game/ruled/ruled_dev_console.h"
#include "../game/zones/view_zone.h"
#include "../game/zones/view_zone_widget.h"
#include "../interface/card_picture_loader/card_picture_loader.h"
#include "../interface/widgets/cards/card_info_frame_widget.h"
#include "../interface/widgets/dialogs/dlg_create_game.h"
#include "../interface/widgets/server/user/user_list_manager.h"
#include "../interface/widgets/utility/line_edit_completer.h"
#include "../interface/window_main.h"
#include "../main.h"
#include "../utility/visibility_change_listener.h"
#include "tab_supervisor.h"

#include <libcockatrice/protocol/pb/serverinfo_card.pb.h>
#include <libcockatrice/utility/zone_names.h>

#include <QAction>
#include <QCompleter>
#include <QDateTime>
#include <QDebug>
#include <QDir>
#include <QDockWidget>
#include <QFile>
#include <QGraphicsSceneMouseEvent>
#include <QHBoxLayout>
#include <QLabel>
#include <QMenu>
#include <QMessageBox>
#include <QStackedWidget>
#include <QTextStream>
#include <QTimer>
#include <QWidget>

// ---------------------------------------------------------------------------
// Diagnostic logger — same log file as game_event_handler.cpp / player_event_handler.cpp.
// Remove after the spell-on-stack visibility bug is resolved.
// ---------------------------------------------------------------------------
static void tgDbgLog(const QString &msg)
{
    static const QString kPath = QDir::homePath() + QStringLiteral("/cockatrice_stack_debug.log");
    QFile f(kPath);
    if (f.open(QIODevice::Append | QIODevice::Text)) {
        QTextStream ts(&f);
        ts << QDateTime::currentDateTime().toString(Qt::ISODateWithMs) << " [TG] " << msg << "\n";
    }
}
#include <libcockatrice/card/database/card_database.h>
#include <libcockatrice/card/database/card_database_manager.h>
#include <libcockatrice/network/client/abstract/abstract_client.h>
#include <libcockatrice/protocol/pb/event_game_joined.pb.h>
#include <libcockatrice/protocol/pb/game_replay.pb.h>
#include <libcockatrice/protocol/pb/serverinfo_player.pb.h>
#include <libcockatrice/protocol/pb/serverinfo_user.pb.h>
#include <libcockatrice/utility/trice_limits.h>

namespace {
class RuledCombatArrowItem : public ArrowItem
{
public:
    RuledCombatArrowItem(Player *player, ArrowTarget *startItem, ArrowTarget *targetItem, const QColor &color)
        : ArrowItem(player, -1, startItem, targetItem, color)
    {
    }

protected:
    void mousePressEvent(QGraphicsSceneMouseEvent *event) override
    {
        event->ignore();
    }
};

// Server card ids (Server_Player::nextCardId) are assigned per player starting at 0, so they
// collide across players. Combat arrows are inherently cross-player (attacker vs. blocker), so
// resolving an engine OID to a CardItem must be scoped to that object's owning player — a plain
// id-only search returns whichever player's table happens to hold the same id first.
CardItem *findOwnedTableCard(AbstractGame *game, int ownerPlayerId, int cardId)
{
    if (!game || cardId < 0 || ownerPlayerId < 0) {
        return nullptr;
    }
    Player *player = game->getPlayerManager()->getPlayers().value(ownerPlayerId, nullptr);
    if (!player) {
        return nullptr;
    }
    return player->getTableZone()->getCard(cardId);
}

// Resolve an engine ObjectId to the table CardItem owned by that object's controller.
CardItem *findTableCardForEngineOid(AbstractGame *game, const RuledClientState *handler, quint32 oid)
{
    if (!handler) {
        return nullptr;
    }
    return findOwnedTableCard(game, handler->playerIdForEngineOid(oid), handler->cardIdForEngineOid(oid));
}

ArrowTarget *findDefendingPlayerTarget(AbstractGame *game, int activePlayerId)
{
    if (!game) {
        return nullptr;
    }

    const QMap<int, Player *> &players = game->getPlayerManager()->getPlayers();
    for (auto it = players.constBegin(); it != players.constEnd(); ++it) {
        Player *player = it.value();
        if (!player || player->getPlayerInfo()->getId() == activePlayerId || player->getConceded()) {
            continue;
        }
        return player->getGraphicsItem()->getPlayerTarget();
    }
    return nullptr;
}
} // namespace

TabGame::TabGame(TabSupervisor *_tabSupervisor, GameReplay *_replay)
    : Tab(_tabSupervisor), sayLabel(nullptr), sayEdit(nullptr), gamePromptWidget(nullptr)
{
    // THIS CTOR IS USED ON REPLAY
    game = new Replay(this, _replay);

    createCardInfoDock(true);
    createPlayerListDock(true);
    createMessageDock(true);
    createPlayAreaWidget(true);
    createDeckViewContainerWidget(true);
    createReplayDock(_replay);

    addDockWidget(Qt::RightDockWidgetArea, cardInfoDock);
    addDockWidget(Qt::RightDockWidgetArea, playerListDock);
    addDockWidget(Qt::RightDockWidgetArea, messageLayoutDock);
    addDockWidget(Qt::BottomDockWidgetArea, replayDock);

    mainWidget = new QStackedWidget(this);
    mainWidget->addWidget(deckViewContainerWidget);
    mainWidget->addWidget(gamePlayAreaWidget);
    setCentralWidget(mainWidget);

    createReplayMenuItems();
    createViewMenuItems();

    connectToGameState();
    connectToPlayerManager();
    connectToGameEventHandler();
    connectPlayerListToGameEventHandler();
    connectMessageLogToGameEventHandler();

    retranslateUi();
    connect(&SettingsCache::instance().shortcuts(), &ShortcutsSettings::shortCutChanged, this,
            &TabGame::refreshShortcuts);
    refreshShortcuts();
    messageLog->logReplayStarted(game->getGameMetaInfo()->gameId());

    QTimer::singleShot(0, this, &TabGame::loadLayout);
}

TabGame::TabGame(TabSupervisor *_tabSupervisor,
                 QList<AbstractClient *> &_clients,
                 const Event_GameJoined &event,
                 const QMap<int, QString> &_roomGameTypes)
    : Tab(_tabSupervisor), userListProxy(_tabSupervisor->getUserListManager()), gamePromptWidget(nullptr)
{
    // THIS CTOR IS USED ON GAMES
    game = new Game(this, _clients, event, _roomGameTypes);

    createCardInfoDock();
    createPlayerListDock();
    createMessageDock();
    createPlayAreaWidget();
    createDeckViewContainerWidget();
    replayDock = nullptr;

    addDockWidget(Qt::RightDockWidgetArea, cardInfoDock);
    addDockWidget(Qt::RightDockWidgetArea, playerListDock);
    addDockWidget(Qt::RightDockWidgetArea, messageLayoutDock);

    mainWidget = new QStackedWidget(this);
    mainWidget->addWidget(deckViewContainerWidget);
    mainWidget->addWidget(gamePlayAreaWidget);
    mainWidget->setContentsMargins(0, 0, 0, 0);
    setCentralWidget(mainWidget);

    createMenuItems();
    createViewMenuItems();

    connectToGameState();
    connectToPlayerManager();
    connectToGameEventHandler();
    connectPlayerListToGameEventHandler();
    connectMessageLogToGameEventHandler();

    retranslateUi();
    connect(&SettingsCache::instance().shortcuts(), &ShortcutsSettings::shortCutChanged, this,
            &TabGame::refreshShortcuts);
    refreshShortcuts();

    // append game to rooms game list for others to see
    for (int i = game->getGameMetaInfo()->gameTypesSize() - 1; i >= 0; i--)
        gameTypes.append(game->getGameMetaInfo()->findRoomGameType(i));

    QTimer::singleShot(0, this, &TabGame::loadLayout);
}

void TabGame::connectToGameState()
{
    connect(game->getGameState(), &GameState::gameStarted, this, &TabGame::startGame);
    connect(game->getGameState(), &GameState::gameStopped, this, &TabGame::stopGame);
    connect(game->getGameState(), &GameState::activePhaseChanged, this, &TabGame::setActivePhase);
    connect(game->getGameState(), &GameState::activePlayerChanged, this, &TabGame::setActivePlayer);
    connect(game->getGameState(), &GameState::priorityPlayerChanged, this, &TabGame::setPriorityPlayer);
}

void TabGame::connectToPlayerManager()
{
    connect(game->getPlayerManager(), &PlayerManager::playerAdded, this, &TabGame::addPlayer);
    connect(game->getPlayerManager(), &PlayerManager::playerRemoved, this, &TabGame::processPlayerLeave);
    // update menu text when player concedes so that "concede" gets updated to "unconcede"
    connect(game->getPlayerManager(), &PlayerManager::playerConceded, this, &TabGame::retranslateUi);
    connect(game->getPlayerManager(), &PlayerManager::playerUnconceded, this, &TabGame::retranslateUi);
}

void TabGame::connectToGameEventHandler()
{
    connect(this, &TabGame::gameLeft, game->getGameEventHandler(), &GameEventHandler::handleGameLeft);
    connect(game->getGameEventHandler(), &GameEventHandler::emitUserEvent, this, &TabGame::emitUserEvent);
    connect(game->getGameEventHandler(), &GameEventHandler::gameStopped, this, &TabGame::stopGame);
    connect(game->getGameEventHandler(), &GameEventHandler::gameStopped, messageLog, &MessageLogWidget::prepareForNewGame);
    connect(game->getGameEventHandler()->ruled(), &RuledClientState::sessionReset, this, [this] {
        if (gamePromptWidget) {
            gamePromptWidget->setRuledStackHasItems(false);
            gamePromptWidget->setSpellCastPending(false);
            // Re-derive rather than blank: on the game-start transition the view model has
            // deliberately kept the incoming session's opening prompt, and blanking here would
            // strand it (the engine is blocked on ChooseStartingPlayer and never re-sends it).
            refreshRuledPromptState();
        }
    });
    connect(game->getGameEventHandler(), &GameEventHandler::gameClosed, this, &TabGame::closeGame);
    connect(game->getGameEventHandler(), &GameEventHandler::localPlayerReadyStateChanged, this,
            &TabGame::processLocalPlayerReadyStateChanged);
    connect(game->getGameEventHandler(), &GameEventHandler::localPlayerSideboardLocked, this,
            &TabGame::processLocalPlayerSideboardLocked);
    connect(game->getGameEventHandler(), &GameEventHandler::localPlayerDeckSelected, this,
            &TabGame::processLocalPlayerDeckSelect);
    connect(game->getGameEventHandler(), &GameEventHandler::remotePlayerDeckSelected, this,
            &TabGame::processRemotePlayerDeckSelect);
    connect(game->getGameEventHandler(), &GameEventHandler::remotePlayersDecksSelected, this,
            &TabGame::processMultipleRemotePlayerDeckSelect);
    connect(game->getGameEventHandler()->ruled(), &RuledClientState::combatStateChanged, this,
            &TabGame::refreshRuledCombatArrows);
    connect(game->getGameEventHandler()->ruled(), &RuledClientState::battlefieldMapUpdated, this,
            &TabGame::refreshRuledCombatArrows);
    connect(game->getGameEventHandler()->ruled(), &RuledClientState::stackHasItemsChanged, this,
            [this](bool /*hasItems*/) {
                if (RuledActions::isRuledGame(game)) {
                    syncStackWindowVisibility();
                }
            });
    connect(game->getGameEventHandler()->ruled(), &RuledClientState::stackOrderChanged, this,
            [this](const QList<quint32> &) {
                // Re-sort the open stack window. Event_MoveCard (which adds the physical card and
                // fires reorganizeCards) arrives before Event_RuledPayload (which updates
                // ruledStackOidOrder), so the first reorganize runs before the OID is registered.
                // This signal fires after all OID updates and triggers the corrective re-sort.
                if (stackView) {
                    if (ZoneViewZone *zv = stackView->getZone()) {
                        zv->reorganizeCards();
                    }
                }
            });
    connect(game->getGameEventHandler()->ruled(), &RuledClientState::triggerGraveyardNeedsTarget, this,
            [this](bool needed) {
                if (!game || !scene) {
                    return;
                }
                const int localId = game->getPlayerManager()->getLocalPlayerId();
                Player *localPlayer = game->getPlayerManager()->getPlayer(localId);
                if (!localPlayer) {
                    return;
                }
                const QString graveName = QStringLiteral("grave");
                if (needed) {
                    // Leave a graveyard the player opened themselves alone — we only ever tidy up
                    // after ourselves.
                    if (!scene->isZoneViewOpen(localPlayer, graveName)) {
                        scene->toggleZoneView(localPlayer, graveName, -1);
                        ruledAutoOpenedGraveyardView = scene->zoneViewWidgetFor(localPlayer, graveName);
                    }
                } else if (ruledAutoOpenedGraveyardView) {
                    // Close only the exact view we opened. A bare "did we open one?" flag is not
                    // enough: the player may have closed ours and opened their own while the
                    // trigger was still pending, and that one is theirs to keep.
                    if (scene->zoneViewWidgetFor(localPlayer, graveName) == ruledAutoOpenedGraveyardView) {
                        ruledAutoOpenedGraveyardView->close();
                    }
                    ruledAutoOpenedGraveyardView = nullptr;
                }
            });
    if (gamePromptWidget) {
        connect(game->getGameEventHandler()->ruled(), &RuledClientState::blockerRejected, gamePromptWidget,
                [this]() {
                    if (gamePromptWidget) {
                        gamePromptWidget->setStickyBlockerError(tr("Illegal blocks."));
                    }
                });
        connect(game->getGameEventHandler()->ruled(), &RuledClientState::enginePromptFeed, gamePromptWidget,
                [this](const QString & /*lines*/) {
                    // Recompute the whole prompt mode once per batch, then let the local player's
                    // pending cast / activation overwrite the line if nothing else claimed it.
                    if (refreshRuledPromptState() != GamePromptWidget::PromptMode::Normal) {
                        return;
                    }
                    const int localId = game->getPlayerManager()->getLocalPlayerId();
                    Player *localPlayer = game->getPlayerManager()->getPlayers().value(localId, nullptr);
                    if (localPlayer && localPlayer->getPlayerActions()) {
                        const QString spellPrompt = localPlayer->getPlayerActions()->pendingRuledSpellPromptText();
                        if (!spellPrompt.isEmpty()) {
                            gamePromptWidget->setPromptText(spellPrompt);
                            return;
                        }
                        const QString abilityPrompt = localPlayer->getPlayerActions()->pendingRuledAbilityPromptText();
                        if (!abilityPrompt.isEmpty()) {
                            gamePromptWidget->setPromptText(abilityPrompt);
                            return;
                        }
                    }
                    // Refresh after the full batch has settled (state is complete here).
                    gamePromptWidget->refreshPromptLabel();
                });
        connect(game->getGameEventHandler()->ruled(), &RuledClientState::cleanupDiscardUiChanged, this,
                [this](int required, int selected) {
                    if (!gamePromptWidget) {
                        return;
                    }
                    refreshRuledPromptState();
                    if (required > 0 && selected == required) {
                        const int localId = game->getPlayerManager()->getLocalPlayerId();
                        Player *localPlayer = game->getPlayerManager()->getPlayers().value(localId, nullptr);
                        if (localPlayer && localPlayer->getPlayerActions()) {
                            localPlayer->getPlayerActions()->sendRuledCleanupDiscardBatchIfComplete();
                        }
                    }
                });
        connect(game->getGameEventHandler()->ruled(), &RuledClientState::openingUiChanged, this,
                [this]() { refreshRuledPromptState(); });
        connect(gamePromptWidget, &GamePromptWidget::ruledOpeningPickSeatRequested, game->getGameEventHandler()->ruled(),
                &RuledClientState::openingPickFirstSeat);
        connect(gamePromptWidget, &GamePromptWidget::ruledOpeningMulliganKeepRequested, game->getGameEventHandler()->ruled(),
                &RuledClientState::openingMulliganKeep);
        connect(gamePromptWidget, &GamePromptWidget::ruledOpeningMulliganRedrawRequested, game->getGameEventHandler()->ruled(),
                &RuledClientState::openingMulliganRedraw);
        connect(gamePromptWidget, &GamePromptWidget::ruledOpeningBottomCancelRequested, game->getGameEventHandler()->ruled(),
                &RuledClientState::openingBottomCancel);
        connect(gamePromptWidget, &GamePromptWidget::ruledOpeningBottomDoneRequested, game->getGameEventHandler()->ruled(),
                &RuledClientState::openingBottomDone);
        connect(game->getGameEventHandler()->ruled(), &RuledClientState::openingBottomUiChanged, this,
                [this](int, int) { refreshRuledPromptState(); });
        connect(game->getGameEventHandler()->ruled(), &RuledClientState::resolutionHandPickUiChanged, this,
                [this](int required, int /*selected*/) {
                    refreshRuledPromptState();
                    // When library-search pick ends (required < 0 = cleared), close the deck zone view.
                    if (required < 0 && librarySearchView) {
                        librarySearchView->close();
                        librarySearchView = nullptr;
                    }
                });
        connect(gamePromptWidget, &GamePromptWidget::ruledResolutionHandPickConfirmRequested,
                game->getGameEventHandler()->ruled(), &RuledClientState::submitResolutionHandPick);
        connect(game->getGameEventHandler()->ruled(), &RuledClientState::librarySearchPickStarted,
                this, &TabGame::onRuledLibrarySearchPickStarted);
        connect(game->getGameEventHandler()->ruled(), &RuledClientState::revealedPickChanged,
                this, &TabGame::onRuledRevealedPickChanged);
        connect(game->getGameState(), &GameState::activePhaseChanged, gamePromptWidget, &GamePromptWidget::setActivePhase);
        connect(game->getGameEventHandler(), &GameEventHandler::logActivePlayer, gamePromptWidget, [this](Player *player) {
            if (player) {
                gamePromptWidget->setActivePlayerName(player->getPlayerInfo()->getName());
            }
        });
        connect(game->getGameEventHandler()->ruled(), &RuledClientState::combatStateChanged, gamePromptWidget,
                [this]() {
                    auto *handler = game->getGameEventHandler()->ruled();
                    if (!handler || !gamePromptWidget) {
                        return;
                    }
                    const auto phase = handler->getCombatPhase();
                    using Phase = RuledClientState::RuledCombatPhase;
                    GamePromptWidget::CombatMode mode = GamePromptWidget::CombatMode::None;
                    bool localHasButtons = false;
                    if (phase == Phase::DeclareAttackers) {
                        mode = GamePromptWidget::CombatMode::DeclareAttackers;
                        localHasButtons = handler->localPlayerIsActive();
                    } else if (phase == Phase::DeclareBlockers) {
                        mode = GamePromptWidget::CombatMode::DeclareBlockers;
                        localHasButtons = handler->localPlayerIsDefender();
                    } else if (phase == Phase::AssignCombatDamage) {
                        mode = GamePromptWidget::CombatMode::AssignCombatDamage;
                        localHasButtons = handler->localPlayerIsActive();
                    }
                    // CR 508.1d / 509.1c: disable the confirm (OK) button while a required
                    // attacker/blocker is still unstaged, so an illegal declaration can't be sent.
                    const bool declarationSatisfied = handler->combatDeclarationSatisfied();
                    gamePromptWidget->setCombatMode(mode, localHasButtons, declarationSatisfied);
                    if (!RuledActions::isRuledGame(game)) {
                        return;
                    }
                    if (phase == Phase::AssignCombatDamage) {
                        if (handler->localPlayerIsActive()) {
                            gamePromptWidget->setCombatDamageStatus(
                                handler->currentCombatDamageAttackerDisplayName(),
                                handler->localCombatDamageAssignedTotal(),
                                handler->currentCombatDamageAttackerPower(),
                                handler->localCombatDamagePlayerDamage(),
                                handler->localCombatDamageAssignmentLegal());
                        } else {
                            gamePromptWidget->setPromptText(
                                tr("Wait — your opponent is assigning combat damage."));
                        }
                    }
                });
        connect(game->getGameEventHandler()->ruled(), &RuledClientState::combatDamageUiChanged, this, [this]() {
            auto *handler = game->getGameEventHandler()->ruled();
            if (!handler || !gamePromptWidget || !RuledActions::isRuledGame(game)) {
                return;
            }
            if (handler->getCombatPhase() != RuledClientState::RuledCombatPhase::AssignCombatDamage ||
                !handler->localPlayerIsActive()) {
                return;
            }
            gamePromptWidget->setCombatDamageStatus(handler->currentCombatDamageAttackerDisplayName(),
                                                    handler->localCombatDamageAssignedTotal(),
                                                    handler->currentCombatDamageAttackerPower(),
                                                    handler->localCombatDamagePlayerDamage(),
                                                    handler->localCombatDamageAssignmentLegal());
        });
        connect(game->getGameEventHandler()->ruled(), &RuledClientState::spellDamageAllocationUiChanged, this, [this]() {
            if (!gamePromptWidget || !RuledActions::isRuledGame(game)) return;
            const int localId = game->getPlayerManager()->getLocalPlayerId();
            Player *local = game->getPlayerManager()->getPlayers().value(localId, nullptr);
            auto *actions = local ? local->getPlayerActions() : nullptr;
            if (!actions) return;
            const bool active = actions->isInSpellDamageAllocationMode();
            gamePromptWidget->setSpellDamageAllocationStatus(
                active,
                active ? actions->spellDamageAllocationAssignedTotal() : 0,
                active ? actions->spellDamageAllocationMaxTotal() : 0);
        });
        connect(gamePromptWidget, &GamePromptWidget::confirmAttackersRequested, game->getGameEventHandler()->ruled(),
                &RuledClientState::confirmAttackers);
        connect(gamePromptWidget, &GamePromptWidget::confirmBlockersRequested, game->getGameEventHandler()->ruled(),
                &RuledClientState::confirmBlockers);
        connect(gamePromptWidget, &GamePromptWidget::resetBlockersRequested, game->getGameEventHandler()->ruled(),
                &RuledClientState::clearPendingBlocks);
        connect(gamePromptWidget, &GamePromptWidget::confirmCombatDamageRequested, game->getGameEventHandler()->ruled(),
                &RuledClientState::confirmCombatDamageForCurrentAttacker);
        connect(gamePromptWidget, &GamePromptWidget::cancelTargetingRequested, this, [this]() {
            if (!game) {
                return;
            }
            const int localPlayerId = game->getPlayerManager()->getLocalPlayerId();
            Player *localPlayer = game->getPlayerManager()->getPlayers().value(localPlayerId, nullptr);
            if (!localPlayer || !localPlayer->getPlayerActions()) {
                return;
            }
            localPlayer->getPlayerActions()->cancelPendingRuledSpellCast();
            localPlayer->getPlayerActions()->cancelPendingActivatedAbility();
        });
        connect(game->getGameEventHandler()->ruled(), &RuledClientState::stackHasItemsChanged, gamePromptWidget,
                &GamePromptWidget::setRuledStackHasItems);
        gamePromptWidget->setRuledStackHasItems(game->getGameEventHandler()->ruled()->hasStackItems());
        connect(game->getGameEventHandler()->ruled(), &RuledClientState::firstStrikeStepPendingChanged,
                gamePromptWidget, &GamePromptWidget::setFirstStrikeStepPending);
        gamePromptWidget->setFirstStrikeStepPending(
            game->getGameEventHandler()->ruled()->isFirstStrikeStepPending());
        connect(game->getGameEventHandler()->ruled(), &RuledClientState::firstStrikeDamageStepActiveChanged,
                gamePromptWidget, &GamePromptWidget::setFirstStrikeDamageStepActive);
        gamePromptWidget->setFirstStrikeDamageStepActive(
            game->getGameEventHandler()->ruled()->inFirstStrikeDamageStep());
    }
}

void TabGame::connectMessageLogToGameEventHandler()
{
    connect(game->getGameEventHandler(), &GameEventHandler::gameFlooded, messageLog, &MessageLogWidget::logGameFlooded);
    connect(game->getGameEventHandler(), &GameEventHandler::containerProcessingStarted, messageLog,
            &MessageLogWidget::containerProcessingStarted);
    connect(game->getGameEventHandler(), &GameEventHandler::containerProcessingDone, messageLog,
            &MessageLogWidget::containerProcessingDone);
    connect(game->getGameEventHandler(), &GameEventHandler::setContextJudgeName, messageLog,
            &MessageLogWidget::setContextJudgeName);
    connect(game->getGameEventHandler(), &GameEventHandler::logSpectatorSay, messageLog,
            &MessageLogWidget::logSpectatorSay);

    connect(game->getGameEventHandler(), &GameEventHandler::logJoinPlayer, messageLog, &MessageLogWidget::logJoin);
    connect(game->getGameEventHandler(), &GameEventHandler::logJoinSpectator, messageLog,
            &MessageLogWidget::logJoinSpectator);
    connect(game->getGameEventHandler(), &GameEventHandler::logLeave, messageLog, &MessageLogWidget::logLeave);
    connect(game->getGameEventHandler(), &GameEventHandler::logKicked, messageLog, &MessageLogWidget::logKicked);
    connect(game->getGameEventHandler(), &GameEventHandler::logConnectionStateChanged, messageLog,
            &MessageLogWidget::logConnectionStateChanged);

    connect(game->getGameEventHandler(), &GameEventHandler::logDeckSelect, messageLog,
            &MessageLogWidget::logDeckSelect);
    connect(game->getGameEventHandler(), &GameEventHandler::logSideboardLockSet, messageLog,
            &MessageLogWidget::logSetSideboardLock);
    connect(game->getGameEventHandler(), &GameEventHandler::logReadyStart, messageLog,
            &MessageLogWidget::logReadyStart);
    connect(game->getGameEventHandler(), &GameEventHandler::logNotReadyStart, messageLog,
            &MessageLogWidget::logNotReadyStart);
    connect(game->getGameEventHandler(), &GameEventHandler::logGameStart, messageLog, &MessageLogWidget::logGameStart);

    connect(game->getGameEventHandler(), &GameEventHandler::logActivePlayer, messageLog,
            &MessageLogWidget::logSetActivePlayer);
    connect(game->getGameEventHandler(), &GameEventHandler::logActivePhaseChanged, messageLog,
            &MessageLogWidget::logSetActivePhase);

    connect(game->getGameEventHandler(), &GameEventHandler::logTurnReversed, messageLog,
            &MessageLogWidget::logReverseTurn);

    connect(game->getGameEventHandler(), &GameEventHandler::logConcede, messageLog, &MessageLogWidget::logConcede);
    connect(game->getGameEventHandler(), &GameEventHandler::logUnconcede, messageLog, &MessageLogWidget::logUnconcede);

    connect(game->getGameEventHandler()->ruled(), &RuledClientState::engineTimeline, messageLog,
            &MessageLogWidget::logRuledGameplay);
    connect(game->getGameEventHandler(), &GameEventHandler::logGameClosed, messageLog,
            &MessageLogWidget::logGameClosed);
}

void TabGame::connectPlayerListToGameEventHandler()
{
    connect(game->getGameEventHandler(), &GameEventHandler::playerJoined, playerListWidget,
            &PlayerListWidget::addPlayer);
    connect(game->getGameEventHandler(), &GameEventHandler::playerLeft, playerListWidget,
            &PlayerListWidget::removePlayer);
    connect(game->getGameEventHandler(), &GameEventHandler::spectatorJoined, playerListWidget,
            &PlayerListWidget::addPlayer);
    connect(game->getGameEventHandler(), &GameEventHandler::spectatorLeft, playerListWidget,
            &PlayerListWidget::removePlayer);
    connect(game->getGameEventHandler(), &GameEventHandler::playerPropertiesChanged, playerListWidget,
            &PlayerListWidget::updatePlayerProperties);
}

void TabGame::addMentionTag(const QString &value)
{
    sayEdit->insert(value + " ");
    sayEdit->setFocus();
}

void TabGame::linkCardToChat(const QString &cardName)
{
    sayEdit->insert("[[" + cardName + "]] ");
    sayEdit->setFocus();
}

void TabGame::resetChatAndPhase()
{
    // reset chat log
    messageLog->clearChat();

    // reset phase markers
    game->getGameState()->setCurrentPhase(-1);
}

void TabGame::emitUserEvent()
{
    bool globalEvent =
        !game->getPlayerManager()->isSpectator() || SettingsCache::instance().getSpectatorNotificationsEnabled();
    emit userEvent(globalEvent);
    updatePlayerListDockTitle();
}

TabGame::~TabGame()
{
    clearRuledCombatArrows();
    if (replayManager) {
        delete replayManager->replay;
    }
    for (auto &player : game->getPlayerManager()->getPlayers()) {
        player->clear();
    }
}

GamePromptWidget::PromptMode TabGame::refreshRuledPromptState()
{
    using PromptMode = GamePromptWidget::PromptMode;
    GamePromptWidget::RuledPromptState state;
    if (!gamePromptWidget || !game) {
        return state.mode;
    }
    RuledClientState *h = game->getGameEventHandler()->ruled();
    if (!h) {
        gamePromptWidget->setRuledPromptState(state);
        return state.mode;
    }

    // Priority order, highest first. A parked resolution pick outranks everything (the engine is
    // blocked on it), then the pre-game opening sequence, then the cleanup discard, then a parked
    // click-a-permanent choice. Anything below that is the ordinary priority prompt.
    using ChoiceKind = RuledClientState::ChoiceKind;
    using OpeningKind = RuledClientState::RuledOpeningUiKind;
    const OpeningKind opening = h->getOpeningUiKind();
    if (h->isResolutionHandPickActive()) {
        state.mode = PromptMode::ResolutionPick;
        state.required = h->resolutionHandPickRequired();
        state.selected = h->resolutionHandPickSelected();
        state.text = h->resolutionHandPickPromptText();
    } else if (opening == OpeningKind::ChooseFirst) {
        const int localId = game->getPlayerManager()->getLocalPlayerId();
        int opponentId = -1;
        for (int pid : game->getPlayerManager()->getPlayers().keys()) {
            if (pid != localId) {
                opponentId = pid;
                break;
            }
        }
        if (opponentId >= 0) {
            state.mode = PromptMode::OpeningChooseFirst;
            state.openingPickSeatIds = {localId, opponentId};
        }
    } else if (opening == OpeningKind::MulliganChoice) {
        state.mode = PromptMode::OpeningMulligan;
        state.required = h->getOpeningMulliganCount();
    } else if (opening == OpeningKind::BottomLibrary) {
        state.mode = PromptMode::OpeningBottom;
        state.required = h->openingBottomRequiredCount();
        state.selected = h->openingBottomSelectedCount();
    } else if (h->localPlayerMustCleanupDiscard()) {
        state.mode = PromptMode::CleanupDiscard;
        state.required = h->cleanupDiscardRequiredCount();
        state.selected = h->cleanupDiscardSelectedCount();
    } else if (h->hasPendingChoiceOfKind(ChoiceKind::CopyTarget)) {
        // CR 707.10c: a spell copy is waiting for the local player to choose new targets.
        state.mode = PromptMode::ClickChoice;
        state.text = h->pendingChoicePromptText(ChoiceKind::CopyTarget) +
                     tr("\nClick a target, or click the original target to keep it.");
    } else if (h->hasPendingChoiceOfKind(ChoiceKind::LegendKeep)) {
        // CR 704.5j: which duplicate legend to keep, chosen on the battlefield.
        state.mode = PromptMode::ClickChoice;
        state.text = h->pendingChoicePromptText(ChoiceKind::LegendKeep) +
                     tr("\nClick the permanent to keep on the battlefield.");
    } else if (h->hasPendingTriggerTarget()) {
        state.mode = PromptMode::ClickChoice;
        state.text = tr("Choose a target for: %1").arg(h->pendingTriggerText());
    } else if (h->engineOpeningPhaseActive()) {
        // Opening phase with nothing for us to do — say who we are waiting for. Runs
        // unconditionally: activePlayerName may already be set from a prior logActivePlayer
        // signal, so checking isEmpty() would miss later rounds.
        const int localId = game->getPlayerManager()->getLocalPlayerId();
        for (auto *player : game->getPlayerManager()->getPlayers()) {
            if (player->getPlayerInfo()->getId() != localId) {
                state.text = tr("Waiting for %1...").arg(player->getPlayerInfo()->getName());
                break;
            }
        }
    }
    gamePromptWidget->setRuledPromptState(state);
    return state.mode;
}

void TabGame::clearRuledCombatArrows()
{
    const QList<QPointer<ArrowItem>> arrows = ruledCombatArrows;
    ruledCombatArrows.clear();
    for (const QPointer<ArrowItem> &arrow : arrows) {
        if (arrow) {
            arrow->delArrow();
        }
    }
}

void TabGame::refreshRuledCombatArrows()
{
    clearRuledCombatArrows();

    if (!game || !RuledActions::isRuledGame(game)) {
        return;
    }

    const RuledClientState *handler = game->getGameEventHandler()->ruled();
    if (!handler) {
        return;
    }

    const auto phase = handler->getCombatPhase();
    if (phase == RuledClientState::RuledCombatPhase::None) {
        return;
    }

    const int localPlayerId = game->getPlayerManager()->getLocalPlayerId();
    Player *arrowOwner = game->getPlayerManager()->getPlayers().value(localPlayerId, nullptr);
    if (!arrowOwner) {
        return;
    }

    QHash<quint32, quint32> blocksToDraw = handler->getCommittedBlocks();
    const QHash<quint32, quint32> &pendingBlocks = handler->getPendingBlocks();
    for (auto it = pendingBlocks.constBegin(); it != pendingBlocks.constEnd(); ++it) {
        blocksToDraw.insert(it.key(), it.value());
    }
    const QHash<quint32, quint32> &remotePreview = handler->getRemoteBlockPreviewPairs();
    for (auto it = remotePreview.constBegin(); it != remotePreview.constEnd(); ++it) {
        blocksToDraw.insert(it.key(), it.value());
    }

    for (auto it = blocksToDraw.constBegin(); it != blocksToDraw.constEnd(); ++it) {
        CardItem *blockerCard = findTableCardForEngineOid(game, handler, it.key());
        CardItem *attackerCard = findTableCardForEngineOid(game, handler, it.value());
        if (!blockerCard || !attackerCard) {
            continue;
        }

        auto *arrow = new RuledCombatArrowItem(arrowOwner, blockerCard, attackerCard, QColor(Qt::blue));
        ruledCombatArrows.append(arrow);
        scene->addItem(arrow);
    }

    QSet<quint32> attackersToDraw = handler->getCurrentAttackerOids();
    const QSet<quint32> &pendingAttackers = handler->getPendingAttackerOids();
    for (const quint32 oid : pendingAttackers) {
        attackersToDraw.insert(oid);
    }
    const QSet<quint32> &remoteAtk = handler->getRemoteAttackerPreviewOids();
    for (const quint32 oid : remoteAtk) {
        attackersToDraw.insert(oid);
    }

    ArrowTarget *defendingPlayerTarget = findDefendingPlayerTarget(game, handler->getActivePlayerId());
    if (!defendingPlayerTarget) {
        return;
    }

    for (const quint32 attackerOid : attackersToDraw) {
        CardItem *attackerCard = findTableCardForEngineOid(game, handler, attackerOid);
        if (!attackerCard) {
            continue;
        }

        auto *arrow =
            new RuledCombatArrowItem(arrowOwner, attackerCard, defendingPlayerTarget, QColor(Qt::red));
        ruledCombatArrows.append(arrow);
        scene->addItem(arrow);
    }
}

void TabGame::updatePlayerListDockTitle()
{
    QString type = replayDock ? tr("Replay") : tr("Game");
    QString tabText = " | " + type + " #" + QString::number(game->getGameMetaInfo()->gameId());
    QString userCountInfo =
        QString(" %1/%2").arg(game->getPlayerManager()->getPlayerCount()).arg(game->getGameMetaInfo()->maxPlayers());
    playerListDock->setWindowTitle(tr("Player List") + userCountInfo +
                                   (playerListDock->isWindow() ? tabText : QString()));
}

void TabGame::retranslateUi()
{
    QString type = replayDock ? tr("Replay") : tr("Game");
    QString tabText = " | " + type + " #" + QString::number(game->getGameMetaInfo()->gameId());

    updatePlayerListDockTitle();
    cardInfoDock->setWindowTitle(tr("Card Info") + (cardInfoDock->isWindow() ? tabText : QString()));
    messageLayoutDock->setWindowTitle(tr("Messages") + (messageLayoutDock->isWindow() ? tabText : QString()));
    if (replayDock)
        replayDock->setWindowTitle(tr("Replay Timeline") + (replayDock->isWindow() ? tabText : QString()));

    if (phasesMenu) {
        for (int i = 0; i < phaseActions.size(); ++i)
            phaseActions[i]->setText(phasesToolbar->getLongPhaseName(i));
        phasesMenu->setTitle(tr("&Phases"));
    }

    gameMenu->setTitle(tr("&Game"));
    if (aNextPhase) {
        aNextPhase->setText(tr("Next &phase"));
    }
    if (aNextPhaseAction) {
        aNextPhaseAction->setText(tr("Next phase with &action"));
    }
    if (aNextTurn) {
        aNextTurn->setText(tr("Next &turn"));
    }
    if (aReverseTurn) {
        aReverseTurn->setText(tr("Reverse turn order"));
    }
    if (aRemoveLocalArrows) {
        aRemoveLocalArrows->setText(tr("&Remove all local arrows"));
    }
    if (aRotateViewCW) {
        aRotateViewCW->setText(tr("Rotate View Cl&ockwise"));
    }
    if (aRotateViewCCW) {
        aRotateViewCCW->setText(tr("Rotate View Co&unterclockwise"));
    }
    if (aGameInfo)
        aGameInfo->setText(tr("Game &information"));
    if (aConcede) {
        if (game->getPlayerManager()->isMainPlayerConceded()) {
            aConcede->setText(tr("Un&concede"));
        } else {
            aConcede->setText(tr("&Concede"));
        }
    }
    if (aLeaveGame) {
        if (replayDock) {
            aLeaveGame->setText(tr("C&lose replay"));
        } else {
            aLeaveGame->setText(tr("&Leave game"));
        }
    }
    if (aFocusChat) {
        aFocusChat->setText(tr("&Focus Chat"));
    }
    if (sayLabel) {
        sayLabel->setText(tr("&Say:"));
    }

    if (aCardMenu) {
        aCardMenu->setText(tr("Selected cards"));
    }

    viewMenu->setTitle(tr("&View"));
    if (aToggleStackWindow) {
        aToggleStackWindow->setText(tr("Stack &window"));
    }

    dockToActions[cardInfoDock].menu->setTitle(tr("Card Info"));
    dockToActions[messageLayoutDock].menu->setTitle(tr("Messages"));
    dockToActions[playerListDock].menu->setTitle(tr("Player List"));

    if (replayDock) {
        dockToActions[replayDock].menu->setTitle(tr("Replay Timeline"));
    }

    for (auto &actions : dockToActions.values()) {
        actions.aVisible->setText(tr("Visible"));
        actions.aFloating->setText(tr("Floating"));
    }

    aResetLayout->setText(tr("Reset layout"));

    cardInfoFrameWidget->retranslateUi();
    if (gamePromptWidget) {
        gamePromptWidget->retranslateUi();
    }

    QMapIterator<int, Player *> i(game->getPlayerManager()->getPlayers());

    while (i.hasNext())
        i.next().value()->getGraphicsItem()->retranslateUi();
    QMapIterator<int, TabbedDeckViewContainer *> j(deckViewContainers);
    while (j.hasNext())
        j.next().value()->playerDeckView->retranslateUi();

    scene->retranslateUi();
}

void TabGame::refreshShortcuts()
{
    ShortcutsSettings &shortcuts = SettingsCache::instance().shortcuts();
    for (int i = 0; i < phaseActions.size(); ++i) {
        QAction *temp = phaseActions.at(i);
        switch (i) {
            case 0:
                temp->setShortcuts(shortcuts.getShortcut("Player/phase0"));
                break;
            case 1:
                temp->setShortcuts(shortcuts.getShortcut("Player/phase1"));
                break;
            case 2:
                temp->setShortcuts(shortcuts.getShortcut("Player/phase2"));
                break;
            case 3:
                temp->setShortcuts(shortcuts.getShortcut("Player/phase3"));
                break;
            case 4:
                temp->setShortcuts(shortcuts.getShortcut("Player/phase4"));
                break;
            case 5:
                temp->setShortcuts(shortcuts.getShortcut("Player/phase5"));
                break;
            case 6:
                temp->setShortcuts(shortcuts.getShortcut("Player/phase6"));
                break;
            case 7:
                temp->setShortcuts(shortcuts.getShortcut("Player/phase7"));
                break;
            case 8:
                temp->setShortcuts(shortcuts.getShortcut("Player/phase8"));
                break;
            case 9:
                temp->setShortcuts(shortcuts.getShortcut("Player/phase9"));
                break;
            case 10:
                temp->setShortcuts(shortcuts.getShortcut("Player/phase10"));
                break;
            default:;
        }
    }

    if (aNextPhase) {
        aNextPhase->setShortcuts(shortcuts.getShortcut("Player/aNextPhase"));
    }
    if (aNextPhaseAction) {
        aNextPhaseAction->setShortcuts(shortcuts.getShortcut("Player/aNextPhaseAction"));
    }
    if (aNextTurn) {
        aNextTurn->setShortcuts(shortcuts.getShortcut("Player/aNextTurn"));
    }
    if (aReverseTurn) {
        aReverseTurn->setShortcuts(shortcuts.getShortcut("Player/aReverseTurn"));
    }
    if (aRemoveLocalArrows) {
        aRemoveLocalArrows->setShortcuts(shortcuts.getShortcut("Player/aRemoveLocalArrows"));
    }
    if (aRotateViewCW) {
        aRotateViewCW->setShortcuts(shortcuts.getShortcut("Player/aRotateViewCW"));
    }
    if (aRotateViewCCW) {
        aRotateViewCCW->setShortcuts(shortcuts.getShortcut("Player/aRotateViewCCW"));
    }
    if (aConcede) {
        aConcede->setShortcuts(shortcuts.getShortcut("Player/aConcede"));
    }
    if (aLeaveGame) {
        aLeaveGame->setShortcuts(shortcuts.getShortcut("Player/aLeaveGame"));
    }
    if (aResetLayout) {
        aResetLayout->setShortcuts(shortcuts.getShortcut("Player/aResetLayout"));
    }
    if (aFocusChat) {
        aFocusChat->setShortcuts(shortcuts.getShortcut("Player/aFocusChat"));
    }
}

bool TabGame::closeRequest()
{
    if (!leaveGame()) {
        return false;
    }

    return close();
}

void TabGame::closeEvent(QCloseEvent *event)
{
    emit gameClosing(this);
    event->accept();
}

void TabGame::updateTimeElapsedLabel(const QString newTime)
{
    timeElapsedLabel->setText(newTime);
}

void TabGame::adminLockChanged(bool lock)
{
    bool v = !(game->getPlayerManager()->isSpectator() && !game->getGameMetaInfo()->spectatorsCanChat() && lock);
    sayLabel->setVisible(v);
    sayEdit->setVisible(v);
}

void TabGame::actGameInfo()
{
    DlgCreateGame dlg(game->getGameMetaInfo()->proto(), game->getGameMetaInfo()->getRoomGameTypes(), this);
    dlg.exec();
}

void TabGame::actConcede()
{
    Player *player = game->getPlayerManager()->getActiveLocalPlayer(game->getGameState()->getActivePlayer());
    if (player == nullptr)
        return;
    if (!player->getConceded()) {
        if (QMessageBox::question(this, tr("Concede"), tr("Are you sure you want to concede this game?"),
                                  QMessageBox::Yes | QMessageBox::No, QMessageBox::No) != QMessageBox::Yes)
            return;
        emit game->getPlayerManager()->activeLocalPlayerConceded();
    } else {
        if (QMessageBox::question(this, tr("Unconcede"),
                                  tr("You have already conceded.  Do you want to return to this game?"),
                                  QMessageBox::Yes | QMessageBox::No, QMessageBox::No) != QMessageBox::Yes)
            return;
        emit game->getPlayerManager()->activeLocalPlayerUnconceded();
    }
}

/**
 * Confirms the leave game and sends the leave game command, if applicable.
 *
 * @return True if the leave game is confirmed
 */
bool TabGame::leaveGame()
{
    if (!game->getGameState()->isGameClosed()) {
        if (!game->getPlayerManager()->isSpectator()) {
            tabSupervisor->setCurrentWidget(this);
            if (QMessageBox::question(this, tr("Leave game"), tr("Are you sure you want to leave this game?"),
                                      QMessageBox::Yes | QMessageBox::No, QMessageBox::No) != QMessageBox::Yes)
                return false;
        }

        if (!replayDock)
            emit gameLeft();
    }
    return true;
}

void TabGame::actSay()
{
    if (completer->popup()->isVisible())
        return;

    if (sayEdit->text().startsWith("/card ")) {
        cardInfoFrameWidget->setCard(sayEdit->text().mid(6));
        sayEdit->clear();
        return;
    }

    if (!sayEdit->text().isEmpty()) {
        emit chatMessageSent(sayEdit->text());
        sayEdit->clear();
    }
}

// Fork: one dev-console line. Parse it, send it, or report why not. Kept short on purpose —
// anything more than transport belongs in RuledDevCommandParser, which is testable headless.
void TabGame::actDevConsoleCommand(const QString &line)
{
    if (!devConsoleWidget) {
        return;
    }
    // Seats ascending; QMap is key-ordered, so ordinal 1 is the lowest player id.
    //
    // keys() returns a list *by value*. It must be materialised into a named variable before
    // taking iterators from it: calling keys().begin() and keys().end() would produce two
    // separate temporaries, both dead by the end of the statement, leaving the iterators dangling.
    const QList<int> seatKeys = game->getPlayerManager()->getPlayers().keys();
    const QVector<int> seatIds(seatKeys.cbegin(), seatKeys.cend());
    const RuledDevCommandParser::Result parsed =
        RuledDevCommandParser::parse(line, game->getPlayerManager()->getLocalPlayerId(), seatIds);

    if (parsed.handledLocally) {
        devConsoleWidget->setStatus(parsed.message, false);
        return;
    }
    if (!parsed.ok) {
        devConsoleWidget->setStatus(parsed.error, true);
        return;
    }
    // The engine has the final say and reports rejections through the game log, which is right
    // above the console — so there is nothing useful to echo here on success.
    RuledActions::sendRuledCommand(game, parsed.command);
}

void TabGame::addPlayerToAutoCompleteList(QString playerName)
{
    if (sayEdit && !autocompleteUserList.contains(playerName)) {
        autocompleteUserList << playerName;
        sayEdit->setCompletionList(autocompleteUserList);
    }
}

void TabGame::removePlayerFromAutoCompleteList(QString playerName)
{
    if (sayEdit && autocompleteUserList.removeOne(playerName)) {
        sayEdit->setCompletionList(autocompleteUserList);
    }
}

void TabGame::removeSpectator(int spectatorId, ServerInfo_User spectator)
{
    Q_UNUSED(spectator);
    QString playerName = "@" + game->getPlayerManager()->getSpectatorName(spectatorId);
    removePlayerFromAutoCompleteList(playerName);
}

void TabGame::actPhaseAction()
{
    int phase = phaseActions.indexOf(static_cast<QAction *>(sender()));
    emit phaseChanged(phase);
}

void TabGame::actNextPhase()
{
    int phase = game->getGameState()->getCurrentPhase();
    if (++phase >= phasesToolbar->phaseCount())
        phase = 0;

    emit phaseChanged(phase);
}

void TabGame::actNextPhaseAction()
{
    int phase = game->getGameState()->getCurrentPhase() + 1;
    if (phase >= phasesToolbar->phaseCount()) {
        phase = 0;
    }

    if (phase == 0) {
        emit turnAdvanced();
        // Only the untap step runs the toolbar "untap all" side effect; do not fire draw/other phase actions here.
        phasesToolbar->triggerPhaseAction(0);
    } else {
        emit phaseChanged(phase);
    }
}

void TabGame::actRemoveLocalArrows()
{
    QMapIterator<int, Player *> playerIterator(game->getPlayerManager()->getPlayers());
    while (playerIterator.hasNext()) {
        Player *player = playerIterator.next().value();
        if (!player->getPlayerInfo()->getLocal())
            continue;
        QMapIterator<int, ArrowItem *> arrowIterator(player->getArrows());
        while (arrowIterator.hasNext()) {
            ArrowItem *a = arrowIterator.next().value();
            emit arrowDeletionRequested(a->getId());
        }
    }
}

void TabGame::actRotateViewCW()
{
    scene->adjustPlayerRotation(-1);
}

void TabGame::actRotateViewCCW()
{
    scene->adjustPlayerRotation(1);
}

void TabGame::actToggleStackWindow()
{
    if (!aToggleStackWindow) {
        return;
    }

    if (aToggleStackWindow->isChecked()) {
        syncStackWindowVisibility();
    } else if (stackView) {
        stackView->close();
    }
}

CardZoneLogic *TabGame::findVisibleStackZone() const
{
    if (!game) {
        return nullptr;
    }
    PlayerManager *pm = game->getPlayerManager();
    const QMap<int, Player *> &players = pm->getPlayers();
    // Prefer the non-empty stack with the most objects (the zone that is actually accumulating spells in 1v1 ruled
    // play). QMap iteration alone can pick another player's stale single card while the active stack grows elsewhere.
    CardZoneLogic *best = nullptr;
    int bestCount = -1;
    Player *localPlayer = pm->isSpectator() ? nullptr : pm->getPlayer(pm->getLocalPlayerId());
    // --- DIAG H2/H3: log all zones and their sizes to see which one gets picked. ---
    {
        QString zoneInfo;
        for (Player *p : players) {
            if (!p || !p->getStackZone()) continue;
            if (!zoneInfo.isEmpty()) zoneInfo += QLatin1Char(';');
            zoneInfo += QStringLiteral("pid=%1 n=%2 isLocal=%3")
                .arg(p->getPlayerInfo()->getId())
                .arg(p->getStackZone()->getCards().size())
                .arg(localPlayer == p);
        }
        tgDbgLog(QStringLiteral("findVisibleStackZone localPid=%1 zones=[%2]")
                     .arg(pm->getLocalPlayerId())
                     .arg(zoneInfo));
    }
    for (Player *player : players) {
        if (!player || !player->getStackZone()) {
            continue;
        }
        CardZoneLogic *zs = player->getStackZone();
        const int n = zs->getCards().size();
        if (n == 0) {
            continue;
        }
        if (n > bestCount) {
            bestCount = n;
            best = zs;
        } else if (n == bestCount && localPlayer && player == localPlayer) {
            best = zs;
        }
    }
    // Ruled mode: activated/triggered abilities use virtual engine stack items with no physical card.
    // Show the local player's stack zone even when empty so the window stays visible while the
    // ability is on the stack and the player can pass priority.
    if (!best && RuledActions::isRuledGame(game)) {
        const auto *handler = game->getGameEventHandler()->ruled();
        if (handler && handler->hasStackItems()) {
            if (localPlayer && localPlayer->getStackZone()) {
                return localPlayer->getStackZone();
            }
        }
    }
    // --- DIAG H2/H3: log which zone was chosen. ---
    {
        const int chosenPid = (best && best->getPlayer()) ? best->getPlayer()->getPlayerInfo()->getId() : -1;
        tgDbgLog(QStringLiteral("findVisibleStackZone → chosenPid=%1 count=%2")
                     .arg(chosenPid)
                     .arg(bestCount));
    }
    return best;
}

CardItem *TabGame::findVisibleStackSpellCardItem(int serverCardId) const
{
    if (serverCardId < 0 || !stackView) {
        // --- DIAG H4: log when the stack window is absent or ID is invalid. ---
        tgDbgLog(QStringLiteral("findVisibleStackSpellCardItem sid=%1 stackViewNull=%2 MISS (no window)")
                     .arg(serverCardId).arg(stackView == nullptr));
        return nullptr;
    }
    ZoneViewZone *zv = stackView->getZone();
    if (!zv) {
        return nullptr;
    }
    CardZoneLogic *logic = zv->getLogic();
    if (!logic || logic->getName().compare(QStringLiteral("stack"), Qt::CaseInsensitive) != 0) {
        return nullptr;
    }
    // --- DIAG H4: log which zone the stack window is showing and whether the card is found. ---
    const int windowPid = logic->getPlayer() ? logic->getPlayer()->getPlayerInfo()->getId() : -1;
    for (CardItem *c : logic->getCards()) {
        if (c && c->getId() == serverCardId) {
            tgDbgLog(QStringLiteral("findVisibleStackSpellCardItem sid=%1 windowPid=%2 HIT scenePos=(%3,%4)")
                         .arg(serverCardId).arg(windowPid)
                         .arg(c->scenePos().x()).arg(c->scenePos().y()));
            return c;
        }
    }
    tgDbgLog(QStringLiteral("findVisibleStackSpellCardItem sid=%1 windowPid=%2 MISS (not in window zone)")
                 .arg(serverCardId).arg(windowPid));
    return nullptr;
}

void TabGame::syncStackWindowVisibility()
{
    if (!aToggleStackWindow) {
        return;
    }

    if (!findVisibleStackZone()) {
        if (stackView) {
            stackView->close();
        }
        aToggleStackWindow->setChecked(false);
        return;
    }

    ensureStackWindow();
}

void TabGame::ensureStackWindow()
{
    if (!scene || !game || !aToggleStackWindow) {
        return;
    }
    if (!RuledActions::isRuledGame(game)) {
        return;
    }
    if (!game->getGameMetaInfo()->started()) {
        return;
    }
    CardZoneLogic *visibleStackZone = findVisibleStackZone();
    if (!visibleStackZone) {
        return;
    }

    // --- DIAG H3: log whether we're reusing an existing window or creating a new one. ---
    {
        const int newPid = visibleStackZone->getPlayer() ? visibleStackZone->getPlayer()->getPlayerInfo()->getId() : -1;
        const int oldPid = (stackViewZone && stackViewZone->getPlayer()) ? stackViewZone->getPlayer()->getPlayerInfo()->getId() : -1;
        const bool reusing = stackView && stackViewZone == visibleStackZone;
        tgDbgLog(QStringLiteral("ensureStackWindow reusing=%1 oldPid=%2 newPid=%3 stackViewNull=%4")
                     .arg(reusing).arg(oldPid).arg(newPid).arg(stackView == nullptr));
    }

    if (stackView && stackViewZone == visibleStackZone) {
        stackView->show();
        stackView->refreshContentLayout();
        if (GameEventHandler *geh = game->getGameEventHandler()) {
            geh->refreshRuledSpellTargetArrows();
        }
        aToggleStackWindow->setChecked(true);
        return;
    }

    if (stackView) {
        stackView->close();
    }

    Player *stackOwner = visibleStackZone->getPlayer();
    if (!stackOwner) {
        return;
    }

    stackView = new ZoneViewWidget(stackOwner, visibleStackZone, -1, true, true, {}, false, false, true);
    stackViewZone = visibleStackZone;
    stackView->setWindowFlags(stackView->windowFlags() | Qt::WindowStaysOnTopHint);
    scene->addItem(stackView);
    stackView->setPos(stackWindowPos);
    if (stackWindowSize.isValid()) {
        stackView->resize(stackWindowSize);
    }
    // Saved geometry can be narrower than a fanned stack; widen from optimum so all objects stay visible.
    stackView->refreshContentLayout();
    if (GameEventHandler *geh = game->getGameEventHandler()) {
        geh->refreshRuledSpellTargetArrows();
    }
    connect(stackView, &ZoneViewWidget::closePressed, this, [this](ZoneViewWidget *) {
        saveStackWindowLayout();
        stackView = nullptr;
        stackViewZone = nullptr;
        if (aToggleStackWindow) {
            aToggleStackWindow->setChecked(false);
        }
    });
    aToggleStackWindow->setChecked(true);
}

void TabGame::saveStackWindowLayout()
{
    if (!stackView) {
        return;
    }
    stackWindowPos = stackView->pos();
    stackWindowSize = stackView->size();
}

void TabGame::actCompleterChanged()
{
    SettingsCache::instance().getChatMentionCompleter() ? completer->setCompletionRole(2)
                                                        : completer->setCompletionRole(1);
}

void TabGame::notifyPlayerJoin(QString playerName)
{
    if (trayIcon) {
        QString gameId(QString::number(game->getGameMetaInfo()->gameId()));
        trayIcon->showMessage(tr("A player has joined game #%1").arg(gameId),
                              tr("%1 has joined the game").arg(playerName));
    }
}

void TabGame::notifyPlayerKicked()
{
    tabSupervisor->setCurrentIndex(tabSupervisor->indexOf(this));
    QMessageBox msgBox(this);
    msgBox.setWindowTitle(getTabText());
    msgBox.setText(tr("You have been kicked out of the game."));
    msgBox.setIcon(QMessageBox::Information);
    msgBox.exec();
}

Player *TabGame::addPlayer(Player *newPlayer)
{
    QString newPlayerName = "@" + newPlayer->getPlayerInfo()->getName();
    addPlayerToAutoCompleteList(newPlayerName);

    scene->addPlayer(newPlayer);

    connect(newPlayer, &Player::newCardAdded, this, &TabGame::newCardAdded);
    connect(newPlayer->getPlayerMenu(), &PlayerMenu::cardMenuUpdated, this, &TabGame::setCardMenu);
    connect(newPlayer->getStackZone(), &StackZoneLogic::cardCountChanged, this, &TabGame::syncStackWindowVisibility);

    messageLog->connectToPlayerEventHandler(newPlayer->getPlayerEventHandler());

    if (game->getGameState()->getIsLocalGame() ||
        (game->getPlayerManager()->isLocalPlayer(newPlayer->getPlayerInfo()->getId()) &&
         !game->getPlayerManager()->isSpectator())) {
        if (game->getGameState()->getIsLocalGame()) {
            newPlayer->getPlayerInfo()->setLocal(true);
        }
        addLocalPlayer(newPlayer, newPlayer->getPlayerInfo()->getId());
        syncStackWindowVisibility();
        if (game->getGameMetaInfo()->started()) {
            syncStackWindowVisibility();
        }
    }

    gameMenu->insertMenu(playersSeparator, newPlayer->getPlayerMenu()->getPlayerMenu());

    createZoneForPlayer(newPlayer, newPlayer->getPlayerInfo()->getId());

    return newPlayer;
}

void TabGame::addLocalPlayer(Player *newPlayer, int playerId)
{
    if (game->getGameState()->getClients().size() == 1) {
        newPlayer->getPlayerMenu()->setShortcutsActive();
    }

    auto *deckView = new TabbedDeckViewContainer(playerId, this);
    connect(deckView->playerDeckView, &DeckViewContainer::newCardAdded, this, &TabGame::newCardAdded);
    deckViewContainers.insert(playerId, deckView);
    deckViewContainerLayout->addWidget(deckView);

    if (gamePromptWidget && newPlayer->getPlayerActions()) {
        connect(newPlayer->getPlayerActions(), &PlayerActions::ruledSpellTargetingChanged, gamePromptWidget,
                &GamePromptWidget::setTargetingMode);
        connect(gamePromptWidget, &GamePromptWidget::confirmSpellDamageRequested,
                newPlayer->getPlayerActions(), &PlayerActions::confirmSpellDamageAllocation);
        connect(newPlayer->getPlayerActions(), &PlayerActions::ruledMultiTargetSelectionUpdated, gamePromptWidget,
                &GamePromptWidget::setMultiTargetSelectionCount);
        connect(gamePromptWidget, &GamePromptWidget::confirmTargetsRequested,
                newPlayer->getPlayerActions(), &PlayerActions::confirmMultiTargetSelection);
        connect(newPlayer->getPlayerActions(), &PlayerActions::landTapUndoAvailableChanged, gamePromptWidget,
                &GamePromptWidget::setLandTapUndoAvailable);
        connect(newPlayer->getPlayerActions(), &PlayerActions::ruledSpellCastPendingChanged, gamePromptWidget,
                &GamePromptWidget::setSpellCastPending);
        connect(newPlayer->getPlayerActions(), &PlayerActions::ruledSpellManaPromptChanged, this,
                [this, newPlayer]() {
                    if (!gamePromptWidget || !newPlayer->getPlayerInfo()->getLocal()) {
                        return;
                    }
                    const QString t = newPlayer->getPlayerActions()->pendingRuledSpellPromptText();
                    if (!t.isEmpty()) {
                        gamePromptWidget->setPromptText(t);
                    }
                });
        connect(newPlayer->getPlayerActions(), &PlayerActions::ruledAbilityActivationPendingChanged,
                gamePromptWidget, &GamePromptWidget::setSpellCastPending);
        connect(newPlayer->getPlayerActions(), &PlayerActions::ruledActivatedAbilityTargetPendingChanged,
                gamePromptWidget, &GamePromptWidget::setActivatedAbilityTargetPending);
        connect(newPlayer->getPlayerActions(), &PlayerActions::ruledAbilityManaPromptChanged, this,
                [this, newPlayer]() {
                    if (!gamePromptWidget || !newPlayer->getPlayerInfo()->getLocal()) {
                        return;
                    }
                    const QString t = newPlayer->getPlayerActions()->pendingRuledAbilityPromptText();
                    if (!t.isEmpty()) {
                        gamePromptWidget->setPromptText(t);
                    }
                });
        connect(gamePromptWidget, &GamePromptWidget::undoLandTapRequested, newPlayer->getPlayerActions(),
                &PlayerActions::undoLastLandTap);
        // CR 605 float courtesy: in ruled mode the Undo button reflects the engine's authoritative
        // undoable-mana count (per local player), which re-emits landTapUndoAvailableChanged.
        connect(game->getGameEventHandler()->ruled(), &RuledClientState::undoableManaAbilitiesChanged,
                newPlayer->getPlayerActions(), &PlayerActions::setRuledUndoableManaCount);
    }

    // auto load deck for player if that debug setting is enabled
    QString deckPath = SettingsCache::instance().debug().getDeckPathForPlayer(newPlayer->getPlayerInfo()->getName());
    if (!deckPath.isEmpty()) {
        QTimer::singleShot(0, this, [deckView, deckPath] {
            deckView->playerDeckView->loadDeckFromFile(deckPath);
            deckView->playerDeckView->readyAndUpdate();
        });
    }
}

void TabGame::processPlayerLeave(Player *leavingPlayer)
{
    QString playerName = "@" + leavingPlayer->getPlayerInfo()->getName();
    removePlayerFromAutoCompleteList(playerName);

    scene->removePlayer(leavingPlayer);

    // When we inserted the playerMenu into the gameMenu earlier, Qt wrapped the playerMenu into a QAction*, which lives
    // independently and does not get cleaned up when the source menu gets destroyed. We have to manually clean here.
    if (leavingPlayer->getPlayerMenu()) {
        QMenu *menu = leavingPlayer->getPlayerMenu()->getPlayerMenu();
        if (menu) {
            // Find and remove the QAction pointing to this menu
            QList<QAction *> actions = gameMenu->actions();
            for (QAction *act : actions) {
                if (act->menu() == menu) {
                    gameMenu->removeAction(act);
                    delete act; // deletes the QAction wrapper around the submenu
                    break;
                }
            }
        }
    }
}

void TabGame::processRemotePlayerDeckSelect(QString deckList, int playerId, QString playerName)
{
    DeckList loader;
    loader.loadFromString_Native(deckList);
    QMapIterator<int, TabbedDeckViewContainer *> i(deckViewContainers);
    while (i.hasNext()) {
        i.next();
        i.value()->addOpponentDeckView(loader, playerId, playerName);
    }
}

void TabGame::processMultipleRemotePlayerDeckSelect(QVector<QPair<int, QPair<QString, QString>>> playerIdDeckMap)
{
    for (const auto &entry : playerIdDeckMap) {
        int playerId = entry.first;
        QString playerName = entry.second.first;
        QString deckList = entry.second.second;

        processRemotePlayerDeckSelect(deckList, playerId, playerName);
    }
}

void TabGame::processLocalPlayerDeckSelect(Player *localPlayer, int playerId, ServerInfo_Player playerInfo)
{
    loadDeckForLocalPlayer(localPlayer, playerId, playerInfo);
    processLocalPlayerReady(playerId, playerInfo);
}

void TabGame::loadDeckForLocalPlayer(Player *localPlayer, int playerId, ServerInfo_Player playerInfo)
{
    TabbedDeckViewContainer *deckViewContainer = deckViewContainers.value(playerId);
    if (playerInfo.has_deck_list()) {
        DeckList deckList = DeckList(QString::fromStdString(playerInfo.deck_list()));
        CardPictureLoader::cacheCardPixmaps(CardDatabaseManager::query()->getCards(deckList.getCardRefList()));
        deckViewContainer->playerDeckView->setDeck(deckList);
        localPlayer->setDeck(deckList);
    }
}

void TabGame::processLocalPlayerReady(int playerId, ServerInfo_Player playerInfo)
{
    processLocalPlayerReadyStateChanged(playerId, playerInfo.properties().ready_start());
    processLocalPlayerSideboardLocked(playerId, playerInfo.properties().sideboard_locked());
}

void TabGame::processLocalPlayerSideboardLocked(int playerId, bool sideboardLocked)
{
    deckViewContainers.value(playerId)->playerDeckView->setSideboardLocked(sideboardLocked);
}

void TabGame::processLocalPlayerReadyStateChanged(int playerId, bool ready)
{
    deckViewContainers.value(playerId)->playerDeckView->setReadyStart(ready);
}

void TabGame::createZoneForPlayer(Player *newPlayer, int playerId)
{
    if (!game->getPlayerManager()->getSpectators().contains(playerId)) {

        // Loop for each player, the idea is to have one assigned zone for each non-spectator player
        for (int i = 1; i <= game->getPlayerManager()->getPlayerCount(); ++i) {
            bool aPlayerHasThisZone = false;
            for (auto &player : game->getPlayerManager()->getPlayers()) {
                if (player->getZoneId() == i) {
                    aPlayerHasThisZone = true;
                    break;
                }
            }
            if (!aPlayerHasThisZone) {
                newPlayer->setZoneId(i);
                break;
            }
        }
    }
}

void TabGame::startGame(bool _resuming)
{
    game->getGameState()->setCurrentPhase(-1);

    QMapIterator<int, TabbedDeckViewContainer *> i(deckViewContainers);
    while (i.hasNext()) {
        i.next();
        i.value()->playerDeckView->setReadyStart(false);
        i.value()->playerDeckView->setVisualDeckStorageExists(false);
        i.value()->hide();
    }

    mainWidget->setCurrentWidget(gamePlayAreaWidget);

    if (!_resuming) {
        QMapIterator<int, Player *> playerIterator(game->getPlayerManager()->getPlayers());
        while (playerIterator.hasNext())
            playerIterator.next().value()->setGameStarted();
    }

    playerListWidget->setGameStarted(true, game->getGameState()->isResuming());
    game->getGameMetaInfo()->setStarted(true);
    static_cast<GameScene *>(gameView->scene())->rearrange();
    syncStackWindowVisibility();

    if (aConcede != nullptr) {
        aConcede->setText(tr("&Concede"));
        aConcede->setEnabled(true);
    }
}

void TabGame::stopGame()
{
    clearRuledCombatArrows();
    QMapIterator<int, TabbedDeckViewContainer *> i(deckViewContainers);
    while (i.hasNext()) {
        i.next();
        i.value()->show();
    }

    mainWidget->setCurrentWidget(deckViewContainerWidget);

    playerListWidget->setActivePlayer(-1);
    playerListWidget->setGameStarted(false, false);

    scene->clearViews();
    if (stackView) {
        saveStackWindowLayout();
        stackView->close();
    }
    stackViewZone = nullptr;
    if (aToggleStackWindow) {
        aToggleStackWindow->setChecked(false);
    }

    if (aConcede != nullptr) {
        aConcede->setText(tr("&Concede"));
        aConcede->setEnabled(false);
    }
}

void TabGame::closeGame()
{
    gameMenu->clear();
    gameMenu->addAction(aLeaveGame);
}

Player *TabGame::setActivePlayer(int id)
{
    Player *player = game->getPlayerManager()->getPlayer(id);
    QMapIterator<int, Player *> i(game->getPlayerManager()->getPlayers());
    while (i.hasNext()) {
        i.next();
        i.value()->setActive(i.value() == player);
    }
    if (gamePromptWidget && player) {
        gamePromptWidget->setActivePlayerName(player->getPlayerInfo()->getName());
        gamePromptWidget->setLocalPlayerIsActive(id == game->getPlayerManager()->getLocalPlayerId());
    }
    game->getGameState()->setCurrentPhase(-1);
    emitUserEvent();
    setPriorityPlayer(id);
    return player;
}

Player *TabGame::setPriorityPlayer(int id)
{
    Player *priorityPlayer = game->getPlayerManager()->getPlayer(id);
    const int localPlayerId = game->getPlayerManager()->getLocalPlayerId();
    if (gamePromptWidget && RuledActions::isRuledGame(game)) {
        const bool localHasPriority = (id == localPlayerId);
        gamePromptWidget->setLocalPlayerHasPriority(localHasPriority);
        if (priorityPlayer) {
            gamePromptWidget->setPriorityPlayerName(priorityPlayer->getPlayerInfo()->getName());
        }
        if (localHasPriority) {
            // Defer the auto-advance decision: combatStateChanged (which updates
            // localPlayerHasCombatButtons) is emitted after the full event batch, but
            // setPriorityPlayer is called during batch processing. Deferring ensures the
            // mustDeclare check sees up-to-date combat state.
            QTimer::singleShot(0, this, [this, localPlayerId]() {
                if (!game || !game->getGameState() || !game->getGameEventHandler()) {
                    return;
                }
                if (game->getGameState()->getPriorityPlayer() != localPlayerId) {
                    return;
                }
                const int currentPhase = game->getGameState()->getCurrentPhase();
                const bool myTurn = (game->getGameState()->getActivePlayer() == localPlayerId);
                const bool hasManualStop = phasesToolbar->shouldStopAtPhase(currentPhase, myTurn);
                const bool stackIsEmpty = !game->getGameEventHandler()->ruled()->hasStackItems();
                const bool cleanupDiscard = game->getGameEventHandler()->ruled()->localPlayerMustCleanupDiscard();
                const bool openingPhase = game->getGameEventHandler()->ruled()->engineOpeningPhaseActive();
                const bool mustDeclare = gamePromptWidget && gamePromptWidget->localPlayerMustDeclareCombat();
                // CR 510.4: the first-strike damage step shares the "Combat Damage" toolbar
                // slot (phase 7) with regular damage, matching MTGO — both substeps obey the
                // same Combat Damage stop. If the player has that stop enabled, they get a
                // priority window for both FS damage and regular damage; if disabled, auto-pass
                // skips both. No special override here.
                if (!openingPhase && !hasManualStop && stackIsEmpty && !cleanupDiscard && !mustDeclare) {
                    game->getGameEventHandler()->handleNextTurn();
                }
            });
        } else {
            Player *localPlayer = game->getPlayerManager()->getPlayers().value(localPlayerId, nullptr);
            if (localPlayer && localPlayer->getPlayerActions()) {
                localPlayer->getPlayerActions()->clearLandTapUndoStack();
            }
        }
    }
    playerListWidget->setActivePlayer(id);
    QMapIterator<int, Player *> i(game->getPlayerManager()->getPlayers());
    while (i.hasNext()) {
        i.next();
        const bool isPriorityPlayer = (i.value() == priorityPlayer);
        i.value()->getGraphicsItem()->setPriorityHighlighted(isPriorityPlayer);
        if (game->getGameState()->getClients().size() > 1) {
            if (isPriorityPlayer) {
                i.value()->getPlayerMenu()->setShortcutsActive();
            } else {
                i.value()->getPlayerMenu()->setShortcutsInactive();
            }
        }
    }
    return priorityPlayer;
}

void TabGame::setActivePhase(int phase)
{
    phasesToolbar->setActivePhase(phase);
}

void TabGame::newCardAdded(AbstractCardItem *card)
{
    connect(card, &AbstractCardItem::hovered, cardInfoFrameWidget,
            qOverload<AbstractCardItem *>(&CardInfoFrameWidget::setCard));
    connect(card, &AbstractCardItem::showCardInfoPopup, this, &TabGame::showCardInfoPopup);
    connect(card, SIGNAL(deleteCardInfoPopup(QString)), this, SLOT(deleteCardInfoPopup(QString)));
    connect(card, &AbstractCardItem::cardShiftClicked, this, &TabGame::linkCardToChat);
}

QString TabGame::getTabText() const
{
    QString gameTypeInfo;
    if (!gameTypes.empty()) {
        gameTypeInfo = gameTypes.at(0);
        if (gameTypes.size() > 1)
            gameTypeInfo.append("...");
    }

    QString gameDesc(game->getGameMetaInfo()->description());
    QString gameId(QString::number(game->getGameMetaInfo()->gameId()));

    QString tabText;
    if (replayDock)
        tabText.append(tr("Replay") + " ");
    if (!gameTypeInfo.isEmpty())
        tabText.append(gameTypeInfo + " ");
    if (!gameDesc.isEmpty()) {
        if (gameDesc.length() >= 15)
            tabText.append("| " + gameDesc.left(15) + "... ");
        else
            tabText.append("| " + gameDesc + " ");
    }
    if (!tabText.isEmpty())
        tabText.append("| ");
    tabText.append("#" + gameId);

    return tabText;
}

/**
 * @param menu The menu to set. Pass in nullptr to set the menu to empty.
 */
void TabGame::setCardMenu(QMenu *menu)
{
    if (!aCardMenu) {
        return;
    }

    if (menu) {
        aCardMenu->setMenu(menu);
    } else {
        aCardMenu->setMenu(new QMenu);
    }
}

void TabGame::createMenuItems()
{
    aNextPhase = new QAction(this);
    connect(aNextPhase, &QAction::triggered, this, &TabGame::actNextPhase);
    connect(this, &TabGame::phaseChanged, game->getGameEventHandler(), &GameEventHandler::handleActivePhaseChanged);
    aNextPhaseAction = new QAction(this);
    connect(aNextPhaseAction, &QAction::triggered, this, &TabGame::actNextPhaseAction);
    connect(this, &TabGame::turnAdvanced, game->getGameEventHandler(), &GameEventHandler::handleNextTurn);
    aNextTurn = new QAction(this);
    connect(aNextTurn, &QAction::triggered, game->getGameEventHandler(), &GameEventHandler::handleNextTurn);
    aReverseTurn = new QAction(this);
    connect(aReverseTurn, &QAction::triggered, game->getGameEventHandler(), &GameEventHandler::handleReverseTurn);
    aRemoveLocalArrows = new QAction(this);
    connect(aRemoveLocalArrows, &QAction::triggered, this, &TabGame::actRemoveLocalArrows);
    connect(this, &TabGame::arrowDeletionRequested, game->getGameEventHandler(),
            &GameEventHandler::handleArrowDeletion);
    aRotateViewCW = new QAction(this);
    connect(aRotateViewCW, &QAction::triggered, this, &TabGame::actRotateViewCW);
    aRotateViewCCW = new QAction(this);
    connect(aRotateViewCCW, &QAction::triggered, this, &TabGame::actRotateViewCCW);
    aToggleStackWindow = new QAction(this);
    aToggleStackWindow->setCheckable(true);
    connect(aToggleStackWindow, &QAction::triggered, this, &TabGame::actToggleStackWindow);
    aGameInfo = new QAction(this);
    connect(aGameInfo, &QAction::triggered, this, &TabGame::actGameInfo);
    aConcede = new QAction(this);
    connect(aConcede, &QAction::triggered, this, &TabGame::actConcede);
    if (!game->getGameMetaInfo()->started()) {
        aConcede->setEnabled(false);
    }
    connect(game->getPlayerManager(), &PlayerManager::activeLocalPlayerConceded, game->getGameEventHandler(),
            &GameEventHandler::handleActiveLocalPlayerConceded);
    connect(game->getPlayerManager(), &PlayerManager::activeLocalPlayerUnconceded, game->getGameEventHandler(),
            &GameEventHandler::handleActiveLocalPlayerUnconceded);
    aLeaveGame = new QAction(this);
    connect(aLeaveGame, &QAction::triggered, this, &TabGame::closeRequest);
    aFocusChat = new QAction(this);
    connect(aFocusChat, &QAction::triggered, sayEdit, qOverload<>(&LineEditCompleter::setFocus));

    phasesMenu = new TearOffMenu(this);

    for (int i = 0; i < phasesToolbar->phaseCount(); ++i) {
        auto *temp = new QAction(QString(), this);
        connect(temp, &QAction::triggered, this, &TabGame::actPhaseAction);
        phasesMenu->addAction(temp);
        phaseActions.append(temp);
    }

    phasesMenu->addSeparator();
    phasesMenu->addAction(aNextPhase);
    phasesMenu->addAction(aNextPhaseAction);

    gameMenu = new QMenu(this);
    playersSeparator = gameMenu->addSeparator();
    gameMenu->addMenu(phasesMenu);
    gameMenu->addAction(aNextTurn);
    gameMenu->addAction(aReverseTurn);
    gameMenu->addSeparator();
    gameMenu->addAction(aRemoveLocalArrows);
    gameMenu->addAction(aRotateViewCW);
    gameMenu->addAction(aRotateViewCCW);
    gameMenu->addSeparator();
    gameMenu->addAction(aGameInfo);
    gameMenu->addAction(aConcede);
    gameMenu->addAction(aFocusChat);
    gameMenu->addAction(aLeaveGame);

    gameMenu->addSeparator();

    aCardMenu = gameMenu->addMenu(new QMenu(this));

    addTabMenu(gameMenu);
}

void TabGame::createReplayMenuItems()
{
    aNextPhase = nullptr;
    aNextPhaseAction = nullptr;
    aNextTurn = nullptr;
    aReverseTurn = nullptr;
    aRemoveLocalArrows = nullptr;
    aRotateViewCW = nullptr;
    aRotateViewCCW = nullptr;
    aResetLayout = nullptr;
    aGameInfo = nullptr;
    aConcede = nullptr;
    aFocusChat = nullptr;
    aToggleStackWindow = nullptr;
    aLeaveGame = new QAction(this);
    connect(aLeaveGame, &QAction::triggered, this, &TabGame::closeRequest);

    phasesMenu = nullptr;
    gameMenu = new QMenu(this);
    gameMenu->addAction(aLeaveGame);

    aCardMenu = nullptr;

    addTabMenu(gameMenu);
}

void TabGame::createViewMenuItems()
{
    viewMenu = new QMenu(this);

    registerDockWidget(viewMenu, cardInfoDock, {250, 360});
    registerDockWidget(viewMenu, messageLayoutDock, {250, 200});
    registerDockWidget(viewMenu, playerListDock, {250, 50});

    if (replayDock) {
        registerDockWidget(viewMenu, replayDock, {900, 100});
    }

    viewMenu->addSeparator();
    if (aToggleStackWindow && RuledActions::isRuledGame(game)) {
        viewMenu->addAction(aToggleStackWindow);
    }
    viewMenu->addSeparator();

    aResetLayout = viewMenu->addAction(QString());
    connect(aResetLayout, &QAction::triggered, this, &TabGame::actResetLayout);
    viewMenu->addAction(aResetLayout);

    addTabMenu(viewMenu);
}

void TabGame::registerDockWidget(QMenu *_viewMenu, QDockWidget *widget, const QSize &defaultSize)
{
    QMenu *menu = _viewMenu->addMenu(QString());

    QAction *aVisible = menu->addAction(QString());
    aVisible->setCheckable(true);

    QAction *aFloating = menu->addAction(QString());
    aFloating->setCheckable(true);
    aFloating->setEnabled(false);

    // user interaction
    connect(aVisible, &QAction::triggered, widget, [widget](bool checked) { widget->setVisible(checked); });
    connect(aFloating, &QAction::triggered, this, [widget](bool checked) { widget->setFloating(checked); });

    // sync aFloating's enabled state with aVisible's checked state
    connect(aVisible, &QAction::toggled, aFloating, [aFloating](bool checked) { aFloating->setEnabled(checked); });

    // sync aFloating with dockWidget's floating state
    connect(widget, &QDockWidget::topLevelChanged, aFloating,
            [aFloating](bool topLevel) { aFloating->setChecked(topLevel); });

    // sync aVisible with dockWidget's visible state
    auto filter = new VisibilityChangeListener(widget);
    connect(filter, &VisibilityChangeListener::visibilityChanged, aVisible,
            [aVisible](bool visible) { aVisible->setChecked(visible); });

    dockToActions.insert(widget, {menu, aVisible, aFloating, defaultSize});
}

void TabGame::loadLayout()
{
    LayoutsSettings &layouts = SettingsCache::instance().layouts();
    if (replayDock) {
        restoreGeometry(layouts.getReplayPlayAreaGeometry());
        restoreState(layouts.getReplayPlayAreaLayoutState());
    } else {
        restoreGeometry(layouts.getGamePlayAreaGeometry());
        restoreState(layouts.getGamePlayAreaLayoutState());
    }
}

void TabGame::actResetLayout()
{
    cardInfoDock->setVisible(true);
    playerListDock->setVisible(true);
    messageLayoutDock->setVisible(true);

    cardInfoDock->setFloating(false);
    playerListDock->setFloating(false);
    messageLayoutDock->setFloating(false);

    addDockWidget(Qt::RightDockWidgetArea, cardInfoDock);
    addDockWidget(Qt::RightDockWidgetArea, playerListDock);
    addDockWidget(Qt::RightDockWidgetArea, messageLayoutDock);

    if (replayDock) {
        replayDock->setVisible(true);
        replayDock->setFloating(false);
        addDockWidget(Qt::BottomDockWidgetArea, replayDock);

        cardInfoDock->resize(250, 360);
        messageLayoutDock->resize(250, 200);
        playerListDock->resize(250, 50);
        replayDock->resize(900, 100);
    } else {
        cardInfoDock->resize(250, 360);
        messageLayoutDock->resize(250, 250);
        playerListDock->resize(250, 50);
    }
}

void TabGame::createPlayAreaWidget(bool bReplay)
{
    phasesToolbar = new PhasesToolbar;
    if (!bReplay)
        connect(phasesToolbar, &PhasesToolbar::sendGameCommand, game->getGameEventHandler(),
                qOverload<const ::google::protobuf::Message &, int>(&GameEventHandler::sendGameCommand));
    scene = new GameScene(phasesToolbar, this);
    connect(game->getPlayerManager(), &PlayerManager::playerConceded, scene, &GameScene::rearrange);
    connect(game->getPlayerManager(), &PlayerManager::playerCountChanged, scene, &GameScene::rearrange);
    gameView = new GameView(scene);

    auto gamePlayAreaVBox = new QVBoxLayout;
    gamePlayAreaVBox->setContentsMargins(0, 0, 0, 0);
    gamePlayAreaVBox->addWidget(gameView);

    gamePlayAreaWidget = new QWidget;
    gamePlayAreaWidget->setObjectName("gamePlayAreaWidget");
    gamePlayAreaWidget->setLayout(gamePlayAreaVBox);
}

void TabGame::createReplayDock(GameReplay *replay)
{
    replayManager = new ReplayManager(this, replay);

    replayDock = new QDockWidget(this);
    replayDock->setObjectName("replayDock");
    replayDock->setFeatures(QDockWidget::DockWidgetClosable | QDockWidget::DockWidgetFloatable |
                            QDockWidget::DockWidgetMovable);
    replayDock->setWidget(replayManager);
    replayDock->setFloating(false);
}

void TabGame::createDeckViewContainerWidget(bool bReplay)
{
    Q_UNUSED(bReplay);

    deckViewContainerWidget = new QWidget();
    deckViewContainerWidget->setObjectName("deckViewContainerWidget");
    deckViewContainerLayout = new QVBoxLayout;
    deckViewContainerLayout->setContentsMargins(0, 0, 0, 0);
    deckViewContainerWidget->setLayout(deckViewContainerLayout);
}

void TabGame::viewCardInfo(const CardRef &cardRef) const
{
    cardInfoFrameWidget->setCard(cardRef);
}

void TabGame::createCardInfoDock(bool bReplay)
{
    Q_UNUSED(bReplay);

    cardInfoFrameWidget = new CardInfoFrameWidget();
    auto cardHInfoLayout = new QHBoxLayout;
    auto cardVInfoLayout = new QVBoxLayout;
    cardVInfoLayout->setContentsMargins(0, 0, 0, 0);
    cardVInfoLayout->addWidget(cardInfoFrameWidget);
    cardVInfoLayout->addLayout(cardHInfoLayout);

    auto cardBoxLayoutWidget = new QWidget;
    cardBoxLayoutWidget->setLayout(cardVInfoLayout);

    cardInfoDock = new QDockWidget(this);
    cardInfoDock->setObjectName("cardInfoDock");
    cardInfoDock->setFeatures(QDockWidget::DockWidgetClosable | QDockWidget::DockWidgetFloatable |
                              QDockWidget::DockWidgetMovable);
    cardInfoDock->setWidget(cardBoxLayoutWidget);
    cardInfoDock->setFloating(false);
}

void TabGame::createPlayerListDock(bool bReplay)
{
    if (bReplay) {
        playerListWidget = new PlayerListWidget(nullptr, nullptr, game);
    } else {
        playerListWidget = new PlayerListWidget(tabSupervisor, game->getGameState()->getClients().first(), game);
        connect(playerListWidget, SIGNAL(openMessageDialog(QString, bool)), this,
                SIGNAL(openMessageDialog(QString, bool)));
    }
    playerListWidget->setFocusPolicy(Qt::NoFocus);

    playerListDock = new QDockWidget(this);
    playerListDock->setObjectName("playerListDock");
    playerListDock->setFeatures(QDockWidget::DockWidgetClosable | QDockWidget::DockWidgetFloatable |
                                QDockWidget::DockWidgetMovable);
    playerListDock->setWidget(playerListWidget);
    playerListDock->setFloating(false);
}

void TabGame::createMessageDock(bool bReplay)
{
    auto messageLogLayout = new QVBoxLayout;
    messageLogLayout->setContentsMargins(0, 0, 0, 0);

    // clock
    if (!bReplay) {
        timeElapsedLabel = new QLabel;
        timeElapsedLabel->setAlignment(Qt::AlignCenter);
        connect(game->getGameState(), &GameState::updateTimeElapsedLabel, this, &TabGame::updateTimeElapsedLabel);

        messageLogLayout->addWidget(timeElapsedLabel);
    }

    if (!bReplay && RuledActions::isRuledGame(game)) {
        gamePromptWidget = new GamePromptWidget(this);
        gamePromptWidget->setPassPriorityEnabled(true);
        gamePromptWidget->setActivePhase(game->getGameState()->getCurrentPhase());
        {
            const int localId = game->getPlayerManager()->getLocalPlayerId();
            const int priorityId = game->getGameState()->getPriorityPlayer();
            const int activeId = game->getGameState()->getActivePlayer();
            gamePromptWidget->setLocalPlayerHasPriority(priorityId == localId);
            gamePromptWidget->setLocalPlayerIsActive(activeId == localId);
            if (Player *ap = game->getPlayerManager()->getPlayer(activeId)) {
                gamePromptWidget->setActivePlayerName(ap->getPlayerInfo()->getName());
            }
            if (Player *pp = game->getPlayerManager()->getPlayer(priorityId)) {
                gamePromptWidget->setPriorityPlayerName(pp->getPlayerInfo()->getName());
            }
        }
        connect(gamePromptWidget, &GamePromptWidget::passPriorityRequested, game->getGameEventHandler(),
                &GameEventHandler::handleNextTurn);
        messageLogLayout->addWidget(gamePromptWidget);
    } else {
        gamePromptWidget = nullptr;
    }

    // Fork: dev-loop console, under the prompt panel in the same dock. Hidden unless explicitly
    // asked for; the enforcing gate is engine-side, this only keeps it out of a normal session.
    if (!bReplay && RuledActions::isRuledGame(game) && RuledDevConsoleWidget::isEnabled()) {
        devConsoleWidget = new RuledDevConsoleWidget(this);
        connect(devConsoleWidget, &RuledDevConsoleWidget::commandSubmitted, this, &TabGame::actDevConsoleCommand);
        messageLogLayout->addWidget(devConsoleWidget);
    } else {
        devConsoleWidget = nullptr;
    }

    // message log
    messageLog = new MessageLogWidget(tabSupervisor, game);
    connect(messageLog, &MessageLogWidget::cardNameHovered, cardInfoFrameWidget,
            qOverload<const QString &>(&CardInfoFrameWidget::setCard));
    connect(messageLog, &MessageLogWidget::showCardInfoPopup, this, &TabGame::showCardInfoPopup);
    connect(messageLog, &MessageLogWidget::deleteCardInfoPopup, this, &TabGame::deleteCardInfoPopup);

    if (!bReplay) {
        connect(messageLog, &MessageLogWidget::openMessageDialog, this, &TabGame::openMessageDialog);
        connect(messageLog, &MessageLogWidget::addMentionTag, this, &TabGame::addMentionTag);
        connect(&SettingsCache::instance(), &SettingsCache::chatMentionCompleterChanged, this,
                &TabGame::actCompleterChanged);
    }

    messageLogLayout->addWidget(messageLog);

    // chat entry
    if (!bReplay) {
        sayLabel = new QLabel;
        sayEdit = new LineEditCompleter;
        sayEdit->setMaxLength(MAX_TEXT_LENGTH);
        sayLabel->setBuddy(sayEdit);
        connect(this, &TabGame::chatMessageSent, game->getGameEventHandler(), &GameEventHandler::handleChatMessageSent);
        completer = new QCompleter(autocompleteUserList, sayEdit);
        completer->setCaseSensitivity(Qt::CaseInsensitive);
        completer->setMaxVisibleItems(5);
        completer->setFilterMode(Qt::MatchStartsWith);

        sayEdit->setCompleter(completer);
        actCompleterChanged();

        if (game->getPlayerManager()->isSpectator()) {
            /* Spectators can only talk if:
             * (a) the game creator allows it
             * (b) the spectator is a moderator/administrator
             * (c) the spectator is a judge
             */
            bool isModOrJudge = !tabSupervisor->getAdminLocked() || game->getPlayerManager()->isJudge();
            if (!isModOrJudge && !game->getGameMetaInfo()->spectatorsCanChat()) {
                sayLabel->hide();
                sayEdit->hide();
            }
        }

        connect(tabSupervisor, &TabSupervisor::adminLockChanged, this, &TabGame::adminLockChanged);
        connect(sayEdit, &LineEditCompleter::returnPressed, this, &TabGame::actSay);

        auto sayHLayout = new QHBoxLayout;
        sayHLayout->addWidget(sayLabel);
        sayHLayout->addWidget(sayEdit);

        messageLogLayout->addLayout(sayHLayout);
    }

    // dock
    auto messageLogLayoutWidget = new QWidget;
    messageLogLayoutWidget->setLayout(messageLogLayout);

    messageLayoutDock = new QDockWidget(this);
    messageLayoutDock->setObjectName("messageLayoutDock");
    messageLayoutDock->setFeatures(QDockWidget::DockWidgetClosable | QDockWidget::DockWidgetFloatable |
                                   QDockWidget::DockWidgetMovable);
    messageLayoutDock->setWidget(messageLogLayoutWidget);
    messageLayoutDock->setFloating(false);
}

void TabGame::hideEvent(QHideEvent *event)
{
    LayoutsSettings &layouts = SettingsCache::instance().layouts();
    if (replayDock) {
        layouts.setReplayPlayAreaState(saveState());
        layouts.setReplayPlayAreaGeometry(saveGeometry());
    } else {
        layouts.setGamePlayAreaState(saveState());
        layouts.setGamePlayAreaGeometry(saveGeometry());
    }

    Tab::hideEvent(event);
}

void TabGame::onRuledLibrarySearchPickStarted(QStringList candidateNames, QVector<int> serverCardIds)
{
    if (!game || !scene) {
        return;
    }
    const int localId = game->getPlayerManager()->getLocalPlayerId();
    Player *localPlayer = game->getPlayerManager()->getPlayers().value(localId, nullptr);
    if (!localPlayer) {
        return;
    }
    CardZoneLogic *deckZone = localPlayer->getZones().value(ZoneNames::DECK);
    if (!deckZone) {
        return;
    }
    // Close any stale library search view from a prior step.
    if (librarySearchView) {
        librarySearchView->close();
    }
    // Build synthetic cards from the engine's candidate list so the view shows named cards.
    // These are stored alongside the view; cleared when the view closes.
    qDeleteAll(librarySearchCards);
    librarySearchCards.clear();
    QList<const ServerInfo_Card *> cardList;
    for (int i = 0; i < candidateNames.size(); ++i) {
        auto *sic = new ServerInfo_Card;
        sic->set_name(candidateNames.at(i).toStdString());
        sic->set_id(i < serverCardIds.size() ? serverCardIds.at(i) : -1);
        sic->set_face_down(false);
        librarySearchCards.append(sic);
        cardList.append(sic);
    }
    // Open revealed, not closeable (resolution is mandatory per CR 608).
    librarySearchView =
        new ZoneViewWidget(localPlayer, deckZone, -1, true, false, cardList, false, true, false, false);
    // The deck zone is only a scaffold for the widget; title the window for what it actually shows.
    librarySearchView->setWindowTitle(game->getGameEventHandler()->ruled()->resolutionHandPickViewTitle());
    scene->addItem(librarySearchView);
    librarySearchView->setPos(340, 80);
    connect(librarySearchView, &ZoneViewWidget::closePressed, this, [this](ZoneViewWidget *) {
        librarySearchView = nullptr;
        qDeleteAll(librarySearchCards);
        librarySearchCards.clear();
    });
}

void TabGame::onRuledRevealedPickChanged(bool started, QStringList cardNames,
                                         QVector<int> serverCardIds, int /*min*/, int /*max*/)
{
    // Clean up any prior revealed pick view and synthetic card objects.
    if (revealedPickView) {
        revealedPickView->close();
        revealedPickView = nullptr;
    }
    qDeleteAll(revealedPickCards);
    revealedPickCards.clear();

    if (!started || cardNames.isEmpty() || !game || !scene) {
        return;
    }
    const int localId = game->getPlayerManager()->getLocalPlayerId();
    Player *localPlayer = game->getPlayerManager()->getPlayers().value(localId, nullptr);
    if (!localPlayer) {
        return;
    }
    // Use the local player's deck zone as the "origin" zone for the view widget.
    // The actual cards shown are overridden by the cardList parameter below.
    CardZoneLogic *deckZone = localPlayer->getZones().value(ZoneNames::DECK);
    if (!deckZone) {
        return;
    }
    // Build synthetic ServerInfo_Card objects from the candidate names + server card IDs.
    QList<const ServerInfo_Card *> cardList;
    for (int i = 0; i < cardNames.size(); ++i) {
        auto *sic = new ServerInfo_Card;
        sic->set_name(cardNames.at(i).toStdString());
        sic->set_id(i < serverCardIds.size() ? serverCardIds.at(i) : -1);
        sic->set_face_down(false);
        revealedPickCards.append(sic);
        cardList.append(sic);
    }
    // Create a revealed zone view (stack-window style: fan layout, no sort controls).
    // revealZone = true shows cards face-up; _showControls = false omits search/sort.
    // Not closeable: resolution is mandatory per CR 608.
    revealedPickView = new ZoneViewWidget(localPlayer, deckZone, -1, true, false, cardList,
                                          false, false, true, false);
    // The deck zone is only a scaffold for the widget: without this the window would announce
    // itself as somebody's library while showing a hand (Thoughtseize) or a revealed set (Gifts).
    revealedPickView->setWindowTitle(game->getGameEventHandler()->ruled()->resolutionHandPickViewTitle());
    scene->addItem(revealedPickView);
    revealedPickView->setPos(340, 80);
    connect(revealedPickView, &ZoneViewWidget::closePressed, this, [this](ZoneViewWidget *) {
        revealedPickView = nullptr;
        qDeleteAll(revealedPickCards);
        revealedPickCards.clear();
    });
}
