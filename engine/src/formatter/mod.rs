use tera::Context;

use crate::analytics::AnalyticsResult;

impl HTML {
    pub fn generate(analytics: &AnalyticsResult) {
        let mut context = Context::new();
        context.insert("product", &product);
        context.insert("vat_rate", &0.20);
        tera.render("index.html", &context)?
    }
}
