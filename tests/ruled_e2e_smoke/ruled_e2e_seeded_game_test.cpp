#include "ruled_e2e_seeded_driver.h"
#include "ruled_e2e_session.h"
namespace ruled_e2e
{
namespace
{
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

    SeededGameDriver p1(SeededGameDriver::Role::Aggressor, QStringLiteral("smokep1"), &transcript);
    SeededGameDriver p2(SeededGameDriver::Role::Hoarder, QStringLiteral("smokep2"), &transcript);

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
               p1.sawAdventurePermissionGroup && p1.sawAdventureExileToStack && p1.sawAdventureStackToBattlefield &&
               p1.sawEntryReplacementChoice && p1.submittedEntryReplacementChoice && p1.sawDiregrafEnterTapped &&
               p1.sawDamagePreventionChoice && p1.submittedDamagePreventionChoice && p1.sawControlTransfer &&
               p1.sawProtectionBranchChoice && p1.submittedProtectionBranchChoice && p1.sawProtectionHandToStack &&
               !p1.protectionLeftStackBeforeChoice && p1.sawProtectionStackToGraveAfterChoice &&
               p1.sawProtectionPhysicalAnnotation && p2.sawProtectionPhysicalAnnotation && p1.sawControlReturn &&
               p1.sawPhysicalControlTransfer && p1.sawPhysicalControlReturn && p1.sawLibraryPermanentMoved &&
               p2.sawLibraryPermanentMoved && p1.sawLibraryTargetAbsentFromBattlefield &&
               p2.sawLibraryTargetAbsentFromBattlefield && p2.sawTopPermanentDrawn &&
               p1.sawOwnLibrarySearchCandidates && p2.sawOpponentLibrarySearchRedacted &&
               p1.sawActivatedLogPresentation && p2.sawActivatedLogPresentation && p1.sawTriggeredLogPresentation &&
               p2.sawTriggeredLogPresentation && p1.submittedEvolvingWildsChoice && p1.sawEvolvingWildsPermanentMoved &&
               p2.sawEvolvingWildsPermanentMoved && p1.sawEvolvingWildsPhysicalDeckToTable &&
               p2.sawEvolvingWildsPhysicalDeckToTable && p1.sawZoneScopeChoice && p1.submittedZoneScopeChoice &&
               p1.sawOwnZoneSearchCandidates && p2.sawOpponentZoneSearchRedacted && p1.submittedZoneSearchChoice &&
               p1.sawAltanakEnterBattlefield && p2.sawAltanakEnterBattlefield && p1.sayItsNameGraveToExileCount == 3 &&
               p2.sayItsNameGraveToExileCount == 3 && p1.sawOwnSurveilCandidates && p2.sawOpponentSurveilRedacted &&
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
               p1.sawOmenGraveyardDestination && p2.sawOmenGraveyardDestination && p1.sawOmenStackToGraveyard &&
               p2.sawOmenStackToGraveyard;
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
    EXPECT_TRUE(p1.omenFizzleCast && p1.omenFizzleBoltCast) << "the targeted Omen fizzle setup was not completed";
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

} // namespace
} // namespace ruled_e2e
