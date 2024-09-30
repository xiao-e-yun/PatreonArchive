use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fs::File,
    io::{BufReader, Write},
    path::PathBuf,
};

use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, info, log_enabled};
use post_archiver::{
    ArchiveAuthor, ArchiveAuthorsList, ArchiveFile, ArchiveFrom, ArchivePost, ArchivePostShort,
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
    post::{Post, PostBody},
    unit_short,
};

pub fn resolve(
    authors: Vec<Author>,
    posts: Vec<Post>,
    config: &Config,
) -> Result<(Vec<ArchiveAuthor>, Vec<ArchivePost>, Vec<(Url, PathBuf)>), Box<dyn Error>> {
    let mut download_files: Vec<(Url, PathBuf)> = Vec::new();
    let mut map_author: HashMap<String, Vec<ArchivePostShort>> = HashMap::new();

    let output = config.output();
    let archive_posts = unit_short!("Resolving Posts", {
        let mut archive_posts = Vec::new();
        for post in posts {
            let body = post.body();
            let out_path = PathBuf::from(post.author()).join(post.id());

            debug!("Resolving Post: {}", post.id());

            debug!("Resolving Files");
            let mut files = body
                .files()
                .into_iter()
                .map(|file| {
                    let file_path = out_path
                        .join(file.filename());
                    download_files.push((file.url(), file_path.clone()));
                    PostBody::parse_video_or_file(file, file_path)
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

            debug!("Resolving Meta Data");
            let id = post.id();
            let title = post.title();
            let author = post.author();
            let updated = post.updated();
            let published = post.published();
            let comments = vec![]; //TODO
            // let comments = post.comments().into_iter().map(|c| c.into()).collect();

            let content = body.content(out_path);

            // Add post to author list
            let author_post_list = map_author.entry(author.clone()).or_default();

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
            debug!("Resolving Author: {}", author.id());
            let id = author.id().to_string();
            let name = author.name();

            debug!("Resolving Author Posts");
            let mut posts = map_author.get(&id).unwrap_or(&vec![]).clone();
            posts.sort_by(|a, b| b.updated.cmp(&a.updated));
            debug!("Posts: {}", posts.len());

            let mut updated_author = ArchiveAuthor {
                id,
                name,
                posts,
                thumb: None,
                from: HashSet::from([ArchiveFrom::Fanbox]),
            };

            let output = output.join(&author.id());
            let path = output.join("author.json");
            debug!("Check old author data: {}", path.display());
            if path.exists() {
                debug!("Loading old author data");
                let file = File::open(&path)?;
                let reader = BufReader::new(file);
                let mut old_author: ArchiveAuthor = serde_json::from_reader(reader)?;
                old_author.extend(updated_author);
                updated_author = old_author;
            }

            debug!("Get author thumb");
            updated_author.thumb = updated_author
                .posts
                .iter()
                .find_map(|post| post.thumb.clone());
            archive_authors.push(updated_author);
        }

        archive_authors
    });

    Ok((archive_authors, archive_posts, download_files))
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
            debug!("Parse authors to ArchiveAuthorsList");
            let mut archive_authors = ArchiveAuthorsList::from_vector(authors.clone());
            let path = output.join("authors.json");
            debug!("Check old authors data: {}", path.display());
            if path.exists() {
                debug!("Loading old authors data");
                let file = File::open(&path).unwrap();
                let reader = BufReader::new(file);
                let mut old_authors: ArchiveAuthorsList = serde_json::from_reader(reader)?;
                old_authors.extend(archive_authors);
                archive_authors = old_authors;
            }

            let path = output.join("authors.json");
            info!("Writing authors.json");
            let mut file = File::create(&path).unwrap();
            file.write_all(serde_json::to_vec(&archive_authors)?.as_slice())
                .unwrap();
        }

        info!(
            "Writing `/[author]/author.json` (total: {})",
            authors.len()
        );
        for author in authors.into_iter() {
            let output = output.join(&author.id);
            let path = output.join("author.json");
            if !output.exists() {
                fs::create_dir(&output).await?;
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

        while let Some(_) = await_files.join_next().await {
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
