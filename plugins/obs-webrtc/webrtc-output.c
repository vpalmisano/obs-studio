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
	pthread_mutex_init_value(&output->write_mutex);

	output->output = obs_output;
	output->obsrtc = NULL;

	if (pthread_mutex_init(&output->write_mutex, NULL) != 0)
		goto fail;

	return output;
fail:
	pthread_mutex_destroy(&output->write_mutex);
	bfree(output);
	return NULL;
}

static void webrtc_output_destroy(void *data) {
	struct webrtc_output *output = data;
	if (output->obsrtc)
		obs_webrtc_output_free(output->obsrtc);
	pthread_mutex_destroy(&output->write_mutex);
	bfree(output);
}

static bool webrtc_output_start(void *data)
{
	struct webrtc_output *output = data;
	obs_service_t *service;
	const char *key, *url;

	service = obs_output_get_service(output->output);
	if (!service) {
		return false;
	}

	if (!obs_output_can_begin_data_capture(output->output, 0)) {
		return false;
	}
	if (!obs_output_initialize_encoders(output->output, 0)) {
		return false;
	}

	output->obsrtc = obs_webrtc_output_new();
	if (!output->obsrtc) {
		blog(LOG_ERROR, "Unable to initialize webrtc output");
		return false;
	}

	obs_webrtc_output_connect(
			output->obsrtc,
			obs_service_get_url(service),
			obs_service_get_key(service)
	);

	obs_output_begin_data_capture(output->output, 0);

	return true;
}

static void webrtc_output_close_unsafe(struct webrtc_output *output) {
	if (output) {
		obs_webrtc_output_close(output->obsrtc);
		obs_webrtc_output_free(output->obsrtc);
		output->obsrtc = NULL;
	}
}

static void webrtc_output_stop(void *data, uint64_t ts)
{
	UNUSED_PARAMETER(ts);

	struct webrtc_output *output = data;

	obs_output_end_data_capture(output->output);

	pthread_mutex_lock(&output->write_mutex);
	webrtc_output_close_unsafe(output);
	pthread_mutex_unlock(&output->write_mutex);
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

	pthread_mutex_lock(&output->write_mutex);
	if (output->obsrtc) {
		if (!obs_webrtc_output_write(output->obsrtc, packet->data, packet->size, duration, is_audio)) {
			// For now, all write errors are treated as connectivity issues that cannot be recovered from
			obs_output_signal_stop(output->output, OBS_OUTPUT_ERROR);
			webrtc_output_close_unsafe(output);
		}
	}
	pthread_mutex_unlock(&output->write_mutex);
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
