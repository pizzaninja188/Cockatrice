#ifndef RULED_GAME_SESSION_H
#define RULED_GAME_SESSION_H

// Fork-owned synchronous lifecycle for one tricerules sidecar session. RuledGameDriver remains
// the public Server_Game facade; this collaborator owns transport, deck admission, canonical
// command envelopes, and per-session policy state without touching physical card identity.

#include <QByteArray>
#include <QHash>
#include <QList>
#include <QString>
#include <QStringList>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <memory>

class RulesRelay;
class Server_Game;
class RuledServerDiagnostics;
class RuledGameResume;

class RuledGameSession
{
    friend class RuledBatchTest;

public:
    enum class StartDisposition
    {
        Blocked,
        Started
    };

    struct StartResult
    {
        StartDisposition disposition = StartDisposition::Blocked;
        ruled::v1::IpcResponse response;
        QList<ruled::v1::PlayerDeck> deckByPlayer;
        quint64 seed = 0;
        QString cardDataHash;
    };

    explicit RuledGameSession(Server_Game *game);
    ~RuledGameSession();

    bool validateDecksForStart();
    StartResult start();
    void resetForNewGame();
    void end();
    void abort();

    [[nodiscard]] bool isActive() const;
    RuledServerDiagnostics *diagnostics() const
    {
        return capture.get();
    }
    const RuledGameResume &resume() const
    {
        return *resumePlan;
    }
    void failResume(const QString &reason);
    bool playerCommand(int playerId, const QByteArray &payload, ruled::v1::IpcResponse &response);
    bool previewPayment(int playerId, const ruled::v1::PreviewPayment &preview, ruled::v1::IpcResponse &response);
    void handleConnectionLost();
    void handleDepartureRejected(const QString &reason);

    bool cacheAutoPassPolicy(int playerId, const ruled::v1::SetAutoPassPolicy &policy);
    [[nodiscard]] QByteArray canonicalGameplayCommand(int playerId, const ruled::v1::RuledCommand &command) const;

private:
    [[nodiscard]] QList<ruled::v1::PlayerDeck> playerDecksByPlayer() const;
    void notifyUnimplementedCards(const QList<ruled::v1::PlayerDeck> &deckByPlayer, const QStringList &missingNames);
    void sendEngineNotice(const QString &title, const QString &message);
    void notifyEngineUnreachable();

    Server_Game *const game;
    std::unique_ptr<RuledGameResume> resumePlan;
    std::unique_ptr<RuledServerDiagnostics> capture;
    std::unique_ptr<RulesRelay> relay;
    quint64 seed = 0;
    bool engineConnectionLost = false;
    QHash<int, ruled::v1::SetAutoPassPolicy> autoPassPolicies;
};

#endif
