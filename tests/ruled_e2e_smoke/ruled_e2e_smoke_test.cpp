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
//   * owner-chosen battlefield-to-library placement: Uncharted Voyage lets the target's owner
//     choose Top, both clients see the public move, and only that owner sees its next draw
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
#include <algorithm>
#include <array>
#include <gtest/gtest.h>
#include <libcockatrice/protocol/pb/command_deck_select.pb.h>
#include <libcockatrice/protocol/pb/command_ready_start.pb.h>
#include <libcockatrice/protocol/pb/command_ruled_payload.pb.h>
#include <libcockatrice/protocol/pb/commands.pb.h>
#include <libcockatrice/protocol/pb/event_create_token.pb.h>
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
#include <libcockatrice/protocol/pb/game_event_container.pb.h>
#include <libcockatrice/protocol/pb/response.pb.h>
#include <libcockatrice/protocol/pb/room_commands.pb.h>
#include <libcockatrice/protocol/pb/room_event.pb.h>
#include <libcockatrice/protocol/pb/ruled_v1.pb.h>
#include <libcockatrice/protocol/pb/server_message.pb.h>
#include <libcockatrice/protocol/pb/session_commands.pb.h>
#include <libcockatrice/protocol/pb/session_event.pb.h>
#include <libcockatrice/utility/zone_names.h>
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
    int spellPaymentPreviewCount = 0;
    ruled::v1::SpellPaymentPreview spellPaymentPreview;

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
        bool planeswalker = false;
        bool battle = false;
        bool sick = false;
        bool haste = false;
        bool reach = false;
        int power = 0;
        int toughness = 0;
        int faceIndex = 0;
        bool faceDown = false;
        quint64 generation = 0;
        int loyalty = -1;
        int defense = -1;
        int battleProtector = -1;
        bool firstAbilityActivatable = false;
        int attachmentPlayerId = -1;
        std::array<bool, 2> roomDoors{false, false};
        int roomDoorCount = 0;
    };
    std::map<int, std::vector<Permanent>> battlefieldByPlayer;
    std::vector<ruled::v1::AttackAssignment> latestAttackPreviewAssignments;
    std::vector<ruled::v1::AttackAssignment> latestDeclaredAttackAssignments;
    std::vector<ruled::v1::AttackAssignment> latestAddedAttackAssignments;
    bool sawMobilizeDefenderChoice = false;
    bool sawMobilizeObserverWait = false;
    bool sawMobilizeTokenCreated = false;
    bool sawMobilizeTokenSacrificed = false;
    quint32 mobilizeTokenOid = 0;
    bool sawTappedOrdinaryTokenCreated = false;
    quint32 tappedOrdinaryTokenOid = 0;
    std::set<int> physicallyTappedCardIds;
    std::set<int> physicallyAttackingCardIds;
    // Actual legacy card presentation delivered to Qt, keyed by seat and physical id.
    std::map<std::pair<int, int>, std::pair<int, QString>> physicalRowAndPt;
    struct Pool
    {
        int w = 0, u = 0, b = 0, r = 0, g = 0, c = 0;
        int total() const
        {
            return w + u + b + r + g + c;
        }
    };
    Pool myPool;
    std::map<int, int> restrictedBlueByPlayer;
    bool sawRestrictedBlueMana = false;
    int specialActionManaSteps = 0;
    int specialActionRestrictedPayments = 0;
    std::optional<ruled::v1::ResolutionChoiceRequired> pendingChoice;
    std::optional<ruled::v1::ResolutionChoiceRequired> lastResolutionChoice;
    // CR 603.3b: the engine blocks on this until it is answered, so the bot must handle it or the
    // whole game deadlocks — every simultaneous multi-trigger board reaches it.
    std::optional<ruled::v1::TriggerOrderRequired> pendingTriggerOrder;
    std::optional<ruled::v1::TriggerNeedsTarget> pendingTriggerTarget;
    std::map<quint32, int> graveyardOwnerByEngineOid;

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
    bool wardManaFlowActive = false;
    bool wardDiscardFlowActive = false;
    quint32 wardManaSpellOid = 0;
    quint32 wardDiscardSpellOid = 0;
    quint32 wardDiscardSourceOid = 0;
    quint32 wardDiscardChosenOid = 0;
    int wardDiscardChosenServerCardId = -1;
    int wardDiscardMovedServerCardId = -1;
    bool sawWardManaAnnotation = false;
    bool sawWardDiscardAnnotation = false;
    bool sawWardManaCountered = false;
    bool sawWardDiscardPrivateCandidates = false;
    bool sawWardDiscardObserverRedaction = false;
    bool sawWardDiscardCardMoved = false;
    bool sawWardDiscardPhysicalHandToGrave = false;
    bool sawWardDiscardSpellResolved = false;
    bool sawWardDiscardSourceToHand = false;
    bool playerSetDiscardFlowActive = false;
    bool sawPlayerSetDiscardPrivateCandidates = false;
    bool sawPlayerSetDiscardObserverRedaction = false;
    quint32 playerSetDiscardChosenOid = 0;
    int playerSetDiscardChosenServerCardId = -1;
    bool optionalCastCostFlowActive = false;
    bool sawPrivateBeholdCandidates = false;
    bool sawBeholdCandidateRedaction = false;
    bool sawBeholdStackReceipt = false;
    bool sawActiveBeholdReveal = false;
    bool sawActiveBeholdRevealClosed = false;
    bool activeBeholdReveal = false;
    bool devCurseConjureSent = false;
    bool devCurseManaSent = false;
    bool devAggressiveVictimSent = false;
    bool devAggressiveLandVictimSent = false;
    bool devAggressiveConjureSent = false;
    bool devAggressiveManaSent = false;
    bool aggressiveCast = false;
    bool sawAggressivePublicReveal = false;
    bool sawAggressiveChooserMask = false;
    bool sawAggressiveObserverReadOnly = false;
    bool aggressivePublicRevealActive = false;
    bool sawAggressivePublicRevealClosed = false;
    QStringList aggressiveRevealNames;
    bool submittedAggressiveChoice = false;
    bool sawAggressiveExile = false;
    bool sawAggressiveCounter = false;
    bool sawAggressivePhysicalHandToExile = false;
    bool aggressivePhysicalIdentityContinuous = true;
    quint32 aggressiveChosenOid = 0;
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
    bool sawRoomPhysicalAnnotation = false;
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
    bool temporaryExileFlowActive = false;
    bool sawTemporaryExilePhysicalMove = false;
    bool sawTemporaryReturnPhysicalMove = false;
    int temporaryExilePhysicalCardId = -1;
    bool harmonizeFlowActive = false;
    bool normalWhisperFlowActive = false;
    bool sawHarmonizeGraveToStack = false;
    bool sawHarmonizeStackToExile = false;
    bool sawHarmonizeStackReceipt = false;
    bool sawNormalWhisperHandToStack = false;
    bool sawNormalWhisperStackToGrave = false;
    bool harmonizePhysicalIdentityContinuous = true;
    int harmonizePhysicalCardId = -1;
    quint32 harmonizeCreatureOid = 0;
    bool graveyardCohortFlowActive = false;
    bool sawOtherTriggerTargetsRedacted = false;
    bool graveyardCohortPhysicalIdentityContinuous = true;
    std::set<int> graveyardCohortExpectedPhysicalIds;
    std::set<int> graveyardCohortMovedPhysicalIds;
    int graveyardLibraryExpectedPhysicalId = -1;
    bool sawGraveyardToLibraryPhysicalMove = false;
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
    bool sawAdventurePermissionGroup = false;
    bool sawAdventureExileToStack = false;
    bool sawAdventureStackToBattlefield = false;
    bool adventurePhysicalIdentityContinuous = true;
    int adventurePhysicalCardId = -1;
    bool devOmenConjureSent = false;
    bool devOmenManaSent = false;
    bool sawOmenFaceActions = false;
    bool omenSuccessCast = false;
    bool sawOmenStackAnnotation = false;
    bool sawOmenLibraryDestination = false;
    bool sawOmenStackToLibrary = false;
    bool omenSuccessPhysicalIdentityContinuous = true;
    int omenSuccessPhysicalCardId = -1;
    quint32 omenSuccessOid = 0;
    bool devOmenFizzleTargetSent = false;
    bool devOmenFizzleConjureSent = false;
    bool devOmenFizzleBoltSent = false;
    bool devOmenFizzleManaSent = false;
    bool omenFizzleCast = false;
    bool omenFizzleBoltCast = false;
    bool sawOmenGraveyardDestination = false;
    bool sawOmenStackToGraveyard = false;
    bool omenFizzlePhysicalIdentityContinuous = true;
    int omenFizzlePhysicalCardId = -1;
    quint32 omenFizzleOid = 0;
    quint32 omenFizzleTargetOid = 0;
    bool omenSequenceEnabled = false;
    bool libraryDetailsStayedConcealed = true;
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
    bool sawOwnerPlacementChoice = false;
    bool submittedOwnerPlacementChoice = false;
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
    int sayItsNameConjured = 0;
    int sayItsNameMovedToGraveyard = 0;
    bool altanakConjuredToHand = false;
    bool altanakConjuredToLibrary = false;
    bool sayItsNameActivated = false;
    bool sawZoneScopeChoice = false;
    bool submittedZoneScopeChoice = false;
    bool sawOwnZoneSearchCandidates = false;
    bool sawOpponentZoneSearchRedacted = false;
    bool submittedZoneSearchChoice = false;
    bool sawAltanakEnterBattlefield = false;
    int sayItsNameGraveToExileCount = 0;
    bool devCruelTruthsSent = false;
    bool devCruelTruthsManaSent = false;
    bool cruelTruthsCast = false;
    bool sawOwnSurveilCandidates = false;
    bool sawOpponentSurveilRedacted = false;
    bool submittedSurveilDestination = false;
    bool sawSurveilPhysicalDeckToGrave = false;
    bool sawCruelTruthsResolved = false;
    bool sawCruelTruthsLifeLoss = false;
    quint32 cruelTruthsOid = 0;
    QString surveilChosenName;
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
    template <typename Pred>::testing::AssertionResult pumpUntil(Pred pred, int timeoutMs, const char *what)
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
                for (const auto &player : gsc.player_list()) {
                    for (const auto &zone : player.zone_list()) {
                        if (zone.name() != ZoneNames::TABLE) {
                            continue;
                        }
                        const int playerId = player.properties().player_id();
                        for (auto it = physicalRowAndPt.begin(); it != physicalRowAndPt.end();) {
                            if (it->first.first == playerId) {
                                it = physicalRowAndPt.erase(it);
                            } else {
                                ++it;
                            }
                        }
                        for (const auto &card : zone.card_list()) {
                            physicalRowAndPt[{playerId, card.id()}] = {card.y(), QString::fromStdString(card.pt())};
                        }
                    }
                }
            }
            if (ev.HasExtension(Event_CreateToken::ext)) {
                const auto &created = ev.GetExtension(Event_CreateToken::ext);
                if (created.zone_name() == ZoneNames::TABLE) {
                    physicalRowAndPt[{ev.player_id(), created.card_id()}] = {created.y(),
                                                                             QString::fromStdString(created.pt())};
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
                const auto oldKey = std::make_pair(mc.start_player_id(), mc.card_id());
                const QString oldPt = physicalRowAndPt.count(oldKey) ? physicalRowAndPt.at(oldKey).second : QString();
                if (from == table) {
                    physicalRowAndPt.erase(oldKey);
                }
                if (to == table) {
                    physicalRowAndPt[{mc.target_player_id(), mc.new_card_id()}] = {mc.y(), oldPt};
                }
                const int omenOwnerId = role == Role::Aggressor ? myId : oppId;
                if (wardDiscardFlowActive && from == hand && to == grave) {
                    wardDiscardMovedServerCardId = mc.card_id();
                    sawWardDiscardPhysicalHandToGrave = true;
                }
                if (from == stack && to == deck && mc.target_player_id() == omenOwnerId &&
                    omenSuccessPhysicalCardId >= 0 && mc.card_id() == omenSuccessPhysicalCardId) {
                    omenSuccessPhysicalIdentityContinuous = true;
                    omenSuccessPhysicalCardId = mc.new_card_id();
                    sawOmenStackToLibrary = true;
                }
                // A failed Omen's graveyard move is public, but its Event_MoveCard name can be
                // empty after the stack annotation is cleared. Follow the already-captured
                // physical card id instead of depending on presentation text.
                if (from == stack && to == grave && mc.target_player_id() == omenOwnerId &&
                    omenFizzlePhysicalCardId >= 0 && mc.card_id() == omenFizzlePhysicalCardId) {
                    omenFizzlePhysicalIdentityContinuous = true;
                    omenFizzlePhysicalCardId = mc.new_card_id();
                    sawOmenStackToGraveyard = true;
                }
                if (from == deck && to == table && mc.face_down()) {
                    sawManifestPhysicalFaceDown = true;
                    if (manifestServerCardId >= 0 && manifestServerCardId != mc.new_card_id()) {
                        ADD_FAILURE() << "manifest-dread face-down move changed physical card id";
                    }
                    manifestServerCardId = mc.new_card_id();
                }
                if (name == QLatin1String("Mountain") && from == deck && to == table) {
                    sawEvolvingWildsPhysicalDeckToTable = true;
                    sawEvolvingWildsPermanentMoved = true;
                    if (evolvingWildsPhysicalCardId >= 0 && mc.new_card_id() != evolvingWildsPhysicalCardId) {
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
                    // Library picker IDs are transient snapshot-local values, not persistent
                    // Server_Card IDs. Capture the physical identity from the authoritative move;
                    // the following HandSlotMap publication proves that exact object landed in hand.
                    typecyclingChosenPhysicalId = mc.new_card_id();
                }
                if (submittedSurveilDestination && !sawSurveilPhysicalDeckToGrave && from == deck && to == grave) {
                    sawSurveilPhysicalDeckToGrave = !surveilChosenName.isEmpty() && name == surveilChosenName;
                }
                if (renewActivated && !sawRenewGraveToExile && from == grave && to == exile) {
                    sawRenewGraveToExile = true;
                    renewPhysicalIdentityContinuous =
                        renewSourcePhysicalId >= 0 && mc.card_id() == renewSourcePhysicalId;
                }
                if (name == QLatin1String("Grizzly Bears") && from == hand && to == exile &&
                    (submittedAggressiveChoice || sawAggressivePublicReveal)) {
                    sawAggressiveExile = true;
                    sawAggressivePhysicalHandToExile = true;
                    aggressivePhysicalIdentityContinuous = mc.card_id() == mc.new_card_id();
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
                if (name.contains(QLatin1String("Dirgur Island Dragon"))) {
                    if (from == hand && to == stack && mc.start_player_id() == omenOwnerId) {
                        if (omenSuccessPhysicalCardId < 0) {
                            omenSuccessPhysicalCardId = mc.new_card_id();
                        } else if (omenFizzlePhysicalCardId < 0) {
                            omenFizzlePhysicalCardId = mc.new_card_id();
                        }
                    } else if (from == stack && to == deck && mc.target_player_id() == omenOwnerId) {
                        omenSuccessPhysicalIdentityContinuous = omenSuccessPhysicalCardId >= 0 &&
                                                                mc.card_id() == omenSuccessPhysicalCardId;
                        omenSuccessPhysicalCardId = mc.new_card_id();
                        sawOmenStackToLibrary = true;
                    } else if (from == stack && to == grave && mc.target_player_id() == omenOwnerId) {
                        omenFizzlePhysicalIdentityContinuous = omenFizzlePhysicalCardId >= 0 &&
                                                               mc.card_id() == omenFizzlePhysicalCardId;
                        omenFizzlePhysicalCardId = mc.new_card_id();
                        sawOmenStackToGraveyard = true;
                    }
                } else if (name.contains(QLatin1String("Bonecrusher Giant"))) {
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
                } else if (name == QLatin1String("Say Its Name")) {
                    if (from == grave && to == exile) {
                        ++sayItsNameGraveToExileCount;
                    }
                } else if (name == QLatin1String("Altanak, the Thrice-Called")) {
                    if (from == deck && to == table) {
                        sawAltanakEnterBattlefield = true;
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
                } else if (name == QLatin1String("Unending Whisper")) {
                    if (harmonizeFlowActive && from == grave && to == stack) {
                        harmonizePhysicalIdentityContinuous =
                            harmonizePhysicalCardId >= 0 && mc.card_id() == harmonizePhysicalCardId &&
                            mc.new_card_id() == harmonizePhysicalCardId;
                        harmonizePhysicalCardId = mc.new_card_id();
                        sawHarmonizeGraveToStack = true;
                    } else if (harmonizeFlowActive && from == stack && to == exile) {
                        harmonizePhysicalIdentityContinuous =
                            harmonizePhysicalIdentityContinuous && harmonizePhysicalCardId >= 0 &&
                            mc.card_id() == harmonizePhysicalCardId && mc.new_card_id() == harmonizePhysicalCardId;
                        harmonizePhysicalCardId = mc.new_card_id();
                        sawHarmonizeStackToExile = true;
                    } else if (normalWhisperFlowActive && from == hand && to == stack) {
                        sawNormalWhisperHandToStack = true;
                    } else if (normalWhisperFlowActive && from == stack && to == grave) {
                        sawNormalWhisperStackToGrave = true;
                    }
                } else if (temporaryExileFlowActive && name == QLatin1String("Grizzly Bears") &&
                           ((from == table && to == exile) || (from == exile && to == table))) {
                    const bool identityContinuous = temporaryExilePhysicalCardId >= 0 &&
                                                    mc.card_id() == temporaryExilePhysicalCardId &&
                                                    mc.new_card_id() == temporaryExilePhysicalCardId;
                    EXPECT_TRUE(identityContinuous)
                        << "temporary exile moved a different physical Grizzly Bears card";
                    sawTemporaryExilePhysicalMove =
                        sawTemporaryExilePhysicalMove || (from == table && to == exile);
                    sawTemporaryReturnPhysicalMove =
                        sawTemporaryReturnPhysicalMove || (from == exile && to == table);
                } else if (graveyardCohortFlowActive && from == grave && to == exile &&
                           graveyardCohortExpectedPhysicalIds.count(mc.card_id()) > 0) {
                    graveyardCohortPhysicalIdentityContinuous =
                        graveyardCohortPhysicalIdentityContinuous && mc.new_card_id() == mc.card_id();
                    graveyardCohortMovedPhysicalIds.insert(mc.card_id());
                } else if (graveyardCohortFlowActive && from == grave && to == deck &&
                           mc.card_id() == graveyardLibraryExpectedPhysicalId) {
                    graveyardCohortPhysicalIdentityContinuous =
                        graveyardCohortPhysicalIdentityContinuous && mc.new_card_id() == mc.card_id();
                    sawGraveyardToLibraryPhysicalMove = true;
                } else if (flashbackCast && (from == grave || to == exile) &&
                           name != QLatin1String("Sagu Pummeler") &&
                           !(name == QLatin1String("Grizzly Bears") &&
                             (submittedAggressiveChoice || sawAggressivePublicReveal))) {
                    // Any *other* card taking the flashback path is the wrong-card bug.
                    ADD_FAILURE() << "unexpected card on the flashback path: " << name.toStdString() << " "
                                  << from.toStdString() << " -> " << to.toStdString();
                }
            }
            if (ev.HasExtension(Event_SetCardAttr::ext)) {
                const auto &attr = ev.GetExtension(Event_SetCardAttr::ext);
                if (attr.attribute() == AttrPT) {
                    physicalRowAndPt[{ev.player_id(), attr.card_id()}].second =
                        QString::fromStdString(attr.attr_value());
                }
                if (attr.attribute() == AttrTapped) {
                    sawPhysicalTap = sawPhysicalTap || attr.attr_value() == "1";
                    sawPhysicalUntap = sawPhysicalUntap || attr.attr_value() == "0";
                    if (attr.attr_value() == "1") {
                        physicallyTappedCardIds.insert(attr.card_id());
                    } else {
                        physicallyTappedCardIds.erase(attr.card_id());
                    }
                } else if (attr.attribute() == AttrAttacking) {
                    if (attr.attr_value() == "1") {
                        physicallyAttackingCardIds.insert(attr.card_id());
                    } else {
                        physicallyAttackingCardIds.erase(attr.card_id());
                    }
                } else if (attr.attribute() == AttrAnnotation) {
                    annotationByServerCardId[attr.card_id()] = QString::fromStdString(attr.attr_value());
                    sawRoomPhysicalAnnotation =
                        sawRoomPhysicalAnnotation ||
                        QString::fromStdString(attr.attr_value())
                            .contains(QStringLiteral("Doors: Derelict Attic (unlocked), Widow's Walk (unlocked)"));
                    sawProtectionPhysicalAnnotation =
                        sawProtectionPhysicalAnnotation ||
                        QString::fromStdString(attr.attr_value()).contains(QStringLiteral("Protection from artifacts"));
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
        if (batch.has_spell_payment_preview()) {
            EXPECT_TRUE(batch.events().empty());
            EXPECT_TRUE(batch.legal_by_player().empty());
            ++spellPaymentPreviewCount;
            spellPaymentPreview = batch.spell_payment_preview();
            return;
        }
        ++stateVersion;
        const ruled::v1::PhaseId previousPhase = phase;
        int phaseEvents = 0;
        bool batchDeclaredAttackers = false;
        bool batchCombatDamage = false;
        bool batchHasPublicReveal = false;
        bool batchIsPreview = false;
        for (const ruled::v1::RuledEvent &ev : batch.events()) {
            batchIsPreview = batchIsPreview || ev.has_attackers_preview() || ev.has_blockers_preview();
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
                inCombatDamageWindow =
                    phase == ruled::v1::PHASE_ID_COMBAT_DAMAGE || phase == ruled::v1::PHASE_ID_FIRST_STRIKE_DAMAGE;
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
                if (cardId == QLatin1String("cruel_truths")) {
                    cruelTruthsOid = sp.object_id();
                }
                if (cardId == QLatin1String("unsummon")) {
                    if (wardManaFlowActive) {
                        wardManaSpellOid = sp.object_id();
                    } else if (wardDiscardFlowActive) {
                        wardDiscardSpellOid = sp.object_id();
                    }
                }
                if (cardId == QLatin1String("caustic_exhale")) {
                    sawBeholdStackReceipt =
                        std::any_of(sp.chosen_cast_cost_labels().begin(), sp.chosen_cast_cost_labels().end(),
                                    [](const std::string &label) { return label == "Behold a Dragon"; });
                }
                if (harmonizeFlowActive && cardId == QLatin1String("unending_whisper")) {
                    sawHarmonizeStackReceipt =
                        std::any_of(sp.chosen_cast_cost_labels().begin(), sp.chosen_cast_cost_labels().end(),
                                    [](const std::string &label) {
                                        const QString receipt = QString::fromStdString(label);
                                        return receipt.contains(QStringLiteral("Harmonize")) &&
                                               receipt.contains(QStringLiteral("reduce {2}"));
                                    }) &&
                        QString::fromStdString(sp.ability_annotation()).contains(QStringLiteral("Harmonize"));
                }
                const QString annotation = QString::fromStdString(sp.ability_annotation());
                if (annotation == QLatin1String("Ward {2}")) {
                    sawWardManaAnnotation = true;
                }
                if (annotation == QStringLiteral("Ward—Discard a card")) {
                    sawWardDiscardAnnotation = true;
                }
                if (cardId == QLatin1String("dirgur_island_dragon_skimming_strike")) {
                    if (omenSuccessOid == 0) {
                        omenSuccessOid = sp.object_id();
                    } else if (sp.object_id() != omenSuccessOid && omenFizzleOid == 0) {
                        omenFizzleOid = sp.object_id();
                    }
                    sawOmenStackAnnotation =
                        sawOmenStackAnnotation ||
                        (sp.description() == "Skimming Strike" && sp.ability_annotation() == "Skimming Strike");
                }
                if (sp.is_triggered() && QString::fromStdString(sp.ability_annotation())
                                             .contains(QStringLiteral("draw two cards"), Qt::CaseInsensitive)) {
                    sawRoomUnlockTrigger = true;
                }
            } else if (ev.has_stack_resolved()) {
                stackDepth = std::max(0, stackDepth - 1);
                if (brainstormOid != 0 && ev.stack_resolved().object_id() == brainstormOid) {
                    sawBrainstormResolved = true;
                }
                if (softCounterConvoluteOid != 0 && ev.stack_resolved().object_id() == softCounterConvoluteOid) {
                    softCounterLeftStackBeforeChoice = !sawSoftCounterPaymentChoice;
                    sawSoftCounterResolveAfterChoice = sawSoftCounterPaymentChoice;
                }
                if (cruelTruthsOid != 0 && ev.stack_resolved().object_id() == cruelTruthsOid) {
                    sawCruelTruthsResolved = true;
                }
                if (omenSuccessOid != 0 && ev.stack_resolved().object_id() == omenSuccessOid &&
                    ev.stack_resolved().destination() == ruled::v1::STACK_RESOLVE_DESTINATION_LIBRARY) {
                    sawOmenLibraryDestination = true;
                }
                if (omenFizzleOid != 0 && ev.stack_resolved().object_id() == omenFizzleOid &&
                    ev.stack_resolved().destination() == ruled::v1::STACK_RESOLVE_DESTINATION_GRAVEYARD) {
                    sawOmenGraveyardDestination = true;
                }
                if (wardDiscardSpellOid != 0 && ev.stack_resolved().object_id() == wardDiscardSpellOid) {
                    sawWardDiscardSpellResolved = true;
                }
            } else if (ev.has_stack_object_countered()) {
                stackDepth = std::max(0, stackDepth - 1);
                if (wardManaSpellOid != 0 && ev.stack_object_countered().object_id() == wardManaSpellOid) {
                    sawWardManaCountered = true;
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
                if (lc.delta() == -2 && cruelTruthsOid != 0) {
                    sawCruelTruthsLifeLoss = true;
                }
                if (lc.delta() < 0 && (batchDeclaredAttackers || batchCombatDamage)) {
                    sawCombatLifeLoss = true;
                }
            } else if (ev.has_attackers_declared()) {
                if (ev.attackers_declared().assignments_size() > 0) {
                    batchDeclaredAttackers = true;
                    sawAttackersDeclared = true;
                    latestDeclaredAttackAssignments.assign(ev.attackers_declared().assignments().begin(),
                                                           ev.attackers_declared().assignments().end());
                    log(QStringLiteral("attackers declared: %1 creature(s)")
                            .arg(ev.attackers_declared().assignments_size()));
                }
            } else if (ev.has_attackers_added()) {
                latestAddedAttackAssignments.assign(ev.attackers_added().assignments().begin(),
                                                     ev.attackers_added().assignments().end());
            } else if (ev.has_attackers_preview()) {
                latestAttackPreviewAssignments.assign(ev.attackers_preview().assignments().begin(),
                                                       ev.attackers_preview().assignments().end());
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
                    if (sawEvolvingWildsPhysicalDeckToTable && evolvingWildsPhysicalCardId >= 0 &&
                        entry.server_card_id() == evolvingWildsPhysicalCardId) {
                        if (evolvingWildsChosenOid != 0 && evolvingWildsChosenOid != entry.engine_object_id()) {
                            evolvingWildsPhysicalIdentityContinuous = false;
                        }
                        evolvingWildsChosenOid = entry.engine_object_id();
                    }
                }
            } else if (ev.has_graveyard_object_map()) {
                for (const auto &entry : ev.graveyard_object_map().entries()) {
                    serverCardByEngineOid[entry.engine_object_id()] = entry.server_card_id();
                    graveyardOwnerByEngineOid[entry.engine_object_id()] = entry.player_id();
                }
            } else if (ev.has_exile_object_map()) {
                for (const auto &entry : ev.exile_object_map().entries()) {
                    serverCardByEngineOid[entry.engine_object_id()] = entry.server_card_id();
                }
            } else if (ev.has_trigger_needs_target()) {
                const auto &trigger = ev.trigger_needs_target();
                if (trigger.controller_player_id() == myId) {
                    pendingTriggerTarget = trigger;
                } else if (graveyardCohortFlowActive) {
                    sawOtherTriggerTargetsRedacted = trigger.targets().groups_size() == 0;
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
                lastResolutionChoice = rcr;
                if (wardDiscardFlowActive && rcr.choice_kind() == ruled::v1::CHOICE_KIND_HAND_CARDS) {
                    if (rcr.deciding_player_id() == myId) {
                        bool sawBear = false;
                        for (int i = 0; i < rcr.candidate_names_size(); ++i) {
                            if (rcr.candidate_names(i) == "Grizzly Bears") {
                                sawBear = true;
                                wardDiscardChosenOid = rcr.candidate_object_ids(i);
                                wardDiscardChosenServerCardId = rcr.candidate_server_card_ids(i);
                            }
                        }
                        sawWardDiscardPrivateCandidates = sawBear && rcr.min() == 0 && rcr.max() == 1 &&
                                                          rcr.candidate_object_ids_size() ==
                                                              rcr.candidate_names_size() &&
                                                          rcr.candidate_object_ids_size() ==
                                                              rcr.candidate_server_card_ids_size();
                    } else {
                        sawWardDiscardObserverRedaction =
                            rcr.candidate_object_ids_size() == 0 && rcr.candidate_card_ids_size() == 0 &&
                            rcr.candidate_names_size() == 0 && rcr.candidate_server_card_ids_size() == 0 &&
                            rcr.prompt_text() == "Opponent is making a resolution choice.";
                    }
                }
                if (playerSetDiscardFlowActive &&
                    rcr.choice_kind() == ruled::v1::CHOICE_KIND_HAND_CARDS) {
                    if (rcr.deciding_player_id() == myId) {
                        int bearIndex = -1;
                        for (int i = 0; i < rcr.candidate_names_size(); ++i) {
                            if (rcr.candidate_names(i) == "Grizzly Bears") {
                                bearIndex = i;
                                break;
                            }
                        }
                        const bool aligned = rcr.candidate_object_ids_size() == rcr.candidate_card_ids_size() &&
                                             rcr.candidate_object_ids_size() == rcr.candidate_names_size() &&
                                             rcr.candidate_object_ids_size() == rcr.candidate_server_card_ids_size() &&
                                             rcr.candidate_object_ids_size() == rcr.candidate_selectable_size();
                        sawPlayerSetDiscardPrivateCandidates =
                            aligned && rcr.min() == 1 && rcr.max() == 1 && bearIndex >= 0 &&
                            rcr.candidate_selectable(bearIndex);
                        if (sawPlayerSetDiscardPrivateCandidates) {
                            playerSetDiscardChosenOid = rcr.candidate_object_ids(bearIndex);
                            playerSetDiscardChosenServerCardId = rcr.candidate_server_card_ids(bearIndex);
                        }
                    } else {
                        sawPlayerSetDiscardObserverRedaction =
                            rcr.candidate_object_ids_size() == 0 && rcr.candidate_card_ids_size() == 0 &&
                            rcr.candidate_names_size() == 0 && rcr.candidate_server_card_ids_size() == 0 &&
                            rcr.candidate_selectable_size() == 0 &&
                            rcr.prompt_text() == "Opponent is making a resolution choice.";
                    }
                }
                if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_SEARCH) {
                    if (rcr.deciding_player_id() == myId) {
                        if (emptyTypecyclingActivated && rcr.candidate_object_ids_size() == 0) {
                            sawEmptyTypecyclingChoice =
                                rcr.min() == 0 && rcr.max() == 1 && rcr.candidate_card_ids_size() == 0 &&
                                rcr.candidate_names_size() == 0 && rcr.candidate_server_card_ids_size() == 0;
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
                if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_ZONE_SEARCH) {
                    if (rcr.deciding_player_id() == myId) {
                        bool hasAltanak = false;
                        for (int i = 0; i < rcr.candidate_names_size(); ++i) {
                            hasAltanak = hasAltanak || rcr.candidate_names(i) == "Altanak, the Thrice-Called";
                        }
                        sawOwnZoneSearchCandidates =
                            hasAltanak && rcr.candidate_object_ids_size() == rcr.candidate_card_ids_size() &&
                            rcr.candidate_object_ids_size() == rcr.candidate_names_size() &&
                            rcr.candidate_object_ids_size() == rcr.candidate_server_card_ids_size() &&
                            rcr.candidate_object_ids_size() == rcr.candidate_source_zones_size();
                    } else {
                        sawOpponentZoneSearchRedacted =
                            rcr.candidate_object_ids_size() == 0 && rcr.candidate_card_ids_size() == 0 &&
                            rcr.candidate_names_size() == 0 && rcr.candidate_server_card_ids_size() == 0 &&
                            rcr.candidate_source_zones_size() == 0 &&
                            rcr.prompt_text() == "Opponent is making a resolution choice.";
                    }
                }
                if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_MANIFEST_DREAD) {
                    if (rcr.deciding_player_id() == myId) {
                        sawManifestChoicePrivate = rcr.candidate_object_ids_size() == 2 &&
                                                   rcr.candidate_names_size() == 2 &&
                                                   rcr.candidate_server_card_ids_size() == 2;
                    } else {
                        sawManifestChoiceRedacted =
                            rcr.candidate_object_ids_size() == 0 && rcr.candidate_card_ids_size() == 0 &&
                            rcr.candidate_names_size() == 0 && rcr.candidate_server_card_ids_size() == 0;
                    }
                }
                if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_LOOK) {
                    if (rcr.deciding_player_id() == myId) {
                        sawOwnSurveilCandidates =
                            rcr.candidate_object_ids_size() == 2 && rcr.candidate_card_ids_size() == 2 &&
                            rcr.candidate_names_size() == 2 && rcr.candidate_server_card_ids_size() == 2 &&
                            rcr.min() == 0 && rcr.max() == 2 && rcr.ordered();
                    } else {
                        sawOpponentSurveilRedacted =
                            rcr.candidate_object_ids_size() == 0 && rcr.candidate_card_ids_size() == 0 &&
                            rcr.candidate_names_size() == 0 && rcr.candidate_server_card_ids_size() == 0;
                    }
                }
                if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_OPPONENT_HAND) {
                    const bool publicReveal =
                        rcr.reveal_audience() == ruled::v1::RESOLUTION_REVEAL_AUDIENCE_ALL_PARTICIPANTS;
                    batchHasPublicReveal = batchHasPublicReveal || publicReveal;
                    if (rcr.deciding_player_id() == myId) {
                        bool hasEligibleBear = false;
                        bool hasLand = false;
                        for (int i = 0; i < rcr.candidate_names_size(); ++i) {
                            if (rcr.candidate_names(i) == "Grizzly Bears" && i < rcr.candidate_selectable_size() &&
                                rcr.candidate_selectable(i)) {
                                hasEligibleBear = true;
                            }
                            hasLand = hasLand || rcr.candidate_names(i) == "Island";
                        }
                        sawAggressiveChooserMask = publicReveal && rcr.has_revealed_zone_owner_player_id() &&
                                                   rcr.revealed_zone_owner_player_id() == oppId &&
                                                   rcr.candidate_object_ids_size() == rcr.candidate_names_size() &&
                                                   rcr.candidate_card_ids_size() == rcr.candidate_names_size() &&
                                                   rcr.candidate_names_size() == rcr.candidate_server_card_ids_size() &&
                                                   rcr.candidate_names_size() == rcr.candidate_selectable_size() &&
                                                   hasEligibleBear && hasLand;
                    } else {
                        bool hasBear = false;
                        bool hasLand = false;
                        for (const auto &name : rcr.candidate_names()) {
                            hasBear = hasBear || name == "Grizzly Bears";
                            hasLand = hasLand || name == "Island";
                        }
                        sawAggressiveObserverReadOnly =
                            publicReveal && rcr.has_revealed_zone_owner_player_id() &&
                            rcr.revealed_zone_owner_player_id() == myId &&
                            rcr.candidate_object_ids_size() == rcr.candidate_names_size() &&
                            rcr.candidate_card_ids_size() == rcr.candidate_names_size() &&
                            rcr.candidate_names_size() == rcr.candidate_server_card_ids_size() &&
                            rcr.candidate_selectable_size() == 0 && hasBear && hasLand &&
                            rcr.prompt_text() == "Opponent is making a resolution choice.";
                    }
                    if (publicReveal) {
                        sawAggressivePublicReveal = true;
                        aggressivePublicRevealActive = true;
                        aggressiveRevealNames.clear();
                        for (const auto &name : rcr.candidate_names()) {
                            aggressiveRevealNames.append(QString::fromStdString(name));
                        }
                    }
                }
                if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_ATTACKING_TOKEN_DEFENDER) {
                    if (rcr.deciding_player_id() == myId) {
                        sawMobilizeDefenderChoice = rcr.min() == 1 && rcr.max() == 1 &&
                                                    rcr.combat_defender_options_size() >= 2;
                    } else {
                        sawMobilizeObserverWait = rcr.combat_defender_options_size() == 0;
                    }
                }
                if (rcr.deciding_player_id() == myId &&
                    (rcr.candidate_object_ids_size() > 0 ||
                     (rcr.choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_SEARCH && rcr.min() == 0) ||
                     rcr.choice_kind() == ruled::v1::CHOICE_KIND_MANA_PAYMENT ||
                     rcr.choice_kind() == ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH ||
                     rcr.choice_kind() == ruled::v1::CHOICE_KIND_ATTACKING_TOKEN_DEFENDER)) {
                    pendingChoice = rcr;
                    log(QStringLiteral("resolution choice: kind %1 min %2 max %3 ordered %4 candidates %5")
                            .arg(QString::fromStdString(ruled::v1::ChoiceKind_Name(rcr.choice_kind())))
                            .arg(rcr.min())
                            .arg(rcr.max())
                            .arg(rcr.ordered())
                            .arg(rcr.candidate_object_ids_size()));
                }
            } else if (ev.has_active_public_reveal_snapshot()) {
                bool hasDragon = false;
                for (const auto &reveal : ev.active_public_reveal_snapshot().reveals()) {
                    hasDragon = hasDragon ||
                                (reveal.card_id() == "adult_gold_dragon" &&
                                 reveal.card_name() == "Adult Gold Dragon");
                }
                if (hasDragon) {
                    sawActiveBeholdReveal = true;
                    activeBeholdReveal = true;
                } else if (activeBeholdReveal) {
                    activeBeholdReveal = false;
                    sawActiveBeholdRevealClosed = true;
                }
            } else if (ev.has_token_created()) {
                const auto &token = ev.token_created();
                if (token.card_id() == "warrior_r_1_1") {
                    sawMobilizeTokenCreated = token.enters_tapped() && token.identity().name() == "Warrior" &&
                                              token.identity().pt() == "1/1";
                    mobilizeTokenOid = token.object_id();
                } else if (token.card_id() == "robot_c_2_2") {
                    sawTappedOrdinaryTokenCreated = token.enters_tapped() && token.identity().name() == "Robot" &&
                                                     token.identity().pt() == "2/2";
                    tappedOrdinaryTokenOid = token.object_id();
                }
            } else if (ev.has_permanent_moved()) {
                const auto &moved = ev.permanent_moved();
                if (mobilizeTokenOid != 0 && moved.object_id() == mobilizeTokenOid &&
                    moved.destination() == ruled::v1::PermanentMoved::DESTINATION_GRAVEYARD) {
                    sawMobilizeTokenSacrificed = true;
                }
                if (wardDiscardFlowActive && moved.card_id() == "grizzly_bears" &&
                    moved.destination() == ruled::v1::PermanentMoved::DESTINATION_GRAVEYARD &&
                    (wardDiscardChosenOid == 0 || moved.object_id() == wardDiscardChosenOid)) {
                    wardDiscardChosenOid = moved.object_id();
                    sawWardDiscardCardMoved = true;
                }
                if (wardDiscardSourceOid != 0 && moved.object_id() == wardDiscardSourceOid &&
                    moved.destination() == ruled::v1::PermanentMoved::DESTINATION_HAND) {
                    sawWardDiscardSourceToHand = true;
                }
                if (moved.destination() == ruled::v1::PermanentMoved::DESTINATION_EXILE &&
                    moved.card_id() == "grizzly_bears" && (submittedAggressiveChoice || sawAggressivePublicReveal)) {
                    aggressiveChosenOid = moved.object_id();
                    sawAggressiveExile = true;
                }
                if (moved.destination() == ruled::v1::PermanentMoved::DESTINATION_BATTLEFIELD &&
                    moved.card_id() == "mountain") {
                    sawEvolvingWildsPermanentMoved = true;
                    if (evolvingWildsChosenOid == 0) {
                        evolvingWildsChosenOid = moved.object_id();
                    } else if (evolvingWildsChosenOid != moved.object_id()) {
                        evolvingWildsPhysicalIdentityContinuous = false;
                    }
                }
                if (moved.card_id() == "altanak,_the_thrice-called" &&
                    moved.destination() == ruled::v1::PermanentMoved::DESTINATION_BATTLEFIELD) {
                    sawAltanakEnterBattlefield = true;
                }
                if (moved.card_id() == "say_its_name" &&
                    moved.destination() == ruled::v1::PermanentMoved::DESTINATION_EXILE) {
                    ++sayItsNameGraveToExileCount;
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
                    libraryDetailsStayedConcealed =
                        libraryDetailsStayedConcealed && pp.library_cards_size() == 0;
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
                            perm.planeswalker = battlefieldObject.is_planeswalker();
                            perm.battle = battlefieldObject.is_battle();
                            perm.sick = battlefieldObject.summoning_sick();
                            perm.power = static_cast<int>(battlefieldObject.power());
                            perm.toughness = static_cast<int>(battlefieldObject.toughness());
                            perm.faceIndex = static_cast<int>(battlefieldObject.face_up_index());
                            perm.faceDown = battlefieldObject.face_down();
                            perm.generation = battlefieldObject.zone_change_generation();
                            perm.loyalty = battlefieldObject.is_planeswalker()
                                               ? static_cast<int>(battlefieldObject.loyalty())
                                               : -1;
                            perm.defense = battlefieldObject.is_battle()
                                               ? static_cast<int>(battlefieldObject.defense())
                                               : -1;
                            perm.battleProtector = battlefieldObject.has_battle_protector_player_id()
                                                       ? battlefieldObject.battle_protector_player_id()
                                                       : -1;
                            perm.firstAbilityActivatable = battlefieldObject.activated_abilities_size() > 0 &&
                                                           battlefieldObject.activated_abilities(0).activatable();
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
                                sawRoomFullyUnlocked = sawRoomFullyUnlocked || (perm.roomDoors[0] && perm.roomDoors[1]);
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
                            perm.haste =
                                std::find(battlefieldObject.keywords().begin(), battlefieldObject.keywords().end(),
                                          "Haste") != battlefieldObject.keywords().end();
                            perm.reach =
                                std::find(battlefieldObject.keywords().begin(), battlefieldObject.keywords().end(),
                                          "Reach") != battlefieldObject.keywords().end();
                            bf.push_back(perm);
                            const int omenOwnerId = role == Role::Aggressor ? myId : oppId;
                            if (devOmenFizzleTargetSent && pp.player_id() == omenOwnerId &&
                                perm.cardId == QLatin1String("grizzly_bears")) {
                                omenFizzleTargetOid = std::max(omenFizzleTargetOid, perm.oid);
                            }
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
                            if (perm.oid == manifestOid && (submittedAggressiveChoice || sawAggressivePublicReveal) &&
                                perm.power >= 6 && perm.toughness >= 6) {
                                sawAggressiveCounter = true;
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
                int restrictedBlue = 0;
                for (const auto &group : mp.restricted_groups()) {
                    restrictedBlue += static_cast<int>(group.u());
                }
                restrictedBlueByPlayer[mp.player_id()] = restrictedBlue;
                sawRestrictedBlueMana = sawRestrictedBlueMana || restrictedBlue > 0;
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
                if (text == QStringLiteral("Uncharted Voyage puts Grizzly Bears on top of its owner's library.")) {
                    sawLibraryPermanentMoved = true;
                }
                log(QStringLiteral("gamelog: %1").arg(text.left(160)));
            }
        }
        if (!batchIsPreview && aggressivePublicRevealActive && !batchHasPublicReveal) {
            aggressivePublicRevealActive = false;
            sawAggressivePublicRevealClosed = true;
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
        if (optionalCastCostFlowActive && role == Role::Hoarder && oppId >= 0) {
            sawBeholdCandidateRedaction =
                sawBeholdCandidateRedaction || batch.legal_by_player().find(oppId) == batch.legal_by_player().end();
        }
        const auto it = batch.legal_by_player().find(myId);
        if (it != batch.legal_by_player().end()) {
            labels.clear();
            for (const std::string &l : it->second.labels()) {
                labels.append(QString::fromStdString(l));
            }
            latestLegal = it->second;
            for (const auto &group : latestLegal.exile_play_permission_groups()) {
                if (QString::fromStdString(group.source_label()).contains(QLatin1String("Bonecrusher Giant")) &&
                    group.object_ids_size() == 1) {
                    sawAdventurePermissionGroup = true;
                }
            }
            if (optionalCastCostFlowActive && role == Role::Aggressor) {
                const auto caustic = std::find_if(
                    latestLegal.hand_actions().begin(), latestLegal.hand_actions().end(), [](const auto &action) {
                        return action.kind() == ruled::v1::HAND_ACTION_CAST_SPELL &&
                               action.card_name() == "Caustic Exhale";
                    });
                if (caustic != latestLegal.hand_actions().end() &&
                    caustic->cost_choices().cast_cost_groups_size() == 1) {
                    const auto &group = caustic->cost_choices().cast_cost_groups(0);
                    sawPrivateBeholdCandidates =
                        group.options_size() == 2 && group.options(0).kind() == ruled::v1::CAST_COST_OPTION_KIND_BEHOLD &&
                        group.options(0).selectable() && group.options(0).valid_hand_indices_size() == 1;
                }
            }
            if (submittedTypecyclingChoice) {
                const auto plains = std::find_if(
                    latestLegal.hand_actions().begin(), latestLegal.hand_actions().end(), [](const auto &action) {
                        return QString::fromStdString(action.card_name()) == QLatin1String("Plains");
                    });
                if (plains != latestLegal.hand_actions().end()) {
                    const auto physical = handServerCardBySlot.find(static_cast<int>(plains->hand_index()));
                    sawTypecyclingDeckToHand = physical != handServerCardBySlot.end();
                    typecyclingPhysicalIdentityContinuous = typecyclingPhysicalIdentityContinuous &&
                                                            sawTypecyclingDeckToHand &&
                                                            typecyclingChosenPhysicalId >= 0 &&
                                                            physical->second == typecyclingChosenPhysicalId;
                }
            }
            const auto hasZoneAbility = [this](const QString &cardName, ruled::v1::AbilitySourceZone sourceZone) {
                return std::any_of(latestLegal.zone_ability_actions().begin(), latestLegal.zone_ability_actions().end(),
                                   [&](const auto &action) {
                                       return QString::fromStdString(action.card_name()) == cardName &&
                                              action.source_zone() == sourceZone;
                                   });
            };
            if (role == Role::Aggressor) {
                sawOwnTypecyclingAction =
                    sawOwnTypecyclingAction ||
                    hasZoneAbility(QStringLiteral("Shepherding Spirits"), ruled::v1::ABILITY_SOURCE_ZONE_HAND);
                sawOwnRenewAction = sawOwnRenewAction || hasZoneAbility(QStringLiteral("Sagu Pummeler"),
                                                                        ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD);
            } else {
                sawOpponentTypecyclingActionRedacted =
                    sawOpponentTypecyclingActionRedacted ||
                    !hasZoneAbility(QStringLiteral("Shepherding Spirits"), ruled::v1::ABILITY_SOURCE_ZONE_HAND);
                sawOpponentRenewActionRedacted =
                    sawOpponentRenewActionRedacted ||
                    !hasZoneAbility(QStringLiteral("Sagu Pummeler"), ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD);
            }
            if (role == Role::Hoarder && sawLibraryPermanentMoved &&
                std::any_of(latestLegal.hand_actions().begin(), latestLegal.hand_actions().end(),
                            [](const auto &action) {
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
            return ::testing::AssertionFailure() << "login failed with code " << responses[loginId].response_code();
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
            return ::testing::AssertionFailure() << "join room failed with code " << responses[joinId].response_code();
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
            return ::testing::AssertionFailure() << "deck select failed with code " << responses[id].response_code();
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
        ruled::v1::RuledCommand explicitCommand = cmd;
        if (explicitCommand.has_cast_spell() &&
            explicitCommand.cast_spell().cast_method() == ruled::v1::CAST_METHOD_UNSPECIFIED) {
            explicitCommand.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
        }
        explicitCommand.SerializeToString(&bytes);
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

    const ruled::v1::LegalHandAction *handAction(ruled::v1::HandActionKind kind,
                                                 const QString &cardName = QString()) const
    {
        for (const auto &action : latestLegal.hand_actions()) {
            if (action.kind() == kind &&
                (cardName.isEmpty() || QString::fromStdString(action.card_name()) == cardName)) {
                return &action;
            }
        }
        return nullptr;
    }

    const ruled::v1::LegalZoneAbilityAction *zoneAbilityAction(const QString &cardName,
                                                               ruled::v1::AbilitySourceZone sourceZone) const
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
        return sawRoomPhysicalAnnotation;
    }

    // Produce a real, separately tracked Peeper contribution for each special-action payment.
    bool prepareRestrictedSpecialActionMana(int paymentIndex)
    {
        const int firstStep = paymentIndex * 2;
        if (specialActionManaSteps == firstStep) {
            ruled::v1::RuledCommand cmd;
            auto *dev = cmd.mutable_dev_command();
            dev->set_target_player_id(myId);
            auto *put = dev->mutable_put_card_in_zone();
            put->set_card_name("Creeping Peeper");
            put->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
            put->set_ready(true);
            ++specialActionManaSteps;
            sendRuled(cmd, QStringLiteral("dev: conjure Creeping Peeper"));
            return false;
        }
        if (specialActionManaSteps == firstStep + 1) {
            if (const auto oid = firstOwnUntapped(QStringLiteral("creeping_peeper"))) {
                ruled::v1::RuledCommand cmd;
                auto *ability = cmd.mutable_activate_ability();
                setBattlefieldAbilitySource(ability, *oid);
                ability->set_ability_index(0);
                ++specialActionManaSteps;
                sendRuled(cmd, QStringLiteral("produce restricted Peeper mana"));
            }
            return false;
        }
        return true;
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
                cast->set_cast_method(ruled::v1::CAST_METHOD_FLASHBACK);
                cast->mutable_source()->set_graveyard_object_id(ga.object_id());
                cast->add_targets()->set_object_id(static_cast<quint32>(oppId));
                flashbackCast = true;
                sendRuled(
                    cmd,
                    QStringLiteral("flashback Bump in the Night (oid %1) at player %2").arg(ga.object_id()).arg(oppId));
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

    bool tryOmenSequence()
    {
        if (!omenSequenceEnabled) {
            return false;
        }
        if (!devOmenConjureSent) {
            devOmenConjureSent = true;
            ruled::v1::RuledCommand cmd;
            auto *dev = cmd.mutable_dev_command();
            dev->set_target_player_id(myId);
            auto *put = dev->mutable_put_card_in_zone();
            put->set_card_name("Dirgur Island Dragon // Skimming Strike");
            put->set_zone(ruled::v1::DEV_ZONE_HAND);
            sendRuled(cmd, QStringLiteral("dev: conjure first Omen into hand"));
            return true;
        }
        if (!devOmenManaSent) {
            devOmenManaSent = true;
            ruled::v1::RuledCommand cmd;
            auto *dev = cmd.mutable_dev_command();
            dev->set_target_player_id(myId);
            dev->mutable_add_mana()->set_u(2);
            sendRuled(cmd, QStringLiteral("dev: add {1}{U} for zero-target Skimming Strike"));
            return true;
        }
        if (!omenSuccessCast) {
            const auto *normal = handAction(ruled::v1::HAND_ACTION_CAST_SPELL,
                                            QStringLiteral("Dirgur Island Dragon"));
            const auto *omen = handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Skimming Strike"));
            if (normal && omen) {
                sawOmenFaceActions = normal->face_index() == 0u && normal->cost() == "{5}{U}" &&
                                     omen->face_index() == 1u && omen->cost() == "{1}{U}" && omen->needs_target();
                ruled::v1::RuledCommand cmd;
                auto *cast = cmd.mutable_cast_spell();
                cast->mutable_source()->set_hand_index(omen->hand_index());
                cast->set_face_index(1u);
                omenSuccessCast = true;
                sendRuled(cmd, QStringLiteral("cast Skimming Strike with explicitly zero targets"));
                return true;
            }
            return true;
        }
        if (!sawOmenStackToLibrary) {
            return true;
        }
        if (!devOmenFizzleTargetSent) {
            devOmenFizzleTargetSent = true;
            ruled::v1::RuledCommand cmd;
            auto *dev = cmd.mutable_dev_command();
            dev->set_target_player_id(myId);
            auto *put = dev->mutable_put_card_in_zone();
            put->set_card_name("Grizzly Bears");
            put->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
            put->set_ready(true);
            sendRuled(cmd, QStringLiteral("dev: conjure the Omen fizzle target"));
            return true;
        }
        if (omenFizzleTargetOid == 0) {
            return true;
        }
        if (!devOmenFizzleConjureSent) {
            devOmenFizzleConjureSent = true;
            ruled::v1::RuledCommand cmd;
            auto *dev = cmd.mutable_dev_command();
            dev->set_target_player_id(myId);
            auto *put = dev->mutable_put_card_in_zone();
            put->set_card_name("Dirgur Island Dragon // Skimming Strike");
            put->set_zone(ruled::v1::DEV_ZONE_HAND);
            sendRuled(cmd, QStringLiteral("dev: conjure targeted Omen into hand"));
            return true;
        }
        if (!devOmenFizzleBoltSent) {
            devOmenFizzleBoltSent = true;
            ruled::v1::RuledCommand cmd;
            auto *dev = cmd.mutable_dev_command();
            dev->set_target_player_id(myId);
            auto *put = dev->mutable_put_card_in_zone();
            put->set_card_name("Lightning Bolt");
            put->set_zone(ruled::v1::DEV_ZONE_HAND);
            sendRuled(cmd, QStringLiteral("dev: conjure the Omen target-removal spell"));
            return true;
        }
        if (!devOmenFizzleManaSent) {
            devOmenFizzleManaSent = true;
            ruled::v1::RuledCommand cmd;
            auto *dev = cmd.mutable_dev_command();
            dev->set_target_player_id(myId);
            dev->mutable_add_mana()->set_u(2);
            dev->mutable_add_mana()->set_r(1);
            sendRuled(cmd, QStringLiteral("dev: add mana for targeted Omen plus Bolt"));
            return true;
        }
        if (!omenFizzleCast) {
            if (const auto *omen =
                    handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Skimming Strike"))) {
                ruled::v1::RuledCommand cmd;
                auto *cast = cmd.mutable_cast_spell();
                cast->mutable_source()->set_hand_index(omen->hand_index());
                cast->set_face_index(1u);
                auto *target = cast->add_targets();
                target->set_object_id(omenFizzleTargetOid);
                target->set_group_index(0u);
                target->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
                omenFizzleCast = true;
                sendRuled(cmd, QStringLiteral("cast Skimming Strike targeting oid %1").arg(omenFizzleTargetOid));
                return true;
            }
            return true;
        }
        return !sawOmenStackToGraveyard;
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
                const bool isProtection =
                    rcr.resolution_branches_size() == 6 && rcr.resolution_branches(0).label() == "artifacts";
                const bool isOwnerPlacement = rcr.resolution_branches_size() == 2 &&
                                              rcr.resolution_branches(0).label() == "Top" &&
                                              rcr.resolution_branches(1).label() == "Bottom";
                const bool isZoneScope =
                    rcr.resolution_branches_size() == 7 &&
                    std::all_of(rcr.resolution_branches().begin(), rcr.resolution_branches().end(),
                                [](const auto &branch) { return branch.search_zones_size() > 0; });
                if (isZoneScope) {
                    sawZoneScopeChoice = true;
                    const auto allZones = std::find_if(
                        rcr.resolution_branches().begin(), rcr.resolution_branches().end(), [](const auto &branch) {
                            return branch.search_zones_size() == 3;
                        });
                    if (allZones == rcr.resolution_branches().end()) {
                        ADD_FAILURE() << "zone-scope choice omitted the all-zones combination";
                        return;
                    }
                    ruled::v1::RuledCommand cmd;
                    auto *choice = cmd.mutable_submit_resolution_choice();
                    choice->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_SELECT_BRANCH);
                    choice->set_selected_branch_index(allZones->branch_index());
                    pendingChoice.reset();
                    submittedZoneScopeChoice = true;
                    sendRuled(cmd, QStringLiteral("search hand, graveyard, and library"));
                    return;
                }
                if (isOwnerPlacement) {
                    sawOwnerPlacementChoice = true;
                    ruled::v1::RuledCommand cmd;
                    auto *choice = cmd.mutable_submit_resolution_choice();
                    choice->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_SELECT_BRANCH);
                    choice->set_selected_branch_index(0);
                    pendingChoice.reset();
                    submittedOwnerPlacementChoice = true;
                    sendRuled(cmd, QStringLiteral("owner chooses top of library"));
                    return;
                }
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
                cmd.mutable_submit_resolution_choice()->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_PAY_MANA);
                paidSoftCounter = true;
                sendRuled(cmd, QStringLiteral("pay Convolute's resolution cost"));
                return;
            }
            const bool isReplacement = rcr.choice_kind() == ruled::v1::CHOICE_KIND_REPLACEMENT_EFFECT;
            const bool isLibrarySearch = rcr.choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_SEARCH;
            const bool isZoneSearch = rcr.choice_kind() == ruled::v1::CHOICE_KIND_ZONE_SEARCH;
            const bool isTypecycling = isLibrarySearch && typecyclingActivated && !submittedTypecyclingChoice;
            const bool isEmptyTypecycling =
                isLibrarySearch && emptyTypecyclingActivated && !submittedEmptyTypecyclingChoice;
            const bool isManifestDread = rcr.choice_kind() == ruled::v1::CHOICE_KIND_MANIFEST_DREAD;
            const bool isSurveil = rcr.choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_LOOK;
            const bool isOpponentHand = rcr.choice_kind() == ruled::v1::CHOICE_KIND_OPPONENT_HAND;
            const QString prompt = QString::fromStdString(rcr.prompt_text());
            const bool isEntryReplacement =
                isReplacement && prompt.contains(QStringLiteral("entering the battlefield"));
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
            const int need = ((isLibrarySearch || isZoneSearch) && rcr.candidate_object_ids_size() > 0) || isSurveil
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
            } else if (isOpponentHand) {
                int chosen = -1;
                for (int i = 0; i < rcr.candidate_names_size(); ++i) {
                    if (rcr.candidate_names(i) == "Grizzly Bears" && i < rcr.candidate_selectable_size() &&
                        rcr.candidate_selectable(i)) {
                        chosen = i;
                        break;
                    }
                }
                if (chosen < 0) {
                    ADD_FAILURE() << "opponent-hand candidates did not contain an eligible Grizzly Bears";
                    pendingChoice.reset();
                    return;
                }
                aggressiveChosenOid = rcr.candidate_object_ids(chosen);
                choice->add_chosen_object_ids(aggressiveChosenOid);
                submittedAggressiveChoice = true;
            } else {
                for (int i = 0; i < need && i < rcr.candidate_object_ids_size(); ++i) {
                    choice->add_chosen_object_ids(rcr.candidate_object_ids(i));
                }
            }
            if (isTypecycling && need == 1) {
                submittedTypecyclingChoice = true;
            } else if (isEmptyTypecycling && need == 0) {
                submittedEmptyTypecyclingChoice = true;
            } else if (isSurveil && need == 1) {
                // Library image IDs are picker-local sequential proxies, not persistent
                // Server_Card IDs. Exact hidden-zone identity is carried by the engine's
                // source_library_position on the ensuing PermanentMoved event.
                surveilChosenName = QString::fromStdString(rcr.candidate_names(0));
                submittedSurveilDestination = true;
            } else if (isLibrarySearch && need == 1) {
                evolvingWildsChosenOid = rcr.candidate_object_ids(0);
                submittedEvolvingWildsChoice = true;
            } else if (isZoneSearch && need == 1) {
                submittedZoneSearchChoice = true;
            }
            pendingChoice.reset();
            if (isDamagePrevention) {
                submittedDamagePreventionChoice = true;
            } else if (isEntryReplacement) {
                submittedEntryReplacementChoice = true;
            } else if (!isManifestDread && !isOpponentHand && !isTypecycling && !isEmptyTypecycling && !isSurveil &&
                       !isZoneSearch) {
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
                QSet<quint32> selectedAttackers;
                for (const auto &assignment : latestLegal.legal_attack_assignments()) {
                    const auto permanent = std::find_if(
                        it->second.cbegin(), it->second.cend(), [&assignment](const Permanent &candidate) {
                            return candidate.oid == assignment.attacker_object_id();
                        });
                    if (permanent != it->second.cend() &&
                        permanent->cardId != QStringLiteral("anti-venom,_horrifying_healer") &&
                        !selectedAttackers.contains(assignment.attacker_object_id())) {
                        *att->add_assignments() = assignment;
                        selectedAttackers.insert(assignment.attacker_object_id());
                    }
                }
            }
            attackersSentThisCombat = true;
            sendRuled(cmd, QStringLiteral("declare attackers (%1)").arg(att->assignments_size()));
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
            if (const auto *convolute = handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Convolute"))) {
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
                std::any_of(ownBattlefield->second.begin(), ownBattlefield->second.end(),
                            [](const Permanent &permanent) {
                                return permanent.cardId == QStringLiteral("diregraf_ghoul") && permanent.tapped;
                            });
        }

        // --- Priority-gated actions ---
        if (priorityPlayer != myId) {
            return;
        }
        const bool inMain =
            (phase == ruled::v1::PHASE_ID_MAIN1 || phase == ruled::v1::PHASE_ID_MAIN2) && activePlayer == myId;

        // Remove the targeted Omen's only target while that Omen is still on the stack. This
        // response sits outside the stack-empty main-phase script by design.
        if (role == Role::Aggressor && omenFizzleCast && !omenFizzleBoltCast && stackDepth == 1 &&
            priorityPlayer == myId) {
            if (const auto *bolt =
                    handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Lightning Bolt"))) {
                ruled::v1::RuledCommand cmd;
                auto *cast = cmd.mutable_cast_spell();
                cast->mutable_source()->set_hand_index(bolt->hand_index());
                auto *target = cast->add_targets();
                target->set_object_id(omenFizzleTargetOid);
                target->set_group_index(0u);
                target->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
                omenFizzleBoltCast = true;
                sendRuled(cmd, QStringLiteral("cast Bolt above Omen at oid %1").arg(omenFizzleTargetOid));
                return;
            }
        }

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
                // {1}{G} for Manifest Dread; two ordinary generic plus Peeper's {U} pay
                // the generic portion of Hill Giant's {3}{R} face-up special action.
                dev->mutable_add_mana()->set_c(3);
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
                if (!prepareRestrictedSpecialActionMana(0)) {
                    return;
                }
                for (const auto &action : latestLegal.permanent_actions()) {
                    if (action.kind() == ruled::v1::PERMANENT_ACTION_KIND_TURN_FACE_UP &&
                        action.object_id() == manifestOid) {
                        EXPECT_EQ(action.zone_change_generation(), manifestGeneration);
                        EXPECT_EQ(action.mana_cost(), "{3}{R}");
                        EXPECT_EQ(action.eligible_restricted_mana_group_ids_size(), 1);
                        if (action.eligible_restricted_mana_group_ids_size() != 1) {
                            return;
                        }
                        ruled::v1::RuledCommand cmd;
                        auto *turn = cmd.mutable_execute_permanent_action();
                        turn->set_kind(ruled::v1::PERMANENT_ACTION_KIND_TURN_FACE_UP);
                        turn->set_object_id(action.object_id());
                        turn->set_expected_zone_change_generation(action.zone_change_generation());
                        auto *payment = turn->add_restricted_mana();
                        payment->set_restriction_group_id(action.eligible_restricted_mana_group_ids(0));
                        payment->set_u(1);
                        ++specialActionRestrictedPayments;
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
                EXPECT_EQ(restrictedBlueByPlayer[myId], 0);
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
                dev->mutable_add_mana()->set_c(4);
                sendRuled(cmd, QStringLiteral("dev: add mana for both Room doors"));
                return;
            }
            if (!roomCast) {
                if (const auto *spell = handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Widow's Walk"))) {
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
                if (!prepareRestrictedSpecialActionMana(1)) {
                    return;
                }
                for (const auto &action : latestLegal.permanent_actions()) {
                    if (action.kind() == ruled::v1::PERMANENT_ACTION_KIND_UNLOCK_ROOM_DOOR &&
                        action.object_id() == roomOid && action.has_face_index() && action.face_index() == 0u) {
                        EXPECT_EQ(action.eligible_restricted_mana_group_ids_size(), 1);
                        if (action.eligible_restricted_mana_group_ids_size() != 1) {
                            return;
                        }
                        ruled::v1::RuledCommand cmd;
                        auto *unlock = cmd.mutable_execute_permanent_action();
                        unlock->set_kind(action.kind());
                        unlock->set_object_id(action.object_id());
                        unlock->set_expected_zone_change_generation(action.zone_change_generation());
                        unlock->set_face_index(action.face_index());
                        auto *payment = unlock->add_restricted_mana();
                        payment->set_restriction_group_id(action.eligible_restricted_mana_group_ids(0));
                        payment->set_u(1);
                        ++specialActionRestrictedPayments;
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
            EXPECT_EQ(restrictedBlueByPlayer[myId], 0);
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
                if (const auto *action =
                        zoneAbilityAction(QStringLiteral("Shepherding Spirits"), ruled::v1::ABILITY_SOURCE_ZONE_HAND)) {
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
                if (const auto *action =
                        zoneAbilityAction(QStringLiteral("Shepherding Spirits"), ruled::v1::ABILITY_SOURCE_ZONE_HAND)) {
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
                if (const auto *action =
                        zoneAbilityAction(QStringLiteral("Sagu Pummeler"), ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD)) {
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
                                                          return permanent.oid == manifestOid && permanent.power == 5 &&
                                                                 permanent.toughness == 5 && permanent.reach;
                                                      });
                    sawRenewCounters = sawRenewCounters || renewed != battlefield->second.end();
                }
                if (!sawRenewCounters) {
                    return;
                }
            }
            if (!devAggressiveLandVictimSent) {
                devAggressiveLandVictimSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(oppId);
                auto *put = dev->mutable_put_card_in_zone();
                put->set_card_name("Island");
                put->set_zone(ruled::v1::DEV_ZONE_HAND);
                sendRuled(cmd, QStringLiteral("dev: conjure public-reveal land into opponent hand"));
                return;
            }
            if (!devAggressiveVictimSent) {
                devAggressiveVictimSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(oppId);
                auto *put = dev->mutable_put_card_in_zone();
                put->set_card_name("Grizzly Bears");
                put->set_zone(ruled::v1::DEV_ZONE_HAND);
                sendRuled(cmd, QStringLiteral("dev: conjure Aggressive Negotiations victim into opponent hand"));
                return;
            }
            if (!devAggressiveConjureSent) {
                devAggressiveConjureSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                auto *put = dev->mutable_put_card_in_zone();
                put->set_card_name("Aggressive Negotiations");
                put->set_zone(ruled::v1::DEV_ZONE_HAND);
                sendRuled(cmd, QStringLiteral("dev: conjure Aggressive Negotiations into hand"));
                return;
            }
            if (!devAggressiveManaSent) {
                devAggressiveManaSent = true;
                ruled::v1::RuledCommand cmd;
                auto *dev = cmd.mutable_dev_command();
                dev->set_target_player_id(myId);
                dev->mutable_add_mana()->set_b(1);
                dev->mutable_add_mana()->set_c(2);
                sendRuled(cmd, QStringLiteral("dev: add {2}{B} for Aggressive Negotiations"));
                return;
            }
            if (!aggressiveCast) {
                if (const auto *spell =
                        handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Aggressive Negotiations"))) {
                    ruled::v1::RuledCommand cmd;
                    auto *cast = cmd.mutable_cast_spell();
                    cast->mutable_source()->set_hand_index(spell->hand_index());
                    auto *opponent = cast->add_targets();
                    opponent->set_object_id(static_cast<quint32>(oppId));
                    opponent->set_group_index(0);
                    opponent->set_kind(ruled::v1::TARGET_REF_KIND_PLAYER);
                    auto *creature = cast->add_targets();
                    creature->set_object_id(manifestOid);
                    creature->set_group_index(1);
                    creature->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
                    aggressiveCast = true;
                    sendRuled(
                        cmd,
                        QStringLiteral("cast Aggressive Negotiations targeting opponent and oid %1").arg(manifestOid));
                    return;
                }
            }
            if (aggressiveCast && !submittedAggressiveChoice) {
                return;
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
            if (tryOmenSequence()) {
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
                if (const auto *salve =
                        handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Healing Salve"))) {
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
                    if (const auto *bolt =
                            handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Lightning Bolt"))) {
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
            if (sawEvolvingWildsPermanentMoved && !sawAltanakEnterBattlefield) {
                if (sayItsNameConjured == sayItsNameMovedToGraveyard && sayItsNameConjured < 3) {
                    ++sayItsNameConjured;
                    ruled::v1::RuledCommand cmd;
                    auto *dev = cmd.mutable_dev_command();
                    dev->set_target_player_id(myId);
                    auto *put = dev->mutable_put_card_in_zone();
                    put->set_card_name("Say Its Name");
                    put->set_zone(ruled::v1::DEV_ZONE_HAND);
                    sendRuled(cmd, QStringLiteral("dev: conjure Say Its Name %1 into hand").arg(sayItsNameConjured));
                    return;
                }
                if (sayItsNameMovedToGraveyard < sayItsNameConjured) {
                    ++sayItsNameMovedToGraveyard;
                    ruled::v1::RuledCommand cmd;
                    auto *dev = cmd.mutable_dev_command();
                    dev->set_target_player_id(myId);
                    auto *move = dev->mutable_move_card();
                    move->set_card_name("Say Its Name");
                    move->set_zone(ruled::v1::DEV_ZONE_GRAVEYARD);
                    sendRuled(cmd,
                              QStringLiteral("dev: move Say Its Name %1 to graveyard")
                                  .arg(sayItsNameMovedToGraveyard));
                    return;
                }
                if (!altanakConjuredToHand) {
                    altanakConjuredToHand = true;
                    ruled::v1::RuledCommand cmd;
                    auto *dev = cmd.mutable_dev_command();
                    dev->set_target_player_id(myId);
                    auto *put = dev->mutable_put_card_in_zone();
                    put->set_card_name("Altanak, the Thrice-Called");
                    put->set_zone(ruled::v1::DEV_ZONE_HAND);
                    sendRuled(cmd, QStringLiteral("dev: conjure Altanak into hand"));
                    return;
                }
                if (!altanakConjuredToLibrary) {
                    altanakConjuredToLibrary = true;
                    ruled::v1::RuledCommand cmd;
                    auto *dev = cmd.mutable_dev_command();
                    dev->set_target_player_id(myId);
                    auto *move = dev->mutable_move_card();
                    move->set_card_name("Altanak, the Thrice-Called");
                    move->set_zone(ruled::v1::DEV_ZONE_LIBRARY);
                    sendRuled(cmd, QStringLiteral("dev: move Altanak into library"));
                    return;
                }
                if (!sayItsNameActivated) {
                    const auto *action =
                        zoneAbilityAction(QStringLiteral("Say Its Name"), ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD);
                    if (!action) {
                        return;
                    }
                    const quint64 key = (static_cast<quint64>(action->object_id()) << 32) | action->ability_index();
                    const auto &costsByAbility = latestLegal.cost_choices_by_ability();
                    const auto costsIt = costsByAbility.find(key);
                    if (costsIt == costsByAbility.end()) {
                        return;
                    }
                    const ruled::v1::LegalCostChoice *graveyardCost = nullptr;
                    for (const auto &cost : costsIt->second.choices()) {
                        if (cost.zone() == ruled::v1::COST_CHOICE_ZONE_GRAVEYARD && cost.min() == 2 &&
                            cost.max() == 2 && cost.candidate_ids_size() >= 2) {
                            graveyardCost = &cost;
                            break;
                        }
                    }
                    if (!graveyardCost) {
                        return;
                    }
                    ruled::v1::RuledCommand cmd;
                    auto *ability = cmd.mutable_activate_ability();
                    ability->set_source_object_id(action->object_id());
                    ability->set_source_zone(action->source_zone());
                    ability->set_expected_zone_change_generation(action->zone_change_generation());
                    ability->set_ability_index(action->ability_index());
                    auto *selection = ability->add_cost_selections();
                    selection->set_cost_index(graveyardCost->cost_index());
                    selection->mutable_graveyard_object_ids()->add_object_ids(graveyardCost->candidate_ids(0));
                    selection->mutable_graveyard_object_ids()->add_object_ids(graveyardCost->candidate_ids(1));
                    sayItsNameActivated = true;
                    sendRuled(cmd, QStringLiteral("activate Say Its Name with two exact graveyard cards"));
                    return;
                }
                if (!submittedZoneSearchChoice) {
                    return;
                }
            }
            if (sawAltanakEnterBattlefield && !sawLibraryTargetAbsentFromBattlefield) {
                if (!devTotallyLostSent) {
                    devTotallyLostSent = true;
                    ruled::v1::RuledCommand cmd;
                    auto *dev = cmd.mutable_dev_command();
                    dev->set_target_player_id(myId);
                    auto *put = dev->mutable_put_card_in_zone();
                    put->set_card_name("Uncharted Voyage");
                    put->set_zone(ruled::v1::DEV_ZONE_HAND);
                    sendRuled(cmd, QStringLiteral("dev: conjure Uncharted Voyage into hand"));
                    return;
                }
                if (!devTotallyLostManaSent) {
                    devTotallyLostManaSent = true;
                    ruled::v1::RuledCommand cmd;
                    auto *dev = cmd.mutable_dev_command();
                    dev->set_target_player_id(myId);
                    dev->mutable_add_mana()->set_u(1);
                    dev->mutable_add_mana()->set_c(3);
                    sendRuled(cmd, QStringLiteral("dev: add {3}{U} for Uncharted Voyage"));
                    return;
                }
                if (!totallyLostCast) {
                    if (const auto *spell =
                            handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Uncharted Voyage"))) {
                        ruled::v1::RuledCommand cmd;
                        auto *cast = cmd.mutable_cast_spell();
                        cast->mutable_source()->set_hand_index(spell->hand_index());
                        cast->add_targets()->set_object_id(controlTargetOid);
                        totallyLostCast = true;
                        sendRuled(cmd, QStringLiteral("cast Uncharted Voyage on oid %1").arg(controlTargetOid));
                        return;
                    }
                }
            }
            if (sawLibraryTargetAbsentFromBattlefield && !sawCruelTruthsResolved) {
                if (!devCruelTruthsSent) {
                    devCruelTruthsSent = true;
                    ruled::v1::RuledCommand cmd;
                    auto *dev = cmd.mutable_dev_command();
                    dev->set_target_player_id(myId);
                    auto *put = dev->mutable_put_card_in_zone();
                    put->set_card_name("Cruel Truths");
                    put->set_zone(ruled::v1::DEV_ZONE_HAND);
                    sendRuled(cmd, QStringLiteral("dev: conjure Cruel Truths into hand"));
                    return;
                }
                if (!devCruelTruthsManaSent) {
                    devCruelTruthsManaSent = true;
                    ruled::v1::RuledCommand cmd;
                    auto *dev = cmd.mutable_dev_command();
                    dev->set_target_player_id(myId);
                    dev->mutable_add_mana()->set_b(1);
                    dev->mutable_add_mana()->set_c(3);
                    sendRuled(cmd, QStringLiteral("dev: add {3}{B} for Cruel Truths"));
                    return;
                }
                if (!cruelTruthsCast) {
                    if (const auto *spell =
                            handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Cruel Truths"))) {
                        ruled::v1::RuledCommand cmd;
                        cmd.mutable_cast_spell()->mutable_source()->set_hand_index(spell->hand_index());
                        cruelTruthsCast = true;
                        sendRuled(cmd, QStringLiteral("cast Cruel Truths"));
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
                cmd.mutable_play_land()->mutable_source()->set_hand_index(land->hand_index());
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
            if (const auto *bolt = !boltCast
                                       ? handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Lightning Bolt"))
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
            if (const auto *charm = boltCast && !borosCharmCast
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
                cmd.mutable_play_land()->mutable_source()->set_hand_index(land->hand_index());
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
        const QByteArray ini = "[server]\n"
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
                return ::testing::AssertionFailure()
                       << "tricerules-server binary not found: " << sidecarExe.toStdString();
            }
            return ::testing::AssertionSuccess() << "SKIP:tricerules-server binary not found (build with "
                                                    "WITH_RULES_ENGINE or run cargo build --release): "
                                                 << sidecarExe.toStdString();
        }
        if (servatriceExe.isEmpty() || !QFile::exists(servatriceExe)) {
            if (require) {
                return ::testing::AssertionFailure() << "servatrice binary not found: " << servatriceExe.toStdString();
            }
            return ::testing::AssertionSuccess()
                   << "SKIP:servatrice binary not found (build with WITH_SERVER): " << servatriceExe.toStdString();
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
    const auto preOmenMilestonesDone = [&] {
        return p2.sentBottom && p1.sawBattlefieldOmission && p2.sawBattlefieldOmission && p1.sawBoltPushWithTarget &&
               p1.sawManifestChoicePrivate && p2.sawManifestChoiceRedacted && p1.submittedManifestChoice &&
               p1.sawManifestPublicFaceDown && p2.sawManifestPublicFaceDown && p1.sawManifestPrivateIdentity &&
               p2.sawOpponentManifestIdentityEmpty && p1.sawManifestPhysicalFaceDown &&
               p2.sawManifestPhysicalFaceDown && p1.sawManifestFaceChanged && p2.sawManifestFaceChanged &&
               p1.sawManifestPhysicalFaceUp && p2.sawManifestPhysicalFaceUp && p1.sawRoomCastDoorState &&
               p2.sawRoomCastDoorState && p1.sawRoomFullyUnlocked && p2.sawRoomFullyUnlocked &&
               p1.sawRoomUnlockTrigger && p2.sawRoomUnlockTrigger && p1.roomPhysicalIdentityContinuous &&
               p2.roomPhysicalIdentityContinuous && p1.hasRoomPhysicalAnnotation() && p2.hasRoomPhysicalAnnotation() &&
               p1.sawOwnTypecyclingAction && p2.sawOpponentTypecyclingActionRedacted && p1.submittedTypecyclingChoice &&
               p1.sawEmptyTypecyclingChoice && p1.submittedEmptyTypecyclingChoice && p1.sawOwnRenewAction &&
               p2.sawOpponentRenewActionRedacted && p1.sawRenewGraveToExile && p1.renewPhysicalIdentityContinuous &&
               p1.sawRenewCounters && p1.sawAggressiveChooserMask && p2.sawAggressiveObserverReadOnly &&
               p1.sawAggressivePublicRevealClosed && p2.sawAggressivePublicRevealClosed &&
               p1.submittedAggressiveChoice && p1.sawAggressiveExile && p2.sawAggressiveExile &&
               p1.sawAggressiveCounter && p2.sawAggressiveCounter && p1.sawAggressivePhysicalHandToExile &&
               p2.sawAggressivePhysicalHandToExile && p1.aggressivePhysicalIdentityContinuous &&
               p2.aggressivePhysicalIdentityContinuous && p1.sawCursePlayerAttachment && p2.sawCursePlayerAttachment &&
               p1.hasCursePhysicalAnnotation() && p2.hasCursePhysicalAnnotation() && p1.sawBoltLifeLoss &&
               p1.sawBorosCharmPushWithMode && p1.sawBorosCharmLifeLoss && p1.sawAttackersDeclared &&
               p1.sawCombatLifeLoss && p2.sawBrainstormChoice && p2.submittedBrainstormChoice &&
               p2.sawBrainstormResolved && p2.sentCleanupDiscard && p1.sawDevConjuredPermanent && p1.sawDevMana &&
               p1.sawWaifFaceChanged && p2.sawWaifFaceChanged && p1.sawWaifBackPt && p2.sawWaifBackPt &&
               p1.sawFlashbackGraveToStack && p1.sawFlashbackStackToExile && p1.sawAdventureStackToExile &&
               p1.sawAdventurePermissionGroup && p1.sawAdventureExileToStack &&
               p1.sawAdventureStackToBattlefield && p1.sawEntryReplacementChoice &&
               p1.submittedEntryReplacementChoice && p1.sawDiregrafEnterTapped && p1.sawDamagePreventionChoice &&
               p1.submittedDamagePreventionChoice && p1.sawControlTransfer && p1.sawProtectionBranchChoice &&
               p1.submittedProtectionBranchChoice && p1.sawProtectionHandToStack &&
               !p1.protectionLeftStackBeforeChoice && p1.sawProtectionStackToGraveAfterChoice &&
               p1.sawProtectionPhysicalAnnotation && p2.sawProtectionPhysicalAnnotation && p1.sawControlReturn &&
               p1.sawPhysicalControlTransfer && p1.sawPhysicalControlReturn && p1.sawLibraryPermanentMoved &&
               p2.sawLibraryPermanentMoved && p1.sawLibraryTargetAbsentFromBattlefield &&
               p2.sawLibraryTargetAbsentFromBattlefield && p2.sawTopPermanentDrawn &&
               p1.sawOwnLibrarySearchCandidates && p2.sawOpponentLibrarySearchRedacted &&
               p1.submittedEvolvingWildsChoice && p1.sawEvolvingWildsPermanentMoved &&
               p2.sawEvolvingWildsPermanentMoved && p1.sawEvolvingWildsPhysicalDeckToTable &&
               p2.sawEvolvingWildsPhysicalDeckToTable && p1.sawZoneScopeChoice &&
               p1.submittedZoneScopeChoice && p1.sawOwnZoneSearchCandidates &&
               p2.sawOpponentZoneSearchRedacted && p1.submittedZoneSearchChoice &&
               p1.sawAltanakEnterBattlefield && p2.sawAltanakEnterBattlefield &&
               p1.sayItsNameGraveToExileCount == 3 && p2.sayItsNameGraveToExileCount == 3 &&
               p1.sawOwnSurveilCandidates && p2.sawOpponentSurveilRedacted &&
               p1.submittedSurveilDestination && p1.sawSurveilPhysicalDeckToGrave && p1.sawCruelTruthsResolved &&
               p2.sawCruelTruthsResolved && p1.sawCruelTruthsLifeLoss && p2.sawCruelTruthsLifeLoss &&
               p1.sawSoftCounterPaymentChoice && p1.activatedManaDuringSoftCounterPayment && p1.paidSoftCounter &&
               p1.sawSoftCounterResolveAfterChoice && p2.softCounterConvoluteCast && p2.sawFlashbackGraveToStack &&
               p2.sawFlashbackStackToExile && p2.handSizeByPlayer.count(p2.myId) && p2.handSizeByPlayer[p2.myId] <= 7;
    };
    const auto omenMilestonesDone = [&] {
        return p1.sawOmenFaceActions && p1.omenSuccessCast && p1.omenFizzleCast && p1.omenFizzleBoltCast &&
               p1.sawOmenStackAnnotation && p2.sawOmenStackAnnotation && p1.sawOmenLibraryDestination &&
               p2.sawOmenLibraryDestination && p1.sawOmenStackToLibrary && p2.sawOmenStackToLibrary &&
               p1.sawOmenGraveyardDestination && p2.sawOmenGraveyardDestination &&
               p1.sawOmenStackToGraveyard && p2.sawOmenStackToGraveyard;
    };
    const auto milestonesDone = [&] { return preOmenMilestonesDone() && omenMilestonesDone(); };
    QElapsedTimer deadline;
    deadline.start();
    while (!milestonesDone() && deadline.elapsed() < kOverallDeadlineMs) {
        if (preOmenMilestonesDone()) {
            p1.omenSequenceEnabled = true;
        }
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
    EXPECT_TRUE(p1.submittedProtectionBranchChoice) << "the ruled client never selected protection from artifacts";
    EXPECT_TRUE(p1.sawProtectionHandToStack) << "Apostle's Blessing never moved from the physical hand to the stack";
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
    EXPECT_EQ(p1.serverCardByEngineOid[p1.protectionTargetOid], p2.serverCardByEngineOid[p2.protectionTargetOid])
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
    EXPECT_TRUE(p1.totallyLostCast) << "Uncharted Voyage was never cast";
    EXPECT_TRUE(p2.sawOwnerPlacementChoice && p2.submittedOwnerPlacementChoice)
        << "the target owner did not receive and submit the Top/Bottom choice";
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
    EXPECT_TRUE(p1.sayItsNameActivated) << "Say Its Name's graveyard ability was never activated";
    EXPECT_TRUE(p1.sawZoneScopeChoice && p1.submittedZoneScopeChoice)
        << "Say Its Name did not offer and accept the seven authored zone-scope choices";
    EXPECT_TRUE(p1.sawOwnZoneSearchCandidates)
        << "Say Its Name's controller did not receive aligned private multi-zone candidates";
    EXPECT_TRUE(p2.sawOpponentZoneSearchRedacted)
        << "Say Its Name leaked private multi-zone candidate metadata to the opponent";
    EXPECT_TRUE(p1.submittedZoneSearchChoice) << "Say Its Name's Altanak candidate was never selected";
    EXPECT_EQ(p1.sayItsNameGraveToExileCount, 3)
        << "Say Its Name did not exile its source plus exactly two chosen namesake cards";
    EXPECT_EQ(p2.sayItsNameGraveToExileCount, 3)
        << "the opponent did not observe exactly three public Say Its Name exile moves";
    EXPECT_TRUE(p1.sawAltanakEnterBattlefield && p2.sawAltanakEnterBattlefield)
        << "both clients did not observe the exact searched Altanak enter the battlefield";
    EXPECT_TRUE(p1.sawOwnSurveilCandidates)
        << "Cruel Truths' controller did not receive its two private surveil candidates";
    EXPECT_TRUE(p2.sawOpponentSurveilRedacted) << "Cruel Truths leaked private surveil identities to the opponent";
    EXPECT_TRUE(p1.submittedSurveilDestination) << "the surveil destination choice was never submitted";
    EXPECT_TRUE(p1.sawSurveilPhysicalDeckToGrave) << "the chosen physical surveil card did not move from DECK to GRAVE";
    EXPECT_TRUE(p1.sawCruelTruthsResolved && p2.sawCruelTruthsResolved)
        << "Cruel Truths did not finish resolving after the surveil choice";
    EXPECT_TRUE(p1.sawCruelTruthsLifeLoss && p2.sawCruelTruthsLifeLoss)
        << "both clients did not observe Cruel Truths' trailing life loss";
    ASSERT_NE(p1.evolvingWildsChosenOid, 0u);
    EXPECT_EQ(p1.evolvingWildsChosenOid, p2.evolvingWildsChosenOid)
        << "clients disagreed on the searched-for Mountain's engine ObjectId";
    ASSERT_TRUE(p1.serverCardByEngineOid.count(p1.evolvingWildsChosenOid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(p2.evolvingWildsChosenOid));
    EXPECT_EQ(p1.serverCardByEngineOid[p1.evolvingWildsChosenOid], p2.serverCardByEngineOid[p2.evolvingWildsChosenOid])
        << "clients disagreed on the searched-for Mountain's physical Server_Card mapping";
    EXPECT_EQ(p1.serverCardByEngineOid[p1.evolvingWildsChosenOid], p1.evolvingWildsPhysicalCardId)
        << "the selected engine ObjectId was not bound to the physical DECK-to-TABLE card";
    EXPECT_EQ(p2.serverCardByEngineOid[p2.evolvingWildsChosenOid], p2.evolvingWildsPhysicalCardId)
        << "the opponent did not retain the same physical DECK-to-TABLE binding";
    EXPECT_TRUE(p1.flashbackCast) << "seat 1 never sent its flashback cast";
    EXPECT_TRUE(p2.flashbackCast) << "seat 2 never sent its flashback cast";
    // One of these two seats does not own the canonical stack, so its cast crosses players.
    EXPECT_TRUE(p1.sawFlashbackGraveToStack) << "seat 1's flashback card never physically moved graveyard -> stack";
    EXPECT_TRUE(p2.sawFlashbackGraveToStack)
        << "seat 2's flashback card never physically moved graveyard -> stack (cross-player move "
           "rejected? see ruledAllowsCrossPlayerMove)";
    EXPECT_TRUE(p1.sawFlashbackStackToExile)
        << "seat 1's flashback card never physically moved stack -> exile (CR 702.34a)";
    EXPECT_TRUE(p2.sawFlashbackStackToExile)
        << "seat 2's flashback card never physically moved stack -> exile (CR 702.34a)";
    EXPECT_TRUE(p1.stompCast) << "Stomp was never cast from hand";
    EXPECT_TRUE(p1.giantCastFromExile) << "Bonecrusher Giant was never cast from its exile permission";
    EXPECT_TRUE(p1.sawAdventurePermissionGroup)
        << "the grantee never received the persistent Adventure permission-group snapshot";
    EXPECT_TRUE(p1.sawAdventureStackToExile) << "Stomp never physically moved stack -> exile";
    EXPECT_TRUE(p1.sawAdventureExileToStack) << "Bonecrusher Giant never physically moved exile -> stack";
    EXPECT_TRUE(p1.sawAdventureStackToBattlefield) << "Bonecrusher Giant never entered the battlefield";
    EXPECT_TRUE(p1.adventurePhysicalIdentityContinuous)
        << "Adventure casting moved a different physical card between zones";
    EXPECT_TRUE(p1.sawOmenFaceActions)
        << "the Omen physical hand card did not publish both engine-authored face names and costs";
    EXPECT_TRUE(p1.omenSuccessCast) << "zero-target Skimming Strike was never cast";
    EXPECT_TRUE(p1.sawOmenStackAnnotation && p2.sawOmenStackAnnotation)
        << "both clients did not receive the Skimming Strike alternate-face stack annotation";
    EXPECT_TRUE(p1.sawOmenLibraryDestination && p2.sawOmenLibraryDestination)
        << "both clients did not receive the successful Omen library destination";
    EXPECT_TRUE(p1.sawOmenStackToLibrary && p2.sawOmenStackToLibrary)
        << "the successful physical Omen did not move face down from stack to its owner's deck";
    EXPECT_TRUE(p1.omenSuccessPhysicalIdentityContinuous && p2.omenSuccessPhysicalIdentityContinuous)
        << "the successful Omen moved a different physical card into the library";
    EXPECT_TRUE(p1.omenFizzleCast && p1.omenFizzleBoltCast)
        << "the targeted Omen fizzle setup was not completed";
    EXPECT_TRUE(p1.sawOmenGraveyardDestination && p2.sawOmenGraveyardDestination)
        << "both clients did not receive the fizzled Omen graveyard destination";
    EXPECT_TRUE(p1.sawOmenStackToGraveyard && p2.sawOmenStackToGraveyard)
        << "the fizzled physical Omen did not move from stack to graveyard";
    EXPECT_TRUE(p1.omenFizzlePhysicalIdentityContinuous && p2.omenFizzlePhysicalIdentityContinuous)
        << "the fizzled Omen moved a different physical card into the graveyard";
    EXPECT_TRUE(p1.libraryDetailsStayedConcealed && p2.libraryDetailsStayedConcealed)
        << "a client received server-only library object identity or ordering";
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
    EXPECT_TRUE(p1.sawManifestPhysicalFaceDown && p2.sawManifestPhysicalFaceDown && p1.sawManifestPhysicalFaceUp &&
                p2.sawManifestPhysicalFaceUp)
        << "the same physical card was not shown face down and then face up on both clients";
    EXPECT_TRUE(p1.sawManifestPhysicalFaceUpIdentity && p2.sawManifestPhysicalFaceUpIdentity)
        << "the face-up physical event did not immediately publish Hill Giant's display identity";
    EXPECT_TRUE(p1.sawRoomCastDoorState && p2.sawRoomCastDoorState && p1.sawRoomFullyUnlocked &&
                p2.sawRoomFullyUnlocked)
        << "both clients did not receive identical cast-door and fully-unlocked Room state";
    EXPECT_TRUE(p1.sawRoomUnlockTrigger && p2.sawRoomUnlockTrigger)
        << "the unlock action produced no physical stack object, but its resulting door trigger was not published";
    EXPECT_EQ(p1.specialActionRestrictedPayments, 2);
    EXPECT_TRUE(p1.sawRestrictedBlueMana && p2.sawRestrictedBlueMana);
    EXPECT_EQ(p1.restrictedBlueByPlayer[p1.myId], 0);
    EXPECT_EQ(p2.restrictedBlueByPlayer[p1.myId], 0);
    EXPECT_TRUE(p1.roomPhysicalIdentityContinuous && p2.roomPhysicalIdentityContinuous && p1.roomServerCardId >= 0 &&
                p1.roomServerCardId == p2.roomServerCardId)
        << "Room casting and unlocking did not preserve one physical Server_Card identity";
    EXPECT_TRUE(p1.hasRoomPhysicalAnnotation() && p2.hasRoomPhysicalAnnotation())
        << "both clients did not receive the fully unlocked Doors annotation";
    EXPECT_TRUE(p1.sawOwnTypecyclingAction && p2.sawOpponentTypecyclingActionRedacted)
        << "the hand ability was not published exclusively to its owner";
    EXPECT_TRUE(p1.submittedTypecyclingChoice && p1.sawTypecyclingHandToGrave && p1.sawTypecyclingDeckToHand &&
                p1.typecyclingPhysicalIdentityContinuous)
        << "Plainscycling physical flags: choice=" << p1.submittedTypecyclingChoice
        << " hand_to_grave=" << p1.sawTypecyclingHandToGrave << " deck_to_hand=" << p1.sawTypecyclingDeckToHand
        << " identity=" << p1.typecyclingPhysicalIdentityContinuous << " source_id=" << p1.typecyclingSourcePhysicalId
        << " chosen_id=" << p1.typecyclingChosenPhysicalId;
    EXPECT_TRUE(p1.sawEmptyTypecyclingChoice && p1.submittedEmptyTypecyclingChoice)
        << "the second Plainscycle did not publish and submit the explicit empty fail-to-find choice";
    EXPECT_TRUE(p1.sawOwnRenewAction && p2.sawOpponentRenewActionRedacted)
        << "the graveyard ability was not published exclusively to its owner";
    EXPECT_TRUE(p1.sawRenewGraveToExile && p1.renewPhysicalIdentityContinuous && p1.sawRenewCounters)
        << "Renew did not exile the same physical source and add two +1/+1 counters plus reach";
    EXPECT_TRUE(p1.sawAggressivePublicReveal && p2.sawAggressivePublicReveal && p1.sawAggressiveChooserMask &&
                p2.sawAggressiveObserverReadOnly)
        << "Aggressive Negotiations did not publish the full hand to both seats with a chooser-only mask";
    EXPECT_EQ(p1.aggressiveRevealNames, p2.aggressiveRevealNames)
        << "Aggressive Negotiations recipients did not receive the same reveal cohort";
    EXPECT_TRUE(p1.sawAggressivePublicRevealClosed && p2.sawAggressivePublicRevealClosed &&
                !p1.aggressivePublicRevealActive && !p2.aggressivePublicRevealActive)
        << "Aggressive Negotiations public reveal did not close for both seats after submission";
    EXPECT_TRUE(p1.submittedAggressiveChoice && p1.sawAggressiveExile && p2.sawAggressiveExile)
        << "Aggressive Negotiations did not submit and publish the selected hand-card exile";
    EXPECT_TRUE(p1.sawAggressiveCounter && p2.sawAggressiveCounter)
        << "the +1/+1 counter was not published after the parked hand choice resumed";
    EXPECT_TRUE(p1.sawAggressivePhysicalHandToExile && p2.sawAggressivePhysicalHandToExile &&
                p1.aggressivePhysicalIdentityContinuous && p2.aggressivePhysicalIdentityContinuous)
        << "Aggressive Negotiations did not preserve the selected physical hand card in exile";
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

TEST_F(RuledE2ESmokeTest, WardManaDeclineAndPrivateDiscardPayment)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    {
        const std::string msg = started.message();
        if (msg.rfind("SKIP:", 0) == 0) {
            GTEST_SKIP() << msg.substr(5);
        }
    }

    SmokeClient p1(SmokeClient::Role::Aggressor, QStringLiteral("wardp1"), &transcript);
    SmokeClient p2(SmokeClient::Role::Hoarder, QStringLiteral("wardp2"), &transcript);
    // This focused cohort needs only a kept opening hand before dev setup.
    p2.didMulligan = true;

    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Mountain")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000,
                             "Ward cohort game start (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000,
                             "Ward cohort game start (p2)"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());

    QElapsedTimer openingDeadline;
    openingDeadline.start();
    while (openingDeadline.elapsed() < 30000) {
        p1.pump(25);
        p2.pump(25);
        if (p1.phase == ruled::v1::PHASE_ID_MAIN1 && p2.phase == ruled::v1::PHASE_ID_MAIN1 &&
            p1.priorityPlayer == p1.myId && p2.priorityPlayer == p1.myId) {
            break;
        }
        p1.act();
        p2.act();
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);
    ASSERT_EQ(p1.priorityPlayer, p1.myId);

    auto sendAndPump = [&](SmokeClient &sender, const ruled::v1::RuledCommand &command,
                           const QString &description) {
        const quint64 p1Version = p1.stateVersion;
        const quint64 p2Version = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while (wait.elapsed() < 10000 &&
               (p1.stateVersion <= p1Version || p2.stateVersion <= p2Version)) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > p1Version && p2.stateVersion > p2Version;
    };
    auto devPut = [&](int targetPlayer, const char *cardName, ruled::v1::DevZone zone, bool ready) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(targetPlayer);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(cardName);
        put->set_zone(zone);
        put->set_ready(ready);
        return sendAndPump(p1, command, QStringLiteral("dev: put %1 for Ward cohort").arg(cardName));
    };
    auto devBlueMana = [&](int targetPlayer) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(targetPlayer);
        dev->mutable_add_mana()->set_u(1);
        return sendAndPump(p1, command, QStringLiteral("dev: add blue mana for Ward cohort"));
    };
    auto passPriority = [&](SmokeClient &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return sendAndPump(client, command, QStringLiteral("pass priority in Ward cohort"));
    };
    auto legalPermanentTargets = [](const SmokeClient &client, const ruled::v1::LegalHandAction &action) {
        std::vector<quint32> targets;
        const quint32 key = action.hand_index() << 8;
        const auto found = client.latestLegal.valid_targets_by_hand_slot().find(key);
        if (found == client.latestLegal.valid_targets_by_hand_slot().end()) {
            return targets;
        }
        for (const auto &group : found->second.groups()) {
            targets.insert(targets.end(), group.valid_permanent_ids().begin(), group.valid_permanent_ids().end());
        }
        return targets;
    };

    // Ward {2}: the targeting player sees the choice, declines, and both clients receive the
    // exact public counter event for the physical Unsummon.
    ASSERT_TRUE(devPut(p1.myId, "Dirgur Island Dragon // Skimming Strike",
                       ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    ASSERT_TRUE(devPut(p2.myId, "Unsummon", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(devBlueMana(p2.myId));
    p1.wardManaFlowActive = true;
    p2.wardManaFlowActive = true;
    ASSERT_TRUE(passPriority(p1));
    const auto *manaUnsummon = p2.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Unsummon"));
    ASSERT_NE(manaUnsummon, nullptr);
    const auto manaTargets = legalPermanentTargets(p2, *manaUnsummon);
    ASSERT_EQ(manaTargets.size(), 1u);
    const quint32 dirgurOid = manaTargets.front();
    ruled::v1::RuledCommand castManaWard;
    castManaWard.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    castManaWard.mutable_cast_spell()->mutable_source()->set_hand_index(manaUnsummon->hand_index());
    auto *manaTarget = castManaWard.mutable_cast_spell()->add_targets();
    manaTarget->set_object_id(dirgurOid);
    manaTarget->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    ASSERT_TRUE(sendAndPump(p2, castManaWard, QStringLiteral("cast Unsummon at Dirgur")));
    ASSERT_TRUE(passPriority(p2));
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(p2.pendingChoice.has_value());
    EXPECT_EQ(p2.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_MANA_PAYMENT);
    EXPECT_FALSE(p1.pendingChoice.has_value());
    ruled::v1::RuledCommand declineManaWard;
    declineManaWard.mutable_submit_resolution_choice()->set_decision(
        ruled::v1::RESOLUTION_CHOICE_DECISION_DECLINE);
    p2.pendingChoice.reset();
    ASSERT_TRUE(sendAndPump(p2, declineManaWard, QStringLiteral("decline Ward {2}")));
    EXPECT_TRUE(p1.sawWardManaAnnotation && p2.sawWardManaAnnotation);
    EXPECT_TRUE(p1.sawWardManaCountered && p2.sawWardManaCountered);
    EXPECT_NE(p1.wardManaSpellOid, 0u);
    EXPECT_EQ(p1.wardManaSpellOid, p2.wardManaSpellOid);
    p1.wardManaFlowActive = false;
    p2.wardManaFlowActive = false;

    // Ward—Discard a card: only the payer receives aligned hand identities. Paying discards the
    // chosen physical Bear, then the preserved Unsummon resolves and returns Spectral Snatcher.
    ASSERT_EQ(p1.priorityPlayer, p1.myId);
    ASSERT_TRUE(devPut(p1.myId, "Spectral Snatcher", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    ASSERT_TRUE(devPut(p2.myId, "Grizzly Bears", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(devPut(p2.myId, "Unsummon", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(devBlueMana(p2.myId));
    p1.wardDiscardFlowActive = true;
    p2.wardDiscardFlowActive = true;
    ASSERT_TRUE(passPriority(p1));
    const auto *discardUnsummon = p2.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Unsummon"));
    ASSERT_NE(discardUnsummon, nullptr);
    const auto discardTargets = legalPermanentTargets(p2, *discardUnsummon);
    const auto snatcher = std::find_if(discardTargets.begin(), discardTargets.end(),
                                      [dirgurOid](quint32 objectId) { return objectId != dirgurOid; });
    ASSERT_NE(snatcher, discardTargets.end());
    const quint32 snatcherOid = *snatcher;
    p1.wardDiscardSourceOid = snatcherOid;
    p2.wardDiscardSourceOid = snatcherOid;
    ruled::v1::RuledCommand castDiscardWard;
    castDiscardWard.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    castDiscardWard.mutable_cast_spell()->mutable_source()->set_hand_index(discardUnsummon->hand_index());
    auto *discardTarget = castDiscardWard.mutable_cast_spell()->add_targets();
    discardTarget->set_object_id(snatcherOid);
    discardTarget->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    ASSERT_TRUE(sendAndPump(p2, castDiscardWard, QStringLiteral("cast Unsummon at Spectral Snatcher")));
    ASSERT_TRUE(passPriority(p2));
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(p2.pendingChoice.has_value());
    EXPECT_EQ(p2.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_HAND_CARDS);
    EXPECT_TRUE(p2.sawWardDiscardPrivateCandidates);
    EXPECT_TRUE(p1.sawWardDiscardObserverRedaction);
    ASSERT_NE(p2.wardDiscardChosenOid, 0u);
    ruled::v1::RuledCommand payDiscardWard;
    payDiscardWard.mutable_submit_resolution_choice()->add_chosen_object_ids(p2.wardDiscardChosenOid);
    p2.pendingChoice.reset();
    ASSERT_TRUE(sendAndPump(p2, payDiscardWard, QStringLiteral("discard Grizzly Bears to pay Ward")));
    ASSERT_EQ(p1.priorityPlayer, p1.myId);
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));

    EXPECT_TRUE(p1.sawWardDiscardAnnotation && p2.sawWardDiscardAnnotation);
    EXPECT_TRUE(p1.sawWardDiscardPhysicalHandToGrave && p2.sawWardDiscardPhysicalHandToGrave);
    EXPECT_EQ(p2.wardDiscardMovedServerCardId, p2.wardDiscardChosenServerCardId);
    EXPECT_TRUE(p1.sawWardDiscardSpellResolved && p2.sawWardDiscardSpellResolved);
    EXPECT_TRUE(p1.sawWardDiscardSourceToHand && p2.sawWardDiscardSourceToHand);
    EXPECT_EQ(p1.wardDiscardSpellOid, p2.wardDiscardSpellOid);
}

TEST_F(RuledE2ESmokeTest, TemporaryExileReturnsTheExactPhysicalCardToBothClients)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    {
        const std::string msg = started.message();
        if (msg.rfind("SKIP:", 0) == 0) {
            GTEST_SKIP() << msg.substr(5);
        }
    }

    SmokeClient p1(SmokeClient::Role::Aggressor, QStringLiteral("exilep1"), &transcript);
    SmokeClient p2(SmokeClient::Role::Hoarder, QStringLiteral("exilep2"), &transcript);
    p2.didMulligan = true;
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Plains")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000,
                             "temporary-exile game start (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000,
                             "temporary-exile game start (p2)"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());

    QElapsedTimer openingDeadline;
    openingDeadline.start();
    while (openingDeadline.elapsed() < 30000) {
        p1.pump(25);
        p2.pump(25);
        if (p1.phase == ruled::v1::PHASE_ID_MAIN1 && p2.phase == ruled::v1::PHASE_ID_MAIN1 &&
            p1.priorityPlayer == p1.myId) {
            break;
        }
        p1.act();
        p2.act();
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);

    auto sendAndPump = [&](SmokeClient &sender, const ruled::v1::RuledCommand &command,
                           const QString &description) {
        const quint64 v1 = p1.stateVersion;
        const quint64 v2 = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while (wait.elapsed() < 10000 && (p1.stateVersion <= v1 || p2.stateVersion <= v2)) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > v1 && p2.stateVersion > v2;
    };
    auto devPut = [&](int playerId, const char *name) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(playerId);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(name);
        put->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
        put->set_ready(true);
        return sendAndPump(p1, command, QStringLiteral("dev put %1").arg(name));
    };
    auto passPriority = [&](SmokeClient &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return sendAndPump(client, command, QStringLiteral("temporary-exile pass"));
    };
    auto findPermanent = [](const SmokeClient &client, const QString &cardId) -> quint32 {
        for (const auto &[playerId, permanents] : client.battlefieldByPlayer) {
            Q_UNUSED(playerId);
            for (const auto &permanent : permanents) {
                if (permanent.cardId == cardId) {
                    return permanent.oid;
                }
            }
        }
        return 0;
    };

    ASSERT_TRUE(devPut(p2.myId, "Grizzly Bears"));
    const quint32 bearOid = findPermanent(p1, QStringLiteral("grizzly_bears"));
    ASSERT_NE(bearOid, 0u);
    ASSERT_TRUE(p1.serverCardByEngineOid.count(bearOid));
    const int physicalBearId = p1.serverCardByEngineOid[bearOid];
    ASSERT_TRUE(p2.serverCardByEngineOid.count(bearOid));
    ASSERT_EQ(p2.serverCardByEngineOid[bearOid], physicalBearId);
    p1.temporaryExileFlowActive = true;
    p2.temporaryExileFlowActive = true;
    p1.temporaryExilePhysicalCardId = physicalBearId;
    p2.temporaryExilePhysicalCardId = physicalBearId;
    ASSERT_TRUE(devPut(p1.myId, "Banishing Light"));
    const quint32 lightOid = findPermanent(p1, QStringLiteral("banishing_light"));
    ASSERT_NE(lightOid, 0u);
    ASSERT_TRUE(p1.pendingTriggerTarget.has_value());
    ruled::v1::RuledCommand chooseTarget;
    auto *choice = chooseTarget.mutable_choose_trigger_target();
    auto *target = choice->add_targets();
    target->set_object_id(bearOid);
    target->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    p1.pendingTriggerTarget.reset();
    ASSERT_TRUE(sendAndPump(p1, chooseTarget, QStringLiteral("choose Banishing Light target")));
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));
    EXPECT_EQ(findPermanent(p1, QStringLiteral("grizzly_bears")), 0u);
    EXPECT_EQ(findPermanent(p2, QStringLiteral("grizzly_bears")), 0u);
    ASSERT_TRUE(p1.serverCardByEngineOid.count(bearOid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(bearOid));
    EXPECT_EQ(p1.serverCardByEngineOid[bearOid], physicalBearId);
    EXPECT_EQ(p2.serverCardByEngineOid[bearOid], physicalBearId);

    ruled::v1::RuledCommand removeLight;
    auto *dev = removeLight.mutable_dev_command();
    dev->set_target_player_id(p1.myId);
    auto *move = dev->mutable_move_card();
    move->set_card_name("Banishing Light");
    move->set_zone(ruled::v1::DEV_ZONE_GRAVEYARD);
    ASSERT_TRUE(sendAndPump(p1, removeLight, QStringLiteral("remove Banishing Light")));

    EXPECT_EQ(findPermanent(p1, QStringLiteral("grizzly_bears")), bearOid);
    EXPECT_EQ(findPermanent(p2, QStringLiteral("grizzly_bears")), bearOid);
    ASSERT_TRUE(p1.serverCardByEngineOid.count(bearOid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(bearOid));
    EXPECT_EQ(p1.serverCardByEngineOid[bearOid], physicalBearId);
    EXPECT_EQ(p2.serverCardByEngineOid[bearOid], physicalBearId);
    EXPECT_TRUE(p1.sawTemporaryExilePhysicalMove);
    EXPECT_TRUE(p2.sawTemporaryExilePhysicalMove);
    EXPECT_TRUE(p1.sawTemporaryReturnPhysicalMove);
    EXPECT_TRUE(p2.sawTemporaryReturnPhysicalMove);
}

TEST_F(RuledE2ESmokeTest, PlayerSetDiscardCollectsPrivateChoicesBeforeOnePhysicalCommit)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    {
        const std::string msg = started.message();
        if (msg.rfind("SKIP:", 0) == 0) {
            GTEST_SKIP() << msg.substr(5);
        }
    }

    SmokeClient p1(SmokeClient::Role::Aggressor, QStringLiteral("playersetp1"), &transcript);
    SmokeClient p2(SmokeClient::Role::Hoarder, QStringLiteral("playersetp2"), &transcript);
    p2.didMulligan = true;

    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Swamp")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000,
                             "player-set discard game start (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000,
                             "player-set discard game start (p2)"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());

    QElapsedTimer openingDeadline;
    openingDeadline.start();
    while (openingDeadline.elapsed() < 30000) {
        p1.pump(25);
        p2.pump(25);
        if (p1.phase == ruled::v1::PHASE_ID_MAIN1 && p2.phase == ruled::v1::PHASE_ID_MAIN1 &&
            p1.priorityPlayer == p1.myId && p2.priorityPlayer == p1.myId) {
            break;
        }
        p1.act();
        p2.act();
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);
    ASSERT_EQ(p1.priorityPlayer, p1.myId);

    auto sendAndPump = [&](SmokeClient &sender, const ruled::v1::RuledCommand &command,
                           const QString &description) {
        const quint64 p1Version = p1.stateVersion;
        const quint64 p2Version = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while (wait.elapsed() < 10000 &&
               (p1.stateVersion <= p1Version || p2.stateVersion <= p2Version)) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > p1Version && p2.stateVersion > p2Version;
    };
    auto devPut = [&](int targetPlayer, const char *cardName, ruled::v1::DevZone zone) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(targetPlayer);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(cardName);
        put->set_zone(zone);
        put->set_ready(false);
        return sendAndPump(p1, command, QStringLiteral("dev: put %1 for player-set discard").arg(cardName));
    };
    auto passPriority = [&](SmokeClient &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return sendAndPump(client, command, QStringLiteral("pass priority in player-set discard"));
    };

    ASSERT_TRUE(devPut(p1.myId, "Grizzly Bears", ruled::v1::DEV_ZONE_HAND));
    ASSERT_TRUE(devPut(p2.myId, "Grizzly Bears", ruled::v1::DEV_ZONE_HAND));
    ASSERT_TRUE(devPut(p1.myId, "Fanatic of the Harrowing", ruled::v1::DEV_ZONE_HAND));
    ruled::v1::RuledCommand mana;
    mana.mutable_dev_command()->set_target_player_id(p1.myId);
    mana.mutable_dev_command()->mutable_add_mana()->set_b(1);
    mana.mutable_dev_command()->mutable_add_mana()->set_c(3);
    ASSERT_TRUE(sendAndPump(p1, mana, QStringLiteral("dev: add Fanatic mana")));

    const auto *fanatic = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL,
                                       QStringLiteral("Fanatic of the Harrowing"));
    ASSERT_NE(fanatic, nullptr);
    ruled::v1::RuledCommand cast;
    cast.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    cast.mutable_cast_spell()->mutable_source()->set_hand_index(fanatic->hand_index());
    ASSERT_TRUE(sendAndPump(p1, cast, QStringLiteral("cast Fanatic of the Harrowing")));
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));

    p1.playerSetDiscardFlowActive = true;
    p2.playerSetDiscardFlowActive = true;
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));
    ASSERT_TRUE(p1.pendingChoice.has_value());
    EXPECT_FALSE(p2.pendingChoice.has_value());
    EXPECT_TRUE(p1.sawPlayerSetDiscardPrivateCandidates);
    EXPECT_TRUE(p2.sawPlayerSetDiscardObserverRedaction);
    ASSERT_NE(p1.playerSetDiscardChosenOid, 0u);
    const int p1HandBefore = p1.handSizeByPlayer[p1.myId];
    const int p2HandBefore = p2.handSizeByPlayer[p2.myId];

    ruled::v1::RuledCommand firstChoice;
    firstChoice.mutable_submit_resolution_choice()->add_chosen_object_ids(p1.playerSetDiscardChosenOid);
    p1.pendingChoice.reset();
    ASSERT_TRUE(sendAndPump(p1, firstChoice, QStringLiteral("stage first APNAP discard")));
    EXPECT_EQ(p1.handSizeByPlayer[p1.myId], p1HandBefore);
    EXPECT_EQ(p2.handSizeByPlayer[p2.myId], p2HandBefore);
    EXPECT_EQ(p1.graveyardOwnerByEngineOid.count(p1.playerSetDiscardChosenOid), 0u);
    ASSERT_TRUE(p2.pendingChoice.has_value());
    EXPECT_FALSE(p1.pendingChoice.has_value());
    EXPECT_TRUE(p2.sawPlayerSetDiscardPrivateCandidates);
    EXPECT_TRUE(p1.sawPlayerSetDiscardObserverRedaction);
    ASSERT_NE(p2.playerSetDiscardChosenOid, 0u);

    ruled::v1::RuledCommand secondChoice;
    secondChoice.mutable_submit_resolution_choice()->add_chosen_object_ids(p2.playerSetDiscardChosenOid);
    p2.pendingChoice.reset();
    ASSERT_TRUE(sendAndPump(p2, secondChoice, QStringLiteral("commit complete APNAP discard")));
    ASSERT_EQ(p1.graveyardOwnerByEngineOid[p1.playerSetDiscardChosenOid], p1.myId);
    ASSERT_EQ(p1.graveyardOwnerByEngineOid[p2.playerSetDiscardChosenOid], p2.myId);
    ASSERT_TRUE(p1.serverCardByEngineOid.count(p1.playerSetDiscardChosenOid));
    ASSERT_TRUE(p1.serverCardByEngineOid.count(p2.playerSetDiscardChosenOid));
    EXPECT_EQ(p1.serverCardByEngineOid[p1.playerSetDiscardChosenOid],
              p1.playerSetDiscardChosenServerCardId);
    EXPECT_EQ(p1.serverCardByEngineOid[p2.playerSetDiscardChosenOid],
              p2.playerSetDiscardChosenServerCardId);
    EXPECT_EQ(p1.handSizeByPlayer[p1.myId], p1HandBefore);
    EXPECT_EQ(p2.handSizeByPlayer[p2.myId], p2HandBefore - 1);
}

TEST_F(RuledE2ESmokeTest, BeholdCastCostIsPrivateUntilItsPublicStackReveal)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    {
        const std::string msg = started.message();
        if (msg.rfind("SKIP:", 0) == 0) {
            GTEST_SKIP() << msg.substr(5);
        }
    }

    SmokeClient p1(SmokeClient::Role::Aggressor, QStringLiteral("beholdp1"), &transcript);
    SmokeClient p2(SmokeClient::Role::Hoarder, QStringLiteral("beholdp2"), &transcript);
    p2.didMulligan = true;

    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Swamp")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000,
                             "Behold cohort game start (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000,
                             "Behold cohort game start (p2)"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());

    QElapsedTimer openingDeadline;
    openingDeadline.start();
    while (openingDeadline.elapsed() < 30000) {
        p1.pump(25);
        p2.pump(25);
        if (p1.phase == ruled::v1::PHASE_ID_MAIN1 && p2.phase == ruled::v1::PHASE_ID_MAIN1 &&
            p1.priorityPlayer == p1.myId && p2.priorityPlayer == p1.myId) {
            break;
        }
        p1.act();
        p2.act();
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);
    ASSERT_EQ(p1.priorityPlayer, p1.myId);

    auto sendAndPump = [&](SmokeClient &sender, const ruled::v1::RuledCommand &command,
                           const QString &description) {
        const quint64 p1Version = p1.stateVersion;
        const quint64 p2Version = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while (wait.elapsed() < 10000 &&
               (p1.stateVersion <= p1Version || p2.stateVersion <= p2Version)) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > p1Version && p2.stateVersion > p2Version;
    };
    auto devPut = [&](int targetPlayer, const char *cardName, ruled::v1::DevZone zone, bool ready) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(targetPlayer);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(cardName);
        put->set_zone(zone);
        put->set_ready(ready);
        return sendAndPump(p1, command, QStringLiteral("dev: put %1 for Behold cohort").arg(cardName));
    };
    auto passPriority = [&](SmokeClient &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return sendAndPump(client, command, QStringLiteral("pass priority in Behold cohort"));
    };

    ASSERT_TRUE(devPut(p1.myId, "Caustic Exhale", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(devPut(p1.myId, "Adult Gold Dragon", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(devPut(p2.myId, "Grizzly Bears", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    p1.optionalCastCostFlowActive = true;
    p2.optionalCastCostFlowActive = true;
    ruled::v1::RuledCommand addMana;
    addMana.mutable_dev_command()->set_target_player_id(p1.myId);
    addMana.mutable_dev_command()->mutable_add_mana()->set_b(1);
    ASSERT_TRUE(sendAndPump(p1, addMana, QStringLiteral("dev: add black mana for Behold cohort")));
    ASSERT_EQ(p1.priorityPlayer, p1.myId);

    const auto *caustic = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Caustic Exhale"));
    ASSERT_NE(caustic, nullptr);
    ASSERT_EQ(caustic->cost_choices().cast_cost_groups_size(), 1);
    const auto &group = caustic->cost_choices().cast_cost_groups(0);
    ASSERT_GE(group.options_size(), 1);
    const auto &behold = group.options(0);
    ASSERT_EQ(behold.kind(), ruled::v1::CAST_COST_OPTION_KIND_BEHOLD);
    ASSERT_EQ(behold.valid_hand_indices_size(), 1);
    ASSERT_TRUE(p1.sawPrivateBeholdCandidates);
    ASSERT_TRUE(p2.sawBeholdCandidateRedaction);

    const quint32 targetKey = caustic->hand_index() << 8;
    const auto targetGroups = p1.latestLegal.valid_targets_by_hand_slot().find(targetKey);
    ASSERT_NE(targetGroups, p1.latestLegal.valid_targets_by_hand_slot().end());
    ASSERT_EQ(targetGroups->second.groups_size(), 1);
    ASSERT_EQ(targetGroups->second.groups(0).valid_permanent_ids_size(), 1);

    ruled::v1::RuledCommand cast;
    cast.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    cast.mutable_cast_spell()->mutable_source()->set_hand_index(caustic->hand_index());
    auto *selection = cast.mutable_cast_spell()->add_cast_cost_group_selections();
    selection->set_group_index(group.group_index());
    selection->set_option_index(behold.option_index());
    selection->set_hand_index(behold.valid_hand_indices(0));
    auto *target = cast.mutable_cast_spell()->add_targets();
    target->set_object_id(targetGroups->second.groups(0).valid_permanent_ids(0));
    target->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    ASSERT_TRUE(sendAndPump(p1, cast, QStringLiteral("cast Caustic Exhale by beholding a Dragon")));

    EXPECT_TRUE(p1.sawBeholdStackReceipt && p2.sawBeholdStackReceipt);
    EXPECT_TRUE(p1.sawActiveBeholdReveal && p2.sawActiveBeholdReveal);
    EXPECT_TRUE(p1.activeBeholdReveal && p2.activeBeholdReveal);

    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));
    EXPECT_TRUE(p1.sawActiveBeholdRevealClosed && p2.sawActiveBeholdRevealClosed);
    EXPECT_FALSE(p1.activeBeholdReveal || p2.activeBeholdReveal);
}

TEST_F(RuledE2ESmokeTest, TappedTargetReductionIsPrivateAndAuthoritativeThroughBothClients)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    {
        const std::string msg = started.message();
        if (msg.rfind("SKIP:", 0) == 0) {
            GTEST_SKIP() << msg.substr(5);
        }
    }

    SmokeClient p1(SmokeClient::Role::Aggressor, QStringLiteral("reductionp1"), &transcript);
    SmokeClient p2(SmokeClient::Role::Hoarder, QStringLiteral("reductionp2"), &transcript);
    p2.didMulligan = true;

    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Plains")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Swamp")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000,
                             "target reduction game start (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000,
                             "target reduction game start (p2)"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());

    QElapsedTimer openingDeadline;
    openingDeadline.start();
    while (openingDeadline.elapsed() < 30000) {
        p1.pump(25);
        p2.pump(25);
        if (p1.phase == ruled::v1::PHASE_ID_MAIN1 && p2.phase == ruled::v1::PHASE_ID_MAIN1 &&
            p1.priorityPlayer == p1.myId && p2.priorityPlayer == p1.myId) {
            break;
        }
        p1.act();
        p2.act();
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);
    ASSERT_EQ(p1.priorityPlayer, p1.myId);

    auto sendAndPump = [&](SmokeClient &sender, const ruled::v1::RuledCommand &command,
                           const QString &description) {
        const quint64 p1Version = p1.stateVersion;
        const quint64 p2Version = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while (wait.elapsed() < 10000 &&
               (p1.stateVersion <= p1Version || p2.stateVersion <= p2Version)) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > p1Version && p2.stateVersion > p2Version;
    };
    auto devPut = [&](int targetPlayer, const char *cardName, ruled::v1::DevZone zone, bool ready) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(targetPlayer);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(cardName);
        put->set_zone(zone);
        put->set_ready(ready);
        return sendAndPump(p1, command, QStringLiteral("dev: put %1 for target reduction").arg(cardName));
    };
    auto passPriority = [&](SmokeClient &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return sendAndPump(client, command, QStringLiteral("pass priority in target reduction cohort"));
    };

    ASSERT_TRUE(devPut(p1.myId, "Luminous Rebuke", ruled::v1::DEV_ZONE_HAND, false));
    // Diregraf Ghoul's intrinsic replacement makes it a naturally tapped creature target.
    ASSERT_TRUE(devPut(p2.myId, "Diregraf Ghoul", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    ruled::v1::RuledCommand addMana;
    addMana.mutable_dev_command()->set_target_player_id(p1.myId);
    addMana.mutable_dev_command()->mutable_add_mana()->set_w(1);
    addMana.mutable_dev_command()->mutable_add_mana()->set_c(1);
    ASSERT_TRUE(sendAndPump(p1, addMana, QStringLiteral("dev: add {1}{W} for target reduction")));

    const auto *rebuke = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Luminous Rebuke"));
    ASSERT_NE(rebuke, nullptr);
    EXPECT_EQ(rebuke->generic_cost_reduction(), 0u);
    EXPECT_EQ(p2.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Luminous Rebuke")), nullptr);
    const quint32 targetKey = rebuke->hand_index() << 8;
    const auto targets = p1.latestLegal.valid_targets_by_hand_slot().find(targetKey);
    ASSERT_NE(targets, p1.latestLegal.valid_targets_by_hand_slot().end());
    ASSERT_EQ(targets->second.groups_size(), 1);
    ASSERT_EQ(targets->second.groups(0).valid_permanent_ids_size(), 1);
    ASSERT_EQ(targets->second.targeted_cost_reduction_applications_size(), 1);
    const quint32 ghoulOid = targets->second.groups(0).valid_permanent_ids(0);
    const auto &reduction = targets->second.targeted_cost_reduction_applications(0);
    EXPECT_EQ(reduction.generic_mana(), 3u);
    ASSERT_EQ(reduction.qualifying_targets_size(), 1);
    EXPECT_EQ(reduction.qualifying_targets(0).kind(), ruled::v1::TARGET_REF_KIND_PERMANENT);
    EXPECT_EQ(reduction.qualifying_targets(0).object_id(), ghoulOid);

    ruled::v1::RuledCommand cast;
    cast.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    cast.mutable_cast_spell()->mutable_source()->set_hand_index(rebuke->hand_index());
    auto *target = cast.mutable_cast_spell()->add_targets();
    target->set_object_id(ghoulOid);
    target->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
    ASSERT_TRUE(sendAndPump(p1, cast, QStringLiteral("cast Luminous Rebuke for {1}{W}")));
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));
}

TEST_F(RuledE2ESmokeTest, HarmonizeUsesOwnerOnlyReductionAndPreservesPhysicalIdentity)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    {
        const std::string msg = started.message();
        if (msg.rfind("SKIP:", 0) == 0) {
            GTEST_SKIP() << msg.substr(5);
        }
    }

    SmokeClient p1(SmokeClient::Role::Aggressor, QStringLiteral("harmonizep1"), &transcript);
    SmokeClient p2(SmokeClient::Role::Hoarder, QStringLiteral("harmonizep2"), &transcript);
    p2.didMulligan = true;

    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000,
                             "Harmonize cohort game start (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000,
                             "Harmonize cohort game start (p2)"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());

    QElapsedTimer openingDeadline;
    openingDeadline.start();
    while (openingDeadline.elapsed() < 30000) {
        p1.pump(25);
        p2.pump(25);
        if (p1.phase == ruled::v1::PHASE_ID_MAIN1 && p2.phase == ruled::v1::PHASE_ID_MAIN1 &&
            p1.priorityPlayer == p1.myId && p2.priorityPlayer == p1.myId) {
            break;
        }
        p1.act();
        p2.act();
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);
    ASSERT_EQ(p1.priorityPlayer, p1.myId);

    auto sendAndPump = [&](SmokeClient &sender, const ruled::v1::RuledCommand &command,
                           const QString &description) {
        const quint64 p1Version = p1.stateVersion;
        const quint64 p2Version = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while (wait.elapsed() < 10000 &&
               (p1.stateVersion <= p1Version || p2.stateVersion <= p2Version)) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > p1Version && p2.stateVersion > p2Version;
    };
    auto devPut = [&](const char *cardName, ruled::v1::DevZone zone, bool ready) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(p1.myId);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(cardName);
        put->set_zone(zone);
        put->set_ready(ready);
        return sendAndPump(p1, command, QStringLiteral("dev: put %1 for Harmonize cohort").arg(cardName));
    };
    auto devMove = [&](const char *cardName, ruled::v1::DevZone zone) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(p1.myId);
        auto *move = dev->mutable_move_card();
        move->set_card_name(cardName);
        move->set_zone(zone);
        return sendAndPump(p1, command, QStringLiteral("dev: move %1 for Harmonize cohort").arg(cardName));
    };
    auto passPriority = [&](SmokeClient &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return sendAndPump(client, command, QStringLiteral("pass priority in Harmonize cohort"));
    };
    auto permanentTapped = [](const SmokeClient &client, int playerId, quint32 oid) {
        const auto battlefield = client.battlefieldByPlayer.find(playerId);
        if (battlefield == client.battlefieldByPlayer.end()) {
            return false;
        }
        const auto permanent = std::find_if(battlefield->second.begin(), battlefield->second.end(),
                                            [oid](const SmokeClient::Permanent &candidate) {
                                                return candidate.oid == oid;
                                            });
        return permanent != battlefield->second.end() && permanent->tapped;
    };

    ASSERT_TRUE(devPut("Unending Whisper", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(devMove("Unending Whisper", ruled::v1::DEV_ZONE_GRAVEYARD));
    ASSERT_TRUE(devPut("Grizzly Bears", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    ruled::v1::RuledCommand addHarmonizeMana;
    addHarmonizeMana.mutable_dev_command()->set_target_player_id(p1.myId);
    addHarmonizeMana.mutable_dev_command()->mutable_add_mana()->set_u(1);
    addHarmonizeMana.mutable_dev_command()->mutable_add_mana()->set_c(3);
    ASSERT_TRUE(sendAndPump(p1, addHarmonizeMana, QStringLiteral("dev: add {3}{U} for Harmonize")));

    const ruled::v1::LegalZoneCastAction *harmonize = nullptr;
    for (const auto &action : p1.latestLegal.zone_cast_actions()) {
        if (action.card_name() == "Unending Whisper" &&
            action.cast_method() == ruled::v1::CAST_METHOD_HARMONIZE) {
            harmonize = &action;
            break;
        }
    }
    ASSERT_NE(harmonize, nullptr);
    EXPECT_TRUE(std::none_of(p2.latestLegal.zone_cast_actions().begin(), p2.latestLegal.zone_cast_actions().end(),
                             [](const ruled::v1::LegalZoneCastAction &action) {
                                 return action.card_name() == "Unending Whisper" &&
                                        action.cast_method() == ruled::v1::CAST_METHOD_HARMONIZE;
                             }));
    ASSERT_EQ(harmonize->cost_choices().cast_cost_groups_size(), 1);
    const auto &group = harmonize->cost_choices().cast_cost_groups(0);
    ASSERT_EQ(group.skip_label(), "Pay full Harmonize cost");
    ASSERT_EQ(group.options_size(), 1);
    const auto &tapOption = group.options(0);
    ASSERT_EQ(tapOption.kind(), ruled::v1::CAST_COST_OPTION_KIND_TAP_PERMANENT_FOR_GENERIC_REDUCTION);
    ASSERT_EQ(tapOption.valid_permanent_ids_size(), 1);
    ASSERT_EQ(tapOption.valid_permanent_generations_size(), 1);
    ASSERT_EQ(tapOption.valid_permanent_generic_reductions_size(), 1);
    EXPECT_EQ(tapOption.valid_permanent_generic_reductions(0), 2u);

    const quint32 whisperOid = harmonize->object_id();
    const quint32 creatureOid = tapOption.valid_permanent_ids(0);
    ASSERT_TRUE(p1.serverCardByEngineOid.count(whisperOid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(whisperOid));
    ASSERT_EQ(p1.serverCardByEngineOid[whisperOid], p2.serverCardByEngineOid[whisperOid]);
    p1.harmonizePhysicalCardId = p1.serverCardByEngineOid[whisperOid];
    p2.harmonizePhysicalCardId = p2.serverCardByEngineOid[whisperOid];
    p1.harmonizeCreatureOid = creatureOid;
    p2.harmonizeCreatureOid = creatureOid;
    p1.harmonizeFlowActive = true;
    p2.harmonizeFlowActive = true;

    ruled::v1::RuledCommand castHarmonize;
    auto *cast = castHarmonize.mutable_cast_spell();
    cast->set_cast_method(ruled::v1::CAST_METHOD_HARMONIZE);
    cast->mutable_source()->set_graveyard_object_id(whisperOid);
    cast->set_face_index(harmonize->face_index());
    auto *selection = cast->add_cast_cost_group_selections();
    selection->set_group_index(group.group_index());
    selection->set_option_index(tapOption.option_index());
    selection->set_permanent_id(creatureOid);
    selection->set_expected_zone_change_generation(tapOption.valid_permanent_generations(0));
    ASSERT_TRUE(sendAndPump(p1, castHarmonize, QStringLiteral("cast Unending Whisper with Harmonize")));
    EXPECT_TRUE(p1.sawHarmonizeStackReceipt && p2.sawHarmonizeStackReceipt);
    EXPECT_TRUE(p1.sawHarmonizeGraveToStack && p2.sawHarmonizeGraveToStack);
    EXPECT_TRUE(permanentTapped(p1, p1.myId, creatureOid));
    EXPECT_TRUE(permanentTapped(p2, p1.myId, creatureOid));

    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));
    EXPECT_TRUE(p1.sawHarmonizeStackToExile && p2.sawHarmonizeStackToExile);
    EXPECT_TRUE(p1.harmonizePhysicalIdentityContinuous && p2.harmonizePhysicalIdentityContinuous);
    EXPECT_EQ(p1.harmonizePhysicalCardId, p2.harmonizePhysicalCardId);

    p1.harmonizeFlowActive = false;
    p2.harmonizeFlowActive = false;
    p1.normalWhisperFlowActive = true;
    p2.normalWhisperFlowActive = true;
    ASSERT_TRUE(devPut("Unending Whisper", ruled::v1::DEV_ZONE_HAND, false));
    ruled::v1::RuledCommand addNormalMana;
    addNormalMana.mutable_dev_command()->set_target_player_id(p1.myId);
    addNormalMana.mutable_dev_command()->mutable_add_mana()->set_u(1);
    ASSERT_TRUE(sendAndPump(p1, addNormalMana, QStringLiteral("dev: add {U} for normal cast")));
    const auto *normal = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Unending Whisper"));
    ASSERT_NE(normal, nullptr);
    ruled::v1::RuledCommand castNormal;
    castNormal.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    castNormal.mutable_cast_spell()->mutable_source()->set_hand_index(normal->hand_index());
    ASSERT_TRUE(sendAndPump(p1, castNormal, QStringLiteral("cast Unending Whisper normally")));
    EXPECT_TRUE(p1.sawNormalWhisperHandToStack && p2.sawNormalWhisperHandToStack);
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));
    EXPECT_TRUE(p1.sawNormalWhisperStackToGrave && p2.sawNormalWhisperStackToGrave);
}

TEST_F(RuledE2ESmokeTest, GraveyardTargetCohortIsPrivateAndMovesExactPhysicalCards)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    {
        const std::string msg = started.message();
        if (msg.rfind("SKIP:", 0) == 0) {
            GTEST_SKIP() << msg.substr(5);
        }
    }

    SmokeClient p1(SmokeClient::Role::Aggressor, QStringLiteral("graveyardp1"), &transcript);
    SmokeClient p2(SmokeClient::Role::Hoarder, QStringLiteral("graveyardp2"), &transcript);
    p2.didMulligan = true;
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Plains")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000,
                             "graveyard cohort game start (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000,
                             "graveyard cohort game start (p2)"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());

    QElapsedTimer openingDeadline;
    openingDeadline.start();
    while (openingDeadline.elapsed() < 30000) {
        p1.pump(25);
        p2.pump(25);
        if (p1.phase == ruled::v1::PHASE_ID_MAIN1 && p2.phase == ruled::v1::PHASE_ID_MAIN1 &&
            p1.priorityPlayer == p1.myId && p2.priorityPlayer == p1.myId) {
            break;
        }
        p1.act();
        p2.act();
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);

    auto sendAndPump = [&](SmokeClient &sender, const ruled::v1::RuledCommand &command,
                           const QString &description) {
        const quint64 p1Version = p1.stateVersion;
        const quint64 p2Version = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while (wait.elapsed() < 10000 &&
               (p1.stateVersion <= p1Version || p2.stateVersion <= p2Version)) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > p1Version && p2.stateVersion > p2Version;
    };
    auto devPut = [&](int targetPlayer, const char *cardName, ruled::v1::DevZone zone, bool ready) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(targetPlayer);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(cardName);
        put->set_zone(zone);
        put->set_ready(ready);
        return sendAndPump(p1, command, QStringLiteral("dev: put %1 for graveyard cohort").arg(cardName));
    };
    auto devMove = [&](int targetPlayer, const char *cardName, ruled::v1::DevZone zone) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(targetPlayer);
        auto *move = dev->mutable_move_card();
        move->set_card_name(cardName);
        move->set_zone(zone);
        return sendAndPump(p1, command, QStringLiteral("dev: move %1 for graveyard cohort").arg(cardName));
    };
    auto passPriority = [&](SmokeClient &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return sendAndPump(client, command, QStringLiteral("pass priority in graveyard cohort"));
    };

    ASSERT_TRUE(devPut(p1.myId, "Grizzly Bears", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(devMove(p1.myId, "Grizzly Bears", ruled::v1::DEV_ZONE_GRAVEYARD));
    ASSERT_TRUE(devPut(p1.myId, "Storm Crow", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(devMove(p1.myId, "Storm Crow", ruled::v1::DEV_ZONE_GRAVEYARD));
    ASSERT_TRUE(devPut(p2.myId, "Forest", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(devMove(p2.myId, "Forest", ruled::v1::DEV_ZONE_GRAVEYARD));
    p1.graveyardCohortFlowActive = true;
    p2.graveyardCohortFlowActive = true;
    ASSERT_TRUE(devPut(p1.myId, "Arashin Sunshield", ruled::v1::DEV_ZONE_BATTLEFIELD, true));

    ASSERT_TRUE(p1.pendingTriggerTarget.has_value());
    ASSERT_EQ(p1.pendingTriggerTarget->targets().groups_size(), 1);
    const auto &group = p1.pendingTriggerTarget->targets().groups(0);
    EXPECT_TRUE(group.same_graveyard());
    EXPECT_EQ(group.min(), 0u);
    EXPECT_EQ(group.max(), 2u);
    EXPECT_TRUE(p2.sawOtherTriggerTargetsRedacted);
    std::vector<quint32> ownTargets;
    for (const quint32 oid : group.valid_graveyard_ids()) {
        if (p1.graveyardOwnerByEngineOid[oid] == p1.myId) {
            ownTargets.push_back(oid);
        }
    }
    ASSERT_EQ(ownTargets.size(), 2u);
    for (const quint32 oid : ownTargets) {
        ASSERT_TRUE(p1.serverCardByEngineOid.count(oid));
        const int physicalId = p1.serverCardByEngineOid[oid];
        p1.graveyardCohortExpectedPhysicalIds.insert(physicalId);
        p2.graveyardCohortExpectedPhysicalIds.insert(physicalId);
    }

    ruled::v1::RuledCommand choose;
    for (const quint32 oid : ownTargets) {
        auto *target = choose.mutable_choose_trigger_target()->add_targets();
        target->set_object_id(oid);
        target->set_group_index(group.group_index());
        target->set_kind(ruled::v1::TARGET_REF_KIND_GRAVEYARD);
    }
    ASSERT_TRUE(sendAndPump(p1, choose, QStringLiteral("choose two cards from one graveyard")));
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));

    EXPECT_EQ(p1.graveyardCohortMovedPhysicalIds, p1.graveyardCohortExpectedPhysicalIds);
    EXPECT_EQ(p2.graveyardCohortMovedPhysicalIds, p2.graveyardCohortExpectedPhysicalIds);
    EXPECT_TRUE(p1.graveyardCohortPhysicalIdentityContinuous);
    EXPECT_TRUE(p2.graveyardCohortPhysicalIdentityContinuous);

    ASSERT_TRUE(devPut(p1.myId, "Malevolent Chandelier", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    ruled::v1::RuledCommand addMana;
    addMana.mutable_dev_command()->set_target_player_id(p1.myId);
    addMana.mutable_dev_command()->mutable_add_mana()->set_c(2);
    ASSERT_TRUE(sendAndPump(p1, addMana, QStringLiteral("dev: add {2} for Malevolent Chandelier")));
    const auto chandelier = std::find_if(
        p1.battlefieldByPlayer[p1.myId].begin(), p1.battlefieldByPlayer[p1.myId].end(),
        [](const SmokeClient::Permanent &permanent) { return permanent.cardId == QStringLiteral("malevolent_chandelier"); });
    ASSERT_NE(chandelier, p1.battlefieldByPlayer[p1.myId].end());
    const quint64 abilityKey = (static_cast<quint64>(chandelier->oid) << 32);
    const auto targets = p1.latestLegal.valid_targets_by_ability().find(abilityKey);
    ASSERT_NE(targets, p1.latestLegal.valid_targets_by_ability().end());
    ASSERT_EQ(targets->second.groups_size(), 1);
    const auto &libraryGroup = targets->second.groups(0);
    const auto opponentTarget = std::find_if(
        libraryGroup.valid_graveyard_ids().begin(), libraryGroup.valid_graveyard_ids().end(),
        [&](quint32 oid) { return p1.graveyardOwnerByEngineOid[oid] == p2.myId; });
    ASSERT_NE(opponentTarget, libraryGroup.valid_graveyard_ids().end());
    ASSERT_TRUE(p1.serverCardByEngineOid.count(*opponentTarget));
    const int libraryPhysicalId = p1.serverCardByEngineOid[*opponentTarget];
    p1.graveyardLibraryExpectedPhysicalId = libraryPhysicalId;
    p2.graveyardLibraryExpectedPhysicalId = libraryPhysicalId;
    ruled::v1::RuledCommand activate;
    auto *ability = activate.mutable_activate_ability();
    ability->set_source_object_id(chandelier->oid);
    ability->set_ability_index(0);
    ability->set_expected_zone_change_generation(chandelier->generation);
    auto *libraryTarget = ability->add_targets();
    libraryTarget->set_object_id(*opponentTarget);
    libraryTarget->set_group_index(libraryGroup.group_index());
    libraryTarget->set_kind(ruled::v1::TARGET_REF_KIND_GRAVEYARD);
    ASSERT_TRUE(sendAndPump(p1, activate, QStringLiteral("activate Malevolent Chandelier")));
    ASSERT_TRUE(passPriority(p1));
    ASSERT_TRUE(passPriority(p2));
    EXPECT_TRUE(p1.sawGraveyardToLibraryPhysicalMove);
    EXPECT_TRUE(p2.sawGraveyardToLibraryPhysicalMove);
    EXPECT_TRUE(p1.graveyardCohortPhysicalIdentityContinuous);
    EXPECT_TRUE(p2.graveyardCohortPhysicalIdentityContinuous);
}

TEST_F(RuledE2ESmokeTest, PlaneswalkerBattleTargetsSplitCombatAndSiegeCastReachBothClients)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }

    SmokeClient p1(SmokeClient::Role::Aggressor, QStringLiteral("battlep1"), &transcript);
    SmokeClient p2(SmokeClient::Role::Hoarder, QStringLiteral("battlep2"), &transcript);
    p2.didMulligan = true;
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Mountain")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000,
                             "issue 72 game start (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000,
                             "issue 72 game start (p2)"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());
    QElapsedTimer opening;
    opening.start();
    while (opening.elapsed() < 30000) {
        p1.pump(25);
        p2.pump(25);
        if (p1.phase == ruled::v1::PHASE_ID_MAIN1 && p2.phase == ruled::v1::PHASE_ID_MAIN1 &&
            p1.priorityPlayer == p1.myId && p2.priorityPlayer == p1.myId) {
            break;
        }
        p1.act();
        p2.act();
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);
    ASSERT_EQ(p1.priorityPlayer, p1.myId);

    auto send = [&](SmokeClient &sender, const ruled::v1::RuledCommand &command, const QString &description) {
        const quint64 before1 = p1.stateVersion;
        const quint64 before2 = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while ((p1.stateVersion <= before1 || p2.stateVersion <= before2) && wait.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > before1 && p2.stateVersion > before2;
    };
    auto put = [&](int player, const char *name, ruled::v1::DevZone zone, bool ready) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(player);
        auto *placement = dev->mutable_put_card_in_zone();
        placement->set_card_name(name);
        placement->set_zone(zone);
        placement->set_ready(ready);
        return send(p1, command, QStringLiteral("issue 72 put %1").arg(name));
    };
    auto mana = [&](int blue, int black, int red) {
        ruled::v1::RuledCommand command;
        command.mutable_dev_command()->set_target_player_id(p1.myId);
        auto *gift = command.mutable_dev_command()->mutable_add_mana();
        gift->set_u(blue);
        gift->set_b(black);
        gift->set_r(red);
        return send(p1, command, QStringLiteral("issue 72 add mana"));
    };
    auto pass = [&](SmokeClient &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(client, command, QStringLiteral("issue 72 pass"));
    };
    auto find = [](const SmokeClient &client, int controller, const QString &id)
        -> std::optional<SmokeClient::Permanent> {
        const auto battlefield = client.battlefieldByPlayer.find(controller);
        if (battlefield == client.battlefieldByPlayer.end()) {
            return std::nullopt;
        }
        const auto card = std::find_if(battlefield->second.begin(), battlefield->second.end(),
                                       [&](const SmokeClient::Permanent &permanent) {
                                           return permanent.cardId == id;
                                       });
        return card == battlefield->second.end() ? std::nullopt : std::optional(*card);
    };
    auto findMatching = [](const SmokeClient &client, int controller, const auto &predicate)
        -> std::optional<SmokeClient::Permanent> {
        const auto battlefield = client.battlefieldByPlayer.find(controller);
        if (battlefield == client.battlefieldByPlayer.end()) {
            return std::nullopt;
        }
        const auto card = std::find_if(battlefield->second.begin(), battlefield->second.end(), predicate);
        return card == battlefield->second.end() ? std::nullopt : std::optional(*card);
    };
    auto castAt = [&](const QString &name, quint32 oid) {
        const auto *action = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, name);
        if (!action) {
            return false;
        }
        const auto published = p1.latestLegal.valid_targets_by_hand_slot().find(action->hand_index() << 8);
        if (published == p1.latestLegal.valid_targets_by_hand_slot().end() ||
            published->second.groups_size() != 1) {
            return false;
        }
        const auto &group = published->second.groups(0);
        if (std::find(group.valid_permanent_ids().begin(), group.valid_permanent_ids().end(), oid) ==
            group.valid_permanent_ids().end()) {
            return false;
        }
        ruled::v1::RuledCommand command;
        auto *cast = command.mutable_cast_spell();
        cast->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
        cast->mutable_source()->set_hand_index(action->hand_index());
        auto *target = cast->add_targets();
        target->set_object_id(oid);
        target->set_group_index(group.group_index());
        target->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
        return send(p1, command, QStringLiteral("issue 72 cast %1").arg(name));
    };

    ASSERT_TRUE(put(p1.myId, "Jace Beleren", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    auto ownJace = find(p1, p1.myId, QStringLiteral("jace_beleren"));
    ASSERT_TRUE(ownJace.has_value());
    ASSERT_TRUE(put(p1.myId, "Shock", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(mana(0, 0, 1));
    ASSERT_TRUE(castAt(QStringLiteral("Shock"), ownJace->oid));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    ownJace = find(p1, p1.myId, QStringLiteral("jace_beleren"));
    ASSERT_TRUE(ownJace.has_value());
    EXPECT_TRUE(ownJace->planeswalker);
    EXPECT_EQ(ownJace->loyalty, 1);
    ASSERT_TRUE(find(p2, p1.myId, QStringLiteral("jace_beleren")).has_value());
    EXPECT_EQ(find(p2, p1.myId, QStringLiteral("jace_beleren"))->loyalty, 1);

    ruled::v1::RuledCommand activate;
    p1.setBattlefieldAbilitySource(activate.mutable_activate_ability(), ownJace->oid);
    activate.mutable_activate_ability()->set_ability_index(0);
    ASSERT_TRUE(send(p1, activate, QStringLiteral("issue 72 activate Jace +2")));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    EXPECT_EQ(find(p1, p1.myId, QStringLiteral("jace_beleren"))->loyalty, 3);
    EXPECT_FALSE(find(p1, p1.myId, QStringLiteral("jace_beleren"))->firstAbilityActivatable);

    ASSERT_TRUE(put(p2.myId, "Jace Beleren", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    auto opposingJace = find(p1, p2.myId, QStringLiteral("jace_beleren"));
    ASSERT_TRUE(opposingJace.has_value());
    ASSERT_TRUE(put(p1.myId, "Finishing Blow", ruled::v1::DEV_ZONE_HAND, false));
    ASSERT_TRUE(mana(0, 5, 0));
    ASSERT_TRUE(castAt(QStringLiteral("Finishing Blow"), opposingJace->oid));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    EXPECT_FALSE(find(p1, p2.myId, QStringLiteral("jace_beleren")).has_value());

    ASSERT_TRUE(put(p2.myId, "Jace Beleren", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    ASSERT_TRUE(put(p1.myId, "Invasion of Ulgrotha // Grandmother Ravi Sengir",
                    ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    ASSERT_TRUE(p1.pendingChoice.has_value());
    ASSERT_EQ(p1.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_BATTLE_PROTECTOR);
    EXPECT_FALSE(p2.pendingChoice.has_value());
    EXPECT_FALSE(findMatching(p1, p1.myId,
                              [](const SmokeClient::Permanent &permanent) { return permanent.battle; })
                     .has_value());
    ruled::v1::RuledCommand chooseProtector;
    chooseProtector.mutable_submit_resolution_choice()->add_chosen_object_ids(p2.myId);
    p1.pendingChoice.reset();
    ASSERT_TRUE(send(p1, chooseProtector, QStringLiteral("issue 72 choose Battle protector")));
    ASSERT_TRUE(p1.pendingTriggerTarget.has_value());
    ruled::v1::RuledCommand targetEtb;
    auto *playerTarget = targetEtb.mutable_choose_trigger_target()->add_targets();
    playerTarget->set_object_id(p2.myId);
    playerTarget->set_kind(ruled::v1::TARGET_REF_KIND_PLAYER);
    p1.pendingTriggerTarget.reset();
    ASSERT_TRUE(send(p1, targetEtb, QStringLiteral("issue 72 choose Battle ETB target")));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));

    auto battle = findMatching(p1, p1.myId,
                               [](const SmokeClient::Permanent &permanent) { return permanent.battle; });
    opposingJace = find(p1, p2.myId, QStringLiteral("jace_beleren"));
    ASSERT_TRUE(battle.has_value() && opposingJace.has_value());
    EXPECT_TRUE(battle->battle);
    EXPECT_EQ(battle->defense, 5);
    EXPECT_EQ(battle->battleProtector, p2.myId);
    ASSERT_TRUE(findMatching(p2, p1.myId,
                             [](const SmokeClient::Permanent &permanent) { return permanent.battle; })
                    .has_value());
    EXPECT_EQ(findMatching(p2, p1.myId,
                           [](const SmokeClient::Permanent &permanent) { return permanent.battle; })
                  ->defense,
              5);
    const quint32 battleOid = battle->oid;
    ASSERT_TRUE(p1.serverCardByEngineOid.count(battleOid));
    const int physicalId = p1.serverCardByEngineOid[battleOid];
    EXPECT_EQ(p2.serverCardByEngineOid[battleOid], physicalId);

    ASSERT_TRUE(put(p1.myId, "Hill Giant", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    ASSERT_TRUE(put(p1.myId, "Serra Angel", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    ASSERT_TRUE(put(p1.myId, "Craw Wurm", ruled::v1::DEV_ZONE_BATTLEFIELD, true));
    QElapsedTimer toAttack;
    toAttack.start();
    while (p1.phase != ruled::v1::PHASE_ID_DECLARE_ATTACKERS && toAttack.elapsed() < 20000) {
        ASSERT_TRUE(pass(p1.priorityPlayer == p1.myId ? p1 : p2));
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_DECLARE_ATTACKERS);
    const auto hill = find(p1, p1.myId, QStringLiteral("hill_giant"));
    const auto angel = find(p1, p1.myId, QStringLiteral("serra_angel"));
    const auto wurm = find(p1, p1.myId, QStringLiteral("craw_wurm"));
    ASSERT_TRUE(hill.has_value() && angel.has_value() && wurm.has_value());
    std::map<quint32, ruled::v1::AttackAssignment> selected;
    for (const auto &assignment : p1.latestLegal.legal_attack_assignments()) {
        const auto &defender = assignment.defender();
        if ((assignment.attacker_object_id() == hill->oid && defender.kind() == ruled::v1::TARGET_REF_KIND_PLAYER &&
             defender.object_id() == static_cast<quint32>(p2.myId)) ||
            (assignment.attacker_object_id() == angel->oid && defender.object_id() == opposingJace->oid) ||
            (assignment.attacker_object_id() == wurm->oid && defender.object_id() == battleOid)) {
            selected[assignment.attacker_object_id()] = assignment;
        }
    }
    ASSERT_EQ(selected.size(), 3u);
    ruled::v1::RuledCommand preview;
    ruled::v1::RuledCommand declare;
    for (const auto &[_, assignment] : selected) {
        *preview.mutable_preview_declare_attackers()->add_assignments() = assignment;
        *declare.mutable_declare_attackers()->add_assignments() = assignment;
    }
    ASSERT_TRUE(send(p1, preview, QStringLiteral("issue 72 preview split attackers")));
    EXPECT_EQ(p1.latestAttackPreviewAssignments.size(), 3u);
    EXPECT_EQ(p2.latestAttackPreviewAssignments.size(), 3u);
    ASSERT_TRUE(send(p1, declare, QStringLiteral("issue 72 declare split attackers")));
    EXPECT_EQ(p1.latestDeclaredAttackAssignments.size(), 3u);
    EXPECT_EQ(p2.latestDeclaredAttackAssignments.size(), 3u);

    // With no eligible blockers, the authoritative engine auto-commits an empty declaration and
    // advances directly into combat damage.
    QElapsedTimer siege;
    siege.start();
    while ((!p1.pendingChoice.has_value() ||
            p1.pendingChoice->choice_kind() != ruled::v1::CHOICE_KIND_SIEGE_CAST) &&
           siege.elapsed() < 30000) {
        if (p1.priorityPlayer == p1.myId) {
            ASSERT_TRUE(pass(p1));
        } else if (p1.priorityPlayer == p2.myId) {
            ASSERT_TRUE(pass(p2));
        } else {
            p1.pump(25);
            p2.pump(25);
        }
    }
    ASSERT_TRUE(p1.pendingChoice.has_value());
    ASSERT_EQ(p1.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_SIEGE_CAST);
    EXPECT_FALSE(p2.pendingChoice.has_value());
    ASSERT_TRUE(p1.serverCardByEngineOid.count(battleOid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(battleOid));
    const int exiledPhysicalId = p1.serverCardByEngineOid[battleOid];
    EXPECT_EQ(p2.serverCardByEngineOid[battleOid], exiledPhysicalId);

    ruled::v1::RuledCommand castBack;
    auto *submission = castBack.mutable_submit_resolution_choice();
    submission->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_CAST_TRANSFORMED);
    auto *cast = submission->mutable_cast_spell();
    cast->set_cast_method(ruled::v1::CAST_METHOD_SIEGE_DEFEAT);
    cast->set_face_index(1);
    cast->mutable_source()->set_exile_object_id(battleOid);
    p1.pendingChoice.reset();
    ASSERT_TRUE(send(p1, castBack, QStringLiteral("issue 72 cast Siege transformed")));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    battle = findMatching(p1, p1.myId,
                          [battleOid](const SmokeClient::Permanent &permanent) { return permanent.oid == battleOid; });
    const auto observer = findMatching(
        p2, p1.myId, [battleOid](const SmokeClient::Permanent &permanent) { return permanent.oid == battleOid; });
    ASSERT_TRUE(battle.has_value() && observer.has_value());
    EXPECT_EQ(battle->oid, battleOid);
    EXPECT_EQ(observer->oid, battleOid);
    EXPECT_EQ(battle->faceIndex, 1);
    EXPECT_EQ(observer->faceIndex, 1);
    EXPECT_TRUE(battle->creature && observer->creature);
    EXPECT_EQ(battle->defense, -1);
    EXPECT_EQ(observer->defense, -1);
    EXPECT_EQ(p1.serverCardByEngineOid[battleOid], exiledPhysicalId);
    EXPECT_EQ(p2.serverCardByEngineOid[battleOid], exiledPhysicalId);
}

TEST_F(RuledE2ESmokeTest, MobilizeDefenderChoiceAndTokenLifecycleReachBothClients)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }

    SmokeClient p1(SmokeClient::Role::Aggressor, QStringLiteral("mobilizep1"), &transcript);
    SmokeClient p2(SmokeClient::Role::Hoarder, QStringLiteral("mobilizep2"), &transcript);
    p2.didMulligan = true;
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Mountain")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000,
                             "issue 106 game start (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000,
                             "issue 106 game start (p2)"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());
    QElapsedTimer opening;
    opening.start();
    while (opening.elapsed() < 30000) {
        p1.pump(25);
        p2.pump(25);
        if (p1.phase == ruled::v1::PHASE_ID_MAIN1 && p2.phase == ruled::v1::PHASE_ID_MAIN1 &&
            p1.priorityPlayer == p1.myId && p2.priorityPlayer == p1.myId) {
            break;
        }
        p1.act();
        p2.act();
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);

    auto send = [&](SmokeClient &sender, const ruled::v1::RuledCommand &command, const QString &description) {
        const quint64 before1 = p1.stateVersion;
        const quint64 before2 = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while ((p1.stateVersion <= before1 || p2.stateVersion <= before2) && wait.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > before1 && p2.stateVersion > before2;
    };
    auto put = [&](int player, const char *name) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(player);
        auto *placement = dev->mutable_put_card_in_zone();
        placement->set_card_name(name);
        placement->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
        placement->set_ready(true);
        return send(p1, command, QStringLiteral("issue 106 put %1").arg(name));
    };
    auto pass = [&](SmokeClient &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(client, command, QStringLiteral("issue 106 pass"));
    };
    auto find = [](const SmokeClient &client, int controller, const QString &id)
        -> std::optional<SmokeClient::Permanent> {
        const auto battlefield = client.battlefieldByPlayer.find(controller);
        if (battlefield == client.battlefieldByPlayer.end()) {
            return std::nullopt;
        }
        const auto card = std::find_if(battlefield->second.begin(), battlefield->second.end(),
                                       [&](const SmokeClient::Permanent &permanent) {
                                           return permanent.cardId == id;
                                       });
        return card == battlefield->second.end() ? std::nullopt : std::optional(*card);
    };

    ASSERT_TRUE(put(p2.myId, "Jace Beleren"));
    ASSERT_TRUE(put(p1.myId, "Dragonback Lancer"));
    const auto lancer = find(p1, p1.myId, QStringLiteral("dragonback_lancer"));
    const auto jace = find(p1, p2.myId, QStringLiteral("jace_beleren"));
    ASSERT_TRUE(lancer.has_value() && jace.has_value());

    QElapsedTimer toAttack;
    toAttack.start();
    while (p1.phase != ruled::v1::PHASE_ID_DECLARE_ATTACKERS && toAttack.elapsed() < 20000) {
        ASSERT_TRUE(pass(p1.priorityPlayer == p1.myId ? p1 : p2));
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_DECLARE_ATTACKERS);

    const auto assignment = std::find_if(
        p1.latestLegal.legal_attack_assignments().begin(), p1.latestLegal.legal_attack_assignments().end(),
        [&](const ruled::v1::AttackAssignment &candidate) {
            return candidate.attacker_object_id() == lancer->oid && candidate.has_defender() &&
                   candidate.defender().kind() == ruled::v1::TARGET_REF_KIND_PLAYER &&
                   candidate.defender().object_id() == static_cast<quint32>(p2.myId);
        });
    ASSERT_NE(assignment, p1.latestLegal.legal_attack_assignments().end());
    ruled::v1::RuledCommand declare;
    *declare.mutable_declare_attackers()->add_assignments() = *assignment;
    ASSERT_TRUE(send(p1, declare, QStringLiteral("issue 106 declare Dragonback Lancer")));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));

    ASSERT_TRUE(p1.pendingChoice.has_value());
    ASSERT_EQ(p1.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_ATTACKING_TOKEN_DEFENDER);
    EXPECT_TRUE(p1.sawMobilizeDefenderChoice);
    EXPECT_TRUE(p2.sawMobilizeObserverWait);
    EXPECT_FALSE(p2.pendingChoice.has_value());
    const auto defender = std::find_if(
        p1.pendingChoice->combat_defender_options().begin(), p1.pendingChoice->combat_defender_options().end(),
        [&](const ruled::v1::CombatDefenderOption &option) {
            return option.has_defender() && option.defender().kind() == ruled::v1::TARGET_REF_KIND_PERMANENT &&
                   option.defender().object_id() == jace->oid &&
                   option.defender_zone_change_generation() == jace->generation;
        });
    ASSERT_NE(defender, p1.pendingChoice->combat_defender_options().end());

    ruled::v1::RuledCommand chooseDefender;
    *chooseDefender.mutable_submit_resolution_choice()->mutable_chosen_combat_defender() = *defender;
    p1.pendingChoice.reset();
    ASSERT_TRUE(send(p1, chooseDefender, QStringLiteral("issue 106 choose Jace defender")));

    ASSERT_TRUE(p1.sawMobilizeTokenCreated && p2.sawMobilizeTokenCreated);
    ASSERT_NE(p1.mobilizeTokenOid, 0u);
    ASSERT_EQ(p1.mobilizeTokenOid, p2.mobilizeTokenOid);
    const quint32 tokenOid = p1.mobilizeTokenOid;
    ASSERT_TRUE(p1.serverCardByEngineOid.count(tokenOid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(tokenOid));
    const int physicalTokenId = p1.serverCardByEngineOid[tokenOid];
    EXPECT_EQ(p2.serverCardByEngineOid[tokenOid], physicalTokenId);
    EXPECT_TRUE(p1.physicallyTappedCardIds.count(physicalTokenId));
    EXPECT_TRUE(p2.physicallyTappedCardIds.count(physicalTokenId));
    EXPECT_TRUE(p1.physicallyAttackingCardIds.count(physicalTokenId));
    EXPECT_TRUE(p2.physicallyAttackingCardIds.count(physicalTokenId));
    ASSERT_EQ(p1.latestAddedAttackAssignments.size(), 1u);
    ASSERT_EQ(p2.latestAddedAttackAssignments.size(), 1u);
    EXPECT_EQ(p1.latestAddedAttackAssignments.front().attacker_object_id(), tokenOid);
    EXPECT_EQ(p1.latestAddedAttackAssignments.front().defender().object_id(), jace->oid);
    EXPECT_EQ(p2.latestAddedAttackAssignments.front().defender().object_id(), jace->oid);

    QElapsedTimer toEndStep;
    toEndStep.start();
    while (!(p1.phase == ruled::v1::PHASE_ID_END_STEP && p1.stackDepth > 0) && toEndStep.elapsed() < 30000) {
        if (p1.priorityPlayer == p1.myId) {
            ASSERT_TRUE(pass(p1));
        } else if (p1.priorityPlayer == p2.myId) {
            ASSERT_TRUE(pass(p2));
        } else {
            p1.pump(25);
            p2.pump(25);
        }
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_END_STEP);
    ASSERT_GT(p1.stackDepth, 0);
    ASSERT_TRUE(pass(p1.priorityPlayer == p1.myId ? p1 : p2));
    ASSERT_TRUE(pass(p1.priorityPlayer == p1.myId ? p1 : p2));
    EXPECT_TRUE(p1.sawMobilizeTokenSacrificed && p2.sawMobilizeTokenSacrificed);
}

TEST_F(RuledE2ESmokeTest, TokenCopiesAndPopulatePreserveBothClientsPhysicalIdentity)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }
    SmokeClient p1(SmokeClient::Role::Aggressor, QStringLiteral("copytokenp1"), &transcript);
    SmokeClient p2(SmokeClient::Role::Hoarder, QStringLiteral("copytokenp2"), &transcript);
    p2.didMulligan = true;
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000, "copy game start"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000, "copy game start"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());
    QElapsedTimer opening;
    opening.start();
    while (opening.elapsed() < 30000) {
        p1.pump(25);
        p2.pump(25);
        if (p1.phase == ruled::v1::PHASE_ID_MAIN1 && p2.phase == ruled::v1::PHASE_ID_MAIN1 &&
            p1.priorityPlayer == p1.myId && p2.priorityPlayer == p1.myId) {
            break;
        }
        p1.act();
        p2.act();
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);
    ASSERT_EQ(p1.priorityPlayer, p1.myId);
    auto send = [&](SmokeClient &sender, const ruled::v1::RuledCommand &command) {
        const auto before1 = p1.stateVersion;
        const auto before2 = p2.stateVersion;
        sender.sendRuled(command, QStringLiteral("issue 46 command"));
        QElapsedTimer wait;
        wait.start();
        while ((p1.stateVersion <= before1 || p2.stateVersion <= before2) && wait.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > before1 && p2.stateVersion > before2;
    };
    auto pass = [&](SmokeClient &sender) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(sender, command);
    };
    auto put = [&](const char *name, ruled::v1::DevZone zone) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(p1.myId);
        dev->mutable_put_card_in_zone()->set_card_name(name);
        dev->mutable_put_card_in_zone()->set_zone(zone);
        return send(p1, command);
    };
    auto cast = [&](const QString &name, quint32 oid) {
        const auto *action = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, name);
        if (!action) {
            return false;
        }
        ruled::v1::RuledCommand command;
        auto *spell = command.mutable_cast_spell();
        spell->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
        spell->mutable_source()->set_hand_index(action->hand_index());
        if (oid != 0) {
            const auto published = p1.latestLegal.valid_targets_by_hand_slot().find(action->hand_index() << 8);
            if (published == p1.latestLegal.valid_targets_by_hand_slot().end() ||
                published->second.groups_size() != 1) {
                return false;
            }
            const auto &group = published->second.groups(0);
            if (std::find(group.valid_permanent_ids().begin(), group.valid_permanent_ids().end(), oid) ==
                group.valid_permanent_ids().end()) {
                return false;
            }
            auto *target = spell->add_targets();
            target->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
            target->set_object_id(oid);
            target->set_group_index(group.group_index());
        }
        return send(p1, command);
    };
    ASSERT_TRUE(put("Serra Angel", ruled::v1::DEV_ZONE_BATTLEFIELD));
    ASSERT_EQ(p1.battlefieldByPlayer[p1.myId].size(), 1u);
    const quint32 original = p1.battlefieldByPlayer[p1.myId][0].oid;
    const int originalPhysical = p1.serverCardByEngineOid.at(original);
    ASSERT_TRUE(put("Cackling Counterpart", ruled::v1::DEV_ZONE_HAND));
    ASSERT_TRUE(put("Wake the Reflections", ruled::v1::DEV_ZONE_HAND));
    ASSERT_TRUE(put("Unsummon", ruled::v1::DEV_ZONE_HAND));
    ruled::v1::RuledCommand mana;
    mana.mutable_dev_command()->set_target_player_id(p1.myId);
    mana.mutable_dev_command()->mutable_add_mana()->set_u(3);
    mana.mutable_dev_command()->mutable_add_mana()->set_w(1);
    mana.mutable_dev_command()->mutable_add_mana()->set_c(1);
    ASSERT_TRUE(send(p1, mana));
    ASSERT_TRUE(cast(QStringLiteral("Cackling Counterpart"), original));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    ASSERT_EQ(p1.battlefieldByPlayer[p1.myId].size(), 2u);
    ASSERT_EQ(p2.battlefieldByPlayer[p1.myId].size(), 2u);
    const auto copied = std::find_if(p1.battlefieldByPlayer[p1.myId].begin(), p1.battlefieldByPlayer[p1.myId].end(),
                                     [original](const auto &permanent) { return permanent.oid != original; });
    ASSERT_NE(copied, p1.battlefieldByPlayer[p1.myId].end());
    const quint32 token = copied->oid;
    EXPECT_EQ(copied->power, 4);
    EXPECT_EQ(copied->toughness, 4);
    EXPECT_NE(p1.serverCardByEngineOid.at(token), originalPhysical);
    EXPECT_EQ(p1.serverCardByEngineOid.at(token), p2.serverCardByEngineOid.at(token));
    ASSERT_TRUE(cast(QStringLiteral("Wake the Reflections"), 0));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    ASSERT_TRUE(p1.pendingChoice.has_value());
    EXPECT_FALSE(p2.pendingChoice.has_value());
    EXPECT_EQ(p1.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_COPY_SOURCE);
    EXPECT_EQ(p1.pendingChoice->min(), 1u);
    ASSERT_EQ(p1.pendingChoice->candidate_object_ids_size(), 1);
    EXPECT_EQ(p1.pendingChoice->candidate_object_ids(0), token);
    ruled::v1::RuledCommand choose;
    choose.mutable_submit_resolution_choice()->add_chosen_object_ids(token);
    p1.pendingChoice.reset();
    ASSERT_TRUE(send(p1, choose));
    ASSERT_EQ(p1.battlefieldByPlayer[p1.myId].size(), 3u);
    ASSERT_EQ(p2.battlefieldByPlayer[p1.myId].size(), 3u);
    EXPECT_EQ(p1.serverCardByEngineOid.at(original), originalPhysical);
    ASSERT_TRUE(cast(QStringLiteral("Unsummon"), token));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));
    ASSERT_EQ(p1.battlefieldByPlayer[p1.myId].size(), 2u);
    ASSERT_EQ(p2.battlefieldByPlayer[p1.myId].size(), 2u);
    EXPECT_EQ(p1.serverCardByEngineOid.at(original), originalPhysical);
    for (const auto &permanent : p1.battlefieldByPlayer[p1.myId]) {
        EXPECT_NE(permanent.oid, token);
        EXPECT_EQ(p1.serverCardByEngineOid.at(permanent.oid), p2.serverCardByEngineOid.at(permanent.oid));
    }
}

TEST_F(RuledE2ESmokeTest, EarthbendBadgeRowsAndGenerationBoundReturnReachBothClients)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }

    SmokeClient p1(SmokeClient::Role::Aggressor, QStringLiteral("earthbendp1"), &transcript);
    SmokeClient p2(SmokeClient::Role::Hoarder, QStringLiteral("earthbendp2"), &transcript);
    p2.didMulligan = true;
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Mountain")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000,
                             "issue 150 game start (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000,
                             "issue 150 game start (p2)"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());

    QElapsedTimer opening;
    opening.start();
    while (opening.elapsed() < 30000) {
        p1.pump(25);
        p2.pump(25);
        if (p1.phase == ruled::v1::PHASE_ID_MAIN1 && p2.phase == ruled::v1::PHASE_ID_MAIN1 &&
            p1.priorityPlayer == p1.myId && p2.priorityPlayer == p1.myId) {
            break;
        }
        p1.act();
        p2.act();
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);

    auto send = [&](SmokeClient &sender, const ruled::v1::RuledCommand &command, const QString &description) {
        const quint64 before1 = p1.stateVersion;
        const quint64 before2 = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while ((p1.stateVersion <= before1 || p2.stateVersion <= before2) && wait.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > before1 && p2.stateVersion > before2;
    };
    auto pass = [&](SmokeClient &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(client, command, QStringLiteral("issue 150 pass"));
    };
    auto put = [&](const char *name, ruled::v1::DevZone zone = ruled::v1::DEV_ZONE_BATTLEFIELD) {
        ruled::v1::RuledCommand command;
        command.mutable_dev_command()->set_target_player_id(p1.myId);
        auto *placement = command.mutable_dev_command()->mutable_put_card_in_zone();
        placement->set_card_name(name);
        placement->set_zone(zone);
        placement->set_ready(true);
        return send(p1, command, QStringLiteral("earthbend put %1").arg(name));
    };
    auto find = [&](const SmokeClient &client, const char *cardId) -> std::optional<SmokeClient::Permanent> {
        const auto &objects = client.battlefieldByPlayer.at(p1.myId);
        const auto found = std::find_if(objects.begin(), objects.end(), [cardId](const auto &object) {
            return object.cardId == QLatin1String(cardId);
        });
        return found == objects.end() ? std::nullopt : std::optional(*found);
    };
    ASSERT_TRUE(put("Forest"));
    const auto forest = find(p1, "forest");
    ASSERT_TRUE(forest.has_value());
    const quint32 oid = forest->oid;
    ASSERT_TRUE(p1.serverCardByEngineOid.count(oid));
    const int physicalId = p1.serverCardByEngineOid.at(oid);
    const auto key = std::make_pair(p1.myId, physicalId);
    for (SmokeClient *client : {&p1, &p2}) {
        ASSERT_TRUE(client->physicalRowAndPt.count(key));
        EXPECT_EQ(client->physicalRowAndPt.at(key).first, 2);
        EXPECT_TRUE(client->physicalRowAndPt.at(key).second.isEmpty());
    }
    for (const auto destination : {ruled::v1::DEV_ZONE_GRAVEYARD, ruled::v1::DEV_ZONE_EXILE}) {
        ASSERT_TRUE(put("Rebellious Captives"));
        const auto captives = find(p1, "rebellious_captives");
        ASSERT_TRUE(captives.has_value());
        ruled::v1::RuledCommand mana;
        mana.mutable_dev_command()->set_target_player_id(p1.myId);
        mana.mutable_dev_command()->mutable_add_mana()->set_c(6);
        ASSERT_TRUE(send(p1, mana, QStringLiteral("earthbend mana")));
        ruled::v1::RuledCommand activate;
        p1.setBattlefieldAbilitySource(activate.mutable_activate_ability(), captives->oid);
        activate.mutable_activate_ability()->set_ability_index(0);
        auto *target = activate.mutable_activate_ability()->add_targets();
        target->set_object_id(oid);
        target->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
        ASSERT_TRUE(send(p1, activate, QStringLiteral("earthbend exhaust")));
        ASSERT_TRUE(pass(p1));
        ASSERT_TRUE(pass(p2));
        for (SmokeClient *client : {&p1, &p2}) {
            const auto animated = find(*client, "forest");
            ASSERT_TRUE(animated.has_value());
            EXPECT_TRUE(animated->creature && animated->haste);
            EXPECT_EQ(animated->power, 2);
            EXPECT_EQ(animated->toughness, 2);
            EXPECT_EQ(client->serverCardByEngineOid.at(oid), physicalId);
            ASSERT_TRUE(client->physicalRowAndPt.count(key));
            EXPECT_EQ(client->physicalRowAndPt.at(key).first, 0);
            EXPECT_EQ(client->physicalRowAndPt.at(key).second, QStringLiteral("2/2"));
        }
        const quint64 beforeGeneration = find(p1, "forest")->generation;
        auto move = [&](const char *name, ruled::v1::DevZone zone) {
            ruled::v1::RuledCommand command;
            command.mutable_dev_command()->set_target_player_id(p1.myId);
            auto *movement = command.mutable_dev_command()->mutable_move_card();
            movement->set_card_name(name);
            movement->set_zone(zone);
            return send(p1, command, QStringLiteral("earthbend move %1").arg(name));
        };
        ASSERT_TRUE(move("Rebellious Captives", ruled::v1::DEV_ZONE_GRAVEYARD));
        // Use real spells: their own stack-to-graveyard move shares the batch with
        // the land's departure and must not swap the two physical identities.
        const char *removal = destination == ruled::v1::DEV_ZONE_GRAVEYARD ? "Lightning Bolt" : "Swords to Plowshares";
        ASSERT_TRUE(put(removal, ruled::v1::DEV_ZONE_HAND));
        ruled::v1::RuledCommand removalMana;
        removalMana.mutable_dev_command()->set_target_player_id(p1.myId);
        auto *gift = removalMana.mutable_dev_command()->mutable_add_mana();
        gift->set_r(1);
        gift->set_w(1);
        ASSERT_TRUE(send(p1, removalMana, QStringLiteral("earthbend removal mana")));
        const auto *action = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QString::fromLatin1(removal));
        ASSERT_NE(action, nullptr);
        ruled::v1::RuledCommand cast;
        cast.mutable_cast_spell()->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
        cast.mutable_cast_spell()->mutable_source()->set_hand_index(action->hand_index());
        auto *removalTarget = cast.mutable_cast_spell()->add_targets();
        removalTarget->set_object_id(oid);
        removalTarget->set_kind(ruled::v1::TARGET_REF_KIND_PERMANENT);
        ASSERT_TRUE(send(p1, cast, QStringLiteral("earthbend cast removal")));
        ASSERT_TRUE(pass(p1));
        ASSERT_TRUE(pass(p2));
        EXPECT_EQ(p1.stackDepth, 1);
        EXPECT_EQ(p2.stackDepth, 1);
        EXPECT_FALSE(find(p1, "forest").has_value());
        EXPECT_FALSE(find(p2, "forest").has_value());
        ASSERT_TRUE(pass(p1));
        ASSERT_TRUE(pass(p2));
        for (SmokeClient *client : {&p1, &p2}) {
            const auto returned = find(*client, "forest");
            ASSERT_TRUE(returned.has_value());
            EXPECT_EQ(returned->generation, beforeGeneration + 2);
            EXPECT_TRUE(returned->tapped);
            EXPECT_FALSE(returned->creature || returned->haste);
            EXPECT_EQ(client->serverCardByEngineOid.at(oid), physicalId);
            ASSERT_TRUE(client->physicalRowAndPt.count(key));
            EXPECT_EQ(client->physicalRowAndPt.at(key).first, 2);
            EXPECT_TRUE(client->physicalRowAndPt.at(key).second.isEmpty());
        }
    }
}
TEST_F(RuledE2ESmokeTest, TappedOrdinaryTokenReachesBothClientsWithoutCombatState)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }

    SmokeClient p1(SmokeClient::Role::Aggressor, QStringLiteral("tappedtokenp1"), &transcript);
    SmokeClient p2(SmokeClient::Role::Hoarder, QStringLiteral("tappedtokenp2"), &transcript);
    p2.didMulligan = true;
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Mountain")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000,
                             "issue 162 game start (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000,
                             "issue 162 game start (p2)"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());

    QElapsedTimer opening;
    opening.start();
    while (opening.elapsed() < 30000) {
        p1.pump(25);
        p2.pump(25);
        if (p1.phase == ruled::v1::PHASE_ID_MAIN1 && p2.phase == ruled::v1::PHASE_ID_MAIN1 &&
            p1.priorityPlayer == p1.myId && p2.priorityPlayer == p1.myId) {
            break;
        }
        p1.act();
        p2.act();
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);

    auto send = [&](SmokeClient &sender, const ruled::v1::RuledCommand &command, const QString &description) {
        const quint64 before1 = p1.stateVersion;
        const quint64 before2 = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while ((p1.stateVersion <= before1 || p2.stateVersion <= before2) && wait.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > before1 && p2.stateVersion > before2;
    };
    auto pass = [&](SmokeClient &client) {
        ruled::v1::RuledCommand command;
        command.mutable_pass_priority();
        return send(client, command, QStringLiteral("issue 162 pass"));
    };
    auto findToken = [](const SmokeClient &client, int controller,
                        quint32 objectId) -> std::optional<SmokeClient::Permanent> {
        const auto battlefield = client.battlefieldByPlayer.find(controller);
        if (battlefield == client.battlefieldByPlayer.end()) {
            return std::nullopt;
        }
        const auto token = std::find_if(battlefield->second.begin(), battlefield->second.end(),
                                        [objectId](const SmokeClient::Permanent &permanent) {
                                            return permanent.oid == objectId;
                                        });
        return token == battlefield->second.end() ? std::nullopt : std::optional(*token);
    };

    ruled::v1::RuledCommand putMoxite;
    putMoxite.mutable_dev_command()->set_target_player_id(p1.myId);
    auto *placement = putMoxite.mutable_dev_command()->mutable_put_card_in_zone();
    placement->set_card_name("Melded Moxite");
    placement->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
    placement->set_ready(true);
    ASSERT_TRUE(send(p1, putMoxite, QStringLiteral("issue 162 put Melded Moxite")));

    ruled::v1::RuledCommand addMana;
    addMana.mutable_dev_command()->set_target_player_id(p1.myId);
    addMana.mutable_dev_command()->mutable_add_mana()->set_c(3);
    ASSERT_TRUE(send(p1, addMana, QStringLiteral("issue 162 add {3}")));

    const auto moxite = std::find_if(
        p1.battlefieldByPlayer[p1.myId].begin(), p1.battlefieldByPlayer[p1.myId].end(),
        [](const SmokeClient::Permanent &permanent) {
            return permanent.cardId == QStringLiteral("melded_moxite");
        });
    ASSERT_NE(moxite, p1.battlefieldByPlayer[p1.myId].end());
    ruled::v1::RuledCommand activate;
    p1.setBattlefieldAbilitySource(activate.mutable_activate_ability(), moxite->oid);
    activate.mutable_activate_ability()->set_ability_index(0);
    ASSERT_TRUE(send(p1, activate, QStringLiteral("issue 162 activate Melded Moxite")));
    ASSERT_TRUE(pass(p1));
    ASSERT_TRUE(pass(p2));

    ASSERT_TRUE(p1.sawTappedOrdinaryTokenCreated && p2.sawTappedOrdinaryTokenCreated);
    ASSERT_NE(p1.tappedOrdinaryTokenOid, 0u);
    ASSERT_EQ(p1.tappedOrdinaryTokenOid, p2.tappedOrdinaryTokenOid);
    const quint32 tokenOid = p1.tappedOrdinaryTokenOid;
    ASSERT_TRUE(p1.serverCardByEngineOid.count(tokenOid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(tokenOid));
    const int physicalTokenId = p1.serverCardByEngineOid[tokenOid];
    EXPECT_EQ(p2.serverCardByEngineOid[tokenOid], physicalTokenId);
    EXPECT_TRUE(p1.physicallyTappedCardIds.count(physicalTokenId));
    EXPECT_TRUE(p2.physicallyTappedCardIds.count(physicalTokenId));
    EXPECT_FALSE(p1.physicallyAttackingCardIds.count(physicalTokenId));
    EXPECT_FALSE(p2.physicallyAttackingCardIds.count(physicalTokenId));
    const auto p1Robot = findToken(p1, p1.myId, tokenOid);
    const auto p2Robot = findToken(p2, p1.myId, tokenOid);
    ASSERT_TRUE(p1Robot.has_value() && p2Robot.has_value());
    EXPECT_EQ(p1Robot->oid, tokenOid);
    EXPECT_EQ(p2Robot->oid, tokenOid);
    EXPECT_TRUE(p1Robot->tapped && p2Robot->tapped);
}

TEST_F(RuledE2ESmokeTest, ConvokePreviewsArePrivateReadOnlyAndCommitExactPhysicalTaps)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }

    SmokeClient p1(SmokeClient::Role::Aggressor, QStringLiteral("convokep1"), &transcript);
    SmokeClient p2(SmokeClient::Role::Hoarder, QStringLiteral("convokep2"), &transcript);
    p2.didMulligan = true;
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000,
                             "issue 145 game start (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000,
                             "issue 145 game start (p2)"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());

    QElapsedTimer opening;
    opening.start();
    while (opening.elapsed() < 30000) {
        p1.pump(25);
        p2.pump(25);
        if (p1.phase == ruled::v1::PHASE_ID_MAIN1 && p2.phase == ruled::v1::PHASE_ID_MAIN1 &&
            p1.priorityPlayer == p1.myId && p2.priorityPlayer == p1.myId) {
            break;
        }
        p1.act();
        p2.act();
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);
    ASSERT_EQ(p1.priorityPlayer, p1.myId);

    auto send = [&](SmokeClient &sender, const ruled::v1::RuledCommand &command, const QString &description) {
        const quint64 before1 = p1.stateVersion;
        const quint64 before2 = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while ((p1.stateVersion <= before1 || p2.stateVersion <= before2) && wait.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > before1 && p2.stateVersion > before2;
    };
    auto putPermanent = [&](const char *name, bool ready) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(p1.myId);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(name);
        put->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
        put->set_ready(ready);
        return send(p1, command, QStringLiteral("issue 145 put %1").arg(name));
    };
    auto findPermanent = [](const SmokeClient &client, int controller,
                            const QString &cardId) -> std::optional<SmokeClient::Permanent> {
        const auto battlefield = client.battlefieldByPlayer.find(controller);
        if (battlefield == client.battlefieldByPlayer.end()) {
            return std::nullopt;
        }
        const auto permanent = std::find_if(
            battlefield->second.begin(), battlefield->second.end(),
            [&cardId](const SmokeClient::Permanent &candidate) { return candidate.cardId == cardId; });
        return permanent == battlefield->second.end() ? std::nullopt : std::optional(*permanent);
    };

    ASSERT_TRUE(putPermanent("Grizzly Bears", false));
    const auto bear = findPermanent(p1, p1.myId, QStringLiteral("grizzly_bears"));
    ASSERT_TRUE(bear.has_value());
    ruled::v1::RuledCommand putSpell;
    auto *put = putSpell.mutable_dev_command();
    put->set_target_player_id(p1.myId);
    put->mutable_put_card_in_zone()->set_card_name("Unexpected Assistance");
    put->mutable_put_card_in_zone()->set_zone(ruled::v1::DEV_ZONE_HAND);
    ASSERT_TRUE(send(p1, putSpell, QStringLiteral("put Convoke spell")));
    ruled::v1::RuledCommand mana;
    mana.mutable_dev_command()->set_target_player_id(p1.myId);
    mana.mutable_dev_command()->mutable_add_mana()->set_u(2);
    mana.mutable_dev_command()->mutable_add_mana()->set_c(2);
    ASSERT_TRUE(send(p1, mana, QStringLiteral("mixed Convoke mana")));
    const auto *hand = p1.handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Unexpected Assistance"));
    ASSERT_NE(hand, nullptr);
    EXPECT_TRUE(hand->has_convoke());
    ruled::v1::RuledCommand query;
    auto *preview = query.mutable_preview_spell_payment();
    preview->set_transaction_id(145);
    preview->set_revision(1);
    auto *cast = preview->mutable_cast_spell();
    cast->set_cast_method(ruled::v1::CAST_METHOD_NORMAL);
    cast->mutable_source()->set_hand_index(hand->hand_index());
    auto *selection = cast->mutable_payment();
    selection->mutable_mana()->set_u(2);
    selection->mutable_mana()->set_c(2);
    auto *creature = selection->add_convoke();
    creature->mutable_object()->set_object_id(bear->oid);
    creature->mutable_object()->set_zone_change_generation(bear->generation);
    creature->set_kind(ruled::v1::CONVOKE_PAYMENT_KIND_GENERIC);
    const auto before1 = p1.stateVersion;
    const auto before2 = p2.stateVersion;
    const auto legal = p1.latestLegal.SerializeAsString();
    p1.sendRuled(query, QStringLiteral("private Convoke preview"));
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.spellPaymentPreviewCount == 1; }, 10000, "Convoke preview"));
    p2.pump(200);
    ASSERT_TRUE(p1.spellPaymentPreview.valid()) << p1.spellPaymentPreview.error();
    ASSERT_TRUE(p1.spellPaymentPreview.complete());
    EXPECT_EQ(p1.stateVersion, before1);
    EXPECT_EQ(p2.stateVersion, before2);
    EXPECT_EQ(p2.spellPaymentPreviewCount, 0);
    EXPECT_EQ(p1.latestLegal.SerializeAsString(), legal);
    EXPECT_FALSE(findPermanent(p1, p1.myId, QStringLiteral("grizzly_bears"))->tapped);
    EXPECT_FALSE(findPermanent(p2, p1.myId, QStringLiteral("grizzly_bears"))->tapped);
    const auto authoritativeRevision = p1.spellPaymentPreview.selection().expected_state_revision();
    preview->set_revision(2);
    p1.sendRuled(query, QStringLiteral("repeat read-only preview"));
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.spellPaymentPreviewCount == 2; }, 10000, "repeat preview"));
    EXPECT_EQ(p1.spellPaymentPreview.selection().expected_state_revision(), authoritativeRevision);
    ruled::v1::RuledCommand commit;
    *commit.mutable_cast_spell() = *cast;
    *commit.mutable_cast_spell()->mutable_payment() = p1.spellPaymentPreview.selection();
    ASSERT_TRUE(send(p1, commit, QStringLiteral("commit mixed Convoke")));
    EXPECT_TRUE(findPermanent(p1, p1.myId, QStringLiteral("grizzly_bears"))->tapped);
    EXPECT_TRUE(findPermanent(p2, p1.myId, QStringLiteral("grizzly_bears"))->tapped);
    ASSERT_TRUE(p1.serverCardByEngineOid.count(bear->oid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(bear->oid));
    EXPECT_EQ(p1.serverCardByEngineOid[bear->oid], p2.serverCardByEngineOid[bear->oid]);
    EXPECT_TRUE(p1.physicallyTappedCardIds.count(p1.serverCardByEngineOid[bear->oid]));
    EXPECT_TRUE(p2.physicallyTappedCardIds.count(p2.serverCardByEngineOid[bear->oid]));
    EXPECT_EQ(p1.myPool.total(), 0);
}

TEST_F(RuledE2ESmokeTest, SelectableTapAndBlightPaymentsPreservePrivacyAndExactCardsForBothClients)
{
    const auto started = startServers();
    if (!started) {
        FAIL() << started.message();
    }
    if (std::string(started.message()).rfind("SKIP:", 0) == 0) {
        GTEST_SKIP() << std::string(started.message()).substr(5);
    }

    SmokeClient p1(SmokeClient::Role::Aggressor, QStringLiteral("tappaymentp1"), &transcript);
    SmokeClient p2(SmokeClient::Role::Hoarder, QStringLiteral("tappaymentp2"), &transcript);
    p2.didMulligan = true;
    ASSERT_TRUE(p1.loginAndJoinRoom());
    ASSERT_TRUE(p2.loginAndJoinRoom());
    ASSERT_TRUE(p1.createRuledGame());
    ASSERT_TRUE(p2.joinRuledGame(p1.gameId));
    ASSERT_TRUE(p1.selectDeck(deckXml({{40, QStringLiteral("Forest")}})));
    ASSERT_TRUE(p2.selectDeck(deckXml({{40, QStringLiteral("Island")}})));
    p1.sendReady();
    p2.sendReady();
    ASSERT_TRUE(p1.pumpUntil([&] { return p1.gameStarted && p1.stateVersion > 0; }, 20000,
                             "issue 144 game start (p1)"));
    ASSERT_TRUE(p2.pumpUntil([&] { return p2.gameStarted && p2.stateVersion > 0; }, 20000,
                             "issue 144 game start (p2)"));
    ASSERT_TRUE(p1.publishMain1Stops());
    ASSERT_TRUE(p2.publishMain1Stops());

    QElapsedTimer opening;
    opening.start();
    while (opening.elapsed() < 30000) {
        p1.pump(25);
        p2.pump(25);
        if (p1.phase == ruled::v1::PHASE_ID_MAIN1 && p2.phase == ruled::v1::PHASE_ID_MAIN1 &&
            p1.priorityPlayer == p1.myId && p2.priorityPlayer == p1.myId) {
            break;
        }
        p1.act();
        p2.act();
    }
    ASSERT_EQ(p1.phase, ruled::v1::PHASE_ID_MAIN1);
    ASSERT_EQ(p1.priorityPlayer, p1.myId);

    auto send = [&](SmokeClient &sender, const ruled::v1::RuledCommand &command, const QString &description) {
        const quint64 before1 = p1.stateVersion;
        const quint64 before2 = p2.stateVersion;
        sender.sendRuled(command, description);
        QElapsedTimer wait;
        wait.start();
        while ((p1.stateVersion <= before1 || p2.stateVersion <= before2) && wait.elapsed() < 10000) {
            p1.pump(25);
            p2.pump(25);
        }
        return p1.stateVersion > before1 && p2.stateVersion > before2;
    };
    auto putPermanent = [&](const char *name, bool ready) {
        ruled::v1::RuledCommand command;
        auto *dev = command.mutable_dev_command();
        dev->set_target_player_id(p1.myId);
        auto *put = dev->mutable_put_card_in_zone();
        put->set_card_name(name);
        put->set_zone(ruled::v1::DEV_ZONE_BATTLEFIELD);
        put->set_ready(ready);
        return send(p1, command, QStringLiteral("issue 144 put %1").arg(name));
    };
    auto findPermanent = [](const SmokeClient &client, int controller,
                            const QString &cardId) -> std::optional<SmokeClient::Permanent> {
        const auto battlefield = client.battlefieldByPlayer.find(controller);
        if (battlefield == client.battlefieldByPlayer.end()) {
            return std::nullopt;
        }
        const auto permanent = std::find_if(
            battlefield->second.begin(), battlefield->second.end(),
            [&cardId](const SmokeClient::Permanent &candidate) { return candidate.cardId == cardId; });
        return permanent == battlefield->second.end() ? std::nullopt : std::optional(*permanent);
    };

    ASSERT_TRUE(putPermanent("Gene Pollinator", true));
    ASSERT_TRUE(putPermanent("Grizzly Bears", false));
    const auto gene = findPermanent(p1, p1.myId, QStringLiteral("gene_pollinator"));
    const auto bear = findPermanent(p1, p1.myId, QStringLiteral("grizzly_bears"));
    ASSERT_TRUE(gene.has_value());
    ASSERT_TRUE(bear.has_value());
    ASSERT_FALSE(bear->tapped);
    ASSERT_TRUE(bear->sick) << "the separate tap payment must accept a newly controlled permanent";

    const quint64 abilityKey = static_cast<quint64>(gene->oid) << 32;
    const auto costs = p1.latestLegal.cost_choices_by_ability().find(abilityKey);
    ASSERT_NE(costs, p1.latestLegal.cost_choices_by_ability().end());
    const ruled::v1::LegalCostChoice *tapCost = nullptr;
    for (const auto &choice : costs->second.choices()) {
        if (choice.kind() == ruled::v1::COST_CHOICE_KIND_TAP && choice.min() == 1 && choice.max() == 1) {
            tapCost = &choice;
            break;
        }
    }
    ASSERT_NE(tapCost, nullptr);
    ASSERT_EQ(tapCost->candidate_objects_size(), 1);
    EXPECT_EQ(tapCost->candidate_objects(0).object_id(), bear->oid);
    EXPECT_EQ(tapCost->candidate_objects(0).zone_change_generation(), bear->generation);

    ruled::v1::RuledCommand activate;
    auto *ability = activate.mutable_activate_ability();
    p1.setBattlefieldAbilitySource(ability, gene->oid);
    ability->set_ability_index(0);
    ability->set_mana_option_index(0);
    auto *selection = ability->add_cost_selections();
    selection->set_cost_index(tapCost->cost_index());
    auto *selected = selection->mutable_battlefield_objects()->add_objects();
    selected->set_object_id(bear->oid);
    selected->set_zone_change_generation(bear->generation);
    ASSERT_TRUE(send(p1, activate, QStringLiteral("issue 144 activate Gene Pollinator")));

    const auto p1Gene = findPermanent(p1, p1.myId, QStringLiteral("gene_pollinator"));
    const auto p1Bear = findPermanent(p1, p1.myId, QStringLiteral("grizzly_bears"));
    const auto p2Gene = findPermanent(p2, p1.myId, QStringLiteral("gene_pollinator"));
    const auto p2Bear = findPermanent(p2, p1.myId, QStringLiteral("grizzly_bears"));
    ASSERT_TRUE(p1Gene.has_value() && p1Bear.has_value() && p2Gene.has_value() && p2Bear.has_value());
    EXPECT_TRUE(p1Gene->tapped && p1Bear->tapped);
    EXPECT_TRUE(p2Gene->tapped && p2Bear->tapped);
    ASSERT_TRUE(p1.serverCardByEngineOid.count(gene->oid) && p1.serverCardByEngineOid.count(bear->oid));
    ASSERT_TRUE(p2.serverCardByEngineOid.count(gene->oid) && p2.serverCardByEngineOid.count(bear->oid));
    EXPECT_EQ(p1.serverCardByEngineOid[gene->oid], p2.serverCardByEngineOid[gene->oid]);
    EXPECT_EQ(p1.serverCardByEngineOid[bear->oid], p2.serverCardByEngineOid[bear->oid]);
    EXPECT_TRUE(p1.physicallyTappedCardIds.count(p1.serverCardByEngineOid[gene->oid]));
    EXPECT_TRUE(p1.physicallyTappedCardIds.count(p1.serverCardByEngineOid[bear->oid]));
    EXPECT_TRUE(p2.physicallyTappedCardIds.count(p2.serverCardByEngineOid[gene->oid]));
    EXPECT_TRUE(p2.physicallyTappedCardIds.count(p2.serverCardByEngineOid[bear->oid]));
    EXPECT_EQ(p1.myPool.total(), 1);

    // Blight reuses the same physical picker, but accepts the already tapped, summoning-sick bear.
    ASSERT_TRUE(putPermanent("Gristle Glutton", true));
    ASSERT_TRUE(putPermanent("Tatterkite", false));
    const auto glutton = findPermanent(p1, p1.myId, QStringLiteral("gristle_glutton"));
    const auto kite = findPermanent(p1, p1.myId, QStringLiteral("tatterkite"));
    ASSERT_TRUE(glutton && kite);
    const auto blightCosts = p1.latestLegal.cost_choices_by_ability().find(static_cast<quint64>(glutton->oid) << 32);
    ASSERT_NE(blightCosts, p1.latestLegal.cost_choices_by_ability().end());
    ASSERT_EQ(blightCosts->second.choices_size(), 1);
    const auto &blight = blightCosts->second.choices(0);
    EXPECT_EQ(blight.kind(), ruled::v1::COST_CHOICE_KIND_BLIGHT);
    EXPECT_EQ(blight.blight_count(), 1u);
    EXPECT_EQ(blight.min(), 1u);
    EXPECT_EQ(blight.max(), 1u);
    EXPECT_TRUE(std::find(blight.candidate_ids().begin(), blight.candidate_ids().end(), bear->oid) !=
                blight.candidate_ids().end());
    EXPECT_TRUE(std::find(blight.candidate_ids().begin(), blight.candidate_ids().end(), kite->oid) ==
                blight.candidate_ids().end());
    ruled::v1::RuledCommand blightActivation;
    auto *blightAbility = blightActivation.mutable_activate_ability();
    p1.setBattlefieldAbilitySource(blightAbility, glutton->oid);
    blightAbility->set_ability_index(0);
    auto *blightSelection = blightAbility->add_cost_selections();
    blightSelection->set_cost_index(blight.cost_index());
    auto *blighted = blightSelection->mutable_battlefield_objects()->add_objects();
    blighted->set_object_id(bear->oid);
    blighted->set_zone_change_generation(bear->generation);
    ASSERT_TRUE(send(p1, blightActivation, QStringLiteral("Blight with tapped bear")));
    EXPECT_EQ(findPermanent(p1, p1.myId, QStringLiteral("grizzly_bears"))->toughness, 1);
    EXPECT_EQ(findPermanent(p2, p1.myId, QStringLiteral("grizzly_bears"))->toughness, 1);
    auto resolveToChoice = [&]() {
        for (int i = 0; i < 8 && !p1.pendingChoice && !p2.pendingChoice; ++i) {
            ruled::v1::RuledCommand pass;
            pass.mutable_pass_priority();
            if (!send(p1.priorityPlayer == p1.myId ? p1 : p2, pass, QStringLiteral("resolve Blight ability")))
                return false;
        }
        return p1.pendingChoice.has_value() || p2.pendingChoice.has_value();
    };
    ASSERT_TRUE(resolveToChoice());
    ASSERT_TRUE(p1.pendingChoice);
    EXPECT_EQ(p1.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_HAND_CARDS);
    EXPECT_FALSE(p2.pendingChoice);
    ASSERT_TRUE(p2.lastResolutionChoice);
    EXPECT_EQ(p2.lastResolutionChoice->candidate_object_ids_size(), 0);
    EXPECT_EQ(p2.lastResolutionChoice->candidate_names_size(), 0);
    const int handBefore = p1.handSizeByPlayer[p1.myId];
    ruled::v1::RuledCommand discard;
    discard.mutable_submit_resolution_choice()->add_chosen_object_ids(p1.pendingChoice->candidate_object_ids(0));
    p1.pendingChoice.reset();
    ASSERT_TRUE(send(p1, discard, QStringLiteral("Gristle discard then draw")));
    EXPECT_EQ(p1.handSizeByPlayer[p1.myId], handBefore);

    ASSERT_TRUE(putPermanent("Dream Seizer", false));
    ASSERT_TRUE(resolveToChoice());
    ASSERT_TRUE(p1.pendingChoice);
    EXPECT_EQ(p1.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_RESOLUTION_BRANCH);
    ruled::v1::RuledCommand branch;
    branch.mutable_submit_resolution_choice()->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_SELECT_BRANCH);
    branch.mutable_submit_resolution_choice()->set_selected_branch_index(0);
    p1.pendingChoice.reset();
    ASSERT_TRUE(send(p1, branch, QStringLiteral("Dream Seizer choose Blight")));
    ASSERT_TRUE(p1.pendingChoice);
    EXPECT_EQ(p1.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_COST_OBJECTS);
    EXPECT_EQ(p1.pendingChoice->candidate_server_card_ids_size(), p1.pendingChoice->candidate_object_ids_size());
    EXPECT_FALSE(p2.pendingChoice);
    ASSERT_TRUE(p2.lastResolutionChoice);
    EXPECT_EQ(p2.lastResolutionChoice->candidate_object_ids_size(), 0);
    EXPECT_EQ(p2.lastResolutionChoice->candidate_server_card_ids_size(), 0);
    const int physicalBear = p1.serverCardByEngineOid[bear->oid];
    ruled::v1::RuledCommand lethalBlight;
    lethalBlight.mutable_submit_resolution_choice()->add_chosen_object_ids(bear->oid);
    p1.pendingChoice.reset();
    ASSERT_TRUE(send(p1, lethalBlight, QStringLiteral("Dream Seizer lethal Blight")));
    ASSERT_TRUE(p2.pendingChoice);
    EXPECT_EQ(p2.pendingChoice->choice_kind(), ruled::v1::CHOICE_KIND_HAND_CARDS);
    EXPECT_FALSE(p1.pendingChoice);
    ASSERT_TRUE(findPermanent(p1, p1.myId, QStringLiteral("grizzly_bears")));
    ASSERT_TRUE(findPermanent(p2, p1.myId, QStringLiteral("grizzly_bears")));
    EXPECT_EQ(p1.serverCardByEngineOid[bear->oid], physicalBear);
    EXPECT_EQ(p2.serverCardByEngineOid[bear->oid], physicalBear);
    ruled::v1::RuledCommand opponentDiscard;
    opponentDiscard.mutable_submit_resolution_choice()->add_chosen_object_ids(
        p2.pendingChoice->candidate_object_ids(0));
    p2.pendingChoice.reset();
    ASSERT_TRUE(send(p2, opponentDiscard, QStringLiteral("finish Dream Seizer discard")));
    EXPECT_FALSE(findPermanent(p1, p1.myId, QStringLiteral("grizzly_bears")));
    EXPECT_FALSE(findPermanent(p2, p1.myId, QStringLiteral("grizzly_bears")));
    EXPECT_EQ(p1.graveyardOwnerByEngineOid[bear->oid], p1.myId);
    EXPECT_EQ(p2.graveyardOwnerByEngineOid[bear->oid], p1.myId);
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
    ASSERT_TRUE(
        p2.selectDeck(deckXml({{24, QStringLiteral("Island")}, {16, QStringLiteral("Merfolk of the Pearl Trident")}})));
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
