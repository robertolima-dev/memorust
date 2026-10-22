use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::sync::Mutex;

use crate::resp::{RespParseResult, encode_array, parse_resp_frame};
use crate::store::SnapshotEntry;
use crate::tokenizer::tokenize;

/// One persisted command, as its argument list (`["SET", "name", "Roberto"]`).
pub type Record = Vec<String>;

/// Append-only log of mutating commands.
///
/// Each record is stored as a RESP array of bulk strings. Lengths are
/// explicit, so any key or value (spaces, quotes, newlines, empty strings)
/// is replayed exactly as it was written.
///
/// Ordering is the caller's job: records must be appended in the same order
/// the store applies them (the command handler does both under the store's
/// write lock).
pub struct Aof {
    path: String,
    inner: Mutex<Inner>,
}

struct Inner {
    writer: BufWriter<File>,
    /// `Some` while a rewrite is running (between `start_rewrite` and
    /// `complete_rewrite`).
    rewrite: Option<RewriteState>,
}

/// What happened to the log while a rewrite was writing its snapshot.
#[derive(Default)]
struct RewriteState {
    /// Records appended after the snapshot was taken; they go at the end of
    /// the new file.
    pending: Vec<Record>,
    /// Set by FLUSHALL: the snapshot no longer describes the data.
    invalidated: bool,
}

impl Aof {
    pub fn new(path: &str) -> io::Result<Self> {
        Ok(Self {
            path: path.to_string(),
            inner: Mutex::new(Inner {
                writer: open_appender(path)?,
                rewrite: None,
            }),
        })
    }

    /// Buffers `records` as one unit (e.g. `SET` + `EXPIREAT` for a `SETEX`).
    pub fn append(&self, records: &[Record]) -> io::Result<()> {
        let mut inner = self.inner.lock().unwrap();

        // Only buffer here; durability is handled out-of-band by `flush`, called
        // ~once per second by the AOF flusher task. This keeps the fsync off the
        // per-command hot path (Redis' `appendfsync everysec` behaviour).
        for record in records {
            inner.writer.write_all(encode_array(record).as_bytes())?;
        }

        if let Some(rewrite) = &mut inner.rewrite {
            rewrite.pending.extend_from_slice(records);
        }

        Ok(())
    }

    /// Flushes the buffered writer to the OS and fsyncs it to disk.
    ///
    /// Bounds data loss to roughly one flush interval instead of paying a write
    /// syscall (and fsync) on every command.
    ///
    /// The expensive part (`sync_all`) runs *outside* the writer lock: we only
    /// hold the lock long enough to drain the buffer to the OS and clone the
    /// file handle. The clone shares the same open file description, so the
    /// fsync still targets this file — but `append` can keep buffering during
    /// it, avoiding a periodic latency spike on every command.
    pub fn flush(&self) -> io::Result<()> {
        let handle = {
            let mut inner = self.inner.lock().unwrap();

            inner.writer.flush()?;
            inner.writer.get_ref().try_clone()?
        };

        handle.sync_all()?;

        Ok(())
    }

    /// Reads every record back, in order.
    ///
    /// Also accepts the older plain-text format (one command per line), so an
    /// existing `appendonly.aof` keeps loading; new records are always RESP.
    ///
    /// A record cut off at the end of the file (a crash mid-write) is dropped
    /// and the file is truncated to the last complete record, so later appends
    /// don't land after the garbage. Corruption anywhere else is an error.
    pub fn load(&self) -> io::Result<Vec<Record>> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(Vec::new());
            }
            Err(error) => return Err(error),
        };

        let mut records = Vec::new();
        let mut position = 0;

        while position < bytes.len() {
            let rest = &bytes[position..];

            if rest.starts_with(b"*") {
                match parse_resp_frame(rest) {
                    RespParseResult::Complete(record, consumed) => {
                        records.push(record);
                        position += consumed;
                    }

                    RespParseResult::Incomplete => {
                        eprintln!(
                            "AOF ends with an incomplete record; truncating it at byte {position}"
                        );

                        let file = OpenOptions::new().write(true).open(&self.path)?;
                        file.set_len(position as u64)?;
                        file.sync_all()?;

                        break;
                    }

                    RespParseResult::Error(error) => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            format!("corrupted AOF at byte {position}: {error}"),
                        ));
                    }
                }
            } else {
                let line_len = rest
                    .iter()
                    .position(|&byte| byte == b'\n')
                    .map_or(rest.len(), |index| index + 1);

                let line = String::from_utf8_lossy(&rest[..line_len]);
                let line = line.trim();

                if !line.is_empty() {
                    match tokenize(line) {
                        Ok(parts) => records.push(parts),
                        Err(error) => eprintln!("Skipping invalid AOF line '{line}': {error}"),
                    }
                }

                position += line_len;
            }
        }

        Ok(records)
    }

    /// Starts a rewrite: from now on, appended records are also kept aside
    /// for `complete_rewrite`.
    ///
    /// Call it while holding the store's write lock, right before taking the
    /// snapshot (`Store::entries`): the snapshot then covers exactly the
    /// records appended before this call, and every later one is kept aside.
    pub fn start_rewrite(&self) -> io::Result<()> {
        let mut inner = self.inner.lock().unwrap();

        if inner.rewrite.is_some() {
            return Err(io::Error::other("an AOF rewrite is already in progress"));
        }

        inner.rewrite = Some(RewriteState::default());

        Ok(())
    }

    /// Replaces the log with `entries` plus whatever was appended since
    /// `start_rewrite`.
    ///
    /// Blocking: run it off the async workers (`spawn_blocking`) and without
    /// the store lock, so the server keeps serving while the snapshot is
    /// written. Only the final step (pending records, fsync, rename) holds the
    /// AOF lock, which briefly stalls appends.
    pub fn complete_rewrite(&self, entries: Vec<SnapshotEntry>) -> io::Result<()> {
        let temp_path = format!("{}.tmp", self.path);

        let snapshot = write_snapshot(&temp_path, entries);

        let mut inner = self.inner.lock().unwrap();

        let rewrite = inner
            .rewrite
            .take()
            .ok_or_else(|| io::Error::other("no AOF rewrite in progress"))?;

        let result = snapshot.and_then(|mut file| {
            if rewrite.invalidated {
                return Err(io::Error::other("AOF rewrite aborted by FLUSHALL"));
            }

            for record in &rewrite.pending {
                file.write_all(encode_array(record).as_bytes())?;
            }

            file.into_inner()?.sync_all()?;

            fs::rename(&temp_path, &self.path)
        });

        if let Err(error) = result {
            let _ = fs::remove_file(&temp_path);
            return Err(error);
        }

        // The previous file was replaced, so point the live writer at the new
        // one. Its unflushed buffer is redundant: those records are either in
        // the snapshot or in `pending`.
        discard(std::mem::replace(
            &mut inner.writer,
            open_appender(&self.path)?,
        ));

        Ok(())
    }

    /// Empties the log (FLUSHALL).
    ///
    /// Buffered records are discarded, not flushed: they describe writes that
    /// FLUSHALL is wiping out. Dropping the old `BufWriter` would flush them
    /// into the freshly truncated file and bring that data back on restart.
    ///
    /// A rewrite in progress is aborted: its snapshot still holds the wiped data.
    pub fn clear(&self) -> io::Result<()> {
        let mut inner = self.inner.lock().unwrap();

        // The clone shares the open file description, so it keeps append mode:
        // after `set_len(0)` the next record is written at offset 0.
        let file = inner.writer.get_ref().try_clone()?;
        file.set_len(0)?;

        discard(std::mem::replace(&mut inner.writer, BufWriter::new(file)));

        if let Some(rewrite) = &mut inner.rewrite {
            rewrite.pending.clear();
            rewrite.invalidated = true;
        }

        Ok(())
    }
}

/// Writes `entries` as `SET` (+ `EXPIREAT`) records to a fresh file at `path`.
///
/// Returns the writer unflushed, so the caller can append more before syncing.
fn write_snapshot(path: &str, entries: Vec<SnapshotEntry>) -> io::Result<BufWriter<File>> {
    let mut file = BufWriter::new(
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?,
    );

    for (key, value, expires_at) in entries {
        file.write_all(encode_array(&["SET", &key, &value]).as_bytes())?;

        if let Some(expires_at) = expires_at {
            let expires_at = expires_at.to_string();
            file.write_all(encode_array(&["EXPIREAT", &key, &expires_at]).as_bytes())?;
        }
    }

    Ok(file)
}

/// Drops a writer without flushing its buffer to the file.
fn discard(writer: BufWriter<File>) {
    let _ = writer.into_parts();
}

fn open_appender(path: &str) -> io::Result<BufWriter<File>> {
    let file = OpenOptions::new().create(true).append(true).open(path)?;

    Ok(BufWriter::new(file))
}
