pub mod common;

use std::fs::File;
use std::fs;
use std::io::{BufReader};
use std::path::PathBuf;
use std::ptr::replace;
use serde::de::{DeserializeOwned, Error as DeError};
use mutest_json::call_graph::*;
use mutest_json::evaluation::*;
use mutest_json::{IdxVec, Span};
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

fn process_mutation(src_path: &PathBuf, mutation: &Mutation) -> Result<String, Box<dyn std::error::Error>> {
    let path = src_path.parent()
        .ok_or_else(|| "Could not get parent path")?
        .join(&mutation.origin_span.path);
    let md = &MutationData::new(&mutation.origin_span);
    let lines = &mut split_lines(&read_file(&path)?);
    let replacement = &mutation.substs.first().unwrap().substitute.replacement; // TODO: not good :(
    replace_mutation(md, lines, replacement);
    Ok(lines.join("\n"))
}

fn replace_mutation(md: &MutationData, file: &mut Vec<String>, replacement: &String) {
    let mut target_lines: Vec<String> = vec![];
    for i in md.begin.line..md.end.line+1 {
        target_lines.push(file[i].clone());
    }
    let target = target_lines.join("\n");
    let replaced = String::from(&target[0..md.begin.char]) + replacement + &target[md.end.char..target.len()];
    file.splice(md.begin.line..md.begin.line, split_lines(&replaced).iter().cloned());
}

fn iterate_over_mutations(src_path: &PathBuf, mutations: &IdxVec<MutationId, Mutation>) {
    mutations.iter().for_each(|mutation| {
        println!("processing mutation {:?}", mutation.mutation_id.0); // TODO: replace with actual logger
        if let Err(e) = process_mutation(src_path, &mutation) {
            println!("Error processing mutation: {}", e);
        }
    })
}

pub fn server(json_dir_path: &PathBuf) {
    let res = read_all_metadata(json_dir_path);
    if let Err(e) = res {
        println!("error: {}", e);
        return;
    }
    let md: Metadata = res.unwrap();
}

pub fn report(json_dir_path: &PathBuf, export_path: &PathBuf) {
    let res = read_all_metadata(json_dir_path);
    if let Err(e) = res {
        println!("error: {}", e);
        return;
    }
    let md: Metadata = res.unwrap();
    println!("metadata read successfully");

    iterate_over_mutations(json_dir_path, &md.mutations.mutations)
}