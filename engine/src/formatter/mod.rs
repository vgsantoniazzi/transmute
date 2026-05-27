use log::{info, warn};
use serde::Serialize;
use std::fs;
use tera::Context;
use tera::Tera;

use crate::analytics::AnalyticsResult;

#[derive(Serialize)]
struct Report<'a> {
    failures: usize,
    analytics: &'a AnalyticsResult,
}

pub fn generate(mut analytics: AnalyticsResult, formatter: &str, output: &str) {
    info!("generating {} output", formatter);

    match formatter {
        "json" => generate_json(&mut analytics, resolved_output(output, "result.json")),
        "html" => generate_html(&mut analytics, resolved_output(output, "index.html")),
        _ => {
            warn!("formatter not found, exporting as JSON");
            generate_json(&mut analytics, resolved_output(output, "result.json"));
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

fn generate_html(analytics: &mut AnalyticsResult, output_path: &str) {
    let template = include_str!("index.html");
    let mut tera = Tera::default();
    tera.add_raw_template("index.html", template).unwrap();

    let mut context = Context::new();
    context.insert("analytics", &*analytics);
    context.insert("failures", &analytics.failures());
    let content = tera.render("index.html", &context).unwrap();
    fs::write(output_path, content).expect("Unable to write file");
}

fn generate_json(analytics: &mut AnalyticsResult, output_path: &str) {
    let report = Report {
        failures: analytics.failures(),
        analytics,
    };
    let content = serde_json::to_string_pretty(&report).unwrap();
    fs::write(output_path, content).expect("Unable to write file");
}
