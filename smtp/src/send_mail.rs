//! Module implementing mail sending using `new-tokio-smtp::send_mail`.

use std::iter::once as one;

use futures::{
    future::{self, Either, Future},
    stream::{self, Stream},
};

use mail::Context;
use mail_internals::{encoder::EncodingBuffer, MailType};

use new_tokio_smtp::{
    send_mail as smtp, send_mail::MailEnvelop, Cmd, Connection, ConnectionConfig, SetupTls,
};

use {error::MailSendError, request::MailRequest};

/// Sends a given mail (request).
///
/// - This will use the given context to encode the mail.
/// - Then it will use the connection config to open a connection to a mail
///   server (likely a Mail Submission Agent (MSA)).
/// - Following this it will send the mail to the server.
/// - After which it will close the connection again.
///
/// You can use `MailRequest: From<Mail>` (i.e. `mail.into()`) to pass in
/// a mail and derive the envelop data (from, to) from it or create your own
/// mail request if different smtp envelop data is needed.
pub fn send<A, S>(
    mail: MailRequest,
    conconf: ConnectionConfig<A, S>,
    ctx: impl Context,
) -> impl Future<Item = (), Error = MailSendError>
where
    A: Cmd,
    S: SetupTls,
{
    let fut = encode(mail, ctx)
        .then(move |envelop_res| Connection::connect_send_quit(conconf, one(envelop_res)).collect())
        .map(|mut results| {
            results
                .pop()
                .expect("[BUG] sending one mail expects one result")
        });

    fut
}

/// Sends a batch of mails to a server.
///
/// - This will use the given context to encode all mails.
/// - After which it will use the connection config to open a connection
///   to the server (like a Mail Submission Agent (MSA)).
/// - Then it will start sending mails.
///   - If a mail fails because of an error code but setting up the connection
///     (which includes auth) didn't fail then others mails in the input will
///     still be send
///   - If the connection is broken because setting it up failed or it was
///     interrupted, then the mail at which place it was noticed will return
///     the given error and all later mails will return a I/0-Error with the
///     `ErrorKind::NoConnection`
/// - It will return a `Stream` which when polled will send the mails
///   and return results _in the order the mails had been supplied_. So
///   for each mail there will be exactly one result.
/// - Once the stream is completed the connection will automatically be
///   closed (even if the stream is not yet dropped, it closes it the
///   moment it notices that there are no more mails to send!)
///
pub fn send_batch<A, S, C>(
    mails: Vec<MailRequest>,
    conconf: ConnectionConfig<A, S>,
    ctx: C,
) -> impl Stream<Item = (), Error = MailSendError>
where
    A: Cmd,
    S: SetupTls,
    C: Context,
{
    let iter = mails.into_iter().map(move |mail| encode(mail, ctx.clone()));

    let fut = collect_res(stream::futures_ordered(iter))
        .map(move |vec_of_res| Connection::connect_send_quit(conconf, vec_of_res))
        .flatten_stream();

    fut
}

//FIXME[futures/v>=0.2] use Error=Never
fn collect_res<S, E>(stream: S) -> impl Future<Item = Vec<Result<S::Item, S::Error>>, Error = E>
where
    S: Stream,
{
    stream.then(|res| Ok(res)).collect()
}

/// Turns a `MailRequest` into a future resolving to a `MailEnvelop`.
///
/// This function is mainly used internally for `send`, `send_batch`
/// but can be used by other libraries when `send`/`send_batch` doesn't
/// quite match their use case. E.g. if they want to have a connection
/// pool and instead of `connect->send->quit` want to have something like
/// `take_from_pool->test->send->place_back_to_pool`, in which case they
/// probably would want to do something along the lines of using encode
/// then take a connection, test it, use the mail envelops with `new-tokio-smtp`'s
/// `SendAllMails` stream with a `on_completion` handler which places it
/// back in the pool.
pub fn encode<C>(
    request: MailRequest,
    ctx: C,
) -> impl Future<Item = MailEnvelop, Error = MailSendError>
where
    C: Context,
{
    let (mail, envelop_data) = match request.into_mail_with_envelop() {
        Ok(pair) => pair,
        Err(e) => return Either::A(future::err(e.into())),
    };

    let fut = mail
        .into_encodable_mail(ctx.clone())
        .and_then(move |enc_mail| {
            ctx.offload_fn(move || {
                let (mail_type, requirement) = if envelop_data.needs_smtputf8() {
                    (
                        MailType::Internationalized,
                        smtp::EncodingRequirement::Smtputf8,
                    )
                } else {
                    (MailType::Ascii, smtp::EncodingRequirement::None)
                };

                let mut buffer = EncodingBuffer::new(mail_type);
                enc_mail.encode(&mut buffer)?;

                let vec_buffer: Vec<_> = buffer.into();
                let smtp_mail = smtp::Mail::new(requirement, vec_buffer);

                Ok(smtp::MailEnvelop::from((smtp_mail, envelop_data)))
            })
        })
        .map_err(MailSendError::from);

    Either::B(fut)
}
