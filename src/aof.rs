use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::sync::Mutex;

pub struct Aof {
    path: String,
    writer: Mutex<BufWriter<File>>,
}

impl Aof {
    pub fn new(path: &str) -> io::Result<Self> {
        Ok(Self {
            path: path.to_string(),
            writer: Mutex::new(open_appender(path)?),
        })
    }

    pub fn append(&self, command: &str) -> io::Result<()> {
        let mut writer = self.writer.lock().unwrap();

        // Only buffer here; durability is handled out-of-band by `flush`, called
        // ~once per second by the AOF flusher task. This keeps the fsync off the
        // per-command hot path (Redis' `appendfsync everysec` behaviour).
        writeln!(writer, "{command}")?;

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
            let mut writer = self.writer.lock().unwrap();

            writer.flush()?;
            writer.get_ref().try_clone()?
        };

        handle.sync_all()?;

        Ok(())
    }

    pub fn load(&self) -> io::Result<Vec<String>> {
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(Vec::new());
            }
            Err(error) => return Err(error),
        };

        let reader = BufReader::new(file);

        reader.lines().collect()
    }

    pub fn rewrite(&self, entries: Vec<(String, String)>) -> io::Result<()> {
        let temp_path = format!("{}.tmp", self.path);

        {
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&temp_path)?;

            for (key, value) in entries {
                writeln!(file, "SET {key} {value}")?;
            }

            file.sync_all()?;
        }

        fs::rename(&temp_path, &self.path)?;

        // The previous file was replaced, so point the live writer at the new one.
        let mut writer = self.writer.lock().unwrap();
        *writer = open_appender(&self.path)?;

        Ok(())
    }

    pub fn clear(&self) -> io::Result<()> {
        let mut writer = self.writer.lock().unwrap();
        *writer = open_truncated(&self.path)?;

        Ok(())
    }
}

fn open_appender(path: &str) -> io::Result<BufWriter<File>> {
    let file = OpenOptions::new().create(true).append(true).open(path)?;

    Ok(BufWriter::new(file))
}

fn open_truncated(path: &str) -> io::Result<BufWriter<File>> {
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;

    Ok(BufWriter::new(file))
}
