use crate::obs_log::ObsLogger;

///
/// Install the rust log adapter to funnel rust logs to
/// libobs blog
#[no_mangle]
pub extern "C" fn obs_webrtc_install_logger() {
    ObsLogger::install();
    log::info!("webrtc plugin preamble initialized")
}
