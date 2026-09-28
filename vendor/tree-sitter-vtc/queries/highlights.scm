; inherits: vcl

; top level commands (vtest2 cmds.h)
[
  "varnishtest"
  "vtest"
  "varnish"
  "client"
  "server"
  "shell"
  "process"
  "setenv"
  "unsetenv"
  "loop"
  "delay"
  "feature"
  "barrier"
  "haproxy"
  "syslog"
  "logexpect"
  "tunnel"
  "filewrite"
  "include"
  "vsm"
  "file"
  "dns"
  "iexpr"
  "define"
] @keyword

; commands inside blocks
[
  "expect"
  "send"
  "recv"
  "stream"
  "tls_config"
  "fail"
  "add"
  "clear"
  "cond"
  "sock"
  "sync"
] @keyword

(vtc_block_statement_with_arguments . _ @keyword)
(tunnel_command) @keyword
(logexpect_abort) @keyword

"<undef>" @constant
(expect_operator) @operator
(macro) @variable

[
  "-vcl"
  "-vcl+backend"
  "-errvcl"
  "-cli"
  "-mcli"
  "-q"
  "-err"
  "-exit"
  "-expect"
  "-match"
  "-ifunset"
  "-cyclic"
  "-a"
] @parameter

(argument_ident) @parameter
