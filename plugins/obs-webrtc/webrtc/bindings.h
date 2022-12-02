#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct OBSWebRTCOutput OBSWebRTCOutput;

struct OBSWebRTCOutput *obs_webrtc_output_init(void);

void obs_webrtc_output_connect(struct OBSWebRTCOutput *obsrtc);

void obs_webrtc_output_data(struct OBSWebRTCOutput *obsrtc,
                            const uint8_t *data,
                            uintptr_t size,
                            uint64_t duration);

void obs_webrtc_output_audio(struct OBSWebRTCOutput *obsrtc,
                             const uint8_t *data,
                             uintptr_t size,
                             uint64_t duration);
