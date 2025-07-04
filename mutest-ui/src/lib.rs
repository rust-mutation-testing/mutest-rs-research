#![allow(unused)]

extern crate core;

pub mod common;
mod mutations;
mod rs_renderer;
mod files;

use std::fs::File;
use std::fs;
use std::io::{BufReader};
use std::path::PathBuf;
use std::process::exit;
use std::ptr::replace;
use serde::de::{DeserializeOwned, Error as DeError};
use syntect::parsing::SyntaxSet;
use syntect_assets::assets::HighlightingAssets;
use log::error;
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

fn split_lines(data: &String) -> Vec<String> {
    data.replace("\r", "").split("\n").map(|s| s.to_string()).collect()
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
    
    let streamlined = mutations::streamline_mutations(res.unwrap());
    let paths = mutations::get_source_file_paths(&streamlined);
    let paths_root = PathBuf::from(json_dir_path.parent().unwrap());
    let source_files = files::Files::new(&paths_root, paths);
    if let Err(e) = source_files {
        println!("error: {}", e);
        exit(1);
    }
    
    let mut renderer = Renderer::new(streamlined, source_files.unwrap().get_files_map());
    renderer.cache_mutations(rs_renderer::SysDiffType::Simple);
    renderer.render_file(json_dir_path.clone());
}