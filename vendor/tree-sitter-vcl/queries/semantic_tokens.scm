; LSP semantic token queries

[
  "acl"
  "sub"
  "backend"
  "probe"
  "vcl"
  "else"
  "elsif"
  "elseif"
  "if"
  "return"
  "import"
  "include"
  "set"
  "unset"
  "new"
  "call"
  "as"
  "from"
] @keyword

[
  "hit"
  "miss"
  "pass"
  "pipe"
  "retry"
  "restart"
  "fail"
  "synth"
  "hash"
  "deliver"
  "abandon"
  "lookup"
  "ok"
  "purge"
  "fetch"
  "error"
  "connect"
  "merge"
  "beresp"
  "obj_stale"
] @keyword

[
  "=="
  "!="
  "~"
  "!~"
  ">"
  "<"
  ">="
  "<="
  "||"
  "&&"
  "+"
  "-"
  "*"
  "/"
  "%"
  "="
  "!"
] @operator

"." @operator
; [ "(" ")" ";"] @delimiter


(string) @string
(number) @number
; (float) @number
(duration) @number
(bytes) @number
(bool) @number
(blob) @string

(ident) @variable
(nested_ident) @property

(binary_expression
  operator: "~"
  right: (literal (string) @regexp (#offset! @regexp 0 1 0 -1)))

(binary_expression
  operator: "!~"
  right: (literal (string) @regexp (#offset! @regexp 0 1 0 -1)))


(ident_call_expr
  ident: [(nested_ident) (ident)] @function)

(ident_call_expr
  ident: (ident) @function.defaultLibrary (#match? @function.defaultLibrary "^(regsub|regsuball|hash_data|synthetic|ban)$"))

(func_call_named_arg
   arg_name: (ident) @parameter)

(sub_declaration ident: (ident) @function.declaration)
(sub_declaration ident: (ident) @function.defaultLibrary (#match? @function.defaultLibrary "^vcl_.*"))
(COMMENT) @comment
