#![allow(unused)]

extern crate core;

mod mutations;
mod files;
mod file_tree;
mod renderer;
pub mod config;

use std::fs::File;
use std::fs;
use std::io::{stdout, BufReader, Write};
use std::path::PathBuf;
use std::sync::Mutex;
use actix_files::Files;
use actix_web::{get, web, App, HttpResponse, HttpServer};
use serde::de::{DeserializeOwned, Error as DeError};
use mutest_json::call_graph::*;
use mutest_json::evaluation::*;
use mutest_json::mutations::*;
use mutest_json::tests::*;
use mutest_json::timings::*;

#[derive(Debug)]
struct Metadata {
    call_graph: CallGraphInfo,
    evaluation: EvaluationInfo,
    mutations: MutationsInfo,
    tests: TestsInfo,
    timings: TimingsInfo,
}

fn read_metadata<T: DeserializeOwned>(json_path: &PathBuf) -> Result<T, serde_json::Error> {
    let file = File::open(json_path).map_err(|err| {
        serde_json::Error::custom(format!("{}", err))
    })?;
    let reader = BufReader::new(file);
    let data: T = serde_json::from_reader(reader)?;
    Ok(data)
}

fn read_all_metadata(json_dir_path: &PathBuf) -> Result<Metadata, serde_json::Error> {
    let call_graph: CallGraphInfo = read_metadata::<CallGraphInfo>(&json_dir_path.join("call_graph.json"))?;
    let evaluation: EvaluationInfo = read_metadata::<EvaluationInfo>(&json_dir_path.join("evaluation.json"))?;
    let mutations: MutationsInfo = read_metadata::<MutationsInfo>(&json_dir_path.join("mutations.json"))?;
    let tests: TestsInfo = read_metadata::<TestsInfo>(&json_dir_path.join("tests.json"))?;
    let timings: TimingsInfo = read_metadata::<TimingsInfo>(&json_dir_path.join("timings.json"))?;
    let data = Metadata { call_graph, evaluation, mutations, tests, timings };
    Ok(data)
}

fn read_file(path: &PathBuf) -> Result<String, std::io::Error> {
    Ok(fs::read_to_string(path)?)
}

fn split_lines(data: &str) -> Vec<&str> {
    data.lines().collect()
}

pub struct AppState {
    renderer: Mutex<renderer::Renderer>
}

async fn show_start(data: web::Data<AppState>) -> HttpResponse {
    let mut body = String::new();
    {
        let mut renderer = data.renderer.lock().unwrap();
        body = renderer.render_start();
    }
    HttpResponse::Ok().body(body)
}

async fn show_file(data: web::Data<AppState>, file: web::Path<PathBuf,>) -> HttpResponse {
    let mut body = String::new();
    {
        let mut renderer = data.renderer.lock().unwrap();
        body = if renderer.valid_path(&file) {
            renderer.render_file(&file)
        } else {
            renderer.render_start_with_error(&format!("file not found: {}", file.display()))
        };
    }
    HttpResponse::Ok().body(body)
}

pub async fn server(conf: config::ServerConfig) -> std::io::Result<()> {
    println!("[mutest-report] loading mutest results...");
    let res = read_all_metadata(&conf.results_dir);
    let mutations_by_file = match res {
        Ok(metadata) => mutations::streamline_mutations(metadata),
        Err(e) => panic!("{:?}", e),
    };

    println!("[mutest-report] loading source files...");
    let paths = mutations::get_source_file_paths(&mutations_by_file);
    let source_files = match conf.source_dir {
        Some(source_dir) => files::Files::new(&source_dir, paths.clone()),
        None => {
            let source_dir = PathBuf::from(&conf.results_dir.parent().unwrap().parent().unwrap());
            files::Files::new(&source_dir, paths.clone())
        }
    }.expect("failed to read source files from path");

    println!("[mutest-report] creating renderer...");
    let mut renderer = renderer::Renderer::new(&conf.resource_dir, source_files.get_files_map(), mutations_by_file);

    println!("[mutest-report] caching interface components...");
    renderer.cache_mutations(conf.sys_diff_type);
    renderer.cache_file_tree(file_tree::FileTree::from_paths(&paths));
    renderer.cache_search();

    if conf.pre_cache_all {
        println!("[mutest-report] pre-caching {} files", paths.len());
        for path in &paths {
            let _ = renderer.render_file(path);
            print!(".");
            stdout().flush();
        }
        println!();
    }

    println!("[mutest-report] get started: http://127.0.0.1:{}/", conf.port);
    let state = web::Data::new(AppState {
        renderer: Mutex::new(renderer),
    });
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .route("/file/{file:.*}", web::get().to(show_file))
            .route("/", web::get().to(show_start))
            .service(
                Files::new("/static", &conf.resource_dir.join("static"))
            )
    })
        .bind(("127.0.0.1", conf.port))?
        .run()
        .await
}