# tachylyte-services

Transport-free, offline-first state and intent boundaries for the first-party
application. This crate does **not** promise compatibility with any undocumented
server, choose endpoints, or perform network I/O. Applications must provide a
policy and transport, and must handle `Offline`, `Unauthenticated`, and
`Degraded` states explicitly. `MockTransport` is deterministic and in-memory;
it is not a protocol emulator.

URL navigation only accepts HTTP(S) and the configured host policy. Relative
recovery paths reject absolute paths and parent traversal. Cryptographic update
verification and platform-side printing/importing remain interfaces/intents;
this crate does not implement those platform operations.
