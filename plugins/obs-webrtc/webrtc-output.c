#include "webrtc-output.h"

static const char *webrtc_output_getname(void *unused)
{
	UNUSED_PARAMETER(unused);
	return obs_module_text("webrtc_output");
}

static void *webrtc_output_create(obs_data_t *settings, obs_output_t *obs_output)
{
	UNUSED_PARAMETER(settings);

	struct webrtc_output *output = bzalloc(sizeof(struct webrtc_output));

	output->output = obs_output;
	output->obsrtc = obs_webrtc_output_init();

	return output;
}

static void webrtc_output_destroy(void *data) {
	UNUSED_PARAMETER(data);

}

static bool webrtc_output_start(void *data)
{
	struct webrtc_output *output = data;

	if (!obs_output_can_begin_data_capture(output->output, 0)) {
		return false;
	}
	if (!obs_output_initialize_encoders(output->output, 0)) {
		return false;
	}

	obs_webrtc_output_connect(output->obsrtc);

	obs_output_begin_data_capture(output->output, 0);

	return true;
}

static void webrtc_output_stop(void *data, uint64_t ts)
{
	UNUSED_PARAMETER(ts);

	struct webrtc_output *output = data;

	obs_output_end_data_capture(output->output);
}

static void webrtc_output_data(void *data, struct encoder_packet *packet)
{
	struct webrtc_output *output = data;
	int64_t duration = 0;
	bool is_audio = false;

	if (packet->type == OBS_ENCODER_VIDEO) {
		duration = packet->dts_usec - output->video_timestamp;
		output->video_timestamp = packet->dts_usec;
	}

	if (packet->type == OBS_ENCODER_AUDIO) {
		is_audio = true;
		duration = packet->dts_usec - output->audio_timestamp;
		output->audio_timestamp = packet->dts_usec;
	}

	obs_webrtc_output_write(output->obsrtc, packet->data, packet->size, duration, is_audio);
}

static void webrtc_output_defaults(obs_data_t *defaults)
{
	UNUSED_PARAMETER(defaults);

}

static obs_properties_t *webrtc_output_properties(void *unused)
{
	UNUSED_PARAMETER(unused);

	obs_properties_t *props = obs_properties_create();

	return props;
}

static uint64_t webrtc_output_total_bytes_sent(void *data)
{
	UNUSED_PARAMETER(data);

	// TODO
	return 0;
}

static int webrtc_output_dropped_frames(void *data)
{
	UNUSED_PARAMETER(data);

	// TODO
	return 0;
}

static int webrtc_output_connect_time(void *data)
{
	UNUSED_PARAMETER(data);

	// TODO
	return 0;
}

struct obs_output_info webrtc_output_info = {
	.id = "webrtc_output",
	.flags = OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED | OBS_OUTPUT_SERVICE |
		 OBS_OUTPUT_MULTI_TRACK,
	.encoded_video_codecs = "h264",
	.encoded_audio_codecs = "opus",
	.get_name = webrtc_output_getname,
	.create = webrtc_output_create,
	.destroy = webrtc_output_destroy,
	.start = webrtc_output_start,
	.stop = webrtc_output_stop,
	.encoded_packet = webrtc_output_data,
	.get_defaults = webrtc_output_defaults,
	.get_properties = webrtc_output_properties,
	.get_total_bytes = webrtc_output_total_bytes_sent,
	.get_connect_time_ms = webrtc_output_connect_time,
	.get_dropped_frames = webrtc_output_dropped_frames,
};
