//!
//! Describes `consumer` errors and converts other errors into our `Error` type.
//!

/// Describes errors that can occur while writing consumer files to disk.
///
#[derive(Debug)]
pub enum Error {
    /// An std::io::Error, from reading files in the `support` directory, or writing to the output
    /// directory. The value of this variant is the std::io::Error that occurred.
    ///
    Io(std::io::Error),
    /// An error from converting the file name (a native OS string) to a `String`. This likely
    /// indicates that a filename contains one or more non-UTF8 characters. The value of this
    /// variant is the name of the file that could not be converted to a `String`.
    ///
    Os(std::ffi::OsString),
    MissingPathSegment,
    BadStringSegment,
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<std::ffi::OsString> for Error {
    fn from(e: std::ffi::OsString) -> Self {
        Self::Os(e)
    }
}
