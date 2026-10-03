use std::path::Path;

use anyhow::Context;
use futures_util::StreamExt;

/// The HTTP client for all external requests: no overall timeout
/// (tarballs can be huge and slow), but a connect timeout so dead
/// hosts fail fast.
fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("reqwest client with a custom timeout")
}

pub async fn http_get(url: &str) -> anyhow::Result<String> {
    let response = http_client()
        .get(url)
        .send()
        .await
        .with_context(|| format!("requesting {url} failed"))?;
    if !response.status().is_success() {
        anyhow::bail!("GET {url} returned {}", response.status());
    }
    let body = response
        .text()
        .await
        .with_context(|| format!("reading the response of {url} failed"))?;
    Ok(body)
}

/// Whether `url` exists, via a HEAD request.
pub async fn http_exists(url: &str) -> bool {
    http_client()
        .head(url)
        .send()
        .await
        .is_ok_and(|response| response.status().is_success())
}

/// Stream `url` to `destination` without buffering it in memory.
pub async fn http_download(url: &str, destination: &Path) -> anyhow::Result<()> {
    let response = http_client()
        .get(url)
        .send()
        .await
        .with_context(|| format!("requesting {url} failed"))?;
    if !response.status().is_success() {
        anyhow::bail!("GET {url} returned {}", response.status());
    }
    let partial = partial_path(destination);
    let mut file = std::fs::File::create(&partial)
        .with_context(|| format!("failed to create {}", partial.display()))?;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(|| format!("reading the response of {url} failed"))?;
        std::io::Write::write_all(&mut file, &chunk)
            .with_context(|| format!("failed to write {}", partial.display()))?;
    }
    complete_download(&partial, destination)
}

/// Where a download is written until it completes, so an interrupted
/// one never looks like a finished `destination`.
fn partial_path(destination: &Path) -> std::path::PathBuf {
    let mut name = destination.as_os_str().to_owned();
    name.push(".part");
    name.into()
}

fn complete_download(partial: &Path, destination: &Path) -> anyhow::Result<()> {
    std::fs::rename(partial, destination).with_context(|| {
        format!(
            "failed to move {} to {}",
            partial.display(),
            destination.display()
        )
    })
}

pub async fn ftp_get(url: &str) -> anyhow::Result<String> {
    let url = url.to_string();
    tokio::task::spawn_blocking(move || {
        let output = curl(&url, &[])?;
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    })
    .await
    .context("ftp fetch task panicked")?
}

/// Download `url` via curl into `destination`, without buffering it
/// in memory.
pub async fn ftp_download(url: &str, destination: &Path) -> anyhow::Result<()> {
    let url = url.to_string();
    let partial = partial_path(destination);
    let output = partial.to_string_lossy().into_owned();
    tokio::task::spawn_blocking(move || curl(&url, &["--output", &output]))
        .await
        .context("ftp download task panicked")??;
    complete_download(&partial, destination)
}

/// Run curl on `url` with extra args, failing with an actionable
/// message when curl is missing.
fn curl(url: &str, args: &[&str]) -> anyhow::Result<std::process::Output> {
    let output = std::process::Command::new("curl")
        .arg("--silent")
        .arg("--show-error")
        .arg("--fail")
        .args(args)
        .arg(url)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!(
                    "curl is required for ftp sources like {url} but is not installed; \
                     install the curl package"
                )
            } else {
                anyhow::anyhow!(e).context(format!("failed to run curl for {url}"))
            }
        })?;
    if !output.status.success() {
        anyhow::bail!(
            "curl fetch of {url} failed (exit status: {}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_http_get_rejects_bad_url() {
        assert!(http_get("not-a-url").await.is_err());
    }

    #[tokio::test]
    async fn test_failed_download_leaves_no_destination() {
        let dir = std::env::temp_dir().join(format!("debmagic-dl-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let destination = dir.join("foo-1.0.tar.gz");
        assert_eq!(partial_path(&destination), dir.join("foo-1.0.tar.gz.part"));

        // curl fails before creating any output
        assert!(
            ftp_download("ftp://127.0.0.1:1/foo-1.0.tar.gz", &destination)
                .await
                .is_err()
        );
        assert!(!destination.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
