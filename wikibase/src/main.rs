#![deny(
    future_incompatible,
    nonstandard_style,
    rust_2018_idioms,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unused_qualifications
)]

use crate::loader::WikibaseLoader;
use argh::FromArgs;
use async_std::future::Future;
use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::task::spawn;
use http_types::content::{Accept, ContentType};
use http_types::{
    bail_status, headers, Error, Method, Mime, Request, Response, Result, StatusCode,
};
use oxigraph::io::GraphFormat;
use oxigraph::model::{GraphName, NamedNode, NamedOrBlankNode};
use oxigraph::sparql::{Query, QueryResults, QueryResultsFormat};
use oxigraph::RocksDbStore;
use std::str::FromStr;
use std::time::Duration;
use url::form_urlencoded;

mod loader;

const MAX_SPARQL_BODY_SIZE: u64 = 1_048_576;
const SERVER: &str = concat!("Oxigraph/", env!("CARGO_PKG_VERSION"));

#[derive(FromArgs)]
/// Oxigraph SPARQL server for Wikibase
struct Args {
    /// specify a server socket to bind using the format $(HOST):$(PORT)
    #[argh(option, short = 'b', default = "\"localhost:7878\".to_string()")]
    bind: String,

    /// directory in which persist the data
    #[argh(option, short = 'f')]
    file: String,

    #[argh(option)]
    /// base URL of the MediaWiki API like https://www.wikidata.org/w/api.php
    mediawiki_api: String,

    #[argh(option)]
    /// base URL of MediaWiki like https://www.wikidata.org/wiki/
    mediawiki_base_url: String,

    #[argh(option)]
    /// namespaces ids to load like "0,120"
    namespaces: Option<String>,

    #[argh(option)]
    /// slot to load like "mediainfo". Could not be use with namespaces
    slot: Option<String>,
}

#[async_std::main]
pub async fn main() -> Result<()> {
    let args: Args = argh::from_env();

    let store = RocksDbStore::open(args.file)?;
    let mediawiki_api = args.mediawiki_api.clone();
    let mediawiki_base_url = args.mediawiki_base_url.clone();
    let namespaces = args
        .namespaces
        .as_deref()
        .unwrap_or("")
        .split(',')
        .flat_map(|t| {
            let t = t.trim();
            if t.is_empty() {
                None
            } else {
                Some(u32::from_str(t).unwrap())
            }
        })
        .collect::<Vec<_>>();
    let slot = args.slot.clone();
    let repo = store.clone();
    let mut loader = WikibaseLoader::new(
        repo,
        &mediawiki_api,
        &mediawiki_base_url,
        &namespaces,
        slot.as_deref(),
        Duration::new(10, 0),
    )
    .unwrap();
    spawn(async move {
        loader.initial_loading().unwrap();
        loader.update_loop();
    });

    println!("Listening for requests at http://{}", &args.bind);

    http_server(&args.bind, move |request| {
        handle_request(request, store.clone())
    })
    .await
}

async fn handle_request(request: Request, store: RocksDbStore) -> Result<Response> {
    Ok(match (request.url().path(), request.method()) {
        ("/query", Method::Get) => {
            configure_and_evaluate_sparql_query(store, url_query(&request), None, request)?
        }
        ("/query", Method::Post) => {
            if let Some(content_type) = request.content_type() {
                if content_type.essence() == "application/sparql-query" {
                    let mut buffer = String::new();
                    let mut request = request;
                    request
                        .take_body()
                        .take(MAX_SPARQL_BODY_SIZE)
                        .read_to_string(&mut buffer)
                        .await?;
                    configure_and_evaluate_sparql_query(
                        store,
                        url_query(&request),
                        Some(buffer),
                        request,
                    )?
                } else if content_type.essence() == "application/x-www-form-urlencoded" {
                    let mut buffer = Vec::new();
                    let mut request = request;
                    request
                        .take_body()
                        .take(MAX_SPARQL_BODY_SIZE)
                        .read_to_end(&mut buffer)
                        .await?;
                    configure_and_evaluate_sparql_query(store, buffer, None, request)?
                } else {
                    bail_status!(415, "Not supported Content-Type given: {}", content_type)
                }
            } else {
                bail_status!(400, "No Content-Type given");
            }
        }
        _ => bail_status!(
            404,
            "{} {} is not supported by this server",
            request.method(),
            request.url().path()
        ),
    })
}

fn url_query(request: &Request) -> Vec<u8> {
    request.url().query().unwrap_or("").as_bytes().to_vec()
}

fn configure_and_evaluate_sparql_query(
    store: RocksDbStore,
    encoded: Vec<u8>,
    mut query: Option<String>,
    request: Request,
) -> Result<Response> {
    let mut default_graph_uris = Vec::new();
    let mut named_graph_uris = Vec::new();
    for (k, v) in form_urlencoded::parse(&encoded) {
        match k.as_ref() {
            "query" => {
                if query.is_some() {
                    bail_status!(400, "Multiple query parameters provided")
                }
                query = Some(v.into_owned())
            }
            "default-graph-uri" => default_graph_uris.push(v.into_owned()),
            "named-graph-uri" => named_graph_uris.push(v.into_owned()),
            _ => (),
        }
    }
    if let Some(query) = query {
        evaluate_sparql_query(store, query, default_graph_uris, named_graph_uris, request)
    } else {
        bail_status!(400, "You should set the 'query' parameter")
    }
}

fn evaluate_sparql_query(
    store: RocksDbStore,
    query: String,
    default_graph_uris: Vec<String>,
    named_graph_uris: Vec<String>,
    request: Request,
) -> Result<Response> {
    let mut query = Query::parse(&query, None).map_err(bad_request)?;
    let default_graph_uris = default_graph_uris
        .into_iter()
        .map(|e| Ok(NamedNode::new(e)?.into()))
        .collect::<Result<Vec<GraphName>>>()
        .map_err(bad_request)?;
    let named_graph_uris = named_graph_uris
        .into_iter()
        .map(|e| Ok(NamedNode::new(e)?.into()))
        .collect::<Result<Vec<NamedOrBlankNode>>>()
        .map_err(bad_request)?;

    if !default_graph_uris.is_empty() || !named_graph_uris.is_empty() {
        query.dataset_mut().set_default_graph(default_graph_uris);
        query
            .dataset_mut()
            .set_available_named_graphs(named_graph_uris);
    }

    let results = store.query(query)?;
    //TODO: stream
    if let QueryResults::Graph(_) = results {
        let format = content_negotiation(
            request,
            &[
                GraphFormat::NTriples.media_type(),
                GraphFormat::Turtle.media_type(),
                GraphFormat::RdfXml.media_type(),
            ],
            GraphFormat::from_media_type,
        )?;
        let mut body = Vec::default();
        results.write_graph(&mut body, format)?;
        let mut response = Response::from(body);
        ContentType::new(format.media_type()).apply(&mut response);
        Ok(response)
    } else {
        let format = content_negotiation(
            request,
            &[
                QueryResultsFormat::Xml.media_type(),
                QueryResultsFormat::Json.media_type(),
                QueryResultsFormat::Csv.media_type(),
                QueryResultsFormat::Tsv.media_type(),
            ],
            QueryResultsFormat::from_media_type,
        )?;
        let mut body = Vec::default();
        results.write(&mut body, format)?;
        let mut response = Response::from(body);
        ContentType::new(format.media_type()).apply(&mut response);
        Ok(response)
    }
}

async fn http_server<
    F: Clone + Send + Sync + 'static + Fn(Request) -> Fut,
    Fut: Send + Future<Output = Result<Response>>,
>(
    host: &str,
    handle: F,
) -> Result<()> {
    async fn accept<F: Fn(Request) -> Fut, Fut: Future<Output = Result<Response>>>(
        stream: TcpStream,
        handle: F,
    ) -> Result<()> {
        async_h1::accept(stream, |request| async {
            let mut response = match handle(request).await {
                Ok(result) => result,
                Err(error) => {
                    if error.status().is_server_error() {
                        eprintln!("{}", error);
                    }
                    let mut response = Response::new(error.status());
                    response.set_body(error.to_string());
                    response
                }
            };
            response.append_header(headers::SERVER, SERVER);
            Ok(response)
        })
        .await
    }

    let listener = TcpListener::bind(&host).await?;
    let mut incoming = listener.incoming();
    while let Some(stream) = incoming.next().await {
        let stream = stream?;
        let handle = handle.clone();
        spawn(async {
            if let Err(err) = accept(stream, handle).await {
                eprintln!("{}", err);
            };
        });
    }
    Ok(())
}

fn bad_request(e: impl Into<Error>) -> Error {
    let mut e = e.into();
    e.set_status(StatusCode::BadRequest);
    e
}

fn content_negotiation<F>(
    request: Request,
    supported: &[&str],
    parse: impl Fn(&str) -> Option<F>,
) -> Result<F> {
    if let Some(mut accept) = Accept::from_headers(request)? {
        let supported: Vec<Mime> = supported
            .iter()
            .map(|h| Mime::from_str(h).unwrap())
            .collect();
        parse(accept.negotiate(&supported)?.value().as_str())
    } else {
        parse(supported.first().ok_or_else(|| {
            Error::from_str(
                StatusCode::InternalServerError,
                "No default MIME type provided",
            )
        })?)
    }
    .ok_or_else(|| Error::from_str(StatusCode::InternalServerError, "Unknown mime type"))
}
