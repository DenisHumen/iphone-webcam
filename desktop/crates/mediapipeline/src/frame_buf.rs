use bytes::Bytes;

use crate::error::MediaPipelineError;

/// Split an NV12 payload (`width*height` Y bytes followed by `width*height/2`
/// CbCr bytes) into separate `Bytes` slices. Both slices share the original
/// buffer (no copy).
pub fn split_nv12(
    payload: Bytes,
    width: u16,
    height: u16,
) -> Result<(Bytes, Bytes), MediaPipelineError> {
    let w = width as usize;
    let h = height as usize;
    let y_size = w * h;
    let uv_size = w * h / 2;
    let expected = y_size + uv_size;
    if payload.len() != expected {
        return Err(MediaPipelineError::PlaneSizeMismatch {
            got: payload.len(),
            expected,
        });
    }
    let y = payload.slice(0..y_size);
    let uv = payload.slice(y_size..);
    Ok((y, uv))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_evenly() {
        let buf = Bytes::from(vec![0u8; 16 * 16 + 16 * 16 / 2]);
        let (y, uv) = split_nv12(buf, 16, 16).unwrap();
        assert_eq!(y.len(), 256);
        assert_eq!(uv.len(), 128);
    }

    #[test]
    fn splits_zero_size() {
        let buf = Bytes::new();
        let (y, uv) = split_nv12(buf, 0, 0).unwrap();
        assert!(y.is_empty());
        assert!(uv.is_empty());
    }

    #[test]
    fn rejects_short_payload() {
        let buf = Bytes::from(vec![0u8; 100]);
        let err = split_nv12(buf, 16, 16).unwrap_err();
        assert!(matches!(err, MediaPipelineError::PlaneSizeMismatch { .. }));
    }
}
