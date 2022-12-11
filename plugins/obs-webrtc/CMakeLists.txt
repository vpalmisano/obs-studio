project(obs-webrtc)

include(FetchContent)

FetchContent_Declare(
  Corrosion
  GIT_REPOSITORY https://github.com/AndrewGaspar/corrosion.git
  GIT_TAG origin/master # Optionally specify a version tag or branch here
)

FetchContent_MakeAvailable(Corrosion)

include_directories(${LIBCURL_INCLUDE_DIRS})

include_directories(${OBS_JANSSON_INCLUDE_DIRS})

option(ENABLE_FFMPEG_LOGGING "Enables obs-ffmpeg logging" OFF)

find_package(
  FFmpeg REQUIRED
  COMPONENTS avcodec
             avfilter
             avdevice
             avutil
             swscale
             avformat
             swresample)
include_directories(${FFMPEG_INCLUDE_DIRS})

set(obs-webrtc_HEADERS webrtc-output.h)
set(obs-webrtc_SOURCES webrtc-service.c webrtc-output.c webrtc.c)

if(WIN32)
  set(MODULE_DESCRIPTION "OBS webrtc module")
  configure_file(${CMAKE_SOURCE_DIR}/cmake/bundle/windows/obs-module.rc.in
                 obs-webrtc.rc)
  list(APPEND obs-webrtc_SOURCES obs-webrtc.rc)
endif()

add_library(obs-webrtc MODULE ${obs-webrtc_SOURCES} ${obs-webrtc_HEADERS})

corrosion_import_crate(MANIFEST_PATH webrtc/Cargo.toml)

# Doesn't work for deps
#if(OS_MACOS)
#  corrosion_set_env_vars(webrtc-rs-lib MACOSX_DEPLOYMENT_TARGET=${CMAKE_OSX_DEPLOYMENT_TARGET})
#endif()

target_link_libraries(obs-webrtc libobs webrtc-rs-lib
${OBS_JANSSON_IMPORT} ${LIBCURL_LIBRARIES} ${FFMPEG_LIBRARIES})

if(OS_MACOS)
  find_library(COREFOUNDATION CoreFoundation)
  find_library(SECURITY_FRAMEWORK Security)
  mark_as_advanced(COREFOUNDATION SECURITY_FRAMEWORK)
  target_link_libraries(obs-webrtc ${COREFOUNDATION} ${SECURITY_FRAMEWORK})
endif()

set_target_properties(obs-webrtc PROPERTIES FOLDER "plugins")

setup_plugin_target(obs-webrtc data)
