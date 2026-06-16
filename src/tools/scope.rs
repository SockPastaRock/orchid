use std::env;
use std::path::Path;

pub fn expand_path(s: &str, working_dir: &str) -> String {
    if s.starts_with("~") {
        let home = env::var("HOME")
            .ok()
            .or_else(|| dirs::home_dir().map(|p| p.to_string_lossy().to_string()));

        if let Some(home_str) = home {
            if s == "~" {
                home_str
            } else if let Some(rest) = s.strip_prefix("~/") {
                format!("{}/{}", home_str, rest)
            } else {
                s.to_string()
            }
        } else {
            s.to_string()
        }
    } else if s.contains("${") || s.contains("$") {
        expand_vars(s)
    } else if s.starts_with("/") {
        s.to_string()
    } else {
        format!("{}/{}", working_dir, s)
    }
}

fn expand_vars(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' {
            if chars.peek() == Some(&'{') {
                chars.next();
                let mut var_name = String::new();
                for c in chars.by_ref() {
                    if c == '}' {
                        break;
                    }
                    var_name.push(c);
                }
                match env::var(&var_name) {
                    Ok(val) => result.push_str(&val),
                    Err(_) => result.push_str(&format!("${{{}}}", var_name)),
                }
            } else {
                let mut var_name = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' {
                        var_name.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if !var_name.is_empty() {
                    match env::var(&var_name) {
                        Ok(val) => result.push_str(&val),
                        Err(_) => result.push_str(&format!("${}", var_name)),
                    }
                } else {
                    result.push('$');
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}

pub fn is_in_scope(path: &str, working_dir: &str) -> bool {
    if path.starts_with("/tmp") || path.starts_with("/var/folders") {
        return true;
    }

    let expanded = expand_path(path, working_dir);
    let canonical_path = normalize_path(&expanded);
    let canonical_working = normalize_path(working_dir);

    // Guard against mismatches caused by partial symlink resolution (e.g. macOS
    // /tmp -> /private/tmp): if the working dir resolved through a symlink we
    // also accept paths that start with the *unresolved* working dir string.
    canonical_path.starts_with(&canonical_working) || expanded.starts_with(working_dir)
}

fn normalize_path(p: &str) -> String {
    let path = Path::new(p);
    match path.canonicalize() {
        Ok(canonical) => canonical.to_string_lossy().to_string(),
        Err(_) => {
            let mut normalized = String::new();
            for component in path.components() {
                use std::path::Component;
                match component {
                    Component::ParentDir => {
                        if let Some(last_slash) = normalized.rfind('/') {
                            normalized.truncate(last_slash);
                        }
                    }
                    Component::CurDir => {}
                    Component::Normal(os_str) => {
                        if !normalized.is_empty() && !normalized.ends_with('/') {
                            normalized.push('/');
                        }
                        normalized.push_str(&os_str.to_string_lossy());
                    }
                    Component::RootDir => {
                        normalized.push('/');
                    }
                    Component::Prefix(_) => {}
                }
            }
            if normalized.is_empty() {
                "/".to_string()
            } else {
                normalized
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_path_absolute() {
        let result = expand_path("/tmp/test", "/home/user");
        assert_eq!(result, "/tmp/test");
    }

    #[test]
    fn test_expand_path_relative() {
        let result = expand_path("file.txt", "/home/user/work");
        assert_eq!(result, "/home/user/work/file.txt");
    }

    #[test]
    fn test_is_in_scope_absolute() {
        assert!(is_in_scope("/tmp/test", "/tmp"));
    }

    #[test]
    fn test_is_in_scope_tmp() {
        assert!(is_in_scope("/tmp/any/path", "/home/user"));
    }

    #[test]
    fn test_expand_vars_simple() {
        env::set_var("TEST_VAR", "value");
        let result = expand_vars("prefix_$TEST_VAR");
        assert_eq!(result, "prefix_value");
    }
}
