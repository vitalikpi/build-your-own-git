# Plan: Implement `clone` (Smart HTTP) — a learning roadmap

> This is a **teaching plan**: it explains every format and algorithm you need,
> with byte-level examples, but contains **no Rust code**. You write all the
> implementation yourself.

## Strategy: build clone out of smaller, locally-testable subcommands

The biggest risk with this stage is debugging *networking + a new binary format
+ checkout* all at once. Avoid that. Build `clone` as a sequence of smaller
subcommands, each of which you can test on its own against **real git** before
moving on. Only the final step talks to the network.

The milestones (sizes are a Python reference solution's changed-line counts, just
to show relative size):

| # | Subcommand | Size | What you can test it with |
|---|------------|------|---------------------------|
| M1 | `checkout-empty <commit>` | ~42 | your own commits from earlier stages |
| M2 | `unpack-objects` (undeltified) | ~56 | `git pack-objects --depth=0` |
| M3 | `unpack-objects` (REF_DELTA) | ~57 | `git pack-objects` + crafted files |
| M4 | `ls-remote <url>` | ~25 | compare to `git ls-remote <url>` |
| M5 | `clone <url> <dir>` | ~45 | `diff -r` vs a real `git clone` |

`clone` at the end is just gluing them together: *create dir → `git init` →
ls-remote → fetch packfile → unpack-objects → checkout-empty*.

The whole thing is: *discover refs → ask for a packfile → unpack it into objects →
walk the HEAD commit's tree and write files*.

---

## Prerequisite: Cargo + a small refactor

1. **Add `reqwest` (blocking).** It is NOT currently in `Cargo.toml` despite the
   stage text. Add it with the `blocking` feature so you make synchronous calls
   (no tokio). You already have `flate2` (zlib), `sha1`, `anyhow`, `bytes`.
2. **Fix `write_object` for reuse.** It calls `fs::create_dir(partition_path)`
   unconditionally. During unpack, many objects share 2-char prefixes, so the 2nd
   object with the same prefix errors with `AlreadyExists`. Make the directory
   creation tolerant of an already-existing dir (treat `AlreadyExists` as success,
   or create-all). Verify this doesn't regress `hash-object`.
3. **Path handling for the clone target.** `init` hardcodes `.git` in the current
   directory. For `clone` you must create `.git` **inside `<some_dir>`**. Simplest:
   create `<some_dir>`, `cd` into it, run your `init` logic, then everything that
   uses relative `.git/...` paths just works.

---

## M1 — `checkout-empty <commit>`

Like `git checkout`, but it **assumes the working directory is empty** (except for
`.git`), so it only has to *write* files and directories — no diffing, no deleting.
This is the natural continuation of `write-tree`/`commit-tree`: there you *wrote*
commit and tree objects; here you *read* them back out onto disk.

Algorithm:
1. Read the commit object → its content starts with `tree <40-hex>\n`; parse out
   the tree SHA.
2. Recursively walk the tree (see **Reference: tree entry format**):
   - `100644` → regular file: write the blob's bytes to disk.
   - `100755` → executable file: write the blob, then set the exec bit
     (`PermissionsExt`, mode `0o755` — you already import `PermissionsExt`).
   - `40000` → subdirectory: create the dir and recurse.
   - `120000`/`160000` (symlink/submodule): unlikely in the test repo; skip or
     handle gracefully.
3. Reuse `read_object` to fetch each blob/tree body. Note it strips the
   `"<type> <len>\0"` header (to the first NUL), which is exactly what you want for
   the entry loop — but double-check against a real tree.

**Refactor opportunity:** `ls_tree` already decodes the tree-entry byte layout.
Pull that parsing into a shared "parse tree entries" helper and use it in both
places instead of duplicating the byte-walking loop.

**Local test:** make a commit with real git in a scratch repo, then run your
`checkout-empty <that-commit-sha>` into an empty dir and `diff -r` against the real
checkout.

---

## M2 — `unpack-objects` (undeltified only)

Like `git unpack-objects`, but only for packfiles with **no** deltified objects.
Reads a packfile (from stdin or a file) and writes every object into
`.git/objects` via your existing `write_object`.

Steps (see **Reference: packfile format**):
1. Parse the 12-byte header: `"PACK"`, version (big-endian, expect 2), object
   count N (big-endian).
2. For each of N objects: read the variable-length type+size header, then
   zlib-decompress the body. The decompressed bytes **are** the object content for
   types commit/tree/blob/tag → `write_object(content, type_str)`.
3. Advance to the next object using the decoder's **`total_in()`** (how many
   *compressed* bytes it consumed) — you only know the *uncompressed* size, so this
   is the only way to find the next object boundary.

**Local test:** create a delta-free packfile and feed it in:
```sh
git pack-objects --depth=0 <prefix>   # --depth=0 disables deltas
```
Then compare the set of SHAs you wrote against what `git unpack-objects` produces.

---

## M3 — `unpack-objects` with REF_DELTA

Extend M2 to also resolve **REF_DELTA** (type 7) objects. (Modern git —
e.g. 2.43 — emits these from `git pack-objects`. You will not need **OFS_DELTA**
because in M4/M5 you simply won't advertise support for it, so the server won't
send it.)

A REF_DELTA object is: 20 raw bytes of the **base object's SHA-1** (binary), then a
zlib-compressed **delta stream**. Reconstruct the target by applying the delta to
the base (see **Reference: delta format**). The reconstructed object's **type is
the base's type**; store it with `write_object(..., base_type)`.

**Ordering gotcha:** a delta's base may appear later in the pack. Resolve all
non-deltas first, then sweep the deltas repeatedly until none remain (keeping an
in-memory `sha -> (type, bytes)` map makes this easy and also serves checkout).

**Local test — craft tiny deltas you can reason about:** add two similar,
not-too-small files with `git hash-object -w`, then pack just those two with
`git pack-objects`. Useful shapes:
- If one file is a **prefix** of the other → the big one is stored undeltified and
  the small one is a single "copy from base" instruction.
- Two files of the **same size with one byte different in the middle** → the delta
  is "copy, insert, copy".

---

## M4 — `ls-remote <url>`

Like `git ls-remote`: fetch the remote's ref → SHA advertisement over HTTP and
print it. This is the first networked step and isolates "can I talk the protocol
and parse pkt-lines" from all the packfile logic.

Two protocol options — pick one and stay consistent through M5:

- **Smart HTTP v1 (documented in detail below).** `GET
  <url>/info/refs?service=git-upload-pack`; parse the pkt-line ref advertisement.
- **Protocol v2.** Same endpoint but send `Git-Protocol: version=2`, then you can
  skip capability parsing and issue explicit `ls-refs` / `fetch` commands. Slightly
  cleaner request/response shape; the forum author found it easier.

Either works against GitHub. See **Reference: pkt-line** and **Reference: ref
discovery (v1)** below.

**Local test:** run your `ls-remote <url>` and diff the output against real
`git ls-remote <url>`.

---

## M5 — `clone <url> <dir>`

Now just combine everything. New code here is only the **fetch** (requesting the
packfile); the rest is M1–M4 wired together:

1. Create `<dir>`, `cd` into it, run your `init` logic.
2. `ls-remote` → pick the SHA to fetch (the one advertised for `HEAD`).
3. **Fetch the packfile** (see **Reference: requesting the packfile**). Tips:
   - Advertise `no-progress` to cut sideband/progress clutter from the response
     (effectively `clone -q`).
   - Do **not** advertise `ofs-delta`, so the server sends only REF_DELTA (which M3
     handles).
4. `unpack-objects` on the returned packfile.
5. `checkout-empty` the fetched commit.
6. Write refs so `.git` looks real: `.git/HEAD` → `ref: refs/heads/<branch>\n`,
   and `.git/refs/heads/<branch>` → commit SHA + `\n`.

**Local test:** clone a small public repo and `diff -r` against a real
`git clone`. Then confirm real git is happy with your output: `git log`,
`git status`, `git cat-file -p HEAD` inside your clone should all work.

---

# Reference specs

## pkt-line
Smart HTTP frames almost everything in **pkt-line**. One line is
`<4 hex-digit length><payload>`:
- The 4 hex digits are the length of the **entire** line **including** those 4
  bytes. Payload `hello\n` (6 bytes) → total 10 → `000ahello\n`.
- `0000` = flush packet (delimiter/end, no payload). `0001`/`0002` are v2
  delimiter / response-end.
- To **read**: read 4 bytes → hex → N. N==0 is a flush; otherwise read N-4 payload
  bytes. Repeat. Write a helper that yields one payload at a time.

## Ref discovery (v1)
`GET <url>/info/refs?service=git-upload-pack` returns pkt-lines:
```
001e# service=git-upload-pack\n   <- service announcement
0000                              <- flush
<sha> <refname>\0<capabilities>\n <- FIRST ref line: capabilities after a NUL
<sha> <refname>\n                 <- subsequent refs
...
0000                              <- flush
```
- Grab the 40-hex SHA for `HEAD` (also often `refs/heads/master`/`main`).
- The first ref line appends capabilities after a `\0` — split on the NUL before
  reading the refname.
- Sanity-check it starts with `# service=git-upload-pack`; HTML back = wrong URL.

## Requesting the packfile (v1)
`POST <url>/git-upload-pack`, header
`Content-Type: application/x-git-upload-pack-request`. Body is pkt-lines you build:
```
<want-line>   e.g. "want <sha> <capabilities>\n"  (wrapped as a pkt-line)
0000          flush
0009done\n    pkt-line whose payload is "done\n"
```
Put capabilities (e.g. `no-progress`; deliberately **omit** `ofs-delta` and
`side-band`) on the first `want` line. Response:
```
0008NAK\n     <- one pkt-line ack
PACK....      <- raw packfile bytes follow immediately
```
Read the first pkt-line (expect `NAK`); everything after it is the packfile.
(If you advertise side-band, the pack is instead chunked inside pkt-lines with a
1-byte stream code — avoid that by not requesting it.)

## Packfile format
12-byte header: `"PACK"` + version (BE, 2) + object count N (BE). The **last 20
bytes** of the pack are a SHA-1 checksum of all preceding bytes (verify or skip).

Each object begins with a variable-length type+size header ("sneaky numbers"):
- First byte: bit 7 (`0x80`) = continuation; bits 6–4 (`0x70`) = type; bits 3–0
  (`0x0f`) = low 4 bits of size.
- While continuation is set, read another byte; its low 7 bits are the next-more-
  significant size bits. **7-bit groups are little-endian** (first byte = least
  significant).

Worked example `0x96 0x0d`:
- `0x96` = `1001 0110`: continuation set, type `001`=1 (commit), size low nibble
  `0110`=6.
- `0x0d` = `0000 1101`: last byte, 7 bits `0001101`=13.
- size = 6 + (13 << 4) = **214**.

Type codes: `1 commit  2 tree  3 blob  4 tag  6 ofs-delta  7 ref-delta`.

Each object's body is **zlib-compressed**; use `total_in()` to find where the next
object starts (you only know the uncompressed size).

## Delta format (REF_DELTA)
Layout: 20 raw bytes of base SHA-1, then a zlib-compressed delta stream:
1. **Source size** — varint (same 7-bit LE scheme). Should equal base length.
2. **Target size** — varint. Size of the reconstructed object.
3. Instructions, each led by one command byte:
   - **Copy** (MSB `0x80` set): low 4 bits pick up to 4 **offset** bytes, bits 4–6
     pick up to 3 **size** bytes; each present byte adds the next LE 8-bit group.
     Copy `base[offset .. offset+size]`. If computed size == 0, treat as
     `0x10000` (65536).
   - **Insert** (MSB clear, byte != 0): low 7 bits = length 1–127; copy that many
     bytes **from the delta stream itself** into the output.
   - `0x00` is reserved/invalid.
   Stop when you've produced `target size` bytes.

## Tree entry format
A tree object body is a sequence of entries:
```
<mode> <name>\0<20-byte-binary-sha>
```
Modes: `100644` file, `100755` exec file, `40000` dir, `120000` symlink,
`160000` gitlink. `ls_tree` already walks this exact layout.

---

## Resources
- `gitprotocol-pack` (ref discovery + want/have negotiation; v2 commands)
- `gitformat-pack` (pack header, object headers, delta instructions)
- "Unpacking Git packfiles" and "Sneaky git number encoding" (varint + delta
  walkthroughs)
- **mitmproxy** — proxy your own `git clone`/`git ls-remote` to see the exact
  requests/responses; its web UI lets you edit and replay queries while you reverse
  the protocol.

## Classic failure points (check these first when a test fails)
- pkt-line length includes the 4 length bytes themselves.
- varint 7-bit groups are little-endian (first byte = least significant).
- advance the pack cursor by `total_in()`, not the uncompressed size.
- delta copy vs insert bit handling; copy size==0 means 65536.
- deltas resolved only after their base exists (resolve loop).
- `write_object` no longer panics on an existing prefix directory.
- objects/refs written inside `<dir>/.git`, not your CWD.
