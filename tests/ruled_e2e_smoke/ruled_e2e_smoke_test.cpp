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

#include <libcockatrice/protocol/pb/command_deck_select.pb.h>
#include <libcockatrice/protocol/pb/command_ready_start.pb.h>
#include <libcockatrice/protocol/pb/command_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/commands.pb.h>
#include <libcockatrice/protocol/pb/event_game_joined.pb.h>
#include <libcockatrice/protocol/pb/event_game_state_changed.pb.h>
#include <libcockatrice/protocol/pb/event_list_games.pb.h>
#include <libcockatrice/protocol/pb/event_list_rooms.pb.h>
#include <libcockatrice/protocol/pb/event_move_card.pb.h>
#include <libcockatrice/protocol/pb/event_notify_user.pb.h>
#include <libcockatrice/protocol/pb/event_ruled_payload.pb.h>
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
    bool attackersSentThisCombat = false;
    bool blockersSentThisCombat = false;
    bool devConjureSent = false;
    bool devBorosCharmSent = false;
    bool devManaSent = false;

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
    bool sawCleanupDiscardActions = false;
    bool sentCleanupDiscard = false;
    bool sawBottomAction = false;
    bool sentBottom = false;
    bool sawDevConjuredPermanent = false;
    bool sawDevMana = false;
    quint32 boltOid = 0;
    quint32 borosCharmOid = 0;
    quint32 brainstormOid = 0;
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
                const QLatin1String exile(ZoneNames::EXILE);
                if (name == QLatin1String("Bump in the Night")) {
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
                } else if (from == grave || to == exile) {
                    // Any *other* card taking the flashback path is the wrong-card bug.
                    ADD_FAILURE() << "unexpected card on the flashback path: "
                                  << name.toStdString() << " " << from.toStdString() << " -> "
                                  << to.toStdString();
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
        for (const ruled::v1::RuledEvent &ev : batch.events()) {
            if (ev.has_phase_changed()) {
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
            } else if (ev.has_stack_resolved()) {
                stackDepth = std::max(0, stackDepth - 1);
                if (brainstormOid != 0 && ev.stack_resolved().object_id() == brainstormOid) {
                    sawBrainstormResolved = true;
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
                if (lc.delta() < 0 && inCombatDamageWindow && sawAttackersDeclared) {
                    sawCombatLifeLoss = true;
                }
            } else if (ev.has_attackers_declared()) {
                if (ev.attackers_declared().attacker_object_ids_size() > 0) {
                    sawAttackersDeclared = true;
                    log(QStringLiteral("attackers declared: %1 creature(s)")
                            .arg(ev.attackers_declared().attacker_object_ids_size()));
                }
            } else if (ev.has_trigger_order_required()) {
                const auto &tor = ev.trigger_order_required();
                if (tor.deciding_player_id() == myId && tor.candidates_size() > 0) {
                    pendingTriggerOrder = tor;
                    log(QStringLiteral("trigger order required: %1 candidates").arg(tor.candidates_size()));
                }
            } else if (ev.has_resolution_choice_required()) {
                const auto &rcr = ev.resolution_choice_required();
                if (rcr.deciding_player_id() == myId && rcr.candidate_object_ids_size() > 0) {
                    pendingChoice = rcr;
                    log(QStringLiteral("resolution choice: kind %1 min %2 max %3 ordered %4 candidates %5")
                            .arg(QString::fromStdString(ruled::v1::ChoiceKind_Name(rcr.choice_kind())))
                            .arg(rcr.min())
                            .arg(rcr.max())
                            .arg(rcr.ordered())
                            .arg(rcr.candidate_object_ids_size()));
                }
            } else if (ev.has_zone_view()) {
                for (const ruled::v1::RuledPerPlayerView &pp : ev.zone_view().per_player()) {
                    auto &bf = battlefieldByPlayer[pp.player_id()];
                    bf.clear();
                    for (const auto &battlefieldObject : pp.battlefield_objects()) {
                        Permanent perm;
                        perm.cardId = QString::fromStdString(battlefieldObject.card_id());
                        perm.oid = battlefieldObject.object_id();
                        perm.tapped = battlefieldObject.tapped();
                        perm.creature = battlefieldObject.is_creature();
                        perm.sick = battlefieldObject.summoning_sick();
                        perm.haste = std::find(battlefieldObject.keywords().begin(), battlefieldObject.keywords().end(),
                                               "Haste") != battlefieldObject.keywords().end();
                        bf.push_back(perm);
                    }
                    if (oppId < 0 && myId >= 0 && pp.player_id() != myId) {
                        oppId = pp.player_id();
                    }
                }
            } else if (ev.has_hand_slot_map()) {
                std::map<int, int> counts;
                for (const auto &entry : ev.hand_slot_map().entries()) {
                    ++counts[entry.player_id()];
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
                log(QStringLiteral("gamelog: %1").arg(QString::fromStdString(ev.log().text()).left(160)));
            }
        }
        const auto it = batch.legal_by_player().find(myId);
        if (it != batch.legal_by_player().end()) {
            labels.clear();
            for (const std::string &l : it->second.labels()) {
                labels.append(QString::fromStdString(l));
            }
            latestLegal = it->second;
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
            for (const auto &ga : latestLegal.graveyard_actions()) {
                if (QString::fromStdString(ga.card_name()) != QLatin1String("Bump in the Night")) {
                    continue;
                }
                if (myPool.r < 1 || myPool.total() < 6) {
                    break; // wait for the dev mana below to land
                }
                ruled::v1::RuledCommand cmd;
                auto *cast = cmd.mutable_cast_spell();
                cast->set_hand_card_index(ga.graveyard_index());
                cast->set_flashback(true);
                cast->add_targets()->set_object_id(static_cast<quint32>(oppId));
                flashbackCast = true;
                sendRuled(cmd, QStringLiteral("flashback Bump in the Night (gy idx %1) at player %2")
                                   .arg(ga.graveyard_index())
                                   .arg(oppId));
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
            if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_HAND_CARDS && rcr.ordered()) {
                sawBrainstormChoice = true;
            }
            ruled::v1::RuledCommand cmd;
            auto *choice = cmd.mutable_submit_resolution_choice();
            const int need = static_cast<int>(rcr.min());
            for (int i = 0; i < need && i < rcr.candidate_object_ids_size(); ++i) {
                choice->add_chosen_object_ids(rcr.candidate_object_ids(i));
            }
            pendingChoice.reset();
            submittedBrainstormChoice = true;
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
                    if (perm.creature && !perm.tapped && (!perm.sick || perm.haste)) {
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

        // --- Priority-gated actions ---
        if (priorityPlayer != myId) {
            return;
        }
        const bool inMain =
            (phase == ruled::v1::PHASE_ID_MAIN1 || phase == ruled::v1::PHASE_ID_MAIN2) && activePlayer == myId;

        if (role == Role::Aggressor && inMain && stackDepth == 0) {
            if (tryFlashbackSequence()) {
                return;
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
            if (const auto *bolt =
                    !boltCast ? handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Lightning Bolt"))
                              : nullptr) {
                if (myPool.r >= 1) {
                    ruled::v1::RuledCommand cmd;
                    auto *cast = cmd.mutable_cast_spell();
                    cast->set_hand_card_index(bolt->hand_index());
                    cast->add_targets()->set_object_id(static_cast<quint32>(oppId));
                    boltCast = true;
                    sendRuled(cmd, QStringLiteral("cast Lightning Bolt at player %1").arg(oppId));
                    return;
                }
                if (const auto oid = firstOwnUntapped(QStringLiteral("mountain"))) {
                    ruled::v1::RuledCommand cmd;
                    auto *ability = cmd.mutable_activate_ability();
                    ability->set_permanent_id(*oid);
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
                    cast->set_hand_card_index(charm->hand_index());
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
                    cmd.mutable_cast_spell()->set_hand_card_index(giant->hand_index());
                    giantCast = true;
                    sendRuled(cmd, QStringLiteral("cast Hill Giant"));
                    return;
                }
                if (myPool.total() < 4 && firstOwnUntapped(QStringLiteral("mountain"))) {
                    const auto oid = firstOwnUntapped(QStringLiteral("mountain"));
                    ruled::v1::RuledCommand cmd;
                    auto *ability = cmd.mutable_activate_ability();
                    ability->set_permanent_id(*oid);
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
                    cmd.mutable_cast_spell()->set_hand_card_index(brainstorm->hand_index());
                    brainstormCast = true;
                    sendRuled(cmd, QStringLiteral("cast Brainstorm"));
                    return;
                }
                if (const auto oid = firstOwnUntapped(QStringLiteral("island"))) {
                    ruled::v1::RuledCommand cmd;
                    auto *ability = cmd.mutable_activate_ability();
                    ability->set_permanent_id(*oid);
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

    const QString deckA = deckXml({{24, QStringLiteral("Mountain")},
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

    // --- Drive the scripted game until every milestone is observed ---
    const auto milestonesDone = [&] {
        return p2.sentBottom && p1.sawBoltPushWithTarget && p1.sawBoltLifeLoss &&
               p1.sawBorosCharmPushWithMode && p1.sawBorosCharmLifeLoss &&
               p1.sawAttackersDeclared && p1.sawCombatLifeLoss && p2.sawBrainstormChoice &&
               p2.submittedBrainstormChoice && p2.sawBrainstormResolved && p2.sentCleanupDiscard &&
               p1.sawDevConjuredPermanent && p1.sawDevMana && p1.sawFlashbackGraveToStack && p1.sawFlashbackStackToExile &&
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
    EXPECT_TRUE(p2.sawBottomAction && p2.sentBottom) << "London mulligan bottoming never happened";
    EXPECT_TRUE(p1.sawBoltPushWithTarget) << "no targeted Lightning Bolt cast was observed on the stack";
    EXPECT_TRUE(p1.sawBoltLifeLoss) << "Lightning Bolt never dealt its 3 damage";
    EXPECT_TRUE(p1.sawBorosCharmPushWithMode) << "Boros Charm chosen-mode metadata was not observed on the stack";
    EXPECT_TRUE(p1.sawBorosCharmLifeLoss) << "Boros Charm's damage mode never dealt its 4 damage";
    EXPECT_TRUE(p1.sawAttackersDeclared) << "no combat with declared attackers was observed";
    EXPECT_TRUE(p1.sawCombatLifeLoss) << "combat damage never changed a life total";
    EXPECT_TRUE(p2.sawBrainstormChoice) << "Brainstorm's tier-3 resolution choice never arrived";
    EXPECT_TRUE(p2.sawBrainstormResolved) << "Brainstorm never finished resolving after the choice";
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
