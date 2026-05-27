use log::{info, warn};
use serde::Serialize;
use std::fs;
use tera::Context;
use tera::Tera;

use crate::analytics::AnalyticsResult;

#[derive(Serialize)]
struct Report<'a> {
    failures: usize,
    uncovered_failures: usize,
    low_confidence_failures: usize,
    analytics: &'a AnalyticsResult,
}

pub fn generate(analytics: AnalyticsResult, formatter: &str, output: &str) {
    info!("generating {} output", formatter);

    match formatter {
        "json" => generate_json(&analytics, resolved_output(output, "result.json")),
        "html" => generate_html(&analytics, resolved_output(output, "index.html")),
        _ => {
            warn!("formatter not found, exporting as JSON");
            generate_json(&analytics, resolved_output(output, "result.json"));
        }
    }
}

fn resolved_output<'a>(requested: &'a str, default: &'a str) -> &'a str {
    if requested.is_empty() {
        default
    } else {
        requested
    }
}

fn generate_html(analytics: &AnalyticsResult, output_path: &str) {
    let template = include_str!("index.html");
    let mut tera = Tera::default();
    tera.add_raw_template("index.html", template)
        .expect("bundled HTML template must compile");

    let mut context = Context::new();
    context.insert("analytics", analytics);
    context.insert("failures", &analytics.failures());
    context.insert("uncovered_failures", &analytics.uncovered_failures());
    context.insert(
        "low_confidence_failures",
        &analytics.low_confidence_failures(),
    );
    let content = tera
        .render("index.html", &context)
        .expect("HTML template must render with the analytics model");
    write_or_fallback(output_path, &content);
}

fn generate_json(analytics: &AnalyticsResult, output_path: &str) {
    let report = Report {
        failures: analytics.failures(),
        uncovered_failures: analytics.uncovered_failures(),
        low_confidence_failures: analytics.low_confidence_failures(),
        analytics,
    };
    let content = serde_json::to_string_pretty(&report)
        .expect("AnalyticsResult must be serializable to JSON");
    write_or_fallback(output_path, &content);
}

fn write_or_fallback(output_path: &str, content: &str) {
    if let Err(e) = fs::write(output_path, content) {
        eprintln!(
            "transmute: failed to write {}: {}; dumping report to stdout",
            output_path, e
        );
        println!("{}", content);
    }
}
