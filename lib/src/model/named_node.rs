use oxiri::{Iri, IriParseError};
use rio_api::model as rio;
use std::borrow::Borrow;
use std::borrow::Cow;
use std::fmt;
use std::ops::Deref;
use std::str::FromStr;

/// A RDF [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri).
///
/// The default string formatter is returning a N-Triples, Turtle and SPARQL compatible representation:
/// ```
/// use oxigraph::model::NamedNode;
///
/// assert_eq!(
///     "<http://example.com/foo>",
///     NamedNode::parse("http://example.com/foo").unwrap().to_string()
/// )
/// ```
///
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Hash)]
#[repr(transparent)]
pub struct NamedNode {
    iri: str,
}

impl NamedNode {
    /// Builds and validate a RDF [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri)
    #[inline]
    pub fn parse(iri: &str) -> Result<&Self, IriParseError> {
        Ok(Self::new_unchecked(Iri::parse(iri)?.into_inner()))
    }

    /// Builds a RDF [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri) from a string.
    ///
    /// It is the caller's responsibility to ensure that `iri` is a valid IRI.
    ///
    /// Except if you really know what you do, you should use [`parse`](#method.parse).
    #[inline]
    #[allow(unsafe_code)]
    pub fn new_unchecked(iri: &str) -> &Self {
        // TODO: find a way to do such conversion between transparent values
        let iri: *const str = iri;
        unsafe { &*(iri as *const Self) }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.iri
    }
}

impl<'a> From<&'a NamedNode> for rio::NamedNode<'a> {
    #[inline]
    fn from(node: &'a NamedNode) -> Self {
        rio::NamedNode { iri: node.as_str() }
    }
}

impl<'a> From<&'a NamedNode> for Cow<'a, NamedNode> {
    #[inline]
    fn from(node: &'a NamedNode) -> Self {
        Cow::Borrowed(node)
    }
}

impl fmt::Display for NamedNode {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        rio::NamedNode::from(self).fmt(f)
    }
}

impl PartialEq<str> for NamedNode {
    fn eq(&self, other: &str) -> bool {
        self.iri == *other
    }
}

impl PartialEq<NamedNode> for str {
    fn eq(&self, other: &NamedNode) -> bool {
        *self == other.iri
    }
}

impl<'a> PartialEq<Cow<'a, NamedNode>> for NamedNode {
    fn eq(&self, other: &Cow<'a, NamedNode>) -> bool {
        self.iri == other.iri
    }
}

impl<'a> PartialEq<NamedNode> for Cow<'a, NamedNode> {
    fn eq(&self, other: &NamedNode) -> bool {
        self.iri == other.iri
    }
}

/// A owned RDF [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri).
///
/// The default string formatter is returning a N-Triples, Turtle and SPARQL compatible representation:
/// ```
/// use oxigraph::model::NamedNodeBuf;
///
/// assert_eq!(
///     "<http://example.com/foo>",
///     NamedNodeBuf::parse("http://example.com/foo").unwrap().to_string()
/// )
/// ```
///
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct NamedNodeBuf {
    iri: String,
}

impl NamedNodeBuf {
    /// Builds and validate a RDF [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri)
    #[inline]
    pub fn parse(iri: impl Into<String>) -> Result<Self, IriParseError> {
        Ok(Self {
            iri: Iri::parse(iri.into())?.into_inner(),
        })
    }

    /// Builds a RDF [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri) from a string.
    ///
    /// It is the caller's responsibility to ensure that `iri` is a valid IRI.
    ///
    /// Except if you really know what you do, you should use [`parse`](#method.parse).
    #[inline]
    pub fn new_unchecked(iri: impl Into<String>) -> Self {
        Self { iri: iri.into() }
    }

    #[inline]
    pub fn into_string(self) -> String {
        self.iri
    }
}

impl AsRef<NamedNode> for NamedNodeBuf {
    #[inline]
    fn as_ref(&self) -> &NamedNode {
        NamedNode::new_unchecked(&self.iri)
    }
}

impl Deref for NamedNodeBuf {
    type Target = NamedNode;
    #[inline]
    fn deref(&self) -> &NamedNode {
        self.as_ref()
    }
}

impl Borrow<NamedNode> for NamedNodeBuf {
    #[inline]
    fn borrow(&self) -> &NamedNode {
        self.as_ref()
    }
}

impl ToOwned for NamedNode {
    type Owned = NamedNodeBuf;

    #[inline]
    fn to_owned(&self) -> NamedNodeBuf {
        NamedNodeBuf::new_unchecked(self.iri.to_owned())
    }
}

impl From<Iri<String>> for NamedNodeBuf {
    #[inline]
    fn from(iri: Iri<String>) -> Self {
        Self::new_unchecked(iri.into_inner())
    }
}

impl<'a> From<NamedNodeBuf> for Cow<'a, NamedNode> {
    #[inline]
    fn from(node: NamedNodeBuf) -> Self {
        Cow::Owned(node)
    }
}

impl FromStr for NamedNodeBuf {
    type Err = IriParseError;
    #[inline]
    fn from_str(s: &str) -> Result<Self, IriParseError> {
        Self::parse(s.to_string())
    }
}

impl fmt::Display for NamedNodeBuf {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_ref().fmt(f)
    }
}

impl PartialEq<NamedNode> for NamedNodeBuf {
    fn eq(&self, other: &NamedNode) -> bool {
        self.as_ref() == other
    }
}

impl PartialEq<NamedNodeBuf> for NamedNode {
    fn eq(&self, other: &NamedNodeBuf) -> bool {
        self == other.as_ref()
    }
}

impl PartialEq<str> for NamedNodeBuf {
    fn eq(&self, other: &str) -> bool {
        self.as_ref() == other
    }
}

impl PartialEq<NamedNodeBuf> for str {
    fn eq(&self, other: &NamedNodeBuf) -> bool {
        self == other.as_ref()
    }
}

impl<'a> PartialEq<Cow<'a, NamedNode>> for NamedNodeBuf {
    fn eq(&self, other: &Cow<'a, NamedNode>) -> bool {
        self.as_ref() == other
    }
}

impl<'a> PartialEq<NamedNodeBuf> for Cow<'a, NamedNode> {
    fn eq(&self, other: &NamedNodeBuf) -> bool {
        self == other.as_ref()
    }
}
