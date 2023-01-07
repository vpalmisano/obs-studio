use crate::whip;
use anyhow::{anyhow, Result};
use bytes::Bytes;
use log::{debug, error, info, trace};
use reqwest::Url;
use std::boxed::Box;
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tokio::select;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::time::interval;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::{MediaEngine, MIME_TYPE_H264, MIME_TYPE_OPUS};
use webrtc::api::setting_engine::SettingEngine;
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_connection_state::RTCIceConnectionState;
use webrtc::interceptor::registry::Registry;
use webrtc::media::Sample;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::rtp_transceiver::rtp_transceiver_direction::RTCRtpTransceiverDirection;
use webrtc::rtp_transceiver::RTCRtpTransceiverInit;
use webrtc::stats::StatsReportType;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;

pub type ErrorCallback = Box<dyn FnMut(OutputStreamError) + Send + Sync>;
#[derive(Debug)]
pub struct EncodedPacket {
    pub data: Vec<u8>,
    pub duration: Duration,
    pub typ: EncodedPacketType,
}

#[derive(Debug)]
pub enum EncodedPacketType {
    Audio,
    Video,
}

/// Errors for [`OutputStream`] worker thread
#[derive(Debug)]
pub enum OutputStreamError {
    ConnectFailed,
    NetworkError,
    WriteError(webrtc::Error),
}

#[derive(Debug)]
pub enum Message {
    Packet(EncodedPacket),
    Error(OutputStreamError),
    Close,
}

pub enum WorkerResult {
    Error(OutputStreamError),
    Close,
}

#[derive(Default)]
struct OutputStreamStats {
    bytes_sent: u64,
    congestion: f64,
    dropped_frames: i32,
    connect_time: Option<Instant>,
    connect_duration: Duration,
}

pub struct OutputStream {
    video_track: Arc<TrackLocalStaticSample>,
    audio_track: Arc<TrackLocalStaticSample>,
    peer_connection: Arc<RTCPeerConnection>,
    stats: Arc<RwLock<OutputStreamStats>>,
    worker_tx: UnboundedSender<Message>,
    worker_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    whip_resource: Arc<Mutex<Option<Url>>>,
    error_callback: Arc<Mutex<Option<ErrorCallback>>>,
}

impl OutputStream {
    fn start_worker(&mut self, mut worker_rx: UnboundedReceiver<Message>) {
        let worker_handle = std::thread::spawn({
            let audio_track = self.audio_track.clone();
            let video_track = self.video_track.clone();
            let peer_connection = self.peer_connection.clone();
            let stats = self.stats.clone();
            let error_callback = self.error_callback.clone();
            move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .thread_name("webrtc_worker")
                    .worker_threads(4)
                    .build()
                    .unwrap();
                let runtime = runtime;
                let result = runtime.block_on(
                    async move {
                        let mut interval = interval(Duration::from_millis(500));
                        'worker_loop: loop {
                            let result = select! {
                                message = worker_rx.recv() => {
                                    match message {
                                        Some(Message::Packet(packet)) => {
                                            let sample = Sample {
                                                data: Bytes::from(packet.data),
                                                duration: packet.duration,
                                                ..Default::default()
                                            };
                                            match (packet.typ, peer_connection.connection_state()) {
                                                (_, RTCPeerConnectionState::Failed | RTCPeerConnectionState::Closed) => Err(WorkerResult::Error(OutputStreamError::NetworkError)),
                                                (EncodedPacketType::Audio, _) => audio_track.write_sample(&sample).await.map_err(|e| WorkerResult::Error(OutputStreamError::WriteError(e))),
                                                (EncodedPacketType::Video, _) => video_track.write_sample(&sample).await.map_err(|e| WorkerResult::Error(OutputStreamError::WriteError(e)))
                                            }
                                        },
                                        Some(Message::Error(e)) => Err(WorkerResult::Error(e)),
                                        Some(Message::Close) | None => Err(WorkerResult::Close),
                                    }
                                }
                                _ = interval.tick() => {
                                    let pc_stats = peer_connection.get_stats().await;

                                    let stats = &mut stats.write().unwrap();
                                    if let Some(StatsReportType::Transport(transport_stats)) = pc_stats.reports.get("ice_transport") {
                                        stats.bytes_sent = transport_stats.bytes_sent as u64;
                                    }
                                    Ok(())
                                }
                            };

                           if let Err(e) = result {
                                break 'worker_loop e;
                           }
                        }
                    }
                );

                match result {
                    WorkerResult::Close => {}
                    WorkerResult::Error(e) => {
                        if let Some(callback) = &mut *error_callback.lock().unwrap() {
                            error!("Worker encountered error: {e:?}");
                            callback(e);
                        }
                    }
                }

                info!("Exiting worker thread");
            }
        });

        *self.worker_handle.lock().unwrap() = Some(worker_handle);
    }

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

        let gather_ips = if_addrs::get_if_addrs()
            .unwrap_or_else(|_| Vec::new())
            .iter()
            .fold(Vec::new(), |mut gather_ips, addr| {
                if !addr.is_loopback() {
                    trace!(
                        "Valid gather address for interface {}: {:#?}",
                        addr.name,
                        addr.ip()
                    );
                    gather_ips.push(addr.ip());
                } else {
                    trace!(
                        "Loopback address ignored for interface {}: {:#?}",
                        addr.name,
                        addr.ip()
                    );
                }
                gather_ips
            });

        let mut setting_engine = SettingEngine::default();

        // Only attempt to limit if we found valid interfaces, otherwise do not attempt
        // to limit as we may be on a platform that doesn't expose enough information
        if !gather_ips.is_empty() {
            debug!("Gather addresses: {gather_ips:#?}");
            setting_engine.set_ip_filter(Box::new(move |ip: std::net::IpAddr| -> bool {
                gather_ips.contains(&ip)
            }));
        }

        let api = APIBuilder::new()
            .with_media_engine(m)
            .with_interceptor_registry(registry)
            .with_setting_engine(setting_engine)
            .build();

        // Prepare the configuration
        let config = RTCConfiguration {
            ..Default::default()
        };

        let peer_connection = Arc::new(api.new_peer_connection(config).await?);
        let stats: Arc<RwLock<OutputStreamStats>> = Arc::new(RwLock::new(OutputStreamStats {
            connect_time: None,
            ..Default::default()
        }));

        let (worker_tx, worker_rx): (UnboundedSender<Message>, UnboundedReceiver<Message>) =
            mpsc::unbounded_channel();
        let error_callback: Arc<Mutex<Option<ErrorCallback>>> = Arc::new(Mutex::new(None));

        let mut output_stream = Self {
            audio_track,
            video_track,
            peer_connection,
            stats,
            worker_tx,
            worker_handle: Arc::new(Mutex::new(None)),
            whip_resource: Arc::new(Mutex::new(None)),
            error_callback,
        };

        output_stream.start_worker(worker_rx);

        Ok(output_stream)
    }

    pub async fn connect(&self, url: &str, bearer_token: Option<&str>) {
        self.connect_internal(url, bearer_token)
            .await
            .unwrap_or_else(|e| {
                error!("Failed connecting to: {e}");
                let _ = self
                    .worker_tx
                    .send(Message::Error(OutputStreamError::ConnectFailed));
            })
    }

    async fn connect_internal(&self, url: &str, bearer_token: Option<&str>) -> Result<()> {
        println!("Setting up webrtc!");

        self.peer_connection
            .add_transceiver_from_track(
                self.video_track.clone(),
                &[RTCRtpTransceiverInit {
                    direction: RTCRtpTransceiverDirection::Sendonly,
                    send_encodings: Vec::new(),
                }],
            )
            .await?;

        self.peer_connection
            .add_transceiver_from_track(
                self.audio_track.clone(),
                &[RTCRtpTransceiverInit {
                    direction: RTCRtpTransceiverDirection::Sendonly,
                    send_encodings: Vec::new(),
                }],
            )
            .await?;

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
        let (answer, whip_resource) = whip::offer(url, bearer_token, offer).await?;
        self.peer_connection.set_remote_description(answer).await?;

        *self.whip_resource.lock().unwrap() = Some(whip_resource);

        Ok(())
    }

    pub async fn close(&self) -> Result<()> {
        let close_result = self.worker_tx.send(Message::Close);

        // Take worker handle so it's dropped
        let worker_future = self.worker_handle.lock().unwrap().take();
        // If close was a success (worker could receive messages), join on thread
        // Otherwise, worker thread is already dead or currently closing due to error callback
        if close_result.is_ok() {
            if let Some(worker_future) = worker_future {
                worker_future
                    .join()
                    .unwrap_or_else(|e| error!("Failed joining worker thread: {e:?}"));
            }
        }

        let whip_resource = self.whip_resource.lock().unwrap().take();
        if let Some(whip_resource) = whip_resource {
            whip::delete(&whip_resource).await?;
        }
        Ok(self.peer_connection.close().await?)
    }

    pub fn bytes_sent(&self) -> u64 {
        self.stats.read().unwrap().bytes_sent
    }

    pub fn congestion(&self) -> f64 {
        self.stats.read().unwrap().congestion
    }

    pub fn connect_time(&self) -> Duration {
        let mut stats = self.stats.write().unwrap();

        if let Some(connect_time) = stats.connect_time {
            stats.connect_duration = Instant::now() - connect_time;
        }

        stats.connect_duration
    }

    pub fn dropped_frames(&self) -> i32 {
        self.stats.read().unwrap().dropped_frames
    }

    pub fn write(&self, packet: EncodedPacket) -> Result<()> {
        self.worker_tx
            .send(Message::Packet(packet))
            .map_err(|e| e.into())
    }

    pub fn set_error_callback(&self, callback: ErrorCallback) {
        *self.error_callback.lock().unwrap() = Some(callback);
    }
}
