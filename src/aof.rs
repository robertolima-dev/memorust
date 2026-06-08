use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use tokio::fs::OpenOptions as TokioOpenOptions;
use tokio::io::{AsyncWriteExt, BufWriter as TokioBufWriter};
use tokio::sync::mpsc;

pub struct Aof {
    path: String,
    sender: mpsc::Sender<String>,
}

impl Aof {
    pub async fn new(path: &str) -> io::Result<Self> {
        let (sender, mut receiver) = mpsc::channel::<String>(10_000);
        let writer = open_async_appender(path).await?;

        tokio::spawn(async move {
            let mut writer = writer;
            let mut flush_interval = tokio::time::interval(std::time::Duration::from_secs(1));

            loop {
                tokio::select! {
                    Some(command) = receiver.recv() => {
                        if let Err(error) = writer.write_all(command.as_bytes()).await {
                            eprintln!("AOF write error: {error}");
                        }

                        if let Err(error) = writer.write_all(b"\n").await {
                            eprintln!("AOF newline write error: {error}");
                        }
                    },

                    _ = flush_interval.tick() => {
                        if let Err(error) = writer.flush().await {
                            eprintln!("AOF flush error: {error}");
                        }
                    },

                    else => {
                        let _ = writer.flush().await;
                        break;
                    }
                }
            }
        });

        Ok(Self {
            path: path.to_string(),
            sender,
        })
    }

    pub async fn append(&self, command: &str) -> io::Result<()> {
        self.sender
            .send(command.to_string())
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "AOF writer task stopped"))?;

        Ok(())
    }

    pub fn flush(&self) -> io::Result<()> {
        // let handle = {
        //     let mut writer = self.writer.lock().unwrap();

        //     writer.flush()?;
        //     writer.get_ref().try_clone()?
        // };

        // handle.sync_all()?;

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
        // let mut writer = self.writer.lock().unwrap();
        // *writer = open_appender(&self.path)?;

        Ok(())
    }

    pub fn clear(&self) -> io::Result<()> {
        // let mut writer = self.writer.lock().unwrap();
        // *writer = open_truncated(&self.path)?;
        open_truncated(&self.path)?;

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

async fn open_async_appender(path: &str) -> io::Result<TokioBufWriter<tokio::fs::File>> {
    let file = TokioOpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?;

    Ok(TokioBufWriter::new(file))
}
