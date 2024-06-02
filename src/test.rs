use crate::{author::get_author_list, config::Config};

#[tokio::test]
async fn test_main() {
    let config = Config::parse();


    let authors = get_author_list(&config).await;
    assert_eq!(authors.is_ok(), true);
}