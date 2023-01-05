use anyhow::Result;
use log::{info, warn};
use reqwest::{
    header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE, LOCATION, USER_AGENT},
    Url,
};
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

use crate::obs_log;

const OBS_VERSION: &'static str = env!("OBS_VERSION");

pub async fn offer(
    url: &str,
    bearer_token: Option<&str>,
    local_desc: RTCSessionDescription,
) -> Result<(RTCSessionDescription, Url)> {
    let client = reqwest::Client::new();

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/sdp"));
    headers.insert(
        USER_AGENT,
        HeaderValue::from_str(&format!("libobs/{OBS_VERSION}"))?,
    );

    if let Some(bearer_token) = bearer_token {
        if !bearer_token.is_empty() {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {bearer_token}"))?,
            );
        }
    }

    if obs_log::debug_whip() {
        info!(
            "[WHIP DEBUG | CAUTION SENSITIVE INFO] Sending offer to {url}: {}",
            local_desc.sdp
        );
    }

    let request = client.post(url).headers(headers).body(local_desc.sdp);

    if obs_log::debug_whip() {
        info!("[WHIP DEBUG | CAUTION SENSITIVE INFO] Offer request {request:#?}");
    }

    let response = request.send().await?;

    if obs_log::debug_whip() {
        info!("[WHIP DEBUG | CAUTION SENSITIVE INFO] Offer response: {response:#?}");
    }

    let mut url = response.url().to_owned();
    if let Some(location) = response.headers().get(LOCATION) {
        url.set_path(location.to_str()?);
    }

    let body = response.text().await?;
    let sdp = RTCSessionDescription::answer(body)?;

    if obs_log::debug_whip() {
        info!("[WHIP DEBUG | CAUTION SENSITIVE INFO] Answer SDP: {sdp:#?}");
    }

    Ok((sdp, url))
}

pub async fn delete(url: &Url) -> Result<()> {
    let client = reqwest::Client::new();

    let request = client.delete(url.to_owned()).header(
        USER_AGENT,
        HeaderValue::from_str(&format!("libobs/{OBS_VERSION}"))?,
    );

    if obs_log::debug_whip() {
        info!("[WHIP DEBUG | CAUTION SENSITIVE INFO] Delete request {request:#?}");
    }

    let response = request.send().await?;

    if obs_log::debug_whip() {
        info!("[WHIP DEBUG | CAUTION SENSITIVE INFO] Delete response {response:#?}");
    }

    if !response.status().is_success() {
        warn!("Failed DELETE of whip resource: {}", response.status())
    }

    Ok(())
}
