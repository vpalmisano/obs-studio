#include "util/text-lookup.h"
#include "util/threading.h"
#include "util/platform.h"
#include "util/dstr.h"
#include "obs-module.h"
#include "file-updater/file-updater.h"

OBS_DECLARE_MODULE()
OBS_MODULE_USE_DEFAULT_LOCALE("obs-webrtc-services", "en-US")
MODULE_EXPORT const char *obs_module_description(void)
{
	return "OBS core WebRTC services";
}

extern struct obs_service_info webrtc_service;

static struct dstr module_name = {0};

const char *get_module_name(void)
{
	return module_name.array;
}

bool obs_module_load(void)
{
	dstr_copy(&module_name, "obs-webrtc-services plugin (libobs ");
	dstr_cat(&module_name, obs_get_version_string());
	dstr_cat(&module_name, ")");

	obs_register_service(&webrtc_service);
	return true;
}

void obs_module_unload(void)
{
	dstr_free(&module_name);
}
