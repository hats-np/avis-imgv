use core::fmt;
use std::{
    error::Error,
    path::{Path, PathBuf},
    time::Instant,
    vec,
};

use rusqlite::{Connection, Result};

use crate::{APPLICATION, ORGANIZATION, QUALIFIER};

pub const IN_CHUNKS: &usize = &500;

pub struct Db {}

impl Db {
    pub fn insert_files_metadata(data: Vec<(String, String)>) -> Result<(), Box<dyn Error>> {
        let conn = Self::get_sqlite_conn()?;
        let now = Instant::now();
        conn.execute("begin transaction;", ())?;

        let mut q = String::from("insert into file (path, metadata) values ");

        q.push_str(
            &data
                .iter()
                .map(|(path, data)| format!("('{}', jsonb('{}'))", path, data.replace('\'', "")))
                .collect::<Vec<String>>()
                .join(","),
        );

        conn.execute(&q, ())?;
        conn.execute("commit transaction;", ())?;
        println!(
            "Spent {}ms inserting {} metadata records into db",
            now.elapsed().as_millis(),
            data.len()
        );

        Ok(())
    }

    pub fn get_cached_images_by_paths(paths: &[String]) -> Result<Vec<String>, Box<dyn Error>> {
        let mut existing_files: Vec<String> = vec![];
        let conn = Self::get_sqlite_conn()?;

        //safeguard lest we go over the limit. Although unlikely since metadata processing is done in chunks too.
        let chunks: Vec<&[String]> = paths.chunks(*IN_CHUNKS).collect();
        for chunk in chunks {
            let mut q = conn.prepare(&format!(
                "SELECT path FROM file where path in ({})",
                DbUtilities::arr_param_from(chunk)
            ))?;

            let mut paths = q
                .query_map([], |row| row.get::<_, String>(0))?
                .filter_map(|x| x.ok())
                .collect::<Vec<String>>();

            existing_files.append(&mut paths);
        }

        Ok(existing_files)
    }

    pub fn get_image_metadata(path: &str) -> Result<Option<String>, Box<dyn Error>> {
        let conn = Self::get_sqlite_conn()?;
        let mut q = conn.prepare("select json(metadata) from file where path = ?1")?;
        match q.query_row([path], |row| {
            let value: String = row.get(0)?;
            Ok(value)
        }) {
            Ok(metadata) => Ok(Some(metadata)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn init_db() -> Result<(), Box<dyn Error>> {
        let conn = Self::get_sqlite_conn()?;

        let stms = vec![
            "create table if not exists file (
                path text not null primary key,
                metadata jsonb not null,
                ts TIMESTAMP not null DEFAULT CURRENT_TIMESTAMP);",
            "create index if not exists file_ts_IDX on file (ts DESC)",
        ];

        for stm in stms {
            conn.execute(stm, ())?;
        }

        conn.pragma_update(None, "journal_mode", "WAL")?;

        Ok(())
    }

    pub fn trim_db(limit: &u32) -> Result<(), Box<dyn Error>> {
        println!("Trimming database, leaving {limit} records");
        let conn = Self::get_sqlite_conn()?;

        let q = format!(
            "delete from file where path not in (select path from file order by ts desc limit {limit})"
        );

        conn.execute(&q, ())?;
        println!("Finished trimming database");
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

    pub fn get_paths_filtered_by_metadata(
        exif_tags: &[(String, String, SqlOperator)],
        order_tag: &str,
        order_direction: &SqlOrder,
    ) -> Result<Vec<PathBuf>, Box<dyn Error>> {
        let mut query = String::from("SELECT distinct(path) FROM file WHERE ");

        query += &exif_tags
            .iter()
            .filter(|x| !x.1.is_empty())
            .map(|x| {
                format!(
                    "json_extract(metadata,'$.{}') {}",
                    x.0,
                    DbUtilities::where_clause_from_str_and_operator(&x.1, &x.2)
                )
            })
            .collect::<Vec<String>>()
            .join(" AND ");

        if !order_tag.is_empty() {
            query += &format!(
                " ORDER BY  json_extract(metadata,'$.{}') {}",
                order_tag,
                order_direction.get_sql()
            );
        }

        println!("Query: {query}");

        let conn = Self::get_sqlite_conn()?;
        let mut q = conn.prepare(&query)?;
        let paths = q
            .query_map([], |row| row.get::<_, String>(0))?
            .filter_map(|x| x.ok())
            .map(PathBuf::from)
            .collect();

        Ok(paths)
    }

    pub fn get_distinct_values_for_exif_tag(exif_tag: &str) -> Result<Vec<String>, Box<dyn Error>> {
        let query = format!(
            "select distinct(json_extract(metadata,'$.{exif_tag}')) as dist from file where dist is not null"
        );

        let conn = Self::get_sqlite_conn()?;
        let mut q = conn.prepare(&query)?;

        let distinct_values = q
            .query_map([], |row| row.get::<_, String>(0))?
            .filter_map(|x| x.ok())
            .collect();

        Ok(distinct_values)
    }

    pub fn get_unique_exif_tags() -> Result<Vec<String>, Box<dyn Error>> {
        let query = "SELECT DISTINCT key FROM file, json_each(metadata) ORDER BY key ASC";

        let conn = Self::get_sqlite_conn()?;
        let mut q = conn.prepare(query)?;

        let unique_tags = q
            .query_map([], |row| row.get::<_, String>(0))?
            .filter_map(|x| x.ok())
            .collect();

        Ok(unique_tags)
    }

    pub fn get_img_count() -> Result<u32, Box<dyn Error>> {
        let conn = Self::get_sqlite_conn()?;
        let mut q = conn.prepare("select count(-1) as count from file")?;

        Ok(q.query_one([], |row| {
            let count: u32 = row.get(0)?;
            Ok(count)
        })?)
    }

    pub fn delete_file_by_path(path: &Path) -> Result<(), Box<dyn Error>> {
        let conn = Self::get_sqlite_conn()?;
        conn.execute("delete from file where path = ?1", [path.to_str()])?;
        Ok(())
    }

    pub fn delete_files_by_paths<T: AsRef<Path>>(paths: &[T]) -> Result<(), Box<dyn Error>> {
        let conn = Self::get_sqlite_conn()?;

        let string_vec: Vec<String> = paths
            .iter()
            .filter_map(|p| p.as_ref().to_str())
            .map(String::from)
            .collect();

        conn.execute(
            &format!(
                "delete from file where path in ({})",
                DbUtilities::arr_param_from(&string_vec)
            ),
            (),
        )?;
        Ok(())
    }

    pub fn get_all_file_paths() -> Result<Vec<String>, Box<dyn Error>> {
        let query = "SELECT path FROM file";

        let conn = Self::get_sqlite_conn()?;
        let mut q = conn.prepare(query)?;

        let paths = q
            .query_map([], |row| row.get::<_, String>(0))?
            .filter_map(|x| x.ok())
            .collect();

        Ok(paths)
    }
}

pub struct DbUtilities {}

impl DbUtilities {
    pub fn arr_param_from(strings: &[String]) -> String {
        format!("\"{}\"", &strings.join("\", \""))
    }

    pub fn where_clause_from_str_and_operator(val: &str, operator: &SqlOperator) -> String {
        let is_numeric = val.parse::<f64>().is_ok();

        match operator {
            SqlOperator::Eq => {
                if is_numeric {
                    format!("+0 = {val}")
                } else {
                    format!("= '{val}'")
                }
            }
            SqlOperator::Like => format!("like '%{val}%'"),
            SqlOperator::SmallerThan => {
                if is_numeric {
                    format!("+0 < {val}")
                } else {
                    format!("< '{val}'")
                }
            }
            SqlOperator::BiggerThan => {
                if is_numeric {
                    format!("+0 > {val}")
                } else {
                    format!("> '{val}'")
                }
            }
            SqlOperator::EqSmallerThan => {
                if is_numeric {
                    format!("+0 <= {val}")
                } else {
                    format!("<= '{val}'")
                }
            }
            SqlOperator::EqBiggerThan => {
                if is_numeric {
                    format!("+0 >= {val}")
                } else {
                    format!(">= '{val}'")
                }
            }
            SqlOperator::Different => {
                if is_numeric {
                    format!("+0 <> {val}")
                } else {
                    format!("<> '{val}'")
                }
            }
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum SqlOperator {
    Like,
    Eq,
    BiggerThan,
    SmallerThan,
    EqBiggerThan,
    EqSmallerThan,
    Different,
}
impl SqlOperator {
    pub fn list() -> Vec<SqlOperator> {
        vec![
            SqlOperator::Like,
            SqlOperator::Eq,
            SqlOperator::BiggerThan,
            SqlOperator::SmallerThan,
            SqlOperator::EqBiggerThan,
            SqlOperator::EqSmallerThan,
            SqlOperator::Different,
        ]
    }
}

impl fmt::Display for SqlOperator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SqlOperator::Like => write!(f, "In"),
            SqlOperator::Eq => write!(f, "="),
            SqlOperator::BiggerThan => write!(f, ">"),
            SqlOperator::SmallerThan => write!(f, "<"),
            SqlOperator::EqBiggerThan => write!(f, ">="),
            SqlOperator::EqSmallerThan => write!(f, "<="),
            SqlOperator::Different => write!(f, "<>"),
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum SqlOrder {
    Asc,
    Desc,
}

impl SqlOrder {
    pub fn list() -> Vec<SqlOrder> {
        vec![SqlOrder::Asc, SqlOrder::Desc]
    }

    pub fn get_sql(&self) -> String {
        match self {
            SqlOrder::Asc => "ASC",
            SqlOrder::Desc => "DESC",
        }
        .to_string()
    }
}

impl fmt::Display for SqlOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SqlOrder::Asc => write!(f, "Ascending"),
            SqlOrder::Desc => write!(f, "Descending"),
        }
    }
}
