# chromadb migrations

The SQL files under `sysdb/`, `metadb/` and `embeddings_queue/` are chromadb 1.5.9's
migrations (`chromadb/migrations/`), copied byte for byte: chromadb checks each applied
migration's MD5 hash, so a store created by the engine must hold the same files.
chromadb is licensed under the Apache License 2.0 (https://github.com/chroma-core/chroma).
