use crate::output::OutputStream;
use anyhow::Result;
use log::{error, info};
use std::{os::raw::c_char, slice, time::Duration};
use tokio::runtime::Runtime;

pub struct OBSWebRTCWHIPOutput {
    stream: OutputStream,
    runtime: Runtime,
}

/// Create a new whip output in rust and leak the pointer to caller
/// # Note
/// You must call `obs_webrtc_whip_output_free` on the returned value
#[no_mangle]
pub extern "C" fn obs_webrtc_whip_output_new() -> *mut OBSWebRTCWHIPOutput {
    (|| -> Result<*mut OBSWebRTCWHIPOutput> {
        let runtime = tokio::runtime::Runtime::new()?;
        let stream = runtime.block_on(async { OutputStream::new().await })?;
        Ok(Box::into_raw(Box::new(OBSWebRTCWHIPOutput {
            stream,
            runtime,
        })))
    })()
    .unwrap_or_else(|e| {
        error!("Unable to create whip output: {e:?}");
        std::ptr::null_mut::<OBSWebRTCWHIPOutput>()
    })
}

/// Free the whip output
/// # Safety
/// Called only from C
#[no_mangle]
pub unsafe extern "C" fn obs_webrtc_whip_output_free(output: *mut OBSWebRTCWHIPOutput) {
    info!("Freeing whip output");
    if !output.is_null() {
        drop(Box::from_raw(output));
    }
}

/// Retrieve the bytes sent during the session by the whip output
/// # Safety
/// Called only from C
#[no_mangle]
pub unsafe extern "C" fn obs_webrtc_whip_output_bytes_sent(
    output: &'static OBSWebRTCWHIPOutput,
) -> u64 {
    output.stream.bytes_sent()
}

/// Retrieve the count of dropped frames during the connected
/// session
#[no_mangle]
pub unsafe extern "C" fn obs_webrtc_whip_output_dropped_frames(
    output: &'static OBSWebRTCWHIPOutput,
) -> i32 {
    output.stream.dropped_frames()
}

/// Retrieve the congestion factor
#[no_mangle]
pub unsafe extern "C" fn obs_webrtc_whip_output_congestion(
    output: &'static OBSWebRTCWHIPOutput,
) -> f64 {
    output.stream.congestion()
}

/// Retrieve the connected duration in milliseconds
#[no_mangle]
pub unsafe extern "C" fn obs_webrtc_whip_output_connect_time_ms(
    output: &'static OBSWebRTCWHIPOutput,
) -> i32 {
    output.stream.connect_time().as_millis() as i32
}

/// Connect to the whip endpoint and begin the peer connection process.
/// # Note
/// This asynchronously returns before the connection has completed
/// # Safety
/// Called only from C
#[no_mangle]
pub unsafe extern "C" fn obs_webrtc_whip_output_connect(
    output: &'static OBSWebRTCWHIPOutput,
    url: *const c_char,
    bearer_token: *const c_char,
) {
    let url = std::ffi::CStr::from_ptr(url).to_str().unwrap().to_owned();
    let bearer_token = if !bearer_token.is_null() {
        Some(
            std::ffi::CStr::from_ptr(bearer_token)
                .to_str()
                .unwrap()
                .to_owned(),
        )
    } else {
        None
    };

    output.runtime.spawn(async move {
        let result = output.stream.connect(&url, bearer_token.as_deref()).await;
        if let Err(e) = result {
            error!("Failed connecting to whip output: {e:?}");
            // Close the peer connection so that future writes fail and disconnect the output
            // TODO: There should be some nuance about a connection failure and a mid-connection failure
            output
                .stream
                .close()
                .await
                .unwrap_or_else(|e| error!("Failed closing whip output after error: {e:?}"));
        }
    });
}

/// Close the whip output and terminate the peer connection
/// # Note
/// Once closed, you cannot call `obs_webrtc_whip_output_connect` again
#[no_mangle]
pub extern "C" fn obs_webrtc_whip_output_close(output: &'static OBSWebRTCWHIPOutput) {
    info!("Closing whip output");
    output
        .runtime
        .block_on(async { output.stream.close().await })
        .unwrap_or_else(|e| error!("Failed closing whip output: {e:?}"))
}

/// Write an audio or video packet to the whip output
/// # Safety
/// Called only from C
#[no_mangle]
pub unsafe extern "C" fn obs_webrtc_whip_output_write(
    output: &'static OBSWebRTCWHIPOutput,
    data: *const u8,
    size: usize,
    duration: u64,
    is_audio: bool,
) -> bool {
    let slice: &[u8] = slice::from_raw_parts(data, size);
    output
        .runtime
        .block_on(async {
            if is_audio {
                output
                    .stream
                    .write_audio(slice, Duration::from_micros(duration))
                    .await
                    .map(|_| true)
            } else {
                output
                    .stream
                    .write_video(slice, Duration::from_micros(duration))
                    .await
                    .map(|_| true)
            }
        })
        .unwrap_or_else(|e| {
            error!("Failed to write packets to whip output: {e:?}");
            false
        })
}
