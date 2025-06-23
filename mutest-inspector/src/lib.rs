pub mod common;

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use serde::de::*;
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

fn read_metadata<T: DeserializeOwned>(json_path: PathBuf) -> Result<T, serde_json::Error> {
    let file = File::open(json_path).map_err(|err| {
        serde_json::Error::custom(format!("{}", err))
    })?;
    let reader = BufReader::new(file);
    let data: T = serde_json::from_reader(reader)?;
    Ok(data)
}

fn read_all_metadata(json_dir_path: PathBuf) -> Result<Metadata, serde_json::Error> {
    let cg: CallGraphInfo = read_metadata::<CallGraphInfo>(json_dir_path.join("call_graph.json"))?;
    let ev: EvaluationInfo = read_metadata::<EvaluationInfo>(json_dir_path.join("evaluation.json"))?;
    let mu: MutationsInfo = read_metadata::<MutationsInfo>(json_dir_path.join("mutations.json"))?;
    let te: TestsInfo = read_metadata::<TestsInfo>(json_dir_path.join("tests.json"))?;
    let ts: TimingsInfo = read_metadata::<TimingsInfo>(json_dir_path.join("timings.json"))?;
    let data = Metadata {
        call_graph: cg,
        evaluation: ev,
        mutations: mu,
        tests: te,
        timings: ts,
    };
    Ok(data)
}

pub fn server(json_dir_path: PathBuf) {
    let res = read_all_metadata(json_dir_path);
    if let Err(e) = res {
        println!("error: {}", e);
        return;
    }
    let md: Metadata = res.unwrap();
}

pub fn report(json_dir_path: PathBuf, export_path: PathBuf) {
    let res = read_all_metadata(json_dir_path);
    if let Err(e) = res {
        println!("error: {}", e);
        return;
    }
    let md: Metadata = res.unwrap();
    println!("metadata read successfully");
}
