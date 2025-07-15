#![allow(unused)]

extern crate core;

pub mod common;
mod mutations;
mod rs_renderer;
mod files;
mod file_tree;

use std::collections::HashMap;
use std::fs::{create_dir_all, File};
use std::{env, fs};
use std::io::{BufReader};
use std::path::{Component, PathBuf};
use std::process::exit;
use std::ptr::replace;
use std::time::Instant;
use serde::de::{DeserializeOwned, Error as DeError};
use syntect::parsing::SyntaxSet;
use log::error;
use walkdir::WalkDir;
use mutest_json::call_graph::*;
use mutest_json::evaluation::*;
use mutest_json::{IdxVec, Span};
use mutest_json::mutations::*;
use mutest_json::tests::*;
use mutest_json::timings::*;
use crate::rs_renderer::Renderer;

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
    let cg: CallGraphInfo = read_metadata::<CallGraphInfo>(&json_dir_path.join("call_graph.json"))?;
    let ev: EvaluationInfo = read_metadata::<EvaluationInfo>(&json_dir_path.join("evaluation.json"))?;
    let mu: MutationsInfo = read_metadata::<MutationsInfo>(&json_dir_path.join("mutations.json"))?;
    let te: TestsInfo = read_metadata::<TestsInfo>(&json_dir_path.join("tests.json"))?;
    let ts: TimingsInfo = read_metadata::<TimingsInfo>(&json_dir_path.join("timings.json"))?;
    let data = Metadata {
        call_graph: cg,
        evaluation: ev,
        mutations: mu,
        tests: te,
        timings: ts,
    };
    Ok(data)
}

fn read_file(path: &PathBuf) -> Result<String, std::io::Error> {
    Ok(fs::read_to_string(path)?)
}

fn split_lines(data: &str) -> Vec<&str> {
    data.lines().collect()
}

fn explore_directory(target_directory: &PathBuf) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = Vec::new();

    for entry in WalkDir::new(target_directory) {
        let path = entry.expect("WalkDir error").into_path();
        if path.is_file() {
            files.push(path);
        }
    }

    files
}

fn asset_dir(dir_name: &str) -> String {
    format!("__mutest_report_assets/{}", dir_name)
}

fn with_res_dir(dir: &str) -> PathBuf {
    match env::var("MUTEST_RESOURCE_DIR") {
        Ok(res_dir) => PathBuf::from(res_dir).join(dir),
        Err(e) => PathBuf::from(dir),
    }
}

fn cp_files(report_root_dir: &PathBuf, moved_root: &str, files: &Vec<PathBuf>) {
    let moved_root_path = PathBuf::from(report_root_dir).join(moved_root);
    create_dir_all(&moved_root_path);
    for file in files {
        let new_style_path = PathBuf::from(&moved_root_path).join(file.file_name().unwrap());
        fs::copy(&file, &new_style_path);
    }
}

fn copy_all(src_dir_name: &str, out_dir_name: &str, out_dir: &PathBuf) {
    let styles = explore_directory(&with_res_dir(src_dir_name));
    cp_files(out_dir, &asset_dir(out_dir_name), &styles);
}

fn path_depth(path: &PathBuf) -> usize {
    path.components().filter(|c| matches!(c, Component::Normal(_))).count()
}

pub fn server(json_dir_path: &PathBuf) {
    // TODO: reuse below code in future
}

pub fn report(json_dir_path: &PathBuf, export_path: &PathBuf) {
    let res = read_all_metadata(json_dir_path);
    if let Err(e) = res {
        println!("error: {}", e);
        exit(1);
    }

    let t_start = Instant::now();

    println!("[mutest-report] loading assets...");
    let load_start = Instant::now();
    let streamlined = mutations::streamline_mutations(res.unwrap());
    let paths = mutations::get_source_file_paths(&streamlined);
    let _paths = paths.clone();
    let paths_root = PathBuf::from(json_dir_path.parent().unwrap().parent().unwrap());
    let source_files = files::Files::new(&paths_root, paths);
    if let Err(e) = source_files {
        println!("error: {}", e);
        exit(1);
    }
    let load_elapsed = load_start.elapsed();

    println!("[mutest-report] creating renderer...");
    let create_renderer_start = Instant::now();
    let mut renderer = Renderer::new(streamlined, source_files.unwrap().get_files_map());
    let create_renderer_elapsed = create_renderer_start.elapsed();

    println!("[mutest-report] caching generic interface components (GICs)...");
    let cache_gic_start = Instant::now();

    let search_frame_start = Instant::now();
    renderer.render_search_frame(&export_path);
    let search_frame_elapsed = search_frame_start.elapsed();

    let cache_gic_elapsed = cache_gic_start.elapsed();

    println!("[mutest-report] caching file tree...");
    let cache_file_tree_start = Instant::now();
    let mut ft = file_tree::FileTree::new();
    for path in &_paths {
        ft.insert_path(path);
    }
    ft.sort();
    renderer.cache_file_tree(ft);
    let cache_file_tree_elapsed = cache_file_tree_start.elapsed();

    println!("[mutest-report] caching mutations...");
    let mutations_cache_start = Instant::now();
    renderer.cache_mutations(rs_renderer::SysDiffType::Advanced);
    let mutations_cache_elapsed = mutations_cache_start.elapsed();

    println!("[mutest-report] copying assets...");
    let copy_assets_start = Instant::now();
    let report_path = PathBuf::from("mutest/report");
    copy_all("styles", "styles", &export_path);
    copy_all("scripts", "scripts", &export_path);
    copy_all("assets/icons", "icons", &export_path);
    let copy_assets_elapsed = copy_assets_start.elapsed();

    println!("[mutest-report] beginning render process...");
    let render_start = Instant::now();
    for path in _paths {
        let depth = path_depth(&path) - 1;
        let file = renderer.render_file(&path, depth);
        let mut fpath = PathBuf::from("mutest/report").join(path);
        fpath.set_extension("rs.html");
        create_dir_all(&fpath.parent().unwrap());
        fs::write(&fpath, file);
        println!("[mutest-report] created {}", &fpath.display());
    }
    let render_elapsed = render_start.elapsed();

    println!("[mutest-report] report created in {:?}, rendered {:?} line blocks. detailed timings below:", t_start.elapsed(), renderer.get_no_lines_rendered());
    println!("    load elapsed:             {:?}", load_elapsed);
    println!("    create renderer elapsed:  {:?}", create_renderer_elapsed);
    println!("    caching GICs elapsed:     {:?}", cache_gic_elapsed);
    println!("        caching search frame: {:?}", search_frame_elapsed);
    println!("    cache file tree elapsed:  {:?}", cache_file_tree_elapsed);
    println!("    mutations cache elapsed:  {:?}", mutations_cache_elapsed);
    println!("    copy assets elapsed:      {:?}", copy_assets_elapsed);
    println!("    render elapsed:           {:?}", render_elapsed);
}