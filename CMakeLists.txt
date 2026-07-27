cmake_minimum_required(VERSION 3.16 FATAL_ERROR)
project(zerodds
    VERSION 1.0.0
    DESCRIPTION "ZERO-DDS C C++ bindings"
    LANGUAGES C CXX
)

option(BUILD_SHARED_LIBS "Build static or shared library" OFF)
option(BUILD_ZERODDS_CPP "Enable building zerodds::zerodds-cpp library" ON)
option(ZERODDS_BUILD_INSTALL "Build install target" OFF)

# Directly set MSVC to avoid mismatch between cmake configuration and detected build type
set(CMAKE_BUILD_TYPE "Release" CACHE STRING "Build type for the library, directly set for MSVC")

# Ensure cargo is installed
set(CARGO_EXECUTABLE)
find_program(CARGO_EXECUTABLE NAMES cargo)
if("${CARGO_EXECUTABLE}" STREQUAL "CARGO_EXECUTABLE-NOTFOUND")
  message(FATAL_ERROR "Cargo not found. Look at: https://rust-lang.org/learn/get-started")
endif()

# Determine library extensions and suffixes for the current OS
if(WIN32)
  set(ZERODDS_SHARED_LIBRARY_EXTENSION "dll")
  set(ZERODDS_STATIC_LIBRARY_EXTENSION "lib")
  set(ZERODDS_LIBRARY_PREFIX "")
else()
  set(ZERODDS_SHARED_LIBRARY_EXTENSION "so")
  set(ZERODDS_STATIC_LIBRARY_EXTENSION "a")
  set(ZERODDS_LIBRARY_PREFIX "lib")
endif()
set(ZERODDS_C_LIBRARY_NAME ${ZERODDS_LIBRARY_PREFIX}${PROJECT_NAME})

# Use the build type to know the folder in which the library will be built
if(${CMAKE_BUILD_TYPE} STREQUAL "Release" OR ${CMAKE_BUILD_TYPE} STREQUAL "Debug")
    string(TOLOWER ${CMAKE_BUILD_TYPE} ZERODDS_BUILD_TYPE)
else()
    set(ZERODDS_BUILD_TYPE "release")
endif()

set(ZERODDS_CMAKE_LIBRARY_TYPE "STATIC")
if(BUILD_SHARED_LIBS)
  set(ZERODDS_CMAKE_LIBRARY_TYPE "SHARED")
endif()

# Set the path to the library binary file depending on the build type
set(ZERODDS_OUTPUT_DIR "${CMAKE_CURRENT_BINARY_DIR}/${ZERODDS_BUILD_TYPE}")
if(BUILD_SHARED_LIBS)
  set(ZERODDS_LIBRARY_OUTPUT ${ZERODDS_OUTPUT_DIR}/${ZERODDS_C_LIBRARY_NAME}.${ZERODDS_SHARED_LIBRARY_EXTENSION})
else()
  set(ZERODDS_LIBRARY_OUTPUT ${ZERODDS_OUTPUT_DIR}/${ZERODDS_C_LIBRARY_NAME}.${ZERODDS_STATIC_LIBRARY_EXTENSION})
endif()

# Set location of library headers
set(ZERODDS_C_INCLUDE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/crates/zerodds-c-api/include")
set(ZERODDS_C_HEADERS "${ZERODDS_C_INCLUDE_DIR}/zerodds.h"
                      "${ZERODDS_C_INCLUDE_DIR}/zerodds_xcdr2.h"
)

# Set build flag for cargo only if building in Release
if(${ZERODDS_BUILD_TYPE} STREQUAL "release")
    set(ZERODDS_CARGO_BUILD_FLAG "--${ZERODDS_BUILD_TYPE}")
endif()

# Add a custom target to call cargo for generating the library
add_custom_target(${PROJECT_NAME}_rust ALL
  ${CARGO_EXECUTABLE} "rustc" "--manifest-path" "${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml"
                      "--package" "zerodds-c-api" "${ZERODDS_CARGO_BUILD_FLAG}"
                      "--crate-type" $<IF:$<BOOL:${BUILD_SHARED_LIBS}>,"cdylib","staticlib">
                      "--target-dir" "${CMAKE_CURRENT_BINARY_DIR}"
  DEPENDS ${ZERODDS_C_HEADERS} # CMake will re-run the command whenever the files change
)

# Generate the imported library for ZERODDS and link it to the custom target to ensure the custom command to be called
add_library(${PROJECT_NAME}-c ${ZERODDS_CMAKE_LIBRARY_TYPE} IMPORTED GLOBAL ${ZERODDS_C_HEADERS})
add_library(${PROJECT_NAME}::${PROJECT_NAME}-c ALIAS ${PROJECT_NAME}-c)

if (WIN32)
  # Link Windows libraries
  target_link_libraries(${PROJECT_NAME}-c INTERFACE ntdll ws2_32 bcrypt)
endif()

# Manually specify the dependency between ZERODDS target and the fake one to ensure compiler will not invert the order
add_dependencies(${PROJECT_NAME}-c ${PROJECT_NAME}_rust)

# Change imported library location to the built one
set(ZERODDS_LIBRARY_IMPORTED_LOCATION ${ZERODDS_LIBRARY_OUTPUT})

# Set imported target library location and implib.
# On Windows if the library is static then both properties
# must be the same .lib file. On Linux the 'IMPORTED_IMPLIB' will be ignored
set_target_properties(
  ${PROJECT_NAME}-c
  PROPERTIES IMPORTED_LOCATION ${ZERODDS_LIBRARY_IMPORTED_LOCATION}
             IMPORTED_IMPLIB ${ZERODDS_LIBRARY_OUTPUT}.${ZERODDS_STATIC_LIBRARY_EXTENSION}
)

target_include_directories(${PROJECT_NAME}-c INTERFACE
  "${ZERODDS_C_INCLUDE_DIR}"
)

# Expose also zerodds_cxx target
if(BUILD_ZERODDS_CPP)
  add_subdirectory("${CMAKE_CURRENT_SOURCE_DIR}/crates/cpp")
endif()

# Export zerodds_idlc_generate function
include("${CMAKE_CURRENT_SOURCE_DIR}/cmake/zerodds_idlc_generate.cmake")

# Configure Install target
if(ZERODDS_BUILD_INSTALL)
  include(CMakePackageConfigHelpers)

  install(FILES ${ZERODDS_C_INCLUDE_DIR} DESTINATION ${CMAKE_INSTALL_INCLUDEDIR})

  # Configuration
  configure_package_config_file(
    ${CMAKE_CURRENT_SOURCE_DIR}/cmake/${PROJECT_NAME}Config.cmake.in
    ${CMAKE_CURRENT_BINARY_DIR}/${PROJECT_NAME}Config.cmake
    INSTALL_DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/${PROJECT_NAME}
  )
  write_basic_package_version_file(
    ${CMAKE_CURRENT_BINARY_DIR}/${PROJECT_NAME}ConfigVersion.cmake
    VERSION ${PROJECT_VERSION}
    COMPATIBILITY SameMajorVersion
  )
  # Package
  install(FILES ${CMAKE_CURRENT_BINARY_DIR}/${PROJECT_NAME}Config.cmake
                ${CMAKE_CURRENT_BINARY_DIR}/${PROJECT_NAME}ConfigVersion.cmake
          DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/${PROJECT_NAME}
  )
  install(
    EXPORT ${PROJECT_NAME}Targets
    FILE ${PROJECT_NAME}Targets.cmake
    NAMESPACE ${PROJECT_NAME}::
    DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/${PROJECT_NAME}
  )
  install(
    TARGETS ${PROJECT_NAME}
    EXPORT ${PROJECT_NAME}Targets
    RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR}
    LIBRARY DESTINATION ${CMAKE_INSTALL_LIBDIR}
    ARCHIVE DESTINATION ${CMAKE_INSTALL_LIBDIR}
  )

  #
  # CPack
  #
  set(CPACK_GENERATOR DEB RPM TXZ TGZ)
  set(CPACK_PACKAGE_VERSION ${PROJECT_VERSION})
  set(CPACK_PACKAGE_NAME ${PROJECT_NAME})
  set(CPACK_PACKAGE_DESCRIPTION_SUMMARY ${PROJECT_DESCRIPTION})
  set(CPACK_PACKAGE_FILE_NAME
      "${CPACK_PACKAGE_NAME}-${CPACK_PACKAGE_VERSION}.${CMAKE_SYSTEM_PROCESSOR}"
  )
  set(CPACK_PACKAGE_RPM_DIR "${CMAKE_CURRENT_BINARY_DIR}/_CPack_Packages/Linux/RPM")
  include(CPack)
endif()
