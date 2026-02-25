// Name conversion utilities for codegen.

/// Convert a PascalCase or UPPER_CASE name to snake_case.
pub fn to_snake_case(name: &str) -> String {
    let mut result = String::with_capacity(name.len() + 8);
    let chars: Vec<char> = name.chars().collect();

    for (i, &ch) in chars.iter().enumerate() {
        if ch.is_ascii_uppercase() {
            if i > 0 {
                let prev = chars[i - 1];
                // Insert underscore before uppercase if preceded by lowercase/digit,
                // or if it starts a new word in an acronym (e.g., "HTTPServer" -> "http_server").
                if prev.is_ascii_lowercase() || prev.is_ascii_digit() {
                    result.push('_');
                } else if prev.is_ascii_uppercase()
                    && i + 1 < chars.len()
                    && chars[i + 1].is_ascii_lowercase()
                {
                    result.push('_');
                }
            }
            result.push(ch.to_ascii_lowercase());
        } else {
            result.push(ch);
        }
    }

    result
}

/// Strip the 'b' prefix from boolean property names (e.g., "bNetTemporary" -> "net_temporary").
pub fn strip_bool_prefix(name: &str) -> String {
    if name.starts_with('b') && name.len() > 1 && name.as_bytes()[1].is_ascii_uppercase() {
        to_snake_case(&name[1..])
    } else {
        to_snake_case(name)
    }
}

const RESERVED_WORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false",
    "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
    "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super",
    "trait", "true", "type", "unsafe", "use", "where", "while", "async",
    "await", "dyn", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
];

/// Check if a name is a Rust reserved word.
pub fn is_reserved(name: &str) -> bool {
    RESERVED_WORDS.contains(&name)
}

/// Escape Rust reserved words by prepending `r#`.
pub fn escape_reserved(name: &str) -> String {
    if is_reserved(name) {
        format!("r#{name}")
    } else {
        name.to_string()
    }
}

/// Strip the UE prefix from a class name (A for actors, U for objects).
/// The JSON `name` field typically already has this stripped, but cpp_name doesn't.
#[allow(dead_code)]
pub fn strip_ue_prefix(cpp_name: &str) -> &str {
    if cpp_name.len() > 1 {
        let first = cpp_name.as_bytes()[0];
        let second = cpp_name.as_bytes()[1];
        if (first == b'A' || first == b'U') && second.is_ascii_uppercase() {
            return &cpp_name[1..];
        }
    }
    cpp_name
}

/// Strip the F prefix from struct names.
#[allow(dead_code)]
pub fn strip_struct_prefix(cpp_name: &str) -> &str {
    if cpp_name.len() > 1 && cpp_name.as_bytes()[0] == b'F' && cpp_name.as_bytes()[1].is_ascii_uppercase() {
        &cpp_name[1..]
    } else {
        cpp_name
    }
}

/// Convert a UE module/package name to a Rust module name.
#[allow(dead_code)]
pub fn to_module_name(name: &str) -> String {
    // Already snake_case or lowercase is common for module names
    let snake = to_snake_case(name);
    escape_reserved(&snake)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("AActor"), "a_actor");
        assert_eq!(to_snake_case("GetObjectCount"), "get_object_count");
        assert_eq!(to_snake_case("bNetTemporary"), "b_net_temporary");
        assert_eq!(to_snake_case("HTTPServer"), "http_server");
        assert_eq!(to_snake_case("FVector"), "f_vector");
        assert_eq!(to_snake_case("ECollisionChannel"), "e_collision_channel");
        assert_eq!(to_snake_case("URL"), "url");
        assert_eq!(to_snake_case("K2_GetActorLocation"), "k2_get_actor_location");
    }

    #[test]
    fn test_strip_bool_prefix() {
        assert_eq!(strip_bool_prefix("bNetTemporary"), "net_temporary");
        assert_eq!(strip_bool_prefix("bHidden"), "hidden");
        assert_eq!(strip_bool_prefix("boolean"), "boolean"); // no strip
    }

    #[test]
    fn test_escape_reserved() {
        assert_eq!(escape_reserved("type"), "r#type");
        assert_eq!(escape_reserved("r#move"), "r#move"); // already escaped? no — "r#move" is not a keyword
        assert_eq!(escape_reserved("move"), "r#move");
        assert_eq!(escape_reserved("actor"), "actor");
    }

    #[test]
    fn test_strip_ue_prefix() {
        assert_eq!(strip_ue_prefix("AActor"), "Actor");
        assert_eq!(strip_ue_prefix("UObject"), "Object");
        assert_eq!(strip_ue_prefix("FVector"), "FVector"); // F is not stripped here
    }
}
