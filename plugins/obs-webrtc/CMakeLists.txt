project(obs-webrtc)

find_package(Corrosion QUIET)
if(NOT Corrosion_FOUND)
  include(FetchContent)
  FetchContent_Declare(
    Corrosion
    GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
    GIT_TAG v0.3.1)
  FetchContent_MakeAvailable(Corrosion)
endif()

set(OBS_WEBRTC_GENERATED_DIR "${CMAKE_CURRENT_BINARY_DIR}/generated")

corrosion_import_crate(MANIFEST_PATH webrtc/Cargo.toml)
corrosion_set_env_vars(
  webrtc-rs-lib "OBS_WEBRTC_GENERATED_DIR=${OBS_WEBRTC_GENERATED_DIR}"
  "OBS_WEBRTC_OBS_VERSION=${OBS_VERSION_CANONICAL}")

# Force dependent crates to link against the correct deployment target
if(OS_MACOS)
  corrosion_set_env_vars(
    webrtc-rs-lib
    "CFLAGS_aarch64_apple_darwin=-mmacosx-version-min=${CMAKE_OSX_DEPLOYMENT_TARGET}"
    "CFLAGS_x86_64_apple_darwin=-mmacosx-version-min=${CMAKE_OSX_DEPLOYMENT_TARGET}"
  )
  corrosion_add_target_rustflags(
    webrtc-rs-lib
    -Clink-arg=-mmacosx-version-min=${CMAKE_OSX_DEPLOYMENT_TARGET})
endif()

add_library(obs-webrtc MODULE)
add_library(OBS::webrtc ALIAS obs-webrtc)

target_sources(
  obs-webrtc PRIVATE whip-service.c whip-output.c webrtc.c whip-output.h
                     "${OBS_WEBRTC_GENERATED_DIR}/bindings.h")
set_source_files_properties("${OBS_WEBRTC_GENERATED_DIR}/bindings.h"
                            PROPERTIES GENERATED TRUE)

target_link_libraries(obs-webrtc OBS::libobs webrtc-rs-lib)
target_include_directories(obs-webrtc PRIVATE ${OBS_WEBRTC_GENERATED_DIR})

if(OS_WINDOWS)
  set(MODULE_DESCRIPTION "OBS webrtc module")
  configure_file("${CMAKE_SOURCE_DIR}/cmake/bundle/windows/obs-module.rc.in"
                 obs-webrtc.rc)
  target_sources(obs-webrtc PRIVATE obs-webrtc.rc)
endif()

if(OS_MACOS)
  find_library(COREFOUNDATION CoreFoundation)
  find_library(SECURITY_FRAMEWORK Security)
  find_library(SYSTEMCONFIGURATION SystemConfiguration)
  mark_as_advanced(COREFOUNDATION SECURITY_FRAMEWOR SYSTEMCONFIGURATION)
  target_link_libraries(obs-webrtc ${COREFOUNDATION} ${SECURITY_FRAMEWORK}
                        ${SYSTEMCONFIGURATION})
endif()

set_target_properties(obs-webrtc PROPERTIES FOLDER "plugins")

setup_plugin_target(obs-webrtc data)
