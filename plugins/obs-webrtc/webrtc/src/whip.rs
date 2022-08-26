use anyhow::Result;

use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;

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
