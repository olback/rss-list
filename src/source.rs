use std::io::Write;

use {
    crate::error::{Error, Result},
    rayon::prelude::*,
    std::{fs, path::PathBuf},
};

#[derive(Debug)]
pub struct Feed {
    pub title: String,
    pub source: String,
    pub description: Option<String>,
    pub url: Option<String>,
    pub icon: Option<PathBuf>,
    pub posts: Vec<Post>,
}

#[derive(Debug)]
pub struct Post {
    pub title: String,
    pub summary: Option<String>,
    pub url: String,
    pub publisher: String,
    pub published: chrono::DateTime<chrono::Utc>,
}

fn get_sources_file_path() -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .map(|d| d.join(env!("CARGO_PKG_NAME")))
        .ok_or(Error::NoConfigDir)?;

    if !dir.is_dir() {
        fs::create_dir_all(&dir)?;
    }

    let file = dir.join("sources.txt");
    if !file.is_file() {
        fs::write(&file, "")?;
    }

    Ok(file)
}

pub fn add_source(source: &str) -> Result<()> {
    fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(get_sources_file_path()?)?
        .write_all(format!("{}\n", source).as_bytes())?;
    Ok(())
}

pub fn write_sources(sources: &[&str]) -> Result<()> {
    let data = sources.join("\n") + "\n";
    fs::write(get_sources_file_path()?, &data)?;
    Ok(())
}

pub fn get_sources() -> Result<Vec<String>> {
    Ok(fs::read_to_string(get_sources_file_path()?)?
        .split('\n')
        .filter_map(|line| {
            if line.trim().is_empty() {
                None
            } else {
                Some(String::from(line))
            }
        })
        .collect::<Vec<String>>())
}

pub fn download(sources: &[&str]) -> (Vec<Feed>, Vec<Error>) {
    sources
        .into_par_iter()
        .map(|site| {
            reqwest::blocking::get(*site)
                .map_err(Error::from)
                .and_then(|r| r.bytes().map_err(Error::from))
                .and_then(|b| feed_rs::parser::parse(&b[..]).map_err(Error::from))
                .and_then(|feed| {
                    // println!("{:#?}", feed);
                    let title = feed.title.map(|t| t.content).ok_or(Error::MissingData {
                        site: String::from(*site),
                        field: "title",
                    })?;
                    Ok(Feed {
                        title: title.clone(),
                        source: String::from(*site),
                        description: feed.description.map(|t| t.content),
                        url: feed.links.get(0).map(|l| l.href.clone()),
                        icon: feed
                            .icon
                            .and_then(|i| reqwest::blocking::get(i.uri).ok())
                            .and_then(|r| r.bytes().ok())
                            .and_then(|b| dirs::cache_dir().map(|c| (b, c)))
                            .and_then(|(bytes, dir)| {
                                let cache_dir = dir.join(env!("CARGO_PKG_NAME"));
                                let file = cache_dir.join(hex::encode(&title));
                                if !cache_dir.exists() {
                                    fs::create_dir_all(&cache_dir).ok()?;
                                }
                                match fs::write(&file, bytes.to_vec()) {
                                    Ok(_) => Some(file),
                                    Err(_) => None,
                                }
                            }),
                        posts: feed
                            .entries
                            .into_iter()
                            .filter_map(move |entry| {
                                Some(Post {
                                    title: entry.title.map(|t| t.content)?,
                                    summary: entry.summary.map(|t| t.content.trim().to_string()),
                                    url: entry.links.get(0).map(|l| l.href.clone())?,
                                    publisher: title.clone(),
                                    published: entry.published.or(entry.updated)?,
                                })
                            })
                            .collect(),
                    })
                })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .fold(
            (Vec::<Feed>::new(), Vec::<Error>::new()),
            |(mut ok, mut err), s| match s {
                Ok(f) => {
                    ok.push(f);
                    (ok, err)
                }
                Err(e) => {
                    err.push(e);
                    (ok, err)
                }
            },
        )
}
