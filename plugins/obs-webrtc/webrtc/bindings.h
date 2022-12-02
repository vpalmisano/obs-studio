#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct OBSWebRTCStream OBSWebRTCStream;

struct OBSWebRTCStream *obs_webrtc_stream_init(const char*);

void obs_webrtc_stream_connect(struct OBSWebRTCStream *obsrtc);

void obs_webrtc_stream_data(struct OBSWebRTCStream *obsrtc,
                            const uint8_t *data,
                            uintptr_t size,
                            uint64_t duration);

void obs_webrtc_stream_audio(struct OBSWebRTCStream *obsrtc,
                             const uint8_t *data,
                             uintptr_t size,
                             uint64_t duration);
