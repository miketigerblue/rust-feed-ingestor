//! browser.rs
//!
//! Connects to a headless Chromium instance (e.g., chromedp/headless-shell or official Chrome running as a sidecar).
//! Fetches and sanitizes web content, allowing JavaScript execution and cookie handling.
//! Uses a remote browser via WebSocket (recommended for Docker environments).

use chromiumoxide::browser::Browser as ChromiumBrowser;
use chromiumoxide::page::ScreenshotParams;
use ammonia::clean;
use anyhow::{Result, Error};
use std::env;
use futures::StreamExt;
use serde_json::Value;
use tokio::time::{sleep, Duration};

/// Wrapper struct for a Chromium browser WebSocket connection.
pub struct Browser {
    inner: ChromiumBrowser,
}

impl Browser {
    /// Connect to a remote Chrome instance using the correct WebSocket URL,
    /// with retry logic for container startup races and robust error handling.
    ///
    /// The base URL is read from the `CHROME_WS_URL` environment variable,
    /// or defaults to "ws://chrome:9222" (Docker Compose service) if not set.
    /// This method fetches the `/json/version` endpoint to get the true
    /// WebSocket debugger endpoint and connects to that.
    pub async fn new() -> Result<Self> {
        // 1. Get the base (host:port) from env or default
        let base = env::var("CHROME_WS_URL").unwrap_or_else(|_| "ws://chrome:9222".to_string());
        println!("[browser.rs] Using base Chrome URL: {base}");

        // 2. Convert ws://... to http://... for the version endpoint
        let version_url = base
            .replace("ws://", "http://")
            .replace("wss://", "https://")
            + "/json/version";
        println!("[browser.rs] Using Chrome version endpoint: {version_url}");

        // 3. Retry up to 30 times (60s) for Chrome to become ready and return a valid JSON response
        let ws_url = {
            let mut last_err = None;
            let mut ws_url = None;
            'retry: for attempt in 0..30 {
                println!("[browser.rs] Attempt {}/30: Fetching Chrome /json/version ...", attempt + 1);
                match reqwest::get(&version_url).await {
                    Ok(resp) => match resp.text().await {
                        Ok(text) => {
                            println!("[browser.rs] Response from /json/version: '{}'", text);
                            if let Ok(json) = serde_json::from_str::<Value>(&text) {
                                if let Some(url) = json["webSocketDebuggerUrl"].as_str() {
                                    println!("[browser.rs] Found webSocketDebuggerUrl: {}", url);
                                    // --- Critical: rewrite localhost to Docker Compose service name ---
                                    let docker_url = url.replace("ws://localhost:9222", "ws://chrome:9222");
                                    ws_url = Some(docker_url);
                                    break 'retry;
                                } else {
                                    println!("[browser.rs] No webSocketDebuggerUrl in JSON response");
                                }
                            } else {
                                // Not valid JSON, likely Chrome not ready yet
                                println!("[browser.rs] Got non-JSON response: '{}'", text);
                                last_err = Some(Error::msg(format!("Got non-JSON response: '{}'", text)));
                            }
                        }
                        Err(e) => {
                            println!("[browser.rs] Error reading response text: {e}");
                            last_err = Some(Error::msg(e));
                        }
                    },
                    Err(e) => {
                        println!("[browser.rs] Error making request to /json/version: {e}");
                        last_err = Some(Error::msg(e));
                    }
                }
                sleep(Duration::from_secs(2)).await;
            }
            ws_url.ok_or_else(|| Error::msg(format!(
                "Could not fetch webSocketDebuggerUrl from Chrome after retries: {:?}",
                last_err
            )))?
        };

        println!("[browser.rs] Final WebSocket URL to connect: {}", ws_url);

        // 4. Retry connecting to the remote browser at the true websocket endpoint
        let mut last_connect_err = None;
        for attempt in 0..30 {
            println!("[browser.rs] Attempt {}/30: Connecting to Chromium at {}", attempt + 1, ws_url);
            match ChromiumBrowser::connect(ws_url.clone()).await {
                Ok((browser, mut handler)) => {
                    println!("[browser.rs] Successfully connected to Chromium!");
                    // Spawn the event handler as a background task to drive the browser's events.
                    tokio::spawn(async move {
                        while let Some(event) = handler.next().await {
                            if let Err(e) = event {
                                eprintln!("Chromium event handler error: {:?}", e);
                            }
                        }
                    });
                    return Ok(Self { inner: browser });
                }
                Err(e) => {
                    println!("[browser.rs] Error connecting to Chromium: {e}");
                    last_connect_err = Some(e);
                    sleep(Duration::from_secs(2)).await;
                }
            }
        }

        Err(Error::msg(format!(
            "Could not connect to Chrome after retries: {:?}",
            last_connect_err
        )))
    }

    /// Fetch and sanitize the content of a web page.
    pub async fn fetch_and_clean(&self, url: &str) -> Result<String> {
        // Open a new tab and navigate to the given URL.
        let page = self.inner.new_page(url)
            .await
            .map_err(Error::msg)?;

        // Wait for navigation to complete (the page is loaded).
        page.wait_for_navigation()
            .await
            .map_err(Error::msg)?;

        // Take a screenshot (optional, ensures page is rendered; can be removed if not needed).
        let params = ScreenshotParams::builder().build();
        let _ = page.screenshot(params).await.map_err(Error::msg)?;

        // Get the full HTML content of the page.
        let content = page.content().await.map_err(Error::msg)?;

        // Sanitize the HTML to remove scripts, styles, and unsafe tags.
        Ok(clean(&content))
    }
}