use lazy_static::lazy_static;
use regex::Regex;
use url::{self, ParseError};

const WIKIPEDIA_HOST: &str = "wikipedia.org";

#[derive(Eq, PartialEq, Hash, Clone)]
pub struct Url {
    val: url::Url,
}

#[derive(Debug, PartialEq)]
pub enum Type {
    Article,
    File,
    ExternalArticle,
    Other,
}

impl Url {
    pub fn new(val: &str) -> Result<Self, ParseError> {
        let val = url::Url::parse(val)?;

        Ok(Self { val })
    }

    pub fn is_wiki(&self) -> bool {
        self.val.host_str().unwrap().contains(WIKIPEDIA_HOST)
    }

    pub fn val(&self) -> &str {
        self.val.as_str()
    }

    pub fn val_for_type(&self) -> String {
        match self.url_type() {
            Type::Article | Type::File => String::from(
                urlencoding::decode(self.val.path())
                    .unwrap()
                    .split('/')
                    .last()
                    .unwrap(),
            ),
            Type::ExternalArticle | Type::Other => self.val().to_string(),
        }
    }

    pub fn url_type(&self) -> Type {
        if self.is_wiki_article() {
            Type::Article
        } else if self.is_file() {
            Type::File
        } else if self.is_external_article() {
            Type::ExternalArticle
        } else {
            Type::Other
        }
    }

    fn is_wiki_article(&self) -> bool {
        lazy_static! {
            static ref RE: Regex =
                Regex::new(r"https://[a-z]{2}\.wikipedia\.org/wiki/([^/.]+)$").unwrap();
        }

        if RE.is_match(self.val()) {
            return !self.val.path().contains(':');
        };

        false
    }

    fn is_file(&self) -> bool {
        let s = self.val.to_string();
        s.ends_with(".png")
            || s.ends_with(".jpg")
            || s.ends_with(".jpeg")
            || s.ends_with(".gif")
            || s.ends_with(".svg")
    }

    fn is_external_article(&self) -> bool {
        let s = self.val.to_string().to_lowercase();
        s.contains("arxiv.org") || s.contains("doi.org")
    }
}
