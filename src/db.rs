use std::{
    error::Error,
    path::{Path, PathBuf},
    time::Instant,
    vec,
};

use sqlite::{Connection, State, Value};

use crate::{APPLICATION, ORGANIZATION, QUALIFIER};

pub const IN_CHUNKS: &usize = &500;

pub struct Db {}

impl Db {
    pub fn insert_files_metadata(data: Vec<(String, String)>) -> Result<(), Box<dyn Error>> {
        let conn = Self::get_sqlite_conn()?;
        let now = Instant::now();
        conn.execute("begin transaction;")?;

        let mut q = String::from("insert into file (path, metadata) values ");

        q.push_str(
            &data
                .iter()
                .map(|(path, data)| format!("('{}', '{}')", path, data.replace('\'', "")))
                .collect::<Vec<String>>()
                .join(","),
        );

        conn.execute(q)?;
        conn.execute("commit transaction;")?;
        println!("Spent {}ms inserting into db", now.elapsed().as_millis());

        Ok(())
    }

    pub fn get_cached_images_by_paths(paths: &[String]) -> Result<Vec<String>, Box<dyn Error>> {
        let mut existing_files: Vec<String> = vec![];
        let conn = Self::get_sqlite_conn()?;

        //safeguard lest we go over the limit. Although unlikely since metadata processing is done in chunks too.
        let chunks: Vec<&[String]> = paths.chunks(*IN_CHUNKS).collect();
        for chunk in chunks {
            let mut q = conn.prepare(format!(
                "SELECT path FROM file where path in ({})",
                Utilities::arr_param_from(chunk)
            ))?;

            while let Ok(State::Row) = q.next() {
                existing_files.push(q.read::<String, _>("path")?)
            }
        }

        Ok(existing_files)
    }

    pub fn get_image_metadata(path: &str) -> Result<Option<String>, Box<dyn Error>> {
        let conn = Self::get_sqlite_conn()?;
        let mut q = conn.prepare("select metadata from file where path = :path")?;
        q.bind::<&[(_, Value)]>(&[(":path", path.to_owned().into())][..])?;

        if q.next().is_ok() {
            return Ok(Some(q.read::<String, _>("metadata")?));
        }

        Ok(None)
    }

    pub fn init_db() -> Result<(), Box<dyn Error>> {
        let conn = Self::get_sqlite_conn()?;

        let q = "
            create table if not exists file (
                path text not null primary key,
                metadata text not null,
                ts TIMESTAMP not null DEFAULT CURRENT_TIMESTAMP); 
            create index if not exists file_ts_IDX  on file (ts DESC);
        ";

        conn.execute(q)?;
        Ok(())
    }

    pub fn trim_db(limit: &u32) -> Result<(), Box<dyn Error>> {
        let conn = Self::get_sqlite_conn()?;

        let q = format!(
            "delete from file where path not in (select path from file order by ts desc limit {})",
            limit
        );

        conn.execute(q)?;
        Ok(())
    }

    pub fn get_sqlite_conn() -> Result<Connection, Box<dyn Error>> {
        //Maybe inefficient to compute this path every time?
        let path = Self::get_db_path()?;

        let conn = Connection::open(path)?;
        Ok(conn)
    }

    pub fn get_db_path() -> Result<PathBuf, Box<dyn Error>> {
        match directories::ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION) {
            Some(dirs) => {
                let cache_dir = dirs.cache_dir().to_owned();

                if !Path::new(&cache_dir).exists() {
                    std::fs::create_dir(&cache_dir)?
                }

                Ok(cache_dir.join(PathBuf::from("db.db")))
            }
            None => Err("Failure getting db path")?,
        }
    }
}

pub struct Utilities {}

impl Utilities {
    pub fn arr_param_from(strings: &[String]) -> String {
        format!("\"{}\"", &strings.join("\", \""))
    }
}
