add_executable(
  modal_cast_menu_test modal_cast_menu_test.cpp
  ${CMAKE_SOURCE_DIR}/cockatrice/src/game/ruled/ruled_pending_cast.cpp
)

target_include_directories(modal_cast_menu_test PRIVATE ${CMAKE_SOURCE_DIR}/cockatrice/src)

target_link_libraries(
  modal_cast_menu_test
  PRIVATE libcockatrice_protocol
  PRIVATE Threads::Threads
  PRIVATE ${GTEST_BOTH_LIBRARIES}
  PRIVATE ${TEST_QT_MODULES}
)

add_test(NAME modal_cast_menu_test COMMAND modal_cast_menu_test)

if(NOT GTEST_FOUND)
  add_dependencies(modal_cast_menu_test gtest)
endif()
