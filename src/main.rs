use git2::{Repository, Signature, ObjectType, IndexAddOption, Direction};
use rustop::opts;
use std::{fs::File, io::Write};

const TARGET_REPO_URL: &str = "https://github.com/Almo7aya/almo7aya.github.io.git";
const CLONED_REPO_PATH: &str = "dist/";
const NOTION_API_URL: &str = "https://api.notion.com/v1/databases/";
const DIST_PATH: &str = "dist/content/reading/";

#[derive(Debug)]
struct Args {
    gh_token: String,
    notion_token: String,
    notion_database_id: String,
}

#[derive(Debug)]
struct ReadingListItem {
    url: String,
    title: String,
    date: String,
}

#[derive(Debug)]
struct ReadingList(Vec<ReadingListItem>);

#[tokio::main]
async fn main() {
    let args = parse_args();
    let database_content = get_database_from_notion(&args)
        .await
        .expect("Failed to load database");

    let data = get_formatted_data_from_database(&database_content)
        .expect("Failed to parse notion database");

    let repo = clone_target_repo_from_gh().expect("Failed to clone target repo");

    write_mdfiles_to_dist(&data).expect("Failed to write MD files");

    setup_target_repo_commit_and_push(&repo, &args).expect("Failed to commit to repo");

    println!("Done uploading files");
}

fn setup_target_repo_commit_and_push(repo: &Repository, args: &Args) -> Result<(), git2::Error> {
    let mut config = repo.config()?;
    config.set_str(
        format!("url.{}.insteadOf", args.gh_token).as_str(),
        "https://github.com/",
    )?;
    let mut index = repo.index()?;
    index.add_all(["*"].iter(), IndexAddOption::DEFAULT, None)?;
    index.write()?;
    let oid = index.write_tree()?;

    let signature = Signature::now(
        "github-actions[bot]",
        "41898282+github-actions[bot]@users.noreply.github.com",
    )?;
    let tree = repo.find_tree(oid)?;
    let parent_commit = repo
        .head()?
        .resolve()?
        .peel(ObjectType::Commit)?
        .into_commit()
        .expect("Failed to get parent commit");

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "chore: update reading files",
        &tree,
        &[&parent_commit],
    )
    .expect("Failed to commit");

    let mut remote = match repo.find_remote("origin") {
        Ok(r) => r,
        Err(_) => repo.remote("origin", TARGET_REPO_URL)?,
    };
    remote.connect(Direction::Push)?;
    remote.push(&["refs/heads/main:refs/heads/main"], None)?;

    Ok(())
}

fn clone_target_repo_from_gh() -> Option<Repository> {
    let repo = match Repository::discover(CLONED_REPO_PATH) {
        Ok(repo) => repo,
        Err(_) => {
            let repo = match Repository::clone(TARGET_REPO_URL, CLONED_REPO_PATH) {
                Ok(repo) => repo,
                Err(e) => panic!("failed to open: {}", e),
            };
            repo
        }
    };

    Some(repo)
}

fn write_mdfiles_to_dist(list: &ReadingList) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(DIST_PATH).unwrap();

    for item in list.0.iter() {
        let mut file = File::create(format!("{}{}.{}", DIST_PATH, item.title, "md"))
            .expect(format!("Failed to open file {}{}.md", DIST_PATH, item.title).as_str());
        let content = format!(
            "\
---
title: \"{}\"
date: {}
draft: false
affiliatelink: {}
---
{}
",
            item.title, item.date, item.url, item.url
        );
        file.write_all(content.as_bytes())?;
    }

    Ok(())
}

fn get_formatted_data_from_database(database: &serde_json::Value) -> Option<ReadingList> {
    let len = database.get("results")?.as_array()?.len();
    let mut reading_list = ReadingList(Vec::with_capacity(len));

    for record in database.get("results")?.as_array()? {
        let props = record.get("properties")?;

        let item = ReadingListItem {
            url: props.get("URL")?.get("url")?.as_str()?.into(),
            title: props
                .get("Name")?
                .get("title")?
                .as_array()?
                .get(0)?
                .get("plain_text")?
                .as_str()?
                .replace("/", "-")
                .into(),
            date: record.get("created_time")?.as_str()?.into(),
        };

        reading_list.0.push(item);
    }

    Some(reading_list)
}

async fn get_database_from_notion(args: &Args) -> Result<serde_json::Value, reqwest::Error> {
    let url = format!("{}{}/query", NOTION_API_URL, args.notion_database_id);

    let client = reqwest::Client::new();
    let res: serde_json::Value = client
        .post(url)
        .header("Notion-Version", "2022-06-28")
        .header("authorization", format!("Bearer {}", args.notion_token))
        .header("accept", "application/json")
        .header("content-type", "application/json")
        .send()
        .await?
        .json()
        .await?;

    Ok(res)
}

fn parse_args() -> Args {
    let (args, _) = opts! {
        param gh_token:Option<String>, desc:"github token.";
        param notion_token:Option<String>, desc:"notion token.";
        param notion_database_id:Option<String>, desc:"notion database id.";
    }
    .parse_or_exit();

    Args {
        gh_token: args.gh_token.unwrap_or("gh_token".to_owned()),
        notion_token: args.notion_token.unwrap_or("notion_token".to_owned()),
        notion_database_id: args
            .notion_database_id
            .unwrap_or("notion_database_id".to_owned()),
    }
}
