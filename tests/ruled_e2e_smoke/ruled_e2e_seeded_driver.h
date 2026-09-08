#ifndef RULED_E2E_SEEDED_DRIVER_H
#define RULED_E2E_SEEDED_DRIVER_H
#include "ruled_e2e_opening_driver.h"
namespace ruled_e2e
{
class SeededGameDriver : public OpeningDriver
{
public:
    enum class Role
    {
        Aggressor, // smokep1: mono-red; starts, bolts, attacks
        Hoarder,   // smokep2: mono-blue; mulligans, brainstorms, discards
    };

    SeededGameDriver(Role role, QString userName, QStringList *transcript)
        : OpeningDriver(role == Role::Aggressor, std::move(userName), transcript, role == Role::Hoarder), role(role)
    {
    }

    Role role;
    void onPhysicalEvent(const GameEvent &ev) override;
    void onPaymentPreview(const ruled::v1::RuledEventBatch &batch) override;
    ruled::v1::PhaseId previousPhase = ruled::v1::PHASE_ID_UNSPECIFIED;
    int phaseEvents = 0;
    bool batchDeclaredAttackers = false;
    bool batchCombatDamage = false;
    bool batchHasPublicReveal = false;
    bool batchIsPreview = false;
    void onBatchBegin(const ruled::v1::RuledEventBatch &) override;
    void onRuledEvent(const ruled::v1::RuledEvent &ev) override;
    void onBatchEventsComplete(const ruled::v1::RuledEventBatch &batch) override;
    void onLegalActions(const ruled::v1::RuledEventBatch &) override;

    bool softCounterPaymentPreviewPending = false;
    std::optional<ruled::v1::RuledCommand> permanentActionPaymentCommit;
    QString permanentActionPaymentLabel;

    bool sawDirectOpeningToMain1 = false;
    int directSettledActivePlayer = -1;

    bool sawRestrictedBlueMana = false;
    int specialActionManaSteps = 0;
    int specialActionRestrictedPayments = 0;
    // CR 603.3b: the engine blocks on this until it is answered, so the bot must handle it or the
    // whole game deadlocks — every simultaneous multi-trigger board reaches it.

    // Policy progress flags
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
    bool playerSetDiscardFlowActive = false;
    bool sawPlayerSetDiscardPrivateCandidates = false;
    bool sawPlayerSetDiscardObserverRedaction = false;
    quint32 playerSetDiscardChosenOid = 0;
    int playerSetDiscardChosenServerCardId = -1;
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
    bool sawActivatedLogPresentation = false;
    bool sawTriggeredLogPresentation = false;
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

    bool hasCursePhysicalAnnotation() const;

    bool hasRoomPhysicalAnnotation() const;

    // Produce a real, separately tracked Peeper contribution for each special-action payment.
    bool prepareRestrictedSpecialActionMana(int paymentIndex);

    std::optional<quint32> firstOwnUntapped(const QString &cardId) const;

    /// Conjure Bump in the Night, bury it, and cast it from the graveyard for its flashback cost.
    /// Returns true when it sent a command (the caller should yield).
    ///
    /// Run by BOTH seats on purpose. Every spell is routed to the single canonical stack zone,
    /// which belongs to the *lowest player id*, so only the other seat's cast is a cross-player
    /// move — the case Server_AbstractPlayer::moveCard rejects unless ruledAllowsCrossPlayerMove
    /// whitelists it. Which client holds the low id depends on join order, so pinning the flashback
    /// to one role silently tests the easy half; that is exactly how a broken grave -> stack move
    /// shipped green.
    bool tryFlashbackSequence();

    bool tryAdventureSequence();

    bool tryOmenSequence();

    // Sends at most one command per observed game version.
    void act();
};
} // namespace ruled_e2e
#endif
