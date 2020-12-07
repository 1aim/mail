//! This module provides the encoding buffer.
//!
//! The encoding buffer is the buffer header implementations
//! write there data to. It provides a view special aspects
//! to make it more robust.
//!
//! For example it handles the writing of trailing newlines for headers
//! (if they don't do it) and it fails if you write utf-8 data
//! (in the header) to an buffer which knows that the used mail
//! type doesn't support it.
//!
//! There is also a special `tracing` (cargo) feature which
//! will make it's usage slower, but which will keep track of
//! what data was inserted in which way making debugging and
//! writing tests easier. (Through it should _only_ be enabled
//! for testing and maybe debugging in some cases).
use std::borrow::Cow;
use std::str;

use failure::Fail;
use soft_ascii_string::{SoftAsciiChar, SoftAsciiStr};

use error::{EncodingError, EncodingErrorKind, UNKNOWN, US_ASCII, UTF_8};
use grammar::is_atext;
use utils::{is_utf8_continuation_byte, vec_insert_bytes};
use MailType;

#[cfg_attr(test, macro_use)]
mod encodable;
#[cfg(feature = "traceing")]
#[cfg_attr(test, macro_use)]
mod trace;

pub use self::encodable::*;
#[cfg(feature = "traceing")]
pub use self::trace::*;

/// as specified in RFC 5322 not including CRLF
pub const LINE_LEN_SOFT_LIMIT: usize = 78;
/// as specified in RFC 5322 (mail) + RFC 5321 (smtp) not including CRLF
pub const LINE_LEN_HARD_LIMIT: usize = 998;

pub const NEWLINE: &str = "\r\n";
pub const NEWLINE_WITH_SPACE: &str = "\r\n ";

/// EncodingBuffer for a Mail providing a buffer for encodable traits.
pub struct EncodingBuffer {
    mail_type: MailType,
    buffer: Vec<u8>,
    #[cfg(feature = "traceing")]
    pub trace: Vec<TraceToken>,
}

impl EncodingBuffer {
    /// Create a new buffer only allowing input compatible with a the specified mail type.
    pub fn new(mail_type: MailType) -> Self {
        EncodingBuffer {
            mail_type,
            buffer: Vec::new(),
            #[cfg(feature = "traceing")]
            trace: Vec::new(),
        }
    }

    /// Returns the mail type for which the buffer was created.
    pub fn mail_type(&self) -> MailType {
        self.mail_type
    }

    /// returns a new EncodingWriter which contains
    /// a mutable reference to the current string buffer
    ///
    pub fn writer(&mut self) -> EncodingWriter {
        #[cfg(not(feature = "traceing"))]
        {
            EncodingWriter::new(self.mail_type, &mut self.buffer)
        }
        #[cfg(feature = "traceing")]
        {
            EncodingWriter::new(self.mail_type, &mut self.buffer, &mut self.trace)
        }
    }

    /// calls the provided function with a EncodingWriter cleaning up afterwards
    ///
    /// After calling `func` with the EncodingWriter following cleanup is performed:
    /// - if `func` returned an error `handle.undo_header()` is called, this won't
    ///   undo anything before a `finish_header()` call but will discard partial
    ///   writes
    /// - if `func` succeeded `handle.finish_header()` is called
    pub fn write_header_line<FN>(&mut self, func: FN) -> Result<(), EncodingError>
    where
        FN: FnOnce(&mut EncodingWriter) -> Result<(), EncodingError>,
    {
        let mut handle = self.writer();
        match func(&mut handle) {
            Ok(()) => {
                handle.finish_header();
                Ok(())
            }
            Err(e) => {
                handle.undo_header();
                Err(e)
            }
        }
    }

    pub fn write_blank_line(&mut self) {
        //TODO/BENCH push_str vs. extends(&[u8])
        self.buffer.extend(NEWLINE.as_bytes());
        #[cfg(feature = "traceing")]
        {
            self.trace.push(TraceToken::BlankLine);
        }
    }

    /// writes a body to the internal buffer, without verifying it's correctness
    pub fn write_body_unchecked(&mut self, body: &impl AsRef<[u8]>) {
        let slice = body.as_ref();
        self.buffer.extend(slice);
        if !slice.ends_with(NEWLINE.as_bytes()) {
            self.buffer.extend(NEWLINE.as_bytes());
        }
    }

    //TODO impl. a alt. `write_body(body,  boundaries)` which:
    // - checks the body (us-ascii or mime8bit/internationalized)
    // - checks for orphan '\r'/'\n' and 0 bytes
    // - check that no string in boundaries appears in the text
    //   - this probably requires creating a regex for each body
    //     through as boundaries are "fixed" there might be an more
    //     efficient algorithm then a regex (i.e. using tries)

    /// # Error
    ///
    /// This can fail if a body does not contain valid utf8.
    pub fn as_str(&self) -> Result<&str, EncodingError> {
        str::from_utf8(self.buffer.as_slice()).map_err(|err| {
            EncodingError::from((
                err.context(EncodingErrorKind::InvalidTextEncoding {
                    expected_encoding: UTF_8,
                    got_encoding: UNKNOWN,
                }),
                self.mail_type(),
            ))
        })
    }

    /// Converts the internal buffer into an utf-8 string if possible.
    pub fn to_string(&self) -> Result<String, EncodingError> {
        Ok(self.as_str()?.to_owned())
    }

    /// Lossy conversion of the internal buffer to an string.
    pub fn to_string_lossy(&self) -> Cow<str> {
        String::from_utf8_lossy(self.buffer.as_slice())
    }

    /// Return a slice view to the underlying buffer.
    pub fn as_slice(&self) -> &[u8] {
        &self.buffer
    }
}

impl Into<Vec<u8>> for EncodingBuffer {
    fn into(self) -> Vec<u8> {
        self.buffer
    }
}

impl Into<(MailType, Vec<u8>)> for EncodingBuffer {
    fn into(self) -> (MailType, Vec<u8>) {
        (self.mail_type, self.buffer)
    }
}

#[cfg(feature = "traceing")]
impl Into<(MailType, Vec<u8>, Vec<TraceToken>)> for EncodingBuffer {
    fn into(self) -> (MailType, Vec<u8>, Vec<TraceToken>) {
        let EncodingBuffer {
            mail_type,
            buffer,
            trace,
        } = self;
        (mail_type, buffer, trace)
    }
}

/// A handle providing method to write to the underlying buffer
/// keeping track of newlines the current line length and places
/// where the line can be broken so that the soft line length
/// limit (78) and the hard length limit (998) can be kept.
///
/// It's basically a string buffer which know how to brake
/// lines at the right place.
///
/// Note any act of writing a header through `EncodingWriter`
/// has to be concluded by either calling `finish_header` or `undo_header`.
/// If not this handle will panic in _test_ builds when being dropped
/// (and the thread is not already panicing) as writes through the handle are directly
/// writes to the underlying buffer which now contains malformed/incomplete
/// data. (Note that this Handle does not own any Drop types so if
/// needed `forget`-ing it won't leak any memory)
///
///
pub struct EncodingWriter<'a> {
    buffer: &'a mut Vec<u8>,
    #[cfg(feature = "traceing")]
    trace: &'a mut Vec<TraceToken>,
    mail_type: MailType,
    line_start_idx: usize,
    last_fws_idx: usize,
    skipped_cr: bool,
    /// if there had ben non WS chars since the last FWS
    /// or last line start, if there had been a line
    /// start since the last fws.
    content_since_fws: bool,
    /// represents if there had ben non WS chars before the last FWS
    /// on the current line (false if there was no FWS yet on the current
    /// line).
    content_before_fws: bool,
    /// represents if if a FWS was just marked (opt-FWS) or was written out
    last_fws_has_char: bool,
    header_start_idx: usize,
    #[cfg(feature = "traceing")]
    trace_start_idx: usize,
}

#[cfg(feature = "traceing")]
impl<'a> Drop for EncodingWriter<'a> {
    fn drop(&mut self) {
        use std::thread;
        if !thread::panicking() && self.has_unfinished_parts() {
            // we really should panic as the back buffer i.e. the mail will contain
            // some partially written header which definitely is a bug
            panic!("dropped Handle which partially wrote header to back buffer (use `finish_header` or `discard`)")
        }
    }
}

impl<'inner> EncodingWriter<'inner> {
    #[cfg(not(feature = "traceing"))]
    fn new(mail_type: MailType, buffer: &'inner mut Vec<u8>) -> Self {
        let start_idx = buffer.len();
        EncodingWriter {
            buffer,
            mail_type,
            line_start_idx: start_idx,
            last_fws_idx: start_idx,
            skipped_cr: false,
            content_since_fws: false,
            content_before_fws: false,
            header_start_idx: start_idx,
            last_fws_has_char: false,
        }
    }

    #[cfg(feature = "traceing")]
    fn new(
        mail_type: MailType,
        buffer: &'inner mut Vec<u8>,
        trace: &'inner mut Vec<TraceToken>,
    ) -> Self {
        let start_idx = buffer.len();
        let trace_start_idx = trace.len();
        EncodingWriter {
            buffer,
            trace,
            mail_type,
            line_start_idx: start_idx,
            last_fws_idx: start_idx,
            skipped_cr: false,
            content_since_fws: false,
            content_before_fws: false,
            header_start_idx: start_idx,
            last_fws_has_char: false,
            trace_start_idx,
        }
    }

    fn reinit(&mut self) {
        let start_idx = self.buffer.len();
        self.line_start_idx = start_idx;
        self.last_fws_idx = start_idx;
        self.skipped_cr = false;
        self.content_since_fws = false;
        self.content_before_fws = false;
        self.header_start_idx = start_idx;
        #[cfg(feature = "traceing")]
        {
            self.trace_start_idx = self.trace.len();
        }
    }

    /// Returns true if this type thinks we are in the process of writing a header.
    #[inline]
    pub fn has_unfinished_parts(&self) -> bool {
        self.buffer.len() != self.header_start_idx
    }

    /// Returns the associated mail type.
    #[inline]
    pub fn mail_type(&self) -> MailType {
        self.mail_type
    }

    /// Returns true if the current line has content, i.e. any non WS char.
    #[inline]
    pub fn line_has_content(&self) -> bool {
        self.content_before_fws | self.content_since_fws
    }

    /// Returns the length of the current line in bytes.
    #[inline]
    pub fn current_line_byte_length(&self) -> usize {
        self.buffer.len() - self.line_start_idx
    }

    /// marks the current position a a place where a soft
    /// line break (i.e. "\r\n ") can be inserted
    ///
    /// # Trace (test build only)
    /// does push a `MarkFWS` Token
    pub fn mark_fws_pos(&mut self) {
        #[cfg(feature = "traceing")]
        {
            self.trace.push(TraceToken::MarkFWS)
        }
        self.content_before_fws |= self.content_since_fws;
        self.content_since_fws = false;
        self.last_fws_idx = self.buffer.len();
        self.last_fws_has_char = false;
    }

    /// writes a ascii char to the underlying buffer
    ///
    /// # Error
    /// - fails if the hard line length limit is breached and the
    ///   line can not be broken with soft line breaks
    /// - buffer would contain a orphan '\r' or '\n' after the write
    ///
    /// # Trace (test build only)
    /// does push `NowChar` and then can push `Text`,`CRLF`
    pub fn write_char(&mut self, ch: SoftAsciiChar) -> Result<(), EncodingError> {
        #[cfg(feature = "traceing")]
        {
            self.trace.push(TraceToken::NowChar)
        }
        let mut buffer = [0xff_u8; 4];
        let ch: char = ch.into();
        let slice = ch.encode_utf8(&mut buffer);
        self.internal_write_char(slice)
    }

    /// writes a ascii str to the underlying buffer
    ///
    /// # Error
    /// - fails if the hard line length limit is breached and the
    ///   line can not be broken with soft line breaks
    /// - buffer would contain a orphan '\r' or '\n' after the write
    ///
    /// Note that in case of an error part of the content might already
    /// have been written to the buffer, therefore it is recommended
    /// to call `undo_header` after an error (especially if the
    /// handle is doped after this!)
    ///
    /// # Trace (test build only)
    /// does push `NowStr` and then can push `Text`,`CRLF`
    ///
    pub fn write_str(&mut self, s: &SoftAsciiStr) -> Result<(), EncodingError> {
        #[cfg(feature = "traceing")]
        {
            self.trace.push(TraceToken::NowStr)
        }
        self.internal_write_str(s.as_str())
    }

    /// writes a utf8 str into a buffer for an internationalized mail
    ///
    /// # Error (ConditionalWriteResult)
    /// - fails with `ConditionFailure` if the underlying MailType
    ///    is not Internationalized
    /// - fails with `GeneralFailure` if the hard line length limit is reached
    /// - or if the buffer would contain a orphan '\r' or '\n' after the write
    ///
    /// Note that in case of an error part of the content might already
    /// have been written to the buffer, therefore it is recommended
    /// to call `undo_header` after an error (especially if the
    /// handle is droped after this!)
    ///
    /// # Trace (test build only)
    /// does push `NowUtf8` and then can push `Text`,`CRLF`
    pub fn write_if_utf8<'short>(
        &'short mut self,
        s: &str,
    ) -> ConditionalWriteResult<'short, 'inner> {
        if self.mail_type().is_internationalized() {
            #[cfg(feature = "traceing")]
            {
                self.trace.push(TraceToken::NowUtf8)
            }
            self.internal_write_str(s).into()
        } else {
            ConditionalWriteResult::ConditionFailure(self)
        }
    }

    pub fn write_utf8(&mut self, s: &str) -> Result<(), EncodingError> {
        if self.mail_type().is_internationalized() {
            #[cfg(feature = "traceing")]
            {
                self.trace.push(TraceToken::NowUtf8)
            }
            self.internal_write_str(s)
        } else {
            let mut err = EncodingError::from((
                EncodingErrorKind::InvalidTextEncoding {
                    expected_encoding: US_ASCII,
                    got_encoding: UTF_8,
                },
                self.mail_type(),
            ));
            let raw_line = &self.buffer[self.line_start_idx..];
            let mut line = String::from_utf8_lossy(raw_line).into_owned();
            line.push_str(s);
            err.set_str_context(line);
            Err(err)
        }
    }

    /// Writes a str assumed to be atext if it is atext given the mail type
    ///
    /// This method is mainly an optimization as the "is atext" and is
    /// "is ascii if MailType is Ascii" aspects are checked at the same
    /// time resulting in a str which you know is ascii _if_ the mail
    /// type is Ascii and which might be non-us-ascii if the mail type
    /// is Internationalized.
    ///
    /// # Error (ConditionalWriteResult)
    /// - fails with `ConditionFailure` if the text is not valid atext,
    ///   this indirectly also includes the utf8/Internationalization check
    ///   as the `atext` grammar differs between normal and internationalized
    ///   mail.
    /// - fails with `GeneralFailure` if the hard line length limit is reached and
    ///   the line can't be broken with soft line breaks
    /// - or if buffer would contain a orphan '\r' or '\n' after the write
    ///   (excluding a tailing `'\r'` as it is still valid if followed by an
    ///    `'\n'`)
    ///
    /// Note that in case of an error part of the content might already
    /// have been written to the buffer, therefore it is recommended
    /// to call `undo_header` after an error (especially if the
    /// handle is doped after this!)
    ///
    /// # Trace (test build only)
    /// does push `NowAText` and then can push `Text`
    ///
    pub fn write_if_atext<'short>(
        &'short mut self,
        s: &str,
    ) -> ConditionalWriteResult<'short, 'inner> {
        if s.chars().all(|ch| is_atext(ch, self.mail_type())) {
            #[cfg(feature = "traceing")]
            {
                self.trace.push(TraceToken::NowAText)
            }
            // the ascii or not aspect is already converted by `is_atext`
            self.internal_write_str(s).into()
        } else {
            ConditionalWriteResult::ConditionFailure(self)
        }
    }

    /// passes the input `s` to the condition evaluation function `cond` and
    /// then writes it _without additional checks_ to the buffer if `cond` returned
    /// true
    ///
    pub fn write_if<'short, FN>(
        &'short mut self,
        s: &str,
        cond: FN,
    ) -> ConditionalWriteResult<'short, 'inner>
    where
        FN: FnOnce(&str) -> bool,
    {
        if cond(s) {
            #[cfg(feature = "traceing")]
            {
                self.trace.push(TraceToken::NowCondText)
            }
            // the ascii or not aspect is already converted by `is_atext`
            self.internal_write_str(s).into()
        } else {
            ConditionalWriteResult::ConditionFailure(self)
        }
    }

    /// writes a string to the encoder without checking if it is compatible
    /// with the mail type, if not used correctly this can write Utf8 to
    /// an Ascii Mail, which is incorrect but has to be safe wrt. rust's safety.
    ///
    /// Use it as a replacement for cases similar to following:
    ///
    /// ```ignore
    /// check_if_text_if_valid(text)?;
    /// if mail_type.is_internationalized() {
    ///     handle.write_utf8(text)?;
    /// } else {
    ///     handle.write_str(SoftAsciiStr::from_unchecked(text))?;
    /// }
    /// ```
    ///
    /// ==> instead ==>
    ///
    /// ```ignore
    /// check_if_text_if_valid(text)?;
    /// handle.write_str_unchecked(text)?;
    /// ```
    ///
    /// through is gives a different tracing its roughly equivalent.
    ///
    pub fn write_str_unchecked(&mut self, s: &str) -> Result<(), EncodingError> {
        #[cfg(feature = "traceing")]
        {
            self.trace.push(TraceToken::NowUnchecked)
        }
        self.internal_write_str(s)
    }

    /// like finish_header, but won't start a new line
    ///
    /// This is meant to be used when _miss-using_ the
    /// writer to write a "think", which is not a full
    /// header. E.g. for testing if a header component
    /// is written correctly. So you _normally_ should
    /// not use it.
    pub fn commit_partial_header(&mut self) {
        #[cfg(feature = "traceing")]
        {
            if let Some(&TraceToken::End) = self.trace.last() {
            } else {
                self.trace.push(TraceToken::End)
            }
        }
        self.reinit();
    }

    /// finishes the writing of a header
    ///
    /// It makes sure the header ends in "\r\n".
    /// If the header ends in a orphan '\r' this
    /// method will just "use" it for the "\r\n".
    ///
    /// If the header ends in a CRLF/start of buffer
    /// followed by only WS (' ' or '\t' ) the valid
    /// header ending is reached by truncating away
    /// the WS padding. This is needed as "blank" lines
    /// are not allowed.
    ///
    /// # Trace (test build only)
    /// - can push 0-1 of `[CRLF, TruncateToCRLF]`
    /// - then does push `End`
    /// - calling `finish_current()` multiple times in a row
    ///   will not generate multiple `End` tokens, just one
    pub fn finish_header(&mut self) {
        self.start_new_line();
        #[cfg(feature = "traceing")]
        {
            if let Some(&TraceToken::End) = self.trace.last() {
            } else {
                self.trace.push(TraceToken::End)
            }
        }
        self.reinit();
    }

    /// undoes all writes to the internal buffer
    /// since the last `finish_header` or `undo_header` or
    /// creation of this handle
    ///
    /// # Trace (test build only)
    /// also removes tokens pushed since the last
    /// `finish_header` or `undo_header` or creation of
    /// this handle
    ///
    pub fn undo_header(&mut self) {
        self.buffer.truncate(self.header_start_idx);
        #[cfg(feature = "traceing")]
        {
            self.trace.truncate(self.trace_start_idx);
        }
        self.reinit();
    }

    //---------------------------------------------------------------------------------------------/
    //-/////////////////////////// methods only using the public iface   /////////////////////////-/

    /// calls mark_fws_pos and then writes a space
    ///
    /// This method exists for convenience.
    ///
    /// Note that it can not fail a you just pushed
    /// a place to brake the line before writing a space.
    ///
    /// Note that currently soft line breaks will not
    /// collapse whitespace. As such if you use `write_fws`
    /// and then the line is broken at that position it will
    /// start with two spaces (one from `\r\n ` and one which
    /// had been there before).
    pub fn write_fws(&mut self) {
        self.mark_fws_pos();
        self.last_fws_has_char = true;
        // OK: Can not error as we just marked a fws pos.
        let _ = self.write_char(SoftAsciiChar::from_unchecked(' '));
    }

    //---------------------------------------------------------------------------------------------/
    //-///////////////////////////          private methods               ////////////////////////-/

    /// this might partial write some data and then fail.
    /// while we could implement a undo option it makes
    /// little sense for the use case the generally available
    /// `undo_header` is enough.
    fn internal_write_str(&mut self, s: &str) -> Result<(), EncodingError> {
        if s.is_empty() {
            return Ok(());
        }
        //TODO I think I wrote a iterator for this somewhere
        let mut start = 0;
        // the first byte is never a continuation byte so we start
        // scanning at the second byte
        for (idx_m1, bch) in s.as_bytes()[1..].iter().enumerate() {
            if !is_utf8_continuation_byte(*bch) {
                // the idx is 1 smaller then it should so add 1
                let end = idx_m1 + 1;
                self.internal_write_char(&s[start..end])?;
                start = end;
            }
        }

        //write last letter
        self.internal_write_char(&s[start..])?;
        Ok(())
    }

    /// if the line has at last one non-WS char a new line
    /// will be started by adding `\r\n` if the current line
    /// only consists of WS then a new line will be started by
    /// removing the blank line (not that WS are only ' ' and '\r')
    fn start_new_line(&mut self) {
        if self.line_has_content() {
            #[cfg(feature = "traceing")]
            {
                self.trace.push(TraceToken::CRLF)
            }

            self.buffer.push(b'\r');
            self.buffer.push(b'\n');
        } else {
            #[cfg(feature = "traceing")]
            {
                if self.buffer.len() > self.line_start_idx {
                    self.trace.push(TraceToken::TruncateToCRLF);
                }
            }
            // e.g. if we "broke" the line on a tailing space => "\r\n  "
            // this would not be valid so we cut awy the trailing white space
            // be if we have "ab  " we do not want to cut away the trailing
            // whitespace but just add "\r\n"
            self.buffer.truncate(self.line_start_idx);
        }
        self.line_start_idx = self.buffer.len();
        self.content_since_fws = false;
        self.content_before_fws = false;
        self.last_fws_idx = self.line_start_idx;
    }

    fn break_line_on_fws(&mut self) -> bool {
        if self.content_before_fws && self.last_fws_idx > self.line_start_idx {
            let newline = if self.last_fws_has_char {
                debug_assert!([b' ', b'\t'].contains(&self.buffer[self.last_fws_idx]));
                NEWLINE
            } else {
                NEWLINE_WITH_SPACE
            };

            vec_insert_bytes(&mut self.buffer, self.last_fws_idx, newline.as_bytes());
            self.line_start_idx = self.last_fws_idx + 2;
            self.content_before_fws = false;
            true
        } else {
            false
        }
    }

    /// # Constraints
    ///
    /// `unchecked_utf8_char` is expected to be exactly
    /// one char, which means it's 1-4 bytes in length.
    ///
    /// The reason why a slice is expected instead of a
    /// char is, that this function will at some point push
    /// to a byte buffer requiring a `&[u8]` and many function
    /// calling this function can directly produce a &[u8]/&str.
    ///
    /// # Panic
    ///
    /// Panics if `unchecked_utf8_char` is empty.
    /// If debug assertions are enabled it also panics, if
    /// unchecked_utf8_char is more than just one char.
    fn internal_write_char(&mut self, unchecked_utf8_char: &str) -> Result<(), EncodingError> {
        debug_assert_eq!(unchecked_utf8_char.chars().count(), 1);

        let bch = unchecked_utf8_char.as_bytes()[0];
        if bch == b'\n' {
            if self.skipped_cr {
                self.start_new_line()
            } else {
                ec_bail!(
                    mail_type: self.mail_type(),
                    kind: Malformed
                );
            }
            self.skipped_cr = false;
            return Ok(());
        } else {
            if self.skipped_cr {
                ec_bail!(
                    mail_type: self.mail_type(),
                    kind: Malformed
                );
            }
            if bch == b'\r' {
                self.skipped_cr = true;
                return Ok(());
            } else {
                self.skipped_cr = false;
            }
        }

        if self.current_line_byte_length() >= LINE_LEN_SOFT_LIMIT {
            self.break_line_on_fws();

            if self.current_line_byte_length() >= LINE_LEN_HARD_LIMIT {
                ec_bail!(
                    mail_type: self.mail_type(),
                    kind: HardLineLengthLimitBreached
                );
            }
        }

        self.buffer.extend(unchecked_utf8_char.as_bytes());
        #[cfg(feature = "traceing")]
        {
            //FIXME[rust/nll]: just use a `if let`-`else` with NLL's
            let need_new =
                if let Some(&mut TraceToken::Text(ref mut string)) = self.trace.last_mut() {
                    string.push_str(unchecked_utf8_char);
                    false
                } else {
                    true
                };
            if need_new {
                let mut string = String::new();
                string.push_str(unchecked_utf8_char);
                self.trace.push(TraceToken::Text(string))
            }
        }

        // we can't allow "blank" lines
        if bch != b' ' && bch != b'\t' {
            // if there is no fws this is equiv to line_has_content
            // else line_has_content = self.content_before_fws|self.content_since_fws
            self.content_since_fws = true;
        }
        Ok(())
    }
}

pub enum ConditionalWriteResult<'a, 'b: 'a> {
    Ok,
    ConditionFailure(&'a mut EncodingWriter<'b>),
    GeneralFailure(EncodingError),
}

impl<'a, 'b: 'a> From<Result<(), EncodingError>> for ConditionalWriteResult<'a, 'b> {
    fn from(v: Result<(), EncodingError>) -> Self {
        match v {
            Ok(()) => ConditionalWriteResult::Ok,
            Err(e) => ConditionalWriteResult::GeneralFailure(e),
        }
    }
}

impl<'a, 'b: 'a> ConditionalWriteResult<'a, 'b> {
    #[inline]
    pub fn handle_condition_failure<FN>(self, func: FN) -> Result<(), EncodingError>
    where
        FN: FnOnce(&mut EncodingWriter) -> Result<(), EncodingError>,
    {
        use self::ConditionalWriteResult as CWR;

        match self {
            CWR::Ok => Ok(()),
            CWR::ConditionFailure(handle) => func(handle),
            CWR::GeneralFailure(err) => Err(err),
        }
    }
}

#[cfg(test)]
mod test {

    use error::EncodingErrorKind;
    use soft_ascii_string::{SoftAsciiChar, SoftAsciiStr};
    use MailType;

    use super::EncodingBuffer as _Encoder;
    use super::TraceToken::*;

    mod test_test_utilities {
        use super::super::simplify_trace_tokens;
        use encoder::TraceToken::*;

        #[test]
        fn does_simplify_tokens_strip_nows() {
            let inp = vec![
                NowChar,
                Text("h".into()),
                CRLF,
                NowStr,
                Text("y yo".into()),
                CRLF,
                NowUtf8,
                Text(", what's".into()),
                CRLF,
                NowUnchecked,
                Text("up!".into()),
                CRLF,
                NowAText,
                Text("abc".into()),
            ];
            let out = simplify_trace_tokens(inp);
            assert_eq!(
                out,
                vec![
                    Text("h".into()),
                    CRLF,
                    Text("y yo".into()),
                    CRLF,
                    Text(", what's".into()),
                    CRLF,
                    Text("up!".into()),
                    CRLF,
                    Text("abc".into())
                ]
            )
        }

        #[test]
        fn simplify_does_collapse_text() {
            let inp = vec![
                NowChar,
                Text("h".into()),
                NowStr,
                Text("y yo".into()),
                NowUtf8,
                Text(", what's".into()),
                NowUnchecked,
                Text(" up! ".into()),
                NowAText,
                Text("abc".into()),
            ];
            let out = simplify_trace_tokens(inp);
            assert_eq!(out, vec![Text("hy yo, what's up! abc".into())]);
        }

        #[test]
        fn simplify_works_with_empty_text() {
            let inp = vec![NowStr, Text("".into()), CRLF];
            assert_eq!(simplify_trace_tokens(inp), vec![Text("".into()), CRLF])
        }

        #[test]
        fn simplify_works_with_trailing_empty_text() {
            let inp = vec![Text("a".into()), CRLF, Text("".into())];
            assert_eq!(
                simplify_trace_tokens(inp),
                vec![Text("a".into()), CRLF, Text("".into())]
            )
        }
    }

    mod EncodableInHeader {
        #![allow(non_snake_case)]
        use self::TraceToken::*;
        use super::super::*;

        #[test]
        fn is_implemented_for_closures() {
            let closure = enc_func!(|handle: &mut EncodingWriter| { handle.write_utf8("hy ho") });

            let mut encoder = EncodingBuffer::new(MailType::Internationalized);
            {
                let mut handle = encoder.writer();
                assert_ok!(closure.encode(&mut handle));
                handle.finish_header();
            }
            assert_eq!(
                encoder.trace.as_slice(),
                &[NowUtf8, Text("hy ho".into()), CRLF, End]
            )
        }
    }

    mod EncodingBuffer {
        #![allow(non_snake_case)]
        use super::_Encoder as EncodingBuffer;
        use super::*;

        #[test]
        fn new_encoder() {
            let encoder = EncodingBuffer::new(MailType::Internationalized);
            assert_eq!(encoder.mail_type(), MailType::Internationalized);
        }

        #[test]
        fn write_body_unchecked() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            let body1 = "una body\r\n";
            let body2 = "another body";

            encoder.write_body_unchecked(&body1);
            encoder.write_blank_line();
            encoder.write_body_unchecked(&body2);

            assert_eq!(
                encoder.as_slice(),
                concat!("una body\r\n", "\r\n", "another body\r\n").as_bytes()
            )
        }
    }

    mod EncodingWriter {
        #![allow(non_snake_case)]
        use std::mem;
        use std::str;

        use super::_Encoder as EncodingBuffer;
        use super::*;

        #[test]
        fn commit_partial_and_drop_does_not_panic() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            {
                let mut handle = encoder.writer();
                assert_ok!(handle.write_str(SoftAsciiStr::from_unchecked("12")));
                handle.commit_partial_header();
            }
            assert_eq!(encoder.as_slice(), b"12");
        }

        #[test]
        fn undo_does_undo() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            {
                let mut handle = encoder.writer();
                assert_ok!(handle.write_str(SoftAsciiStr::from_unchecked("Header-One: 12")));
                handle.undo_header();
            }
            assert_eq!(encoder.as_slice(), b"");
        }

        #[test]
        fn undo_does_not_undo_to_much() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            {
                let mut handle = encoder.writer();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str("Header-One: 12").unwrap()));
                handle.finish_header();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str("ups: sa").unwrap()));
                handle.undo_header();
            }
            assert_eq!(encoder.as_slice(), b"Header-One: 12\r\n");
        }

        #[test]
        fn finish_adds_crlf_if_needed() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            {
                let mut handle = encoder.writer();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str("Header-One: 12").unwrap()));
                handle.finish_header();
            }
            assert_eq!(encoder.as_slice(), b"Header-One: 12\r\n");
        }

        #[test]
        fn finish_does_not_add_crlf_if_not_needed() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            {
                let mut handle = encoder.writer();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str("Header-One: 12\r\n").unwrap()));
                handle.finish_header();
            }
            assert_eq!(encoder.as_slice(), b"Header-One: 12\r\n");
        }

        #[test]
        fn finish_does_truncat_if_needed() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            {
                let mut handle = encoder.writer();
                assert_ok!(
                    handle.write_str(SoftAsciiStr::from_str("Header-One: 12\r\n   ").unwrap())
                );
                handle.finish_header();
            }
            assert_eq!(encoder.as_slice(), b"Header-One: 12\r\n");
        }

        #[test]
        fn finish_can_handle_fws() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            {
                let mut handle = encoder.writer();
                assert_ok!(
                    handle.write_str(SoftAsciiStr::from_str("Header-One: 12 +\r\n 4").unwrap())
                );
                handle.finish_header();
            }
            assert_eq!(encoder.as_slice(), b"Header-One: 12 +\r\n 4\r\n");
        }

        #[test]
        fn finish_only_truncats_if_needed() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            {
                let mut handle = encoder.writer();
                assert_ok!(
                    handle.write_str(SoftAsciiStr::from_str("Header-One: 12 +\r\n 4  ").unwrap())
                );
                handle.finish_header();
            }
            assert_eq!(encoder.as_slice(), b"Header-One: 12 +\r\n 4  \r\n");
        }

        #[test]
        fn orphan_lf_error() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            {
                let mut handle = encoder.writer();
                assert_err!(handle.write_str(SoftAsciiStr::from_str("H: \na").unwrap()));
                handle.undo_header()
            }
        }
        #[test]
        fn orphan_cr_error() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            {
                let mut handle = encoder.writer();
                assert_err!(handle.write_str(SoftAsciiStr::from_str("H: \ra").unwrap()));
                handle.undo_header()
            }
        }

        #[test]
        fn orphan_trailing_lf() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            {
                let mut handle = encoder.writer();
                assert_err!(handle.write_str(SoftAsciiStr::from_str("H: a\n").unwrap()));
                handle.undo_header();
            }
        }

        #[test]
        fn orphan_trailing_cr() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            {
                let mut handle = encoder.writer();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str("H: a\r").unwrap()));
                //it's fine not to error in the trailing \r case as we want to write
                //a \r\n anyway
                handle.finish_header();
            }
            assert_eq!(encoder.as_slice(), b"H: a\r\n");
        }

        #[test]
        fn soft_line_limit_can_be_breached() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            {
                let mut handle = encoder.writer();
                for _ in 0u32..500 {
                    assert_ok!(handle.internal_write_char("a"));
                }
                handle.finish_header();
            }
        }

        #[test]
        fn hard_line_limit_can_not_be_breached() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            {
                let mut handle = encoder.writer();
                for _ in 0u32..998 {
                    assert_ok!(handle.internal_write_char("a"));
                }

                assert_err!(handle.internal_write_char("b"));
                handle.finish_header();
            }
        }

        #[test]
        fn break_line_on_fws() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            {
                let mut handle = encoder.writer();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str("A23456789:").unwrap()));
                handle.mark_fws_pos();
                assert_ok!(handle.write_str(
                    SoftAsciiStr::from_str(concat!(
                        "20_3456789",
                        "30_3456789",
                        "40_3456789",
                        "50_3456789",
                        "60_3456789",
                        "70_3456789",
                        "12345678XX"
                    ))
                    .unwrap()
                ));
                handle.finish_header();
            }
            assert_eq!(
                encoder.as_str().unwrap(),
                concat!(
                    "A23456789:\r\n ",
                    "20_3456789",
                    "30_3456789",
                    "40_3456789",
                    "50_3456789",
                    "60_3456789",
                    "70_3456789",
                    "12345678XX\r\n"
                )
            );
        }

        #[test]
        fn break_line_on_fws_does_not_insert_unessesary_space() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            {
                let mut handle = encoder.writer();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str("A23456789:").unwrap()));
                handle.write_fws();
                assert_ok!(handle.write_str(
                    SoftAsciiStr::from_str(concat!(
                        "20_3456789",
                        "30_3456789",
                        "40_3456789",
                        "50_3456789",
                        "60_3456789",
                        "70_3456789",
                        "12345678XX"
                    ))
                    .unwrap()
                ));
                handle.finish_header();
            }

            assert_eq!(
                encoder.as_str().unwrap(),
                concat!(
                    "A23456789:\r\n ",
                    "20_3456789",
                    "30_3456789",
                    "40_3456789",
                    "50_3456789",
                    "60_3456789",
                    "70_3456789",
                    "12345678XX\r\n"
                )
            );
        }

        #[test]
        fn to_long_unbreakable_line() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            {
                let mut handle = encoder.writer();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str("A23456789:").unwrap()));
                handle.mark_fws_pos();
                assert_ok!(handle.write_str(
                    SoftAsciiStr::from_str(concat!(
                        "10_3456789",
                        "20_3456789",
                        "30_3456789",
                        "40_3456789",
                        "50_3456789",
                        "60_3456789",
                        "70_3456789",
                        "80_3456789",
                        "90_3456789",
                        "00_3456789",
                    ))
                    .unwrap()
                ));
                handle.finish_header();
            }
            assert_eq!(
                encoder.as_str().unwrap(),
                concat!(
                    "A23456789:\r\n ",
                    "10_3456789",
                    "20_3456789",
                    "30_3456789",
                    "40_3456789",
                    "50_3456789",
                    "60_3456789",
                    "70_3456789",
                    "80_3456789",
                    "90_3456789",
                    "00_3456789\r\n",
                )
            );
        }

        #[test]
        fn multiple_lines_breaks() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            {
                let mut handle = encoder.writer();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str("A23456789:").unwrap()));
                handle.mark_fws_pos();
                assert_ok!(handle.write_str(
                    SoftAsciiStr::from_str(concat!(
                        "10_3456789",
                        "20_3456789",
                        "30_3456789",
                        "40_3456789",
                        "50_3456789",
                        "60_3456789",
                        "70_3456789",
                    ))
                    .unwrap()
                ));
                handle.mark_fws_pos();
                assert_ok!(handle.write_str(
                    SoftAsciiStr::from_str(concat!(
                        "10_3456789",
                        "20_3456789",
                        "30_3456789",
                        "40_3456789",
                    ))
                    .unwrap()
                ));
                handle.finish_header();
            }
            assert_eq!(
                encoder.as_str().unwrap(),
                concat!(
                    "A23456789:\r\n ",
                    "10_3456789",
                    "20_3456789",
                    "30_3456789",
                    "40_3456789",
                    "50_3456789",
                    "60_3456789",
                    "70_3456789\r\n ",
                    "10_3456789",
                    "20_3456789",
                    "30_3456789",
                    "40_3456789\r\n",
                )
            );
        }

        #[test]
        fn hard_line_limit() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            {
                let mut handle = encoder.writer();
                for x in 0..998 {
                    if let Err(_) = handle.write_char(SoftAsciiChar::from_unchecked('X')) {
                        panic!("error when writing char nr.: {:?}", x + 1)
                    }
                }
                let res = &[
                    handle
                        .write_char(SoftAsciiChar::from_unchecked('X'))
                        .is_err(),
                    handle
                        .write_char(SoftAsciiChar::from_unchecked('X'))
                        .is_err(),
                    handle
                        .write_char(SoftAsciiChar::from_unchecked('X'))
                        .is_err(),
                    handle
                        .write_char(SoftAsciiChar::from_unchecked('X'))
                        .is_err(),
                ];
                assert_eq!(res, &[true, true, true, true]);
                handle.undo_header();
            }
        }

        #[test]
        fn write_utf8_fail_on_ascii_mail() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            {
                let mut handle = encoder.writer();
                assert_err!(handle.write_utf8("↓"));
                handle.undo_header();
            }
        }

        #[test]
        fn write_utf8_ascii_string_fail_on_ascii_mail() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            {
                let mut handle = encoder.writer();
                assert_err!(handle.write_utf8("just_ascii"));
                handle.undo_header();
            }
        }

        #[test]
        fn write_utf8_ok_on_internationalized_mail() {
            let mut encoder = EncodingBuffer::new(MailType::Internationalized);
            {
                let mut handle = encoder.writer();
                assert_ok!(handle.write_utf8("❤"));
                handle.finish_header();
            }
            assert_eq!(encoder.as_str().unwrap(), "❤\r\n");
        }

        #[test]
        fn try_write_atext_ascii() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            {
                let mut handle = encoder.writer();
                assert_ok!(handle
                    .write_if_atext("hoho")
                    .handle_condition_failure(|_| panic!("no condition failur expected")));
                let mut had_cond_failure = false;
                assert_ok!(handle.write_if_atext("a(b").handle_condition_failure(|_| {
                    had_cond_failure = true;
                    Ok(())
                }));
                assert!(had_cond_failure);
                assert_ok!(handle
                    .write_if_atext("")
                    .handle_condition_failure(|_| panic!("no condition failur expected")));
                handle.finish_header();
            }
            assert_eq!(encoder.as_slice(), b"hoho\r\n");
        }

        #[test]
        fn try_write_atext_internationalized() {
            let mut encoder = EncodingBuffer::new(MailType::Internationalized);
            {
                let mut handle = encoder.writer();
                assert_ok!(handle
                    .write_if_atext("hoho")
                    .handle_condition_failure(|_| panic!("no condition failur expected")));
                let mut had_cond_failure = false;
                assert_ok!(handle.write_if_atext("a(b").handle_condition_failure(|_| {
                    had_cond_failure = true;
                    Ok(())
                }));
                assert!(had_cond_failure);
                assert_ok!(handle
                    .write_if_atext("❤")
                    .handle_condition_failure(|_| panic!("no condition failur expected")));
                handle.finish_header();
            }
            assert_eq!(encoder.as_str().unwrap(), "hoho❤\r\n");
        }

        #[test]
        fn multiple_finish_calls_are_ok() {
            let mut encoder = EncodingBuffer::new(MailType::Internationalized);
            {
                let mut handle = encoder.writer();
                assert_ok!(handle
                    .write_if_atext("hoho")
                    .handle_condition_failure(|_| panic!("no condition failur expected")));
                let mut had_cond_failure = false;
                assert_ok!(handle.write_if_atext("a(b").handle_condition_failure(|_| {
                    had_cond_failure = true;
                    Ok(())
                }));
                assert!(had_cond_failure);
                assert_ok!(handle
                    .write_if_atext("❤")
                    .handle_condition_failure(|_| panic!("no condition failur expected")));
                handle.finish_header();
                handle.finish_header();
                handle.finish_header();
                handle.finish_header();
            }
            assert_eq!(encoder.as_str().unwrap(), "hoho❤\r\n");
        }

        #[test]
        fn multiple_finish_and_undo_calls() {
            let mut encoder = EncodingBuffer::new(MailType::Internationalized);
            {
                let mut handle = encoder.writer();
                assert_ok!(handle
                    .write_if_atext("hoho")
                    .handle_condition_failure(|_| panic!("no condition failur expected")));
                handle.undo_header();
                handle.finish_header();
                handle.undo_header();
                handle.undo_header();
            }
            assert_eq!(encoder.as_slice(), b"");
        }

        #[test]
        fn header_body_header() {
            let mut encoder = EncodingBuffer::new(MailType::Internationalized);
            {
                let mut handle = encoder.writer();
                assert_ok!(handle.write_utf8("H: yay"));
                handle.finish_header();
            }
            encoder.write_body_unchecked(&"da body");
            {
                let mut handle = encoder.writer();
                assert_ok!(handle.write_utf8("❤"));
                handle.finish_header();
            }
            assert_eq!(
                encoder.as_slice(),
                concat!("H: yay\r\n", "da body\r\n", "❤\r\n").as_bytes()
            );
        }

        #[test]
        fn has_unfinished_parts() {
            let mut encoder = EncodingBuffer::new(MailType::Internationalized);
            {
                let mut handle = encoder.writer();
                assert_ok!(handle.write_utf8("Abc:"));
                assert!(handle.has_unfinished_parts());
                handle.undo_header();
                assert_not!(handle.has_unfinished_parts());
                assert_ok!(handle.write_utf8("Abc: c"));
                assert!(handle.has_unfinished_parts());
                handle.finish_header();
                assert_not!(handle.has_unfinished_parts());
            }
        }

        #[test]
        fn drop_without_write_is_ok() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            let handle = encoder.writer();
            mem::drop(handle)
        }

        #[test]
        fn drop_after_undo_is_ok() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            let mut handle = encoder.writer();
            assert_ok!(handle.write_str(SoftAsciiStr::from_str("Header-One").unwrap()));
            handle.undo_header();
            mem::drop(handle);
        }

        #[test]
        fn drop_after_finish_is_ok() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            let mut handle = encoder.writer();
            assert_ok!(handle.write_str(SoftAsciiStr::from_str("Header-One: 12").unwrap()));
            handle.finish_header();
            mem::drop(handle);
        }

        #[should_panic]
        #[test]
        fn drop_unfinished_panics() {
            let mut encoder = EncodingBuffer::new(MailType::Ascii);
            let mut handle = encoder.writer();
            assert_ok!(handle.write_str(SoftAsciiStr::from_str("Header-One:").unwrap()));
            mem::drop(handle);
        }

        #[test]
        fn trace_and_undo() {
            let mut encoder = EncodingBuffer::new(MailType::Internationalized);
            {
                let mut handle = encoder.writer();
                assert_ok!(handle.write_utf8("something"));
                handle.mark_fws_pos();
                assert_ok!(handle.write_utf8("<else>"));
                handle.undo_header();
            }
            assert_eq!(encoder.trace.len(), 0);
        }

        #[test]
        fn trace_and_undo_does_do_to_much() {
            let mut encoder = EncodingBuffer::new(MailType::Internationalized);
            {
                let mut handle = encoder.writer();
                assert_ok!(handle.write_utf8("H: a"));
                handle.finish_header();
                assert_ok!(handle.write_utf8("something"));
                handle.mark_fws_pos();
                assert_ok!(handle.write_utf8("<else>"));
                handle.undo_header();
            }
            assert_eq!(encoder.trace, vec![NowUtf8, Text("H: a".into()), CRLF, End]);
        }

        #[test]
        fn trace_traces() {
            let mut encoder = EncodingBuffer::new(MailType::Internationalized);
            {
                let mut handle = encoder.writer();
                assert_ok!(handle.write_str(SoftAsciiStr::from_str("Header").unwrap()));
                assert_ok!(handle.write_char(SoftAsciiChar::from_unchecked(':')));
                let mut had_cond_failure = false;
                assert_ok!(handle
                    .write_if_atext("a(b)c")
                    .handle_condition_failure(|_| {
                        had_cond_failure = true;
                        Ok(())
                    }));
                assert_ok!(handle
                    .write_if_atext("abc")
                    .handle_condition_failure(|_| panic!("unexpected cond failure")));
                assert_ok!(handle.write_utf8("❤"));
                assert_ok!(handle.write_str_unchecked("remove me\r\n"));
                assert_ok!(handle.write_utf8("   "));
                handle.finish_header()
            }
            assert_eq!(
                encoder.trace,
                vec![
                    NowStr,
                    Text("Header".into()),
                    NowChar,
                    Text(":".into()),
                    NowAText,
                    Text("abc".into()),
                    NowUtf8,
                    Text("❤".into()),
                    NowUnchecked,
                    Text("remove me".into()),
                    CRLF,
                    NowUtf8,
                    Text("   ".into()),
                    TruncateToCRLF,
                    End
                ]
            );
        }

        #[test]
        fn with_handle_on_error() {
            let mut encoder = EncodingBuffer::new(MailType::Internationalized);
            let res = encoder.write_header_line(|hdl| {
                hdl.write_utf8("some partial writes")?;
                Err(EncodingErrorKind::Other { kind: "error ;=)" }.into())
            });
            assert_err!(res);
            assert_eq!(encoder.trace, vec![]);
            assert_eq!(encoder.as_slice(), b"");
        }

        #[test]
        fn with_handle_partial_writes() {
            let mut encoder = EncodingBuffer::new(MailType::Internationalized);
            let res = encoder.write_header_line(|hdl| hdl.write_utf8("X-A: 12"));
            assert_ok!(res);
            assert_eq!(
                encoder.trace,
                vec![NowUtf8, Text("X-A: 12".into()), CRLF, End]
            );
            assert_eq!(encoder.as_slice(), b"X-A: 12\r\n");
        }

        #[test]
        fn with_handle_ok() {
            let mut encoder = EncodingBuffer::new(MailType::Internationalized);
            let res = encoder.write_header_line(|hdl| {
                hdl.write_utf8("X-A: 12")?;
                hdl.finish_header();
                Ok(())
            });
            assert_ok!(res);
            assert_eq!(
                encoder.trace,
                vec![NowUtf8, Text("X-A: 12".into()), CRLF, End,]
            );
            assert_eq!(encoder.as_slice(), b"X-A: 12\r\n")
        }

        #[test]
        fn douple_write_fws() {
            let mut encoder = EncodingBuffer::new(MailType::Internationalized);
            let res = encoder.write_header_line(|hdl| {
                hdl.write_fws();
                hdl.write_fws();
                Ok(())
            });
            assert_ok!(res);
            assert_eq!(
                encoder.trace,
                vec![
                    MarkFWS,
                    NowChar,
                    Text(" ".to_owned()),
                    MarkFWS,
                    NowChar,
                    Text(" ".to_owned()),
                    TruncateToCRLF,
                    End
                ]
            );
            assert_eq!(encoder.as_slice(), b"")
        }

        #[test]
        fn double_write_fws_then_long_line() {
            let long_line = concat!(
                "10_3456789",
                "20_3456789",
                "30_3456789",
                "40_3456789",
                "50_3456789",
                "60_3456789",
                "70_3456789",
                "80_3456789",
            );
            let mut encoder = EncodingBuffer::new(MailType::Internationalized);
            let res = encoder.write_header_line(|hdl| {
                hdl.write_fws();
                hdl.write_fws();
                hdl.write_utf8(long_line)?;
                Ok(())
            });
            assert_ok!(res);
            assert_eq!(
                encoder.trace,
                vec![
                    MarkFWS,
                    NowChar,
                    Text(" ".to_owned()),
                    MarkFWS,
                    NowChar,
                    Text(" ".to_owned()),
                    NowUtf8,
                    Text(long_line.to_owned()),
                    CRLF,
                    End
                ]
            );
            assert_eq!(
                encoder.as_slice(),
                format!("  {}\r\n", long_line).as_bytes()
            )
        }

        #[test]
        fn semantic_ws_are_not_eaten_with_line_breaking() {
            let long_line_1 = concat!(
                "Header:789",
                "20_3456789",
                "30_3456789",
                "40_3456789",
                "50_3456789",
                "60_3456789",
            );
            let long_line_2 = concat!(" xxxxxxxxx", "80_3456789");

            let expected_res = concat!(
                "Header:789",
                "20_3456789",
                "30_3456789",
                "40_3456789",
                "50_3456789",
                "60_3456789",
                "\r\n ",
                " xxxxxxxxx",
                "80_3456789",
                "\r\n"
            );

            let mut encoder = EncodingBuffer::new(MailType::Internationalized);
            encoder
                .write_header_line(|hdl| {
                    hdl.write_utf8(long_line_1).unwrap();
                    hdl.mark_fws_pos();
                    hdl.write_utf8(long_line_2).unwrap();
                    hdl.finish_header();
                    Ok(())
                })
                .unwrap();

            let got = str::from_utf8(encoder.as_slice()).unwrap();
            assert_eq!(expected_res, got);
        }
    }

    ec_test! {
        does_ec_test_work,
        {
            use super::EncodingWriter;
            enc_func!(|x: &mut EncodingWriter| {
                x.write_utf8("hy")
            })
        } => Utf8 => [
            Text "hy"
        ]
    }

    ec_test! {
        does_ec_test_work_with_encode_closure,
        {
            use super::EncodingWriter;
            let think = "hy";
            enc_closure!(move |x: &mut EncodingWriter| {
                x.write_utf8(think)
            })
        } => Utf8 => [
            Text "hy"
        ]
    }

    ec_test! {
        does_ec_test_allow_early_return,
        {
            use super::EncodingWriter;
            // this is just a type system test, if it compiles it can bail
            if false { ec_bail!(kind: Other { kind: "if false ..." }) }
            enc_func!(|x: &mut EncodingWriter| {
                x.write_utf8("hy")
            })
        } => Utf8 => [
            Text "hy"
        ]
    }

    mod trait_object {
        use super::super::*;

        #[derive(Default, Clone, PartialEq, Debug)]
        struct TestType(&'static str);

        impl EncodableInHeader for TestType {
            fn encode(&self, encoder: &mut EncodingWriter) -> Result<(), EncodingError> {
                encoder.write_utf8(self.0)
            }

            fn boxed_clone(&self) -> Box<EncodableInHeader> {
                Box::new(self.clone())
            }
        }

        #[derive(Default, Clone, PartialEq, Debug)]
        struct AnotherType(&'static str);

        impl EncodableInHeader for AnotherType {
            fn encode(&self, encoder: &mut EncodingWriter) -> Result<(), EncodingError> {
                encoder.write_utf8(self.0)
            }

            fn boxed_clone(&self) -> Box<EncodableInHeader> {
                Box::new(self.clone())
            }
        }

        #[test]
        fn is() {
            let tt = TestType::default();
            let erased: &EncodableInHeader = &tt;
            assert_eq!(true, erased.is::<TestType>());
            assert_eq!(false, erased.is::<AnotherType>());
        }

        #[test]
        fn downcast_ref() {
            let tt = TestType::default();
            let erased: &EncodableInHeader = &tt;
            let res: Option<&TestType> = erased.downcast_ref::<TestType>();
            assert_eq!(Some(&tt), res);
            assert_eq!(None, erased.downcast_ref::<AnotherType>());
        }

        #[test]
        fn downcast_mut() {
            let mut tt_nr2 = TestType::default();
            let mut tt = TestType::default();
            let erased: &mut EncodableInHeader = &mut tt;
            {
                let res: Option<&mut TestType> = erased.downcast_mut::<TestType>();
                assert_eq!(Some(&mut tt_nr2), res);
            }
            assert_eq!(None, erased.downcast_mut::<AnotherType>());
        }

        #[test]
        fn downcast() {
            let tt = Box::new(TestType::default());
            let erased: Box<EncodableInHeader> = tt;
            let erased = assert_err!(erased.downcast::<AnotherType>());
            let _: Box<TestType> = assert_ok!(erased.downcast::<TestType>());
        }
    }
}
