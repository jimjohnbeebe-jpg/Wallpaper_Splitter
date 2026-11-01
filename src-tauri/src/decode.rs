// decode: every source format -> pixels for export, plus ImageInfo and a WebView2-readable preview proxy.
// Signatures are the contract; bodies are built in pass 1.
#![allow(dead_code)]
use crate::model::ImageInfo;
use std::path::Path;

/// Reads the header (and writes a preview proxy into `cache_dir` for formats WebView2 cannot show).
pub fn open(path: &str, cache_dir: &Path) -> Result<ImageInfo, String> {
    let _ = (path, cache_dir);
    Err("Opening images is not built yet".into())
}
