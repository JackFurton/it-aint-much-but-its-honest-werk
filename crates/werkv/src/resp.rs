#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    Simple(String),
    Error(String),
    Integer(i64),
    Bulk(Vec<u8>),
    Null,
    Array(Vec<Frame>),
}

#[derive(Debug, PartialEq)]
pub enum ParseError {
    Incomplete,
    Invalid(String),
}

/// Parses one frame from the front of `buf`, returning it and how many bytes it used.
pub fn parse(buf: &[u8]) -> Result<(Frame, usize), ParseError> {
    let (&tag, rest) = buf.split_first().ok_or(ParseError::Incomplete)?;
    let (line, line_len) = read_line(rest)?;
    let header_len = 1 + line_len;

    match tag {
        b'+' => Ok((Frame::Simple(utf8(line)?), header_len)),
        b'-' => Ok((Frame::Error(utf8(line)?), header_len)),
        b':' => Ok((Frame::Integer(int(line)?), header_len)),
        b'$' => {
            let Ok(len) = usize::try_from(int(line)?) else {
                return Ok((Frame::Null, header_len));
            };
            let end = header_len + len;
            if buf.len() < end + 2 {
                return Err(ParseError::Incomplete);
            }
            if &buf[end..end + 2] != b"\r\n" {
                return Err(ParseError::Invalid(
                    "bulk string not terminated by CRLF".into(),
                ));
            }
            Ok((Frame::Bulk(buf[header_len..end].to_vec()), end + 2))
        }
        b'*' => {
            let Ok(len) = usize::try_from(int(line)?) else {
                return Ok((Frame::Null, header_len));
            };
            let mut items = Vec::new();
            let mut pos = header_len;
            for _ in 0..len {
                let (item, used) = parse(&buf[pos..])?;
                items.push(item);
                pos += used;
            }
            Ok((Frame::Array(items), pos))
        }
        other => Err(ParseError::Invalid(format!(
            "unknown type byte {other:#04x}"
        ))),
    }
}

fn read_line(buf: &[u8]) -> Result<(&[u8], usize), ParseError> {
    let end = buf
        .windows(2)
        .position(|w| w == b"\r\n")
        .ok_or(ParseError::Incomplete)?;
    Ok((&buf[..end], end + 2))
}

fn utf8(bytes: &[u8]) -> Result<String, ParseError> {
    String::from_utf8(bytes.to_vec()).map_err(|_| ParseError::Invalid("invalid UTF-8".into()))
}

fn int(bytes: &[u8]) -> Result<i64, ParseError> {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| ParseError::Invalid("invalid integer".into()))
}

impl Frame {
    pub fn write_to(&self, out: &mut Vec<u8>) {
        match self {
            Frame::Simple(s) => write_line(out, b'+', s.as_bytes()),
            Frame::Error(s) => write_line(out, b'-', s.as_bytes()),
            Frame::Integer(n) => write_line(out, b':', n.to_string().as_bytes()),
            Frame::Bulk(data) => {
                write_line(out, b'$', data.len().to_string().as_bytes());
                out.extend_from_slice(data);
                out.extend_from_slice(b"\r\n");
            }
            Frame::Null => out.extend_from_slice(b"$-1\r\n"),
            Frame::Array(items) => {
                write_line(out, b'*', items.len().to_string().as_bytes());
                for item in items {
                    item.write_to(out);
                }
            }
        }
    }
}

fn write_line(out: &mut Vec<u8>, tag: u8, body: &[u8]) {
    out.push(tag);
    out.extend_from_slice(body);
    out.extend_from_slice(b"\r\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(frame: &Frame) -> Vec<u8> {
        let mut out = Vec::new();
        frame.write_to(&mut out);
        out
    }

    #[test]
    fn parses_command_array() {
        let input = b"*2\r\n$3\r\nGET\r\n$3\r\nfoo\r\n";
        let (frame, used) = parse(input).unwrap();
        assert_eq!(used, input.len());
        assert_eq!(
            frame,
            Frame::Array(vec![
                Frame::Bulk(b"GET".to_vec()),
                Frame::Bulk(b"foo".to_vec())
            ])
        );
    }

    #[test]
    fn partial_input_is_incomplete() {
        let input = b"*2\r\n$3\r\nGET\r\n$3\r\nfo";
        for end in 0..input.len() {
            assert_eq!(
                parse(&input[..end]),
                Err(ParseError::Incomplete),
                "prefix {end}"
            );
        }
    }

    #[test]
    fn leaves_trailing_bytes_for_next_frame() {
        let (frame, used) = parse(b"+OK\r\n:42\r\n").unwrap();
        assert_eq!(frame, Frame::Simple("OK".into()));
        assert_eq!(used, 5);
    }

    #[test]
    fn bulk_strings_are_binary_safe() {
        let frame = Frame::Bulk(b"a\r\nb\0c".to_vec());
        assert_eq!(parse(&encode(&frame)).unwrap().0, frame);
    }

    #[test]
    fn round_trips() {
        let frame = Frame::Array(vec![
            Frame::Simple("OK".into()),
            Frame::Error("ERR nope".into()),
            Frame::Integer(-7),
            Frame::Null,
            Frame::Array(vec![]),
        ]);
        assert_eq!(
            parse(&encode(&frame)).unwrap(),
            (frame.clone(), encode(&frame).len())
        );
    }

    #[test]
    fn rejects_unknown_type() {
        assert!(matches!(parse(b"?huh\r\n"), Err(ParseError::Invalid(_))));
    }
}
