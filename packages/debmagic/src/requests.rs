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
    let mut file = std::fs::File::create(destination)
        .with_context(|| format!("failed to create {}", destination.display()))?;
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(|| format!("reading the response of {url} failed"))?;
        std::io::Write::write_all(&mut file, &chunk)
            .with_context(|| format!("failed to write {}", destination.display()))?;
    }
    Ok(())
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
    let destination = destination.to_string_lossy().into_owned();
    tokio::task::spawn_blocking(move || {
        curl(&url, &["--output", &destination])?;
        Ok(())
    })
    .await
    .context("ftp download task panicked")?
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
}
