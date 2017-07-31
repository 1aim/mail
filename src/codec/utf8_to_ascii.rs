use ascii::{ AsciiString, AsAsciiStr };
use codec::MailEncoder;
use quoted_printable::encode;
use char_validators::encoded_word::EncodedWordContext;

pub fn q_encode_for_encoded_word<E>( encoder: &mut E, _ctx: EncodedWordContext, input: &str )
    where E: MailEncoder
{
    //TODO I suspect the `quoted_printable` crate is not
    // completely correct wrt. to some aspects, have to
    // check this
    //FIXME does need the current line length and wather or not it is a header
    let raw = encode( input.as_bytes() );
    let asciied = unsafe { AsciiString::from_ascii_unchecked( raw ) };
    encoder.write_str( &*asciied )
}

pub fn puny_code_domain<E>( input: &str, encoder: &mut E )
    where E: MailEncoder
{
    if let Ok( val ) = input.as_ascii_str() {
        encoder.write_str( val )
    } else {
        unimplemented!();
    }
}