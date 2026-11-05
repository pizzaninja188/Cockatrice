#include "game/ruled_deck_validation.h"

#include <gtest/gtest.h>
#include <libcockatrice/deck_list/deck_list.h>

TEST(RuledDeckValidation, NormalizesTheExactOrderedOneCopySideboardPlan)
{
    DeckList deck(QStringLiteral("<cockatrice_deck version=\"1\"><commanders><commander providerId=\"print\">Kami of "
                                 "the Crescent Moon</commander></commanders>"
                                 "<zone name=\"main\"><card number=\"60\" name=\"Forest\"/></zone>"
                                 "<zone name=\"side\"><card number=\"1\" name=\"Island\"/></zone></cockatrice_deck>"));
    MoveCard_ToZone move;
    move.set_start_zone("main");
    move.set_target_zone("side");
    move.set_card_name("Forest");
    deck.setCurrentSideboardPlan({move});
    ruled::v1::PlayerDeck row;
    QString error;
    ASSERT_TRUE(RuledDeckValidation::normalize(deck, 7, row, error)) << error.toStdString();
    EXPECT_EQ(row.player_id(), 7);
    EXPECT_EQ(row.mainboard_card_name_size(), 59);
    ASSERT_EQ(row.sideboard_card_name_size(), 2);
    EXPECT_EQ(row.sideboard_card_name(0), "Island");
    EXPECT_EQ(row.sideboard_card_name(1), "Forest");
    EXPECT_EQ(row.commander_setup().declared().card_name(0), "Kami of the Crescent Moon");
    // Normalization is read-only and does not mix a commander into the library.
    EXPECT_EQ(deck.getCommanders().size(), 1);
    move.set_start_zone("side");
    move.set_target_zone("main");
    move.set_card_name("Island");
    deck.setCurrentSideboardPlan({move});
    ASSERT_TRUE(RuledDeckValidation::normalize(deck, 7, row, error));
    EXPECT_EQ(row.mainboard_card_name_size(), 61);
    EXPECT_EQ(row.mainboard_card_name(60), "Island");
}

TEST(RuledDeckValidation, RefusesInvalidMovesAndUnboundedQuantities)
{
    DeckList deck(QStringLiteral("<cockatrice_deck version=\"1\"><zone name=\"main\"><card number=\"60\" "
                                 "name=\"Forest\"/></zone></cockatrice_deck>"));
    ruled::v1::PlayerDeck row;
    QString error;
    MoveCard_ToZone move;
    move.set_start_zone("main");
    move.set_target_zone("command");
    move.set_card_name("Forest");
    deck.setCurrentSideboardPlan({move});
    EXPECT_FALSE(RuledDeckValidation::normalize(deck, 0, row, error));
    move.set_target_zone("side");
    move.set_card_name("Absent");
    deck.setCurrentSideboardPlan({move});
    EXPECT_FALSE(RuledDeckValidation::normalize(deck, 0, row, error));
    DeckList oversized(QStringLiteral("<cockatrice_deck version=\"1\"><zone name=\"main\"><card number=\"10001\" "
                                      "name=\"Forest\"/></zone></cockatrice_deck>"));
    EXPECT_FALSE(RuledDeckValidation::normalize(oversized, 0, row, error));
}
