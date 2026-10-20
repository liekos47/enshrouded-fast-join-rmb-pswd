# Enshrouded Quick Join

A small client-side plugin for Enshrouded with two features:

- **Fast join**: the Join screen no longer asks Steam for the worldwide server list. That query held every join
  reply for about 17 seconds, so a password confirmed quickly timed out ("Lobby creation failed: 16"). With it off,
  joins go through in a second or two. The Join screen then lists favourites, history, friends and player-hosted
  games, not every server in the world.
- **Remember password**: type a server's password once in the Join box. After that, press Join with the box
  **empty** and the saved password is sent. Typing a password always wins and replaces the saved one.

No gameplay changes, client only. Tested on the Steam client, Enshrouded 1.0 build 1269579.

## How it runs

Enshrouded has no mod API. This is a plugin (`quickjoin.dll`) for the **shroudkit** loader, a `dbghelp.dll` proxy
placed next to `enshrouded.exe` that loads `shroudkit/plugins/*.dll` at start-up. The loader is not part of this
repository; it ships in the ready-made package together with this plugin.

Game folder layout:

```
Enshrouded/
  enshrouded.exe
  dbghelp.dll                     (shroudkit loader)
  shroudkit/
    config.ini                    [quickjoin] fast_join = true, remember_password = true
    quickjoin.ini                 saved passwords (created on first start, plain text, do not share)
    plugins/quickjoin.dll         (this repository)
```

Both switches also show in the loader's in-game menu (Insert). The log is `shroudkit/shroudkit.log`.

## Build

Rust 1.85+ (edition 2024), Windows x64:

```
cargo build --release -p quickjoin
cargo test --release -p quickjoin
```

The output is `target/release/quickjoin.dll`. `shroudkit-api` is the loader's plugin interface (API version 7).

## How it works

Every patch is located by a byte signature that must match exactly once, so after a game update a feature switches
itself off ("signature not found" in the log) and the game keeps working.

### Fast join (`quickjoin/src/lib.rs`)

Each time the Join screen opens, its server-browser tick calls `ISteamMatchmakingServers` three times:
RequestInternetServerList, RequestFavoritesServerList and RequestHistoryServerList. The internet query takes about
17 s, and Steam delivers none of the join's replies (lobby, auth ticket, connection) until it is answered. Two
in-place patches stop the internet request:

| What | Signature | Change |
|---|---|---|
| `mov rax,[rcx]; call [rax]` (the request) | `48 8B 01 FF 10 8B 57 B4 45 33 C9 48 89 47 B8` | `xor eax,eax` + nops: no request, handle 0 |
| `test rdx,rdx; je fail` (handle check) | `48 85 D2 74 15 4C 39 67 C0 74 0F 48 85 C0 74 0A B0 02`, +3 | `je` → two nops |

Every reader of the internet-list handle tolerates 0.

### Remember password (`remember.rs`, `cave.rs`, `store.rs`)

The Join screen sets up a game string for the password box: pointer `[rbp+80h]` to a 64-byte inline buffer at
`rbp+0A0h`, length `[rbp+88h]`, capacity `[rbp+90h]`. When a join is confirmed (OK, Enter, or the extra Join button
for servers with an optional password), the code reads that string and posts it to the pregame logic, which
starts the join:

```
mov rax, [rbp+80h]      <- detour (signature 48 8B 85 80 00 00 00 48 89 44 24 70 48 8B 85 88 00 00 00 48 89 44 24 78)
mov [rsp+70h], rax
mov rax, [rbp+88h]
mov [rsp+78h], rax
```

The detour goes through a cave that saves the flags, all general registers and xmm0-5, and calls Rust:

- something typed → it is saved in `quickjoin.ini` under a key made from the selected server's 34-byte identifier
  (the block at `rdi+40h` in the Join screen's state);
- box empty and a password saved for that server → the password is written into the screen's own inline buffer
  (only if it is still the untouched inline buffer and fits), and the length is set. The game then sends it as if
  it had been typed.

While the switch is off, the cave only runs the original instruction.

## Notes

- Passwords are stored as plain text in `shroudkit/quickjoin.ini`. Never share that file.
- Starting the game with Steam's `+connect` / `+password` options does not help here. A start-up `+password` makes
  every Join-screen join fail with "wrong password", and the game writes its command line, password included, to
  `enshrouded.log`.
