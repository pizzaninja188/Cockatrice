add_executable(
  game_prompt_widget_test game_prompt_widget_test.cpp game_prompt_priority_test.cpp ${VERSION_STRING_CPP}
  ${CMAKE_SOURCE_DIR}/cockatrice/src/game/prompt/game_prompt_widget.cpp
  ${CMAKE_SOURCE_DIR}/cockatrice/src/game/ruled/ruled_large_choice_picker.cpp
  ${CMAKE_SOURCE_DIR}/cockatrice/src/game/ruled/ruled_resolution_choice_dialog.cpp
  ${CMAKE_SOURCE_DIR}/cockatrice/src/game/ruled/ruled_counter_quantity_dialog.cpp
)

target_include_directories(game_prompt_widget_test PRIVATE ${CMAKE_SOURCE_DIR}/cockatrice/src
                                                           ${CMAKE_SOURCE_DIR}/cockatrice/src/game/prompt)

target_link_libraries(
  game_prompt_widget_test
  PRIVATE Threads::Threads
  PRIVATE libcockatrice_game
  PRIVATE ${GTEST_BOTH_LIBRARIES}
  PRIVATE ${TEST_QT_MODULES}
)

add_test(NAME game_prompt_widget_test COMMAND game_prompt_widget_test)

if(NOT GTEST_FOUND)
  add_dependencies(game_prompt_widget_test gtest)
endif()
