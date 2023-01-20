#include "obs-module.h"

struct whip_service_state {
	char *server;
};

static const char *whip_service_name(void *type_data)
{
	UNUSED_PARAMETER(type_data);

	return obs_module_text("WHIPService");
}

static void whip_service_update(void *data, obs_data_t *settings)
{
	struct whip_service_state *service = data;

	bfree(service->server);

	service->server = bstrdup(obs_data_get_string(settings, "server"));
}

static void whip_service_destroy(void *data)
{
	struct whip_service_state *service = data;

	bfree(service->server);
	bfree(service);
}

static void *whip_service_create(obs_data_t *settings, obs_service_t *service)
{
	struct whip_service_state *data =
		bzalloc(sizeof(struct whip_service_state));

	whip_service_update(data, settings);

	UNUSED_PARAMETER(service);
	return data;
}

static const char *whip_service_url(void *data)
{
	struct whip_service_state *service = data;
	return service->server;
}

static obs_properties_t *whip_service_properties(void *data)
{
	UNUSED_PARAMETER(data);

	obs_properties_t *ppts = obs_properties_create();

	obs_properties_add_text(ppts, "server", "URL", OBS_TEXT_DEFAULT);
	obs_properties_add_text(ppts, "bearer_token",
				obs_module_text("BearerToken"),
				OBS_TEXT_PASSWORD);

	return ppts;
}

static const char *whip_service_get_output_type(void *data)
{
	UNUSED_PARAMETER(data);

	return "whip_output";
}

static void whip_service_apply_encoder_settings(void *data,
						obs_data_t *video_settings,
						obs_data_t *audio_settings)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(audio_settings);

	// For now, ensure maximum compatibility with webrtc peers
	if (video_settings) {
		obs_data_set_int(video_settings, "bf", 0);
		obs_data_set_string(video_settings, "profile", "baseline");
		obs_data_set_string(video_settings, "rate_control", "CBR");
		obs_data_set_bool(video_settings, "repeat_headers", true);
	}
}

struct obs_service_info whip_service_info = {
	.id = "whip_custom",
	.get_name = whip_service_name,
	.create = whip_service_create,
	.destroy = whip_service_destroy,
	.update = whip_service_update,
	.get_properties = whip_service_properties,
	.get_url = whip_service_url,
	.get_output_type = whip_service_get_output_type,
	.apply_encoder_settings = whip_service_apply_encoder_settings,
};
