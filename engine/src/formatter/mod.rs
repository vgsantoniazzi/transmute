use tera::Context;
use tera::Tera;
use std::fs;

use crate::analytics::AnalyticsResult;

pub fn generate(analytics: &AnalyticsResult) {
    let mut context = Context::new();
    let tera = Tera::new("*.html").unwrap();
    let content = tera.render("src/formatter/index.html", &context).unwrap();
    fs::write("index.html", content).expect("Unable to write file");
}
