pub mod metrics;

pub use metrics::Metrics;

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Log output format for server binaries.
#[derive(Debug, Clone, Copy, Default)]
pub enum LogFormat {
    #[default]
    Text,
    Json,
}

impl std::str::FromStr for LogFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "text" => Ok(LogFormat::Text),
            "json" => Ok(LogFormat::Json),
            other => Err(format!(
                "unknown log format: {other} (expected text or json)"
            )),
        }
    }
}

impl std::fmt::Display for LogFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogFormat::Text => write!(f, "text"),
            LogFormat::Json => write!(f, "json"),
        }
    }
}

/// Configuration for server telemetry initialization.
pub struct ServerTelemetryConfig {
    /// OTLP exporter endpoint (e.g., "http://localhost:4317"). OTEL disabled when `None`.
    pub otlp_endpoint: Option<String>,
    /// Log output format (text or JSON).
    pub log_format: LogFormat,
    /// Base log level filter (e.g., "info", "debug", "pokeplanner=debug,info").
    pub log_level: String,
}

/// Guard that flushes and shuts down OTEL providers on drop.
pub struct TelemetryGuard {
    tracer_provider: Option<SdkTracerProvider>,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(provider) = self.tracer_provider.take() {
            if let Err(e) = provider.shutdown() {
                eprintln!("Failed to shut down tracer provider: {e}");
            }
        }
    }
}

/// Initialize telemetry for server binaries (REST, gRPC).
///
/// Sets up a layered tracing subscriber:
/// - `EnvFilter` for level filtering (respects `RUST_LOG` env var, falls back to `config.log_level`)
/// - `fmt` layer for stdout output (text or JSON)
/// - Optional OTEL tracing layer (when `config.otlp_endpoint` is set)
///
/// Returns a `TelemetryGuard` that must be held until server shutdown.
pub fn init_server_telemetry(config: ServerTelemetryConfig) -> TelemetryGuard {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level));

    let (tracer_provider, otel_layer) = if let Some(ref endpoint) = config.otlp_endpoint {
        match init_otel_tracing(endpoint) {
            Ok((provider, layer)) => (Some(provider), Some(layer)),
            Err(e) => {
                eprintln!("Failed to initialize OTEL tracing: {e}");
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    // OTEL layer must be added directly on Registry (it implements Layer<Registry>).
    // EnvFilter and fmt are added on top.
    match config.log_format {
        LogFormat::Text => {
            tracing_subscriber::registry()
                .with(otel_layer)
                .with(env_filter)
                .with(fmt::layer())
                .init();
        }
        LogFormat::Json => {
            tracing_subscriber::registry()
                .with(otel_layer)
                .with(env_filter)
                .with(fmt::layer().json())
                .init();
        }
    }

    TelemetryGuard { tracer_provider }
}

/// Initialize telemetry for the CLI binary.
///
/// Simple fmt subscriber with verbosity-based filtering:
/// - `0` → warn (default, minimal output)
/// - `1` (-v) → info (cache hits, job progress)
/// - `2+` (-vv) → debug (filtering decisions, API calls)
pub fn init_cli_telemetry(verbosity: u8) {
    let level = match verbosity {
        0 => "warn",
        1 => "info",
        _ => "debug",
    };

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));

    tracing_subscriber::fmt().with_env_filter(env_filter).init();
}

fn init_otel_tracing(
    endpoint: &str,
) -> Result<
    (
        SdkTracerProvider,
        tracing_opentelemetry::OpenTelemetryLayer<
            tracing_subscriber::Registry,
            opentelemetry_sdk::trace::SdkTracer,
        >,
    ),
    Box<dyn std::error::Error + Send + Sync>,
> {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(endpoint)
        .build()?;

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .build();

    let tracer = provider.tracer("pokeplanner");
    let layer = tracing_opentelemetry::layer().with_tracer(tracer);

    opentelemetry::global::set_tracer_provider(provider.clone());

    Ok((provider, layer))
}
