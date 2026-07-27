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
#include "../game/player/player.h"
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
    QList<QPointer<ArrowItem>> ruledCombatArrows;
    /// The graveyard view a pending trigger opened for us, so it can be closed again without
    /// touching one the player opened themselves. Null when we have none open.
    QPointer<ZoneViewWidget> ruledAutoOpenedGraveyardView;
    QPointer<ZoneViewWidget> stackView;
    CardZoneLogic *stackViewZone = nullptr;
    // Deck zone view auto-opened for LibrarySearch (Gifts Ungiven search step).
    QPointer<ZoneViewWidget> librarySearchView;
    // ServerInfo_Card storage for the library search popup (owned by this instance).
    QList<ServerInfo_Card *> librarySearchCards;
    // Revealed-cards popup shown during RevealedCards pick (Gifts Ungiven opponent step).
    QPointer<ZoneViewWidget> revealedPickView;
    // ServerInfo_Card storage for the revealed-cards popup (owned by this instance).
    QList<ServerInfo_Card *> revealedPickCards;
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
    void onRuledRevealedPickChanged(bool started, QStringList cardNames, QVector<int> serverCardIds,
                                    int min, int max);

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
