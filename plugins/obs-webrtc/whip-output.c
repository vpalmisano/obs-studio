#include "whip-output.h"

static void whip_output_close_unsafe(struct whip_output *output)
{
	if (output && output->whip_output) {
		obs_webrtc_whip_output_close(output->whip_output);
		obs_webrtc_whip_output_free(output->whip_output);
		output->whip_output = NULL;
	}
}

static const char *whip_output_getname(void *type_data)
{
	UNUSED_PARAMETER(type_data);

	return obs_module_text("whip_output");
}

static void *whip_output_create(obs_data_t *settings, obs_output_t *obs_output)
{
	UNUSED_PARAMETER(settings);

	struct whip_output *output = bzalloc(sizeof(struct whip_output));
	pthread_mutex_init_value(&output->write_mutex);

	output->output = obs_output;
	output->whip_output = NULL;

	// This needs to be recursive due to `obs_output_signal_stop` calling
	// `whip_output_total_bytes_sent` (also guarded by mutex)
	if (pthread_mutex_init_recursive(&output->write_mutex) != 0)
		goto fail;

	return output;

fail:
	pthread_mutex_destroy(&output->write_mutex);
	bfree(output);
	return NULL;
}

static void whip_output_destroy(void *data)
{
	struct whip_output *output = data;

	pthread_mutex_lock(&output->write_mutex);
	whip_output_close_unsafe(output);
	pthread_mutex_unlock(&output->write_mutex);

	pthread_mutex_destroy(&output->write_mutex);
	bfree(output);
}

static int map_whip_error_to_obs_output_error(OBSWebRTCWHIPOutputError error)
{
	switch (error) {
	case OBSWebRTCWHIPOutputError_ConnectFailed:
		return OBS_OUTPUT_CONNECT_FAILED;
	case OBSWebRTCWHIPOutputError_NetworkError:
		return OBS_OUTPUT_ERROR;
	default:
		blog(LOG_ERROR, "Invalid whip error code: %d", error);
		return OBS_OUTPUT_ERROR;
	}
}

static void whip_output_error_callback(void *data,
				       OBSWebRTCWHIPOutputError error)
{
	struct whip_output *output = data;
	pthread_mutex_lock(&output->write_mutex);
	if (output->whip_output) {
		whip_output_close_unsafe(output);
		obs_output_signal_stop(
			output->output,
			map_whip_error_to_obs_output_error(error));
	}
	pthread_mutex_unlock(&output->write_mutex);
}

static bool whip_output_start(void *data)
{
	struct whip_output *output = data;
	obs_service_t *service;
	obs_data_t *service_settings;
	const char *url, *bearer_token;

	service = obs_output_get_service(output->output);
	if (!service)
		return false;

	if (!obs_output_can_begin_data_capture(output->output, 0))
		return false;

	if (!obs_output_initialize_encoders(output->output, 0))
		return false;

	output->whip_output = obs_webrtc_whip_output_new();
	if (!output->whip_output) {
		blog(LOG_ERROR, "Unable to initialize whip output");
		return false;
	}

	obs_webrtc_whip_output_set_error_callback(
		output->whip_output,
		(OBSWebRTCWHIPOutputErrorCallback)whip_output_error_callback,
		output);

	service_settings = obs_service_get_settings(service);
	if (!service_settings)
		return false;

	url = obs_service_get_url(service);
	bearer_token = obs_data_get_string(service_settings, "bearer_token");
	obs_webrtc_whip_output_connect(output->whip_output, url, bearer_token);

	obs_output_begin_data_capture(output->output, 0);

	obs_data_release(service_settings);
	return true;
}

static void whip_output_stop(void *data, uint64_t ts)
{
	UNUSED_PARAMETER(ts);

	struct whip_output *output = data;

	pthread_mutex_lock(&output->write_mutex);
	whip_output_close_unsafe(output);
	pthread_mutex_unlock(&output->write_mutex);

	obs_output_signal_stop(output->output, OBS_OUTPUT_SUCCESS);
}

static void whip_output_data(void *data, struct encoder_packet *packet)
{
	struct whip_output *output = data;
	int64_t duration = 0;
	bool is_audio = false;

	if (packet->type == OBS_ENCODER_VIDEO) {
		duration = packet->dts_usec - output->video_timestamp;
		output->video_timestamp = packet->dts_usec;
	} else if (packet->type == OBS_ENCODER_AUDIO) {
		is_audio = true;
		duration = packet->dts_usec - output->audio_timestamp;
		output->audio_timestamp = packet->dts_usec;
	}

	pthread_mutex_lock(&output->write_mutex);
	if (output->whip_output) {
		if (!obs_webrtc_whip_output_write(output->whip_output,
						  packet->data, packet->size,
						  duration, is_audio)) {
			blog(LOG_ERROR,
			     "Unable to write packets to whip output");
		}
	}
	pthread_mutex_unlock(&output->write_mutex);
}

static void whip_output_defaults(obs_data_t *defaults)
{
	UNUSED_PARAMETER(defaults);
}

static obs_properties_t *whip_output_properties(void *unused)
{
	UNUSED_PARAMETER(unused);

	obs_properties_t *props = obs_properties_create();

	return props;
}

static uint64_t whip_output_total_bytes_sent(void *data)
{
	struct whip_output *output = data;
	pthread_mutex_lock(&output->write_mutex);
	uint64_t bytes_sent = 0;
	if (output->whip_output)
		bytes_sent =
			obs_webrtc_whip_output_bytes_sent(output->whip_output);
	pthread_mutex_unlock(&output->write_mutex);

	return bytes_sent;
}

static int whip_output_dropped_frames(void *data)
{
	struct whip_output *output = data;
	pthread_mutex_lock(&output->write_mutex);
	uint32_t dropped_frames = 0;
	if (output->whip_output)
		dropped_frames = obs_webrtc_whip_output_dropped_frames(
			output->whip_output);
	pthread_mutex_unlock(&output->write_mutex);

	return dropped_frames;
}

static int whip_output_connect_time_ms(void *data)
{
	struct whip_output *output = data;
	pthread_mutex_lock(&output->write_mutex);
	uint32_t connect_time = 0;
	if (output->whip_output)
		connect_time = obs_webrtc_whip_output_connect_time_ms(
			output->whip_output);
	pthread_mutex_unlock(&output->write_mutex);

	return connect_time;
}

struct obs_output_info whip_output_info = {
	.id = "whip_output",
	.flags = OBS_OUTPUT_AV | OBS_OUTPUT_ENCODED | OBS_OUTPUT_SERVICE,
	.encoded_video_codecs = "h264",
	.encoded_audio_codecs = "opus",
	.get_name = whip_output_getname,
	.create = whip_output_create,
	.destroy = whip_output_destroy,
	.start = whip_output_start,
	.stop = whip_output_stop,
	.encoded_packet = whip_output_data,
	.get_defaults = whip_output_defaults,
	.get_properties = whip_output_properties,
	.get_total_bytes = whip_output_total_bytes_sent,
	.get_connect_time_ms = whip_output_connect_time_ms,
	.get_dropped_frames = whip_output_dropped_frames,
};
