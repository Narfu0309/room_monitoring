use chrono::{DateTime, TimeDelta, Utc};
use chrono_tz::{Asia, Tz};
use embedded_svc::http::client::Client as HttpClient;
use esp_idf_svc::{
    http::client::{
        Configuration as HttpConfiguration, EspHttpConnection, Response,
    }, io::{Write, utils::try_read_full}, sys::esp_crt_bundle_attach,
};
use heapless::spsc::Queue;
use serde::{Deserialize, Serialize};

/// Allowed number of posts [`SAFETY_STANDARDS_TIME`]
const SAFETY_STANDARDS_NUMBER: usize = 5;
/// [`SAFETY_STANDARDS_NUMBER`] posts are permitted within this time period.
const SAFETY_STANDARDS_TIME: TimeDelta = TimeDelta::minutes(1);

const WEBHOOK_URL: &str = include_str!("../webhook_url");

pub struct DiscordClient {
    client: HttpClient<EspHttpConnection>,
    /// for safety
    history: Queue<DateTime<Tz>, SAFETY_STANDARDS_NUMBER>,
}

impl DiscordClient {
    pub fn new() -> Self {
        Self {
            client: HttpClient::wrap(EspHttpConnection::new(&HttpConfiguration {
                crt_bundle_attach: Some(esp_crt_bundle_attach), // Support for HTTPS
                ..Default::default()
            }).unwrap()),
            history: Default::default(),
        }
    }

    pub fn post(&mut self, payload: String) -> Response<&mut EspHttpConnection> {
        while let Some(front) = self.history.peek()
            && Utc::now().with_timezone(&Asia::Tokyo) - front > SAFETY_STANDARDS_TIME
        {
            unsafe { self.history.dequeue_unchecked() };
        }
        // Panic will occur if the posting frequency exceeds the permitted level.
        self.history.enqueue(Utc::now().with_timezone(&Asia::Tokyo))
            .expect("Post の頻度が許容量を超過しました。");

        let payload = serde_json::to_string(&DiscordWebhookPostData {
            content: Some(payload),
            ..Default::default()
        }).unwrap();

        let mut request = self.client.post(
            WEBHOOK_URL,
            &[("content-type", "application/json")],
        ).unwrap();
        request.write_all(&payload.into_bytes()).unwrap();

        request.flush().unwrap();
        request.submit().unwrap()
    }
}

pub fn handle_response(res: Response<&mut EspHttpConnection>, buf: &mut [u8]) {
    println!("status: {}", res.status());
    let bytes_read = try_read_full(res, buf).map_err(|e| e.0).unwrap();
    println!("Read {bytes_read} bytes");
    match std::str::from_utf8(&buf[0..bytes_read]) {
        Ok(body_string) => println!(
            "Response body (truncated to {} bytes): {body_string:?}",
            buf.len()
        ),
        Err(e) => eprintln!("Error decoding response body: {e}"),
    };
}

impl Default for DiscordClient {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct DiscordWebhookPostData {
    pub content: Option<String>,
    pub username: Option<String>,
    pub avatar_url: Option<String>,
}