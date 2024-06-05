use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fs::File,
    io::{BufReader, Write},
    path::PathBuf,
};

use chrono::{DateTime, Local};
use indicatif::{ProgressBar, ProgressStyle};
use log::{info, log_enabled};
use post_archiver::{
    ArchiveAuthor, ArchiveAuthorsList, ArchiveFrom, ArchiveFile, ArchivePost, ArchivePostShort,
};
use tokio::{
    fs,
    task::JoinSet,
    time::{sleep, Duration},
};
use url::Url;

use crate::{
    api::{ArchiveClient, FanboxClient},
    author::Author,
    config::Config,
    post::Post,
    unit_short,
};

pub fn resolve(
    authors: Vec<Author>,
    posts: Vec<Post>,
) -> (Vec<ArchiveAuthor>, Vec<ArchivePost>, Vec<(Url, PathBuf)>) {
    let mut download_files: Vec<(Url, PathBuf)> = Vec::new();
    let mut map_author: HashMap<
        String,
        ((DateTime<Local>, Option<PathBuf>), Vec<ArchivePostShort>),
    > = HashMap::new();

    let archive_posts = unit_short!("Resolving Posts", {
        let mut archive_posts = Vec::new();
        for post in posts {
            let body = post.body();
            let out_path = PathBuf::from(post.author()).join(post.id());

            let mut files = body
                .files()
                .into_iter()
                .map(|file| {
                    let file_path = out_path.join(file.filename());
                    download_files.push((file.url(), file_path.clone()));
                    ArchiveFile::File {
                        path: file_path,
                        filename: file.filename().into(),
                    }
                })
                .collect::<Vec<ArchiveFile>>();

            let mut thumb = None;
            let images = body
                .images()
                .into_iter()
                .map(|image| {
                    let file_path = out_path.join(image.filename());
                    download_files.push((image.url(), file_path.clone()));
                    let image = ArchiveFile::Image {
                        width: image.width,
                        height: image.height,
                        path: file_path,
                        filename: image.filename().into(),
                    };
                    thumb = thumb.clone().or(Some(image.path().clone()));
                    image
                })
                .collect::<Vec<ArchiveFile>>();
            files.extend(images);

            let id = post.id();
            let title = post.title();
            let author = post.author();
            let updated = post.updated();
            let published = post.published();
            let comments = post.comments().into_iter().map(|c| c.into()).collect();

            let content = body.content(out_path);

            let ((thumb_published, author_thumb), author_post_list) =
                map_author.entry(author.clone()).or_default();
            if published > *thumb_published && thumb.is_some() {
                *thumb_published = published.clone();
                *author_thumb = thumb.clone();
            }

            let post = ArchivePost {
                id,
                title,
                files,
                thumb,
                author,
                content,
                updated,
                comments,
                published,
                from: ArchiveFrom::Fanbox,
            };

            author_post_list.push(post.clone().into());

            archive_posts.push(post);
        }
        archive_posts
    });

    let archive_authors = unit_short!("Resolving Authors", {
        let mut archive_authors = Vec::new();

        for author in authors {
            let id = author.id().to_string();
            let name = author.name();

            let ((_, thumb), mut posts) = map_author
                .get(&id)
                .unwrap_or(&(Default::default(), vec![]))
                .clone();
            posts.sort_by(|a, b| b.updated.cmp(&a.updated));
            posts.reverse();

            archive_authors.push(ArchiveAuthor {
                id,
                name,
                posts,
                thumb,
                from: HashSet::from([ArchiveFrom::Fanbox]),
            });
        }

        archive_authors
    });

    (archive_authors, archive_posts, download_files)
}

pub async fn build(
    authors: Vec<ArchiveAuthor>,
    posts: Vec<ArchivePost>,
    files: Vec<(Url, PathBuf)>,
    config: &Config,
) -> Result<(), Box<dyn Error>> {
    let output = config.output();
    fs::create_dir_all(&output).await?;

    unit_short!("Write Data", {
        {
            let mut archive_authors = ArchiveAuthorsList::from_vector(authors.clone());
            let path = output.join("authors.json");
            if path.exists() {
                let file = File::open(&path)?;
                let reader = BufReader::new(file);
                let mut old_authors: ArchiveAuthorsList = serde_json::from_reader(reader)?;
                old_authors.extend(archive_authors);
                archive_authors = old_authors;
            }

            info!("Writing authors.json");
            let mut file = File::create(&path).unwrap();
            file.write_all(serde_json::to_vec(&archive_authors)?.as_slice())
                .unwrap();
        }

        info!("Writing `/[author]/author.json` (total: {})", authors.len());
        for mut author in authors.into_iter() {
            let output = output.join(&author.id);
            if !output.exists() {
                fs::create_dir(&output).await?;
            }
            let path = output.join("author.json");
            if path.exists() {
                let file = File::open(&path)?;
                let reader = BufReader::new(file);
                let mut old_author: ArchiveAuthor = serde_json::from_reader(reader)?;
                old_author.extend(author);
                author = old_author;
            }
            let mut file = File::create(&path).unwrap();
            file.write_all(serde_json::to_vec(&author)?.as_slice())
                .unwrap();
        }

        info!(
            "Writing `/[author]/[post]/post.json` (total: {})",
            posts.len()
        );
        for post in posts.into_iter() {
            let output = output.join(&post.author).join(&post.id);
            if !output.exists() {
                fs::create_dir(&output).await?;
            }
            let path = output.join("post.json");
            let mut file = File::create(&path).unwrap();
            file.write_all(serde_json::to_vec(&post)?.as_slice())
                .unwrap();
        }
    });

    unit_short!("Download Files", {
        let mut await_files = JoinSet::new();
        let client = FanboxClient::new(config.clone());

        let mut i = 0;
        for (url, path) in files {
            let file_path = output.join(&path);
            if !file_path.exists() {
                i += 1;
                let client = client.clone();
                await_files.spawn(async move { client.download(url, file_path).await });
            }
        }
        let pg = if log_enabled!(log::Level::Info) {
            info!("Downloading {} files", i);
            Some(
                config.multi_progress.add(ProgressBar::new(i)).with_style(
                    ProgressStyle::with_template(
                        "[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len}",
                    )
                    .unwrap(),
                ),
            )
        } else {
            None
        };

        if let Some(pg) = &pg {
            let pg = pg.clone();
            await_files.spawn(async move {
                loop {
                    if pg.length().unwrap() == i {
                        return ();
                    }
                    sleep(Duration::from_secs(1)).await;
                    pg.tick();
                }
            });
        }

        while await_files.join_next().await.is_some() {
            if let Some(pg) = &pg {
                pg.inc(1);
            }
        }

        if let Some(pg) = &pg {
            pg.finish_with_message("All downloaded")
        }
    });

    Ok(())
}
