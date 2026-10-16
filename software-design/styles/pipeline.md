# Pipeline Architecture

## Use when
- Data flows through ordered stages: ingestion and indexing, ETL, compilers, media processing, batch jobs, CI tools.
- Stages can be developed, tested and replaced independently.

## Structure
- **Stages**: each one takes a well-defined input type and returns a well-defined output type (scan → parse → transform → enrich → write).
- **Stage contracts**: the data types passed between stages are the architecture; keep them explicit, versionable and serializable when stages may run in different processes.
- **Orchestrator**: runs the stages in order, owns retries, cancellation, progress reporting and the overall result. Stages do not call each other.
- **Sources and sinks**: reading inputs and writing outputs are stages too, behind interfaces, so the pipeline can run on fixtures in tests.

## Rules
- A stage has no hidden dependency on another stage's internals; it only sees its input.
- Stages are deterministic for the same input when possible; side effects are confined to sink stages.
- Errors are per item when items are independent: one bad file is recorded and skipped, it does not abort the run, unless the error makes the whole run invalid.
- Incremental work is explicit: change detection (hashes, timestamps) decides what enters the pipeline, and outputs of unchanged items are not rewritten or duplicated.
- Every run reports what it did (items in, out, skipped, failed, duration) in a structured result and in logs.
- Long runs support cooperative cancellation checked between items.

## Where things go
| Piece | Location |
|---|---|
| Detecting what changed | Source/scan stage |
| Format-specific parsing | One parser per format behind a common interface |
| Filtering and quality checks | A dedicated stage, with reasons recorded |
| Writing to storage or indexes | Sink stage(s) |
| Order, retries, progress, cancellation | Orchestrator |

## Common mistakes
- An orchestrator function that also parses, filters and writes (one giant loop).
- Stages communicating through shared mutable state or global variables.
- Re-processing everything on every run because change detection was bolted on late.
- Appending outputs without removing the previous version of the same item (duplicates).

## Examples
- Document ingestion: scanner → chunkers per format → quality gate → enricher → embedder → vector and lexical index writers.
- Compiler: lexer → parser → semantic analysis → optimizer → code generator.
