# What DayZ says over A2S

DayZ answers Valve's server queries (A2S) on its query port, like any Source
game, but it puts most of what a launcher needs where Valve's spec leaves
room: in A2S_INFO's keywords and, in a binary block of Bohemia's, among the
A2S_RULES. This page records what is there, how `dz-a2s` reads it, and where
each fact comes from. Checked against live servers in September 2026 (DayZ
1.29).

## A2S_INFO

The standard answer ([Valve][valve]), with a few DayZ habits:

- The request needs a challenge: the first `A2S_INFO` gets `0x41` and a
  4-byte challenge to send back ([Valve][valve]).
- The app id is 221100, which does not fit the 16-bit field: read the 64-bit
  GameID from the extended data instead ([Valve][valve], `async-a2s`).
- `bots` is often padded to equal `players` to fake a full server; the app
  counts `players - bots` as the real players (`human_player_count`).
- The extended data flag carries the game port, the Steam id and the
  **keywords**, a comma-separated list of tags:

```
battleye,no3rd,external,privHive,shard123ABC,lqs0,etm8.000000,entm2.000000,mod,20:43
```

| Tag                   | Meaning                                              | Read as                  |
| --------------------- | ---------------------------------------------------- | ------------------------ |
| `battleye`            | BattlEye is on                                       | `battleye`               |
| `no3rd`               | Third-person view is off                             | `first_person_only`      |
| `external`            | A community server (official ones lack it)           | `external`               |
| `privHive`            | The server keeps its own characters                  | `private_hive`           |
| `mod`                 | The server runs mods                                 | `modded`                 |
| `whitelisting`        | Whitelisted players only                             | `whitelisted`            |
| `allowedFilePatching` | Clients may load unpacked files                      | `file_patching`          |
| `isDLC`               | The map is a DLC                                     | `dlc`                    |
| `shardXXX`            | Hive shard (`000`, `001` on official servers)        | `shard`                  |
| `lqsN`                | Players waiting in the login queue                   | `login_queue`            |
| `etmX`                | Day time acceleration                                | `time_accel`             |
| `entmX`               | Night time acceleration                              | `night_time_accel`       |
| `HH:MM`               | In-game clock                                        | `game_time`              |
| `portN`               | Game port, when stated here                          | `game_port`              |

Sources: [woozymasta/a2s `keywords/dayz.go`][wm-keywords] and its
[package documentation][wm-keywords-doc]; [velvetcache][velvet] for the
master-server spelling. The Steam master server (the Web API's server list)
returns the same tags joined by `#`, with `-` as the decimal point
(`etm6-000000`); `DayzInfo::parse` accepts both. An official server is one
without `external` and without `privHive`; a live one sent
`battleye,shard000,lqs0,etm4.200000,entm4.000000,06:56`.

## A2S_RULES

Also challenged. The answer is too big for one datagram on a modded server,
so it comes split ([Valve][valve]): each fragment starts `FE FF FF FF`, then
the answer id (i32; bit 31 would mean bzip2, which DayZ never uses), the
fragment count (u8), the fragment number (u8) and a size (u16), then the
data. The fragments put back in order start with `FF FF FF FF 45`.

Then a count (u16) and name/value pairs, NUL-terminated. DayZ sends nine
plain rules:

| Rule              | Example         | Meaning                              |
| ----------------- | --------------- | ------------------------------------ |
| `allowedBuild`    | `0`             | Oldest client build allowed          |
| `clientPort`      | `0`             |                                      |
| `dedicated`       | `1`             | A dedicated server                   |
| `island`          | `chernarusplus` | The map                              |
| `language`        | `65545`         | Language id                          |
| `platform`        | `win` / `lin`   | The server's OS                      |
| `requiredBuild`   | `0`             |                                      |
| `requiredVersion` | `129`           | Game version, `1.29`                 |
| `timeLeft`        | `15`            |                                      |

Sources: [woozymasta/a2s][wm] (`a3sb/rules_dayz.go`, [raw][wm-rules]) and the captured
answers in `crates/core/a2s/tests/fixtures`.

### The binary block

The other rules have two-byte names, the chunk's index (from 1) and the
number of chunks, and their values put together in index order form
Bohemia's "Server Browser Protocol 3" block ([BI wiki][bi]). The count byte
is printable from 32 chunks on (a space), so a chunk is recognised by its
index byte alone. A chunk is at most 124 bytes; NULs and a few other bytes are
escaped: `01 01` is `01`, `01 02` is `00`, `01 03` is `FF`.

DayZ's block is simpler than Arma 3's: no flags, DLC mask or difficulty
([woozymasta][wm-rules], checked on live answers). Little-endian:

```
u32   version (2)
u8    mod count
      per mod:
u32     hash of the mod's files
u8      id width (low nibble: 1, 4 or 8 bytes)
..      Steam Workshop id
u8+..   name (length, then bytes)
u8    signature count
      per signature: u8+..  name of a .bikey the server accepts
u8+.. description ("Official DayZ game server" on official servers, else empty)
```

A live answer from a server running 101 mods came in 32 chunks, the last of
which never arrived: the mods were whole and only the signatures were cut.
`parse_rules` keeps the mods in that case; it gives up (`mods_unreadable`)
only when a chunk is missing in the middle or the block ends inside the mods.

## What the app reads, and how

- `dz_a2s::DayzInfo::parse` reads the keywords (tests on live strings).
- `dz_a2s::rules::fetch_rules` sends A2S_RULES itself: challenge (up to three
  in a row), single or split answers, 3 s per attempt, one retry after 300 ms.
  `parse_rules` reads the plain rules and the block into mods with ids and
  hashes, signatures and description. Tested on four answers captured once
  (an official server, 2, 4 and 101 mods).
- `query_a2s` returns `dayz` and `mods_a2s` beside the older fields; the
  server panel shows the hive, the queue, the time speed and the live clock,
  and an unlisted server's mods with their install state.

## What async-a2s misses

`async-a2s` (the crate the app uses for INFO and PLAYERS) also reads rules,
but for DayZ:

- `Rule::from_cursor` recognises a chunk only when both name bytes are
  control characters, so from 32 chunks on (count byte `0x20`) the block is
  taken for plain rules and the mods are lost.
- It turns the mods into rules named `mod` holding only the name: the
  Workshop id and hash that `protocol3::parse_protocol3` read are dropped,
  and no raw rules are exposed to read them otherwise.
- `parse_protocol3` takes a first byte of 3 or less for an Arma 3 version,
  so a DayZ block with 0 to 3 mods (the version u32 is consumed as the
  signature) is read as a flags and DLC header and fails or yields garbage.
- Signatures and the description are not read.

Exposing the decoded `Protocol3Data` (or the raw rules) and reading DayZ's
layout as above would let `dz-a2s` drop its own reader.

[valve]: https://developer.valvesoftware.com/wiki/Server_queries
[bi]: https://community.bistudio.com/wiki/Arma_3:_ServerBrowserProtocol3
[wm]: https://github.com/WoozyMasta/a2s
[wm-keywords]: https://raw.githubusercontent.com/WoozyMasta/a2s/master/pkg/keywords/dayz.go
[wm-keywords-doc]: https://pkg.go.dev/github.com/woozymasta/a2s/pkg/keywords
[wm-rules]: https://raw.githubusercontent.com/WoozyMasta/a2s/master/pkg/a3sb/rules_dayz.go
[velvet]: https://velvetcache.org/2024/05/23/dayz-server-browsers/
