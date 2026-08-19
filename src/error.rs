//! Deterministic error codes (SPEC §20).
//!
//! Error text is stable and machine-readable. ScopeFolio MUST return
//! deterministic errors for all failure modes and MUST NOT silently
//! substitute another file or another line.

/// Error output string for a missing file.
pub const FILE_NOT_FOUND: &str = "FILE_NOT_FOUND";

/// Error output string for an unreadable file.
pub fn io_error(kind: IoErrorKind) -> String {
    match kind {
        IoErrorKind::PermissionDenied => "IO_ERROR: permission denied".to_string(),
        IoErrorKind::InvalidUtf8 => "IO_ERROR: invalid UTF-8".to_string(),
        IoErrorKind::ReadFailure => "IO_ERROR: read failure".to_string(),
    }
}

/// Categories of I/O failure.
pub enum IoErrorKind {
    PermissionDenied,
    InvalidUtf8,
    ReadFailure,
}

/// Map a std::io::Error to IoErrorKind for read operations.
pub fn map_io_error_read(e: &std::io::Error) -> IoErrorKind {
    match e.kind() {
        std::io::ErrorKind::PermissionDenied => IoErrorKind::PermissionDenied,
        _ => IoErrorKind::ReadFailure,
    }
}
