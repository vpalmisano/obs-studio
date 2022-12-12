use crate::whip;
use anyhow::{anyhow, bail, Result};
use bytes::Bytes;
use log::{debug, error, info};
use std::boxed::Box;
use std::sync::{Arc, Mutex};
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_H264, MIME_TYPE_OPUS};
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState;
use webrtc::interceptor::registry::Registry;
use webrtc::media::Sample;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::rtp_transceiver::RTCRtpTransceiverInit;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;

pub struct OutputStream {
    video_track: Arc<TrackLocalStaticSample>,
    audio_track: Arc<TrackLocalStaticSample>,
    peer_connection: Arc<RTCPeerConnection>,
}

impl OutputStream {
    pub async fn new() -> Result<Self> {
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

        let peer_connection = Arc::new(api.new_peer_connection(config).await?);

        Ok(Self {
            audio_track,
            video_track,
            peer_connection,
        })
    }

    pub async fn connect(&self, url: &str, stream_key: &str) -> Result<()> {
        println!("Setting up webrtc!");

        self.peer_connection.add_transceiver_from_track(self.video_track.clone(), &[RTCRtpTransceiverInit {
            direction: webrtc::rtp_transceiver::rtp_transceiver_direction::RTCRtpTransceiverDirection::Sendonly,
            send_encodings: Vec::new()
        }]).await?;

        self.peer_connection.add_transceiver_from_track(self.audio_track.clone(), &[RTCRtpTransceiverInit {
            direction: webrtc::rtp_transceiver::rtp_transceiver_direction::RTCRtpTransceiverDirection::Sendonly,
            send_encodings: Vec::new()
        }]).await?;

        self.peer_connection
            .on_ice_connection_state_change(Box::new(
                move |connection_state: RTCIceConnectionState| {
                    info!("Connection State has changed {}", connection_state);
                    if connection_state == RTCIceConnectionState::Connected {
                        // on_success();
                    }
                    Box::pin(async {})
                },
            ));

        self.peer_connection
            .on_peer_connection_state_change(Box::new(move |s: RTCPeerConnectionState| {
                debug!("Peer Connection State has changed: {}", s);

                if s == RTCPeerConnectionState::Failed {
                    error!("Peer connection state went to failed")
                }

                Box::pin(async {})
            }));

        let offer = self.peer_connection.create_offer(None).await?;
        let mut gather_complete = self.peer_connection.gathering_complete_promise().await;
        self.peer_connection.set_local_description(offer).await?;

        // Block until gathering complete
        let _ = gather_complete.recv().await;

        let offer = self
            .peer_connection
            .local_description()
            .await
            .ok_or_else(|| anyhow!("No local description available"))?;
        let answer = whip::offer(&url, stream_key, offer).await?;
        self.peer_connection.set_remote_description(answer).await?;

        Ok(())
    }

    pub async fn close(&self) -> Result<()> {
        Ok(self.peer_connection.close().await?)
    }

    pub async fn write_video(&self, data: &[u8], duration: u64) -> Result<()> {
        let sample = Sample {
            data: Bytes::from(data.to_owned()),
            duration: std::time::Duration::from_nanos(duration),
            ..Default::default()
        };

        match self.peer_connection.connection_state() {
            s @ RTCPeerConnectionState::Failed | s @ RTCPeerConnectionState::Closed => {
                bail!("Invalid connection state for write_video: {s}",)
            }
            _ => {
                self.video_track.write_sample(&sample).await?;
            }
        }
        return Ok(());
    }

    pub async fn write_audio(&self, data: &[u8], duration: u64) -> Result<()> {
        let sample = Sample {
            data: Bytes::from(data.to_owned()),
            duration: std::time::Duration::from_nanos(duration),
            ..Default::default()
        };

        match self.peer_connection.connection_state() {
            s @ RTCPeerConnectionState::Failed | s @ RTCPeerConnectionState::Closed => {
                bail!("Invalid connection state for write_audio: {s}",)
            }
            _ => {
                self.audio_track.write_sample(&sample).await?;
            }
        }
        return Ok(());
    }
}
