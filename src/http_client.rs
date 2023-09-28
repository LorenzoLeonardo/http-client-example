// Standard libraries
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

// 3rd party crates
use async_curl::async_curl::AsyncCurl;
use curl::easy::{Easy2, Handler, WriteError};
use http::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use http::method::Method;
use http::status::StatusCode;
use url::Url;

///
/// Error type returned by failed curl HTTP requests.
///
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Error returned by curl crate.
    #[error("curl request failed")]
    Curl(#[source] curl::Error),
    /// Non-curl HTTP error.
    #[error("HTTP error")]
    Http(#[source] http::Error),
    /// Error returned by curl crate.
    #[error("async curl request failed")]
    AsyncCurl(#[source] async_curl::async_curl_error::AsyncCurlError),
    /// Error returned by curl crate.
    #[error("File error")]
    IOError(#[source] std::io::Error),
    /// Error returned by curl crate.
    #[error("Parse error")]
    ParseError(#[source] url::ParseError),
    /// Other error.
    #[error("Other error: {}", _0)]
    Other(String),
}

#[derive(Clone, Debug)]
pub struct HttpRequest {
    pub url: Url,
    pub method: http::method::Method,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct HttpResponse {
    pub status_code: http::status::StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

#[derive(Clone)]
struct DebugHttpRequest {
    url: Url,
    body: Vec<u8>,
    header: HeaderMap<HeaderValue>,
    method: Method,
}

impl From<&HttpRequest> for DebugHttpRequest {
    fn from(value: &HttpRequest) -> Self {
        Self {
            url: value.url.to_owned(),
            body: value.body.to_owned(),
            header: value.headers.to_owned(),
            method: value.method.to_owned(),
        }
    }
}

impl fmt::Display for DebugHttpRequest {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Request:\n\tUrl:{}\n\tMethod:{}\n\tHeader:{:?}\n\tBody:{}",
            self.url,
            self.method,
            self.header,
            String::from_utf8(self.body.to_owned()).unwrap_or(String::new())
        )
    }
}

/// ```
#[derive(Debug)]
pub struct DownloadHandler {
    file: File,
    path: PathBuf,
}

impl Handler for DownloadHandler {
    /// This will store the response from the server
    /// to the data vector.
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
        match self.file.write_all(data) {
            Ok(_) => Ok(data.len()),
            Err(_) => Err(WriteError::Pause),
        }
    }
}

impl DownloadHandler {
    pub fn new(path: PathBuf) -> Result<Self, Error> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(&path)
            .map_err(Error::IOError)?;
        Ok(Self { file, path })
    }

    #[allow(unused)]
    pub fn existing_file_size(&self) -> usize {
        if let Ok(metadata) = std::fs::metadata(&self.path) {
            metadata.len() as usize
        } else {
            0
        }
    }
}

pub struct Build;
pub struct Perform;

pub struct HttpClient<S> {
    curl: AsyncCurl<DownloadHandler>,
    easy: Easy2<DownloadHandler>,
    _state: S,
}

impl HttpClient<Build> {
    pub fn new(curl: AsyncCurl<DownloadHandler>, easy: Easy2<DownloadHandler>) -> Self {
        Self {
            curl,
            easy,
            _state: Build,
        }
    }

    pub fn request(mut self, request: HttpRequest) -> Result<HttpClient<Perform>, Error> {
        println!("{}", DebugHttpRequest::from(&request));

        self.easy.url(&request.url.to_string()[..]).map_err(|e| {
            println!("{:?}", e);
            Error::Curl(e)
        })?;

        let mut headers = curl::easy::List::new();
        request.headers.iter().try_for_each(|(name, value)| {
            headers
                .append(&format!(
                    "{}: {}",
                    name,
                    value.to_str().map_err(|_| Error::Other(format!(
                        "invalid {} header value {:?}",
                        name,
                        value.as_bytes()
                    )))?
                ))
                .map_err(|e| {
                    println!("{:?}", e);
                    Error::Curl(e)
                })
        })?;

        self.easy.http_headers(headers).map_err(|e| {
            println!("{:?}", e);
            Error::Curl(e)
        })?;

        if let Method::POST = request.method {
            self.easy.post(true).map_err(Error::Curl)?;
            self.easy
                .post_field_size(request.body.len() as u64)
                .map_err(|e| {
                    println!("{:?}", e);
                    Error::Curl(e)
                })?;
            self.easy
                .post_fields_copy(request.body.as_slice())
                .map_err(|e| {
                    println!("{:?}", e);
                    Error::Curl(e)
                })?;
        } else {
            assert_eq!(request.method, Method::GET);
        }
        Ok(HttpClient::<Perform> {
            curl: self.curl,
            easy: self.easy,
            _state: Perform,
        })
    }
}

impl HttpClient<Perform> {
    pub async fn perform(self) -> Result<HttpResponse, Error> {
        let mut easy = self.curl.send_request(self.easy).await.map_err(|e| {
            println!("{:?}", e);
            Error::AsyncCurl(e)
        })?;

        //let data = easy.get_ref().to_owned().get_data();
        let status_code = easy.response_code().map_err(|e| {
            println!("{:?}", e);
            Error::Curl(e)
        })? as u16;
        let response_header = easy
            .content_type()
            .map_err(|e| {
                println!("{:?}", e);
                Error::Curl(e)
            })?
            .map(|content_type| {
                Ok(vec![(
                    CONTENT_TYPE,
                    HeaderValue::from_str(content_type).map_err(|err| {
                        println!("{:?}", err);
                        Error::Http(err.into())
                    })?,
                )]
                .into_iter()
                .collect::<HeaderMap>())
            })
            .transpose()?
            .unwrap_or_else(HeaderMap::new);

        let data = Vec::new();
        println!(
            "Response:\n\tHeader:{:?}\n\tBody:{}\n\tStatus Code:{}\n\n",
            &response_header,
            String::from_utf8(data.to_owned()).unwrap_or(String::new()),
            &status_code
        );
        Ok(HttpResponse {
            status_code: StatusCode::from_u16(status_code).map_err(|err| {
                println!("{:?}", err);
                Error::Http(err.into())
            })?,
            headers: response_header,
            body: data,
        })
    }
}
