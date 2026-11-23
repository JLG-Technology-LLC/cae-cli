# Bug Assessment: pr-4-mock-faucet-partial-tcp-read

- **Slug**: pr-4-mock-faucet-partial-tcp-read
- **Created**: 2026-11-22
- **Source**: PR #4 (Comment ID: `4660961024`)
- **Verdict**: valid
- **Status**: Assessed (Awaiting Fix)
- **Severity**: medium
- **PR Thread**: PR #4 (Comment ID: `4660961024`)
- **GitHub Issue**: #21
- **Related Spec**: None

## Symptom & Reproduction

The in-process mock HTTP faucet servers (`spawn_mock_faucet`) in `tests/wallet_tests.rs` and `tests/e2e_all_commands.rs` perform only a single 1024-byte `TcpStream::read()` call before sending an HTTP response and closing the socket. When `reqwest`/`hyper` delivers the HTTP POST headers and JSON body in separate TCP writes (or when headers exceed 1024 bytes), the mock captures only a truncated request (missing the JSON body) and closes the socket with unread kernel buffer bytes, causing flaky assertion failures (`seen[0].contains(&address)`, `seen[0].contains("usdc")`) and TCP `RST` errors on the client.

**Observed**: `spawn_mock_faucet` reads a single TCP chunk (`stream.read(&mut chunk)`) and immediately responds and drops `stream`.
**Expected**: `spawn_mock_faucet` loops reading from `stream` until the `\r\n\r\n` header terminator and the full `Content-Length` payload bytes have been received (consistent with `spawn_mock_rpc`), then writes and flushes the HTTP response cleanly.

**Reproduction**:
1. Inspect `spawn_mock_faucet` in `tests/wallet_tests.rs:507-532` and `tests/e2e_all_commands.rs:288-311`.
2. Send an HTTP `POST` request to the mock server where the request line/headers and JSON body are written in two separate `write_all` calls (with a brief yield/delay between them, simulating `hyper` header/body framing).
3. Observe `rx.recv()` contains only the HTTP headers (missing `"address"` and `"usdc"`), failing assertions like `tests/wallet_tests.rs:588-589`, and closing the connection with unread bytes can cause a TCP `RST` on the client.

## Root Cause Analysis

- **Suspected Files**:
  - `tests/wallet_tests.rs:507-532` (`spawn_mock_faucet`)
  - `tests/e2e_all_commands.rs:288-311` (`spawn_mock_faucet`)
  - Reference implementation: `tests/wallet_tests.rs:322-378` and `tests/e2e_all_commands.rs:234-286` (`spawn_mock_rpc`)
- **Cause**: TCP is a byte stream with no message boundaries; a single `TcpStream::read()` is only guaranteed to return between 1 and `buf.len()` available bytes. `reqwest::blocking::Client` (`src/wallet/faucet.rs:49-64`) sends a `POST` request with a JSON body (`address`, `network`, `token`) and `Content-Length`, which `hyper` may flush in separate TCP writes for headers and body. Unlike `spawn_mock_rpc` (which loops until `\r\n\r\n` and `Content-Length` body bytes arrive), `spawn_mock_faucet` performs one `stream.read()` (`tests/wallet_tests.rs:517`, `tests/e2e_all_commands.rs:297`), forwards the partial bytes to the channel, writes the HTTP 200 response without flushing, and drops the socket.
- **Adjacent Risks**:
  - In `spawn_mock_rpc` (`tests/wallet_tests.rs:341-353`), comparing `text.len()` on a `String::from_utf8_lossy` result against `Content-Length` (bytes) can drift if non-UTF-8 bytes exist; implementing the header/body boundary check on raw `&[u8]` (`buf.windows(4).position(|w| w == b"\r\n\r\n")`) and comparing `buf.len() >= header_end + 4 + content_len` is exact and avoids UTF-8 length skew.
  - Setting a short read timeout on the accepted stream prevents the background mock thread from hanging indefinitely on a stalled connection.
  - Confidence: **high**.

## Proposed Remediation Plan

**Files likely to change**:
- `tests/wallet_tests.rs`
- `tests/e2e_all_commands.rs`

**Steps**:
1. Update `spawn_mock_faucet` in both `tests/wallet_tests.rs:507-532` and `tests/e2e_all_commands.rs:288-311` to set a read timeout on `stream` and loop calling `stream.read(&mut chunk)` until the byte sequence `b"\r\n\r\n"` is found and at least `Content-Length` body bytes (case-insensitive header parse) have been accumulated in `buf` (or EOF / timeout).
2. Send the complete captured request (or parsed body string) via `tx`, write the HTTP response, and call `stream.flush()` before dropping the stream.
3. Add a targeted regression test (e.g., in `tests/wallet_tests.rs`) that exercises `spawn_mock_faucet` with split TCP writes (headers in one write, body in a second write) to verify the full JSON body is reliably received without truncation or connection reset, and run `cargo test --test wallet_tests --test e2e_all_commands`.
