use quoted_string::spec::{GeneralQSSpec, PartialCodePoint, WithoutQuotingValidator};

use media_type_impl_utils::quoted_string;
use MailType;

/// A Quoted String specification in context of Mail ([rfc5322](https://tools.ietf.org/html/rfc5322#section-2.2.3))
///
/// This implementation of MailQsSpec _does not_ include support for the obsolete parts of the grammar
/// as it's meant for generation/encoding and no obsolete parts should be generated at all (Through
/// a parser would have to be able to parse them for compatibility reasons).
///
#[derive(Copy, Clone, Debug, Default)]
pub struct MailQsSpec;

impl GeneralQSSpec for MailQsSpec {
    type Quoting = quoted_string::NormalQuoting;
    type Parsing = quoted_string::MimeParsing;
}

/// A Quoted String specification in context of Mail ([rfc5322](https://tools.ietf.org/html/rfc5322#section-2.2.3))
///
/// This implementation of MailQsSpec _does not_ include support for the obsolete parts of the grammar
/// as it's meant for generation/encoding and no obsolete parts should be generated at all (Through
/// a parser would have to be able to parse them for compatibility reasons).
#[derive(Copy, Clone, Debug, Default)]
pub struct InternationalizedMailQsSpec;

impl GeneralQSSpec for InternationalizedMailQsSpec {
    type Quoting = quoted_string::NormalUtf8Quoting;
    type Parsing = quoted_string::MimeParsingUtf8;
}

pub use self::quoted_string::MimeTokenValidator as UnquotedTokenValidator;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct UnquotedATextValidator {
    mail_type: MailType,
}

impl UnquotedATextValidator {
    pub fn new(mail_type: MailType) -> Self {
        UnquotedATextValidator { mail_type }
    }
}

impl WithoutQuotingValidator for UnquotedATextValidator {
    fn next(&mut self, pcp: PartialCodePoint) -> bool {
        is_atext(pcp, self.mail_type)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct UnquotedDotAtomTextValidator {
    mail_type: MailType,
    allow_dot: bool,
}

impl UnquotedDotAtomTextValidator {
    pub fn new(mail_type: MailType) -> Self {
        UnquotedDotAtomTextValidator {
            mail_type,
            allow_dot: false,
        }
    }
}

impl WithoutQuotingValidator for UnquotedDotAtomTextValidator {
    fn next(&mut self, pcp: PartialCodePoint) -> bool {
        if is_atext(pcp, self.mail_type) {
            self.allow_dot = true;
            true
        } else if self.allow_dot && pcp.as_u8() == b'.' {
            self.allow_dot = false;
            true
        } else {
            false
        }
    }

    fn end(&self) -> bool {
        // it can't end in a dot so it's the same as allow_dot
        // (as empty or starting with a dot is also not allowed)
        self.allow_dot
    }
}

//TODO replace with lookup table (which could be placed in `::grammar`)!
fn is_atext(pcp: PartialCodePoint, mail_type: MailType) -> bool {
    use grammar::is_special;
    let iu8 = pcp.as_u8();
    if iu8 > 0x7f {
        mail_type == MailType::Internationalized
    } else {
        b'!' <= iu8 && iu8 <= b'~' && !is_special(iu8 as char)
    }
}
