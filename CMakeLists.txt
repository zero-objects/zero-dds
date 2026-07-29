cmake_minimum_required(VERSION 3.16 FATAL_ERROR)

set(ZERODDS_PROJECT_VERSION "")

if(NOT EXISTS "${CMAKE_CURRENT_SOURCE_DIR}/cargo.toml")
  message(WARNING "Unable to find 'cargo.toml' in current directory: project version UNKNOWN")
else()
  file(STRINGS "${CMAKE_CURRENT_SOURCE_DIR}/cargo.toml" ZERODDS_CARGO_TOML)
  set(ZERODDS_SEARCHING_FOR_VERSION OFF)
  foreach(line IN LISTS ZERODDS_CARGO_TOML)
    # Search in cargo.toml for the pattern 'major[.minor][.patch][.tweak]'
    if(ZERODDS_SEARCHING_FOR_VERSION AND "${line}" MATCHES "version[\r\n\t ]*=[\r\n\t ]*\"?([0-9]*([\.][0-9]+)?([\.][0-9]+)?([\.][0-9]+)?)\"?")
      set(ZERODDS_PROJECT_VERSION ${CMAKE_MATCH_1})
      break()
    endif()

    if(${line} MATCHES "workspace.package")
      set(ZERODDS_SEARCHING_FOR_VERSION ON)
    endif()
  endforeach()
endif()

if(NOT ZERODDS_PROJECT_VERSION)
  message(WARNING "Unable to read rust library version")
endif()

project(zerodds
    VERSION ${ZERODDS_PROJECT_VERSION}
    DESCRIPTION "ZERO-DDS C C++ bindings"
    LANGUAGES C CXX
)

option(BUILD_SHARED_LIBS "Build static or shared library" OFF)
option(BUILD_ZERODDS_CPP "Enable building zerodds::zerodds-cpp library" ON)

# Set CMAKE_BUILD_TYPE if neither parent nor project set it or if we are using a multi-config generator
get_property(ZERODDS_MULTI_CONFIG GLOBAL PROPERTY GENERATOR_IS_MULTI_CONFIG)
if(NOT ZERODDS_MULTI_CONFIG AND NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE "Release" CACHE STRING "Build type for the library")
endif()

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
if("${CMAKE_BUILD_TYPE}" STREQUAL "Release" OR "${CMAKE_BUILD_TYPE}" STREQUAL "Debug")
    string(TOLOWER "${CMAKE_BUILD_TYPE}" ZERODDS_BUILD_TYPE)
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
  BYPRODUCTS ${ZERODDS_LIBRARY_OUTPUT}
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

# Set imported target library location and implib.
# On Windows if the library is static then both properties
# must be the same .lib file. On Linux the 'IMPORTED_IMPLIB' will be ignored
set_target_properties(
  ${PROJECT_NAME}-c
  PROPERTIES IMPORTED_LOCATION ${ZERODDS_LIBRARY_OUTPUT}
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
