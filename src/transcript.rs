use std::collections::HashMap;

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Response {
    data: Option<Data>,
    #[serde(default)]
    errors: Vec<GraphQlError>,
}

#[derive(Deserialize)]
struct GraphQlError {
    message: String,
}

#[derive(Deserialize)]
struct Data {
    #[serde(rename = "meetingNote")]
    meeting_note: Option<Meeting>,
}

#[derive(Deserialize)]
struct Meeting {
    captions: Option<Vec<Caption>>,
    #[serde(rename = "speakerMeta")]
    speaker_meta: Option<HashMap<String, String>>,
}

#[derive(Deserialize)]
struct Caption {
    sentence: String,
    speaker_id: u32,
    time: f64,
    #[serde(rename = "endTime")]
    end_time: f64,
}

#[derive(Debug, Serialize)]
pub struct Entry {
    sentence: String,
    #[serde(rename = "startTime")]
    start_time: String,
    #[serde(rename = "endTime")]
    end_time: String,
    speaker_name: String,
    speaker_id: u32,
}

pub fn parse(bytes: &[u8]) -> Result<Vec<Entry>> {
    let response: Response =
        serde_json::from_slice(bytes).context("invalid Fireflies response JSON")?;
    if !response.errors.is_empty() {
        bail!(
            "Fireflies GraphQL error: {}",
            response
                .errors
                .iter()
                .map(|e| e.message.as_str())
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
    let meeting = response
        .data
        .and_then(|d| d.meeting_note)
        .context("meeting not found or not accessible")?;
    let captions = meeting
        .captions
        .context("meeting has no transcript available yet")?;
    let speakers = meeting.speaker_meta.unwrap_or_default();
    captions
        .into_iter()
        .enumerate()
        .map(|(index, caption)| {
            ensure!(
                caption.end_time >= caption.time,
                "caption {index}: end time precedes start time"
            );
            // In the hive response, captions are zero-based and speakerMeta keys are one-based.
            let speaker_key = (u64::from(caption.speaker_id) + 1).to_string();
            let speaker_name = speakers
                .get(&speaker_key)
                .filter(|name| !name.trim().is_empty())
                .cloned()
                .unwrap_or_else(|| format!("Speaker {speaker_key}"));
            Ok(Entry {
                sentence: caption.sentence,
                start_time: timestamp(caption.time)
                    .with_context(|| format!("caption {index}: invalid start time"))?,
                end_time: timestamp(caption.end_time)
                    .with_context(|| format!("caption {index}: invalid end time"))?,
                speaker_name,
                speaker_id: caption.speaker_id,
            })
        })
        .collect()
}

fn timestamp(seconds: f64) -> Result<String> {
    ensure!(
        seconds.is_finite() && seconds >= 0.0 && seconds < u64::MAX as f64,
        "timestamp must be a finite, non-negative number of seconds"
    );
    let seconds = seconds.floor() as u64;
    Ok(format!("{:02}:{:02}", seconds / 60, seconds % 60))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn response(captions: serde_json::Value, speakers: serde_json::Value) -> Vec<u8> {
        serde_json::to_vec(
            &json!({"data": {"meetingNote": {"captions": captions, "speakerMeta": speakers}}}),
        )
        .unwrap()
    }

    #[test]
    fn converts_exact_output_schema_and_one_based_speakers() {
        let bytes = response(
            json!([
                {"sentence": "Hello.", "time": 272.9, "endTime": 273.8, "speaker_id": 0},
                {"sentence": "Hi, 世界!", "time": 274.0, "endTime": 274.5, "speaker_id": 1}
            ]),
            json!({"1": "Xavier Antoviaque", "2": "Avinash Sah"}),
        );
        let entries = parse(&bytes).unwrap();
        assert_eq!(
            serde_json::to_value(entries).unwrap(),
            json!([
                {"sentence": "Hello.", "startTime": "04:32", "endTime": "04:33", "speaker_name": "Xavier Antoviaque", "speaker_id": 0},
                {"sentence": "Hi, 世界!", "startTime": "04:34", "endTime": "04:34", "speaker_name": "Avinash Sah", "speaker_id": 1}
            ])
        );
    }

    #[test]
    fn handles_missing_speaker_without_dropping_caption() {
        let bytes = response(
            json!([{"sentence": "Hi", "time": 0, "endTime": 1, "speaker_id": 4}]),
            json!(null),
        );
        assert_eq!(parse(&bytes).unwrap()[0].speaker_name, "Speaker 5");
    }

    #[test]
    fn formats_elapsed_minutes_and_rejects_invalid_times() {
        assert_eq!(timestamp(0.0).unwrap(), "00:00");
        assert_eq!(timestamp(59.99).unwrap(), "00:59");
        assert_eq!(timestamp(3600.0).unwrap(), "60:00");
        for value in [-1.0, f64::NAN, f64::INFINITY, u64::MAX as f64] {
            assert!(timestamp(value).is_err());
        }
    }

    #[test]
    fn reports_graphql_and_missing_data_errors() {
        let error = parse(br#"{"data":null,"errors":[{"message":"Access denied"}]}"#).unwrap_err();
        assert!(error.to_string().contains("Access denied"));
        for bytes in [
            b"not json".as_slice(),
            br#"{"data":null}"#,
            br#"{"data":{"meetingNote":null}}"#,
        ] {
            assert!(parse(bytes).is_err());
        }
        assert!(parse(&response(json!(null), json!({}))).is_err());
        assert!(parse(&response(json!([]), json!({}))).unwrap().is_empty());
    }

    #[test]
    fn rejects_malformed_captions_and_reversed_times() {
        assert!(parse(&response(json!([{"sentence":"missing fields"}]), json!({}))).is_err());
        let bytes = response(
            json!([{"sentence":"Hi", "time":2, "endTime":1, "speaker_id":0}]),
            json!({}),
        );
        assert!(
            parse(&bytes)
                .unwrap_err()
                .to_string()
                .contains("end time precedes")
        );
    }
}
