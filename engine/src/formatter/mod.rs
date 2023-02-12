use log::{info, warn};
use std::fs;
use tera::Context;
use tera::Tera;

use crate::analytics::AnalyticsResult;

pub fn generate(analytics: AnalyticsResult, formatter: String) {
    info!("generating {} output", formatter);

    match formatter.as_str() {
        "json" => generate_json(analytics),
        "html" => generate_html(analytics),
        _ => {
            warn!("formatter not found, exporting as JSON");
            generate_json(analytics)
        }
    }
}

fn generate_html(mut analytics: AnalyticsResult) {
    let template = include_str!("index.html");
    let mut tera = Tera::default();
    tera.add_raw_template("index.html", template).unwrap();

    let mut context = Context::new();
    context.insert("analytics", &analytics);
    context.insert("failures", &analytics.failures());
    let content = tera.render("index.html", &context).unwrap();
    fs::write("index.html", content).expect("Unable to write file");
}

fn generate_json(analytics: AnalyticsResult) {
    let content = serde_json::to_string_pretty(&analytics).unwrap();
    fs::write("result.json", content).expect("Unable to write file")
}
