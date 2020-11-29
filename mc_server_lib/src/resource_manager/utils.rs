use anyhow::Result;
use log::*;
use std::io::Cursor;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio_compat_02::FutureExt;

pub async fn download_file_to_writer(
    file: &mut (impl AsyncWrite + Unpin),
    url: &str,
) -> Result<()> {
    let mut server_jar_response = reqwest::get(url).compat().await?;
    let size = server_jar_response.content_length().unwrap_or(0);
    let quiet = size < 6_000_000;
    let mut last_remaining = 0;
    if !quiet {
        debug!("Downloading {}", url);
        debug!("0MB/{}MB", size / 1000000)
    };
    while let Some(chunk) = server_jar_response.chunk().await? {
        let remaining = server_jar_response.content_length().unwrap_or(0);
        if (last_remaining as i64 - remaining as i64).abs() > 3000000_i64 {
            if !quiet {
                debug!("{}MB/{}MB", (size - remaining) / 1000000, size / 1000000)
            };
            last_remaining = remaining;
        }
        file.write_all(&chunk).await?;
    }
    if !quiet {
        debug!("{}MB/{}MB - END", size / 1000000, size / 1000000)
    };
    Ok(())
}
pub async fn download_file_to_binary(url: &str) -> Result<Vec<u8>> {
    let mut buf = vec![];
    download_file_to_writer(&mut Cursor::new(&mut buf), url).await?;
    Ok(buf)
}
pub async fn download_file_to_string(url: &str) -> Result<String> {
    Ok(String::from_utf8(download_file_to_binary(url).await?)?)
}
pub async fn download_file_to_json(url: &str) -> Result<serde_json::Value> {
    Ok(serde_json::from_str(&download_file_to_string(url).await?)?)
}
