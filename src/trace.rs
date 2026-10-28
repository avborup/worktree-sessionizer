//! Lightweight profiling via `tracing` spans.
//!
//! Set **`WT_PROFILE=1`** to print each span exit on stderr as an indented line:
//! depth reflects nesting at span entry, followed by the span name, recorded fields
//! (instrument arguments), and elapsed wall time since entry.
//!
//! Lines appear in **completion order** (children before parents). Combine with how spans nest
//! to read the tree.
//!
//! With **`WT_PROFILE=1`**, crate **`wt`** is logged at **`info`** (merged into **`RUST_LOG`**
//! when set). Without **`WT_PROFILE`**, only **`RUST_LOG`** / the default error filter apply,
//! using the compact `tracing-subscriber` **`fmt`** layer.

use std::io::Write as _;
use std::time::{Duration, Instant};

use tracing::Subscriber;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
use tracing_subscriber::registry::{LookupSpan, Registry};
use tracing_subscriber::util::SubscriberInitExt;

const PROFILE_ENV: &str = "WT_PROFILE";

pub fn init() {
    let profile = profile_enabled();

    let mut filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("error"));

    if profile {
        filter = filter.add_directive("wt=info".parse().expect("wt=info directive"));
    }

    if profile {
        Registry::default()
            .with(filter)
            .with(TreeProfileLayer)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .with_writer(std::io::stderr)
            .init();
    }
}

fn profile_enabled() -> bool {
    std::env::var(PROFILE_ENV)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

/// Span-local storage: nesting depth (at entry), start time, and merged fields.
struct SpanStored {
    depth: usize,
    start: Instant,
    fields: Vec<(String, String)>,
}

#[derive(Default)]
struct TreeProfileLayer;

impl<S> Layer<S> for TreeProfileLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            return;
        };

        let depth = span_depth(&span);

        let mut rec = RecordFields(Vec::new());
        attrs.record(&mut rec);
        rec.0.sort_by(|a, b| a.0.cmp(&b.0));

        span.extensions_mut().insert(SpanStored {
            depth,
            start: Instant::now(),
            fields: rec.0,
        });
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else {
            return;
        };
        let mut extensions = span.extensions_mut();
        let Some(stored) = extensions.get_mut::<SpanStored>() else {
            return;
        };

        let mut rec = RecordFields(Vec::new());
        values.record(&mut rec);

        for (k, v) in rec.0 {
            if let Some(existing) = stored.fields.iter_mut().find(|(fk, _)| fk == &k) {
                existing.1 = v;
            } else {
                stored.fields.push((k, v));
            }
        }
        stored.fields.sort_by(|a, b| a.0.cmp(&b.0));
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(&id) else {
            return;
        };
        let Some(stored) = span.extensions_mut().remove::<SpanStored>() else {
            return;
        };

        let indent = "  ".repeat(stored.depth.saturating_sub(1));
        let name = span.metadata().name();

        let fields = if stored.fields.is_empty() {
            String::new()
        } else {
            stored
                .fields
                .iter()
                .map(|(k, v)| format!(" {k}={v}"))
                .collect::<String>()
        };

        let dur = format_duration(stored.start.elapsed());

        let mut stderr = std::io::stderr().lock();
        let _ = writeln!(stderr, "{indent}{name}{fields}  {dur}");
    }
}

fn span_depth<S>(span: &tracing_subscriber::registry::SpanRef<'_, S>) -> usize
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let mut n = 1;
    let mut opt = span.parent();
    while let Some(parent) = opt {
        n += 1;
        opt = parent.parent();
    }
    n
}

fn format_duration(d: Duration) -> String {
    let micros = d.as_micros();
    if micros >= 1_000_000 {
        format!("{:.2}s", d.as_secs_f64())
    } else if micros >= 1000 {
        format!("{}ms", d.as_millis())
    } else {
        format!("{micros}µs")
    }
}

struct RecordFields(Vec<(String, String)>);

impl Visit for RecordFields {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.0
            .push((field.name().to_string(), format!("{value:?}")));
    }
}
