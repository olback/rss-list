use std::io::Write;

use {
    crate::error::{Error, Result},
    rayon::prelude::*,
    std::{fs, path::PathBuf},
};

#[derive(Debug)]
pub struct Feed {
    pub title: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub posts: Vec<Post>,
}

#[derive(Debug)]
pub struct Post {
    pub title: String,
    pub url: String,
    pub publisher: String,
    pub published: chrono::DateTime<chrono::Utc>,
}

fn get_sources_file_path() -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .map(|d| d.join("rss-list"))
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
                    let title = feed.title.map(|t| t.content).ok_or(Error::MissingData {
                        site: String::from(*site),
                        field: "title",
                    })?;
                    Ok(Feed {
                        title: title.clone(),
                        description: feed.description.map(|t| t.content),
                        icon: feed.icon.map(|i| i.uri),
                        posts: feed
                            .entries
                            .into_iter()
                            .filter_map(move |entry| {
                                Some(Post {
                                    title: entry.title.map(|t| t.content)?,
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
