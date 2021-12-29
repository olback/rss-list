use giftwrap::Wrap;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Wrap)]
pub enum Error {
    Glib(gtk::glib::Error),
    Bool(gtk::glib::BoolError),
    Reqwest(reqwest::Error),
    Feed(feed_rs::parser::ParseFeedError),
    Io(std::io::Error),
    #[noWrap]
    MissingData {
        site: String,
        field: &'static str,
    },
    #[noWrap]
    NoConfigDir,
}
