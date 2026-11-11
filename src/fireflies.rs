use std::time::Duration;

use anyhow::{Context, Result, ensure};
use reqwest::Url;
use serde_json::json;

const ENDPOINT: &str = "https://app.fireflies.ai/api/v4/hive";
const QUERY: &str = include_str!("queries/fetch_notepad_meeting.graphql");

pub fn meeting_id_from_url(input: &str) -> Result<String> {
    let url = Url::parse(input).context("expected a full Fireflies meeting URL")?;
    ensure!(
        url.scheme() == "https"
            && url.host_str() == Some("app.fireflies.ai")
            && url.username().is_empty()
            && url.password().is_none()
            && url.port_or_known_default() == Some(443),
        "expected an https://app.fireflies.ai/view/<title>::<meeting-id> URL"
    );
    let slug = url
        .path()
        .strip_prefix("/view/")
        .context("meeting URL must have a /view/ path")?
        .trim_end_matches('/');
    ensure!(!slug.contains('/'), "unexpected extra path in meeting URL");
    let (_, id) = slug
        .rsplit_once("::")
        .context("meeting URL must end in ::<meeting-id>")?;
    ensure!(
        !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric()),
        "meeting ID must contain only letters and numbers"
    );
    Ok(id.to_owned())
}

pub async fn fetch(meeting_id: &str) -> Result<Vec<u8>> {
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .user_agent(concat!("fireflies-transcript/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("could not initialize HTTP client")?;
    let response = client
        .post(ENDPOINT)
        .json(&json!({
            "operationName": "fetchNotepadMeeting",
            "variables": { "meetingNoteId": meeting_id },
            "query": QUERY,
        }))
        .send()
        .await
        .context("could not reach Fireflies")?
        .error_for_status()
        .context("Fireflies returned an HTTP error; check meeting access")?;
    Ok(response
        .bytes()
        .await
        .context("could not read Fireflies response")?
        .to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_id_ignoring_tracking_and_fragment() {
        let url = "https://app.fireflies.ai/view/Sprint-planning-retro-Whitebox::01M6XXEXKGYZHDTS28NP3RR8NN?ref=recap&track=another-id&sg=nb#notes";
        assert_eq!(
            meeting_id_from_url(url).unwrap(),
            "01M6XXEXKGYZHDTS28NP3RR8NN"
        );
        assert_eq!(
            meeting_id_from_url("https://app.fireflies.ai/view/title::ABC123/").unwrap(),
            "ABC123"
        );
    }

    #[test]
    fn rejects_invalid_urls() {
        for url in [
            "not a URL",
            "https://example.com/view/title::ABC",
            "http://app.fireflies.ai/view/title::ABC",
            "https://app.fireflies.ai/view/title",
            "https://app.fireflies.ai/view/title::",
            "https://app.fireflies.ai/view/title::ABC/extra",
            "https://app.fireflies.ai/view/title::ABC%2FDEF",
            "https://app.fireflies.ai/other/title::ABC",
        ] {
            assert!(meeting_id_from_url(url).is_err(), "accepted {url}");
        }
    }
}
