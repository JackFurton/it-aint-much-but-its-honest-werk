use std::time::{Duration, Instant};

use crate::db::{Db, Ttl};
use crate::resp::Frame;

pub fn execute(frame: Frame, db: &Db, now: Instant) -> Frame {
    let Some(args) = into_args(frame) else {
        return Frame::Error("ERR expected an array of bulk strings".into());
    };
    let Some((name, args)) = args.split_first() else {
        return Frame::Error("ERR empty command".into());
    };
    let name = name.to_ascii_lowercase();

    match (name.as_slice(), args) {
        (b"ping", []) => Frame::Simple("PONG".into()),
        (b"ping" | b"echo", [msg]) => Frame::Bulk(msg.clone()),
        (b"get", [key]) => db.get(key, now).map_or(Frame::Null, Frame::Bulk),
        (b"set", [key, value]) => {
            db.set(key.clone(), value.clone());
            Frame::Simple("OK".into())
        }
        (b"del", keys) if !keys.is_empty() => {
            Frame::Integer(keys.iter().filter(|key| db.del(key, now)).count() as i64)
        }
        (b"expire", [key, seconds]) => {
            let Some(seconds) = parse_int(seconds) else {
                return Frame::Error("ERR value is not an integer or out of range".into());
            };
            // Redis treats a non-positive timeout as "delete now".
            let Ok(seconds) = u64::try_from(seconds) else {
                return Frame::Integer(db.del(key, now) as i64);
            };
            let Some(at) = now.checked_add(Duration::from_secs(seconds)) else {
                return Frame::Error("ERR invalid expire time in 'expire' command".into());
            };
            Frame::Integer(db.expire(key, at, now) as i64)
        }
        (b"ttl", [key]) => Frame::Integer(match db.ttl(key, now) {
            Ttl::Missing => -2,
            Ttl::Persistent => -1,
            // Round to the nearest second like Redis does, so a fresh EXPIRE 10 reads back as 10.
            Ttl::Remaining(left) => ((left.as_millis() + 500) / 1000) as i64,
        }),
        // redis-cli asks for command docs on connect; an empty reply just turns off its hints.
        (b"command", _) => Frame::Array(vec![]),
        (b"ping" | b"echo" | b"get" | b"set" | b"del" | b"expire" | b"ttl", _) => {
            Frame::Error(format!(
                "ERR wrong number of arguments for '{}' command",
                String::from_utf8_lossy(&name)
            ))
        }
        _ => Frame::Error(format!(
            "ERR unknown command '{}'",
            String::from_utf8_lossy(&name)
        )),
    }
}

fn into_args(frame: Frame) -> Option<Vec<Vec<u8>>> {
    let Frame::Array(items) = frame else {
        return None;
    };
    items
        .into_iter()
        .map(|item| match item {
            Frame::Bulk(bytes) => Some(bytes),
            _ => None,
        })
        .collect()
}

fn parse_int(bytes: &[u8]) -> Option<i64> {
    std::str::from_utf8(bytes).ok()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(db: &Db, args: &[&str]) -> Frame {
        run_at(db, args, Instant::now())
    }

    fn run_at(db: &Db, args: &[&str], now: Instant) -> Frame {
        let frame = Frame::Array(
            args.iter()
                .map(|a| Frame::Bulk(a.as_bytes().to_vec()))
                .collect(),
        );
        execute(frame, db, now)
    }

    #[test]
    fn expire_and_ttl() {
        let db = Db::default();
        let now = Instant::now();
        let at = |secs| now + Duration::from_secs(secs);

        assert_eq!(run_at(&db, &["TTL", "k"], now), Frame::Integer(-2));
        assert_eq!(run_at(&db, &["EXPIRE", "k", "10"], now), Frame::Integer(0));

        run_at(&db, &["SET", "k", "v"], now);
        assert_eq!(run_at(&db, &["TTL", "k"], now), Frame::Integer(-1));
        assert_eq!(run_at(&db, &["EXPIRE", "k", "10"], now), Frame::Integer(1));
        assert_eq!(run_at(&db, &["TTL", "k"], now), Frame::Integer(10));
        assert_eq!(run_at(&db, &["TTL", "k"], at(7)), Frame::Integer(3));
        assert_eq!(run_at(&db, &["GET", "k"], at(10)), Frame::Null);
    }

    #[test]
    fn expire_edge_cases() {
        let db = Db::default();
        let now = Instant::now();
        run_at(&db, &["SET", "k", "v"], now);

        assert!(matches!(
            run_at(&db, &["EXPIRE", "k", "soon"], now),
            Frame::Error(_)
        ));
        assert!(matches!(
            run_at(&db, &["EXPIRE", "k", &i64::MAX.to_string()], now),
            Frame::Error(e) if e.contains("invalid expire time")
        ));
        assert_eq!(run_at(&db, &["EXPIRE", "k", "-1"], now), Frame::Integer(1));
        assert_eq!(run_at(&db, &["GET", "k"], now), Frame::Null);
    }

    #[test]
    fn set_get_del() {
        let db = Db::default();
        assert_eq!(run(&db, &["GET", "k"]), Frame::Null);
        assert_eq!(run(&db, &["SET", "k", "v"]), Frame::Simple("OK".into()));
        assert_eq!(run(&db, &["get", "k"]), Frame::Bulk(b"v".to_vec()));
        assert_eq!(run(&db, &["DEL", "k", "missing"]), Frame::Integer(1));
        assert_eq!(run(&db, &["GET", "k"]), Frame::Null);
    }

    #[test]
    fn wrong_arity_and_unknown() {
        let db = Db::default();
        assert!(matches!(run(&db, &["GET"]), Frame::Error(e) if e.contains("wrong number")));
        assert!(matches!(run(&db, &["NOPE"]), Frame::Error(e) if e.contains("unknown command")));
    }
}
