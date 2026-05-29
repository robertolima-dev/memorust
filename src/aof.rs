use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};

pub struct Aof {
    path: String,
}

impl Aof {
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
        }
    }

    pub fn append(&self, command: &str) -> io::Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;

        writeln!(file, "{}", command)?;

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
                writeln!(file, "SET {} {}", key, value)?;
            }

            file.sync_all()?;
        }

        fs::rename(temp_path, &self.path)?;

        Ok(())
    }

    pub fn clear(&self) -> io::Result<()> {
        OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&self.path)?;

        Ok(())
    }
}
