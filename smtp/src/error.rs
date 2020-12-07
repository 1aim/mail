//! Module containing all custom errors.
use std::io as std_io;

use new_tokio_smtp::error::{ConnectingFailed, GeneralError, LogicError};

use headers::error::HeaderValidationError;
use mail::error::MailError;

/// Error used when sending a mail fails.
///
/// Failing to encode a mail before sending
/// it also counts as a `MailSendError`, as
/// it's done "on the fly" when sending a mail.
#[derive(Debug, Fail)]
pub enum MailSendError {
    /// Something is wrong with the mail instance (e.g. it can't be encoded).
    ///
    /// This can happen for a number of reasons including:
    ///
    /// 1. Missing header fields.
    /// 2. Invalid header fields.
    /// 2. Encoding header fields fails.
    /// 3. Loading resources failed (resources like e.g. appendix, logo embedded in html mail, etc.)
    #[fail(display = "{}", _0)]
    Mail(MailError),

    /// Sending the mail failed.
    ///
    /// This can happen for a number of reasons including:
    /// 1. Server rejects mail transaction because of send or receiver
    ///    address or body data (e.g. body to long).
    /// 2. Mail address requires smtputf8 support, which is not given.
    /// 3. Server rejects sending the mail for other reasons (it's
    ///    closing, overloaded etc.).
    #[fail(display = "{}", _0)]
    Smtp(LogicError),

    /// Setting up the connection failed.
    ///
    /// Failures can include but are not limited to:
    ///
    /// - Connecting with TCP failed.
    /// - Starting TLS failed.
    /// - Server does not want to be used (e.g. failure on sending EHLO).
    /// - Authentication failed.
    #[fail(display = "{}", _0)]
    Connecting(ConnectingFailed),

    /// An I/O error happened while using the connection.
    ///
    /// This is mainly for I/O errors after the setup of the connection
    /// was successful, which normally includes sending Ehlo and Auth
    /// commands.
    #[fail(display = "{}", _0)]
    Io(std_io::Error),
}

impl From<MailError> for MailSendError {
    fn from(err: MailError) -> Self {
        MailSendError::Mail(err)
    }
}

impl From<LogicError> for MailSendError {
    fn from(err: LogicError) -> Self {
        MailSendError::Smtp(err)
    }
}

impl From<std_io::Error> for MailSendError {
    fn from(err: std_io::Error) -> Self {
        MailSendError::Io(err)
    }
}

impl From<ConnectingFailed> for MailSendError {
    fn from(err: ConnectingFailed) -> Self {
        MailSendError::Connecting(err)
    }
}

impl From<GeneralError> for MailSendError {
    fn from(err: GeneralError) -> Self {
        use self::GeneralError::*;
        match err {
            Connecting(err) => Self::from(err),
            Cmd(err) => Self::from(err),
            Io(err) => Self::from(err),
        }
    }
}

#[derive(Debug, Fail)]
pub enum OtherValidationError {
    #[fail(display = "no To header was present")]
    NoTo,
}

impl From<OtherValidationError> for HeaderValidationError {
    fn from(ove: OtherValidationError) -> Self {
        HeaderValidationError::Custom(ove.into())
    }
}

impl From<OtherValidationError> for MailError {
    fn from(ove: OtherValidationError) -> Self {
        MailError::from(HeaderValidationError::from(ove))
    }
}
