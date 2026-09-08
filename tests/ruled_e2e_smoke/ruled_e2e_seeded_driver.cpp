#include "ruled_e2e_seeded_driver.h"
namespace ruled_e2e
{
void SeededGameDriver::onPhysicalEvent(const GameEvent &ev)
{
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
        const int omenOwnerId = role == Role::Aggressor ? myId : oppId;

        if (from == stack && to == deck && mc.target_player_id() == omenOwnerId && omenSuccessPhysicalCardId >= 0 &&
            mc.card_id() == omenSuccessPhysicalCardId) {
            omenSuccessPhysicalIdentityContinuous = true;
            omenSuccessPhysicalCardId = mc.new_card_id();
            sawOmenStackToLibrary = true;
        }
        // A failed Omen's graveyard move is public, but its Event_MoveCard name can be
        // empty after the stack annotation is cleared. Follow the already-captured
        // physical card id instead of depending on presentation text.
        if (from == stack && to == grave && mc.target_player_id() == omenOwnerId && omenFizzlePhysicalCardId >= 0 &&
            mc.card_id() == omenFizzlePhysicalCardId) {
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
            renewPhysicalIdentityContinuous = renewSourcePhysicalId >= 0 && mc.card_id() == renewSourcePhysicalId;
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
                protectionLeftStackBeforeChoice = protectionLeftStackBeforeChoice || !submittedProtectionBranchChoice;
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
                omenSuccessPhysicalIdentityContinuous =
                    omenSuccessPhysicalCardId >= 0 && mc.card_id() == omenSuccessPhysicalCardId;
                omenSuccessPhysicalCardId = mc.new_card_id();
                sawOmenStackToLibrary = true;
            } else if (from == stack && to == grave && mc.target_player_id() == omenOwnerId) {
                omenFizzlePhysicalIdentityContinuous =
                    omenFizzlePhysicalCardId >= 0 && mc.card_id() == omenFizzlePhysicalCardId;
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
        } else if (flashbackCast && (from == grave || to == exile) && name != QLatin1String("Sagu Pummeler") &&
                   !(name == QLatin1String("Grizzly Bears") &&
                     (submittedAggressiveChoice || sawAggressivePublicReveal))) {
            // Any *other* card taking the flashback path is the wrong-card bug.
            ADD_FAILURE() << "unexpected card on the flashback path: " << name.toStdString() << " "
                          << from.toStdString() << " -> " << to.toStdString();
        }
    }
    if (ev.HasExtension(Event_SetCardAttr::ext)) {
        const auto &attr = ev.GetExtension(Event_SetCardAttr::ext);

        if (attr.attribute() == AttrTapped) {
            sawPhysicalTap = sawPhysicalTap || attr.attr_value() == "1";
            sawPhysicalUntap = sawPhysicalUntap || attr.attr_value() == "0";

        } else if (attr.attribute() == AttrAnnotation) {
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
}

void SeededGameDriver::onPaymentPreview(const ruled::v1::RuledEventBatch &batch)
{
    OpeningDriver::onPaymentPreview(batch);
    if (softCounterPaymentPreviewPending && paymentPreview.transaction_id() == 8801) {
        softCounterPaymentPreviewPending = false;
        EXPECT_TRUE(paymentPreview.valid()) << paymentPreview.error();
        EXPECT_TRUE(paymentPreview.complete());
        ruled::v1::RuledCommand cmd;
        auto *choice = cmd.mutable_submit_resolution_choice();
        choice->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_PAY_MANA);
        *choice->mutable_payment() = paymentPreview.selection();
        pendingChoice.reset();
        paidSoftCounter = true;
        sendRuled(cmd, QStringLiteral("pay Convolute's exact resolution cost"));
    }
    if (permanentActionPaymentCommit &&
        (paymentPreview.transaction_id() == 9801 || paymentPreview.transaction_id() == 9802)) {
        EXPECT_TRUE(paymentPreview.valid()) << paymentPreview.error();
        EXPECT_TRUE(paymentPreview.complete());
        auto command = std::move(*permanentActionPaymentCommit);
        permanentActionPaymentCommit.reset();
        auto *action = command.mutable_execute_permanent_action();
        *action->mutable_payment() = paymentPreview.selection();
        action->clear_restricted_mana();
        for (const auto &selection : paymentPreview.restricted_mana())
            *action->add_restricted_mana() = selection;
        sendRuled(command, permanentActionPaymentLabel);
    }
}

void SeededGameDriver::onBatchBegin(const ruled::v1::RuledEventBatch &)
{
    previousPhase = phase;
    phaseEvents = 0;
    batchDeclaredAttackers = false;
    batchCombatDamage = false;
    batchHasPublicReveal = false;
    batchIsPreview = false;
}

void SeededGameDriver::onRuledEvent(const ruled::v1::RuledEvent &ev)
{
    OpeningDriver::onRuledEvent(ev);
    batchIsPreview = batchIsPreview || ev.has_attackers_preview() || ev.has_blockers_preview();
    if (ev.has_phase_changed()) {
        ++phaseEvents;
        if (phase == ruled::v1::PHASE_ID_UNTAP || phase == ruled::v1::PHASE_ID_DECLARE_ATTACKERS) {
            attackersSentThisCombat = false;
            blockersSentThisCombat = false;
        }
        inCombatDamageWindow =
            phase == ruled::v1::PHASE_ID_COMBAT_DAMAGE || phase == ruled::v1::PHASE_ID_FIRST_STRIKE_DAMAGE;
    } else if (ev.has_stack_pushed()) {
        const auto &sp = ev.stack_pushed();
        const QString cardId = QString::fromStdString(sp.card_id());
        if (cardId == QLatin1String("lightning_bolt") && sp.targets_size() > 0) {
            sawBoltPushWithTarget = true;
            boltOid = sp.object_id();
            latestBoltOid = sp.object_id();
        }
        if (cardId == QLatin1String("boros_charm") && sp.targets_size() == 1 && sp.chosen_mode_indices_size() == 1 &&
            sp.chosen_mode_indices(0) == 0 && sp.chosen_mode_labels_size() == 1) {
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

        if (cardId == QLatin1String("dirgur_island_dragon_skimming_strike")) {
            if (omenSuccessOid == 0) {
                omenSuccessOid = sp.object_id();
            } else if (sp.object_id() != omenSuccessOid && omenFizzleOid == 0) {
                omenFizzleOid = sp.object_id();
            }
            sawOmenStackAnnotation = sawOmenStackAnnotation || (sp.description() == "Skimming Strike" &&
                                                                sp.ability_annotation() == "Skimming Strike");
        }
        if (sp.is_triggered() && sp.has_primary_presentation() &&
            sp.primary_presentation().card_id() == "derelict_attic_widows_walk" &&
            sp.primary_presentation().face_id() == "derelict_attic" && sp.primary_presentation().path_size() > 0 &&
            sp.primary_presentation().path(sp.primary_presentation().path_size() - 1).id() == "triggered_01") {
            sawRoomUnlockTrigger = true;
        }
    } else if (ev.has_stack_resolved()) {
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

    }

    else if (ev.has_life_changed()) {
        const auto &lc = ev.life_changed();

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
            if (sawEvolvingWildsPhysicalDeckToTable && evolvingWildsPhysicalCardId >= 0 &&
                entry.server_card_id() == evolvingWildsPhysicalCardId) {
                if (evolvingWildsChosenOid != 0 && evolvingWildsChosenOid != entry.engine_object_id()) {
                    evolvingWildsPhysicalIdentityContinuous = false;
                }
                evolvingWildsChosenOid = entry.engine_object_id();
            }
        }
    }

    else if (ev.has_face_down_object_map()) {
        for (const auto &entry : ev.face_down_object_map().entries()) {
            if (entry.controller_player_id() == myId && entry.card_name() == "Hill Giant") {
                manifestOid = entry.engine_object_id();
                manifestGeneration = entry.zone_change_generation();
                manifestServerCardId = entry.server_card_id();
                sawManifestPrivateIdentity = true;
            }
        }
        if (role == Role::Hoarder && sawManifestChoiceRedacted && ev.face_down_object_map().entries_size() == 0) {
            sawOpponentManifestIdentityEmpty = true;
        }
    } else if (ev.has_resolution_choice_required()) {
        const auto &rcr = ev.resolution_choice_required();

        if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_SEARCH) {
            if (rcr.deciding_player_id() == myId) {
                if (emptyTypecyclingActivated && rcr.candidate_object_ids_size() == 0) {
                    sawEmptyTypecyclingChoice = rcr.min() == 0 && rcr.max() == 1 &&
                                                rcr.candidate_card_ids_size() == 0 && rcr.candidate_names_size() == 0 &&
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
        if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_ZONE_SEARCH) {
            if (rcr.deciding_player_id() == myId) {
                bool hasAltanak = false;
                for (int i = 0; i < rcr.candidate_names_size(); ++i) {
                    hasAltanak = hasAltanak || rcr.candidate_names(i) == "Altanak, the Thrice-Called";
                }
                sawOwnZoneSearchCandidates = hasAltanak &&
                                             rcr.candidate_object_ids_size() == rcr.candidate_card_ids_size() &&
                                             rcr.candidate_object_ids_size() == rcr.candidate_names_size() &&
                                             rcr.candidate_object_ids_size() == rcr.candidate_server_card_ids_size() &&
                                             rcr.candidate_object_ids_size() == rcr.candidate_source_zones_size();
            } else {
                sawOpponentZoneSearchRedacted = rcr.candidate_object_ids_size() == 0 &&
                                                rcr.candidate_card_ids_size() == 0 && rcr.candidate_names_size() == 0 &&
                                                rcr.candidate_server_card_ids_size() == 0 &&
                                                rcr.candidate_source_zones_size() == 0 &&
                                                rcr.prompt_text() == "Opponent is making a resolution choice.";
            }
        }
        if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_MANIFEST_DREAD) {
            if (rcr.deciding_player_id() == myId) {
                sawManifestChoicePrivate = rcr.candidate_object_ids_size() == 2 && rcr.candidate_names_size() == 2 &&
                                           rcr.candidate_server_card_ids_size() == 2;
            } else {
                sawManifestChoiceRedacted = rcr.candidate_object_ids_size() == 0 &&
                                            rcr.candidate_card_ids_size() == 0 && rcr.candidate_names_size() == 0 &&
                                            rcr.candidate_server_card_ids_size() == 0;
            }
        }
        if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_LIBRARY_LOOK) {
            if (rcr.deciding_player_id() == myId) {
                sawOwnSurveilCandidates = rcr.candidate_object_ids_size() == 2 && rcr.candidate_card_ids_size() == 2 &&
                                          rcr.candidate_names_size() == 2 &&
                                          rcr.candidate_server_card_ids_size() == 2 && rcr.min() == 0 &&
                                          rcr.max() == 2 && rcr.ordered();
            } else {
                sawOpponentSurveilRedacted = rcr.candidate_object_ids_size() == 0 &&
                                             rcr.candidate_card_ids_size() == 0 && rcr.candidate_names_size() == 0 &&
                                             rcr.candidate_server_card_ids_size() == 0;
            }
        }
        if (rcr.choice_kind() == ruled::v1::CHOICE_KIND_OPPONENT_HAND) {
            const bool publicReveal = rcr.has_public_reveal();
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
                sawAggressiveChooserMask =
                    publicReveal && rcr.has_public_reveal() && rcr.public_reveal().zone_owner_player_id() == oppId &&
                    rcr.candidate_object_ids_size() == rcr.candidate_names_size() &&
                    rcr.candidate_card_ids_size() == rcr.candidate_names_size() &&
                    rcr.candidate_names_size() == rcr.candidate_server_card_ids_size() &&
                    rcr.candidate_names_size() == rcr.candidate_selectable_size() && hasEligibleBear && hasLand;
            } else {
                bool hasBear = false;
                bool hasLand = false;
                for (const auto &name : rcr.candidate_names()) {
                    hasBear = hasBear || name == "Grizzly Bears";
                    hasLand = hasLand || name == "Island";
                }
                sawAggressiveObserverReadOnly = publicReveal && rcr.has_public_reveal() &&
                                                rcr.public_reveal().zone_owner_player_id() == myId &&
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

    }

    else if (ev.has_permanent_moved()) {
        const auto &moved = ev.permanent_moved();

        if (moved.destination() == ruled::v1::PermanentMoved::DESTINATION_EXILE && moved.card_id() == "grizzly_bears" &&
            (submittedAggressiveChoice || sawAggressivePublicReveal)) {
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
        if (moved.card_id() == "say_its_name" && moved.destination() == ruled::v1::PermanentMoved::DESTINATION_EXILE) {
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

            if (!ev.zone_view().battlefields_unchanged()) {
                for (const auto &perm : battlefieldByPlayer.at(pp.player_id())) {
                    if (perm.roomDoorCount == 2) {
                        roomOid = perm.oid;
                        roomGeneration = perm.generation;
                        sawRoomCastDoorState = sawRoomCastDoorState || (!perm.roomDoors[0] && perm.roomDoors[1]);
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
        }
    } else if (ev.has_mana_pool_updated()) {
        sawRestrictedBlueMana =
            sawRestrictedBlueMana || restrictedBlueByPlayer.at(ev.mana_pool_updated().player_id()) > 0;
    } else if (ev.has_log()) {
        if (ev.log().has_ability_presentation()) {
            const auto &presentation = ev.log().ability_presentation();
            if (presentation.has_ability() && presentation.ability().oracle_line_indices_size() > 0) {
                sawActivatedLogPresentation =
                    sawActivatedLogPresentation || presentation.ability().card_id() == "evolving_wilds";
                sawTriggeredLogPresentation =
                    sawTriggeredLogPresentation || presentation.prefix().starts_with("Triggered: ");
            }
        }
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
    }
}

void SeededGameDriver::onBatchEventsComplete(const ruled::v1::RuledEventBatch &batch)
{
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
    OpeningDriver::onBatchEventsComplete(batch);
}

void SeededGameDriver::onLegalActions(const ruled::v1::RuledEventBatch &)
{
    for (const auto &group : latestLegal.exile_play_permission_groups()) {
        if (QString::fromStdString(group.source_label()).contains(QLatin1String("Bonecrusher Giant")) &&
            group.object_ids_size() == 1) {
            sawAdventurePermissionGroup = true;
        }
    }

    if (submittedTypecyclingChoice) {
        const auto plains =
            std::find_if(latestLegal.hand_actions().begin(), latestLegal.hand_actions().end(), [](const auto &action) {
                return QString::fromStdString(action.card_name()) == QLatin1String("Plains");
            });
        if (plains != latestLegal.hand_actions().end()) {
            const auto physical = handServerCardBySlot.find(static_cast<int>(plains->hand_index()));
            sawTypecyclingDeckToHand = physical != handServerCardBySlot.end();
            typecyclingPhysicalIdentityContinuous = typecyclingPhysicalIdentityContinuous && sawTypecyclingDeckToHand &&
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
        sawOwnTypecyclingAction = sawOwnTypecyclingAction || hasZoneAbility(QStringLiteral("Shepherding Spirits"),
                                                                            ruled::v1::ABILITY_SOURCE_ZONE_HAND);
        sawOwnRenewAction = sawOwnRenewAction ||
                            hasZoneAbility(QStringLiteral("Sagu Pummeler"), ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD);
    } else {
        sawOpponentTypecyclingActionRedacted =
            sawOpponentTypecyclingActionRedacted ||
            !hasZoneAbility(QStringLiteral("Shepherding Spirits"), ruled::v1::ABILITY_SOURCE_ZONE_HAND);
        sawOpponentRenewActionRedacted =
            sawOpponentRenewActionRedacted ||
            !hasZoneAbility(QStringLiteral("Sagu Pummeler"), ruled::v1::ABILITY_SOURCE_ZONE_GRAVEYARD);
    }
    if (role == Role::Hoarder && sawLibraryPermanentMoved &&
        std::any_of(latestLegal.hand_actions().begin(), latestLegal.hand_actions().end(), [](const auto &action) {
            return QString::fromStdString(action.card_name()) == QLatin1String("Grizzly Bears");
        })) {
        sawTopPermanentDrawn = true;
    }
}

bool SeededGameDriver::hasCursePhysicalAnnotation() const
{
    const auto card = serverCardByEngineOid.find(curseOid);
    if (curseOid == 0 || card == serverCardByEngineOid.end()) {
        return false;
    }
    const auto annotation = annotationByServerCardId.find(card->second);
    return annotation != annotationByServerCardId.end() &&
           annotation->second.contains(QStringLiteral("Enchanting: smokep2"));
}

bool SeededGameDriver::hasRoomPhysicalAnnotation() const
{
    return sawRoomPhysicalAnnotation;
}

bool SeededGameDriver::prepareRestrictedSpecialActionMana(int paymentIndex)
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

std::optional<quint32> SeededGameDriver::firstOwnUntapped(const QString &cardId) const
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

bool SeededGameDriver::tryFlashbackSequence()
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
            cast->mutable_source()->set_expected_zone_change_generation(ga.zone_change_generation());
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

bool SeededGameDriver::tryAdventureSequence()
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
            cast->mutable_source()->set_expected_zone_change_generation(action.zone_change_generation());
            cast->set_face_index(action.face_index());
            cast->set_cast_method(action.cast_method());
            if (action.has_casting_permission_id()) {
                cast->set_casting_permission_id(action.casting_permission_id());
            }
            giantCastFromExile = true;
            sendRuled(cmd, QStringLiteral("cast Bonecrusher Giant from exile oid %1").arg(action.object_id()));
            return true;
        }
    }
    return false;
}

bool SeededGameDriver::tryOmenSequence()
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
        const auto *normal = handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Dirgur Island Dragon"));
        const auto *omen = handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Skimming Strike"));
        if (normal && omen) {
            sawOmenFaceActions = normal->face_index() == 0u && normal->cost() == "{5}{U}" && omen->face_index() == 1u &&
                                 omen->cost() == "{1}{U}" && omen->needs_target();
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
        if (const auto *omen = handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Skimming Strike"))) {
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

void SeededGameDriver::act()
{
    if (myId < 0 || !gameStarted || stateVersion == 0 || lastActedVersion == stateVersion) {
        return;
    }

    if (actOpening())
        return;

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
            const bool isZoneScope = rcr.resolution_branches_size() == 7 &&
                                     std::all_of(rcr.resolution_branches().begin(), rcr.resolution_branches().end(),
                                                 [](const auto &branch) { return branch.search_zones_size() > 0; });
            if (isZoneScope) {
                sawZoneScopeChoice = true;
                const auto allZones = std::find_if(rcr.resolution_branches().begin(), rcr.resolution_branches().end(),
                                                   [](const auto &branch) { return branch.search_zones_size() == 3; });
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
            auto *preview = cmd.mutable_preview_payment();
            preview->set_transaction_id(8801);
            preview->set_revision(1);
            auto *choice = preview->mutable_resolution_choice();
            choice->set_decision(ruled::v1::RESOLUTION_CHOICE_DECISION_PAY_MANA);
            choice->mutable_payment()->mutable_mana()->set_u(4);
            softCounterPaymentPreviewPending = true;
            sendRuled(cmd, QStringLiteral("preview Convolute's exact resolution cost"));
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
                const auto permanent =
                    std::find_if(it->second.cbegin(), it->second.cend(), [&assignment](const Permanent &candidate) {
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

    // Remove the targeted Omen's only target while that Omen is still on the stack. This
    // response sits outside the stack-empty main-phase script by design.
    if (role == Role::Aggressor && omenFizzleCast && !omenFizzleBoltCast && stackDepth == 1 && priorityPlayer == myId) {
        if (const auto *bolt = handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Lightning Bolt"))) {
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
            if (const auto *spell = handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Manifest Dread"))) {
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
                    ruled::v1::RuledCommand commit;
                    auto *turn = commit.mutable_execute_permanent_action();
                    turn->set_kind(ruled::v1::PERMANENT_ACTION_KIND_TURN_FACE_UP);
                    turn->set_object_id(action.object_id());
                    turn->set_expected_zone_change_generation(action.zone_change_generation());
                    auto *payment = turn->add_restricted_mana();
                    payment->set_restriction_group_id(action.eligible_restricted_mana_group_ids(0));
                    payment->set_u(1);
                    ruled::v1::RuledCommand cmd;
                    auto *preview = cmd.mutable_preview_payment();
                    preview->set_transaction_id(9801);
                    preview->set_revision(1);
                    *preview->mutable_execute_permanent_action() = *turn;
                    auto *mana = preview->mutable_execute_permanent_action()->mutable_payment()->mutable_mana();
                    mana->set_w(myPool.w);
                    mana->set_u(myPool.u);
                    mana->set_b(myPool.b);
                    mana->set_r(myPool.r);
                    mana->set_g(myPool.g);
                    mana->set_c(myPool.c);
                    permanentActionPaymentCommit = commit;
                    permanentActionPaymentLabel = QStringLiteral("turn manifested Hill Giant face up");
                    ++specialActionRestrictedPayments;
                    turnManifestFaceUpSent = true;
                    sendRuled(cmd, QStringLiteral("preview turning manifested Hill Giant face up"));
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
                    ruled::v1::RuledCommand commit;
                    auto *unlock = commit.mutable_execute_permanent_action();
                    unlock->set_kind(action.kind());
                    unlock->set_object_id(action.object_id());
                    unlock->set_expected_zone_change_generation(action.zone_change_generation());
                    unlock->set_face_index(action.face_index());
                    auto *payment = unlock->add_restricted_mana();
                    payment->set_restriction_group_id(action.eligible_restricted_mana_group_ids(0));
                    payment->set_u(1);
                    ruled::v1::RuledCommand cmd;
                    auto *preview = cmd.mutable_preview_payment();
                    preview->set_transaction_id(9802);
                    preview->set_revision(1);
                    *preview->mutable_execute_permanent_action() = *unlock;
                    auto *mana = preview->mutable_execute_permanent_action()->mutable_payment()->mutable_mana();
                    mana->set_w(myPool.w);
                    mana->set_u(myPool.u);
                    mana->set_b(myPool.b);
                    mana->set_r(myPool.r);
                    mana->set_g(myPool.g);
                    mana->set_c(myPool.c);
                    permanentActionPaymentCommit = commit;
                    permanentActionPaymentLabel = QStringLiteral("unlock Derelict Attic door");
                    ++specialActionRestrictedPayments;
                    roomUnlockSent = true;
                    sendRuled(cmd, QStringLiteral("preview unlocking Derelict Attic door"));
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
                    cmd, QStringLiteral("cast Aggressive Negotiations targeting opponent and oid %1").arg(manifestOid));
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
                          QStringLiteral("dev: move Say Its Name %1 to graveyard").arg(sayItsNameMovedToGraveyard));
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
                    if (cost.zone() == ruled::v1::COST_CHOICE_ZONE_GRAVEYARD && cost.min() == 2 && cost.max() == 2 &&
                        cost.candidate_ids_size() >= 2) {
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
                ASSERT_GE(graveyardCost->candidate_objects_size(), 2);
                *selection->mutable_graveyard_objects()->add_objects() = graveyardCost->candidate_objects(0).object();
                *selection->mutable_graveyard_objects()->add_objects() = graveyardCost->candidate_objects(1).object();
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
                if (const auto *spell = handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Cruel Truths"))) {
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
        if (const auto *bolt =
                !boltCast ? handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Lightning Bolt")) : nullptr) {
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
        if (const auto *brainstorm = !brainstormCast
                                         ? handAction(ruled::v1::HAND_ACTION_CAST_SPELL, QStringLiteral("Brainstorm"))
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
} // namespace ruled_e2e
