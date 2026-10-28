# it ain't much but it's honest werk

A Rust workspace for learning by building things that are bigger than a weekend.

## werkv

A Redis-compatible key-value server, growing toward a replicated database. It speaks RESP, so `redis-cli` and any Redis client library work against it.

```
cargo run -p werkv              # listens on 127.0.0.1:6380
redis-cli -p 6380 set hello werk
redis-cli -p 6380 get hello
```

Right now: `PING`, `ECHO`, `GET`, `SET`, `DEL`, in memory only.

The roadmap lives in the milestones:

1. Speaks Redis: core commands, expiry, integration tests
2. Survives restarts: write-ahead log, crash recovery, LSM tree
3. Data types: lists, hashes, sorted sets, pub/sub
4. Raft: three nodes, kill one, lose nothing

https://github.com/JackFurton/it-aint-much-but-its-honest-werk/milestones

## Working on it

Version control is [jj](https://github.com/jj-vcs/jj), colocated with git, so plain git works too.

```
jj new main                     # start a change on top of main
jj describe -m "werkv: ..."     # write the message whenever
jj git push --change @          # push it as a branch, then open a PR
```

CI runs `cargo fmt --check`, `clippy` with warnings as errors, and `cargo test`.
