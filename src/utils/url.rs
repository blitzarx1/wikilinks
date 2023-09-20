pub fn open_url(url: &str) {
    open::that(url).unwrap();
}