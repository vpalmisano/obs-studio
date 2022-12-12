#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct OBSWebRTCOutput OBSWebRTCOutput;

struct OBSWebRTCOutput *obs_webrtc_output_new(void);

void obs_webrtc_output_free(struct OBSWebRTCOutput *output);

void obs_webrtc_output_connect(const struct OBSWebRTCOutput *output,
                               const char *url,
                               const char *stream_key);

void obs_webrtc_output_close(const struct OBSWebRTCOutput *output);

bool obs_webrtc_output_write(const struct OBSWebRTCOutput *output,
                             const uint8_t *data,
                             uintptr_t size,
                             uint64_t duration,
                             bool is_audio);

void obs_webrtc_install_logger(void);
