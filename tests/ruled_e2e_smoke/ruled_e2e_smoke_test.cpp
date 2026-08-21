// Ruled E2E smoke test (refactor roadmap Step 3).
//
// Launches the real tricerules-server sidecar and a real servatrice instance, then drives two
// scripted protobuf-level clients (raw QTcpSocket, no Qt GUI) through one fixed seeded ruled
// game. This is the only layer that exercises servatrice relay + sidecar + protocol wiring
// *together*; it exists as the safety net for the Step 4/5 extraction refactors.
//
// Covered end-to-end:
//   * deck validation gate: an unimplemented card (Black Lotus) blocks game start with an
//     Event_NotifyUser CUSTOM popup and un-readies players; swapping the deck unblocks it
//   * SessionStart with a forced seed (COCKATRICE_RULED_SEED) — verified via servatrice's
//     stderr log line (the seed is deliberately never broadcast to clients: a client that
//     knows the seed could predict shuffles)
//   * opening: ChooseStartingPlayer, one London mulligan + PutOpeningHandOnBottom, keeps
//   * land plays (battlefield object map gains basic lands)
//   * targeted casts: Lightning Bolt and modal Boros Charm at the opponent (StackPushed
//     targets/chosen-mode metadata, resolve, LifeChanged)
//   * one combat: DeclareAttackers (Hill Giant), empty DeclareBlockers, combat LifeChanged
//   * a tier-3 resolution choice: Brainstorm's ordered 2-card put-back
//     (ResolutionChoiceRequired / SubmitResolutionChoice)
//   * cleanup discard: DiscardToHandSize back down to 7
//   * dev commands (backlog dev-loop piece 2): a conjured Serra Angel — in neither decklist, so
//     it drives the mid-game catalog refresh and the minted Server_Card — and added mana. This
//     is the only cross-language check that a C++-built DevCommand decodes and applies in Rust.
//   * continuous control change: Act of Treason moves one physical permanent to the caster's
//     TABLE for the turn, grants haste, then returns it to the owner's TABLE at cleanup
//   * battlefield-to-library placement: Totally Lost moves that permanent to the top of its
//     owner's private DECK; both clients see the public move and only the owner sees its next draw
//   * player-attached Aura: Curse of Disturbance targets the other seat, stays on its controller's
//     TABLE, and publishes the same typed recipient / physical mapping to both clients
//
// Scope: engine + relay + protocol wiring only — no Qt client UI logic (that arrives with the
// Step 5 headless client-core suite).
//
// The scripted clients are *reactive*: they act on structured LegalHandAction records, display labels, zone
// views, and object maps rather than a hardcoded move list, so the script stays legal for any
// shuffle; the fixed seed pins the exact stream for reproducible debugging.
//
// Scenario roles (decided by login name, independent of who the engine picks as chooser):
//   smokep1 "aggressor": 24 Mountain / 8 Hill Giant / 8 Lightning Bolt. Always ends up the
//     starting player (whoever gets the opening pick arranges it). Plays a land every turn,
//     bolts the opponent's face once, casts one Hill Giant, attacks every combat.
//   smokep2 "hoarder":   20 Island / 12 Brainstorm / 8 Merfolk of the Pearl Trident.
//     Mulligans once (bottoms hand index 0), plays exactly one Island, casts one Brainstorm,
//     never blocks, otherwise hoards cards until cleanup forces a discard.
//
// Binary discovery: RULED_E2E_SERVATRICE / RULED_E2E_TRICERULES env vars override the
// CMake-injected paths. If either binary is missing the test SKIPs (set RULED_E2E_REQUIRE=1
// to turn that into a failure).

#include <QByteArray>
#include <QCoreApplication>
#include <QDir>
#include <QElapsedTimer>
#include <QFile>
#include <QProcess>
#include <QProcessEnvironment>
#include <QRegularExpression>
#include <QString>
#include <QStringList>
#include <QTcpSocket>
#include <QTemporaryDir>
#include <gtest/gtest.h>

#include <array>

#include <libcockatrice/protocol/pb/command_deck_select.pb.h>
#include <libcockatrice/protocol/pb/command_ready_start.pb.h>
#include <libcockatrice/protocol/pb/command_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/commands.pb.h>
#include <libcockatrice/protocol/pb/event_flip_card.pb.h>
#include <libcockatrice/protocol/pb/event_game_joined.pb.h>
#include <libcockatrice/protocol/pb/event_game_state_changed.pb.h>
#include <libcockatrice/protocol/pb/event_list_games.pb.h>
#include <libcockatrice/protocol/pb/event_list_rooms.pb.h>
#include <libcockatrice/protocol/pb/event_move_card.pb.h>
#include <libcockatrice/protocol/pb/event_notify_user.pb.h>
#include <libcockatrice/protocol/pb/event_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/event_set_card_attr.pb.h>
#include <libcockatrice/protocol/pb/game_commands.pb.h>
#include <libcockatrice/protocol/pb/game_event.pb.h>
#include <libcockatrice/utility/zone_names.h>
#include <libcockatrice/protocol/pb/game_event_container.pb.h>
#include <libcockatrice/protocol/pb/response.pb.h>
#include <libcockatrice/protocol/pb/room_commands.pb.h>
#include <libcockatrice/protocol/pb/room_event.pb.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <libcockatrice/protocol/pb/server_message.pb.h>
#include <libcockatrice/protocol/pb/session_commands.pb.h>
#include <libcockatrice/protocol/pb/session_event.pb.h>

#include <algorithm>
#include <map>
#include <optional>
#include <set>
#include <string>
#include <vector>

namespace
{

constexpr quint64 kForcedSeed = 421700421700ULL;
constexpr int kServatricePort = 47997;
constexpr int kTriceRulesPort = 17391;
constexpr int kOverallDeadlineMs = 120 * 1000;

QString envOr(const char *name, const QString &fallback)
{
    const QByteArray v = qgetenv(name);
    return v.isEmpty() ? fallback : QString::fromLocal8Bit(v);
}

QString servatriceExePath()
{
#ifdef RULED_E2E_SERVATRICE_PATH
    return envOr("RULED_E2E_SERVATRICE", QStringLiteral(RULED_E2E_SERVATRICE_PATH));
#else
    return envOr("RULED_E2E_SERVATRICE", QString());
#endif
}

QString triceRulesExePath()
{
#ifdef RULED_E2E_TRICERULES_PATH
    return envOr("RULED_E2E_TRICERULES", QStringLiteral(RULED_E2E_TRICERULES_PATH));
#else
    return envOr("RULED_E2E_TRICERULES", QString());
#endif
}

QString deckXml(const std::vector<std::pair<int, QString>> &mainboard)
{
    QString cards;
    for (const auto &entry : mainboard) {
        cards += QStringLiteral("<card number=\"%1\" name=\"%2\"/>").arg(entry.first).arg(entry.second);
    }
    return QStringLiteral("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"
                          "<cockatrice_deck version=\"1\"><deckname>smoke</deckname><comments></comments>"
                          "<zone name=\"main\">%1</zone></cockatrice_deck>")
        .arg(cards);
}

/// Waits until a TCP connect to 127.0.0.1:port succeeds (the process is accepting) or timeout.
bool waitForPortOpen(int port, int timeoutMs)
{
    QElapsedTimer t;
    t.start();
    while (t.elapsed() < timeoutMs) {
        QTcpSocket probe;
        probe.connectToHost(QStringLiteral("127.0.0.1"), static_cast<quint16>(port));
        if (probe.waitForConnected(300)) {
            probe.disconnectFromHost();
            return true;
        }
    }
    return false;
}

/// One scripted protobuf-level participant: framing, login, room/game plumbing, and a
/// reactive per-batch policy driven by the engine's LegalActions.
class SmokeClient
{
public:
    enum class Role
    {
        Aggressor, // smokep1: mono-red; starts, bolts, attacks
        Hoarder,   // smokep2: mono-blue; mulligans, brainstorms, discards
    };

    SmokeClient(Role role, QString userName, QStringList *transcript)
        : role(role), userName(std::move(userName)), transcript(transcript)
    {
    }

    Role role;
    QString userName;
    QStringList *transcript;

    QTcpSocket sock;
    QByteArray inBuf;
    bool sawHandshakeGarbage = false;
    quint64 nextCmdId = 1;

    // Session / pregame state
    int roomId = -1;
    int gameId = -1;
    int myId = -1;
    int oppId = -1;
    bool gameStarted = false;
    int notifyCustomCount = 0;
    QString lastNotifyContent;

    // Latest ruled view (rebuilt from each Event_RuledPayload batch)
    quint64 stateVersion = 0;
    quint64 lastActedVersion = 0;
    ruled::v1::PhaseId phase = ruled::v1::PHASE_ID_UNSPECIFIED;
    int activePlayer = -1;
    int priorityPlayer = -1;
    int stackDepth = 0;
    bool sawDirectOpeningToMain1 = false;
    int directSettledActivePlayer = -1;
    QStringList labels;
    std::map<int, int> handSizeByPlayer;
    std::map<int, int> lifeByPlayer;
    struct Permanent
    {
        QString cardId;
        quint32 oid = 0;
        bool tapped = false;
        bool creature = false;
        bool sick = false;
        bool haste = false;
        bool reach = false;
        int power = 0;
        int toughness = 0;
        int faceIndex = 0;
        bool faceDown = false;
        quint64 generation = 0;
        int attachmentPlayerId = -1;
        std::array<bool, 2> roomDoors{false, false};
        int roomDoorCount = 0;
    };
    std::map<int, std::vector<Permanent>> battlefieldByPlayer;
    struct Pool
    {
        int w = 0, u = 0, b = 0, r = 0, g = 0, c = 0;
        int total() const
        {
            return w + u + b + r + g + c;
        }
    };
    Pool myPool;
    std::optional<ruled::v1::ResolutionChoiceRequired> pendingChoice;
    // CR 603.3b: the engine blocks on this until it is answered, so the bot must handle it or the
    // whole game deadlocks — every simultaneous multi-trigger board reaches it.
    std::optional<ruled::v1::TriggerOrderRequired> pendingTriggerOrder;

    // Policy progress flags
    bool didMulligan = false;
    bool boltCast = false;
    bool borosCharmCast = false;
    bool giantCast = false;
    bool brainstormCast = false;
    bool softCounterConvoluteConjured = false;
    bool softCounterOrbRemoved = false;
    bool softCounterManaGranted = false;
    bool softCounterBoltConjured = false;
    bool softCounterBoltCast = false;
    bool softCounterConvoluteCast = false;
    bool sawSoftCounterPaymentChoice = false;
    bool activatedManaDuringSoftCounterPayment = false;
    bool paidSoftCounter = false;
    quint32 softCounterConvoluteOid = 0;
    bool softCounterLeftStackBeforeChoice = false;
    bool sawSoftCounterResolveAfterChoice = false;
    quint32 latestBoltOid = 0;
    bool devCurseConjureSent = false;
    bool devCurseManaSent = false;
    bool curseCast = false;
    bool devManifestSpellConjured = false;
    bool devManifestManaSent = false;
    bool manifestSpellCast = false;
    bool sawManifestChoicePrivate = false;
    bool sawManifestChoiceRedacted = false;
    bool submittedManifestChoice = false;
    bool sawManifestPublicFaceDown = false;
    bool sawManifestPrivateIdentity = false;
    bool sawOpponentManifestIdentityEmpty = false;
    bool turnManifestFaceUpSent = false;
    bool sawManifestFaceChanged = false;
    bool sawManifestPhysicalFaceDown = false;
    bool sawManifestPhysicalFaceUp = false;
    bool sawManifestPhysicalFaceUpIdentity = false;
    quint32 manifestOid = 0;
    quint64 manifestGeneration = 0;
    int manifestServerCardId = -1;
    bool devRoomConjureSent = false;
    bool devRoomManaSent = false;
    bool roomCast = false;
    bool roomUnlockSent = false;
    bool sawRoomCastDoorState = false;
    bool sawRoomFullyUnlocked = false;
    bool sawRoomUnlockTrigger = false;
    bool roomPhysicalIdentityContinuous = true;
    quint32 roomOid = 0;
    quint64 roomGeneration = 0;
    int roomServerCardId = -1;
    // Flashback (CR 702.34) exercises the one relay path nothing else covers: the physical
    // card is sourced from the GRAVE pile rather than the hand. Tracked through the freeform
    // Event_MoveCard stream, because the ruled batch looks identical whether or not the relay
    // actually moved the right card — that is exactly how a wrong-card bug got shipped.
    bool devFlashbackConjureSent = false;
    bool devFlashbackMoveSent = false;
    bool devFlashbackManaSent = false;
    bool flashbackCast = false;
    bool sawFlashbackGraveToStack = false;
    bool sawFlashbackStackToExile = false;
    bool devTypecyclingConjureSent = false;
    bool devTypecyclingManaSent = false;
    bool typecyclingActivated = false;
    bool submittedTypecyclingChoice = false;
    bool sawTypecyclingHandToGrave = false;
    bool sawTypecyclingDeckToHand = false;
    bool typecyclingPhysicalIdentityContinuous = true;
    int typecyclingSourcePhysicalId = -1;
    int typecyclingChosenPhysicalId = -1;
    bool sawOwnTypecyclingAction = false;
    bool sawOpponentTypecyclingActionRedacted = false;
    bool devEmptyTypecyclingConjureSent = false;
    bool devEmptyTypecyclingManaSent = false;
    bool emptyTypecyclingActivated = false;
    bool sawEmptyTypecyclingChoice = false;
    bool submittedEmptyTypecyclingChoice = false;
    bool devRenewConjureSent = false;
    bool devRenewMoveSent = false;
    bool devRenewManaSent = false;
    bool renewActivated = false;
    bool sawOwnRenewAction = false;
    bool sawOpponentRenewActionRedacted = false;
    bool sawRenewGraveToExile = false;
    bool sawRenewCounters = false;
    bool renewPhysicalIdentityContinuous = true;
    int renewSourcePhysicalId = -1;
    bool devAdventureConjureSent = false;
    bool devAdventureManaSent = false;
    bool stompCast = false;
    bool giantCastFromExile = false;
    bool sawAdventureStackToExile = false;
    bool sawAdventureExileToStack = false;
    bool sawAdventureStackToBattlefield = false;
    bool adventurePhysicalIdentityContinuous = true;
    int adventurePhysicalCardId = -1;
    bool attackersSentThisCombat = false;
    bool blockersSentThisCombat = false;
    bool devConjureSent = false;
    bool devWaifSent = false;
    bool devBorosCharmSent = false;
    bool devManaSent = false;
    bool devAntiVenomSent = false;
    bool devOrbSent = false;
    bool devDiregrafSent = false;
    bool devDiregrafRemoved = false;
    bool devBorosCharmManaSent = false;
    bool devPreventionSalveSent = false;
    bool devPreventionBlazeSent = false;
    bool devPreventionManaSent = false;
    bool preventionSalveCast = false;
    bool preventionBlazeCast = false;
    bool devProtectionBlessingSent = false;
    bool devProtectionManaSent = false;
    bool protectionBlessingCast = false;
    bool sawProtectionBranchChoice = false;
    bool submittedProtectionBranchChoice = false;
    bool sawProtectionHandToStack = false;
    bool protectionLeftStackBeforeChoice = false;
    bool sawProtectionStackToGraveAfterChoice = false;
    bool sawProtectionPhysicalAnnotation = false;
    quint32 protectionTargetOid = 0;
    bool devControlTargetSent = false;
    bool devActOfTreasonSent = false;
    bool devControlManaSent = false;
    bool actOfTreasonCast = false;
    quint32 controlTargetOid = 0;
    bool sawControlTransfer = false;
    bool sawControlReturn = false;
    bool sawPhysicalControlTransfer = false;
    bool sawPhysicalControlReturn = false;
    bool devTotallyLostSent = false;
    bool devTotallyLostManaSent = false;
    bool totallyLostCast = false;
    bool sawLibraryPermanentMoved = false;
    bool sawLibraryTargetAbsentFromBattlefield = false;
    bool sawTopPermanentDrawn = false;
    bool devEvolvingWildsSent = false;
    bool evolvingWildsActivated = false;
    bool sawOwnLibrarySearchCandidates = false;
    bool sawOpponentLibrarySearchRedacted = false;
    bool submittedEvolvingWildsChoice = false;
    bool sawEvolvingWildsPermanentMoved = false;
    bool sawEvolvingWildsPhysicalDeckToTable = false;
    bool evolvingWildsPhysicalIdentityContinuous = true;
    quint32 evolvingWildsChosenOid = 0;
    int evolvingWildsPhysicalCardId = -1;
    bool sawCursePlayerAttachment = false;
    quint32 curseOid = 0;
    std::map<quint32, int> serverCardByEngineOid;
    std::map<int, int> handServerCardBySlot;
    std::map<int, QString> annotationByServerCardId;

    // Milestone observations (asserted by the fixture)
    bool sawBoltPushWithTarget = false;
    bool sawBoltLifeLoss = false;
    bool sawBorosCharmPushWithMode = false;
    bool sawBorosCharmLifeLoss = false;
    bool sawAttackersDeclared = false;
    bool sawCombatLifeLoss = false;
    bool sawBrainstormChoice = false;
    bool submittedBrainstormChoice = false;
    bool sawBrainstormResolved = false;
    bool sawDamagePreventionChoice = false;
    bool submittedDamagePreventionChoice = false;
    bool sawEntryReplacementChoice = false;
    bool submittedEntryReplacementChoice = false;
    bool sawDiregrafEnterTapped = false;
    bool sawOpponentCleanupDiscard = false;
    bool sawCleanupDiscardActions = false;
    bool sentCleanupDiscard = false;
    bool sawBottomAction = false;
    bool sentBottom = false;
    bool sawDevConjuredPermanent = false;
    bool sawWaifOnBattlefield = false;
    bool sawWaifFaceChanged = false;
    bool sawWaifBackPt = false;
    bool sawDevMana = false;
    bool sawBattlefieldOmission = false;
    bool sawPhysicalTap = false;
    bool sawPhysicalUntap = false;
    quint32 boltOid = 0;
    quint32 borosCharmOid = 0;
    quint32 brainstormOid = 0;
    quint32 waifOid = 0;
    bool inCombatDamageWindow = false;

    void log(const QString &line)
    {
        transcript->append(QStringLiteral("[%1] %2").arg(userName, line));
    }

    // ---- transport ----

    bool connectToServer()
    {
        sock.connectToHost(QStringLiteral("127.0.0.1"), static_cast<quint16>(kServatricePort));
        if (!sock.waitForConnected(10000)) {
            return false;
        }
        // TCP handshake: the server treats the first CommandContainer *without* a cmd_id as the
        // session-init trigger (see TcpServerSocketInterface::readClient) and silently swallows
        // everything until it arrives.
        CommandContainer hello;
        std::string bytes;
        hello.SerializeToString(&bytes);
        writeFrame(bytes);
        return true;
    }

    void writeFrame(const std::string &bytes)
    {
        QByteArray frame;
        const quint32 len = static_cast<quint32>(bytes.size());
        frame.append(static_cast<char>((len >> 24) & 0xff));
        frame.append(static_cast<char>((len >> 16) & 0xff));
        frame.append(static_cast<char>((len >> 8) & 0xff));
        frame.append(static_cast<char>(len & 0xff));
        frame.append(bytes.data(), static_cast<int>(bytes.size()));
        sock.write(frame);
        sock.flush();
    }

    void sendContainer(CommandContainer &cont)
    {
        cont.set_cmd_id(nextCmdId++);
        std::string bytes;
        cont.SerializeToString(&bytes);
        writeFrame(bytes);
    }

    /// Reads whatever is available (waiting up to waitMs) and processes complete frames.
    /// Returns true if at least one ServerMessage was processed.
    bool pump(int waitMs)
    {
        if (sock.state() != QAbstractSocket::ConnectedState) {
            return false;
        }
        if (sock.bytesAvailable() == 0) {
            sock.waitForReadyRead(waitMs);
        }
        bool processed = false;
        inBuf.append(sock.readAll());
        for (;;) {
            if (!sawHandshakeGarbage && inBuf.size() >= 4 && inBuf.startsWith("<?xm")) {
                // v14 compatibility preamble some server builds emit; skip its 60 bytes.
                if (inBuf.size() < 60) {
                    break;
                }
                inBuf.remove(0, 60);
                sawHandshakeGarbage = true;
                continue;
            }
            if (inBuf.size() < 4) {
                break;
            }
            const quint32 len = (static_cast<quint32>(static_cast<unsigned char>(inBuf[0])) << 24) +
                                (static_cast<quint32>(static_cast<unsigned char>(inBuf[1])) << 16) +
                                (static_cast<quint32>(static_cast<unsigned char>(inBuf[2])) << 8) +
                                static_cast<quint32>(static_cast<unsigned char>(inBuf[3]));
            if (static_cast<quint32>(inBuf.size()) < 4 + len) {
                break;
            }
            ServerMessage msg;
            const bool ok = msg.ParseFromArray(inBuf.constData() + 4, static_cast<int>(len));
            inBuf.remove(0, static_cast<int>(4 + len));
            if (ok) {
                handleServerMessage(msg);
                processed = true;
            }
        }
        return processed;
    }

    /// Pumps until pred() is true or timeout; fails the test on timeout.
    template <typename Pred> ::testing::AssertionResult pumpUntil(Pred pred, int timeoutMs, const char *what)
    {
        QElapsedTimer t;
        t.start();
        while (!pred()) {
            if (t.elapsed() > timeoutMs) {
                return ::testing::AssertionFailure() << userName.toStdString() << ": timeout waiting for " << what;
            }
            pump(50);
        }
        return ::testing::AssertionSuccess();
    }

    // ---- message handling ----

    std::map<quint64, Response> responses;
    std::set<quint64> ruledCmdIds;

    void handleServerMessage(const ServerMessage &msg)
    {
        switch (msg.message_type()) {
            case ServerMessage::RESPONSE:
                responses[msg.response().cmd_id()] = msg.response();
                if (msg.response().response_code() != Response::RespOk &&
                    ruledCmdIds.count(msg.response().cmd_id()) > 0) {
                    // A rejected ruled command means the policy misjudged legality; the
                    // transcript makes the wedge diagnosable if the game then stalls.
                    log(QStringLiteral("!! ruled command %1 rejected with code %2")
                            .arg(msg.response().cmd_id())
                            .arg(msg.response().response_code()));
                }
                break;
            case ServerMessage::SESSION_EVENT:
                handleSessionEvent(msg.session_event());
                break;
            case ServerMessage::GAME_EVENT_CONTAINER:
                handleGameEventContainer(msg.game_event_container());
                break;
            case ServerMessage::ROOM_EVENT:
                handleRoomEvent(msg.room_event());
                break;
            default:
                break;
        }
    }

    void handleSessionEvent(const SessionEvent &ev)
    {
        if (ev.HasExtension(Event_ListRooms::ext)) {
            const auto &lr = ev.GetExtension(Event_ListRooms::ext);
            if (roomId < 0 && lr.room_list_size() > 0) {
                roomId = lr.room_list(0).room_id();
            }
        }
        if (ev.HasExtension(Event_GameJoined::ext)) {
            const auto &gj = ev.GetExtension(Event_GameJoined::ext);
            gameId = gj.game_info().game_id();
            myId = gj.player_id();
            log(QStringLiteral("joined game %1 as player %2").arg(gameId).arg(myId));
        }
        if (ev.HasExtension(Event_NotifyUser::ext)) {
            const auto &nu = ev.GetExtension(Event_NotifyUser::ext);
            if (nu.type() == Event_NotifyUser::CUSTOM) {
                ++notifyCustomCount;
                lastNotifyContent = QString::fromStdString(nu.custom_content());
                log(QStringLiteral("NotifyUser CUSTOM: %1").arg(lastNotifyContent.left(120)));
            }
        }
    }

    std::vector<ServerInfo_Game> announcedGames;

    void handleRoomEvent(const RoomEvent &ev)
    {
        if (ev.HasExtension(Event_ListGames::ext)) {
            const auto &lg = ev.GetExtension(Event_ListGames::ext);
            for (int i = 0; i < lg.game_list_size(); ++i) {
                announcedGames.push_back(lg.game_list(i));
            }
        }
    }

    void handleGameEventContainer(const GameEventContainer &cont)
    {
        for (int i = 0; i < cont.event_list_size(); ++i) {
            const GameEvent &ev = cont.event_list(i);
            if (ev.HasExtension(Event_GameStateChanged::ext)) {
                const auto &gsc = ev.GetExtension(Event_GameStateChanged::ext);
                if (gsc.has_game_started()) {
                    gameStarted = gsc.game_started();
                }
            }
            if (ev.HasExtension(Event_MoveCard::ext)) {
                const auto &mc = ev.GetExtension(Event_MoveCard::ext);
                const QString from = QString::fromStdString(mc.start_zone());
                const QString to = QString::fromStdString(mc.target_zone());
                const QString name = QString::fromStdString(mc.card_name());
                // ZoneNames, not literals: Cockatrice's exile zone is spelled "rfg".
                const QLatin1String grave(ZoneNames::GRAVE);
                const QLatin1String stack(ZoneNames::STACK);
                const QLatin1String hand(ZoneNames::HAND);
                const QLatin1String exile(ZoneNames::EXILE);
                const QLatin1String table(ZoneNames::TABLE);
                const QLatin1String deck(ZoneNames::DECK);
                if (from == deck && to == table && mc.face_down()) {
                    sawManifestPhysicalFaceDown = true;
                    if (manifestServerCardId >= 0 && manifestServerCardId != mc.new_card_id()) {
                        ADD_FAILURE() << "manifest-dread face-down move changed physical card id";
                    }
                    manifestServerCardId = mc.new_card_id();
                }
                if (name == QLatin1String("Mountain") && from == deck && to == table) {
                    sawEvolvingWildsPhysicalDeckToTable = true;
                    if (evolvingWildsPhysicalCardId >= 0 &&
                        mc.new_card_id() != evolvingWildsPhysicalCardId) {
                        evolvingWildsPhysicalIdentityContinuous = false;
                    }
                    evolvingWildsPhysicalCardId = mc.new_card_id();
                }
                if (typecyclingActivated && !sawTypecyclingHandToGrave && from == hand && to == grave) {
                    sawTypecyclingHandToGrave = true;
                    typecyclingPhysicalIdentityContinuous =
                        typecyclingSourcePhysicalId >= 0 && mc.card_id() == typecyclingSourcePhysicalId;
                }
                if (submittedTypecyclingChoice && !sawTypecyclingDeckToHand && from == deck && to == hand) {
                    sawTypecyclingDeckToHand = true;
                    typecyclingPhysicalIdentityContinuous =
                        typecyclingPhysicalIdentityContinuous && typecyclingChosenPhysicalId >= 0 &&
                        mc.card_id() == typecyclingChosenPhysicalId;
                }
                if (renewActivated && !sawRenewGraveToExile && from == grave && to == exile) {
                    sawRenewGraveToExile = true;
                    renewPhysicalIdentityContinuous = renewSourcePhysicalId >= 0 && mc.card_id() == renewSourcePhysicalId;
                }
                if (name == QLatin1String("Apostle's Blessing") &&
                    (mc.start_player_id() == myId || mc.target_player_id() == myId)) {
                    if (from == hand && to == stack) {
                        sawProtectionHandToStack = true;
                    } else if (from == stack && to == grave) {
                        protectionLeftStackBeforeChoice =
                            protectionLeftStackBeforeChoice || !submittedProtectionBranchChoice;
                        sawProtectionStackToGraveAfterChoice =
                            sawProtectionStackToGraveAfterChoice || submittedProtectionBranchChoice;
                    }
                }
                if (name == QLatin1String("Grizzly Bears") && from == table && to == table) {
                    if (mc.start_player_id() == oppId && mc.target_player_id() == myId) {
                        sawPhysicalControlTransfer = true;
                    }
                    if (sawPhysicalControlTransfer && mc.start_player_id() == myId && mc.target_player_id() == oppId) {
                        sawPhysicalControlReturn = true;
                    }
                }
                if (name.contains(QLatin1String("Bonecrusher Giant"))) {
                    const QLatin1String hand(ZoneNames::HAND);
                    auto followPhysicalCard = [&] {
                        if (adventurePhysicalCardId >= 0 && mc.card_id() != adventurePhysicalCardId) {
                            adventurePhysicalIdentityContinuous = false;
                        }
                        adventurePhysicalCardId = mc.new_card_id();
                    };
                    if (from == hand && to == stack && mc.start_player_id() == myId) {
                        adventurePhysicalCardId = mc.new_card_id();
                    } else if (from == stack && to == exile && mc.target_player_id() == myId) {
                        followPhysicalCard();
                        sawAdventureStackToExile = true;
                    } else if (from == exile && to == stack && mc.start_player_id() == myId) {
                        followPhysicalCard();
                        sawAdventureExileToStack = true;
                    } else if (from == stack && to == table && mc.target_player_id() == myId) {
                        followPhysicalCard();
                        sawAdventureStackToBattlefield = true;
                    }
                } else if (name == QLatin1String("Bump in the Night")) {
                    // Scope to THIS seat's card. Every client sees both seats' moves, so an
                    // unscoped flag would be satisfied by the other seat's successful flashback
                    // and hide a rejected one — which is the whole failure being tested for.
                    // Casting leaves this seat's graveyard; resolving lands in this seat's exile.
                    if (from == grave && to == stack && mc.start_player_id() == myId) {
                        sawFlashbackGraveToStack = true;
                        log(QStringLiteral("flashback: '%1' grave -> stack (mine)").arg(name));
                    }
                    if (from == stack && to == exile && mc.target_player_id() == myId) {
                        sawFlashbackStackToExile = true;
                        log(QStringLiteral("flashback: '%1' stack -> exile (mine)").arg(name));
                    }
                } else if ((from == grave || to == exile) && name != QLatin1String("Sagu Pummeler")) {
                    // Any *other* card taking the flashback path is the wrong-card bug.
                    ADD_FAILURE() << "unexpected card on the flashback path: "
                                  << name.toStdString() << " " << from.toStdString() << " -> "
                                  << to.toStdString();
                }
            }
            if (ev.HasExtension(Event_SetCardAttr::ext)) {
                const auto &attr = ev.GetExtension(Event_SetCardAttr::ext);
                if (attr.attribute() == AttrTapped) {
                    sawPhysicalTap = sawPhysicalTap || attr.attr_value() == "1";
                    sawPhysicalUntap = sawPhysicalUntap || attr.attr_value() == "0";
                } else if (attr.attribute() == AttrAnnotation) {
                    annotationByServerCardId[attr.card_id()] = QString::fromStdString(attr.attr_value());
                    sawProtectionPhysicalAnnotation =
                        sawProtectionPhysicalAnnotation ||
                        QString::fromStdString(attr.attr_value()).contains(
                        QStringLiteral("Protection from artifacts"));
                } else if (attr.attribute() == AttrFaceDown && manifestServerCardId >= 0 &&
                           attr.card_id() == manifestServerCardId) {
                    sawManifestPhysicalFaceDown = sawManifestPhysicalFaceDown || attr.attr_value() == "1";
                    sawManifestPhysicalFaceUp = sawManifestPhysicalFaceUp || attr.attr_value() == "0";
                }
            }
            if (ev.HasExtension(Event_FlipCard::ext)) {
                const auto &flip = ev.GetExtension(Event_FlipCard::ext);
                if (manifestServerCardId >= 0 && flip.card_id() == manifestServerCardId && !flip.face_down() &&
                    flip.card_name() == "Hill Giant") {
                    sawManifestPhysicalFaceUp = true;
                    sawManifestPhysicalFaceUpIdentity = true;
                }
            }
            if (ev.HasExtension(Event_RuledPayload::ext)) {
                ruled::v1::RuledEventBatch batch;
                if (batch.ParseFromString(ev.GetExtension(Event_RuledPayload::ext).payload())) {
                    applyRuledBatch(batch);
                }
            }
        }
    }

    void applyRuledBatch(const ruled::v1::RuledEventBatch &batch)
    {
        ++stateVersion;
        const ruled::v1::PhaseId previousPhase = phase;
        int phaseEvents = 0;
        bool batchDeclaredAttackers = false;
        bool batchCombatDamage = false;
        for (const ruled::v1::RuledEvent &ev : batch.events()) {
            if (ev.has_phase_changed()) {
                ++phaseEvents;
                const ruled::v1::PhaseId newPhase = ev.phase_changed().phase_id();
                if (newPhase != phase) {
                    log(QStringLiteral("phase: %1 (active %2)")
                            .arg(QString::fromStdString(ruled::v1::PhaseId_Name(newPhase)))
                            .arg(ev.phase_changed().active_player_id()));
                }
                phase = newPhase;
                activePlayer = ev.phase_changed().active_player_id();
                if (phase == ruled::v1::PHASE_ID_UNTAP || phase == ruled::v1::PHASE_ID_DECLARE_ATTACKERS) {
                    attackersSentThisCombat = false;
                    blockersSentThisCombat = false;
                }
                inCombatDamageWindow = phase == ruled::v1::PHASE_ID_COMBAT_DAMAGE ||
                                       phase == ruled::v1::PHASE_ID_FIRST_STRIKE_DAMAGE;
            } else if (ev.has_priority_changed()) {
                priorityPlayer = ev.priority_changed().player_id();
            } else if (ev.has_stack_pushed()) {
                ++stackDepth;
                const auto &sp = ev.stack_pushed();
                const QString cardId = QString::fromStdString(sp.card_id());
                log(QStringLiteral("stack push oid %1 card '%2' targets %3")
                        .arg(sp.object_id())
                        .arg(cardId)
                        .arg(sp.targets_size()));
                if (cardId == QLatin1String("lightning_bolt") && sp.targets_size() > 0) {
                    sawBoltPushWithTarget = true;
                    boltOid = sp.object_id();
                    latestBoltOid = sp.object_id();
                }
                if (cardId == QLatin1String("boros_charm") && sp.targets_size() == 1 &&
                    sp.chosen_mode_indices_size() == 1 && sp.chosen_mode_indices(0) == 0 &&
                    sp.chosen_mode_labels_size() == 1) {
                    sawBorosCharmPushWithMode = true;
                    borosCharmOid = sp.object_id();
                }
                if (cardId == QLatin1String("brainstorm")) {
                    brainstormOid = sp.object_id();
                }
                if (cardId == QLatin1String("convolute")) {
                    softCounterConvoluteOid = sp.object_id();
                }
                if (sp.is_triggered() &&
                    QString::fromStdString(sp.ability_annotation()).contains(
                        QStringLiteral("draw two cards"), Qt::CaseInsensitive)) {
                    sawRoomUnlockTrigger = true;
                }
            } else if (ev.has_stack_resolved()) {
                stackDepth = std::max(0, stackDepth - 1);
                if (brainstormOid != 0 && ev.stack_resolved().object_id() == brainstormOid) {
                    sawBrainstormResolved = true;
                }
                if (softCounterConvoluteOid != 0 &&
                    ev.stack_resolved().object_id() == softCounterConvoluteOid) {
                    softCounterLeftStackBeforeChoice = !sawSoftCounterPaymentChoice;
                    sawSoftCounterResolveAfterChoice = sawSoftCounterPaymentChoice;
                }
            } else if (ev.has_life_changed()) {
                const auto &lc = ev.life_changed();
                lifeByPlayer[lc.player_id()] = lc.new_total();
                log(QStringLiteral("life: player %1 -> %2 (delta %3)")
                        .arg(lc.player_id())
                        .arg(lc.new_total())
                        .arg(lc.delta()));
                if (lc.delta() == -3 && boltOid != 0 && !sawBoltLifeLoss && !inCombatDamageWindow) {
                    sawBoltLifeLoss = true;
                }
                if (lc.delta() == -4 && borosCharmOid != 0 && !sawBorosCharmLifeLoss && !inCombatDamageWindow) {
                    sawBorosCharmLifeLoss = true;
                }
                if (lc.delta() < 0 && (batchDeclaredAttackers || batchCombatDamage)) {
                    sawCombatLifeLoss = true;
                }
            } else if (ev.has_attackers_declared()) {
                if (ev.attackers_declared().attacker_object_ids_size() > 0) {
                    batchDeclaredAttackers = true;
                    sawAttackersDeclared = true;
                    log(QStringLiteral("attackers declared: %1 creature(s)")
                            .arg(ev.attackers_declared().attacker_object_ids_size()));
                }
            } else if (ev.has_face_changed()) {
                const auto &face = ev.face_changed();
                if (waifOid != 0 && face.object_id() == waifOid && face.face_up_index() == 1) {
                    sawWaifFaceChanged = true;
                }
                if (manifestOid != 0 && face.object_id() == manifestOid && !face.face_down()) {
                    sawManifestFaceChanged = true;
                }
            } else if (ev.has_battlefield_object_map()) {
                for (const auto &entry : ev.battlefield_object_map().entries()) {
                    serverCardByEngineOid[entry.engine_object_id()] = entry.server_card_id();
                }
            } else if (ev.has_graveyard_object_map()) {
                for (const auto &entry : ev.graveyard_object_map().entries()) {
                    serverCardByEngineOid[entry.engine_object_id()] = entry.server_card_id();
                }
            } else if (ev.has_face_down_object_map()) {
                for (const auto &entry : ev.face_down_object_map().entries()) {
                    if (entry.controller_player_id() == myId && entry.card_name() == "Hill Giant") {
                        manifestOid = entry.engine_object_id();
                        manifestGeneration = entry.zone_change_generation();
                        manifestServerCardId = entry.server_card_id();
                        sawManifestPrivateIdentity = true;
                    }
                }
                if (role == Role::Hoarder && sawManifestChoiceRedacted &&
                    ev.face_down_object_map().entries_size() == 0) {
                    sawOpponentManifestIdentityEmpty = true;
                }
            } else if (ev.has_trigger_order_required()) {
                const auto &tor = ev.trigger_order_required();
                if (tor.deciding_player_id() == myId && tor.candidates_size() > 0) {
                    pendingTriggerOrder = tor;
                    log(QStringLiteral("trigger order required: %1 candidates").arg(tor.candidates_size()));
                }
            } else if (ev.has_resolution_choice_required()) {
                const auto &rcr = ev.resolution_choice_required();
                if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_SEARCH) {
                    if (rcr.deciding_player_id() == myId) {
                        if (emptyTypecyclingActivated && rcr.candidate_object_ids_size() == 0) {
                            sawEmptyTypecyclingChoice = rcr.min() == 0 && rcr.max() == 1 &&
                                                       rcr.candidate_card_ids_size() == 0 &&
                                                       rcr.candidate_names_size() == 0 &&
                                                       rcr.candidate_server_card_ids_size() == 0;
                        } else {
                            sawOwnLibrarySearchCandidates =
                                sawOwnLibrarySearchCandidates ||
                                (rcr.candidate_object_ids_size() > 0 &&
                                 rcr.candidate_object_ids_size() == rcr.candidate_card_ids_size() &&
                                 rcr.candidate_object_ids_size() == rcr.candidate_names_size() &&
                                 rcr.candidate_object_ids_size() == rcr.candidate_server_card_ids_size());
                        }
                    } else {
                        sawOpponentLibrarySearchRedacted =
                            rcr.candidate_object_ids_size() == 0 && rcr.candidate_card_ids_size() == 0 &&
                            rcr.candidate_names_size() == 0 && rcr.candidate_server_card_ids_size() == 0;
                    }
                }
                if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_MANIFEST_DREAD) {
                    if (rcr.deciding_player_id() == myId) {
                        sawManifestChoicePrivate = rcr.candidate_object_ids_size() == 2 &&
                                                   rcr.candidate_names_size() == 2 &&
                                                   rcr.candidate_server_card_ids_size() == 2;
                    } else {
                        sawManifestChoiceRedacted = rcr.candidate_object_ids_size() == 0 &&
                                                    rcr.candidate_card_ids_size() == 0 &&
                                                    rcr.candidate_names_size() == 0 &&
                                                    rcr.candidate_server_card_ids_size() == 0;
                    }
                }
                if (rcr.deciding_player_id() == myId &&
                    (rcr.candidate_object_ids_size() > 0 ||
                     (rcr.choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_SEARCH && rcr.min() == 0) ||
                     rcr.choice_kind() == ruled::v1::CHOICE_KIND_MANA_PAYMENT ||
                     rcr.choice_kind() == ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH)) {
                    pendingChoice = rcr;
                    log(QStringLiteral("resolution choice: kind %1 min %2 max %3 ordered %4 candidates %5")
                            .arg(QString::fromStdString(ruled::v1::ChoiceKind_Name(rcr.choice_kind())))
                            .arg(rcr.min())
                            .arg(rcr.max())
                            .arg(rcr.ordered())
                            .arg(rcr.candidate_object_ids_size()));
                }
            } else if (ev.has_permanent_moved()) {
                const auto &moved = ev.permanent_moved();
                if (moved.destination() == ruled::v1::PermanentMoved::DESTINATION_BATTLEFIELD &&
                    (moved.card_id() == "mountain" || sawEvolvingWildsPhysicalDeckToTable)) {
                    sawEvolvingWildsPermanentMoved = true;
                    if (evolvingWildsChosenOid == 0) {
                        evolvingWildsChosenOid = moved.object_id();
                    } else if (evolvingWildsChosenOid != moved.object_id()) {
                        evolvingWildsPhysicalIdentityContinuous = false;
                    }
                }
                if (moved.object_id() == controlTargetOid &&
                    moved.destination() == ruled::v1::PermanentMoved::DESTINATION_LIBRARY) {
                    sawLibraryPermanentMoved = true;
                }
            } else if (ev.has_zone_view()) {
                if (ev.zone_view().battlefields_unchanged()) {
                    sawBattlefieldOmission = true;
                    for (const auto &pp : ev.zone_view().per_player()) {
                        if (pp.battlefield_objects_size() != 0) {
                            ADD_FAILURE() << "battlefield omission carried replacement objects";
                        }
                    }
                }
                for (const ruled::v1::RuledPerPlayerView &pp : ev.zone_view().per_player()) {
                    auto &bf = battlefieldByPlayer[pp.player_id()];
                    if (!ev.zone_view().battlefields_unchanged()) {
                        bf.clear();
                        for (const auto &battlefieldObject : pp.battlefield_objects()) {
                            Permanent perm;
                            perm.cardId = QString::fromStdString(battlefieldObject.card_id());
                            if (perm.cardId.isEmpty() && !battlefieldObject.face_down()) {
                                perm.cardId = QString::fromStdString(battlefieldObject.effective_display_name())
                                                  .toLower()
                                                  .replace(QLatin1Char(' '), QLatin1Char('_'));
                            }
                            perm.oid = battlefieldObject.object_id();
                            perm.tapped = battlefieldObject.tapped();
                            perm.creature = battlefieldObject.is_creature();
                            perm.sick = battlefieldObject.summoning_sick();
                            perm.power = static_cast<int>(battlefieldObject.power());
                            perm.toughness = static_cast<int>(battlefieldObject.toughness());
                            perm.faceIndex = static_cast<int>(battlefieldObject.face_up_index());
                            perm.faceDown = battlefieldObject.face_down();
                            perm.generation = battlefieldObject.zone_change_generation();
                            perm.roomDoorCount = std::min(2, battlefieldObject.room_doors_size());
                            for (int door = 0; door < perm.roomDoorCount; ++door) {
                                const auto &publishedDoor = battlefieldObject.room_doors(door);
                                if (publishedDoor.face_index() < perm.roomDoors.size()) {
                                    perm.roomDoors[publishedDoor.face_index()] = publishedDoor.unlocked();
                                }
                            }
                            if (perm.roomDoorCount == 2) {
                                roomOid = perm.oid;
                                roomGeneration = perm.generation;
                                sawRoomCastDoorState =
                                    sawRoomCastDoorState || (!perm.roomDoors[0] && perm.roomDoors[1]);
                                sawRoomFullyUnlocked =
                                    sawRoomFullyUnlocked || (perm.roomDoors[0] && perm.roomDoors[1]);
                                const auto physical = serverCardByEngineOid.find(perm.oid);
                                if (physical != serverCardByEngineOid.end()) {
                                    if (roomServerCardId < 0) {
                                        roomServerCardId = physical->second;
                                    } else if (roomServerCardId != physical->second) {
                                        roomPhysicalIdentityContinuous = false;
                                    }
                                }
                            }
                            if (battlefieldObject.has_attachment_recipient() &&
                                battlefieldObject.attachment_recipient().recipient_case() ==
                                    ruled::v1::AttachmentRecipient::kPlayerId) {
                                perm.attachmentPlayerId = battlefieldObject.attachment_recipient().player_id();
                            }
                            perm.haste = std::find(battlefieldObject.keywords().begin(),
                                                   battlefieldObject.keywords().end(),
                                                   "Haste") != battlefieldObject.keywords().end();
                            perm.reach = std::find(battlefieldObject.keywords().begin(),
                                                  battlefieldObject.keywords().end(),
                                                  "Reach") != battlefieldObject.keywords().end();
                            bf.push_back(perm);
                            if (perm.faceDown && perm.creature && perm.power == 2 && perm.toughness == 2) {
                                if (manifestOid == 0 || manifestOid == perm.oid) {
                                    manifestOid = perm.oid;
                                    manifestGeneration = perm.generation;
                                    sawManifestPublicFaceDown = true;
                                }
                            }
                            if (perm.cardId == QLatin1String("reckless_waif_merciless_predator") ||
                                perm.cardId == QLatin1String("reckless_waif")) {
                                sawWaifOnBattlefield = true;
                                waifOid = perm.oid;
                                if (perm.faceIndex == 1 && perm.power == 3 && perm.toughness == 2) {
                                    sawWaifBackPt = true;
                                }
                            }
                            if (waifOid != 0 && perm.oid == waifOid && perm.faceIndex == 1 && perm.power == 3 &&
                                perm.toughness == 2) {
                                sawWaifBackPt = true;
                            }
                            if (perm.cardId == QLatin1String("anti-venom,_horrifying_healer")) {
                                protectionTargetOid = perm.oid;
                            }
                            const int expectedCurseTarget = role == Role::Aggressor ? oppId : myId;
                            if (perm.cardId == QLatin1String("curse_of_disturbance") &&
                                perm.attachmentPlayerId == expectedCurseTarget) {
                                sawCursePlayerAttachment = true;
                                curseOid = perm.oid;
                            }
                        }
                    }
                    if (oppId < 0 && myId >= 0 && pp.player_id() != myId) {
                        oppId = pp.player_id();
                    }
                }
            } else if (ev.has_hand_slot_map()) {
                handServerCardBySlot.clear();
                std::map<int, int> counts;
                for (const auto &entry : ev.hand_slot_map().entries()) {
                    ++counts[entry.player_id()];
                    if (entry.player_id() == myId) {
                        handServerCardBySlot[static_cast<int>(entry.hand_index())] = entry.server_card_id();
                    }
                }
                for (const auto &kv : counts) {
                    handSizeByPlayer[kv.first] = kv.second;
                    if (oppId < 0 && myId >= 0 && kv.first != myId) {
                        oppId = kv.first;
                    }
                }
            } else if (ev.has_mana_pool_updated()) {
                const auto &mp = ev.mana_pool_updated();
                if (mp.player_id() == myId) {
                    myPool.w = mp.w();
                    myPool.u = mp.u();
                    myPool.b = mp.b();
                    myPool.r = mp.r();
                    myPool.g = mp.g();
                    myPool.c = mp.c();
                }
            } else if (ev.has_log()) {
                const QString text = QString::fromStdString(ev.log().text());
                if (text == QStringLiteral("Combat damage dealt.")) {
                    batchCombatDamage = true;
                }
                if (oppId >= 0 && text.startsWith(QStringLiteral("P%1 discards ").arg(oppId)) &&
                    text.endsWith(QStringLiteral(" (cleanup)"))) {
                    sawOpponentCleanupDiscard = true;
                }
                if (text == QStringLiteral("Totally Lost puts Grizzly Bears on top of its owner's library.")) {
                    sawLibraryPermanentMoved = true;
                }
                log(QStringLiteral("gamelog: %1").arg(text.left(160)));
            }
        }
        if ((previousPhase == ruled::v1::PHASE_ID_OPENING_CHOOSE_FIRST ||
             previousPhase == ruled::v1::PHASE_ID_OPENING_MULLIGAN) &&
            phase == ruled::v1::PHASE_ID_MAIN1 && phaseEvents == 1) {
            sawDirectOpeningToMain1 = true;
            directSettledActivePlayer = activePlayer;
        }
        const auto playerHasControlTarget = [this](int playerId) {
            const auto playerBattlefield = battlefieldByPlayer.find(playerId);
            return playerBattlefield != battlefieldByPlayer.end() &&
                   std::any_of(playerBattlefield->second.begin(), playerBattlefield->second.end(),
                               [this](const Permanent &permanent) { return permanent.oid == controlTargetOid; });
        };
        if (actOfTreasonCast && controlTargetOid != 0 && playerHasControlTarget(myId)) {
            sawControlTransfer = true;
        }
        if (sawControlTransfer && playerHasControlTarget(oppId)) {
            sawControlReturn = true;
        }
        if (sawLibraryPermanentMoved && !playerHasControlTarget(myId) && !playerHasControlTarget(oppId)) {
            sawLibraryTargetAbsentFromBattlefield = true;
        }
        EXPECT_LE(phaseEvents, 1) << "one settled ruled batch published multiple phase states";
        const auto it = batch.legal_by_player().find(myId);
        if (it != batch.legal_by_player().end()) {
            labels.clear();
            for (const std::string &l : it->second.labels()) {
                labels.append(QString::fromStdString(l));
            }
            latestLegal = it->second;
            if (submittedTypecyclingChoice) {
                const auto plains = std::find_if(latestLegal.hand_actions().begin(),
                                                 latestLegal.hand_actions().end(), [](const auto &action) {
                                                     return QString::fromStdString(action.card_name()) ==
                                                            QLatin1String("Plains");
                                                 });
                if (plains != latestLegal.hand_actions().end()) {
                    sawTypecyclingDeckToHand =
                        handServerCardBySlot.count(static_cast<int>(plains->hand_index())) > 0;
                    typecyclingPhysicalIdentityContinuous =
                        typecyclingPhysicalIdentityContinuous && sawTypecyclingDeckToHand;
                }
            }
            const auto hasZoneAbility = [this](const QString &cardName, ruled::v1::AbilitySourceZone sourceZone) {
                return std::any_of(latestLegal.zone_ability_actions().begin(),
                                   latestLegal.zone_ability_actions().end(),
                                   [&](const auto &action) {
                                       return QString::fromStdString(action.card_name()) == cardName &&
                                              action.source_zone() == sourceZone;
                                   });
            };
            if (role == Role::Aggressor) {
                sawOwnTypecyclingAction = sawOwnTypecyclingAction ||
                                           hasZoneAbility(QStringLiteral("Shepherding Spirits"),
                                                          ruled::v1::ABILITY_SOURCE_ZONE_HAND);
                sawOwnRenewAction = sawOwnRenewAction ||
                                    hasZoneAbility(QStringLiteral("Sagu Pummeler"),
                                                   ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD);
            } else {
                sawOpponentTypecyclingActionRedacted =
                    sawOpponentTypecyclingActionRedacted ||
                    !hasZoneAbility(QStringLiteral("Shepherding Spirits"),
                                    ruled::v1::ABILITY_SOURCE_ZONE_HAND);
                sawOpponentRenewActionRedacted =
                    sawOpponentRenewActionRedacted ||
                    !hasZoneAbility(QStringLiteral("Sagu Pummeler"),
                                    ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD);
            }
            if (role == Role::Hoarder && sawLibraryPermanentMoved &&
                std::any_of(latestLegal.hand_actions().begin(), latestLegal.hand_actions().end(), [](const auto &action) {
                    return QString::fromStdString(action.card_name()) == QLatin1String("Grizzly Bears");
                })) {
                sawTopPermanentDrawn = true;
            }
        }
    }

    ruled::v1::LegalActions latestLegal;

    // ---- pregame plumbing ----

    ::testing::AssertionResult loginAndJoinRoom()
    {
        if (!connectToServer()) {
            return ::testing::AssertionFailure() << "could not connect to servatrice";
        }
        CommandContainer cont;
        auto *login = cont.add_session_command()->MutableExtension(Command_Login::ext);
        login->set_user_name(userName.toStdString());
        login->set_password("");
        login->set_clientver("ruled-e2e-smoke");
        const quint64 loginId = nextCmdId;
        sendContainer(cont);
        auto r = pumpUntil([&] { return responses.count(loginId) > 0; }, 10000, "login response");
        if (!r) {
            return r;
        }
        if (responses[loginId].response_code() != Response::RespOk) {
            return ::testing::AssertionFailure()
                   << "login failed with code " << responses[loginId].response_code();
        }
        CommandContainer listCont;
        listCont.add_session_command()->MutableExtension(Command_ListRooms::ext);
        sendContainer(listCont);
        r = pumpUntil([&] { return roomId >= 0; }, 10000, "room list");
        if (!r) {
            return r;
        }
        CommandContainer joinCont;
        auto *join = joinCont.add_session_command()->MutableExtension(Command_JoinRoom::ext);
        join->set_room_id(roomId);
        const quint64 joinId = nextCmdId;
        sendContainer(joinCont);
        r = pumpUntil([&] { return responses.count(joinId) > 0; }, 10000, "join room response");
        if (!r) {
            return r;
        }
        if (responses[joinId].response_code() != Response::RespOk) {
            return ::testing::AssertionFailure()
                   << "join room failed with code " << responses[joinId].response_code();
        }
        return ::testing::AssertionSuccess();
    }

    ::testing::AssertionResult createRuledGame()
    {
        CommandContainer cont;
        cont.set_room_id(roomId);
        auto *create = cont.add_room_command()->MutableExtension(Command_CreateGame::ext);
        create->set_description("ruled e2e smoke");
        create->set_max_players(2);
        create->set_spectators_allowed(false);
        create->set_starting_life_total(20);
        create->set_ruled_game(true);
        sendContainer(cont);
        return pumpUntil([&] { return gameId >= 0 && myId >= 0; }, 10000, "game created/joined");
    }

    ::testing::AssertionResult joinRuledGame(int targetGameId)
    {
        CommandContainer cont;
        cont.set_room_id(roomId);
        auto *join = cont.add_room_command()->MutableExtension(Command_JoinGame::ext);
        join->set_game_id(targetGameId);
        join->set_spectator(false);
        sendContainer(cont);
        return pumpUntil([&] { return gameId == targetGameId && myId >= 0; }, 10000, "game joined");
    }

    ::testing::AssertionResult selectDeck(const QString &xml)
    {
        CommandContainer cont;
        cont.set_game_id(gameId);
        auto *sel = cont.add_game_command()->MutableExtension(Command_DeckSelect::ext);
        sel->set_deck(xml.toStdString());
        const quint64 id = nextCmdId;
        sendContainer(cont);
        auto r = pumpUntil([&] { return responses.count(id) > 0; }, 10000, "deck select response");
        if (!r) {
            return r;
        }
        if (responses[id].response_code() != Response::RespOk) {
            return ::testing::AssertionFailure()
                   << "deck select failed with code " << responses[id].response_code();
        }
        return ::testing::AssertionSuccess();
    }

    void sendReady()
    {
        CommandContainer cont;
        cont.set_game_id(gameId);
        auto *ready = cont.add_game_command()->MutableExtension(Command_ReadyStart::ext);
        ready->set_ready(true);
        sendContainer(cont);
    }

    // ---- ruled command plumbing ----

    void sendRuled(const ruled::v1::RuledCommand &cmd, const QString &what)
    {
        CommandContainer cont;
        cont.set_game_id(gameId);
        std::string bytes;
        cmd.SerializeToString(&bytes);
        cont.add_game_command()->MutableExtension(Command_RuledPayload::ext)->set_payload(bytes);
        ruledCmdIds.insert(nextCmdId);
        sendContainer(cont);
        lastActedVersion = stateVersion;
        log(QStringLiteral("-> %1").arg(what));
    }

    ::testing::AssertionResult publishMain1Stops()
    {
        ruled::v1::RuledCommand cmd;
        auto *policy = cmd.mutable_set_auto_pass_policy();
        // The scripted clients need one ordinary priority window in which to play their lands and
        // spells. Stack entries and required combat/cleanup choices stop automatically; every
        // other empty step is intentionally allowed to settle without an inferred legal-action stop.
        policy->add_stop_on_own_turn(ruled::v1::PHASE_ID_MAIN1);
        policy->add_stop_on_opponent_turn(ruled::v1::PHASE_ID_MAIN1);
        CommandContainer cont;
        cont.set_game_id(gameId);
        std::string bytes;
        cmd.SerializeToString(&bytes);
        cont.add_game_command()->MutableExtension(Command_RuledPayload::ext)->set_payload(bytes);
        const quint64 commandId = nextCmdId;
        sendContainer(cont);
        log(QStringLiteral("-> publish Main1 stop policy"));
        auto result = pumpUntil([&] { return responses.count(commandId) > 0; }, 10000, "auto-pass policy response");
        if (!result) {
            return result;
        }
        if (responses[commandId].response_code() != Response::RespOk) {
            return ::testing::AssertionFailure()
                   << "auto-pass policy failed with code " << responses[commandId].response_code();
        }
        return ::testing::AssertionSuccess();
    }

    // ---- reactive policy ----

    bool labelMatching(const QRegularExpression &re, QRegularExpressionMatch *out = nullptr) const
    {
        for (const QString &l : labels) {
            const QRegularExpressionMatch m = re.match(l);
            if (m.hasMatch()) {
                if (out) {
                    *out = m;
                }
                return true;
            }
        }
        return false;
    }

    const ruled::v1::LegalHandAction *
    handAction(ruled::v1::HandActionKind kind, const QString &cardName = QString()) const
    {
        for (const auto &action : latestLegal.hand_actions()) {
            if (action.kind() == kind &&
                (cardName.isEmpty() || QString::fromStdString(action.card_name()) == cardName)) {
                return &action;
            }
        }
        return nullptr;
    }

    const ruled::v1::LegalZoneAbilityAction *
    zoneAbilityAction(const QString &cardName, ruled::v1::AbilitySourceZone sourceZone) const
    {
        for (const auto &action : latestLegal.zone_ability_actions()) {
            if (QString::fromStdString(action.card_name()) == cardName && action.source_zone() == sourceZone) {
                return &action;
            }
        }
        return nullptr;
    }

    QList<const ruled::v1::LegalHandAction *> handActions(ruled::v1::HandActionKind kind) const
    {
        QList<const ruled::v1::LegalHandAction *> result;
        for (const auto &action : latestLegal.hand_actions()) {
            if (action.kind() == kind) {
                result.append(&action);
            }
        }
        return result;
    }

    int countOwn(const QString &cardId, bool untappedOnly) const
    {
        const auto it = battlefieldByPlayer.find(myId);
        if (it == battlefieldByPlayer.end()) {
            return 0;
        }
        int n = 0;
        for (const Permanent &perm : it->second) {
            if (perm.cardId == cardId && (!untappedOnly || !perm.tapped)) {
                ++n;
            }
        }
        return n;
    }

    bool hasCursePhysicalAnnotation() const
    {
        const auto card = serverCardByEngineOid.find(curseOid);
        if (curseOid == 0 || card == serverCardByEngineOid.end()) {
            return false;
        }
        const auto annotation = annotationByServerCardId.find(card->second);
        return annotation != annotationByServerCardId.end() &&
               annotation->second.contains(QStringLiteral("Enchanting: smokep2"));
    }

    bool hasRoomPhysicalAnnotation() const
    {
        if (roomServerCardId < 0) {
            return false;
        }
        const auto annotation = annotationByServerCardId.find(roomServerCardId);
        return annotation != annotationByServerCardId.end() &&
               annotation->second.contains(QStringLiteral("Doors: Derelict Attic (unlocked), Widow's Walk (unlocked)"));
    }

    std::optional<quint32> firstOwnUntapped(const QString &cardId) const
    {
        const auto it = battlefieldByPlayer.find(myId);
        if (it == battlefieldByPlayer.end()) {
            return std::nullopt;
        }
        for (const Permanent &perm : it->second) {
            if (perm.cardId == cardId && !perm.tapped) {
                return perm.oid;
            }
        }
        return std::nullopt;
    }

    void setBattlefieldAbilitySource(ruled::v1::ActivateAbility *ability, quint32 oid) const
    {
        ability->set_source_object_id(oid);
        ability->set_source_zone(ruled::v1::ABILITY_SOURCE_ZONE_BATTLEFIELD);
        const auto it = battlefieldByPlayer.find(myId);
        if (it == battlefieldByPlayer.end()) {
            return;
        }
        const auto permanent = std::find_if(it->second.begin(), it->second.end(),
                                            [oid](const Permanent &candidate) { return candidate.oid == oid; });
        if (permanent != it->second.end()) {
            ability->set_expected_zone_change_generation(permanent->generation);
        }
    }

    /// Runs the role policy against the current view; sends at most one command per state version.
    /// Conjure Bump in the Night, bury it, and cast it from the graveyard for its flashback cost.
    /// Returns true when it sent a command (the caller should yield).
    ///
    /// Run by BOTH seats on purpose. Every spell is routed to the single canonical stack zone,
    /// which belongs to the *lowest player id*, so only the other seat's cast is a cross-player
    /// move — the case Server_AbstractPlayer::moveCard rejects unless ruledAllowsCrossPlayerMove
    /// whitelists it. Which client holds the low id depends on join order, so pinning the flashback
    /// to one role silently tests the easy half; that is exactly how a broken grave -> stack move
    /// shipped green.
    bool tryFlashbackSequence()
    {
        // Flashback: conjure Bump in the Night, push it to the graveyard, then cast it from
        // there. `put` can only reach hand/battlefield, so the graveyard needs the move.
        if (!devFlashbackConjureSent) {
            devFlashbackConjureSent = true;
            ruled::v1::RuledCommand cmd;
            auto *dev = cmd.mutable_dev_command();
            dev->set_target_player_id(myId);
            auto *put = dev->mutable_put_card_in_zone();
            put->set_card_name("Bump in the Night");
            put->set_zone(ruled::v1::DEV_ZONE_HAND);
            sendRuled(cmd, QStringLiteral("dev: conjure Bump in the Night into hand"));
            return true;
        }
        if (!devFlashbackMoveSent) {
            devFlashbackMoveSent = true;
            ruled::v1::RuledCommand cmd;
            auto *dev = cmd.mutable_dev_command();
            dev->set_target_player_id(myId);
            auto *move = dev->mutable_move_card();
            move->set_card_name("Bump in the Night");
            move->set_zone(ruled::v1::DEV_ZONE_GRAVEYARD);
            sendRuled(cmd, QStringLiteral("dev: move Bump in the Night to the graveyard"));
            return true;
        }
        // Fund the flashback cost ({5}{R}) outright. The block below spends it on the very
        // next action, so it never reaches the affordability checks the rest of the script
        // makes — and the test stays a ~1s smoke run instead of waiting on land drops.
        if (devFlashbackMoveSent && !devFlashbackManaSent) {
            devFlashbackManaSent = true;
            ruled::v1::RuledCommand cmd;
            auto *dev = cmd.mutable_dev_command();
            dev->set_target_player_id(myId);
            dev->mutable_add_mana()->set_r(1);
            dev->mutable_add_mana()->set_c(5);
            sendRuled(cmd, QStringLiteral("dev: add {5}{R} for the flashback cast"));
            return true;
        }
        if (!flashbackCast && devFlashbackManaSent) {
            for (const auto &ga : latestLegal.zone_cast_actions()) {
                if (ga.source_zone() != ruled::v1::CAST_SOURCE_ZONE_GRAVEYARD) {
                    continue;
                }
                if (QString::fromStdString(ga.card_name()) != QLatin1String("Bump in the Night")) {
                    continue;
                }
                if (myPool.r < 1 || myPool.total() < 6) {
                    break; // wait for the dev mana below to land
                }
                ruled::v1::RuledCommand cmd;
                auto *cast = cmd.mutable_cast_spell();
                cast->mutable_source()->set_graveyard_object_id(ga.object_id());
                cast->add_targets()->set_object_id(static_cast<quint32>(oppId));
                flashbackCast = true;
                sendRuled(cmd, QStringLiteral("flashback Bump in the Night (oid %1) at player %2")
                                   .arg(ga.object_id())
                                   .arg(oppId));
                return true;
            }
        }
        return false;
    }

    bool tryAdventureSequence()
    {
        if (!devAdventureConjureSent) {
            devAdventureConjureSent = true;
            ruled::v1::RuledCommand cmd;
            auto *dev = cmd.mutable_dev_command();
            dev->set_target_player_id(myId);
            auto *put = dev->mutable_put_card_in_zone();
            put->set_card_name("Bonecrusher Giant // Stomp");
            put->set_zone(ruled::v1::DEV_ZONE_HAND);
            sendRuled(cmd, QStringLiteral("dev: conjure Bonecrusher Giant // Stomp"));
            return true;
        }
        if (!devAdventureManaSent) {
            devAdventureManaSent = true;
            ruled::v1::RuledCommand cmd;
            auto *dev = cmd.mutable_dev_command();
            dev->set_target_player_id(myId);
            dev->mutable_add_mana()->set_r(2);
            dev->mutable_add_mana()->set_c(3);
            sendRuled(cmd, QStringLiteral("dev: add mana for both Adventure casts"));
            return true;
        }
        if (!stompCast) {
            if (const auto *stomp = handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Stomp"))) {
                ruled::v1::RuledCommand cmd;
                auto *cast = cmd.mutable_cast_spell();
                cast->mutable_source()->set_hand_index(stomp->hand_index());
                cast->set_face_index(1);
                cast->add_targets()->set_object_id(static_cast<quint32>(oppId));
                stompCast = true;
                sendRuled(cmd, QStringLiteral("cast Stomp at player %1").arg(oppId));
                return true;
            }
        }
        if (stompCast && !giantCastFromExile) {
            for (const auto &action : latestLegal.zone_cast_actions()) {
                if (action.source_zone() != ruled::v1::CAST_SOURCE_ZONE_EXILE ||
                    QString::fromStdString(action.card_name()) != QLatin1String("Bonecrusher Giant")) {
                    continue;
                }
                ruled::v1::RuledCommand cmd;
                auto *cast = cmd.mutable_cast_spell();
                cast->mutable_source()->set_exile_object_id(action.object_id());
                cast->set_face_index(action.face_index());
                giantCastFromExile = true;
                sendRuled(cmd, QStringLiteral("cast Bonecrusher Giant from exile oid %1").arg(action.object_id()));
                return true;
            }
        }
        return false;
    }

    void act()
    {
        if (myId < 0 || !gameStarted || stateVersion == 0 || lastActedVersion == stateVersion) {
            return;
        }

        // --- Opening sequence (display labels plus structured hand actions; no priority yet) ---
        if (labelMatching(QRegularExpression(QStringLiteral("^You start \\(opening pick\\)$")))) {
            ruled::v1::RuledCommand cmd;
            // Both roles arrange for the aggressor to take the first turn.
            cmd.mutable_choose_starting_player()->set_starting_player_id(role == Role::Aggressor ? myId : oppId);
            sendRuled(cmd, QStringLiteral("choose starting player -> %1").arg(role == Role::Aggressor ? myId : oppId));
            return;
        }
        if (labelMatching(QRegularExpression(QStringLiteral("^Keep opening hand \\(opening\\)$")))) {
            ruled::v1::RuledCommand cmd;
            const bool takeMulligan = role == Role::Hoarder && !didMulligan;
            cmd.mutable_mulligan()->set_keep(!takeMulligan);
            if (takeMulligan) {
                didMulligan = true;
            }
            sendRuled(cmd, takeMulligan ? QStringLiteral("mulligan") : QStringLiteral("keep hand"));
            return;
        }
        if (const auto *bottom = handAction(ruled::v1::HAND_ACTION_OPENING_BOTTOM)) {
            sawBottomAction = true;
            ruled::v1::RuledCommand cmd;
            cmd.mutable_put_opening_hand_on_bottom()->set_hand_card_index(bottom->hand_index());
            sentBottom = true;
            sendRuled(cmd, QStringLiteral("bottom hand idx %1").arg(bottom->hand_index()));
            return;
        }

        // --- Simultaneous trigger ordering (CR 603.3b) ---
        // Ahead of the resolution choice, matching the engine's own precedence. Answered in the
        // offered APNAP order: the bot has no preference, it just has to unblock the game.
        if (pendingTriggerOrder) {
            // One pick per prompt: the engine re-asks with what is left (or places the last one
            // itself), so the bot just takes whichever it was offered first.
            ruled::v1::RuledCommand cmd;
            const auto &first = pendingTriggerOrder->candidates(0);
            cmd.mutable_submit_trigger_order()->set_trigger_object_id(first.trigger_object_id());
            const QString name = QString::fromStdString(first.source_card_name());
            pendingTriggerOrder.reset();
            sendRuled(cmd, QStringLiteral("put %1's trigger on the stack next").arg(name));
            return;
        }

        // --- Tier-3 resolution choice (may target either player at any point) ---
        if (pendingChoice) {
            const auto &rcr = *pendingChoice;
            if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH) {
                const bool isProtection = rcr.resolution_branches_size() == 6 &&
                                          rcr.resolution_branches(0).label() == "artifacts";
                if (!isProtection) {
                    ADD_FAILURE() << "unexpected authored resolution-branch choice";
                    pendingChoice.reset();
                    return;
                }
                sawProtectionBranchChoice = true;
                if (!sawProtectionHandToStack || protectionLeftStackBeforeChoice) {
                    ADD_FAILURE() << "Apostle's Blessing was not physically present on the stack during its choice";
                }
                ruled::v1::RuledCommand cmd;
                auto *choice = cmd.mutable_submit_resolution_choice();
                choice->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_SELECT_BRANCH);
                choice->set_selected_branch_index(0);
                pendingChoice.reset();
                submittedProtectionBranchChoice = true;
                sendRuled(cmd, QStringLiteral("choose protection from artifacts"));
                return;
            }
            if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_MANA_PAYMENT) {
                sawSoftCounterPaymentChoice = true;
                pendingChoice.reset();
                if (!rcr.payment_currently_legal()) {
                    const auto island = firstOwnUntapped(QStringLiteral("island"));
                    if (!island) {
                        ADD_FAILURE() << "soft-counter payment was unaffordable with no untapped Island";
                        return;
                    }
                    ruled::v1::RuledCommand cmd;
                    auto *ability = cmd.mutable_activate_ability();
                    setBattlefieldAbilitySource(ability, *island);
                    ability->set_ability_index(0);
                    activatedManaDuringSoftCounterPayment = true;
                    sendRuled(cmd, QStringLiteral("tap Island oid %1 during Convolute resolution").arg(*island));
                    return;
                }
                ruled::v1::RuledCommand cmd;
                cmd.mutable_submit_resolution_choice()->set_decision(
                    ruled::v1::RESOLUTION_CHOICE_DECISION_PAY_MANA);
                paidSoftCounter = true;
                sendRuled(cmd, QStringLiteral("pay Convolute's resolution cost"));
                return;
            }
            const bool isReplacement = rcr.choice_kind() == ruled::v1::CHOICE_KIND_REPLACEMENT_EFFECT;
            const bool isLibrarySearch = rcr.choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_SEARCH;
            const bool isTypecycling = isLibrarySearch && typecyclingActivated && !submittedTypecyclingChoice;
            const bool isEmptyTypecycling =
                isLibrarySearch && emptyTypecyclingActivated && !submittedEmptyTypecyclingChoice;
            const bool isManifestDread = rcr.choice_kind() == ruled::v1::CHOICE_KIND_MANIFEST_DREAD;
            const QString prompt = QString::fromStdString(rcr.prompt_text());
            const bool isEntryReplacement = isReplacement && prompt.contains(QStringLiteral("entering the battlefield"));
            const bool isDamagePrevention = isReplacement && !isEntryReplacement;
            if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_HAND_CARDS && rcr.ordered()) {
                sawBrainstormChoice = true;
            }
            if (isDamagePrevention) {
                sawDamagePreventionChoice = true;
            }
            if (isEntryReplacement) {
                sawEntryReplacementChoice = true;
            }
            ruled::v1::RuledCommand cmd;
            auto *choice = cmd.mutable_submit_resolution_choice();
            const int need = isLibrarySearch && rcr.candidate_object_ids_size() > 0
                                 ? 1
                                 : static_cast<int>(rcr.min());
            if (isManifestDread) {
                int chosen = -1;
                for (int i = 0; i < rcr.candidate_names_size(); ++i) {
                    if (rcr.candidate_names(i) == "Hill Giant") {
                        chosen = i;
                        break;
                    }
                }
                if (chosen < 0) {
                    ADD_FAILURE() << "manifest-dread private candidates did not contain Hill Giant";
                    pendingChoice.reset();
                    return;
                }
                choice->add_chosen_object_ids(rcr.candidate_object_ids(chosen));
                manifestOid = rcr.candidate_object_ids(chosen);
                submittedManifestChoice = true;
            } else {
                for (int i = 0; i < need && i < rcr.candidate_object_ids_size(); ++i) {
                    choice->add_chosen_object_ids(rcr.candidate_object_ids(i));
                }
            }
            if (isTypecycling && need == 1) {
                typecyclingChosenPhysicalId = rcr.candidate_server_card_ids(0);
                submittedTypecyclingChoice = true;
            } else if (isEmptyTypecycling && need == 0) {
                submittedEmptyTypecyclingChoice = true;
            } else if (isLibrarySearch && need == 1) {
                evolvingWildsChosenOid = rcr.candidate_object_ids(0);
                submittedEvolvingWildsChoice = true;
            }
            pendingChoice.reset();
            if (isDamagePrevention) {
                submittedDamagePreventionChoice = true;
            } else if (isEntryReplacement) {
                submittedEntryReplacementChoice = true;
            } else if (!isManifestDread && !isTypecycling && !isEmptyTypecycling) {
                submittedBrainstormChoice = true;
            }
            sendRuled(cmd, QStringLiteral("submit resolution choice (%1 cards)").arg(need));
            return;
        }

        // --- Combat declarations (priority is locked; phase + role drive these) ---
        if (phase == ruled::v1::PHASE_ID_DECLARE_ATTACKERS && activePlayer == myId && !attackersSentThisCombat) {
            ruled::v1::RuledCommand cmd;
            auto *att = cmd.mutable_declare_attackers();
            const auto it = battlefieldByPlayer.find(myId);
            // Stop attacking once the combat-damage milestone is in: the smoke game must not
            // kill the opponent before the later milestones (Brainstorm, cleanup discard) land.
            if (it != battlefieldByPlayer.end() && !sawCombatLifeLoss) {
                for (const Permanent &perm : it->second) {
                    if (perm.cardId != QStringLiteral("anti-venom,_horrifying_healer") && perm.creature &&
                        !perm.tapped &&
                        (!perm.sick || perm.haste)) {
                        att->add_creature_ids(perm.oid);
                    }
                }
            }
            attackersSentThisCombat = true;
            sendRuled(cmd, QStringLiteral("declare attackers (%1)").arg(att->creature_ids_size()));
            return;
        }
        if (phase == ruled::v1::PHASE_ID_DECLARE_BLOCKERS && activePlayer != myId && !blockersSentThisCombat) {
            ruled::v1::RuledCommand cmd;
            cmd.mutable_declare_blockers();
            blockersSentThisCombat = true;
            sendRuled(cmd, QStringLiteral("declare no blockers"));
            return;
        }

        // Issue #88: the nonactive player responds to the specially conjured Bolt with Convolute.
        // The Bolt controller will then activate Islands from the parked payment prompt above.
        if (role == Role::Hoarder && stackDepth == 1 && latestBoltOid != 0 && !softCounterConvoluteCast) {
            if (const auto *convolute = handAction(ruled::v1::HAND_ACTION_CAST_SPELL,
                                                   QStringLiteral("Convolute"))) {
                ruled::v1::RuledCommand cmd;
                auto *cast = cmd.mutable_cast_spell();
                cast->mutable_source()->set_hand_index(convolute->hand_index());
                cast->add_targets()->set_object_id(latestBoltOid);
                softCounterConvoluteCast = true;
                sendRuled(cmd, QStringLiteral("cast Convolute at Bolt oid %1").arg(latestBoltOid));
                return;
            }
        }

        // --- Cleanup discard ---
        {
            const auto discardActions = handActions(ruled::v1::HAND_ACTION_CLEANUP_DISCARD);
            if (!discardActions.isEmpty()) {
                sawCleanupDiscardActions = true;
                const int excess = discardActions.size() - 7;
                if (excess > 0) {
                    ruled::v1::RuledCommand cmd;
                    auto *disc = cmd.mutable_discard_to_hand_size();
                    for (int i = 0; i < excess; ++i) {
                        disc->add_hand_card_indices(discardActions.at(i)->hand_index());
                    }
                    sentCleanupDiscard = true;
                    sendRuled(cmd, QStringLiteral("cleanup discard %1 card(s)").arg(excess));
                    return;
                }
            }
        }

        // Dev-command effects, observed the same way as any other engine state: the conjured
        // permanent has to reach the battlefield object map, and the minted mana the pool.
        if (devConjureSent && countOwn(QStringLiteral("serra_angel"), false) > 0) {
            sawDevConjuredPermanent = true;
        }
        if (devManaSent && myPool.g >= 2) {
            sawDevMana = true;
        }
        const auto ownBattlefield = battlefieldByPlayer.find(myId);
        if (ownBattlefield != battlefieldByPlayer.end()) {
            sawDiregrafEnterTapped =
                sawDiregrafEnterTapped ||
                std::any_of(ownBattlefield->second.begin(), ownBattlefield->second.end(), [](const Permanent &permanent) {
                    return permanent.cardId == QStringLiteral("diregraf_ghoul") && permanent.tapped;
                });
        }

        // --- Priority-gated actions ---
        if (priorityPlayer != myId) {
            return;
        }
        const bool inMain =
            (phase == ruled::v1::PHASE_ID_MAIN1 || phase == ruled::v1::PHASE_ID_MAIN2) && activePlayer == myId;

        if (role == Role::Aggressor && inMain && stackDepth == 0) {
            // Issue #98: the fixed seed puts Hill Giant and Lightning Bolt on top. Cast Manifest
            // Dread, exercise private candidate publication, then use the generation-bound
            // special action to flip the exact physical Hill Giant in place.
            if (!devManifestSpellConjured) {
                devManifestSpellConjured = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                auto *put = dev->mutable_put_card_in_zone();
                put->set_card_name("Manifest Dread");
                put->set_zone(ruled::v1::DEV_ZONE_HAND);
                sendRuled(cmd, QStringLiteral("dev: conjure Manifest Dread"));
                return;
            }
            if (!devManifestManaSent) {
                devManifestManaSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                dev->mutable_add_mana()->set_g(1);
                dev->mutable_add_mana()->set_r(1);
                // {1}{G} for Manifest Dread plus {3}{R} for Hill Giant's special action.
                dev->mutable_add_mana()->set_c(4);
                sendRuled(cmd, QStringLiteral("dev: add mana for Manifest Dread and turn face up"));
                return;
            }
            if (!manifestSpellCast) {
                if (const auto *spell =
                        handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Manifest Dread"))) {
                    ruled::v1::RuledCommand cmd;
                    cmd.mutable_cast_spell()->mutable_source()->set_hand_index(spell->hand_index());
                    manifestSpellCast = true;
                    sendRuled(cmd, QStringLiteral("cast Manifest Dread"));
                    return;
                }
            }
            if (manifestSpellCast && !submittedManifestChoice) {
                return;
            }
            if (submittedManifestChoice && !turnManifestFaceUpSent) {
                for (const auto &action : latestLegal.permanent_actions()) {
                    if (action.kind() == ruled::v1::PERMANENT_ACTION_KIND_TURN_FACE_UP &&
                        action.object_id() == manifestOid) {
                        EXPECT_EQ(action.zone_change_generation(), manifestGeneration);
                        EXPECT_EQ(action.mana_cost(), "{3}{R}");
                        ruled::v1::RuledCommand cmd;
                        auto *turn = cmd.mutable_execute_permanent_action();
                        turn->set_kind(ruled::v1::PERMANENT_ACTION_KIND_TURN_FACE_UP);
                        turn->set_object_id(action.object_id());
                        turn->set_expected_zone_change_generation(action.zone_change_generation());
                        turnManifestFaceUpSent = true;
                        sendRuled(cmd, QStringLiteral("turn manifested Hill Giant face up"));
                        return;
                    }
                }
                return;
            }
            if (turnManifestFaceUpSent && !sawManifestFaceChanged) {
                return;
            }
            // Issue #99: cast one door of a physical Room, then unlock the other through the
            // generic permanent-action contract. The unlock itself never becomes a stack object;
            // Derelict Attic's resulting triggered ability does.
            if (!devRoomConjureSent) {
                devRoomConjureSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                auto *put = dev->mutable_put_card_in_zone();
                put->set_card_name("Derelict Attic // Widow's Walk");
                put->set_zone(ruled::v1::DEV_ZONE_HAND);
                sendRuled(cmd, QStringLiteral("dev: conjure a Room into hand"));
                return;
            }
            if (!devRoomManaSent) {
                devRoomManaSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                dev->mutable_add_mana()->set_b(2);
                dev->mutable_add_mana()->set_c(5);
                sendRuled(cmd, QStringLiteral("dev: add mana for both Room doors"));
                return;
            }
            if (!roomCast) {
                if (const auto *spell = handAction(ruled::v1::HAND_ACTION_CAST_SPELL,
                                                   QStringLiteral("Widow's Walk"))) {
                    ruled::v1::RuledCommand cmd;
                    auto *cast = cmd.mutable_cast_spell();
                    cast->mutable_source()->set_hand_index(spell->hand_index());
                    cast->set_face_index(1u);
                    roomCast = true;
                    sendRuled(cmd, QStringLiteral("cast Widow's Walk door"));
                    return;
                }
            }
            if (roomCast && (!sawRoomCastDoorState || stackDepth != 0)) {
                return;
            }
            if (!roomUnlockSent) {
                for (const auto &action : latestLegal.permanent_actions()) {
                    if (action.kind() == ruled::v1::PERMANENT_ACTION_KIND_UNLOCK_ROOM_DOOR &&
                        action.object_id() == roomOid && action.has_face_index() && action.face_index() == 0u) {
                        ruled::v1::RuledCommand cmd;
                        auto *unlock = cmd.mutable_execute_permanent_action();
                        unlock->set_kind(action.kind());
                        unlock->set_object_id(action.object_id());
                        unlock->set_expected_zone_change_generation(action.zone_change_generation());
                        unlock->set_face_index(action.face_index());
                        roomUnlockSent = true;
                        sendRuled(cmd, QStringLiteral("unlock Derelict Attic door"));
                        return;
                    }
                }
                return;
            }
            if (!sawRoomFullyUnlocked || !sawRoomUnlockTrigger || stackDepth != 0) {
                return;
            }
            // Issue #101: activate a subtypecycling ability from the hand, then a Renew
            // ability from the graveyard. Both commands use the exact engine ObjectId and
            // zone-change generation published only to the owning client.
            if (!devTypecyclingConjureSent) {
                devTypecyclingConjureSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                auto *put = dev->mutable_put_card_in_zone();
                put->set_card_name("Shepherding Spirits");
                put->set_zone(ruled::v1::DEV_ZONE_HAND);
                sendRuled(cmd, QStringLiteral("dev: conjure Shepherding Spirits into hand"));
                return;
            }
            if (!devTypecyclingManaSent) {
                devTypecyclingManaSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                dev->mutable_add_mana()->set_c(2);
                sendRuled(cmd, QStringLiteral("dev: add {2} for Plainscycling"));
                return;
            }
            if (!typecyclingActivated) {
                if (const auto *action = zoneAbilityAction(QStringLiteral("Shepherding Spirits"),
                                                           ruled::v1::ABILITY_SOURCE_ZONE_HAND)) {
                    ruled::v1::RuledCommand cmd;
                    auto *ability = cmd.mutable_activate_ability();
                    ability->set_source_object_id(action->object_id());
                    ability->set_source_zone(action->source_zone());
                    ability->set_expected_zone_change_generation(action->zone_change_generation());
                    ability->set_ability_index(action->ability_index());
                    if (action->has_hand_index()) {
                        const auto physical = handServerCardBySlot.find(static_cast<int>(action->hand_index()));
                        if (physical != handServerCardBySlot.end()) {
                            typecyclingSourcePhysicalId = physical->second;
                        }
                    }
                    typecyclingActivated = true;
                    sendRuled(cmd, QStringLiteral("Plainscycle Shepherding Spirits oid %1").arg(action->object_id()));
                    return;
                }
                return;
            }
            if (typecyclingActivated && !submittedTypecyclingChoice) {
                return;
            }
            // The scripted deck contains exactly one Plains. Cycle a second copy after the first
            // search moved that Plains to hand so the relay/client path must preserve the explicit
            // zero-candidate, min-zero fail-to-find choice instead of deadlocking.
            if (!devEmptyTypecyclingConjureSent) {
                devEmptyTypecyclingConjureSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                auto *put = dev->mutable_put_card_in_zone();
                put->set_card_name("Shepherding Spirits");
                put->set_zone(ruled::v1::DEV_ZONE_HAND);
                sendRuled(cmd, QStringLiteral("dev: conjure second Shepherding Spirits into hand"));
                return;
            }
            if (!devEmptyTypecyclingManaSent) {
                devEmptyTypecyclingManaSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                dev->mutable_add_mana()->set_c(2);
                sendRuled(cmd, QStringLiteral("dev: add {2} for empty Plainscycling search"));
                return;
            }
            if (!emptyTypecyclingActivated) {
                if (const auto *action = zoneAbilityAction(QStringLiteral("Shepherding Spirits"),
                                                           ruled::v1::ABILITY_SOURCE_ZONE_HAND)) {
                    ruled::v1::RuledCommand cmd;
                    auto *ability = cmd.mutable_activate_ability();
                    ability->set_source_object_id(action->object_id());
                    ability->set_source_zone(action->source_zone());
                    ability->set_expected_zone_change_generation(action->zone_change_generation());
                    ability->set_ability_index(action->ability_index());
                    emptyTypecyclingActivated = true;
                    sendRuled(cmd,
                              QStringLiteral("Plainscycle second Shepherding Spirits oid %1").arg(action->object_id()));
                    return;
                }
                return;
            }
            if (!submittedEmptyTypecyclingChoice) {
                return;
            }
            if (!devRenewConjureSent) {
                devRenewConjureSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                auto *put = dev->mutable_put_card_in_zone();
                put->set_card_name("Sagu Pummeler");
                put->set_zone(ruled::v1::DEV_ZONE_HAND);
                sendRuled(cmd, QStringLiteral("dev: conjure Sagu Pummeler into hand"));
                return;
            }
            if (!devRenewMoveSent) {
                devRenewMoveSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                auto *move = dev->mutable_move_card();
                move->set_card_name("Sagu Pummeler");
                move->set_zone(ruled::v1::DEV_ZONE_GRAVEYARD);
                sendRuled(cmd, QStringLiteral("dev: move Sagu Pummeler to the graveyard"));
                return;
            }
            if (!devRenewManaSent) {
                devRenewManaSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                dev->mutable_add_mana()->set_g(1);
                dev->mutable_add_mana()->set_c(4);
                sendRuled(cmd, QStringLiteral("dev: add {4}{G} for Renew"));
                return;
            }
            if (!renewActivated) {
                if (const auto *action = zoneAbilityAction(QStringLiteral("Sagu Pummeler"),
                                                           ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD)) {
                    ruled::v1::RuledCommand cmd;
                    auto *ability = cmd.mutable_activate_ability();
                    ability->set_source_object_id(action->object_id());
                    ability->set_source_zone(action->source_zone());
                    ability->set_expected_zone_change_generation(action->zone_change_generation());
                    ability->set_ability_index(action->ability_index());
                    ability->add_targets()->set_object_id(manifestOid);
                    const auto physical = serverCardByEngineOid.find(action->object_id());
                    if (physical != serverCardByEngineOid.end()) {
                        renewSourcePhysicalId = physical->second;
                    }
                    renewActivated = true;
                    sendRuled(cmd, QStringLiteral("Renew Sagu Pummeler oid %1 onto Hill Giant oid %2")
                                       .arg(action->object_id())
                                       .arg(manifestOid));
                    return;
                }
                return;
            }
            if (renewActivated) {
                const auto battlefield = battlefieldByPlayer.find(myId);
                if (battlefield != battlefieldByPlayer.end()) {
                    const auto renewed = std::find_if(battlefield->second.begin(), battlefield->second.end(),
                                                      [this](const Permanent &permanent) {
                                                          return permanent.oid == manifestOid &&
                                                                 permanent.power == 5 && permanent.toughness == 5 &&
                                                                 permanent.reach;
                                                      });
                    sawRenewCounters = renewed != battlefield->second.end();
                }
                if (!sawRenewCounters) {
                    return;
                }
            }
            if (!devCurseConjureSent) {
                devCurseConjureSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                auto *put = dev->mutable_put_card_in_zone();
                put->set_card_name("Curse of Disturbance");
                put->set_zone(ruled::v1::DEV_ZONE_HAND);
                sendRuled(cmd, QStringLiteral("dev: conjure Curse of Disturbance into hand"));
                return;
            }
            if (!devCurseManaSent) {
                devCurseManaSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                dev->mutable_add_mana()->set_b(1);
                dev->mutable_add_mana()->set_c(2);
                sendRuled(cmd, QStringLiteral("dev: add {2}{B} for Curse of Disturbance"));
                return;
            }
            if (!curseCast) {
                if (const auto *curse =
                        handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Curse of Disturbance"))) {
                    if (myPool.b >= 1 && myPool.total() >= 3) {
                        ruled::v1::RuledCommand cmd;
                        auto *cast = cmd.mutable_cast_spell();
                        cast->mutable_source()->set_hand_index(curse->hand_index());
                        cast->add_targets()->set_object_id(static_cast<quint32>(oppId));
                        curseCast = true;
                        sendRuled(cmd, QStringLiteral("cast Curse of Disturbance enchanting player %1").arg(oppId));
                        return;
                    }
                }
            }
            if (curseCast && !sawCursePlayerAttachment) {
                return;
            }
            if (tryFlashbackSequence()) {
                return;
            }
            if (tryAdventureSequence()) {
                return;
            }
            if (!devOrbSent) {
                devOrbSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                auto *put = dev->mutable_put_card_in_zone();
                put->set_card_name("Orb of Dreams");
                put->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
                sendRuled(cmd, QStringLiteral("dev: conjure Orb of Dreams onto the battlefield"));
                return;
            }
            if (!devDiregrafSent && countOwn(QStringLiteral("orb_of_dreams"), false) > 0) {
                devDiregrafSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                auto *put = dev->mutable_put_card_in_zone();
                put->set_card_name("Diregraf Ghoul");
                put->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
                sendRuled(cmd, QStringLiteral("dev: propose Diregraf Ghoul battlefield entry"));
                return;
            }
            if (sawDiregrafEnterTapped && !devDiregrafRemoved) {
                devDiregrafRemoved = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                auto *move = dev->mutable_move_card();
                move->set_card_name("Diregraf Ghoul");
                move->set_zone(ruled::v1::DEV_ZONE_HAND);
                sendRuled(cmd, QStringLiteral("dev: remove Diregraf Ghoul after entry calibration"));
                return;
            }
            if (devDiregrafSent && !devDiregrafRemoved) {
                return;
            }
            if (!devAntiVenomSent) {
                devAntiVenomSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                auto *put = dev->mutable_put_card_in_zone();
                put->set_card_name("Anti-Venom, Horrifying Healer");
                put->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
                put->set_ready(true);
                sendRuled(cmd, QStringLiteral("dev: conjure Anti-Venom onto the battlefield"));
                return;
            }
            if (!devPreventionSalveSent) {
                devPreventionSalveSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                auto *put = dev->mutable_put_card_in_zone();
                put->set_card_name("Healing Salve");
                put->set_zone(ruled::v1::DEV_ZONE_HAND);
                sendRuled(cmd, QStringLiteral("dev: conjure Healing Salve into hand"));
                return;
            }
            if (!devPreventionBlazeSent) {
                devPreventionBlazeSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                auto *put = dev->mutable_put_card_in_zone();
                put->set_card_name("Blaze");
                put->set_zone(ruled::v1::DEV_ZONE_HAND);
                sendRuled(cmd, QStringLiteral("dev: conjure Blaze into hand"));
                return;
            }
            if (!devPreventionManaSent) {
                devPreventionManaSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                dev->mutable_add_mana()->set_w(1);
                dev->mutable_add_mana()->set_r(1);
                dev->mutable_add_mana()->set_c(5);
                sendRuled(cmd, QStringLiteral("dev: add mana for prevention-order smoke"));
                return;
            }
            std::optional<quint32> antiVenomOid;
            const auto battlefield = battlefieldByPlayer.find(myId);
            if (battlefield != battlefieldByPlayer.end()) {
                for (const Permanent &permanent : battlefield->second) {
                    if (permanent.cardId == QStringLiteral("anti-venom,_horrifying_healer")) {
                        antiVenomOid = permanent.oid;
                        break;
                    }
                }
            }
            if (!preventionSalveCast && antiVenomOid) {
                if (const auto *salve = handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Healing Salve"))) {
                    ruled::v1::RuledCommand cmd;
                    auto *cast = cmd.mutable_cast_spell();
                    cast->mutable_source()->set_hand_index(salve->hand_index());
                    auto *mode = cast->add_selected_modes();
                    mode->set_mode_index(1);
                    mode->add_targets()->set_object_id(*antiVenomOid);
                    preventionSalveCast = true;
                    sendRuled(cmd, QStringLiteral("cast Healing Salve prevention mode on Anti-Venom"));
                    return;
                }
            }
            if (preventionSalveCast && !preventionBlazeCast && antiVenomOid) {
                if (const auto *blaze = handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Blaze"))) {
                    ruled::v1::RuledCommand cmd;
                    auto *cast = cmd.mutable_cast_spell();
                    cast->mutable_source()->set_hand_index(blaze->hand_index());
                    cast->set_x_value(5);
                    cast->add_targets()->set_object_id(*antiVenomOid);
                    preventionBlazeCast = true;
                    sendRuled(cmd, QStringLiteral("cast Blaze for 5 on shielded Anti-Venom"));
                    return;
                }
            }
            if (submittedDamagePreventionChoice && !devProtectionBlessingSent) {
                devProtectionBlessingSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                auto *put = dev->mutable_put_card_in_zone();
                put->set_card_name("Apostle's Blessing");
                put->set_zone(ruled::v1::DEV_ZONE_HAND);
                sendRuled(cmd, QStringLiteral("dev: conjure Apostle's Blessing into hand"));
                return;
            }
            if (devProtectionBlessingSent && !devProtectionManaSent) {
                devProtectionManaSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                dev->mutable_add_mana()->set_w(1);
                dev->mutable_add_mana()->set_c(1);
                sendRuled(cmd, QStringLiteral("dev: add {1}{W} for Apostle's Blessing"));
                return;
            }
            if (devProtectionManaSent && !protectionBlessingCast && antiVenomOid) {
                if (const auto *blessing =
                        handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Apostle's Blessing"))) {
                    ruled::v1::RuledCommand cmd;
                    auto *cast = cmd.mutable_cast_spell();
                    cast->mutable_source()->set_hand_index(blessing->hand_index());
                    cast->add_targets()->set_object_id(*antiVenomOid);
                    protectionBlessingCast = true;
                    sendRuled(cmd, QStringLiteral("cast Apostle's Blessing on Anti-Venom"));
                    return;
                }
            }
            if (protectionBlessingCast && !submittedProtectionBranchChoice) {
                return;
            }
            if (preventionBlazeCast && submittedProtectionBranchChoice && !devControlTargetSent) {
                devControlTargetSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(oppId);
                auto *put = dev->mutable_put_card_in_zone();
                put->set_card_name("Grizzly Bears");
                put->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
                put->set_ready(true);
                sendRuled(cmd, QStringLiteral("dev: conjure control target for player %1").arg(oppId));
                return;
            }
            if (devControlTargetSent && controlTargetOid == 0) {
                const auto opponentBattlefield = battlefieldByPlayer.find(oppId);
                if (opponentBattlefield != battlefieldByPlayer.end()) {
                    const auto target = std::find_if(
                        opponentBattlefield->second.begin(), opponentBattlefield->second.end(),
                        [](const Permanent &permanent) { return permanent.cardId == QStringLiteral("grizzly_bears"); });
                    if (target != opponentBattlefield->second.end()) {
                        controlTargetOid = target->oid;
                    }
                }
            }
            if (controlTargetOid != 0 && !devActOfTreasonSent) {
                devActOfTreasonSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                auto *put = dev->mutable_put_card_in_zone();
                put->set_card_name("Act of Treason");
                put->set_zone(ruled::v1::DEV_ZONE_HAND);
                sendRuled(cmd, QStringLiteral("dev: conjure Act of Treason into hand"));
                return;
            }
            if (devActOfTreasonSent && !devControlManaSent) {
                devControlManaSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                dev->mutable_add_mana()->set_r(1);
                dev->mutable_add_mana()->set_c(2);
                sendRuled(cmd, QStringLiteral("dev: add {2}{R} for Act of Treason"));
                return;
            }
            if (devControlManaSent && !actOfTreasonCast) {
                if (const auto *act = handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Act of Treason"))) {
                    if (myPool.r >= 1 && myPool.total() >= 3) {
                        ruled::v1::RuledCommand cmd;
                        auto *cast = cmd.mutable_cast_spell();
                        cast->mutable_source()->set_hand_index(act->hand_index());
                        cast->add_targets()->set_object_id(controlTargetOid);
                        actOfTreasonCast = true;
                        sendRuled(cmd, QStringLiteral("cast Act of Treason on oid %1").arg(controlTargetOid));
                        return;
                    }
                }
            }
            // --- Dev commands (roadmap backlog dev-loop piece 2) ---------------------------
            // The only cross-language check that a C++-built DevCommand decodes and applies in
            // Rust; the behaviour itself is covered by the engine's scenario suite. Serra Angel
            // is in neither decklist, so this drives the whole conjure path: the mid-game catalog
            // refresh that lets the zone reconcile resolve an unknown name, and the physical
            // Server_Card the relay has to mint for it. If either were missing, the reconcile
            // would abandon its sync with only a qWarning and the permanent would never appear.
            if (!devConjureSent) {
                devConjureSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                auto *put = dev->mutable_put_card_in_zone();
                put->set_card_name("Serra Angel");
                put->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
                put->set_ready(true);
                sendRuled(cmd, QStringLiteral("dev: conjure Serra Angel onto the battlefield"));
                return;
            }
            if (sawControlReturn && sawOpponentCleanupDiscard && !paidSoftCounter) {
                if (!softCounterOrbRemoved) {
                    softCounterOrbRemoved = true;
                    ruled::v1::RuledCommand cmd;
                    auto *dev = cmd.mutable_dev_command();
                    dev->set_target_player_id(myId);
                    auto *move = dev->mutable_move_card();
                    move->set_card_name("Orb of Dreams");
                    move->set_zone(ruled::v1::DEV_ZONE_HAND);
                    sendRuled(cmd, QStringLiteral("dev: remove Orb of Dreams before Convolute setup"));
                    return;
                }
                if (countOwn(QStringLiteral("island"), false) < 4) {
                    ruled::v1::RuledCommand cmd;
                    auto *dev = cmd.mutable_dev_command();
                    dev->set_target_player_id(myId);
                    auto *put = dev->mutable_put_card_in_zone();
                    put->set_card_name("Island");
                    put->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
                    put->set_ready(true);
                    sendRuled(cmd, QStringLiteral("dev: add ready Island for Convolute payment"));
                    return;
                }
                if (!softCounterConvoluteConjured) {
                    softCounterConvoluteConjured = true;
                    ruled::v1::RuledCommand cmd;
                    auto *dev = cmd.mutable_dev_command();
                    dev->set_target_player_id(oppId);
                    auto *put = dev->mutable_put_card_in_zone();
                    put->set_card_name("Convolute");
                    put->set_zone(ruled::v1::DEV_ZONE_HAND);
                    sendRuled(cmd, QStringLiteral("dev: conjure Convolute for opponent"));
                    return;
                }
                if (!softCounterManaGranted) {
                    softCounterManaGranted = true;
                    ruled::v1::RuledCommand cmd;
                    auto *dev = cmd.mutable_dev_command();
                    dev->set_target_player_id(oppId);
                    dev->mutable_add_mana()->set_u(1);
                    dev->mutable_add_mana()->set_c(2);
                    sendRuled(cmd, QStringLiteral("dev: add {2}{U} for opponent's Convolute"));
                    return;
                }
                if (!softCounterBoltConjured) {
                    softCounterBoltConjured = true;
                    ruled::v1::RuledCommand cmd;
                    auto *dev = cmd.mutable_dev_command();
                    dev->set_target_player_id(myId);
                    auto *put = dev->mutable_put_card_in_zone();
                    put->set_card_name("Lightning Bolt");
                    put->set_zone(ruled::v1::DEV_ZONE_HAND);
                    sendRuled(cmd, QStringLiteral("dev: conjure Bolt for Convolute scenario"));
                    return;
                }
                if (!softCounterBoltCast) {
                    if (myPool.r < 1) {
                        if (const auto mountain = firstOwnUntapped(QStringLiteral("mountain"))) {
                            ruled::v1::RuledCommand cmd;
                            auto *ability = cmd.mutable_activate_ability();
                            setBattlefieldAbilitySource(ability, *mountain);
                            ability->set_ability_index(0);
                            sendRuled(cmd, QStringLiteral("tap Mountain for soft-counter Bolt"));
                            return;
                        }
                    }
                    if (const auto *bolt = handAction(ruled::v1::HAND_ACTION_CAST_SPELL,
                                                      QStringLiteral("Lightning Bolt"))) {
                        ruled::v1::RuledCommand cmd;
                        auto *cast = cmd.mutable_cast_spell();
                        cast->mutable_source()->set_hand_index(bolt->hand_index());
                        cast->add_targets()->set_object_id(static_cast<quint32>(oppId));
                        softCounterBoltCast = true;
                        sendRuled(cmd, QStringLiteral("cast Bolt for Convolute scenario"));
                        return;
                    }
                }
            }
            if (sawControlReturn && paidSoftCounter && sawSoftCounterResolveAfterChoice &&
                !sawEvolvingWildsPermanentMoved) {
                if (!devEvolvingWildsSent) {
                    devEvolvingWildsSent = true;
                    ruled::v1::RuledCommand cmd;
                    auto *dev = cmd.mutable_dev_command();
                    dev->set_target_player_id(myId);
                    auto *put = dev->mutable_put_card_in_zone();
                    put->set_card_name("Evolving Wilds");
                    put->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
                    put->set_ready(true);
                    sendRuled(cmd, QStringLiteral("dev: conjure Evolving Wilds onto the battlefield"));
                    return;
                }
                if (!evolvingWildsActivated) {
                    const auto wilds = firstOwnUntapped(QStringLiteral("evolving_wilds"));
                    if (wilds) {
                        ruled::v1::RuledCommand cmd;
                        auto *ability = cmd.mutable_activate_ability();
                        setBattlefieldAbilitySource(ability, *wilds);
                        ability->set_ability_index(0);
                        evolvingWildsActivated = true;
                        sendRuled(cmd, QStringLiteral("activate Evolving Wilds oid %1").arg(*wilds));
                        return;
                    }
                }
            }
            if (sawEvolvingWildsPermanentMoved && !sawLibraryTargetAbsentFromBattlefield) {
                if (!devTotallyLostSent) {
                    devTotallyLostSent = true;
                    ruled::v1::RuledCommand cmd;
                    auto *dev = cmd.mutable_dev_command();
                    dev->set_target_player_id(myId);
                    auto *put = dev->mutable_put_card_in_zone();
                    put->set_card_name("Totally Lost");
                    put->set_zone(ruled::v1::DEV_ZONE_HAND);
                    sendRuled(cmd, QStringLiteral("dev: conjure Totally Lost into hand"));
                    return;
                }
                if (!devTotallyLostManaSent) {
                    devTotallyLostManaSent = true;
                    ruled::v1::RuledCommand cmd;
                    auto *dev = cmd.mutable_dev_command();
                    dev->set_target_player_id(myId);
                    dev->mutable_add_mana()->set_u(1);
                    dev->mutable_add_mana()->set_c(4);
                    sendRuled(cmd, QStringLiteral("dev: add {4}{U} for Totally Lost"));
                    return;
                }
                if (!totallyLostCast) {
                    if (const auto *spell =
                            handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Totally Lost"))) {
                        ruled::v1::RuledCommand cmd;
                        auto *cast = cmd.mutable_cast_spell();
                        cast->mutable_source()->set_hand_index(spell->hand_index());
                        cast->add_targets()->set_object_id(controlTargetOid);
                        totallyLostCast = true;
                        sendRuled(cmd, QStringLiteral("cast Totally Lost on oid %1").arg(controlTargetOid));
                        return;
                    }
                }
            }
            if (!devWaifSent) {
                devWaifSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                auto *put = dev->mutable_put_card_in_zone();
                put->set_card_name("Reckless Waif");
                put->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
                put->set_ready(true);
                sendRuled(cmd, QStringLiteral("dev: conjure Reckless Waif onto the battlefield"));
                return;
            }
            if (!devBorosCharmSent) {
                devBorosCharmSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                auto *put = dev->mutable_put_card_in_zone();
                put->set_card_name("Boros Charm");
                put->set_zone(ruled::v1::DEV_ZONE_HAND);
                sendRuled(cmd, QStringLiteral("dev: conjure Boros Charm into hand"));
                return;
            }
            if (!devManaSent) {
                devManaSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                // Green: nothing in this deck produces or spends it, so the added mana cannot
                // change which spells the rest of the script decides it can afford.
                dev->mutable_add_mana()->set_g(2);
                dev->mutable_add_mana()->set_r(1);
                dev->mutable_add_mana()->set_w(1);
                sendRuled(cmd, QStringLiteral("dev: add {G}{G}{R}{W}"));
                return;
            }

            if (const auto *land = handAction(ruled::v1::HAND_ACTION_PLAY_LAND, QStringLiteral("Mountain"))) {
                ruled::v1::RuledCommand cmd;
                cmd.mutable_play_land()->set_hand_card_index(land->hand_index());
                sendRuled(cmd, QStringLiteral("play Mountain (idx %1)").arg(land->hand_index()));
                return;
            }
            if (sawOpponentCleanupDiscard && boltCast && !borosCharmCast && !devBorosCharmManaSent) {
                devBorosCharmManaSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                dev->mutable_add_mana()->set_r(1);
                dev->mutable_add_mana()->set_w(1);
                sendRuled(cmd, QStringLiteral("dev: add {R}{W} for post-combat Boros Charm"));
                return;
            }
            if (const auto *bolt =
                    !boltCast ? handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Lightning Bolt"))
                              : nullptr) {
                if (myPool.r >= 1) {
                    ruled::v1::RuledCommand cmd;
                    auto *cast = cmd.mutable_cast_spell();
                    cast->mutable_source()->set_hand_index(bolt->hand_index());
                    cast->add_targets()->set_object_id(static_cast<quint32>(oppId));
                    boltCast = true;
                    sendRuled(cmd, QStringLiteral("cast Lightning Bolt at player %1").arg(oppId));
                    return;
                }
                if (const auto oid = firstOwnUntapped(QStringLiteral("mountain"))) {
                    ruled::v1::RuledCommand cmd;
                    auto *ability = cmd.mutable_activate_ability();
                    setBattlefieldAbilitySource(ability, *oid);
                    ability->set_ability_index(0);
                    sendRuled(cmd, QStringLiteral("tap Mountain oid %1 (for Bolt)").arg(*oid));
                    return;
                }
            }
            if (const auto *charm =
                    boltCast && !borosCharmCast
                        ? handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Boros Charm"))
                        : nullptr) {
                if (myPool.r >= 1 && myPool.w >= 1) {
                    ruled::v1::RuledCommand cmd;
                    auto *cast = cmd.mutable_cast_spell();
                    cast->mutable_source()->set_hand_index(charm->hand_index());
                    auto *mode = cast->add_selected_modes();
                    mode->set_mode_index(0);
                    mode->add_targets()->set_object_id(static_cast<quint32>(oppId));
                    borosCharmCast = true;
                    sendRuled(cmd, QStringLiteral("cast Boros Charm damage mode at player %1").arg(oppId));
                    return;
                }
            }
            if (const auto *giant = boltCast && !giantCast
                                        ? handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Hill Giant"))
                                        : nullptr) {
                if (myPool.total() >= 4) {
                    ruled::v1::RuledCommand cmd;
                    cmd.mutable_cast_spell()->mutable_source()->set_hand_index(giant->hand_index());
                    giantCast = true;
                    sendRuled(cmd, QStringLiteral("cast Hill Giant"));
                    return;
                }
                if (myPool.total() < 4 && firstOwnUntapped(QStringLiteral("mountain"))) {
                    const auto oid = firstOwnUntapped(QStringLiteral("mountain"));
                    ruled::v1::RuledCommand cmd;
                    auto *ability = cmd.mutable_activate_ability();
                    setBattlefieldAbilitySource(ability, *oid);
                    ability->set_ability_index(0);
                    sendRuled(cmd, QStringLiteral("tap Mountain oid %1 (for Giant)").arg(*oid));
                    return;
                }
            }
        }

        if (role == Role::Hoarder && inMain && stackDepth == 0) {
            if (tryFlashbackSequence()) {
                return;
            }
            if (const auto *land = countOwn(QStringLiteral("island"), false) < 1
                                       ? handAction(ruled::v1::HAND_ACTION_PLAY_LAND, QStringLiteral("Island"))
                                       : nullptr) {
                ruled::v1::RuledCommand cmd;
                cmd.mutable_play_land()->set_hand_card_index(land->hand_index());
                sendRuled(cmd, QStringLiteral("play Island (idx %1)").arg(land->hand_index()));
                return;
            }
            if (const auto *brainstorm =
                    !brainstormCast ? handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Brainstorm"))
                                    : nullptr) {
                if (myPool.u >= 1) {
                    ruled::v1::RuledCommand cmd;
                    cmd.mutable_cast_spell()->mutable_source()->set_hand_index(brainstorm->hand_index());
                    brainstormCast = true;
                    sendRuled(cmd, QStringLiteral("cast Brainstorm"));
                    return;
                }
                if (const auto oid = firstOwnUntapped(QStringLiteral("island"))) {
                    ruled::v1::RuledCommand cmd;
                    auto *ability = cmd.mutable_activate_ability();
                    setBattlefieldAbilitySource(ability, *oid);
                    ability->set_ability_index(0);
                    sendRuled(cmd, QStringLiteral("tap Island oid %1 (for Brainstorm)").arg(*oid));
                    return;
                }
            }
        }

        // Default: pass priority.
        if (labelMatching(QRegularExpression(QStringLiteral("^Pass priority$")))) {
            ruled::v1::RuledCommand cmd;
            cmd.mutable_pass_priority();
            sendRuled(cmd, QStringLiteral("pass priority"));
        }
    }
};

class RuledE2ESmokeTest : public ::testing::Test
{
protected:
    QTemporaryDir tempDir;
    QProcess sidecar;
    QProcess servatrice;
    QStringList transcript;
    QByteArray servatriceStderr;
    QByteArray sidecarStderr;

    void collectServerLogs()
    {
        // QProcess pipe data reaches the internal buffers only when events are processed;
        // this harness runs no event loop, so pump one explicitly before reading.
        QCoreApplication::processEvents();
        servatriceStderr += servatrice.readAllStandardError();
        sidecarStderr += sidecar.readAllStandardError();
    }

    void TearDown() override
    {
        collectServerLogs();
        if (servatrice.state() != QProcess::NotRunning) {
            servatrice.kill();
            servatrice.waitForFinished(5000);
        }
        if (sidecar.state() != QProcess::NotRunning) {
            sidecar.kill();
            sidecar.waitForFinished(5000);
        }
        if (::testing::Test::HasFailure()) {
            fprintf(stderr, "---- E2E transcript (%d lines) ----\n", static_cast<int>(transcript.size()));
            for (const QString &line : transcript) {
                fprintf(stderr, "%s\n", line.toUtf8().constData());
            }
            collectServerLogs();
            fprintf(stderr, "---- servatrice stderr ----\n%s\n", servatriceStderr.constData());
            fprintf(stderr, "---- tricerules-server stderr ----\n%s\n", sidecarStderr.constData());
        }
    }

    QString writeServatriceIni()
    {
        const QString path = tempDir.filePath(QStringLiteral("servatrice-e2e.ini"));
        QFile f(path);
        EXPECT_TRUE(f.open(QIODevice::WriteOnly | QIODevice::Text));
        const QByteArray ini =
            "[server]\n"
            "name=\"ruled e2e smoke\"\n"
            "id=1\n"
            "host=127.0.0.1\n"
            "port=" +
            QByteArray::number(kServatricePort) +
            "\n"
            "number_pools=1\n"
            "websocket_number_pools=0\n"
            "statusupdate=15000\n"
            "writelog=0\n"
            "clientkeepalive=1\n"
            "max_player_inactivity_time=9999\n"
            "idleclienttimeout=0\n"
            "requireclientid=false\n"
            "requiredfeatures=\"\"\n"
            "[authentication]\n"
            "method=none\n"
            "regonly=false\n"
            "[users]\n"
            "minnamelength=2\n"
            "maxnamelength=12\n"
            "allowlowercase=true\n"
            "allowuppercase=true\n"
            "allownumerics=true\n"
            "[database]\n"
            "type=none\n"
            "[rooms]\n"
            "method=config\n"
            "roomlist\\size=1\n"
            "roomlist\\1\\name=\"Smoke room\"\n"
            "roomlist\\1\\description=\"e2e\"\n"
            "roomlist\\1\\autojoin=false\n"
            "roomlist\\1\\joinmessage=\"\"\n"
            "roomlist\\1\\game_types\\size=0\n"
            "[game]\n"
            "max_game_inactivity_time=9999\n"
            "store_replays=false\n"
            "[security]\n"
            "enable_max_user_limit=false\n"
            "max_users_per_address=10\n"
            "message_counting_interval=10\n"
            "max_message_size_per_interval=100000\n"
            "max_message_count_per_interval=10000\n"
            "max_games_per_user=5\n"
            "command_counting_interval=10\n"
            "max_command_count_per_interval=10000\n"
            "[logging]\n"
            "enablelogquery=false\n";
        f.write(ini);
        f.close();
        return path;
    }

    /// @param sidecarIdleTimeoutSecs when non-empty, TRICERULES_IDLE_TIMEOUT_SECS for the sidecar,
    /// so a test can make the engine hang up on an idle game within its own runtime.
    ::testing::AssertionResult startServers(const QString &sidecarIdleTimeoutSecs = QString())
    {
        const QString sidecarExe = triceRulesExePath();
        const QString servatriceExe = servatriceExePath();
        const bool require = qgetenv("RULED_E2E_REQUIRE") == "1";
        if (sidecarExe.isEmpty() || !QFile::exists(sidecarExe)) {
            if (require) {
                return ::testing::AssertionFailure() << "tricerules-server binary not found: "
                                                     << sidecarExe.toStdString();
            }
            return ::testing::AssertionSuccess() << "SKIP:tricerules-server binary not found (build with "
                                                    "WITH_RULES_ENGINE or run cargo build --release): "
                                                 << sidecarExe.toStdString();
        }
        if (servatriceExe.isEmpty() || !QFile::exists(servatriceExe)) {
            if (require) {
                return ::testing::AssertionFailure() << "servatrice binary not found: "
                                                     << servatriceExe.toStdString();
            }
            return ::testing::AssertionSuccess() << "SKIP:servatrice binary not found (build with WITH_SERVER): "
                                                 << servatriceExe.toStdString();
        }

        QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
        env.insert(QStringLiteral("TRICERULES_PORT"), QString::number(kTriceRulesPort));
        env.insert(QStringLiteral("COCKATRICE_RULED_SEED"), QString::number(kForcedSeed));
        // Both halves of the dev-command gate: servatrice asks (COCKATRICE_RULED_DEV) and the
        // sidecar permits (TRICERULES_DEV_COMMANDS). Neither alone opens it — which is why both
        // go into the environment both processes inherit.
        env.insert(QStringLiteral("COCKATRICE_RULED_DEV"), QStringLiteral("1"));
        env.insert(QStringLiteral("TRICERULES_DEV_COMMANDS"), QStringLiteral("1"));
        if (!sidecarIdleTimeoutSecs.isEmpty()) {
            env.insert(QStringLiteral("TRICERULES_IDLE_TIMEOUT_SECS"), sidecarIdleTimeoutSecs);
        }

        sidecar.setProcessEnvironment(env);
        sidecar.start(sidecarExe, {});
        if (!sidecar.waitForStarted(10000)) {
            return ::testing::AssertionFailure() << "failed to start tricerules-server";
        }
        if (!waitForPortOpen(kTriceRulesPort, 30000)) {
            return ::testing::AssertionFailure() << "tricerules-server never opened port " << kTriceRulesPort;
        }

        servatrice.setProcessEnvironment(env);
        servatrice.setWorkingDirectory(tempDir.path());
        servatrice.start(servatriceExe, {QStringLiteral("--config"), writeServatriceIni()});
        if (!servatrice.waitForStarted(10000)) {
            return ::testing::AssertionFailure() << "failed to start servatrice";
        }
        if (!waitForPortOpen(kServatricePort, 30000)) {
            return ::testing::AssertionFailure() << "servatrice never opened port " << kServatricePort;
        }
        return ::testing::AssertionSuccess();
    }
};

TEST_F(RuledE2ESmokeTest, FullSeededGame)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    {
        // startServers signals "binary missing" via a success carrying a SKIP message.
        const std::string msg = started.message();
        if (msg.rfind("SKIP:", 0) == 0) {
            GTEST_SKIP() << msg.substr(5);
        }
    }

    SmokeClient p1(SmokeClient::Role::Aggressor, QStringLiteral("smokep1"), &transcript);
    SmokeClient p2(SmokeClient::Role::Hoarder, QStringLiteral("smokep2"), &transcript);

    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));

    const QString deckA = deckXml({{23, QStringLiteral("Mountain")},
                                   {1, QStringLiteral("Plains")},
                                   {8, QStringLiteral("Hill Giant")},
                                   {8, QStringLiteral("Lightning Bolt")}});
    const QString deckB = deckXml({{20, QStringLiteral("Island")},
                                   {12, QStringLiteral("Brainstorm")},
                                   {8, QStringLiteral("Merfolk of the Pearl Trident")}});
    const QString deckBad = deckXml({{39, QStringLiteral("Island")}, {1, QStringLiteral("Black Lotus")}});

    // --- Deck validation gate: unimplemented card blocks game start ---
    ASSERT_TRUE(p1.selectDeck(deckA));
    ASSERT_TRUE(p2.selectDeck(deckBad));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.notifyCustomCount > 0; }, 20000, "unimplemented-cards popup (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.notifyCustomCount > 0; }, 20000, "unimplemented-cards popup (p2)"));
    EXPECT_TRUE(p2.lastNotifyContent.contains(QStringLiteral("Black Lotus")))
        << "popup should name the unimplemented card: " << p2.lastNotifyContent.toStdString();
    EXPECT_FALSE(p1.gameStarted);
    EXPECT_FALSE(p2.gameStarted);

    // --- Swap to an implemented deck; game starts for real ---
    ASSERT_TRUE(p2.selectDeck(deckB));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "ruled game start (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "ruled game start (p2)"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());

    // --- Drive the scripted game until every milestone is observed ---
    const auto milestonesDone = [&] {
        return p2.sentBottom && p1.sawBattlefieldOmission && p2.sawBattlefieldOmission && p1.sawBoltPushWithTarget &&
               p1.sawManifestChoicePrivate && p2.sawManifestChoiceRedacted && p1.submittedManifestChoice &&
               p1.sawManifestPublicFaceDown && p2.sawManifestPublicFaceDown && p1.sawManifestPrivateIdentity &&
               p2.sawOpponentManifestIdentityEmpty && p1.sawManifestPhysicalFaceDown &&
               p2.sawManifestPhysicalFaceDown && p1.sawManifestFaceChanged && p2.sawManifestFaceChanged &&
               p1.sawManifestPhysicalFaceUp && p2.sawManifestPhysicalFaceUp &&
               p1.sawRoomCastDoorState && p2.sawRoomCastDoorState && p1.sawRoomFullyUnlocked &&
               p2.sawRoomFullyUnlocked && p1.sawRoomUnlockTrigger && p2.sawRoomUnlockTrigger &&
               p1.roomPhysicalIdentityContinuous && p2.roomPhysicalIdentityContinuous &&
               p1.hasRoomPhysicalAnnotation() && p2.hasRoomPhysicalAnnotation() &&
               p1.sawOwnTypecyclingAction && p2.sawOpponentTypecyclingActionRedacted &&
               p1.submittedTypecyclingChoice && p1.sawEmptyTypecyclingChoice &&
               p1.submittedEmptyTypecyclingChoice &&
               p1.sawOwnRenewAction && p2.sawOpponentRenewActionRedacted && p1.sawRenewGraveToExile &&
               p1.renewPhysicalIdentityContinuous && p1.sawRenewCounters &&
               p1.sawCursePlayerAttachment && p2.sawCursePlayerAttachment && p1.hasCursePhysicalAnnotation() &&
               p2.hasCursePhysicalAnnotation() &&
               p1.sawBoltLifeLoss && p1.sawBorosCharmPushWithMode && p1.sawBorosCharmLifeLoss &&
               p1.sawAttackersDeclared && p1.sawCombatLifeLoss && p2.sawBrainstormChoice &&
               p2.submittedBrainstormChoice && p2.sawBrainstormResolved && p2.sentCleanupDiscard &&
               p1.sawDevConjuredPermanent && p1.sawDevMana && p1.sawWaifFaceChanged && p2.sawWaifFaceChanged &&
               p1.sawWaifBackPt && p2.sawWaifBackPt && p1.sawFlashbackGraveToStack && p1.sawFlashbackStackToExile &&
               p1.sawAdventureStackToExile && p1.sawAdventureExileToStack && p1.sawAdventureStackToBattlefield &&
               p1.sawEntryReplacementChoice && p1.submittedEntryReplacementChoice && p1.sawDiregrafEnterTapped &&
               p1.sawDamagePreventionChoice && p1.submittedDamagePreventionChoice && p1.sawControlTransfer &&
               p1.sawProtectionBranchChoice && p1.submittedProtectionBranchChoice &&
               p1.sawProtectionHandToStack && !p1.protectionLeftStackBeforeChoice &&
               p1.sawProtectionStackToGraveAfterChoice &&
               p1.sawProtectionPhysicalAnnotation && p2.sawProtectionPhysicalAnnotation &&
               p1.sawControlReturn && p1.sawPhysicalControlTransfer && p1.sawPhysicalControlReturn &&
               p1.sawLibraryPermanentMoved && p2.sawLibraryPermanentMoved &&
               p1.sawLibraryTargetAbsentFromBattlefield && p2.sawLibraryTargetAbsentFromBattlefield &&
               p2.sawTopPermanentDrawn &&
               p1.sawOwnLibrarySearchCandidates && p2.sawOpponentLibrarySearchRedacted &&
               p1.submittedEvolvingWildsChoice && p1.sawEvolvingWildsPermanentMoved &&
               p2.sawEvolvingWildsPermanentMoved && p1.sawEvolvingWildsPhysicalDeckToTable &&
               p2.sawEvolvingWildsPhysicalDeckToTable &&
               p1.sawSoftCounterPaymentChoice && p1.activatedManaDuringSoftCounterPayment && p1.paidSoftCounter &&
               p1.sawSoftCounterResolveAfterChoice &&
               p2.softCounterConvoluteCast &&
               p2.sawFlashbackGraveToStack && p2.sawFlashbackStackToExile && p2.handSizeByPlayer.count(p2.myId) &&
               p2.handSizeByPlayer[p2.myId] <= 7;
    };
    QElapsedTimer deadline;
    deadline.start();
    while (!milestonesDone() && deadline.elapsed() < kOverallDeadlineMs) {
        p1.pump(25);
        p2.pump(25);
        p1.act();
        p2.act();
    }

    // The forced seed must have reached the engine (server-side only; the seed is never
    // broadcast to clients, so the check reads the sidecar's session-start log line).
    const QByteArray seedNeedle = "seed " + QByteArray::number(kForcedSeed);
    {
        QElapsedTimer logWait;
        logWait.start();
        while (!sidecarStderr.contains(seedNeedle) && logWait.elapsed() < 5000) {
            collectServerLogs();
        }
    }
    EXPECT_TRUE(sidecarStderr.contains(seedNeedle))
        << "tricerules-server never logged a session with the forced seed " << kForcedSeed;
    EXPECT_TRUE(p2.didMulligan) << "hoarder never took its scripted mulligan";
    EXPECT_TRUE(p1.sawDirectOpeningToMain1 && p2.sawDirectOpeningToMain1)
        << "both clients did not jump directly from opening to the same settled first main phase";
    EXPECT_EQ(p1.directSettledActivePlayer, p2.directSettledActivePlayer)
        << "clients disagreed on the player active in the directly published settled state";
    EXPECT_TRUE(p2.sawBottomAction && p2.sentBottom) << "London mulligan bottoming never happened";
    EXPECT_TRUE(p1.sawBattlefieldOmission && p2.sawBattlefieldOmission)
        << "no unchanged battlefield snapshot was omitted end to end";
    EXPECT_TRUE(p1.curseCast) << "Curse of Disturbance was never cast at the opposing player";
    EXPECT_TRUE(p1.sawCursePlayerAttachment && p2.sawCursePlayerAttachment)
        << "both clients did not receive the typed player attachment";
    ASSERT_NE(p1.curseOid, 0u);
    EXPECT_EQ(p1.curseOid, p2.curseOid) << "clients disagreed on the Curse engine ObjectId";
    ASSERT_TRUE(p1.serverCardByEngineOid.count(p1.curseOid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(p2.curseOid));
    EXPECT_EQ(p1.serverCardByEngineOid[p1.curseOid], p2.serverCardByEngineOid[p2.curseOid])
        << "clients disagreed on the Curse physical Server_Card mapping";
    EXPECT_TRUE(p1.hasCursePhysicalAnnotation() && p2.hasCursePhysicalAnnotation())
        << "both clients did not receive Enchanting: smokep2 for the same physical Curse";
    EXPECT_TRUE(p1.sawPhysicalTap && p2.sawPhysicalTap)
        << "a mana activation never produced a physical tapped-card event for both clients";
    EXPECT_TRUE(p1.sawPhysicalUntap && p2.sawPhysicalUntap)
        << "an untap step never produced a physical untapped-card event for both clients";
    EXPECT_TRUE(p1.sawBoltPushWithTarget) << "no targeted Lightning Bolt cast was observed on the stack";
    EXPECT_TRUE(p1.sawSoftCounterPaymentChoice) << "Convolute never produced its resolution payment choice";
    EXPECT_TRUE(p1.activatedManaDuringSoftCounterPayment)
        << "the Bolt controller never activated a mana ability during Convolute's parked resolution";
    EXPECT_TRUE(p1.paidSoftCounter) << "the Bolt controller never submitted PAY_MANA";
    EXPECT_FALSE(p1.softCounterLeftStackBeforeChoice)
        << "Convolute left the stack before its resolution payment was answered";
    EXPECT_TRUE(p1.sawSoftCounterResolveAfterChoice)
        << "Convolute did not leave the stack after its resolution payment completed";
    EXPECT_TRUE(p2.softCounterConvoluteCast) << "the responding client never cast Convolute";
    EXPECT_TRUE(p1.sawBoltLifeLoss) << "Lightning Bolt never dealt its 3 damage";
    EXPECT_TRUE(p1.sawBorosCharmPushWithMode) << "Boros Charm chosen-mode metadata was not observed on the stack";
    EXPECT_TRUE(p1.sawBorosCharmLifeLoss) << "Boros Charm's damage mode never dealt its 4 damage";
    EXPECT_TRUE(p1.sawAttackersDeclared) << "no combat with declared attackers was observed";
    EXPECT_TRUE(p1.sawCombatLifeLoss) << "combat damage never changed a life total";
    EXPECT_TRUE(p2.sawBrainstormChoice) << "Brainstorm's tier-3 resolution choice never arrived";
    EXPECT_TRUE(p2.sawBrainstormResolved) << "Brainstorm never finished resolving after the choice";
    EXPECT_TRUE(p1.sawDamagePreventionChoice) << "damage-prevention ordering choice never arrived";
    EXPECT_TRUE(p1.submittedDamagePreventionChoice) << "damage-prevention ordering choice was never submitted";
    EXPECT_TRUE(p1.sawProtectionBranchChoice)
        << "Apostle's Blessing never published its six protection-quality branches";
    EXPECT_TRUE(p1.submittedProtectionBranchChoice)
        << "the ruled client never selected protection from artifacts";
    EXPECT_TRUE(p1.sawProtectionHandToStack)
        << "Apostle's Blessing never moved from the physical hand to the stack";
    EXPECT_FALSE(p1.protectionLeftStackBeforeChoice)
        << "Apostle's Blessing left the physical stack before its resolution choice";
    EXPECT_TRUE(p1.sawProtectionStackToGraveAfterChoice)
        << "Apostle's Blessing did not leave the physical stack after its resolution choice";
    EXPECT_TRUE(p1.sawProtectionPhysicalAnnotation && p2.sawProtectionPhysicalAnnotation)
        << "both clients did not receive Protection from artifacts on the same physical permanent";
    ASSERT_NE(p1.protectionTargetOid, 0u);
    EXPECT_EQ(p1.protectionTargetOid, p2.protectionTargetOid)
        << "clients disagreed on the protected permanent's engine ObjectId";
    ASSERT_TRUE(p1.serverCardByEngineOid.count(p1.protectionTargetOid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(p2.protectionTargetOid));
    EXPECT_EQ(p1.serverCardByEngineOid[p1.protectionTargetOid],
              p2.serverCardByEngineOid[p2.protectionTargetOid])
        << "clients disagreed on the protected permanent's physical Server_Card mapping";
    EXPECT_TRUE(p1.sawEntryReplacementChoice) << "battlefield-entry replacement ordering choice never arrived";
    EXPECT_TRUE(p1.submittedEntryReplacementChoice)
        << "battlefield-entry replacement ordering choice was never submitted";
    EXPECT_TRUE(p1.sawDiregrafEnterTapped) << "Diregraf Ghoul did not physically enter tapped";
    EXPECT_TRUE(p1.actOfTreasonCast) << "Act of Treason was never cast";
    EXPECT_TRUE(p1.sawControlTransfer) << "the control target never entered the caster's battlefield view";
    EXPECT_TRUE(p1.sawPhysicalControlTransfer) << "the physical control target never crossed TABLE zones";
    EXPECT_TRUE(p1.sawControlReturn) << "the control target did not return at cleanup";
    EXPECT_TRUE(p1.sawPhysicalControlReturn) << "the physical control target did not return to its owner's TABLE";
    EXPECT_TRUE(p1.totallyLostCast) << "Totally Lost was never cast";
    EXPECT_TRUE(p1.sawLibraryPermanentMoved && p2.sawLibraryPermanentMoved)
        << "both clients did not receive the public battlefield-to-library move";
    EXPECT_TRUE(p1.sawLibraryTargetAbsentFromBattlefield && p2.sawLibraryTargetAbsentFromBattlefield)
        << "both clients did not remove the target from their battlefield views";
    EXPECT_TRUE(p2.sawTopPermanentDrawn) << "the owner did not draw the permanent placed on top";
    EXPECT_TRUE(p1.sawOwnLibrarySearchCandidates)
        << "Evolving Wilds' controller did not receive aligned private library candidates";
    EXPECT_TRUE(p2.sawOpponentLibrarySearchRedacted)
        << "the opponent received private Evolving Wilds library identities";
    EXPECT_TRUE(p1.submittedEvolvingWildsChoice) << "Evolving Wilds' private candidate was never selected";
    EXPECT_TRUE(p1.sawEvolvingWildsPermanentMoved && p2.sawEvolvingWildsPermanentMoved)
        << "both clients did not receive the public library-to-battlefield move";
    EXPECT_TRUE(p1.sawEvolvingWildsPhysicalDeckToTable && p2.sawEvolvingWildsPhysicalDeckToTable)
        << "the chosen physical Mountain did not move from DECK to TABLE for both clients";
    EXPECT_TRUE(p1.evolvingWildsPhysicalIdentityContinuous && p2.evolvingWildsPhysicalIdentityContinuous)
        << "Evolving Wilds moved a different physical card than the chosen library candidate";
    ASSERT_NE(p1.evolvingWildsChosenOid, 0u);
    EXPECT_EQ(p1.evolvingWildsChosenOid, p2.evolvingWildsChosenOid)
        << "clients disagreed on the searched-for Mountain's engine ObjectId";
    ASSERT_TRUE(p1.serverCardByEngineOid.count(p1.evolvingWildsChosenOid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(p2.evolvingWildsChosenOid));
    EXPECT_EQ(p1.serverCardByEngineOid[p1.evolvingWildsChosenOid],
              p2.serverCardByEngineOid[p2.evolvingWildsChosenOid])
        << "clients disagreed on the searched-for Mountain's physical Server_Card mapping";
    EXPECT_EQ(p1.serverCardByEngineOid[p1.evolvingWildsChosenOid], p1.evolvingWildsPhysicalCardId)
        << "the selected engine ObjectId was not bound to the physical DECK-to-TABLE card";
    EXPECT_EQ(p2.serverCardByEngineOid[p2.evolvingWildsChosenOid], p2.evolvingWildsPhysicalCardId)
        << "the opponent did not retain the same physical DECK-to-TABLE binding";
    EXPECT_TRUE(p1.flashbackCast) << "seat 1 never sent its flashback cast";
    EXPECT_TRUE(p2.flashbackCast) << "seat 2 never sent its flashback cast";
    // One of these two seats does not own the canonical stack, so its cast crosses players.
    EXPECT_TRUE(p1.sawFlashbackGraveToStack)
        << "seat 1's flashback card never physically moved graveyard -> stack";
    EXPECT_TRUE(p2.sawFlashbackGraveToStack)
        << "seat 2's flashback card never physically moved graveyard -> stack (cross-player move "
           "rejected? see ruledAllowsCrossPlayerMove)";
    EXPECT_TRUE(p1.sawFlashbackStackToExile)
        << "seat 1's flashback card never physically moved stack -> exile (CR 702.34a)";
    EXPECT_TRUE(p2.sawFlashbackStackToExile)
        << "seat 2's flashback card never physically moved stack -> exile (CR 702.34a)";
    EXPECT_TRUE(p1.stompCast) << "Stomp was never cast from hand";
    EXPECT_TRUE(p1.giantCastFromExile) << "Bonecrusher Giant was never cast from its exile permission";
    EXPECT_TRUE(p1.sawAdventureStackToExile) << "Stomp never physically moved stack -> exile";
    EXPECT_TRUE(p1.sawAdventureExileToStack) << "Bonecrusher Giant never physically moved exile -> stack";
    EXPECT_TRUE(p1.sawAdventureStackToBattlefield) << "Bonecrusher Giant never entered the battlefield";
    EXPECT_TRUE(p1.adventurePhysicalIdentityContinuous)
        << "Adventure casting moved a different physical card between zones";
    EXPECT_TRUE(p2.sawCleanupDiscardActions && p2.sentCleanupDiscard) << "cleanup discard never happened";
    ASSERT_TRUE(p2.handSizeByPlayer.count(p2.myId));
    EXPECT_LE(p2.handSizeByPlayer[p2.myId], 7) << "hand size not enforced after cleanup discard";

    // Battlefield object map / zone views should show both basic lands in play.
    EXPECT_GE(p1.countOwn(QStringLiteral("mountain"), false), 1) << "no Mountain on the aggressor's battlefield";
    EXPECT_GE(p2.countOwn(QStringLiteral("island"), false), 1) << "no Island on the hoarder's battlefield";

    // Dev commands crossed the language boundary: a C++-built DevCommand decoded in Rust, and its
    // effects came back through the ordinary event path. Serra Angel is in neither decklist, so
    // its presence also proves the mid-game catalog refresh and the minted Server_Card both work
    // — without them the zone reconcile would have bailed out silently.
    EXPECT_TRUE(p1.sawDevConjuredPermanent)
        << "dev conjure never put Serra Angel on the battlefield (check the servatrice log for "
           "'applyRuledEngineZoneView: count mismatch' or 'missing')";
    EXPECT_GE(p1.countOwn(QStringLiteral("serra_angel"), false), 1) << "conjured permanent missing at end of game";
    EXPECT_TRUE(p1.sawDevMana) << "dev mana never reached the aggressor's pool";
    EXPECT_TRUE(p1.sawManifestChoicePrivate && p2.sawManifestChoiceRedacted)
        << "manifest-dread candidates were not private to the deciding player";
    EXPECT_TRUE(p1.sawManifestPrivateIdentity && p2.sawOpponentManifestIdentityEmpty)
        << "face-down identity map was not restricted to the controller";
    EXPECT_TRUE(p1.sawManifestPublicFaceDown && p2.sawManifestPublicFaceDown)
        << "both clients did not receive the public face-down 2/2";
    EXPECT_TRUE(p1.sawManifestFaceChanged && p2.sawManifestFaceChanged)
        << "both clients did not receive the in-place turn-face-up change";
    EXPECT_TRUE(p1.sawManifestPhysicalFaceDown && p2.sawManifestPhysicalFaceDown &&
                p1.sawManifestPhysicalFaceUp && p2.sawManifestPhysicalFaceUp)
        << "the same physical card was not shown face down and then face up on both clients";
    EXPECT_TRUE(p1.sawManifestPhysicalFaceUpIdentity && p2.sawManifestPhysicalFaceUpIdentity)
        << "the face-up physical event did not immediately publish Hill Giant's display identity";
    EXPECT_TRUE(p1.sawRoomCastDoorState && p2.sawRoomCastDoorState &&
                p1.sawRoomFullyUnlocked && p2.sawRoomFullyUnlocked)
        << "both clients did not receive identical cast-door and fully-unlocked Room state";
    EXPECT_TRUE(p1.sawRoomUnlockTrigger && p2.sawRoomUnlockTrigger)
        << "the unlock action produced no physical stack object, but its resulting door trigger was not published";
    EXPECT_TRUE(p1.roomPhysicalIdentityContinuous && p2.roomPhysicalIdentityContinuous &&
                p1.roomServerCardId >= 0 && p1.roomServerCardId == p2.roomServerCardId)
        << "Room casting and unlocking did not preserve one physical Server_Card identity";
    EXPECT_TRUE(p1.hasRoomPhysicalAnnotation() && p2.hasRoomPhysicalAnnotation())
        << "both clients did not receive the fully unlocked Doors annotation";
    EXPECT_TRUE(p1.sawOwnTypecyclingAction && p2.sawOpponentTypecyclingActionRedacted)
        << "the hand ability was not published exclusively to its owner";
    EXPECT_TRUE(p1.submittedTypecyclingChoice && p1.sawTypecyclingHandToGrave && p1.sawTypecyclingDeckToHand &&
                p1.typecyclingPhysicalIdentityContinuous)
        << "Plainscycling physical flags: choice=" << p1.submittedTypecyclingChoice
        << " hand_to_grave=" << p1.sawTypecyclingHandToGrave
        << " deck_to_hand=" << p1.sawTypecyclingDeckToHand
        << " identity=" << p1.typecyclingPhysicalIdentityContinuous
        << " source_id=" << p1.typecyclingSourcePhysicalId
        << " chosen_id=" << p1.typecyclingChosenPhysicalId;
    EXPECT_TRUE(p1.sawEmptyTypecyclingChoice && p1.submittedEmptyTypecyclingChoice)
        << "the second Plainscycle did not publish and submit the explicit empty fail-to-find choice";
    EXPECT_TRUE(p1.sawOwnRenewAction && p2.sawOpponentRenewActionRedacted)
        << "the graveyard ability was not published exclusively to its owner";
    EXPECT_TRUE(p1.sawRenewGraveToExile && p1.renewPhysicalIdentityContinuous && p1.sawRenewCounters)
        << "Renew did not exile the same physical source and add two +1/+1 counters plus reach";
    EXPECT_EQ(p1.manifestServerCardId, p2.manifestServerCardId)
        << "clients disagreed on manifested Server_Card identity";
    EXPECT_TRUE(p1.sawWaifOnBattlefield && p2.sawWaifOnBattlefield)
        << "both clients did not receive the conjured Reckless Waif battlefield identity";
    EXPECT_NE(p1.waifOid, 0u);
    EXPECT_EQ(p1.waifOid, p2.waifOid) << "clients disagreed on the permanent's engine OID";
    EXPECT_TRUE(p1.sawWaifFaceChanged && p2.sawWaifFaceChanged)
        << "both clients did not receive the in-place Merciless Predator face change";
    EXPECT_TRUE(p1.sawWaifBackPt && p2.sawWaifBackPt)
        << "both clients did not receive Merciless Predator's 3/2 battlefield characteristics";

    if (::testing::Test::HasFailure()) {
        ADD_FAILURE() << "milestones incomplete after " << deadline.elapsed() << " ms; see transcript below";
    }
}

// When the sidecar hangs up an idle connection it frees the engine session, and no reconnect can
// rebuild it. Servatrice used to reconnect anyway: the fresh connection answered "no session",
// which is an ok=false the driver reports as a plain context error — invisible in the client. The
// game just froze, buttons doing nothing. It must announce the loss instead. Runs a real sidecar
// with a 1 s idle timeout so the drop happens inside the test.
TEST_F(RuledE2ESmokeTest, IdleEngineHangupIsAnnouncedRatherThanFreezingTheGame)
{
    const auto started = startServers(QStringLiteral("1"));
    if (!started) {
        FAIL() << started.message();
    }
    {
        const std::string msg = started.message();
        if (msg.rfind("SKIP:", 0) == 0) {
            GTEST_SKIP() << msg.substr(5);
        }
    }

    SmokeClient p1(SmokeClient::Role::Aggressor, QStringLiteral("idlep1"), &transcript);
    SmokeClient p2(SmokeClient::Role::Hoarder, QStringLiteral("idlep2"), &transcript);

    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));

    ASSERT_TRUE(p1.selectDeck(deckXml({{24, QStringLiteral("Mountain")}, {16, QStringLiteral("Hill Giant")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{24, QStringLiteral("Island")},
                                       {16, QStringLiteral("Merfolk of the Pearl Trident")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "ruled game start (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "ruled game start (p2)"));

    const int noticesBefore = p1.notifyCustomCount;

    // Idle past the sidecar's 1 s timeout: pump the client sockets (so the clients stay alive at the
    // Cockatrice protocol level) but never call act(), so no ruled command reaches the engine.
    QElapsedTimer idle;
    idle.start();
    while (idle.elapsed() < 2500) {
        p1.pump(25);
        p2.pump(25);
    }
    {
        QElapsedTimer logWait;
        logWait.start();
        while (!sidecarStderr.contains("dropping session") && logWait.elapsed() < 5000) {
            collectServerLogs();
            p1.pump(25);
        }
    }
    ASSERT_TRUE(sidecarStderr.contains("dropping session"))
        << "the sidecar never idled out, so this test proves nothing; sidecar log:\n"
        << sidecarStderr.constData();

    // The engine is gone. The next command must produce the disconnect notice, not silence.
    ruled::v1::RuledCommand cmd;
    cmd.mutable_pass_priority();
    p1.sendRuled(cmd, QStringLiteral("pass priority after engine hangup"));

    EXPECT_TRUE(p1.pumpUntil([&] { return p1.notifyCustomCount > noticesBefore; }, 20000,
                             "engine-disconnected popup after idle hangup"));
    EXPECT_TRUE(p1.lastNotifyContent.contains(QStringLiteral("rules engine"), Qt::CaseInsensitive))
        << "popup should explain the engine connection was lost, got: " << p1.lastNotifyContent.toStdString();
}

} // namespace

int main(int argc, char **argv)
{
    QCoreApplication app(argc, argv);
    ::testing::InitGoogleTest(&argc, argv);
    return RUN_ALL_TESTS();
}
