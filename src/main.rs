use std::path::PathBuf;

use async_curl::async_curl::AsyncCurl;
use curl::easy::Easy2;
use http::{HeaderMap, Method};
use http_client::Error;
use url::Url;

use crate::http_client::{Build, DownloadHandler, HttpClient, HttpRequest};

mod http_client;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Error> {
    let request = HttpRequest {
        url: Url::parse(
            "https://www.free-css.com/assets/images/free-css-templates/page296/healet.jpg",
        )
        .map_err(Error::ParseError)?,
        method: Method::GET,
        headers: HeaderMap::new(),
        body: Vec::new(),
    };
    let curl = AsyncCurl::new();
    let easy = Easy2::new(DownloadHandler::new(PathBuf::from(
        "E:\\VS_Codes\\http-client-example\\healet.jpg",
    ))?);
    let response = HttpClient::<Build>::new(curl, easy)
        .request(request)?
        .perform()
        .await?;

    println!("{:?}", response);
    Ok(())
}
