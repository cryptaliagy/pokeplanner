use opentelemetry::metrics::{Counter, Histogram, Meter};

/// Shared metrics instruments for the PokePlanner service.
///
/// Created from an OpenTelemetry `Meter` when OTEL is enabled.
/// All instruments are no-ops when the global meter provider is not configured.
#[derive(Clone)]
pub struct Metrics {
    // HTTP/gRPC request metrics
    pub request_counter: Counter<u64>,
    pub request_duration: Histogram<f64>,

    // PokeAPI client metrics
    pub pokeapi_request_counter: Counter<u64>,
    pub pokeapi_request_duration: Histogram<f64>,
    pub pokeapi_cache_hit_counter: Counter<u64>,
    pub pokeapi_cache_miss_counter: Counter<u64>,

    // Job metrics
    pub job_submitted_counter: Counter<u64>,
    pub job_completed_counter: Counter<u64>,
    pub job_failed_counter: Counter<u64>,
    pub job_duration: Histogram<f64>,

    // Team planner metrics
    pub team_candidate_pool_size: Histogram<u64>,
    pub team_plans_generated: Counter<u64>,
    pub move_selection_fallback_counter: Counter<u64>,
}

impl Metrics {
    pub fn new(meter: &Meter) -> Self {
        Self {
            request_counter: meter
                .u64_counter("http.server.request.count")
                .with_description("Total HTTP/gRPC requests")
                .build(),
            request_duration: meter
                .f64_histogram("http.server.request.duration")
                .with_description("HTTP/gRPC request duration in seconds")
                .with_unit("s")
                .build(),

            pokeapi_request_counter: meter
                .u64_counter("pokeapi.request.count")
                .with_description("Total PokeAPI HTTP requests")
                .build(),
            pokeapi_request_duration: meter
                .f64_histogram("pokeapi.request.duration")
                .with_description("PokeAPI request duration in seconds")
                .with_unit("s")
                .build(),
            pokeapi_cache_hit_counter: meter
                .u64_counter("pokeapi.cache.hit")
                .with_description("PokeAPI cache hits")
                .build(),
            pokeapi_cache_miss_counter: meter
                .u64_counter("pokeapi.cache.miss")
                .with_description("PokeAPI cache misses")
                .build(),

            job_submitted_counter: meter
                .u64_counter("job.submitted")
                .with_description("Jobs submitted")
                .build(),
            job_completed_counter: meter
                .u64_counter("job.completed")
                .with_description("Jobs completed successfully")
                .build(),
            job_failed_counter: meter
                .u64_counter("job.failed")
                .with_description("Jobs failed")
                .build(),
            job_duration: meter
                .f64_histogram("job.duration")
                .with_description("Job execution duration in seconds")
                .with_unit("s")
                .build(),

            team_candidate_pool_size: meter
                .u64_histogram("team.candidate_pool_size")
                .with_description("Number of candidates after filtering")
                .build(),
            team_plans_generated: meter
                .u64_counter("team.plans_generated")
                .with_description("Total team plans generated")
                .build(),
            move_selection_fallback_counter: meter
                .u64_counter("move_selection.fallback")
                .with_description("Move selection fallback events")
                .build(),
        }
    }

    /// Create a `Metrics` instance from the global meter provider.
    pub fn from_global() -> Self {
        let meter = opentelemetry::global::meter("pokeplanner");
        Self::new(&meter)
    }
}
