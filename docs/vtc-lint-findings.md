# Linting the varnish-cache-plus test suite

Date: 2026-09-29. Every `.vtc` under `bin/varnishtest/tests` and
`bin/varnishtest/vmodtests` of varnish-cache-plus (2496 files) was linted with
`varnishls lint --level hint`, run from the repo root with a `.varnishls.toml`
containing only `vcc_paths = ["./lib"]`. Diagnostics inside
`varnish vN -errvcl { ... }` blocks were separated out, since that VCL is
supposed to fail.

## Numbers

| | Count |
|---|---|
| Files linted | 2496 |
| Files with unexpected errors | 491 |
| Unexpected errors | 2020 |
| Expected errors (inside `-errvcl`) | 179 |
| Style hints (lowercase headers, `else if`, `beresp.cacheable`) | 976 |
| Parser panics | 0 |

Parse coverage of the same files with the tree-sitter grammars: 1263/1275
`tests`, 1203/1203 `vmodtests`. The 12 remaining failures are intentionally
invalid negative tests.

## Unexpected errors by root cause

Ordered by count. "Fix" describes what varnishls has to do.

| Count | Diagnostic | Root cause | Fix |
|---|---|---|---|
| ~600 | `Not found` on `s1`, `s2`, ... | `-vcl+backend` makes varnishtest generate one backend per `server sN` in the file. varnishls does not model this. | When a `vcl_argument` is `-vcl+backend`, define a BACKEND for each `server` ident in the vtc document. |
| 647 + 126 | `X is undefined`, `Undefined subroutine` | Tests `include "aws/init.vcl"` etc. and rely on `param.set vcl_path ${topsrc}/lib`. varnishls only searches `vcl_paths`, so objects and subs from those files are missing, and so are vmods they import (`crypto`, `std`, `json`, `brotli`). | Configuration: add `./lib` to `vcl_paths` in the repo's `.varnishls.toml`. Verified: aws_vcl/test05 and s3/test19 drop to zero errors. Optionally parse `param.set vcl_path` from `-cliok` arguments. |
| 253 | `X is defined in <file> at line N` | A vtc with several `varnish v1 -vcl { }` blocks is one scope to varnishls, so `new jwt_reader` in a later block is a duplicate of the first. Also causes `b.encode is not a method` (8). | Give each `-vcl` block its own scope for definitions (or skip the duplicate check across blocks). |
| 215 | `Cannnot be called from vcl_synth` etc. | The sub-to-context table in `src/document.rs` (near the `$Restrict` check) maps only six subs. `vcl_synth` (177), `vcl_backend_error` (30), `vcl_hit`, `vcl_miss`, `vcl_pass`, `vcl_pipe`, `vcl_hash`, `vcl_purge`, `vcl_connect`, `vcl_backend_refresh` never match `$Restrict client`/`backend`. | Complete the table: client = recv, pipe, pass, hash, purge, miss, hit, deliver, synth, connect; backend = backend_fetch, backend_response, backend_error, backend_refresh; housekeeping = init, fini. |
| 66 + 58 + ~50 | `Reserved keyword`, `Enum value  not found` (empty value), `Not found` on `TRUE`/`FALSE`/`True` | Arguments to ENUM parameters (`vtc.workspace_alloc(backend, -1)`, `mon1.get_info(new)`, `set_flag(TRUE)`) are checked as identifiers. The reserved-keyword check blanks the text, so the enum lookup then fails with an empty value. | When the parameter type is ENUM, take the argument text verbatim and validate it against the enum values; skip identifier and keyword checks. |
| ~60 | `Not found` on builtin variables | Missing from `src/varnish_builtins.rs`: `bereq.time`, `resp.time`, `obj.time`, `req.esi`, `req.is_reset`, `obj.can_esi`, `obj.storage`, `resp.proto`, `beresp.storage` (typed), `storage.<name>.free_space/used_space/happy`. | Add them. `storage.<name>` needs a wildcard member. |
| 30 | `store.add_redis_server is not a method` (kv) | In `vmod_kv.vcc` a `$Restrict` line follows `$Object init(...)` directly. The vcc parser's object-method loop stops at the first part that is not `$Method`, so the object gets no methods at all. | In `parse_vcc`, skip `$Restrict` (and other non-method directives) between `$Object` and its `$Method`s. |
| 42 | `STRING cannot cast into BLOB` / `BOOL` | `req.hash` and `bereq.hash` are typed STRING but are BLOB. STRING to BOOL is legal for function arguments in VCL (header presence). | Fix the two types; allow STRING to BOOL in `can_this_cast_into`. |
| 36 | `Backend property «tls» does not exist` | Enterprise properties are absent from `BACKEND_FIELDS`: `tls`, `tls_sni`, `tls_verify_host`, `tls_verify_peer`, `ssl`, `ssl_sni`, `ssl_nosni`, `ssl_noverify`, `ssl_verify_host`, `ssl_verify_peer`, `certificate`, `wait_limit`, `wait_timeout`, `dscp`, `expect_close`, `tcponly`. | Add them (source: enterprise `lib/libvcc/vcc_backend.c`). |
| 10 | `«req» does not exist in «vcl_connect»` | Enterprise sub `vcl_connect` is unknown. Related: any custom sub whose name starts with `vcl_` (e.g. `vcl_vha_internal`) is treated as a builtin and gets builtin variable rules. | Add `vcl_connect`; apply builtin rules only to the real builtin sub names. |
| 10 | `Syntax error` | Remaining grammar gaps, mostly negative tests written outside `-errvcl` blocks or the enterprise `shell -err "msg" {cmd}` form. | Nothing needed. |
| 2 | `Extraneous argument` | `$Function STRING imported_name(debug.exportable)`: an argument typed as a vmod object type is dropped by the vcc parser. | Parse `<vmod>.<object>` argument types. |

## Expected errors

179 errors are inside `-errvcl` blocks and are correct. They still show up as
red errors in the editor. Suggestion: suppress diagnostics inside
`errvcl_argument` nodes, or downgrade them to hints.

## Already fixed while collecting this data

- vcc parser panics on `$Object NULL_OK ...` (akamai, debug, kv, mmdb, stat)
  silently dropped the whole vmod; the parser is now panic free.
- `$Restrict` swallowed the function documentation.
- vcc files are now found recursively below `vcc_paths`, so
  `lib/varnish-modules/vmod_cookie.vcc` etc. resolve (cookie, var, tcp,
  vsthrottle, header, saintmode, xkey, bodyaccess: 259 errors).
- VCL strings have no escape sequences; `"\\"` mis-tokenized whole files.
- The enclosing sub for diagnostics is now found through the ancestors, so a
  parse error elsewhere no longer produces bogus "cannot be called from" errors.
- Relative include paths no longer panic.

## Reproducing

```sh
# from the varnish-cache-plus root, with vcc_paths = ["./lib"] in .varnishls.toml
find bin/varnishtest -name '*.vtc' | xargs -P 8 -I{} sh -c \
  'varnishls lint --level hint {} > /tmp/lint/$(echo {} | tr / _).txt 2>&1'
# -errvcl block ranges, to separate expected errors (from vendor/tree-sitter-vtc):
tree-sitter parse FILE | grep -o 'errvcl_argument \[.*\]'
```

## Results after the fixes (same run, same config)

| | Before | After |
|---|---|---|
| Files with unexpected errors | 491 | 128 |
| Unexpected errors | 2020 | 793 |
| ... of which need `./lib` in `vcl_paths` (included VCL) | 773 | 773 |
| ... other | 1247 | 20 |
| Errors shown inside `-errvcl` blocks | 179 | 0 |

The 20 remaining errors: 10 syntax errors in negative tests written outside
`-errvcl`, 8 `b.encode is not a method` in m00043.vtc where `backend b` in
the first `-vcl` block shadows `new b = blob.blob()` in a later block (needs
per-block scoping), and 2 `debug.fill_synth` restriction errors that look
genuine.

Done in this round: sub-to-context table (and custom `vcl_*` subs are no
longer treated as builtin), ENUM arguments, `-vcl+backend` generated
backends, `$Restrict` after `$Object`, unknown vcc argument types keep their
position, enterprise backend properties (booleans accept `true` and `1`),
missing builtin variables, BLOB hashes, STRING to BOOL, `storage.<name>.*`,
`param.<name>`, case insensitive `True`/`TRUE`, named argument names, the
first definition of a name wins across `-vcl` blocks, and no diagnostics
inside `-errvcl` blocks.

## Results after the second round

Per-`-vcl`-block scoping, `vcl_path` read from the vtc file itself, and the
defined-after-use check limited to one document. Same 2496 files, config is
only `vcc_paths = ["./lib"]`, no `vcl_paths`:

| | Count |
|---|---|
| Files with errors | 7 |
| Errors | 14 |

The 14: 10 syntax errors in negative tests written outside `-errvcl`,
2 `debug.fill_synth` restriction errors that look genuine, and 2 `math is
undefined` (there is no libvmod_math in the repo). Nothing is open from
the categorization.

## Suggested order of fixes (original)

1. Sub-to-context table (215 errors, one table).
2. ENUM arguments (~170 errors, one code path).
3. `-vcl+backend` generated backends (~600 errors).
4. `$Restrict` after `$Object` in the vcc parser (30 errors, tiny).
5. Backend properties, builtin variables, BLOB/BOOL types (~140 errors, data).
6. Per-`-vcl`-block scoping (253 errors, larger change).
7. Suppress diagnostics inside `-errvcl`.
8. Custom `vcl_*` subs and `vcl_connect`.

## Open source varnish-cache (trunk, 945 files)

Same method on `bin/varnishtest/tests` and `vmod/tests` of a varnish-cache
trunk checkout, config `vcc_paths = ["./vmod"]`.

| | Before | After |
|---|---|---|
| Files with unexpected errors | 33 | 6 |
| Unexpected errors | 142 | 28 |

Fixed: quoted header names (`req.http."a.b"`, `{"..."}`, `"""..."""`),
unary minus, trunk variables (`req.max_age`, `*.filters`,
`bereq.retry_connect`, `bereq.task_deadline`, `sess.*_timeout*`), `obj_stale`
in `vcl_backend_refresh`, `STRING.lower()/upper()` and `BACKEND.resolve()`,
`import x as y` (also when the same vmod is imported plainly too),
`call vmod.function();`, vcc argument names with a C name (`bool:boolean`),
and BLOB literals as backend property values.

The 29 remaining: 24 in `blob_b00011.vtc`, which includes a VCL file it
first writes to `${tmpdir}`, so the include cannot be resolved statically;
and 4 syntax errors in negative tests written outside `-errvcl`.
(`objx.bar`, defined with `$Alias .bar obj.foo`, is now supported too.)

Enterprise suite after these changes: 7 files, 10 errors (4 negative-test
syntax errors, 2 genuine restriction errors, 2 `math` which has no vcc in
the repo, 2 `Not found` in a test that only now parses and uses an
enterprise variable, since added).
