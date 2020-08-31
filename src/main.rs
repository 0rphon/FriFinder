use regex;
use serde::Deserialize;
use reqwest::Error;

use std::{fmt, error};
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

const SUB_URL: &str = "https://www.reddit.com/r/Instagram.json";
const POST_APPEND: &str = ".json?limit=10000";
const ERROR_LOG: &str = "error.log";
const OUTPUT_FILE: &str = "output.txt";
const KEY_WORD: &str = "Follow Friday";

#[derive(Debug)]
pub enum CustomError {
    GetUrlError,
    ParseError(String),
    NullJsonError,
}
impl fmt::Display for CustomError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CustomError::GetUrlError        => write!(f, "GetUrlError: Failed to find post url"),
            CustomError::ParseError(part)   => write!(f, "ParseError: could not parse comment: \"{}\"", part),
            CustomError::NullJsonError      => write!(f, "NullJsonError: json field was empty"),
        }
    }
}
impl error::Error for CustomError {}

fn log_event<T: fmt::Display>(event: T) -> T {
    if !Path::new(ERROR_LOG).exists() {
        File::create(ERROR_LOG)
            .unwrap_or_else(|e| panic!("Error creating log file during crash: {}\n crash: {}",e,event));
        log_event(format!("Log file created"));
    }

    let mut file = File::open(ERROR_LOG)
        .unwrap_or_else(|e| panic!("Error opening log during crash: {}\n crash: {}",e,event));

    let mut log = String::new();
    file.read_to_string(&mut log)
        .unwrap_or_else(|e| panic!("Error reading log during crash: {}\n crash: {}",e,event));
    if log != "" { log = format!("{}\n{}", log, event.to_string())}
    else {log = event.to_string()}

    let mut file = File::create(ERROR_LOG)
        .unwrap_or_else(|e| panic!("Error creating log during crash: {}\n crash: {}",e,event));
    file.write_all(log.as_bytes())
        .unwrap_or_else(|e| panic!("Error writing log during crash: {}\n crash: {}",e,event));

    event
}



#[derive(Deserialize, Debug)]
pub struct SubResponse {
    pub data: Option<SubRData>
}

#[derive(Deserialize, Debug)]
pub struct SubRData {
    pub children:   Option<Vec<SubChild>>,
}

#[derive(Deserialize, Debug)]
pub struct SubChild {
    pub data: Option<SubCData>,
}

#[derive(Deserialize, Debug)]
pub struct SubCData {
    pub title:      Option<String>,
    pub stickied:   Option<bool>,
    pub url:        Option<String>,
}

pub fn get_page(url: &str) -> Result<SubResponse, Error> {
    Ok(reqwest::blocking::get(url)?.json()?)
}

#[derive(Deserialize, Debug)]
pub struct PostResponse {
    pub data: Option<PostData>
}

#[derive(Deserialize, Debug)]
pub struct PostData {
    pub children: Option<Vec<PostChildren>>
}

#[derive(Deserialize, Debug)]
pub struct PostChildren {
    pub data: Option<PostCommentData>
}

#[derive(Deserialize, Debug)]
pub struct PostCommentData {
    pub body: Option<String>
}

pub fn get_post(url: &str) -> Result<Vec<PostResponse>, Error> {
    Ok(reqwest::blocking::get(url)?.json()?)
}



fn get_friday_url(resp: SubResponse) -> Option<String> {
    for child in resp.data?.children? {
        if let Some(cdata) = child.data {
            if cdata.stickied.unwrap_or(false) 
                && cdata.title.unwrap_or(String::new()).contains(KEY_WORD) {
                if let Some(url) = cdata.url {
                    return Some(url)
                }
            }
        }
    }
    None
}

fn get_comments(post: Vec<PostResponse>) -> Option<Vec<String>> {
    let mut comments = Vec::new();
    for child in post.get(1)?.data.as_ref()?.children.as_ref()? {
        if let Some(data) = &child.data {
            if let Some(body) = &data.body {
                comments.push(body.clone())
            }
        }
    }
    Some(comments)
}

fn get_instagrams(comments: Vec<String>) -> Result<Vec<String>, Box<dyn error::Error>> {
    let mut usernames = Vec::new();
    let re = regex::Regex::new(r"([\[\]\n ]+)")?;
    for comment in comments {
        let parts = re.split(&comment).collect::<Vec<&str>>();
        for i in 0..parts.len() {
            let re = regex::Regex::new(r"[\\()*]")?;
            if parts[i].contains("@") {
                if parts[i].len() != 1 {usernames.push(re.replace_all(parts[i],"").to_string())}
                else {usernames.push(format!("@{}",re.replace_all(parts[i+1],"")))}
                break
            } else if parts[i].contains("https://www.instagram.com") {
                let part = parts[i].split("/").nth(3).ok_or(CustomError::ParseError(parts[i].to_string()))?;
                usernames.push(format!("@{}",re.replace_all(part,"")));
                break
            }
        }
    }
    usernames.sort();
    usernames.dedup();
    Ok(usernames)
}

fn main() {
    fn execute() -> Result<(), Box<dyn error::Error>> {
        let resp = get_page(SUB_URL)?;
        let post_url = get_friday_url(resp).ok_or(CustomError::GetUrlError)?;
        let post = get_post(&format!("{}{}",post_url, POST_APPEND))?;
        let posts = get_comments(post).ok_or(CustomError::NullJsonError)?;
        let users = get_instagrams(posts)?;
        let mut file = File::create(OUTPUT_FILE)?;
        for user in &users {
            println!("{}",user);
            file.write(format!("{}\n",user).as_bytes())?;
        }
        println!("{} results saved to {}", users.len(), OUTPUT_FILE);
        Ok(())
    }
    execute().unwrap_or_else(|e|panic!("{}",log_event(e)))
}