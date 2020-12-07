use media_type::BOUNDARY;
use soft_ascii_string::{SoftAsciiChar, SoftAsciiStr, SoftAsciiString};

use headers::{
    headers::{ContentTransferEncoding, ContentType},
    HeaderKind, HeaderName, HeaderObj, HeaderObjTrait,
};
use internals::{
    encoder::{EncodingBuffer, EncodingWriter},
    error::{EncodingError, EncodingErrorKind, Place, US_ASCII, UTF_8},
};

use {
    error::MailError,
    mail::{assume_encoded, EncodableMail, Mail},
};

///
/// # Panics
/// if the body is not yet resolved use `Body::poll_body` or `IntoFuture`
/// on `Mail` to prevent this from happening
///
#[inline(always)]
pub(crate) fn encode_mail(
    mail: &EncodableMail,
    top: bool,
    encoder: &mut EncodingBuffer,
) -> Result<(), MailError> {
    _encode_mail(&*mail, top, encoder).map_err(|err| {
        let mail_type = encoder.mail_type();
        use self::MailError::*;

        match err {
            Encoding(enc_err) => Encoding(enc_err.with_mail_type_or_else(|| Some(mail_type))),
            other => other,
        }
    })
}

fn _encode_mail(mail: &Mail, top: bool, encoder: &mut EncodingBuffer) -> Result<(), MailError> {
    encode_headers(&mail, top, encoder)?;

    //the empty line between the headers and the body
    encoder.write_blank_line();

    encode_mail_part(&mail, encoder)?;

    Ok(())
}

///
/// # Panics
/// if the body is not yet resolved use `Body::poll_body` or `IntoFuture`
/// on `Mail` to prevent this from happening
///
fn encode_headers(mail: &Mail, top: bool, encoder: &mut EncodingBuffer) -> Result<(), MailError> {
    use super::MailBody::*;

    let mut handle = encoder.writer();
    if top {
        handle.write_str(SoftAsciiStr::from_unchecked("MIME-Version: 1.0"))?;
        handle.finish_header();
    }

    for (name, hbody) in mail.headers().iter() {
        let name_as_str = name.as_str();
        let ignored_header =
            !top && !(name_as_str.starts_with("Content-") || name_as_str.starts_with("X-"));

        if ignored_header {
            warn!(
                "non `Content-` header in MIME body: {:?}: {:?}",
                name, hbody
            );
        }

        encode_header(&mut handle, name, hbody)?;
    }

    match mail.body() {
        SingleBody { ref body } => {
            let data = assume_encoded(body);
            let header = ContentTransferEncoding::body(data.encoding());
            encode_header(&mut handle, header.name(), &header)?;
            let header = ContentType::body(data.media_type().clone());
            encode_header(&mut handle, header.name(), &header)?;
        }
        MultipleBodies {
            hidden_text: _,
            bodies: _,
        } => {}
    }
    Ok(())
}

fn encode_header(
    handle: &mut EncodingWriter,
    name: HeaderName,
    header: &HeaderObj,
) -> Result<(), EncodingError> {
    //FIXME[rust/catch] use catch block
    let res = (|| -> Result<(), EncodingError> {
        handle.write_str(name.as_ascii_str())?;
        handle.write_char(SoftAsciiChar::from_unchecked(':'))?;
        handle.write_fws();
        header.encode(handle)?;
        handle.finish_header();
        Ok(())
    })();

    res.map_err(|err| {
        err.with_place_or_else(|| {
            Some(Place::Header {
                name: name.as_str(),
            })
        })
    })
}

///
/// # Panics
/// if the body is not yet resolved use `Body::poll_body` or `IntoFuture`
/// on `Mail` to prevent this from happening
///
fn encode_mail_part(mail: &Mail, encoder: &mut EncodingBuffer) -> Result<(), MailError> {
    use super::MailBody::*;

    let minus = SoftAsciiChar::from_unchecked('-');

    match mail.body() {
        SingleBody { ref body } => {
            let data = assume_encoded(body);
            let buffer = data.transfer_encoded_buffer();
            encoder.write_body_unchecked(buffer);
        }
        MultipleBodies {
            ref hidden_text,
            ref bodies,
        } => {
            if hidden_text.len() > 0 {
                //TODO find out if there is any source using the hidden text
                // (e.g. for some form of validation, prove of senders validity etc.)
                // if not drop the "hidden_text" field
                warn!("\"hidden text\" in multipart bodies is dropped")
            }

            let mail_was_validated_err_msg = "[BUG] mail was already validated";
            let boundary = mail
                .headers()
                .get_single(ContentType)
                .expect(mail_was_validated_err_msg)
                .expect(mail_was_validated_err_msg)
                .get_param(BOUNDARY)
                .expect(mail_was_validated_err_msg)
                .to_content();

            let boundary = SoftAsciiString::from_string(boundary).map_err(|orig_string| {
                EncodingError::from(EncodingErrorKind::InvalidTextEncoding {
                    got_encoding: UTF_8,
                    expected_encoding: US_ASCII,
                })
                .with_place_or_else(|| {
                    Some(Place::Header {
                        name: "Content-Type",
                    })
                })
                .with_str_context(orig_string.into_source())
            })?;

            for mail in bodies.iter() {
                encoder.write_header_line(|handle| {
                    handle.write_char(minus)?;
                    handle.write_char(minus)?;
                    handle.write_str(&*boundary)
                })?;
                _encode_mail(mail, false, encoder)?;
            }

            if bodies.len() > 0 {
                encoder.write_header_line(|handle| {
                    handle.write_char(minus)?;
                    handle.write_char(minus)?;
                    handle.write_str(&*boundary)?;
                    handle.write_char(minus)?;
                    handle.write_char(minus)
                })?;
            }
        }
    }
    Ok(())
}
