/**
 * @file ruled_autopilot.h
 * @ingroup Ruled
 * @brief Dev-loop autopilot: drives one client from login to "ready" without any clicking.
 *
 * Fork-owned. Exists so `scripts/launch-ruled-game.ps1` can bring up a complete two-seat ruled
 * game with one command — the pre-game sequence (join room, create/join game, load deck, ready)
 * is pure ceremony that has to be repeated by hand for every manual verification.
 *
 * It is a *driver*, not a player: it stops the moment the game starts, so everything after the
 * opening hand is the real UI being exercised by a human. It never fakes engine state and sends
 * only the same commands the buttons send.
 *
 * Off unless `--autopilot host|join` is passed, so a normally-launched client is untouched.
 */

#ifndef RULED_AUTOPILOT_H
#define RULED_AUTOPILOT_H

#include <QLoggingCategory>
#include <QObject>
#include <QPointer>
#include <QString>

class AbstractClient;
class DeckViewContainer;
class Event_GameJoined;
class MainWindow;
class ServerInfo_PlayerProperties;
class TabSupervisor;

inline Q_LOGGING_CATEGORY(RuledAutopilotLog, "ruled_autopilot");

/**
 * Drives one client through the pre-game sequence. One instance per client process, parented to
 * the MainWindow.
 */
class RuledAutopilot : public QObject
{
    Q_OBJECT
public:
    /// What the autopilot should do once logged in.
    struct Config
    {
        bool enabled = false;
        /// true: create the game. false: join the game the host created (matched by name).
        bool host = false;
        /// Deck file handed to the normal client-side deck loader (.cod, .dec, .txt, …).
        QString deckFile;
        /// Game description, and the key the joining seat matches on.
        QString gameName;
        /// Whose games the joining seat asks the server about. Ignored when hosting.
        QString hostUser;
        /// Create the game with the ruled engine enabled.
        bool ruled = true;
    };

    /**
     * The single entry point main.cpp calls. Parses the command line values, and installs an
     * autopilot on @p window when a role was given. A no-op otherwise, so the normal launch path
     * costs one call and nothing else.
     */
    static void installFromCommandLine(MainWindow *window, const QString &role, const QString &deckFile);

    RuledAutopilot(TabSupervisor *supervisor, Config config, QObject *parent = nullptr);

private:
    /// Where we are in the pre-game sequence. Strictly forward-moving; any failure logs and parks
    /// in Done rather than retrying forever.
    enum class Stage
    {
        WaitingForLogin,
        WaitingForRoom,
        WaitingForGame,
        PreparingDeck,
        WaitingForDeckAck,
        Done
    };

    void handleGameJoined(const Event_GameJoined &event);
    void handlePlayerProperties(const ServerInfo_PlayerProperties &prop, int propPlayerId);

    /// Waits for the lobby room the server auto-joins us into, then hosts or starts hunting for
    /// the host's game. Polled rather than signal-driven: the room tab is built from a response
    /// this object never sees.
    void pollForRoom();
    void createGame(int roomId);
    /// Asks the server which games the host is in until the target one shows up. Polling rather
    /// than listening for game-list events, because a game created before this client finished
    /// logging in arrives in the join-room response instead of an event, and would be missed.
    void pollForHostGame();
    void joinGame(int roomId, int targetGameId);
    /// Locates this seat's deck view and starts the deck load. Retries briefly: we race the
    /// TabSupervisor slot that builds the tab off the same signal.
    void prepareDeck();
    void giveUp(const QString &reason);

    TabSupervisor *supervisor;
    AbstractClient *client;
    Config config;

    Stage stage = Stage::WaitingForLogin;
    int gameId = -1;
    int playerId = -1;
    int roomPollAttempts = 0;
    int gamePollAttempts = 0;
    int deckViewAttempts = 0;
    /// One outstanding game probe at a time, so a slow response cannot stack up duplicates.
    bool probeInFlight = false;
    /// Ready is sent exactly once. A failed ruled deck validation un-readies both players, and
    /// re-readying on that would loop the game start against an unimplemented card forever.
    bool readySent = false;
    QPointer<DeckViewContainer> deckView;
};

#endif // RULED_AUTOPILOT_H
