use internals::grammar::{is_vchar, is_ws};
use internals::MailType;

#[derive(Copy, Clone, Debug, Fail, PartialEq, Eq, Hash)]
#[fail(display = "text contained control characters")]
pub struct PartitionError;

#[derive(Copy, Clone)]
pub enum Partition<'a> {
    //from -> to the start of the next block
    SPACE(&'a str),
    VCHAR(&'a str),
}

#[derive(Clone, Copy, PartialEq)]
enum Type {
    SPACE,
    VCHAR,
}

pub fn partition<'a>(text: &'a str) -> Result<Vec<Partition<'a>>, PartitionError> {
    use self::Type::*;

    if text.len() == 0 {
        return Ok(Vec::new());
    }

    // unwrap is ok, as we return earlier if len == 0
    let start_with_vchar = is_vchar(text.chars().next().unwrap(), MailType::Internationalized);

    let mut partitions = Vec::new();
    let mut current_type = if start_with_vchar { VCHAR } else { SPACE };

    let mut start_of_current = 0;
    for (idx, ch) in text.char_indices() {
        if is_vchar(ch, MailType::Internationalized) {
            if current_type == SPACE {
                // idx is the start index of the current char, with is the
                // (exclusive) end index of the previous char which is the
                // last char of the Partition we want to push
                partitions.push(Partition::SPACE(&text[start_of_current..idx]));
                start_of_current = idx;
                current_type = VCHAR
            }
        } else if is_ws(ch) || ch == '\r' || ch == '\n' {
            if current_type == VCHAR {
                partitions.push(Partition::VCHAR(&text[start_of_current..idx]));
                start_of_current = idx;
                current_type = SPACE
            }
        } else {
            //TODO look into this error case and especially PartitionError's Display impl
            return Err(PartitionError);
        }
    }

    partitions.push(match current_type {
        SPACE => Partition::SPACE(&text[start_of_current..]),
        VCHAR => Partition::VCHAR(&text[start_of_current..]),
    });

    Ok(partitions)
}
