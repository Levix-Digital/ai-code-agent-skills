---
plan_id: drift
project: parity
title: Sync engine
version: v2.0.0
status: active
created_at: '2026-02-01T10:00:00'
milestones:
- name: M1 Design API
  done: true
  done_at: '2026-02-02'
  by: Someone
- name: M2 Build sync engine
  done: false
- name: M3 Ship
  done: false
---

# Sync engine

Remove LangGraph from the agent and build the sync engine in Rust.
Kafka stays for events; replace the old queue.
