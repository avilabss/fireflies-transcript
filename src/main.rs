mod fireflies;
mod transcript;

use std::{fs, io::Write, path::PathBuf, process::ExitCode};

use anyhow::{Context, Result};
use clap::Parser;

/// Download a Fireflies meeting transcript as JSON.
#[derive(Debug, Parser)]
#[command(version, group(clap::ArgGroup::new("source").required(true).args(["url", "response_file"])))]
struct Args {
    /// Fireflies view URL (quote URLs containing & in your shell).
    #[arg(value_name = "URL", value_parser = fireflies::meeting_id_from_url)]
    url: Option<String>,

    /// Convert a saved Fireflies response instead of making a request.
    #[arg(long, value_name = "FILE")]
    response_file: Option<PathBuf>,

    /// Output path (default: <meeting-id>-transcript.json, or transcript.json offline).
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> ExitCode {
    match run(Args::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Error: {error:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run(args: Args) -> Result<()> {
    let output = args.output.unwrap_or_else(|| {
        args.url.as_ref().map_or_else(
            || PathBuf::from("transcript.json"),
            |id| PathBuf::from(format!("{id}-transcript.json")),
        )
    });
    let response = match (args.url, args.response_file) {
        (Some(id), None) => fireflies::fetch(&id).await?,
        (None, Some(path)) => {
            fs::read(&path).with_context(|| format!("could not read {}", path.display()))?
        }
        _ => anyhow::bail!("provide either a meeting URL or --response-file"),
    };
    let entries = transcript::parse(&response)?;
    let mut json = serde_json::to_vec_pretty(&entries).context("could not serialize transcript")?;
    json.push(b'\n');

    // create_new prevents accidentally replacing an existing transcript or input file.
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .with_context(|| {
            format!(
                "could not create {} (output must not already exist)",
                output.display()
            )
        })?;
    file.write_all(&json)
        .with_context(|| format!("could not write {}", output.display()))?;
    println!("Wrote {} captions to {}", entries.len(), output.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_exactly_one_source() {
        assert!(Args::try_parse_from(["app"]).is_err());
        assert!(Args::try_parse_from(["app", "--response-file", "response.json"]).is_ok());
        assert!(
            Args::try_parse_from([
                "app",
                "https://app.fireflies.ai/view/title::ABC123",
                "--response-file",
                "response.json",
            ])
            .is_err()
        );
    }
}
