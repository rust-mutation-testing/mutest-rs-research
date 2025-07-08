#![allow(unused)]

extern crate core;

pub mod common;
mod mutations;
mod rs_renderer;
mod files;

use std::fs::{create_dir_all, File};
use std::fs;
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

fn cp_scripts_and_styles(target_report_dir: &PathBuf) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let styles = explore_directory(&PathBuf::from("mutest-ui/src/styles"));
    let scripts = explore_directory(&PathBuf::from("mutest-ui/src/scripts"));
    let mut moved_styles: Vec<PathBuf> = Vec::new();
    let mut moved_scripts: Vec<PathBuf> = Vec::new();

    let styles_path = PathBuf::from(target_report_dir).join("__mutest_report_assets/styles");
    create_dir_all(&styles_path);
    for style in &styles {
        let new_style_path = PathBuf::from(&styles_path).join(style.file_name().unwrap());
        fs::copy(&style, &new_style_path);
        moved_styles.push(new_style_path);
    }

    let scripts_path = PathBuf::from(target_report_dir).join("__mutest_report_assets/scripts");
    create_dir_all(&scripts_path);
    for script in &scripts {
        let new_script_path = PathBuf::from(&scripts_path).join(script.file_name().unwrap());
        fs::copy(&script, &new_script_path);
        moved_scripts.push(new_script_path);
    }

    (moved_styles, moved_scripts)
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
    // TODO: cache GICs
    let cache_gic_elapsed = cache_gic_start.elapsed();

    println!("[mutest-report] caching file tree...");
    let cache_file_tree_start = Instant::now();
    // TODO: cache the file tree
    let cache_file_tree_elapsed = cache_file_tree_start.elapsed();

    println!("[mutest-report] caching mutations...");
    let mutations_cache_start = Instant::now();
    renderer.cache_mutations(rs_renderer::SysDiffType::Advanced);
    let mutations_cache_elapsed = mutations_cache_start.elapsed();

    println!("[mutest-report] copying assets...");
    let copy_assets_start = Instant::now();
    let (style_paths, script_paths) = cp_scripts_and_styles(&PathBuf::from("mutest/report"));
    let copy_assets_elapsed = copy_assets_start.elapsed();

    println!("[mutest-report] beginning render process...");
    let render_start = Instant::now();
    for path in _paths {
        let depth = path_depth(&path) + 1;
        let file = renderer.render_file(&path, depth, &style_paths, &script_paths);
        let mut fpath = PathBuf::from("mutest/report").join(path);
        fpath.set_extension("rs.html");
        create_dir_all(&fpath.parent().unwrap());
        fs::write(&fpath, file);
        println!("[mutest-report] created {}", &fpath.display());
    }
    let render_elapsed = render_start.elapsed();

    println!("[mutest-report] report created in {:?}, rendered {:?} line blocks. detailed timings below:", t_start.elapsed(), renderer.get_no_lines_rendered());
    println!("    load elapsed:            {:?}", load_elapsed);
    println!("    create renderer elapsed: {:?}", create_renderer_elapsed);
    println!("    caching GICs elapsed:    {:?}", cache_gic_elapsed);
    println!("    cache file tree elapsed: {:?}", cache_file_tree_elapsed);
    println!("    mutations cache elapsed: {:?}", mutations_cache_elapsed);
    println!("    copy assets elapsed:     {:?}", copy_assets_elapsed);
    println!("    render elapsed:          {:?}", render_elapsed);
}
