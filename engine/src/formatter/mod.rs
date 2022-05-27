use std::fs;
use tera::Context;
use tera::Tera;

use crate::analytics::AnalyticsResult;

pub fn generate(analytics: &AnalyticsResult) {
    let template = include_str!("index.html");
    let mut tera = Tera::default();
    tera.add_raw_template("index.html", template).unwrap();

    let mut context = Context::new();
    context.insert("analytics", analytics);
    let content = tera.render("index.html", &context).unwrap();
    fs::write("index.html", content).expect("Unable to write file");
}
