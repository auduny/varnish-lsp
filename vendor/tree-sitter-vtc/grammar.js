const vclGrammar = require('../tree-sitter-vcl/grammar.js');

/**
 * Grammar for varnishtest / vtest2 test cases.
 *
 * SEE: https://varnish-cache.org/docs/trunk/reference/vtc.html
 * SEE: https://github.com/vtest/VTest2/blob/master/src/cmds.h
 *
 * The vtc tokenizer (vtc.c parse_string) is line oriented: a command runs to
 * the end of the line unless the line ends with a backslash, and its
 * arguments are whitespace separated words, "quoted strings" or {brace
 * balanced blocks}. This grammar mirrors that: newlines terminate statements
 * (they are skipped as whitespace anywhere else, e.g. inside embedded VCL).
 */

// bin/varnishtest/vtc_http.c CMD_HTTP(...)
const HTTP_COMMANDS = [
  'accept',
  'close',
  'send_n',
  'send_urgent',
  'sendhex',
  'shutdown',
  'timeout',
  'fatal',
  'non_fatal',
  'gunzip',
  'write_body',
  'chunked',
  'chunkedlen',
  'rxchunk',
  'settings',
  'rxresp',
  'rxrespbody',
  'rxresphdrs',
  'txpri',
  'txreq',
  'rxpri',
  'rxreq',
  'rxreqbody',
  'rxreqhdrs',
  'txresp',
  'upgrade',
  'expect_close',
  'expect_pattern',
  // Varnish Enterprise
  'reset',
  'unbrotli',
  'tls_handshake',
  'tls_renegotiate',
  'tls_key_update',
];

// bin/varnishtest/vtc_http2.c CMD_STREAM(...)
const STREAM_COMMANDS = [
  'rxcont',
  'rxdata',
  'rxframe',
  'rxgoaway',
  'rxhdrs',
  'rxping',
  'rxprio',
  'rxpush',
  'rxrst',
  'rxsettings',
  'rxwinup',
  'txcont',
  'txdata',
  'txgoaway',
  'txping',
  'txprio',
  'txpush',
  'txrst',
  'txsettings',
  'txwinup',
];

const BLOCK_COMMANDS = [...new Set([...HTTP_COMMANDS, ...STREAM_COMMANDS])];

const NL = choice('\n', '\r\n');

// newline separated statements, blank lines allowed, last line may be
// unterminated (end of file or directly followed by «}»)
function lines(stmt) {
  return seq(repeat(choice(NL, seq(stmt, NL))), optional(stmt));
}

function commandBlock(stmt) {
  return seq('{', lines(stmt), '}');
}

module.exports = grammar(vclGrammar, {
  name: 'vtc',

  // line continuation gets precedence so a trailing «\» is never a word
  extras: $ => [/\s/, $.COMMENT, $.inline_c, token(prec(1, /\\\r?\n/))],

  rules: {
    source_file: $ => lines($.vtc_statement),
    source_file_vcl: $ => repeat1($.toplev_declaration),

    // vtest2 accepts «delay .5»
    number: () => /-?(\d+(\.\d+)?|\.\d+)/,

    // bin/varnishtest/cmds.h CMD_GLOBAL(...) + CMD_TOP(...)
    vtc_statement: $ =>
      choice(
        $.varnishtest,
        $.server,
        $.client,
        $.varnish,
        $.barrier,
        $.haproxy,
        $.syslog,
        $.delay,
        $.shell,
        $.setenv,
        $.logexpect,
        $.feature_statement,
        $.tunnel_block,
        $.process,
        $.filewrite,
        $.include_statement,
        $.vsm,
        $.loop_statement,
        // Varnish Enterprise
        $.file,
        $.dns,
        $.iexpr,
        $.unsetenv,
        $.define,
      ),

    /*
     * argument values
     */

    // «-option»; defined before «word» so it wins ties in states where both are valid
    argument_ident: () => /-[A-Za-z0-9_+-]+/,
    // a bare word, may contain ${macros}
    word: () => /([^\s"{}$\\]|\\[^\s]|\$(\{[^}]*\})?)+/,
    // vtc strings cannot span lines, and «{"» must not start a VCL long string
    _vtc_string: () => token(seq('"', /([^"\\\n$]|\\.|\$(\{[^}]*\})?)*/, '"')),
    // brace balanced block of raw text (shell script, body, ...)
    block_argument_value: $ =>
      seq('{', repeat(choice($._raw_text, $.block_argument_value)), '}'),
    _raw_text: () => token(prec(2, /[^{}]+/)),

    _value: $ => choice($.word, alias($._vtc_string, $.string), $.block_argument_value),
    _value_no_block: $ => choice($.word, alias($._vtc_string, $.string)),

    // generic «-option [values...]»
    argument: $ => prec.right(seq($.argument_ident, repeat($._value))),
    // for commands where a «{» after the options starts a command block
    argument_no_block: $ =>
      prec.right(seq($.argument_ident, repeat($._value_no_block))),

    /*
     * top level commands
     */

    varnishtest: $ => seq(choice('varnishtest', 'vtest'), $._value),

    server: $ =>
      seq(
        'server',
        field('ident', $.ident),
        repeat($.argument_no_block),
        optional(seq(field('block', $._http_spec), repeat($.argument))),
      ),
    client: $ =>
      seq(
        'client',
        field('ident', $.ident),
        repeat($.argument_no_block),
        optional(seq(field('block', $._http_spec), repeat($.argument))),
      ),
    _http_spec: $ => choice($.http_block, alias($._vtc_string, $.string)),
    http_block: $ => commandBlock($.vtc_block_statement),

    varnish: $ =>
      seq(
        'varnish',
        field('ident', $.ident),
        // bare words allow macro-defined arguments, e.g. «varnish v1 ${m2_arg} ${m2_vcl}»
        repeat(choice($.vcl_argument, $.errvcl_argument, $.argument, $.word)),
      ),
    vcl_argument: $ =>
      seq(choice('-vcl', '-vcl+backend'), $._vcl_spec),
    // -errvcl "expected error" { vcl }
    errvcl_argument: $ =>
      seq('-errvcl', field('message', $._value), $._vcl_spec),
    _vcl_spec: $ =>
      choice(
        seq('{', optional($.source_file_vcl), '}'),
        alias($._vtc_string, $.string),
      ),

    barrier: $ =>
      seq(
        'barrier',
        field('ident', $.ident),
        choice('cond', 'sock', 'sync'),
        optional($.number),
        optional('-cyclic'),
      ),

    haproxy: $ =>
      seq(
        'haproxy',
        field('ident', $.ident),
        repeat(choice($.haproxy_cli_argument, $.argument)),
      ),
    haproxy_cli_argument: $ =>
      seq(
        choice('-cli', '-mcli'),
        commandBlock(choice($.send, $.expect_pattern_only, $.delay, $.barrier, $.shell)),
      ),

    syslog: $ =>
      seq(
        'syslog',
        field('ident', $.ident),
        repeat($.argument_no_block),
        optional(seq(field('block', $.syslog_block), repeat($.argument))),
      ),
    syslog_block: $ =>
      commandBlock(choice($.recv, $.expect_pattern_only, $.delay, $.barrier, $.shell)),

    delay: $ => seq('delay', choice($.number, $.word)),
    shell: $ => seq('shell', repeat($.shell_option), $._value, repeat($.argument)),
    shell_option: $ =>
      choice(
        '-err',
        seq('-exit', $._value_no_block),
        seq(choice('-expect', '-match'), $._value),
      ),
    setenv: $ => seq('setenv', optional('-ifunset'), field('name', $.word), $._value_no_block),
    process: $ =>
      seq(
        'process',
        field('ident', $.ident),
        optional(choice($.block_argument_value, alias($._vtc_string, $.string))),
        repeat($.argument),
      ),
    filewrite: $ => seq('filewrite', optional('-a'), field('path', $._value_no_block), repeat($._value)),
    include_statement: $ => seq('include', $._value_no_block),
    vsm: $ => seq('vsm', field('ident', $.ident), repeat($.argument)),
    feature_statement: $ => seq('feature', repeat1($._value)),

    // Varnish Enterprise commands
    file: $ => seq('file', field('ident', $.ident), repeat($.argument)),
    dns: $ => seq('dns', repeat(choice($._value, $.argument))),
    iexpr: $ => seq('iexpr', field('name', $.word), repeat1($._value)),
    unsetenv: $ => seq('unsetenv', field('name', $.word)),
    // define <macro> <name> <value>
    define: $ => seq('define', field('macro', $.word), field('name', $.word), $._value),

    // ${macro} used directly inside embedded VCL, e.g. «== ${s1_port}»
    literal: ($, prev) => choice(prev, $.macro),
    macro: () => /\$\{[^}]*\}/,

    loop_statement: $ =>
      seq('loop', choice($.number, $.word), field('block', choice(commandBlock($.vtc_statement), $.word))),

    tunnel_block: $ =>
      seq(
        'tunnel',
        field('ident', $.ident),
        repeat($.argument_no_block),
        optional(seq(field('block', $.tunnel_body), repeat($.argument))),
      ),
    tunnel_body: $ =>
      commandBlock(choice($.tunnel_command, $.send, $.recv, $.delay, $.barrier, $.shell)),
    tunnel_command: () => choice('pause', 'resume'),

    logexpect: $ =>
      seq(
        'logexpect',
        field('ident', $.ident),
        repeat(choice($.logexpect_query, $.argument_no_block)),
        optional(seq(field('block', $.logexpect_block), repeat($.argument))),
      ),
    // -q may take a {block}
    logexpect_query: $ => seq('-q', $._value),
    logexpect_block: $ =>
      commandBlock(
        choice($.logexpect_expect, $.logexpect_fail, $.logexpect_abort, $.logexpect_loop),
      ),
    // expect <skip> <vxid> <tag> [regex]
    logexpect_expect: $ => seq('expect', repeat1($._value)),
    logexpect_fail: $ => seq('fail', choice('clear', seq('add', repeat1($._value)))),
    logexpect_abort: () => 'abort',
    logexpect_loop: $ =>
      seq('loop', choice($.number, $.word), choice(commandBlock(
        choice($.logexpect_expect, $.logexpect_fail, $.logexpect_abort, $.logexpect_loop),
      ), $.word)),

    /*
     * commands inside server/client/stream blocks
     */

    vtc_block_statement: $ =>
      choice(
        $.vtc_block_statement_with_arguments,
        $.expect,
        $.send,
        $.recv,
        $.barrier,
        $.delay,
        $.shell,
        $.include_statement,
        $.loop_block_statement,
        $.stream_block,
        $.tls_config,
      ),
    vtc_block_statement_with_arguments: $ =>
      prec.right(seq(choice(...BLOCK_COMMANDS), repeat(choice($.argument, $._value)))),

    send: $ => seq('send', $._value),
    // syslog «recv» takes no argument
    recv: $ => seq('recv', optional($._value)),

    // expect <var> <op> <value>
    expect: $ =>
      seq(
        'expect',
        field('left', $.expect_value),
        field('operator', $.expect_operator),
        field('right', $.expect_value),
      ),
    // syslog / haproxy cli: expect <op> <value>
    expect_pattern_only: $ =>
      seq('expect', field('operator', $.expect_operator), field('right', $.expect_value)),
    // bin/varnishtest/vtc_subr.c vtc_expect()
    expect_operator: () =>
      choice('==', '!=', '~', '!~', '<', '<=', '>', '>=', '-eq', '-ne', '-lt', '-le', '-gt', '-ge'),
    expect_value: $ =>
      choice('<undef>', $.word, alias($._vtc_string, $.string), $.block_argument_value),

    loop_block_statement: $ =>
      seq('loop', choice($.number, $.word), field('block', choice(commandBlock($.vtc_block_statement), $.word))),

    // h2: stream <id> { ... } -run
    stream_block: $ =>
      seq(
        'stream',
        choice($.number, $.word),
        repeat($.argument_no_block),
        optional(
          seq(
            field('block', choice(commandBlock($.vtc_block_statement), $.word)),
            repeat($.argument),
          ),
        ),
      ),

    // Varnish Enterprise: tls_config { key = value ... }
    tls_config: $ =>
      seq('tls_config', commandBlock($.tls_config_statement), repeat($.argument)),
    tls_config_statement: $ =>
      seq(field('key', $.word), '=', repeat1($._value_no_block)),
  },
});
