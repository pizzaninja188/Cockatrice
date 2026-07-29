# NOTE: Qt modules for tests are defined centrally in cmake/FindQtRuntime.cmake (the _TEST_NEEDED variable).
# If a new test needs additional Qt modules, add them there — not in individual test CMakeLists.txt files.
enable_testing()

add_test(NAME dummy_test COMMAND dummy_test)
add_test(NAME expression_test COMMAND expression_test)
add_test(NAME test_age_formatting COMMAND test_age_formatting)
add_test(NAME password_hash_test COMMAND password_hash_test)
add_test(NAME ruled_utils_test COMMAND ruled_utils_test)

add_test(NAME deck_hash_performance_test COMMAND deck_hash_performance_test)
set_tests_properties(deck_hash_performance_test PROPERTIES TIMEOUT 5)

# Find GTest

add_executable(dummy_test dummy_test.cpp)
add_executable(expression_test expression_test.cpp)
add_executable(test_age_formatting test_age_formatting.cpp)
add_executable(password_hash_test password_hash_test.cpp)
add_executable(deck_hash_performance_test deck_hash_performance_test.cpp)
add_executable(ruled_utils_test ruled_utils_test.cpp)

find_package(GTest)

if(NOT GTEST_FOUND)
  if(NOT EXISTS "${CMAKE_BINARY_DIR}/gtest-build")
    message(STATUS "Downloading googletest")
    configure_file(
      "${CMAKE_SOURCE_DIR}/cmake/gtest-CMakeLists.txt.in" "${CMAKE_BINARY_DIR}/gtest-download/CMakeLists.txt"
    )
    execute_process(
      COMMAND ${CMAKE_COMMAND} -G "${CMAKE_GENERATOR}" . WORKING_DIRECTORY ${CMAKE_BINARY_DIR}/gtest-download
    )
    execute_process(COMMAND ${CMAKE_COMMAND} --build . WORKING_DIRECTORY ${CMAKE_BINARY_DIR}/gtest-download)
  else()
    message(STATUS "GoogleTest directory exists")
  endif()

  # Add gtest directly to our build
  add_subdirectory(${CMAKE_BINARY_DIR}/gtest-src ${CMAKE_BINARY_DIR}/gtest-build EXCLUDE_FROM_ALL)

  # Add the gtest include directory, since gtest
  # doesn't add that dependency to its gtest target
  target_include_directories(gtest INTERFACE "$<BUILD_INTERFACE:${CMAKE_BINARY_DIR}/gtest-src/include>")

  set(GTEST_INCLUDE_DIRS "${CMAKE_BINARY_DIR}/gtest-src/include")
  set(GTEST_BOTH_LIBRARIES gtest)
  add_dependencies(dummy_test gtest)
  add_dependencies(expression_test gtest)
  add_dependencies(test_age_formatting gtest)
  add_dependencies(password_hash_test gtest)
  add_dependencies(deck_hash_performance_test gtest)
  add_dependencies(ruled_utils_test gtest)
endif()

include_directories(${GTEST_INCLUDE_DIRS})
target_link_libraries(dummy_test Threads::Threads ${GTEST_BOTH_LIBRARIES})
target_link_libraries(expression_test libcockatrice_utility Threads::Threads ${GTEST_BOTH_LIBRARIES} ${TEST_QT_MODULES})
target_link_libraries(test_age_formatting Threads::Threads ${GTEST_BOTH_LIBRARIES} ${TEST_QT_MODULES})
target_link_libraries(
  password_hash_test libcockatrice_utility Threads::Threads ${GTEST_BOTH_LIBRARIES} ${TEST_QT_MODULES}
)
target_link_libraries(
  deck_hash_performance_test libcockatrice_deck_list libcockatrice_utility Threads::Threads ${GTEST_BOTH_LIBRARIES}
  ${TEST_QT_MODULES}
)
target_link_libraries(
  ruled_utils_test libcockatrice_network_server_remote Threads::Threads ${GTEST_BOTH_LIBRARIES} ${TEST_QT_MODULES}
)

add_subdirectory(card_zone_algorithms)
add_subdirectory(carddatabase)
add_subdirectory(dev_console)
add_subdirectory(game_prompt)
add_subdirectory(modal_cast_menu)
add_subdirectory(loading_from_clipboard)
add_subdirectory(movecard_tests)
add_subdirectory(oracle)
add_subdirectory(ruled_batch_tests)
add_subdirectory(ruled_client_tests)
if(TARGET servatrice)
  add_subdirectory(ruled_e2e_smoke)
endif()

# Windows CTest can successfully launch one Qt/vcpkg-linked test and then report
# STATUS_DLL_NOT_FOUND (0xc0000135) for later tests in the same CTest process, even though every
# executable succeeds when run alone with the same PATH. Give each test a tiny launcher process
# and declare its runtime environment here so a normal combined `ctest` invocation is reliable
# and callers no longer need to edit PATH by hand.
if(WIN32)
  set(_cockatrice_test_names
      dummy_test
      expression_test
      test_age_formatting
      password_hash_test
      ruled_utils_test
      deck_hash_performance_test
      card_zone_algorithms_test
      carddatabase_test
      filter_string_test
      ruled_dev_console_test
      game_prompt_widget_test
      modal_cast_menu_test
      loading_from_clipboard_test
      reverse_card_move_test
      parse_cipt_test
      ruled_batch_test
      ruled_client_test
  )
  if(TARGET ruled_e2e_smoke_test)
    list(APPEND _cockatrice_test_names ruled_e2e_smoke_test)
  endif()

  # TEST_LAUNCHER was added in CMake 3.29. Older supported CMake versions still receive the
  # original direct test command; current Windows developer/CI CMake versions use the repository
  # launcher to supply the runtime environment and avoid the repeated-launch loader failure.
  if(CMAKE_VERSION VERSION_GREATER_EQUAL 3.29)
    foreach(_test_target IN LISTS _cockatrice_test_names)
      set_property(
        TARGET ${_test_target}
        PROPERTY TEST_LAUNCHER
                 "powershell.exe;-NoProfile;-ExecutionPolicy;Bypass;-File;${CMAKE_SOURCE_DIR}/scripts/run-windows-test.ps1;-QtBin;$<TARGET_FILE_DIR:${COCKATRICE_QT_VERSION_NAME}::Core>;-VcpkgBin;${CMAKE_BINARY_DIR}/vcpkg_installed/${VCPKG_TARGET_TRIPLET}/bin"
      )
    endforeach()
  endif()
endif()
