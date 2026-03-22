# Add guard pattern for job state transitions to prevent stuck jobs

## TL;DR

The job lifecycle in `run_team_plan_job` relies on manual `fail_job()` + `return` at every error path. Missing either one leaves jobs stuck in `Running` forever. Restructure to guarantee terminal state on all exit paths.

## Problem

In `crates/pokeplanner-service/src/lib.rs:225-410`, the `run_team_plan_job` function is a 185-line async function that manually manages job state transitions. Every error path must:

1. Call `Self::fail_job(&storage, &mut job, "message").await`
2. Execute `return`

There are currently 3 `fail_job` + `return` pairs (at lines 266-272, 285-291, 345-351) and one early return without `fail_job` (line 236, when the initial job fetch fails). The `run_generic_job` function (lines 66-83) has a similar pattern but is simpler.

If a developer adds a new step and forgets the `fail_job` call, or if a panic occurs between status transitions, the job remains in `Running` permanently with no recovery mechanism.

## Acceptance Criteria

- [ ] Jobs are guaranteed to reach a terminal state (`Completed` or `Failed`) on all exit paths, including panics
- [ ] Adding a new fallible step to the job pipeline doesn't require remembering to call `fail_job`
- [ ] Existing job progress updates (3-phase tracking) still work correctly
- [ ] `cargo test` passes, including existing job lifecycle tests

## Implementation Guidance

### Recommended approach: inner function + single error handler

Restructure `run_team_plan_job` to separate orchestration from execution. The outer function handles state transitions; the inner function uses `?` for all errors:

```rust
async fn run_team_plan_job(storage: Arc<S>, pokeapi: Arc<P>, job_id: JobId, request: TeamPlanRequest) {
    let mut job = match storage.get_job(&job_id).await {
        Ok(j) => j,
        Err(e) => { warn!("Failed to get job {job_id}: {e}"); return; }
    };

    job.status = JobStatus::Running;
    // ... initial setup ...

    match Self::execute_team_plan(&storage, &pokeapi, &mut job, &request).await {
        Ok(plans) => {
            job.status = JobStatus::Completed;
            job.result = Some(JobResult { message: "...", data: serde_json::to_value(&plans).ok() });
        }
        Err(e) => {
            job.status = JobStatus::Failed;
            job.result = Some(JobResult { message: e.to_string(), data: None });
            warn!(job_id = %job_id, "job failed: {e}");
        }
    }

    if let Err(e) = storage.update_job(&job).await {
        warn!(job_id = %job_id, "Failed to persist final job state: {e}");
    }
}

async fn execute_team_plan(
    storage: &S, pokeapi: &P, job: &mut Job, request: &TeamPlanRequest
) -> Result<Vec<TeamPlan>, AppError> {
    // Step 1: Fetch candidates — use `?` for errors
    // Step 2: Filter — return Err if empty
    // Step 3: Plan — use `?` for errors
    // Progress updates via job mutation + storage writes
    Ok(plans)
}
```

This gives you `?` propagation, a single error-handling site, and guaranteed final state persistence.

### Alternative: Drop guard (more complex)

A `JobGuard` struct that marks jobs as `Failed` on drop has the async-Drop problem — `Drop` can't be async. Workarounds include:

1. `tokio::spawn` a cleanup task in `Drop` that takes ownership of storage Arc + job ID (fire-and-forget)
2. Use `tokio::task::block_in_place` in Drop (blocks the thread)

The inner-function approach is simpler and recommended.

### Files to modify

- `crates/pokeplanner-service/src/lib.rs` — refactor `run_team_plan_job` (lines 225-410)
- Optionally refactor `run_generic_job` (lines 66-83) for consistency

## Things to Note

- The `fail_job` helper at lines 456-466 can be removed or simplified to just a logging utility once the restructure handles state transitions.
- Progress updates (`job.progress = Some(JobProgress { ... })`) happen mid-execution at lines 241-245, 308-313, and 355-360. The inner function approach preserves these — the `job: &mut Job` parameter allows progress mutations during execution.
- `run_generic_job` (lines 66-83) has the same pattern but is trivial (sleep 100ms, complete). Consider applying the same restructure for consistency, or leave it as-is since it has no error paths beyond the initial fetch.
- The `tokio::spawn` that calls `run_team_plan_job` at line 217 means panics in the job function are caught by tokio (logged as a warning). A panic would still leave the job stuck. If panic safety is desired, wrap the spawned future in `std::panic::AssertUnwindSafe` + `catch_unwind` and mark the job failed on panic.
