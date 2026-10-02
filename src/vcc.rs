use std::iter::{Iterator, Peekable};
use std::str::SplitTerminator;

use crate::varnish_builtins::*;

// https://github.com/varnishcache/varnish-cache/blob/a3bc025c2df28e4a76e10c2c41217c9864e9963b/lib/libvcc/vmodtool.py#L890
/*
fn tokenize<'a>(txt: &'a str, seps: Option<&str>, quotes: Option<&str>) -> Vec<&'a str> {
    let mut seps_str = String::from("[](){},=");
    if let Some(s) = seps {
        seps_str = s.to_string();
    }
    let mut quotes_str = String::from("'\"");
    if let Some(q) = quotes {
        quotes_str = q.to_string();
    }

    let mut quote: Option<char> = None;
    let mut token_start = 0;
    let mut out: Vec<&str> = Vec::new();
    let mut inside = false;

    for (i, c) in txt.char_indices() {
        if let Some(q) = quote {
            if c == q {
                inside = false;
                quote = None;
                out.push(&txt[token_start..(i + 1)]);
            }
        } else if c.is_whitespace() {
            if inside {
                out.push(&txt[token_start..i]);
            }
            inside = false;
        } else if seps_str.contains(c) {
            if inside {
                out.push(&txt[token_start..i]);
            }
            inside = false;
            out.push(&txt[i..(i + 1)]);
        } else if quotes_str.contains(c) {
            if inside {
                out.push(&txt[token_start..i]);
            }
            quote = Some(c);
            inside = true;
            token_start = i;
        } else if !inside {
            token_start = i;
            inside = true;
        }
    }

    if inside {
        out.push(&txt[token_start..txt.len()]);
    }

    out
}
*/

struct Tokenizer<'a> {
    cursor: usize,
    txt: &'a str,
    seps_str: String,
    quotes_str: String,
}

impl<'a> Tokenizer<'a> {
    fn new(txt: &'a str) -> Self {
        Self {
            cursor: 0,
            txt,
            seps_str: String::from("[](){},="),
            quotes_str: String::from("'\""),
        }
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = &'a str;

    // https://github.com/varnishcache/varnish-cache/blob/a3bc025c2df28e4a76e10c2c41217c9864e9963b/lib/libvcc/vmodtool.py#L890
    fn next(&mut self) -> Option<Self::Item> {
        let mut quote: Option<char> = None;
        let mut token_start = 0;
        let mut inside = false;
        let slice = &self.txt[self.cursor..];

        macro_rules! emit_token {
            ($value:expr) => {
                Some(&slice[token_start..$value])
            }
        }

        for (i, c) in slice.char_indices() {
            self.cursor += 1;
            if let Some(q) = quote {
                if c == q {
                    return emit_token!(i + 1);
                }
            } else if c.is_whitespace() {
                if inside {
                    return emit_token!(i);
                }
            } else if self.seps_str.contains(c) {
                if inside {
                    self.cursor -= 1;
                    return emit_token!(i);
                }
                return Some(&slice[i..(i + 1)]);
            } else if self.quotes_str.contains(c) {
                if inside {
                    return emit_token!(i);
                }
                quote = Some(c);
                inside = true;
                token_start = i;
            } else if !inside {
                token_start = i;
                inside = true;
            }
        }

        emit_token!(slice.len()).filter(|s| !s.is_empty())
    }
}

fn parse_doc(lines_iter: &mut SplitTerminator<&str>) -> String {
    lines_iter
        .take_while(|double_line| {
            // stop if this chunk is a doc header (e.g. https://github.com/varnishcache/varnish-cache/blob/a3bc025c2df28e4a76e10c2c41217c9864e9963b/lib/libvmod_std/vmod.vcc#L441-L442)
            double_line.lines().all(|line| {
                line.is_empty()
                    || line
                        .chars()
                        .fold(0, |count, char| count + (char == '=') as usize)
                        != line.len()
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn parse_type<'a>(toks: &mut Peekable<impl Iterator<Item = &'a str>>) -> Option<Type> {
    let type_str = toks.next()?;
    match type_str {
        "STRING" => Some(Type::String),
        "STRING_LIST" => Some(Type::String),
        "REGEX" => Some(Type::String),
        "STRANDS" => Some(Type::String),
        "BOOL" => Some(Type::Bool),
        "INT" => Some(Type::Number),
        "REAL" => Some(Type::Number),
        "IP" => Some(Type::IP),
        "DURATION" => Some(Type::Duration),
        "TIME" => Some(Type::Time),
        "BYTES" => Some(Type::Bytes), 
        "BLOB" => Some(Type::Blob),
        "BACKEND" => Some(Type::Backend),
        "PROBE" => Some(Type::Probe),
        "ACL" => Some(Type::Acl),
        "HTTP" => Some(Type::Obj(Default::default())), // for now
        "HEADER" => Some(Type::String),
        "ENUM" => {
            let mut values = vec![];

            if toks.next_if(|tok| *tok == "{").is_none() {
                // unknown ENUM form, treat as a plain enum without known values
                return Some(Type::Enum(values));
            }
            while let Some(tok) = toks.next() {
                if tok == "}" {
                    break;
                }
                values.push(tok.to_string());
                match toks.next() {
                    Some(",") => continue,
                    _ => break, // «}» or unexpected token / end of enums
                }
            }
            Some(Type::Enum(values))
        }
        "VOID" | "PRIV_TASK" | "PRIV_VCL" | "PRIV_CALL" | "STEVEDORE" => None,
        _ => {
            // todo!("NOT IMPLEMENTED: {type_str:?}");
            None
        }
    }
}

fn parse_func<'a>(
    toks: &mut Peekable<impl Iterator<Item = &'a str>>,
    lines_iter: &mut SplitTerminator<&str>,
) -> Option<Func> {
    let r#return = parse_type(toks).map(Box::new);

    let name = toks.next()?.trim_start_matches('.').to_string();
    if toks.next() != Some("(") {
        return None; // expected arguments
    }

    let args = parse_func_args(toks);
    let doc = parse_doc(lines_iter);

    Some(Func {
        name,
        r#return,
        args,
        doc: Some(doc),
        ..Default::default()
    })
}

fn parse_func_args<'a>(toks: &mut Peekable<impl Iterator<Item = &'a str>>) -> Vec<FuncArg> {
    let mut arg_toks = toks.take_while(|tok| *tok != ")").peekable();
    let mut args: Vec<FuncArg> = vec![];
    while arg_toks.peek().is_some() {
        let optional = arg_toks.next_if(|tok| *tok == "[").is_some();
        // PRIV_* arguments are provided by varnish, not by the vcl caller
        let is_priv = arg_toks.peek().is_some_and(|tok| tok.starts_with("PRIV_"));
        let r#type = parse_type(&mut arg_toks);
        // «name:cname» gives the C name after the colon (e.g. «BOOL bool:boolean»)
        let name = arg_toks
            .next_if(|tok| *tok != ",")
            .map(|tok| tok.split(':').next().unwrap_or(tok).to_string());

        let default_value: Option<String> = arg_toks
            .next_if(|tok| *tok == "=")
            .map(|_| arg_toks.next().unwrap().to_string());

        // an argument of an unknown type (e.g. a vmod object type like «debug.exportable»)
        // still takes a position; it is just not type checked
        if !is_priv {
            args.push(FuncArg {
                name,
                optional,
                r#type,
                default_value,
            });
        }

        if optional && arg_toks.next() != Some("]") {
            break; // expected end of optional argument
        }

        match arg_toks.peek() {
            Some(&",") => {
                arg_toks.next();
                continue;
            }
            _ => break, // «)», end of tokens, or something we do not understand
        }
    }

    args
}

/**
 * Parse a «$Restrict» directive following a function or method. Returns the
 * restrictions and the documentation that follows the directive (the vcc
 * documentation of a restricted function comes after its «$Restrict» line).
 */
fn check_restrict<'a>(
    parts: &mut Peekable<impl Iterator<Item = &'a str>>,
) -> Option<(Vec<String>, String)> {
    let next_part = parts.peek()?;
    if next_part.starts_with("Restrict ") {
        let part = parts.next().unwrap();
        let mut lines_iter = part.split_terminator("\n\n");
        let first = lines_iter.next().unwrap_or(part);
        let line = first.lines().next().unwrap_or(first);
        let restricted = line["Restrict ".len()..]
            .split_whitespace()
            .map(|str| str.to_string())
            .collect();
        Some((restricted, parse_doc(&mut lines_iter)))
    } else {
        None
    }
}

fn apply_restrict<'a>(func: &mut Func, parts: &mut Peekable<impl Iterator<Item = &'a str>>) {
    if let Some((restricted, doc)) = check_restrict(parts) {
        func.restricted = Some(restricted);
        if func.doc.as_deref().is_none_or(str::is_empty) {
            func.doc = Some(doc);
        }
    }
}

/**
 * Parse a vmod vcc file, using the varnish vmodtool.py tokenizer, and collect all functions,
 * objects and object methods.
 *
 * This includes the documentation for the function/method.
 */
pub fn parse_vcc(vcc_file: String) -> Type {
    // split file by newline+dollar sign
    let vcc_file_with_starting_newline = format!("\n{vcc_file}");
    let mut parts = vcc_file_with_starting_newline.split("\n$").peekable();
    parts.next(); // remove first part

    let mut scope = Obj {
        read_only: true,
        ..Default::default()
    };
    // «$Alias .bar obj.foo» / «$Alias newname oldname», applied once everything is parsed
    let mut aliases: Vec<(String, String)> = vec![];

    // for each part, split by double newline (statements can span multiple lines)
    while let Some(s) = parts.next() {
        let mut lines_iter = s.split_terminator("\n\n");
        let first_line = lines_iter.next().expect("No lines");
        // let tok_vec = tokenize(first_line, None, None);
        // let mut toks = tok_vec.iter().peekable().map(|tok| *tok);
        let mut toks = Tokenizer::new(first_line).peekable();
        match toks.next().expect("No tokens") {
            "Module" => {
                scope.name = toks.next().unwrap().to_string();
            }
            "ABI" => {}
            "Restrict" => {}
            "Alias" => {
                if let (Some(alias), Some(target)) = (toks.next(), toks.next()) {
                    aliases.push((alias.to_string(), target.to_string()));
                }
            }
            "Function" => {
                let Some(mut func) = parse_func(&mut toks, &mut lines_iter) else {
                    continue;
                };
                apply_restrict(&mut func, &mut parts);
                scope
                    .properties
                    .insert(func.name.to_owned(), Type::Func(func));
            }
            "Object" => {
                // $Object [NULL_OK] [EXPORT] name(args), flags come before the name
                let mut name = None;
                while let Some(tok) = toks.next() {
                    if toks.peek() == Some(&"(") {
                        name = Some(tok);
                        break;
                    }
                }
                let (Some(name), Some("(")) = (name, toks.next()) else {
                    continue; // could not find «name(»
                };
                let args = parse_func_args(&mut toks);
                let mut doc = parse_doc(&mut lines_iter);
                let mut obj = Obj {
                    name: name.to_string(),
                    ..Default::default()
                };
                // take all following methods and put into obj
                while let Some(ps) = parts.peek() {
                    let mut lines_iter = ps.split_terminator("\n\n");
                    let first_line = lines_iter.next().unwrap();
                    // let tok_vec = tokenize(first_line, None, None);
                    // let mut toks = tok_vec.iter().peekable().map(|tok| *tok);
                    let mut toks = Tokenizer::new(first_line).peekable();

                    match toks.next() {
                        Some("Method") => {}
                        // «$Restrict» right after «$Object» applies to the constructor
                        Some("Restrict") => {
                            let part = parts.next().unwrap();
                            let mut restrict_lines = part.split_terminator("\n\n");
                            restrict_lines.next();
                            if doc.is_empty() {
                                doc = parse_doc(&mut restrict_lines);
                            }
                            continue;
                        }
                        _ => break,
                    }

                    parts.next();
                    let Some(mut func) = parse_func(&mut toks, &mut lines_iter) else {
                        continue;
                    };
                    apply_restrict(&mut func, &mut parts);
                    obj.properties
                        .insert(func.name.to_owned(), Type::Func(func));
                }
                let func = Func {
                    name: name.to_string(),
                    doc: Some(doc),
                    args,
                    r#return: Some(Box::new(Type::Obj(obj))),
                    ..Default::default()
                };
                scope.properties.insert(name.to_string(), Type::Func(func));
            }
            _ => {}
        }
    }

    for (alias, target) in aliases {
        if let Some(method) = alias.strip_prefix('.') {
            // method alias: «$Alias .bar obj.foo»
            let Some((obj_name, target_method)) = target.split_once('.') else {
                continue;
            };
            let Some(Type::Func(obj_func)) = scope.properties.get_mut(obj_name) else {
                continue;
            };
            let Some(Type::Obj(obj)) = obj_func.r#return.as_deref_mut() else {
                continue;
            };
            if let Some(func) = obj.properties.get(target_method).cloned() {
                obj.properties.insert(method.to_string(), func);
            }
        } else if let Some(func) = scope.properties.get(&target).cloned() {
            scope.properties.insert(alias, func);
        }
    }

    Type::Obj(scope)
}

pub fn parse_vcc_file_by_path(
    vcc_path: &std::path::Path,
) -> Result<Type, Box<dyn std::error::Error + Send + Sync>> {
    let vcc_file = std::fs::read_to_string(vcc_path)?;
    // parse_vcc should not panic, but a vcc file must never take vmod loading down
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| parse_vcc(vcc_file)))
        .map_err(|_| format!("panic while parsing {}", vcc_path.display()).into())
}

#[cfg(test)]
mod tests {
    use crate::vcc::*;

    #[test]
    fn test_tokenizer() {
        let toks =
            Tokenizer::new("Module directors 3 \"Varnish Directors Module\"").collect::<Vec<_>>();
        assert_eq!(
            toks,
            vec!["Module", "directors", "3", "\"Varnish Directors Module\""]
        );
    }

    #[test]
    fn test_tokenizer_seps() {
        let toks = Tokenizer::new("a=b[c,d]").collect::<Vec<_>>();
        assert_eq!(toks, vec!["a", "=", "b", "[", "c", ",", "d", "]"]);
    }

    #[test]
    fn test_parse_object_with_flags_and_restrict() {
        let src = "$Module stat 3 \"Stat\"\n\n$Function BACKEND backend_prometheus(PRIV_TASK, STRING filters = 0)\n$Restrict backend\n\nDocumentation about backend_prometheus.\n\n$Object NULL_OK event_observatory([STRING name], ENUM { skip, create } counter)\n$Restrict vcl_init\n\nObject doc.\n\n$Method VOID .increment(INT amount = 1)\n\nMethod doc.\n";
        let Type::Obj(scope) = parse_vcc(src.to_string()) else {
            panic!("expected object scope");
        };
        let Some(Type::Func(func)) = scope.properties.get("backend_prometheus") else {
            panic!("backend_prometheus missing");
        };
        assert_eq!(func.restricted, Some(vec!["backend".to_string()]));
        assert_eq!(func.doc.as_deref(), Some("Documentation about backend_prometheus."));
        let Some(Type::Func(obj_func)) = scope.properties.get("event_observatory") else {
            panic!("event_observatory missing");
        };
        let Some(Type::Obj(obj)) = obj_func.r#return.as_deref() else {
            panic!("expected object return type");
        };
        assert!(obj.properties.contains_key("increment"));
    }

    #[test]
    fn test_parse_alias() {
        let src = "$Module debug 3 \"Debug\"\n\n$Function STRING foo()\n\nDoc.\n\n$Object obj(STRING s)\n\nDoc.\n\n$Method STRING .foo(STRING why)\n\nDoc.\n\n$Alias bar foo\n\nDoc.\n\n$Alias .bar obj.foo\n\nDoc.\n";
        let Type::Obj(scope) = parse_vcc(src.to_string()) else {
            panic!("expected object scope");
        };
        assert!(matches!(scope.properties.get("bar"), Some(Type::Func(_))));
        let Some(Type::Func(obj_func)) = scope.properties.get("obj") else {
            panic!("obj missing");
        };
        let Some(Type::Obj(obj)) = obj_func.r#return.as_deref() else {
            panic!("expected object");
        };
        assert!(matches!(obj.properties.get("bar"), Some(Type::Func(_))));
    }

    #[test]
    fn test_parse_func() {
        let mut toks =
            Tokenizer::new("VOID barf(BOOL rainbow = false, ENUM { IN, OUT } direction = OUT)")
                .peekable();
        let doc = "doc doc doc";
        let func = parse_func(&mut toks, &mut doc.split_terminator("\n")).unwrap();
        assert_eq!(func.name, "barf");
        assert_eq!(func.args.len(), 2);
        assert_eq!(func.args[0].name, Some("rainbow".into()));
        assert_eq!(func.args[1].name, Some("direction".into()));
        if let Some(Type::Enum(ref enum_vals)) = func.args[1].r#type {
            assert_eq!(*enum_vals, vec!["IN".to_string(), "OUT".to_string()]);
        } else {
            panic!("arg 2 not enum");
        }
        assert_eq!(func.args[1].default_value, Some("OUT".into()));
    }

    #[test]
    fn test_restricted() {
        let Type::Obj(scope) = parse_vcc(
            r#"
$Function VOID my_func()
$Restrict vcl_pipe backend housekeeping
"#
            .to_string(),
        ) else {
            unreachable!()
        };
        let Type::Func(func) = scope.properties.get("my_func").unwrap() else {
            unreachable!()
        };
        assert_eq!(func.name, "my_func");
        assert_eq!(
            func.restricted,
            Some(vec![
                "vcl_pipe".to_string(),
                "backend".to_string(),
                "housekeeping".to_string()
            ])
        );
    }

    #[test]
    fn test_tokenizer_single_token() {
        let toks = Tokenizer::new("License").collect::<Vec<_>>();
        assert_eq!(toks, vec!["License"]);
    }
}
