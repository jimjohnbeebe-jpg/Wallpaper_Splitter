// export: resize each tile, write PNG/JPG/JXR named from the template, optional span image.
// Signatures are the contract; bodies are built in pass 1.
#![allow(dead_code)]
use crate::model::{ExportJob, ExportResult, MonitorInfo};

/// Writes one file per monitor (and the span image when asked). `sdr_white_nits` scales HDR output.
pub fn export(
    job: &ExportJob,
    monitors: &[MonitorInfo],
    sdr_white_nits: f64,
) -> Result<ExportResult, String> {
    let _ = (job, monitors, sdr_white_nits);
    Err("Export is not built yet".into())
}
