use std::{os::raw::c_char, slice};

use anyhow::Result;
use log::error;
use output::OutputStream;
use tokio::runtime::Runtime;

mod obs_log;
mod output;
mod whip;

pub struct OBSWebRTCOutput {
    stream: OutputStream,
    runtime: Runtime,
}

#[no_mangle]
pub extern "C" fn obs_webrtc_output_new() -> *mut OBSWebRTCOutput {
    (|| -> Result<*mut OBSWebRTCOutput> {
        Ok(Box::into_raw(Box::new(OBSWebRTCOutput {
            stream: OutputStream::new()?,
            runtime: tokio::runtime::Runtime::new()?,
        })))
    })()
    .unwrap_or_else(|e| {
        error!("Unable to create webrtc output: {:?}", e);
        std::ptr::null_mut::<OBSWebRTCOutput>()
    })
}

#[no_mangle]
pub unsafe extern "C" fn obs_webrtc_output_free(output: *mut OBSWebRTCOutput) {
    Box::from_raw(output);
}

#[no_mangle]
pub extern "C" fn obs_webrtc_output_connect(
    output: &'static OBSWebRTCOutput,
    url: *const c_char,
    stream_key: *const c_char,
) {
    let url = unsafe { std::ffi::CStr::from_ptr(url).to_str().unwrap() }.to_owned();
    let stream_key = unsafe { std::ffi::CStr::from_ptr(stream_key).to_str().unwrap() }.to_owned();

    output.runtime.spawn(async move {
        output
            .stream
            .connect(&url, &stream_key)
            .await
            .unwrap_or_else(|e| error!("Failed connecting to webrtc output: {:?}", e));
    });
}

#[no_mangle]
pub extern "C" fn obs_webrtc_output_write(
    output: &'static OBSWebRTCOutput,
    data: *const u8,
    size: usize,
    duration: u64,
    is_audio: bool,
) {
    let slice: &[u8] = unsafe { slice::from_raw_parts(data, size) };

    output.runtime.block_on(async {
        if is_audio {
            output
                .stream
                .write_audio(slice, duration)
                .await
                .unwrap_or_else(|e| error!("Unable to write audio to connection: {:?}", e))
        } else {
            output
                .stream
                .write_video(slice, duration)
                .await
                .unwrap_or_else(|e| error!("Unable to write audio to connection: {:?}", e))
        }
    });
}
