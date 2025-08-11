use std::path::PathBuf;

use cached_path::cached_path;

pub async fn download_url(url: impl AsRef<str> + Send + Sync + 'static) -> eyre::Result<PathBuf> {
	let path = tokio::task::spawn_blocking(move || cached_path(url.as_ref())).await??;
	Ok(path)
}
