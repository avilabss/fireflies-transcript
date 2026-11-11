# fireflies-transcript

A small Rust CLI that downloads a Fireflies meeting and saves its captions as transcript JSON.

## Usage

With Rust and Cargo installed:

```sh
cargo run -- 'https://app.fireflies.ai/view/Sprint-planning-retro-Whitebox::01M6XXEXKGYZHDTS28NP3RR8NN?ref=recap&track=01M6XXEXKGYZHDTS28NP3RR8NN&sg=nb' --output transcript.json
```

Quote the URL so the shell does not interpret `&`. Without `--output` (`-o`), the filename is `<meeting-id>-transcript.json` in the current directory. The destination directory must exist; existing files are never overwritten.

Convert a previously downloaded response without network access:

```sh
cargo run -- --response-file response.json --output transcript.json
```

Offline mode defaults to `transcript.json`. Supply either a URL or `--response-file`, not both.

Build a standalone executable with `cargo build --release`; run `target/release/fireflies-transcript --help` for options.

## Output

```json
[
  {
    "sentence": "Hello.",
    "startTime": "04:32",
    "endTime": "04:33",
    "speaker_name": "Avinash Sah",
    "speaker_id": 0
  }
]
```

Caption order, sentence text, and numeric speaker IDs are preserved. Times are elapsed `MM:SS`, truncated to whole seconds; a recording past one hour uses `60:00`, `61:00`, etc.

Based on the supplied hive response, `speakerMeta` uses one-based keys while captions use zero-based IDs: caption ID `0` maps to metadata key `"1"`. Missing or blank names become `Speaker 1`, `Speaker 2`, etc. Empty caption arrays produce `[]`; missing/null captions report that the transcript is unavailable.

The app uses the Fireflies web application's `/api/v4/hive` endpoint and the query in `src/queries/fetch_notepad_meeting.graphql`. This is an internal endpoint and its schema may change. No login credentials are sent, so the meeting must be accessible without authentication. HTTP errors, GraphQL errors, invalid responses, and file errors produce a message on stderr and a nonzero exit status. Requests have a 10-second connection timeout and a 60-second overall timeout.

## Development

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

`main.rs` handles CLI arguments and output, `fireflies.rs` handles URLs and HTTP, and `transcript.rs` handles response validation and conversion. CLI parsing uses [clap's derive API](https://docs.rs/clap/latest/clap/_derive/_tutorial/).
