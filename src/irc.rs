//! Tools for parsing IRC messages, designed to be as efficient and minimal as possible, while
//! not compromising correctness.

/// Extracts the verb part of an IRC message.
///
/// If the return value represents a meaningful IRC verb, then the input is either well-formed, or
/// badly-formed in an insignificant way (too many spaces, for example). For other badly-formed
/// inputs, the return value will either be an empty string or nonsense. In other words, if the
/// returned slice is meaningful as an IRC verb, then the slice *definitely* appears as the verb
/// part of the input IRC message.
///
/// # Examples
///
/// Extracting the verb from correctly-formed messages:
///
/// ```rust
/// assert_eq!(b"TEST", extract_verb(":server TEST arg :long arg".as_bytes()));
/// assert_eq!(b"TEST", extract_verb("  @tag=value  TEST    arg :long arg".as_bytes()));
/// assert_eq!(b"TEST", extract_verb("    TEST    ".as_bytes()));
/// ```
///
/// Messages without verbs
///
/// ```rust
/// assert_eq!(b"", extract_verb(":server   ".as_bytes()));
/// assert_eq!(b"", extract_verb("  @tag=value  :some.server  ".as_bytes()));
/// ```
pub fn extract_verb(line: &[u8]) -> &[u8] {
    let mut i = 0;

    // skip any initial spaces
    while i < line.len() && line[i] == b' ' { i += 1; }

    // skip tags
    if i < line.len() && line[i] == b'@' {
        while i < line.len() && line[i] != b' ' { i += 1; }
        while i < line.len() && line[i] == b' ' { i += 1; }
    }

    // skip prefix
    if i < line.len() && line[i] == b':' {
        while i < line.len() && line[i] != b' ' { i += 1; }
        while i < line.len() && line[i] == b' ' { i += 1; }
    }

    let mut j = i;

    // skip verb
    while j < line.len() && line[j] != b' ' { j += 1; }

    &line[i..j]
}

/// The parsed form of an IRC `CAP` message.
pub struct CapMessage<'m> {
    pub subcommand: &'m [u8],
    pub trailing: &'m [u8],
}

// CAP LS 302
// CAP LS
// CAP REQ :list of caps
// CAP END

// CAP * LS * :long list of caps
// CAP * LS :long list of caps
// CAP * ACK :list of caps
// CAP * NAK :list of caps

/// Extracts the subcommand and trailing parts of an IRCv3 CAP message.
///
/// For example, `CAP * LS * :multi-prefix sasl`, with `is_server` set to true, would return `LS`
/// as the subcommand, and `multi-prefix sasl` as the trailing portion. (If `is_server` were set
/// to false, then the subcommand would be incorrectly identified as the `*` appearing after the
/// main verb.)
///
/// The `is_server` parameter indicates whether the line came from a server or a client. Servers
/// include an extra "client identifier" parameter between the verb and the subcommand, and it's
/// not possible in general to determine if a given string is either a client identifier or a
/// subcommand.
///
/// If the input message does not appear to be a CAP command, then `None` is returned.
pub fn extract_cap<'m>(line: &'m [u8], is_server: bool) -> Option<CapMessage<'m>> {
    let mut i = 0;

    // skip any initial spaces
    while i < line.len() && line[i] == b' ' { i += 1; }

    // skip tags
    if i < line.len() && line[i] == b'@' {
        while i < line.len() && line[i] != b' ' { i += 1; }
        while i < line.len() && line[i] == b' ' { i += 1; }
    }

    // skip prefix
    if i < line.len() && line[i] == b':' {
        while i < line.len() && line[i] != b' ' { i += 1; }
        while i < line.len() && line[i] == b' ' { i += 1; }
    }

    // confirm we're pointing at "CAP "
    if i + 4 > line.len() || &line[i..i+4] != b"CAP " { return None; }
    i += 4;

    // skip any spaces following CAP
    while i < line.len() && line[i] == b' ' { i += 1; }

    // if this is a server message, skip the client identifier
    if is_server {
        while i < line.len() && line[i] != b' ' { i += 1; }
        while i < line.len() && line[i] == b' ' { i += 1; }
    }

    // extract the subcommand
    let mut j = i;
    while j < line.len() && line[j] != b' ' { j += 1; }
    let subcommand = &line[i..j];

    // j points at the first character after the subcommand.
    // now we iterate j to the end of the line.
    // k will point directly after the first " :", or be 0
    // i will point to the last non-space character encountered
    i = 0;
    let mut k = 0;
    while j < line.len() {
        if k == 0 && line[j-1] == b' ' && line[j] == b':' { k = j+1; break; }
        if line[j] != b' ' { i = j; }
        j += 1;
    }

    // extract the trailing part
    let trailing = if k != 0 {
        // if we encountered a colon, the trailing part is everything from k to the end
        &line[k..]
    } else if i != 0 {
        // if we did not encounter a colon, but did encounter a non-space character, then
        // we point j at the last non-space character (i) and iterate backwards until encountering
        // either the start of the line, or a space character
        j = i;
        while j > 0 && line[j] != b' ' { j -= 1; }
        &line[j+1..i+1]
    } else {
        // otherwise, there does not appear to be a trailing part. return an empty slice
        &line[0..0]
    };

    Some(CapMessage { subcommand: subcommand, trailing: trailing })
}

#[cfg(test)]
mod tests {
    use super::*;
    use test;

    fn strify(s: &[u8]) -> &str { unsafe { ::std::str::from_utf8_unchecked(s) } }

    #[test]
    fn test_extract_verb() {
        let inputs = {
            let tag_parts = vec!["", "@tag ", "  @tag ", "@tag   "];
            let srv_parts = vec!["", ":srv ", "  :srv ", ":srv   "];
            let end_parts = vec!["", "  ", " extra", "  extra extra"];

            let mut inputs = Vec::new();

            for tag_part in tag_parts.iter() {
                for srv_part in srv_parts.iter() {
                    for end_part in end_parts.iter() {
                        inputs.push(format!("{}{}TEST{}", tag_part, srv_part, end_part));
                    }
                }
            }

            assert_eq!(inputs.len(), tag_parts.len() * srv_parts.len() * end_parts.len());
            assert!(inputs.len() > 0);

            inputs
        };

        for input in inputs.iter() {
            println!("input: {}$", input);
            assert_eq!("TEST", strify(extract_verb(input.as_bytes())));
        }
    }

    #[test]
    fn test_extract_verb_missing() {
        let inputs = {
            let tag_parts = vec!["", "@tag ", "  @tag ", "@tag   "];
            let srv_parts = vec!["", ":srv ", "  :srv ", ":srv   "];
            let end_parts = vec!["", "   "];

            let mut inputs = Vec::new();

            for tag_part in tag_parts.iter() {
                for srv_part in srv_parts.iter() {
                    for end_part in end_parts.iter() {
                        inputs.push(format!("{}{}{}", tag_part, srv_part, end_part));
                    }
                }
            }

            assert_eq!(inputs.len(), tag_parts.len() * srv_parts.len() * end_parts.len());
            assert!(inputs.len() > 0);

            inputs
        };

        for input in inputs.iter() {
            println!("input: {}$", input);
            assert_eq!("", strify(extract_verb(input.as_bytes())));
        }
    }

    #[test]
    fn test_extract_cap_client_with_colon() {
        let inputs = vec![
            "CAP REQ :multi-prefix sasl",
            "   CAP  REQ    :multi-prefix sasl",
            "   CAP  REQ  *   :multi-prefix sasl",
            "  @tag     CAP        REQ      :multi-prefix sasl",
        ];

        for input in inputs.iter() {
            println!("input: {}$", input);
            let cap = extract_cap(input.as_bytes(), false).unwrap();
            assert_eq!("REQ", strify(cap.subcommand));
            assert_eq!("multi-prefix sasl", strify(cap.trailing));
        }
    }

    #[test]
    fn test_extract_cap_client_no_colon() {
        let inputs = vec![
            "CAP REQ multi-prefix sasl",
            "   CAP  REQ    multi-prefix    sasl    ",
            "     @tag      CAP  REQ    multi-prefix sasl    ",
        ];

        for input in inputs.iter() {
            println!("input: {}$", input);
            let cap = extract_cap(input.as_bytes(), false).unwrap();
            assert_eq!("REQ", strify(cap.subcommand));
            assert_eq!("sasl", strify(cap.trailing));
        }
    }

    #[test]
    fn test_extract_cap_server_with_colon() {
        let inputs = vec![
            "CAP * ACK :multi-prefix sasl",
            "CAP * ACK * :multi-prefix sasl",
            "   CAP  *  ACK    :multi-prefix sasl",
            "   CAP  *  ACK  *  :multi-prefix sasl",
            "  @tag     CAP      * ACK      :multi-prefix sasl",
            "  @tag  CAP  you ACK      :multi-prefix sasl",
            "  @tag  CAP  you ACK  *    :multi-prefix sasl",
            ":me CAP you ACK :multi-prefix sasl",
        ];

        for input in inputs.iter() {
            println!("input: {}$", input);
            let cap = extract_cap(input.as_bytes(), true).unwrap();
            assert_eq!("ACK", strify(cap.subcommand));
            assert_eq!("multi-prefix sasl", strify(cap.trailing));
        }
    }

    #[test]
    fn test_extract_cap_server_no_colon() {
        let inputs = vec![
            "CAP * ACK multi-prefix  sasl",
            "   CAP  *  ACK    multi-prefix sasl  ",
            "  @tag     CAP      * ACK      multi-prefix  sasl",
            "  @tag  CAP  you ACK      multi-prefix sasl     ",
            ":me CAP you ACK multi-prefix sasl ",
        ];

        for input in inputs.iter() {
            println!("input: {}$", input);
            let cap = extract_cap(input.as_bytes(), true).unwrap();
            assert_eq!("ACK", strify(cap.subcommand));
            assert_eq!("sasl", strify(cap.trailing));
        }
    }

    #[test]
    fn test_extract_cap_invalid() {
        let inputs = vec![
            "NICK",
            "@tag NICK",
            ":me NICK",
            "@tag :me NICK",
            "NICK extra"
        ];

        for input in inputs.iter() {
            println!("input: {}$", input);
            assert!(extract_cap(input.as_bytes(), false).is_none());
        }
    }

    #[bench]
    fn bench_extract_verb(b: &mut test::Bencher) {
        let real_input = ":irc.mynetwork.net PRIVMSG #channel :hello everyone".as_bytes();

        b.iter(|| {
            let input = test::black_box(real_input);
            test::black_box(extract_verb(input));
        });
    }

    #[bench]
    fn bench_extract_cap(b: &mut test::Bencher) {
        let real_input = ":irc.server CAP * ACK :multi-prefix sasl".as_bytes();

        b.iter(|| {
            let input = test::black_box(real_input);
            test::black_box(extract_cap(input, true));
        });
    }
}
