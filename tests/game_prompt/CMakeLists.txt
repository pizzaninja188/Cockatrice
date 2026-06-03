add_executable(
  game_prompt_widget_test game_prompt_widget_test.cpp
  ${CMAKE_SOURCE_DIR}/cockatrice/src/game/prompt/game_prompt_widget.cpp
)

target_include_directories(game_prompt_widget_test PRIVATE ${CMAKE_SOURCE_DIR}/cockatrice/src
                                                           ${CMAKE_SOURCE_DIR}/cockatrice/src/game/prompt)

target_link_libraries(
  game_prompt_widget_test
  PRIVATE Threads::Threads
  PRIVATE ${GTEST_BOTH_LIBRARIES}
  PRIVATE ${TEST_QT_MODULES}
)

add_test(NAME game_prompt_widget_test COMMAND game_prompt_widget_test)

if(NOT GTEST_FOUND)
  add_dependencies(game_prompt_widget_test gtest)
endif()
