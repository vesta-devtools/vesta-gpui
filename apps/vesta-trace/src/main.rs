#![forbid(unsafe_code)]
mod analysis;
mod control;
mod counters;
mod gpu;
#[cfg(test)]
mod gpu_tests;
mod import;
mod inspection;
mod native;
mod plot;
mod query;
#[cfg(test)]
mod query_tests;
mod session;
#[cfg(test)]
mod tests;
mod timeline;
mod view;
mod worker;

use std::{path::PathBuf, sync::atomic::AtomicBool};

fn set_scene(current: &mut native::Scene, next: native::Scene) -> Result<(), session::TraceError> {
    if *current != native::Scene::Basic && *current != next {
        return Err(session::TraceError::new(
            "trace_usage",
            "choose one native acceptance scene",
        ));
    }
    *current = next;
    Ok(())
}

fn run() -> Result<(), session::TraceError> {
    let mut input = None;
    let mut capture = None;
    let mut summary = false;
    let mut smoke = false;
    let mut scene = native::Scene::Basic;
    let mut alternate = None;
    let mut filter = String::new();
    let mut query_stdio = false;
    let mut schema = false;
    let mut self_trace = false;
    for argument in std::env::args().skip(1) {
        if argument == "--trace-ui" || argument.starts_with("--trace-ui=") {
            self_trace = true;
        } else if argument == "--schema" {
            schema = true;
        } else if argument == "--query-stdio" {
            query_stdio = true;
        } else if argument == "--summary" {
            summary = true;
        } else if argument == "--smoke" {
            smoke = true;
        } else if argument == "--inspect-smoke" {
            smoke = true;
            set_scene(&mut scene, native::Scene::Inspection)?;
        } else if let Some(value) = argument.strip_prefix("--inspect-capture=") {
            capture = Some(PathBuf::from(value));
            smoke = true;
            set_scene(&mut scene, native::Scene::Inspection)?;
        } else if let Some(value) = argument.strip_prefix("--navigation-capture=") {
            capture = Some(PathBuf::from(value));
            smoke = true;
            set_scene(&mut scene, native::Scene::Navigation)?;
        } else if let Some(value) = argument.strip_prefix("--focus-capture=") {
            capture = Some(PathBuf::from(value));
            smoke = true;
            set_scene(&mut scene, native::Scene::Focus)?;
        } else if let Some(value) = argument.strip_prefix("--lifecycle-capture=") {
            capture = Some(PathBuf::from(value));
            smoke = true;
            set_scene(&mut scene, native::Scene::Lifecycle)?;
        } else if let Some(value) = argument
            .strip_prefix("--focus-source=")
            .or_else(|| argument.strip_prefix("--lifecycle-source="))
        {
            if alternate.replace(PathBuf::from(value)).is_some() {
                return Err(session::TraceError::new(
                    "trace_usage",
                    "one alternate native source is required",
                ));
            }
        } else if let Some(value) = argument.strip_prefix("--smoke-capture=") {
            capture = Some(PathBuf::from(value));
            smoke = true;
        } else if let Some(value) = argument.strip_prefix("--filter=") {
            filter = value.into();
        } else if !argument.starts_with('-') && input.is_none() {
            input = Some(PathBuf::from(argument));
        } else {
            return Err(session::TraceError::new(
                "trace_usage",
                "vesta-trace [GPUI trace.json|report.html] [--schema|--query-stdio|--summary|--smoke] [--filter=text]",
            ));
        }
    }
    if usize::from(schema) + usize::from(query_stdio) + usize::from(summary) + usize::from(smoke)
        > 1
        || ((schema || query_stdio) && (!filter.is_empty() || capture.is_some()))
        || (schema && input.is_some())
        || (self_trace && (schema || summary || query_stdio))
        || matches!(scene, native::Scene::Focus | native::Scene::Lifecycle) != alternate.is_some()
    {
        return Err(session::TraceError::new(
            "trace_usage",
            "choose one mode; query selection belongs in typed requests",
        ));
    }
    if filter.len() > 256 {
        return Err(session::TraceError::new(
            "trace_filter_budget",
            "filter exceeds 256 UTF-8 bytes",
        ));
    }
    if schema {
        println!("{}", query::transport::schemas());
        return Ok(());
    }
    if query_stdio {
        let input = input.ok_or_else(|| {
            session::TraceError::new(
                "trace_usage",
                "query-stdio requires one explicit input capture",
            )
        })?;
        let cancel = AtomicBool::new(false);
        let session = import::read(&input, &cancel)?;
        return query::transport::serve(
            &session,
            &mut std::io::stdin().lock(),
            &mut std::io::stdout().lock(),
            &cancel,
        );
    }
    if !summary {
        return native::run(
            input,
            native::Options {
                smoke,
                filter,
                capture_dir: capture,
                scene,
                alternate,
            },
        )
        .map_err(|e| session::TraceError::new("trace_native", e.to_string()));
    }
    let input = input
        .ok_or_else(|| session::TraceError::new("trace_usage", "summary requires a trace file"))?;
    let cancel = AtomicBool::new(false);
    let session = import::read(&input, &cancel)?;
    let result = analysis::analyze(&session, session.bounds, &filter, &cancel)?;
    let (callers, callees) = result
        .timers
        .first()
        .map(|t| {
            analysis::relations(
                &session,
                session.bounds,
                &t.category,
                &t.name,
                &analysis::Selection::default(),
                &mut control::Control::new(&cancel, u64::MAX, u64::MAX),
            )
        })
        .transpose()?
        .unwrap_or_default();
    let visible = session
        .lanes
        .iter()
        .map(|l| l.visible(&session, session.bounds).count())
        .sum::<usize>();
    println!(
        "{}",
        serde_json::json!({"status":"analyzed","events":session.events.len(),"visible_events":visible,"frames":result.frames.len(),"lanes":session.lanes.iter().map(|l|serde_json::json!({"process":l.key.process,"thread":l.key.thread,"max_depth":l.max_depth})).collect::<Vec<_>>(),"range_ns":[result.range.start,result.range.end],"duration_ns":result.range.duration(),"frame_percentile_ns":result.percentile_ns,"worst_frame":result.max_frame.map(|(k,d)|serde_json::json!({"process":k.process,"event":k.event,"duration_ns":d})),"timers":result.timers.len(),"top_callers":callers.len(),"top_callees":callees.len(),"detail":session.detail,"dropped_events":session.dropped_events,"unframed_events":session.unframed_events,"hierarchy_reliable":result.hierarchy_reliable,"diagnostics":session.diagnostics.iter().map(|d|serde_json::json!({"code":d.code,"message":d.message})).collect::<Vec<_>>() })
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        if std::env::args().any(|arg| arg == "--query-stdio") {
            use std::io::Write;
            let mut output = std::io::stdout().lock();
            let response = query::failure(None, None, error);
            let _ = serde_json::to_writer(&mut output, &response);
            let _ = output.write_all(b"\n");
            std::process::exit(1);
        }
        eprintln!(
            "{}",
            serde_json::json!({"status":"error","code":error.code,"message":error.message})
        );
        std::process::exit(1);
    }
}
