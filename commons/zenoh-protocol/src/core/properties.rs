//
// Copyright (c) 2022 ZettaScale Technology
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ZettaScale Zenoh Team, <zenoh@zettascale.tech>
//
use alloc::borrow::Cow;
use core::{borrow::Borrow, fmt};
#[cfg(feature = "std")]
use std::collections::HashMap;

const LIST_SEPARATOR: char = ';';
const FIELD_SEPARATOR: char = '=';
const VALUE_SEPARATOR: char = '|';

fn split_once(s: &str, c: char) -> (&str, &str) {
    match s.find(c) {
        Some(index) => {
            let (l, r) = s.split_at(index);
            (l, &r[1..])
        }
        None => (s, ""),
    }
}

/// A map of key/value (String,String) properties.
/// It can be parsed from a String, using `;` or `<newline>` as separator between each properties
/// and `=` as separator between a key and its value. Keys and values are trimed.
///
/// Example:
/// ```
/// use zenoh_collections::Properties;
///
/// let a = "a=1;b=2;c=3|4|5;d=6";
/// let p = Properties::from(a);
///
/// // Retrieve values
/// assert!(!p.is_empty());
/// assert_eq!(p.get("a").unwrap(), "1");
/// assert_eq!(p.get("b").unwrap(), "2");
/// assert_eq!(p.get("c").unwrap(), "3|4|5");
/// assert_eq!(p.get("d").unwrap(), "6");
/// assert_eq!(p.values("c").collect::<Vec<&str>>(), vec!["3", "4", "5"]);
///
/// // Iterate over properties
/// let mut iter = p.iter();
/// assert_eq!(iter.next().unwrap(), ("a", "1"));
/// assert_eq!(iter.next().unwrap(), ("b", "2"));
/// assert_eq!(iter.next().unwrap(), ("c", "3|4|5"));
/// assert_eq!(iter.next().unwrap(), ("d", "6"));
/// assert!(iter.next().is_none());
///
/// // Create properties from iterators
/// let pi = Properties::from_iter(vec![("a", "1"), ("b", "2"), ("c", "3|4|5"), ("d", "6")]);
/// assert_eq!(p, pi);
/// ```
#[derive(Clone, PartialEq, Eq, Default)]
pub struct Properties<'s>(Cow<'s, str>);

impl Properties<'_> {
    /// Returns `true` if properties does not contain anything.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns properties as [`str`].
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns `true` if properties contains the specified key.
    pub fn contains_key<K>(&self, k: K) -> bool
    where
        K: Borrow<str>,
    {
        self.get(k).is_some()
    }

    /// Returns a reference to the `&str`-value corresponding to the key.
    pub fn get<K>(&self, k: K) -> Option<&str>
    where
        K: Borrow<str>,
    {
        self.iter()
            .find(|(key, _)| *key == k.borrow())
            .map(|(_, value)| value)
    }

    /// Returns an iterator to the `&str`-values corresponding to the key.
    pub fn values<K>(&self, k: K) -> impl DoubleEndedIterator<Item = &str>
    where
        K: Borrow<str>,
    {
        match self.get(k) {
            Some(v) => v.split(VALUE_SEPARATOR),
            None => {
                let mut i = "".split(VALUE_SEPARATOR);
                i.next();
                i
            }
        }
    }

    /// Returns an iterator on the key-value pairs as `(&str, &str)`.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (&str, &str)> + Clone {
        self.as_str()
            .split(LIST_SEPARATOR)
            .filter(|p| !p.is_empty())
            .map(|p| split_once(p, FIELD_SEPARATOR))
    }

    /// Inserts a key-value pair into the map.
    /// If the map did not have this key present, [`None`]` is returned.
    /// If the map did have this key present, the value is updated, and the old value is returned.
    pub fn insert<K, V>(&mut self, k: K, v: V) -> Option<String>
    where
        K: Borrow<str>,
        V: Borrow<str>,
    {
        let item = self
            .iter()
            .find(|(key, _)| *key == k.borrow())
            .map(|(_, v)| v.to_string());

        let current = self.iter().filter(|x| x.0 != k.borrow());
        let new = Some((k.borrow(), v.borrow())).into_iter();
        let iter = current.chain(new);

        *self = Self::from_iter(iter);
        item
    }

    /// Removes a key from the map, returning the value at the key if the key was previously in the properties.    
    pub fn remove<K>(&mut self, k: K) -> Option<String>
    where
        K: Borrow<str>,
    {
        let item = self
            .iter()
            .find(|(key, _)| *key == k.borrow())
            .map(|(_, v)| v.to_string());
        let iter = self.iter().filter(|x| x.0 != k.borrow());

        *self = Self::from_iter(iter);
        item
    }

    /// Extend these properties with other properties.
    pub fn extend(&mut self, other: &Properties) {
        self.extend_from_iter(other.iter());
    }

    /// Extend these properties from an iterator.
    pub fn extend_from_iter<'s, I, K, V>(&mut self, iter: I)
    where
        I: Iterator<Item = (&'s K, &'s V)> + Clone,
        K: Borrow<str> + 's + ?Sized,
        V: Borrow<str> + 's + ?Sized,
    {
        let new: I = iter.clone();
        let current = self
            .iter()
            .filter(|(kc, _)| !new.clone().any(|(kn, _)| *kc == kn.borrow()));
        let iter = current.chain(iter.map(|(k, v)| (k.borrow(), v.borrow())));

        *self = Self::from_iter(iter);
    }

    /// Convert these properties into owned properties.
    pub fn into_owned(self) -> Properties<'static> {
        Properties(Cow::Owned(self.0.into_owned()))
    }
}

impl<'s> From<&'s str> for Properties<'s> {
    fn from(mut value: &'s str) -> Self {
        value = value.trim_end_matches(|c| {
            c == LIST_SEPARATOR || c == FIELD_SEPARATOR || c == VALUE_SEPARATOR
        });
        Self(Cow::Borrowed(value))
    }
}

impl From<String> for Properties<'_> {
    fn from(mut value: String) -> Self {
        let s = value.trim_end_matches(|c| {
            c == LIST_SEPARATOR || c == FIELD_SEPARATOR || c == VALUE_SEPARATOR
        });
        value.truncate(s.len());
        Self(Cow::Owned(value))
    }
}

impl<'s> From<Cow<'s, str>> for Properties<'s> {
    fn from(value: Cow<'s, str>) -> Self {
        match value {
            Cow::Borrowed(s) => Properties::from(s),
            Cow::Owned(s) => Properties::from(s),
        }
    }
}

impl<'s, K, V> FromIterator<(&'s K, &'s V)> for Properties<'_>
where
    K: Borrow<str> + 's + ?Sized,
    V: Borrow<str> + 's + ?Sized,
{
    fn from_iter<T: IntoIterator<Item = (&'s K, &'s V)>>(iter: T) -> Self {
        fn concat<'s, I>(iter: I) -> String
        where
            I: Iterator<Item = (&'s str, &'s str)>,
        {
            let mut into = String::new();
            let mut first = true;
            for (k, v) in iter.filter(|(k, _)| !k.is_empty()) {
                if !first {
                    into.push(LIST_SEPARATOR);
                }
                into.push_str(k);
                if !v.is_empty() {
                    into.push(FIELD_SEPARATOR);
                    into.push_str(v);
                }
                first = false;
            }
            into
        }

        let iter = iter.into_iter();
        let inner = concat(iter.map(|(k, v)| (k.borrow(), v.borrow())));

        Self(Cow::Owned(inner))
    }
}

impl<'s, K, V> FromIterator<&'s (K, V)> for Properties<'_>
where
    K: Borrow<str> + 's,
    V: Borrow<str> + 's,
{
    fn from_iter<T: IntoIterator<Item = &'s (K, V)>>(iter: T) -> Self {
        Self::from_iter(iter.into_iter().map(|(k, v)| (k.borrow(), v.borrow())))
    }
}

impl<'s, K, V> From<&'s [(K, V)]> for Properties<'_>
where
    K: Borrow<str> + 's,
    V: Borrow<str> + 's,
{
    fn from(value: &'s [(K, V)]) -> Self {
        Self::from_iter(value.iter())
    }
}

#[cfg(feature = "std")]
impl<K, V> From<HashMap<K, V>> for Properties<'_>
where
    K: Borrow<str>,
    V: Borrow<str>,
{
    fn from(map: HashMap<K, V>) -> Self {
        Self::from_iter(map.iter())
    }
}

#[cfg(feature = "std")]
impl<'s> From<&'s Properties<'s>> for HashMap<&'s str, &'s str> {
    fn from(props: &'s Properties<'s>) -> Self {
        HashMap::from_iter(props.iter())
    }
}

#[cfg(feature = "std")]
impl From<&Properties<'_>> for HashMap<String, String> {
    fn from(props: &Properties<'_>) -> Self {
        HashMap::from_iter(props.iter().map(|(k, v)| (k.to_string(), v.to_string())))
    }
}

#[cfg(feature = "std")]
impl<'s> From<&'s Properties<'s>> for HashMap<Cow<'s, str>, Cow<'s, str>> {
    fn from(props: &'s Properties<'s>) -> Self {
        HashMap::from_iter(props.iter().map(|(k, v)| (Cow::from(k), Cow::from(v))))
    }
}

#[cfg(feature = "std")]
impl From<Properties<'_>> for HashMap<String, String> {
    fn from(props: Properties) -> Self {
        HashMap::from(&props)
    }
}

impl fmt::Display for Properties<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Debug for Properties<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_properties() {
        assert!(Properties::from("").0.is_empty());

        assert_eq!(Properties::from("p1"), Properties::from(&[("p1", "")][..]));

        assert_eq!(
            Properties::from("p1=v1"),
            Properties::from(&[("p1", "v1")][..])
        );

        assert_eq!(
            Properties::from("p1=v1;p2=v2;"),
            Properties::from(&[("p1", "v1"), ("p2", "v2")][..])
        );

        assert_eq!(
            Properties::from("p1=v1;p2=v2;|="),
            Properties::from(&[("p1", "v1"), ("p2", "v2")][..])
        );

        assert_eq!(
            Properties::from("p1=v1;p2;p3=v3"),
            Properties::from(&[("p1", "v1"), ("p2", ""), ("p3", "v3")][..])
        );

        assert_eq!(
            Properties::from("p1=v 1;p 2=v2"),
            Properties::from(&[("p1", "v 1"), ("p 2", "v2")][..])
        );

        assert_eq!(
            Properties::from("p1=x=y;p2=a==b"),
            Properties::from(&[("p1", "x=y"), ("p2", "a==b")][..])
        );

        let mut hm: HashMap<String, String> = HashMap::new();
        hm.insert("p1".to_string(), "v1".to_string());
        assert_eq!(Properties::from(hm), Properties::from("p1=v1"));

        let mut hm: HashMap<&str, &str> = HashMap::new();
        hm.insert("p1", "v1");
        assert_eq!(Properties::from(hm), Properties::from("p1=v1"));

        let mut hm: HashMap<Cow<str>, Cow<str>> = HashMap::new();
        hm.insert(Cow::from("p1"), Cow::from("v1"));
        assert_eq!(Properties::from(hm), Properties::from("p1=v1"));
    }
}
