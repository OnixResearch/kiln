//! Mapping from Tree-sitter capture names onto the fixed class vocabulary.

use crate::Class;

/// How one capture affects the class of the text it covers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Capture {
    /// The capture assigns this class.
    Class(Class),
    /// The capture explicitly resets nested text to plain (`@none`, `@embedded`).
    Plain,
    /// The capture has no class; the enclosing class still applies.
    Transparent,
}

/// Map a capture by its longest known dot-separated prefix.
///
/// `string.special.uri` tries `string.special.uri`, then `string.special`,
/// then `string`. A capture with no known prefix is transparent.
pub(crate) fn capture(name: &str) -> Capture {
    let mut candidate = name;
    loop {
        if let Some(found) = known(candidate) {
            return found;
        }
        match candidate.rsplit_once('.') {
            Some((prefix, _)) => candidate = prefix,
            None => return Capture::Transparent,
        }
    }
}

fn known(name: &str) -> Option<Capture> {
    use Class::*;
    let class = match name {
        "none" | "embedded" => return Some(Capture::Plain),
        "keyword" | "conditional" | "repeat" | "include" | "exception" | "storageclass"
        | "import" | "charset" | "keyframes" | "media" | "supports" | "namespace.keyword" => {
            Keyword
        }
        "string.special.uri" | "string.special.url" | "text.uri" | "text.reference"
        | "markup.link" | "link" => Link,
        "string.escape" | "escape" | "string.special.escape" => Escape,
        "string.special.key" | "property" | "field" | "variable.member" => Property,
        "string" | "character" | "text.literal" | "markup.raw" => String,
        "comment" => Comment,
        "number" | "float" | "constant.numeric" => Number,
        "constant" | "boolean" => Constant,
        "type" | "constructor" | "namespace" | "module" => Type,
        "function.macro" | "macro" => Macro,
        "function" | "method" => Function,
        "attribute" | "tag.attribute" | "decorator" => Attribute,
        "variable" | "parameter" | "label" => Variable,
        "operator" => Operator,
        "punctuation" | "delimiter" => Punctuation,
        "tag" => Tag,
        "text.title" | "markup.heading" | "heading" | "title" => Heading,
        _ => return None,
    };
    Some(Capture::Class(class))
}

#[cfg(test)]
mod tests {
    use super::{Capture, capture};
    use crate::Class;

    #[test]
    fn captures_map_by_longest_known_prefix() {
        assert_eq!(capture("string.special.uri"), Capture::Class(Class::Link));
        assert_eq!(
            capture("string.special.path"),
            Capture::Class(Class::String)
        );
        assert_eq!(capture("string.escape"), Capture::Class(Class::Escape));
        assert_eq!(capture("function.macro"), Capture::Class(Class::Macro));
        assert_eq!(
            capture("function.method.builtin"),
            Capture::Class(Class::Function)
        );
        assert_eq!(
            capture("keyword.control.return"),
            Capture::Class(Class::Keyword)
        );
        assert_eq!(capture("tag.attribute"), Capture::Class(Class::Attribute));
        assert_eq!(capture("tag.error"), Capture::Class(Class::Tag));
        assert_eq!(capture("text.title"), Capture::Class(Class::Heading));
    }

    #[test]
    fn unknown_and_reset_captures_emit_no_class() {
        assert_eq!(capture("spell"), Capture::Transparent);
        assert_eq!(capture("text.emphasis"), Capture::Transparent);
        assert_eq!(capture("stringy"), Capture::Transparent);
        assert_eq!(capture("_private"), Capture::Transparent);
        assert_eq!(capture("embedded"), Capture::Plain);
        assert_eq!(capture("none"), Capture::Plain);
    }
}
