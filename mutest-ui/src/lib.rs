#![allow(unused)]

extern crate core;

pub mod common;
mod formatter;
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

struct MutationPoint {
    line: usize,
    char: usize,
}

impl MutationPoint {
    fn new((point_line, point_char): &(usize, usize)) -> MutationPoint {
        MutationPoint { line: *point_line - 1, char: *point_char - 1 } // offset for arrays starting at 0
    }
}

struct MutationData {
    begin: MutationPoint,
    end: MutationPoint,
}

impl MutationData {
    fn new(mutation: &Span) -> MutationData {
        MutationData {begin: MutationPoint::new(&mutation.begin), end: MutationPoint::new(&mutation.end)}
    }
}

fn process_mutation(src_path: &PathBuf, mutation: &Mutation, opts: &formatter::HighlighterOpts) -> Result<String, Box<dyn std::error::Error>> {
    let path = src_path.parent()
        .ok_or_else(|| "Could not get parent path")?
        .join(&mutation.origin_span.path);
    let md = &MutationData::new(&mutation.origin_span);
    let lines = &mut split_lines(&read_file(&path)?);
    let replacement = &mutation.substs.first().unwrap().substitute.replacement; // TODO: not good :(
    replace_mutation(md, lines, replacement);

    let diffs = formatter::LineDiff::new();

    let formatted = formatter::highlight_and_diff2(opts, lines, diffs)?; // TODO: not good :(
    Ok(formatted)
}

fn replace_mutation(md: &MutationData, file: &mut Vec<String>, replacement: &String) {
    let target_lines = file[md.begin.line..=md.end.line].to_owned();
    let target = target_lines.join("\n");
    let replaced = String::from(&target[0..md.begin.char]) + replacement + &target[md.end.char..target.len()];
    file.splice(md.begin.line..md.begin.line, split_lines(&replaced).iter().cloned());
}

fn iterate_over_mutations(src_path: &PathBuf, mutations: &IdxVec<MutationId, Mutation>, export_path: &PathBuf, opts: &formatter::HighlighterOpts) {
    for mutation in mutations {
        println!("processing mutation {:?}", mutation.mutation_id.0); // TODO: replace with actual logger
        let proc = process_mutation(src_path, &mutation, opts);
        if let Err(e) = &proc {
            println!("Error processing mutation: {}", e);
        }
        let formatted = formatter::render_tpl(&proc.unwrap());
    }
}

fn get_highlighter_opts() -> formatter::HighlighterOpts {
    let syntax_set = SyntaxSet::load_defaults_newlines();
    let assets = HighlightingAssets::from_binary();
    let theme = assets.get_theme("Monokai Extended Origin");
    formatter::HighlighterOpts::new(syntax_set, theme.clone())
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
    renderer.cache_mutations();
    // let opts = get_highlighter_opts();
    // iterate_over_mutations(json_dir_path, &md.mutations.mutations, export_path, &opts);
    // println!("operation completed");
}