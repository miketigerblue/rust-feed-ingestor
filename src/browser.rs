//! browser.rs
//!
//! Responsible for fetching and sanitizing web content safely.
//! Because raw internet content is like a wild beast — it needs taming before you bring it home.

use reqwest::Client;
use scraper::{Html, Selector};
use ammonia::clean;

/// A polite little web browser for our OSINT pipeline.
/// Fetches pages and cleans out the nasties (scripts, styles, ads, etc.)
pub struct Browser {
    client: Client,
}

impl Browser {
    /// Create a new Browser with a sensible user-agent.
    /// We don't want to be mistaken for a botty bot!
    pub fn new() -> Self {
        Browser {
            client: Client::builder()
                .user_agent("OSINT-Enricher-Bot/1.0 (+https://yourdomain.example)")
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Fetch the HTML content at the given URL, parse it,
    /// extract the main text content, and sanitize it.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to fetch content from.
    ///
    /// # Returns
    ///
    /// * `Ok(String)` containing clean text content on success.
    /// * `Err(reqwest::Error)` if the HTTP request or parsing fails.
    ///
    /// # Panics
    ///
    /// This function does not panic unless the selector parsing fails,
    /// which should never happen with a hard-coded selector.
    pub async fn fetch_and_clean(&self, url: &str) -> Result<String, reqwest::Error> {
        // Fetch the page content over HTTP(S)
        let resp = self.client.get(url).send().await?.text().await?;

        // Parse the HTML document
        let document = Html::parse_document(&resp);

        // Define selectors for main content areas — article, main, or fallback to body
        // This is a simple heuristic; can be improved with readability algorithms
        let selector = Selector::parse("article, main, body").expect("Selector parsing failed");

        // Collect text from selected elements
        let mut content = String::new();
        for element in document.select(&selector) {
            // Join all text nodes inside the element with spaces
            content.push_str(&element.text().collect::<Vec<_>>().join(" "));
        }

        // Sanitize the collected text to remove scripts, styles, and other nasties
        let clean_content = clean(&content);

        // Return the cleaned text content for safe consumption
        Ok(clean_content)
    }
}