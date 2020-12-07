/// Specifies what kind of mail we want to create.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum MailType {
    /// A 7-bit us-ascii mail.
    Ascii,

    /// A us-ascii mail, but the body can contain 8bit values.
    ///
    /// This for example allows sending a mail with an utf-8
    /// formatted body. But be aware that aspects like line
    /// length limit still count an the ascii bytes for "\r\n"
    /// still count as newlines. So using this for any non-us-ascii
    /// compatible encoding (e.g. utf-16) isn't a good idea.
    /// Neither is it suited for directly containing resources
    /// like images.
    Mime8BitEnabled,

    /// A internationalized mail.
    ///
    /// Internationalized mails extend multiple grammar parts
    /// to allow any non us-ascii utf-8 code point additionally
    /// to the already allowed utf-8 code points. Internationalized
    /// mails are required for any mail containing a mailbox with
    /// an non us-ascii local/user part (the part before the `@`).
    /// They also strongly simplify non ascii utf-8 in all other
    /// places like e.g. the `Subject` header.
    Internationalized,
}

impl MailType {
    /// Returns true if the self is equal to `Internationalized`
    #[inline]
    pub fn is_internationalized(&self) -> bool {
        *self == MailType::Internationalized
    }

    /// Returns true if self is either `Internationalized` or `Mime8BitEnabled`
    pub fn supports_8bit_bodies(&self) -> bool {
        use self::MailType::*;
        match *self {
            Ascii => false,
            Mime8BitEnabled => true,
            Internationalized => true,
        }
    }
}
