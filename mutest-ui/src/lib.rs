#![allow(unused)]

extern crate core;

mod mutations;
mod files;
mod file_tree;
mod renderer;
pub mod config;

use std::collections::HashSet;
use std::fs::File;
use std::fs;
use std::io::{stdout, BufReader, Write};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Mutex;
use actix_files::Files;
use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use serde::de::{DeserializeOwned, Error as DeError};
use serde::{Deserialize, Serialize};
use mutest_json::call_graph::*;
use mutest_json::evaluation::*;
use mutest_json::{DefId, Idx, IdxVec};
use mutest_json::mutations::*;
use mutest_json::tests::*;
use mutest_json::timings::*;
use crate::mutations::Mutations;

#[derive(Debug, Clone)]
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
    renderer: Mutex<renderer::Renderer>,
    call_graph: CallGraphInfo,
    mutations: MutationsInfo,
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

#[derive(Deserialize)]
struct TraceParams {
    mutation_id: u32,
}

#[derive(Clone)]
struct MonoCallTrace {
    entry_point_id: EntryPointId,
    nested_calls: Vec<CalleeId>,
}

/// CallTrace2
#[derive(Hash, Eq, PartialEq)]
struct DefCallTrace {
    entry_point_id: EntryPointId,
    nested_calls: Vec<DefId>,
}

struct DefTraceGroup {
    entry_point_id: EntryPointId,
    nested_traces: Vec<Vec<DefId>>,
}

async fn get_traces(data: web::Data<AppState>, query: web::Query<TraceParams>) -> HttpResponse {
    fn build_traces(call_graph: &CallGraphInfo, target_def_id: DefId, call_trace: &mut MonoCallTrace, call_traces: &mut Vec<MonoCallTrace>) {
        let [.., callee_id] = &call_trace.nested_calls[..] else { return };
        let callee = &call_graph.call_graph.callees[*callee_id];

        if callee.def_id == target_def_id {
            call_traces.push(call_trace.clone());
            return;
        }

        for (nested_callee_id, _) in &callee.calls {
            if call_trace.nested_calls.iter().any(|c| c == nested_callee_id) {
                continue;
            }

            call_trace.nested_calls.push(*nested_callee_id);
            build_traces(call_graph, target_def_id, call_trace, call_traces);
            call_trace.nested_calls.pop();
        }
    }

    let mutation = &data.mutations.mutations[MutationId(query.mutation_id)];
    let target = &data.mutations.targets[mutation.target_id];
    let mut call_traces: Vec<MonoCallTrace> = Vec::new();
    for entry_point_name in target.reachable_from.keys() {
        let Some(entry_point) = data.call_graph.call_graph.entry_points.iter().find(|e| &e.path == entry_point_name) else {
            return HttpResponse::NotFound().finish()
        };
        for (callee_id, _) in &entry_point.calls {
            let mut call_trace = MonoCallTrace { entry_point_id: entry_point.entry_point_id, nested_calls: vec![*callee_id] };
            build_traces(&data.call_graph, target.def_id, &mut call_trace, &mut call_traces);
        }
    }
    
    let mut def_call_traces = HashSet::new();
    
    for call_trace in call_traces {
        let nested_calls = call_trace.nested_calls.iter().map(|callee_id| {
            let callee = &data.call_graph.call_graph.callees[*callee_id];
            callee.def_id
        }).collect();
        def_call_traces.insert(DefCallTrace { entry_point_id: call_trace.entry_point_id, nested_calls });
    }

    let mut call_trace_groups: Vec<DefTraceGroup> = Vec::new();

    'outer: for call_trace in &def_call_traces {
        for call_trace_group in &mut call_trace_groups {
            if call_trace_group.entry_point_id == call_trace.entry_point_id {
                call_trace_group.nested_traces.push(call_trace.nested_calls.clone());
                continue 'outer;
            }
        }
        call_trace_groups.push(DefTraceGroup { 
            entry_point_id: call_trace.entry_point_id,
            nested_traces: vec![call_trace.nested_calls.clone()]
        });
    }

    let mut body = String::new();

    {
        let mut renderer = data.renderer.lock().unwrap();
        renderer.render_call_traces_component(&mut body, query.mutation_id, &call_trace_groups, &data.call_graph);
    }

    HttpResponse::Ok().body(body)
}

pub async fn server(conf: config::ServerConfig) -> std::io::Result<()> {
    println!("[mutest-report] loading mutest results...");
    let res = read_all_metadata(&conf.results_dir);
    let mutations_by_file = match &res {
        Ok(metadata) => mutations::streamline_mutations(metadata.clone()),
        Err(e) => panic!("{:?}", e),
    };
    let call_graph = match &res {
        Ok(metadata) => Some(&metadata.call_graph),
        Err(e) => None,
    };

    println!("[mutest-report] loading source files...");
    let paths = match call_graph {
        Some(call_graph) => {
            let mut paths: HashSet<PathBuf> = HashSet::new();
            for definition in &call_graph.definitions {
                if let Some(span) = &definition.span {
                    paths.insert(span.path.clone());
                }
            }
            paths.into_iter().collect()
        },
        None => mutations::get_source_file_paths(&mutations_by_file),
    };
    let source_files = match &conf.source_dir {
        Some(source_dir) => files::Files::new(&source_dir, paths.clone()),
        None => {
            let source_dir = PathBuf::from(&conf.results_dir.parent().unwrap().parent().unwrap());
            files::Files::new(&source_dir, paths.clone())
        }
    }.expect("failed to read source files from path");

    println!("[mutest-report] creating renderer...");
    let ft = file_tree::FileTree::from_paths(&mutations_by_file.keys().cloned().collect());
    let mut renderer = renderer::Renderer::new(&conf.resource_dir, source_files.get_files_map(), mutations_by_file);

    println!("[mutest-report] caching interface components...");
    renderer.cache_mutations(conf.sys_diff_type);
    renderer.cache_file_tree(ft);
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
        call_graph: call_graph.unwrap().clone(),
        mutations: res?.mutations,
    });
    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .route("/", web::get().to(show_start))
            .route("/file/{file:.*}", web::get().to(show_file))
            .route("/api/traces", web::get().to(get_traces))
            .service(
                Files::new("/static", &conf.resource_dir.join("static"))
            )
    })
        .bind(("127.0.0.1", conf.port))?
        .run()
        .await
}