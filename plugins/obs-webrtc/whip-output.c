#include "whip-output.h"

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

	if (pthread_mutex_init(&output->write_mutex, NULL) != 0)
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
	if (output->whip_output)
		obs_webrtc_whip_output_free(output->whip_output);
	pthread_mutex_destroy(&output->write_mutex);
	bfree(output);
}



static bool whip_output_start(void *data)
{
	struct whip_output *output = data;
	obs_service_t *service;
	obs_data_t *service_settings;
	const char *url, *bearer_token;

	const char *video_codec =
		obs_encoder_get_codec(obs_output_get_video_encoder(output->output));

	const char *audio_codec =
		obs_encoder_get_codec(obs_output_get_audio_encoder(output->output, 0));

	service = obs_output_get_service(output->output);
	if (!service)
		return false;

	if (!obs_output_can_begin_data_capture(output->output, 0))
		return false;

	if (!obs_output_initialize_encoders(output->output, 0))
		return false;

	output->whip_output = obs_webrtc_whip_output_new(video_codec, audio_codec);

	if (!output->whip_output) {
		blog(LOG_ERROR, "Unable to initialize whip output");
		return false;
	}

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

static void whip_output_close_unsafe(struct whip_output *output)
{
	if (output) {
		obs_webrtc_whip_output_close(output->whip_output);
		obs_webrtc_whip_output_free(output->whip_output);
		output->whip_output = NULL;
	}
}

static void whip_output_stop(void *data, uint64_t ts)
{
	UNUSED_PARAMETER(ts);

	struct whip_output *output = data;

	pthread_mutex_lock(&output->write_mutex);
	whip_output_close_unsafe(output);
	pthread_mutex_unlock(&output->write_mutex);

	obs_output_end_data_capture(output->output);
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
			// For now, all write errors are treated as connectivity issues that cannot be recovered from
			obs_output_signal_stop(output->output,
					       OBS_OUTPUT_ERROR);
			whip_output_close_unsafe(output);
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
	.encoded_video_codecs = "h264;hevc",
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
