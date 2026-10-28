use crate::db::Db;
use crate::resp::Frame;

pub fn execute(frame: Frame, db: &Db) -> Frame {
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
        (b"get", [key]) => db.get(key).map_or(Frame::Null, Frame::Bulk),
        (b"set", [key, value]) => {
            db.set(key.clone(), value.clone());
            Frame::Simple("OK".into())
        }
        (b"del", keys) if !keys.is_empty() => {
            Frame::Integer(keys.iter().filter(|key| db.del(key)).count() as i64)
        }
        // redis-cli asks for command docs on connect; an empty reply just turns off its hints.
        (b"command", _) => Frame::Array(vec![]),
        (b"ping" | b"echo" | b"get" | b"set" | b"del", _) => Frame::Error(format!(
            "ERR wrong number of arguments for '{}' command",
            String::from_utf8_lossy(&name)
        )),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn run(db: &Db, args: &[&str]) -> Frame {
        let frame = Frame::Array(
            args.iter()
                .map(|a| Frame::Bulk(a.as_bytes().to_vec()))
                .collect(),
        );
        execute(frame, db)
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
