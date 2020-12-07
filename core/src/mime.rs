//! Module containing some utilities for MIME usage/creation.
use rand::{self, Rng};

// The maximal boundary with wich " boundary=\"...\"" fits into 78 chars line length limit
const MULTIPART_BOUNDARY_MAX_LENGTH: usize = 66;

// Does not include ' ' to remove special handling for last char.
static BOUNDARY_CHARS: &[char] = &[
    '\'', '(', ')', '+', ',', '-', '.', '/', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ':',
    '=', '?', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q',
    'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '_', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i',
    'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
];

/// Prevent collisions with Base64/Quoted-Printable
static ANTI_COLLISION_CHARS: &str = "=_^";

/// Generate a boundary from a counter, "=_^" and a random sequence of boundary chars.
///
/// # Usage Note
///
/// _Be aware that it might be required to quote the boundary._
///
/// # Implementation Details
///
/// The boundary  will start with `=_^` which is neither valid for base64 nor
/// quoted-printable encoding followed by a hex repr. of the given count,
/// a `.` and a random sequence of boundary chars.
///
/// The boundary will be 66 chars long, this is so that if a boundary parameter is
/// placed on it's own line it won't be more then 78 chars. (66 chars boundary,
/// + 2 chars quotation + 9 chars for 'boundary=' + 1 char because of `\r\n<WS>`
/// == 78 chars)
///
/// The remaining characters will be picked based one the grammar defined in rfc2046,
/// which relevant part is:
///
/// ```BNF
/// boundary := 0*69<bchars> bcharsnospace
/// bchars := bcharsnospace / " "
/// bcharsnospace := DIGIT / ALPHA / "'" / "(" / ")" /
///                  "+" / "_" / "," / "-" / "." /
///                  "/" / ":" / "=" / "?"
/// ```
///
/// Note that `' '` isn't used for simplicity.
///
pub fn create_structured_random_boundary(count: usize) -> String {
    let mut out = format!(
        "{anti_collision}{count:x}.",
        anti_collision = ANTI_COLLISION_CHARS,
        count = count
    );

    let rem = MULTIPART_BOUNDARY_MAX_LENGTH - out.len();
    out.reserve(rem);

    let mut rng = rand::thread_rng();
    let len = BOUNDARY_CHARS.len();
    for _ in 0..rem {
        let idx = rng.gen_range(0, len);
        out.push(BOUNDARY_CHARS[idx]);
    }

    out
}

#[cfg(test)]
mod test {

    mod write_random_boundary_to {
        use super::super::*;

        #[test]
        fn boundary_is_not_quoted() {
            let out = create_structured_random_boundary(0);
            assert!(!out.starts_with("\""));
            assert!(!out.ends_with("\""));
        }

        #[test]
        fn boundary_start_special() {
            let out = create_structured_random_boundary(0);
            assert!(out.starts_with("=_^0."));
        }

        #[test]
        fn boundary_has_a_resonable_length() {
            let out = create_structured_random_boundary(0);
            assert!(out.len() > 22 && out.len() <= MULTIPART_BOUNDARY_MAX_LENGTH);
            let out = create_structured_random_boundary(1000);
            assert!(out.len() > 22 && out.len() <= MULTIPART_BOUNDARY_MAX_LENGTH);
        }

        #[test]
        fn boundary_does_not_contain_space_or_slach_or_quotes() {
            // while it could contain them it's recommended not to do it
            let out = create_structured_random_boundary(0);

            for ch in out[1..out.len() - 1].chars() {
                assert!(ch as u32 >= 32);
                assert!(ch as u32 <= 126);
                assert_ne!(ch, '\t');
                assert_ne!(ch, '\\');
                assert_ne!(ch, '"');
            }

            assert_ne!(out.as_bytes()[out.len() - 1], b' ');
        }
    }
}
