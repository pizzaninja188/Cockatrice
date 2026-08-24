/**
 * @file tab_game.h
 * @ingroup Tabs
 * @ingroup GameWidgets
 * @ingroup Lobby
 * @brief TODO: Document this.
 */

#ifndef TAB_GAME_H
#define TAB_GAME_H

#include "../game/abstract_game.h"
#include "../game/log/message_log_widget.h"
// For GamePromptWidget::PromptMode on refreshRuledPromptState() — ruled prompt panel (fork).
#include "../game/prompt/game_prompt_widget.h"
// For RuledTriggerOrderCandidate on the CR 603.3b ordering signal (fork).
#include "../game/player/player.h"
#include "../game/ruled/ruled_client_state.h"
#include "../interface/widgets/menus/tearoff_menu.h"
#include "../interface/widgets/replay/replay_manager.h"
#include "tab.h"

#include <QCompleter>
#include <QLoggingCategory>
#include <QMap>
#include <QPointF>
#include <QPointer>
#include <QSizeF>

class ServerInfo_PlayerProperties;
class TabbedDeckViewContainer;
inline Q_LOGGING_CATEGORY(TabGameLog, "tab_game");

class UserListProxy;
class DeckViewContainer;
class AbstractClient;
class CardDatabase;
class GameView;
class GameScene;
class ReplayManager;
class CardInfoFrameWidget;
class QTimer;
class QSplitter;
class QLabel;
class QToolButton;
class QMenu;
class ZoneViewLayout;
class ZoneViewWidget;
class CardZoneLogic;
class PhasesToolbar;
class PlayerListWidget;
class ReplayTimelineWidget;
class CardZone;
class AbstractCardItem;
class CardItem;
class ArrowItem;
class QVBoxLayout;
class QHBoxLayout;
class GameReplay;
class LineEditCompleter;
class QDockWidget;
class QStackedWidget;
class GamePromptWidget;
class RuledDevConsoleWidget;

class TabGame : public Tab
{
    Q_OBJECT

    friend class RuledAutopilot; // fork: dev-loop autopilot needs this seat's deck view

private:
    AbstractGame *game;
    const UserListProxy *userListProxy;
    ReplayManager *replayManager = nullptr;
    QStringList gameTypes;
    QCompleter *completer;
    QStringList autocompleteUserList;
    QStackedWidget *mainWidget;

    CardInfoFrameWidget *cardInfoFrameWidget;
    PlayerListWidget *playerListWidget;
    QLabel *timeElapsedLabel;
    MessageLogWidget *messageLog;
    QLabel *sayLabel;
    LineEditCompleter *sayEdit;
    PhasesToolbar *phasesToolbar;
    GameScene *scene;
    GameView *gameView;
    QMap<int, TabbedDeckViewContainer *> deckViewContainers;
    QVBoxLayout *deckViewContainerLayout;
    QWidget *gamePlayAreaWidget, *deckViewContainerWidget;
    QDockWidget *cardInfoDock, *messageLayoutDock, *playerListDock, *replayDock;
    GamePromptWidget *gamePromptWidget;
    /// Fork: dev-loop command entry. Null unless --dev-console was passed on a ruled, non-replay
    /// game — guard every use, the way replayDock is guarded.
    RuledDevConsoleWidget *devConsoleWidget;
    QList<QPointer<ArrowItem>> ruledCombatArrows;
    /// Graveyard views a pending trigger or cast opened for us, keyed by the player whose
    /// graveyard it is, so each can be closed again without touching one the player opened
    /// themselves. A spell may target any graveyard (Reanimate), so this is per-player rather
    /// than a single widget. Empty when we have none open.
    QHash<int, QPointer<ZoneViewWidget>> ruledAutoOpenedGraveyardViews;
    QPointer<ZoneViewWidget> stackView;
    CardZoneLogic *stackViewZone = nullptr;
    // Deck zone view auto-opened for LibrarySearch (Gifts Ungiven search step).
    QPointer<ZoneViewWidget> librarySearchView;
    // ServerInfo_Card storage for the library search popup (owned by this instance).
    QList<ServerInfo_Card *> librarySearchCards;
    // Sole reveal popup, reused by private Gifts-style picks and public hand reveals.
    QPointer<ZoneViewWidget> revealedPickView;
    // ServerInfo_Card storage for the revealed-cards popup (owned by this instance).
    QList<ServerInfo_Card *> revealedPickCards;
    /// Read-only exact snapshot of cards revealed for spells that remain on the stack. Separate
    /// from resolution reveals so the two lifecycles cannot close or overwrite each other.
    QPointer<ZoneViewWidget> activeCastRevealView;
    QList<ServerInfo_Card *> activeCastRevealCards;
    /// One closeable, control-free mirror per active engine permission cohort. Closing a group
    /// records a local dismissal until that group disappears from the authoritative snapshot.
    QHash<quint64, QPointer<ZoneViewWidget>> exilePlayPermissionViews;
    QHash<quint64, QList<ServerInfo_Card *>> exilePlayPermissionCards;
    QSet<quint64> dismissedExilePlayPermissionGroups;
    // CR 603.3b: card-image popup for picking which trigger goes on the stack next. Alive only
    // while this client is the deciding player, and rebuilt after each pick with what remains.
    QPointer<ZoneViewWidget> triggerOrderView;
    // ServerInfo_Card storage for that popup (owned by this instance).
    QList<ServerInfo_Card *> triggerOrderCards;
    QPointF stackWindowPos = QPointF(340, 80);
    QSizeF stackWindowSize;
    QAction *playersSeparator;
    QMenu *gameMenu, *viewMenu;
    TearOffMenu *phasesMenu;
    QAction *aGameInfo, *aConcede, *aLeaveGame, *aNextPhase, *aNextPhaseAction, *aNextTurn, *aReverseTurn,
        *aRemoveLocalArrows, *aRotateViewCW, *aRotateViewCCW, *aResetLayout, *aResetReplayLayout;
    QAction *aFocusChat;
    QAction *aToggleStackWindow;
    QList<QAction *> phaseActions;
    QAction *aCardMenu;

    /**
     * @brief The actions associated with managing a QDockWidget
     */
    struct DockActions
    {
        QMenu *menu;
        QAction *aVisible;
        QAction *aFloating;
        QSize defaultSize;
    };

    QMap<QDockWidget *, DockActions> dockToActions;

    Player *addPlayer(Player *newPlayer);
    void addLocalPlayer(Player *newPlayer, int playerId);
    void processRemotePlayerDeckSelect(QString deckList, int playerId, QString playerName);
    void processMultipleRemotePlayerDeckSelect(QVector<QPair<int, QPair<QString, QString>>> playerIdDeckMap);
    void processLocalPlayerDeckSelect(Player *localPlayer, int playerId, ServerInfo_Player playerInfo);
    void loadDeckForLocalPlayer(Player *localPlayer, int playerId, ServerInfo_Player playerInfo);
    void processLocalPlayerReady(int playerId, ServerInfo_Player playerInfo);
    void createZoneForPlayer(Player *newPlayer, int playerId);

    void startGame(bool resuming);
    void stopGame();
    void closeGame();
    bool leaveGame();

    Player *setActivePlayer(int id);
    Player *setPriorityPlayer(int id);
    void setActivePhase(int phase);
    void createMenuItems();
    void createReplayMenuItems();
    void createViewMenuItems();
    void registerDockWidget(QMenu *_viewMenu, QDockWidget *widget, const QSize &defaultSize);
    void createCardInfoDock(bool bReplay = false);
    void createPlayerListDock(bool bReplay = false);
    void createMessageDock(bool bReplay = false);
    void createPlayAreaWidget(bool bReplay = false);
    void createDeckViewContainerWidget(bool bReplay = false);
    void createReplayDock(GameReplay *replay);
    void clearRuledCombatArrows();
    void refreshRuledCombatArrows();
    /// Recompute the ruled prompt panel's exclusive mode from the view model and push it as one
    /// state. The single place the mode priority is decided; returns the mode it pushed.
    GamePromptWidget::PromptMode refreshRuledPromptState();
    void publishRuledAutoPassPolicy();
    void ensureStackWindow();
    void saveStackWindowLayout();
    CardZoneLogic *findVisibleStackZone() const;
    void syncStackWindowVisibility();
signals:
    void gameClosing(TabGame *tab);
    void containerProcessingStarted(const GameEventContext &context);
    void containerProcessingDone();
    void openMessageDialog(const QString &userName, bool focus);
    void openDeckEditor(const LoadedDeck &deck);
    void notIdle();

    void phaseChanged(int phase);
    void gameLeft();
    void chatMessageSent(QString chatMessage);
    void turnAdvanced();
    void arrowDeletionRequested(int arrowId);

private slots:
    void adminLockChanged(bool lock);
    void newCardAdded(AbstractCardItem *card);
    void setCardMenu(QMenu *menu);

    void actGameInfo();
    void actConcede();
    void actRemoveLocalArrows();
    void actRotateViewCW();
    void actRotateViewCCW();
    void actToggleStackWindow();
    void actSay();
    /// Fork: parse one dev-console line and send it. The only new upstream slot the console needs;
    /// everything it does beyond the send lives in RuledDevCommandParser.
    void actDevConsoleCommand(const QString &line);
    void actPhaseAction();
    void actNextPhase();
    void actNextPhaseAction();

    void addMentionTag(const QString &value);
    void linkCardToChat(const QString &cardName);

    void refreshShortcuts();

    void loadLayout();
    void actCompleterChanged();
    void notifyPlayerJoin(QString playerName);
    void notifyPlayerKicked();
    void processPlayerLeave(Player *leavingPlayer);
    void actResetLayout();

    void hideEvent(QHideEvent *event) override;

    /// Opens the local player's deck zone view for the LibrarySearch pick (Gifts Ungiven step 1).
    void onRuledLibrarySearchPickStarted(QStringList candidateNames, QVector<int> serverCardIds);
    /// Creates or closes the revealed-cards popup for RevealedCards pick (Gifts Ungiven step 2).
    void onRuledRevealedPickChanged(bool started, QStringList cardNames, QVector<int> serverCardIds, int min, int max);
    /// Reconciles the one engine-authored public hand reveal as an exact snapshot.
    void onRuledPublicRevealChanged(bool active,
                                    quint32 sourceObjectId,
                                    int zoneOwnerPlayerId,
                                    QStringList cardNames,
                                    QVector<int> serverCardIds);
    void onRuledActivePublicRevealsChanged(QStringList cardNames, QVector<int> revealingPlayerIds);
    void onRuledExilePlayPermissionGroupsChanged();
    /// CR 603.3b: opens or closes the simultaneous-trigger ordering window. Only the deciding
    /// player is sent `active = true`, so at most one client shows it.
    void onRuledTriggerOrderUiChanged(bool active, QVector<RuledTriggerOrderCandidate> candidates);

protected slots:
    void closeEvent(QCloseEvent *event) override;

public:
    TabGame(TabSupervisor *_tabSupervisor,
            QList<AbstractClient *> &_clients,
            const Event_GameJoined &event,
            const QMap<int, QString> &_roomGameTypes);
    void connectToGameState();
    void connectToPlayerManager();
    void connectToGameEventHandler();
    void connectMessageLogToGameEventHandler();
    void connectPlayerListToGameEventHandler();
    TabGame(TabSupervisor *_tabSupervisor, GameReplay *replay);
    ~TabGame() override;
    void retranslateUi() override;
    void updatePlayerListDockTitle();
    bool closeRequest() override;

    [[nodiscard]] QString getTabText() const override;

    [[nodiscard]] AbstractGame *getGame() const
    {
        return game;
    }

    /// When the ruled stack window is open, stack `CardItem`s live in the zone view (visible positions).
    /// Returns nullptr if the window is closed or the id is not found in the view.
    [[nodiscard]] CardItem *findVisibleStackSpellCardItem(int serverCardId) const;

    /// As above for a graveyard pile: when `playerId`'s graveyard is open in a zone view, its
    /// `CardItem`s have visible positions a targeting arrow can point at. Returns nullptr when the
    /// view is closed, in which case the caller should fall back to the pile itself.
    [[nodiscard]] CardItem *findVisibleGraveyardCardItem(int playerId, int serverCardId) const;

public slots:
    void viewCardInfo(const CardRef &cardRef = {}) const;
    void resetChatAndPhase();
    void updateTimeElapsedLabel(QString newTime);
    void addPlayerToAutoCompleteList(QString playerName);
    void removePlayerFromAutoCompleteList(QString playerName);
    void removeSpectator(int spectatorId, ServerInfo_User spectator);
    void processLocalPlayerSideboardLocked(int playerId, bool sideboardLocked);
    void processLocalPlayerReadyStateChanged(int playerId, bool ready);
    void emitUserEvent();
};

#endif
