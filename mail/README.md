# mail &emsp;

Documentation can be [viewed on docs.rs](https://docs.rs/mail).

Facade which re-exports functionality from a number of mail related crates.

The facade should re-export enough functionality for using the mail carate
to create/modify/encode mail, send them over smtp (feature) or use a handlebars
based template engine to create them from a template (feature).

Functionality steming from following crates is re-exported:
- `mail-core` provides a `Mail` type and the core functionality
  around creating/modifing/encoding mails.
- `mail-headers` provides implementations for the headers of the mail.
  This also includes a number of header components which appear in mail
  header bodies but are also re-used in other placed (e.g. `MediaType`
  stemming from the `Content-Type` header or `Domain`).
- `mail-smtp` bindings to `new-tokio-smtp` to  make it easier to send
  mails over smtp. This also includes functionality to automatically
  derive the _smtp_ sender/receiver from the mail if no sender/receiver
  is explicitly given (Smtp by it's standard does not use the `From`/`To`
  headers of a mail. Instead it treats the mail, including it's headers
  mostly as a opaque block of data. But in practice the addresses in
  `From`/`To`/`Sender` tend to match the smtp sender/recipient).
- `mail-template` provides a simple way to bind template engine to
  generate mails. It has a feature which if enable directly includes
  bindings to `handlebars`. This feature is re-exported in the crate
  as the `handlebars` feature.
- `mail-internals` provides some shared mostly internal parts the other
   crates use. This is normally only needed if you write your own mail
   header implementations. But even then the does this crate re-expost
   the parts most likely needed (in the `header_encoding` module).

## Examples

### [`mail_by_hand`](https://github.com/1aim/mail/tree/master/mail/examples/mail_by_hand.rs)

Creates and encodes a simple mail without using any fancy helpers, templates or
similar.

### [`mail_from_template`](https://github.com/1aim/mail/tree/master/mail/examples/mail_from_template/main.rs)

Uses the bindings for the `handlebars` template engine to create a mail, including
alternate bodies and an attachment.

### [`send_mail`](https://github.com/1aim/mail/tree/master/mail/examples/send_mail/main.rs)

A simple program which queries the user for information and then sends a
(simple) mail to an MSA (Mail Submission Agent).  While it is currently limited
to STARTTLS on port 587, Auth Plain and only simple text mails this is a
limitation of this cli program not the mail libraries which can handle other
forms of connecting and authenticating etc.

Note that this is meant to send data to an MSA NOT a MX (Mail Exchanger), e.g.
`smtp.gmail.com` is a MSA but `gmail-smtp-in.l.google.com` is an MX.  Also note
that some mail providers do not accept Auth Plain (at least not without
enabling it in the security settings). The reason for this is that they prefer
that applications do not use username+password for authentication but other
formats e.g. OAuth2 tokens.

Rest assured that the authentication data is only sent over a TLS encrypted
channel. Still if you don't trust it consider using some throw away or testing
mail service e.g. `ethereal.email`.

Lastly the examples uses the same unique seed every time, which means that
Message-ID's, and Content-ID's are not guaranteed to be world unique even
through they should (again a limitation of the example not the mail crate).
Nevertheless given that it also doesn't use its "own" domain but a `.test`
domain it can't guarantee world uniqueness anyway and would fail many spam filters,
so if you use it make sure to change this to the right values for your use
case.

## Features

### `smtp`

Provides bindings to `new-tokio-smtp` under `mail::smtp` by reexporting the
`mail-smtp` crate

### `handlebars`

Provides a `mail-template` engine implementation using the `handlebars`
crate. It can be found under `mail::template::handlebars::Handlebars`;


### `traceing`

Enables the `traceing` debugging functionality in the `EncodingBuffer`
from `mail-internals`, this is only used for testing header implementations
and comes with noticeable overhead. **As such this should not be enabled
except for testing header implementations**. Also `EncodingBuffer` isn't
re-exported as it can be seen as an internal part of the implementation
which normally doesn't need to be accessed directly.

## License

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.
