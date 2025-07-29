#![feature(array_windows)]
#![allow(unused)]

extern crate core;

mod mutations;
mod files;
mod file_tree;
mod renderer;
pub mod config;

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::{fs, iter};
use std::io::{stdout, BufReader, Write};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Mutex;
use actix_files::Files;
use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use actix_web::http::header::LOCATION;
use actix_web::web::Redirect;
use serde::de::{DeserializeOwned, Error as DeError};
use serde::{Deserialize, Serialize};
use mutest_json::call_graph::*;
use mutest_json::evaluation::*;
use mutest_json::{DefId, Definition, Idx, IdxVec, Span};
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
struct TracesParams {
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

async fn get_traces(data: web::Data<AppState>, query: web::Query<TracesParams>) -> HttpResponse {
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

#[derive(Deserialize, Debug)]
struct TraceParams {
    mutation_id: u32,
    entry_point_id: u32,
    definition_ids: String,
}

pub enum NameOrPath {
    Name(String),
    Path(String),
    Err,
}

fn def_name_or_path(c: &Definition) -> NameOrPath {
    match (&c.name, &c.path) {
        (Some(name), _) => NameOrPath::Name(name.clone()),
        (_, Some(path)) => NameOrPath::Path(path.clone()),
        _ => NameOrPath::Err,
    }
}

pub struct CompleteDisplayCallee<'a> {
    caller_def_span: Span,
    snippet_end: (usize, usize),
    caller_name_or_path: NameOrPath,
    callee_name_or_path: NameOrPath,
    call_spans: Vec<&'a Span>,
}

pub struct IncompleteDisplayCallee {
    caller_path: String,
    callee_name_or_path: NameOrPath,
}

pub struct MutatedDisplayCallee {
    target_span: Span,
    snippet_end: (usize, usize),
    target_name: String,
    mutation_id: MutationId
}

pub enum DisplayCallee<'a> {
    Complete(CompleteDisplayCallee<'a>),
    Incomplete(IncompleteDisplayCallee),
    Mutated(MutatedDisplayCallee),
}

async fn get_trace(data: web::Data<AppState>, query: web::Query<TraceParams>) -> HttpResponse {
    fn get_all_ep_call_span(entry_point_id: EntryPointId, callee_def_id: DefId, call_graph: &CallGraph) -> Vec<&Span> {
        let mut call_spans = call_graph.entry_points[entry_point_id].calls.iter()
            .filter(|(cid, _)| call_graph.callees[**cid].def_id == callee_def_id)
            .map(|(_, ci)| ci.iter().map(|c| &c.span))
            .flatten()
            .flatten()
            .collect::<Vec<_>>();
        call_spans.sort_by(|a, b| Ord::cmp(&a.end.0, &b.end.0));
        call_spans
    }

    fn get_array_def_call_span(body_def_id: DefId, callee_def_id: DefId, call_graph: &CallGraph) -> Vec<&Span> {
        let mut call_spans = call_graph.callees.iter()
            .filter(|c| c.def_id == body_def_id)
            .flat_map(|c| c.calls.iter()
                .filter(|(cid, _)| call_graph.callees[**cid].def_id == callee_def_id))
            .flat_map(|(_, ci)| ci.iter().map(|c| &c.span))
            .flatten()
            .collect::<Vec<_>>();
        call_spans.sort_by(|a, b| Ord::cmp(&a.end.0, &b.end.0));
        call_spans
    }

    let definition_ids: Vec<DefId> = query.definition_ids.split(",").filter_map(|i| i.parse().ok()).map(|u| DefId(u)).collect();
    let mut body = String::new();
    let mut spans: Vec<DisplayCallee> = Vec::new();
    
    if definition_ids.is_empty() {
        return HttpResponse::NotFound().finish();
    }
    
    let Some(entry_point) = data.call_graph.call_graph.entry_points.iter().find(|e| e.entry_point_id == EntryPointId(query.entry_point_id)) else {
        return HttpResponse::NotFound().finish()
    };
    let call_spans = get_all_ep_call_span(entry_point.entry_point_id, definition_ids[0], &data.call_graph.call_graph);
    let callee_name_or_path = def_name_or_path(&data.call_graph.definitions[definition_ids[0]]);
    match (&entry_point.span, call_spans.last()) {
        (Some(span), Some(last_call_span)) => spans.push(DisplayCallee::Complete(CompleteDisplayCallee {
            caller_def_span: span.clone(),
            snippet_end: last_call_span.end,
            caller_name_or_path: NameOrPath::Name(entry_point.name.clone()),
            callee_name_or_path,
            call_spans,
        })),
        (Some(span), None) => spans.push(DisplayCallee::Complete(CompleteDisplayCallee {
            caller_def_span: span.clone(),
            snippet_end: span.end,
            caller_name_or_path: NameOrPath::Name(entry_point.name.clone()),
            callee_name_or_path,
            call_spans,
        })),
        _ => spans.push(DisplayCallee::Incomplete(IncompleteDisplayCallee {
            caller_path: entry_point.path.clone(),
            callee_name_or_path,
        })),
    }

    for [def_id, next_def_id] in definition_ids.array_windows::<2>() {
        let def = &data.call_graph.definitions[*def_id];
        let call_spans = get_array_def_call_span(*def_id, *next_def_id, &data.call_graph.call_graph);
        let caller_name_or_path = def_name_or_path(def);
        let callee_name_or_path = def_name_or_path(&data.call_graph.definitions[*next_def_id]);
        match (&def.span, call_spans.last()) {
            (Some(span), Some(last_call_span)) => spans.push(DisplayCallee::Complete(CompleteDisplayCallee {
                caller_def_span: span.clone(),
                snippet_end: last_call_span.end,
                caller_name_or_path,
                callee_name_or_path,
                call_spans,
            })),
            (Some(span), None) => spans.push(DisplayCallee::Complete(CompleteDisplayCallee {
                caller_def_span: span.clone(),
                snippet_end: span.end,
                caller_name_or_path,
                callee_name_or_path,
                call_spans,
            })),
            _ => spans.push(DisplayCallee::Incomplete(IncompleteDisplayCallee {
                caller_path: def.path.clone().unwrap_or_default(),
                callee_name_or_path,
            })),
        }
    }
    
    let target_def = &data.call_graph.definitions[*definition_ids.last().unwrap()];
    let mutation_id = MutationId(query.mutation_id);
    spans.push(DisplayCallee::Mutated(MutatedDisplayCallee {
        target_span: target_def.span.clone().unwrap(),
        snippet_end: data.mutations.mutations[mutation_id].origin_span.end,
        target_name: target_def.name.clone().unwrap_or_default(),
        mutation_id,
    }));

    { 
        let mut renderer = data.renderer.lock().unwrap();
        match renderer.render_trace(query.mutation_id, spans) {
            Ok(b) => body = b,
            Err(err) => {
                println!("\x1b[31m[mutest-report] {}\x1b[0m", err);
                return HttpResponse::SeeOther()
                    .append_header((LOCATION, "/"))
                    .finish()
            }
        }
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
            .route("/trace", web::get().to(get_trace))
            .service(
                Files::new("/static", &conf.resource_dir.join("static"))
            )
    })
        .bind(("127.0.0.1", conf.port))?
        .run()
        .await
}
