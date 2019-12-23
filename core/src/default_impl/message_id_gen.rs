use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::sync::atomic::{AtomicUsize, Ordering};

use rand;
use soft_ascii_string::SoftAsciiString;

use context::MailIdGenComponent;
use headers::header_components::{ContentId, Domain, MessageId};
use internals::error::EncodingError;

static MAIL_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn counter_next() -> usize {
    MAIL_COUNTER.fetch_add(1, Ordering::AcqRel)
}

fn anonymize_through_random_hash(num: usize) -> u64 {
    let rnum = rand::random::<u32>();
    let mut hasher = DefaultHasher::new();
    hasher.write_usize(num);
    hasher.write_u32(rnum);
    hasher.finish()
}

fn gen_next_program_unique_number() -> u64 {
    anonymize_through_random_hash(counter_next())
}

/// a id gen implementation using hash-ing to generate part of it's left hand side
#[derive(Debug, Clone)]
pub struct HashedIdGen {
    domain: SoftAsciiString,
    part_unique_in_domain: SoftAsciiString,
}

impl HashedIdGen {
    /// create a new id gen from a `Domain` and a unique part.
    ///
    /// The domain is used as the right hand side of the message
    /// id and the `unique_in_domain_part` is concatenated with `"."`
    /// and a hash from the left part. The hash is generated from
    /// and integrated and a random number generated from a internal
    /// program global counter.
    ///
    /// The tuple (`domain`,`part_unique_in_domain`) has to be world unique.
    /// I.e. for "your" domain you have to make sure the `part_unique_in_domain`
    /// is unique in it's usage for message id's.
    ///
    /// # Error
    ///
    /// If the domain is not ascii and puny code encoding it fails
    ///
    /// # Design Notes (usage of `part_unique_in_domain`)
    ///
    /// While the internal global counter is enough to generate seemingly
    /// unique message id's it has two problems:
    ///
    /// 1. the id's are only _program_ unique but they need to be
    ///    world unique, i.e. unique between restarts of the program
    ///    and multiple instances running in parallel
    ///
    /// 2. they allow guessing the underlying number exposing private
    ///    information about how many mails are send
    ///
    /// The unique part can solves one of the problems, if it is used correctly:
    ///
    /// 1. by providing unique bytes for `part_unique_in_domain` so
    ///    that every time a program using this library is started
    ///    _different_ bytes are passed in all any collision in
    ///    message/content id's are prevented
    ///
    /// The other problem is solved by hashing the counter with
    /// a random part.
    pub fn new(
        domain: Domain,
        part_unique_in_domain: SoftAsciiString,
    ) -> Result<Self, EncodingError> {
        let domain = domain.into_ascii_string()?;
        Ok(HashedIdGen {
            domain,
            part_unique_in_domain,
        })
    }
}

impl MailIdGenComponent for HashedIdGen {
    fn generate_message_id(&self) -> MessageId {
        let msg_id = format!(
            "{unique}.{hash:x}@{domain}",
            unique = self.part_unique_in_domain,
            hash = gen_next_program_unique_number(),
            domain = self.domain
        );
        MessageId::from_unchecked(msg_id)
    }

    fn generate_content_id(&self) -> ContentId {
        self.generate_message_id().into()
    }
}

#[cfg(test)]
mod test {

    mod HashedIdGen {
        #![allow(non_snake_case)]

        use headers::header_components::Domain;
        use headers::HeaderTryFrom;
        use soft_ascii_string::SoftAsciiString;
        use std::collections::HashSet;
        use std::sync::Arc;

        //NOTE: this is a rust bug, the import is not unused
        use super::super::HashedIdGen;
        #[allow(unused_imports)]
        use context::MailIdGenComponent;

        fn setup() -> Arc<HashedIdGen> {
            let unique_part = SoftAsciiString::from_unchecked("bfr7tz4");
            let domain = Domain::try_from("fooblabar.test").unwrap();
            Arc::new(HashedIdGen::new(domain, unique_part).unwrap())
        }

        mod get_message_id {
            use super::*;

            #[test]
            fn should_always_return_a_new_id() {
                let id_gen = setup();
                let mut cids = HashSet::new();
                for _ in 0..20 {
                    assert!(cids.insert(id_gen.generate_message_id()))
                }
            }
        }

        mod generate_content_id {
            use super::*;

            #[test]
            fn should_always_return_a_new_id() {
                let id_gen = setup();
                let mut cids = HashSet::new();
                for _ in 0..20 {
                    assert!(cids.insert(id_gen.generate_content_id()))
                }
            }
        }
    }
}
