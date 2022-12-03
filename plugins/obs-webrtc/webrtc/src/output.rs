use anyhow::Result;

use std::boxed::Box;
use std::slice;
use std::sync::Arc;

use bytes::Bytes;

use tokio::runtime::Runtime;

use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_H264, MIME_TYPE_OPUS};
use webrtc::api::APIBuilder;
use webrtc::interceptor::registry::Registry;
use webrtc::media::Sample;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_local::TrackLocal;

pub struct OBSWebRTCOutput {
    runtime: Runtime,
    video_track: Arc<TrackLocalStaticSample>,
    audio_track: Arc<TrackLocalStaticSample>,
}

fn add_track_to_peerconnection(
    peer_connection: Arc<RTCPeerConnection>,
    track: Arc<dyn TrackLocal + Send + Sync>,
) {
    tokio::spawn(async move {
        // Add this newly created track to the PeerConnection
        let rtp_sender = peer_connection.add_track(track).await?;

        // Read incoming RTCP packets. By calling Read the RTCP
        // packets are processed the interceptors. Interceptors perform
        // operations like responding to NACKs
        let mut rtcp_buf = vec![0u8; 1500];
        while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
        Result::<()>::Ok(())
    });
}

pub async fn do_whip(local_desc: RTCSessionDescription) -> Result<RTCSessionDescription> {
    let client = reqwest::Client::new();
    let res = client
        .post("http://127.0.0.1:8080/api/whip")
        .body(local_desc.sdp)
        .send()
        .await?;

    let body = res.text().await?;
    let sdp = RTCSessionDescription::answer(body)?;
    Ok(sdp)
}

async fn connect(
    video_track: Arc<dyn TrackLocal + Send + Sync>,
    audio_track: Arc<dyn TrackLocal + Send + Sync>,
) -> Result<()> {
    let mut m = MediaEngine::default();
    m.register_default_codecs()?;

    let mut registry = Registry::new();
    registry = register_default_interceptors(registry, &mut m)?;

    let api = APIBuilder::new()
        .with_media_engine(m)
        .with_interceptor_registry(registry)
        .build();

    // Prepare the configuration
    let config = RTCConfiguration {
        ..Default::default()
    };

    // Create a new RTCPeerConnection
    let peer_connection = Arc::new(api.new_peer_connection(config).await?);

    add_track_to_peerconnection(
        Arc::clone(&peer_connection),
        Arc::clone(&audio_track) as Arc<dyn TrackLocal + Send + Sync>,
    );

    add_track_to_peerconnection(
        Arc::clone(&peer_connection),
        Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>,
    );

    // Create an offer to send to the browser
    let offer = peer_connection.create_offer(None).await?;
    peer_connection.set_local_description(offer.clone()).await?;

    let answer = do_whip(offer).await?;
    peer_connection.set_remote_description(answer).await?;

    Ok(())
}

#[no_mangle]
pub extern "C" fn obs_webrtc_output_init() -> *mut OBSWebRTCOutput {
    let video_track = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: MIME_TYPE_H264.to_owned(),
            clock_rate: 90000,
            sdp_fmtp_line: "profile-level-id=428014; max-fs=3600; max-mbps=108000; max-br=1400"
                .to_string(),
            ..Default::default()
        },
        "video".to_owned(),
        "webrtc-rs".to_owned(),
    ));

    let audio_track = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: MIME_TYPE_OPUS.to_owned(),
            ..Default::default()
        },
        "audio".to_owned(),
        "webrtc-rs".to_owned(),
    ));

    Box::into_raw(Box::new(OBSWebRTCOutput {
        runtime: tokio::runtime::Runtime::new().unwrap(),
        video_track: video_track,
        audio_track: audio_track,
    }))
}

#[no_mangle]
pub extern "C" fn obs_webrtc_output_connect(obsrtc: *mut OBSWebRTCOutput) {
    let obs_webrtc = unsafe { &*obsrtc };
    let video_track = Arc::clone(&obs_webrtc.video_track) as Arc<dyn TrackLocal + Send + Sync>;
    let audio_track = Arc::clone(&obs_webrtc.audio_track) as Arc<dyn TrackLocal + Send + Sync>;
    unsafe {
        (*obsrtc).runtime.spawn(async {
            connect(video_track, audio_track).await;
        });
    }
}

#[no_mangle]
pub extern "C" fn obs_webrtc_output_write(
    obsrtc: *mut OBSWebRTCOutput,
    data: *const u8,
    size: usize,
    duration: u64,
    is_audio: bool,
) {
    if obsrtc.is_null() {
        return;
    }

    let slice: &[u8] = unsafe { slice::from_raw_parts(data, size) };

    let sample = Sample {
        data: Bytes::from(slice),
        duration: std::time::Duration::from_micros(duration),
        ..Default::default()
    };

    unsafe {
        (*obsrtc).runtime.block_on(async {
            if is_audio {
                (*obsrtc).audio_track.write_sample(&sample).await;
            } else {
                (*obsrtc).video_track.write_sample(&sample).await;
            }
        });
    }
}
